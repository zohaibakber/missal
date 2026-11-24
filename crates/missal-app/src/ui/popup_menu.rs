use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::AnimationExt;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Anchor, AnyElement, App, AppContext, Bounds, ClickEvent, Context, Entity, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Pixels, Point, Render, RenderOnce, SharedString, Styled, Window, anchored,
    canvas, deferred, div, point, px,
};

use crate::shortcuts::{BlurFocus, PreviewClose};
use crate::theme::MENU_ITEM;
use crate::theme::{Palette, motion_fast};
use crate::ui::menu::{
    menu_group_label, menu_header, menu_header_detail, menu_item, menu_item_element, menu_popup,
    menu_separator,
};
use crate::ui::{ActionButton, IconButton, IconSegment, UrRole, icon_segmented, ur_label};

pub struct Dismissed;

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
enum Label {
    Text(SharedString),
    Urdu(SharedString),
}

#[derive(Clone)]
pub struct MenuEntry {
    id: SharedString,
    label: Label,
    icon: Option<IconName>,
    check: Option<bool>,
    keys: Option<SharedString>,
    count: Option<usize>,
    dot: Option<gpui_kit::Hsla>,
    danger: bool,
    disabled: bool,
    keep_open: bool,
    action: Action,
}

impl MenuEntry {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> MenuEntry {
        MenuEntry {
            id: id.into(),
            label: Label::Text(label.into()),
            icon: None,
            check: None,
            keys: None,
            count: None,
            dot: None,
            danger: false,
            disabled: false,
            keep_open: false,
            action: Rc::new(action),
        }
    }

    pub fn urdu(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> MenuEntry {
        MenuEntry {
            label: Label::Urdu(label.into()),
            ..MenuEntry::new(id, "", action)
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn check(mut self, checked: bool) -> Self {
        self.check = Some(checked);
        self
    }

    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    pub fn dot(mut self, color: gpui_kit::Hsla) -> Self {
        self.dot = Some(color);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn keep_open(mut self) -> Self {
        self.keep_open = true;
        self
    }
}

#[derive(Clone)]
pub struct MenuSwitchOption {
    id: SharedString,
    icon: IconName,
    label: SharedString,
    action: Action,
}

impl MenuSwitchOption {
    pub fn new(
        id: impl Into<SharedString>,
        icon: IconName,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> MenuSwitchOption {
        MenuSwitchOption {
            id: id.into(),
            icon,
            label: label.into(),
            action: Rc::new(action),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Placement {
    pub position: Point<Pixels>,
    pub anchor: Anchor,
    pub width: f32,
}

#[derive(Clone)]
enum Row {
    Entry(MenuEntry),
    Label(SharedString),
    Header(SharedString, Option<SharedString>),
    Switch {
        label: SharedString,
        options: Vec<MenuSwitchOption>,
        selected: usize,
    },
    Separator,
}

#[derive(Clone, Default)]
pub struct MenuContent {
    rows: Vec<Row>,
}

impl FluentBuilder for MenuContent {}

impl MenuContent {
    pub fn new() -> Self {
        MenuContent::default()
    }

    pub fn item(mut self, entry: MenuEntry) -> Self {
        self.rows.push(Row::Entry(entry));
        self
    }

    pub fn items(mut self, entries: impl IntoIterator<Item = MenuEntry>) -> Self {
        self.rows.extend(entries.into_iter().map(Row::Entry));
        self
    }

    pub fn separator(mut self) -> Self {
        self.rows.push(Row::Separator);
        self
    }

    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.rows.push(Row::Label(text.into()));
        self
    }

    pub fn header(mut self, title: impl Into<SharedString>, detail: Option<SharedString>) -> Self {
        self.rows.push(Row::Header(title.into(), detail));
        self
    }

    pub fn switch(
        mut self,
        label: impl Into<SharedString>,
        options: Vec<MenuSwitchOption>,
        selected: usize,
    ) -> Self {
        self.rows.push(Row::Switch {
            label: label.into(),
            options,
            selected,
        });
        self
    }

    fn into_rows(self) -> Vec<Row> {
        let mut rows: Vec<Row> = Vec::with_capacity(self.rows.len());
        for row in self.rows {
            let separator = matches!(row, Row::Separator);
            if separator
                && rows
                    .last()
                    .is_none_or(|last| matches!(last, Row::Separator))
            {
                continue;
            }
            rows.push(row);
        }
        if matches!(rows.last(), Some(Row::Separator)) {
            rows.pop();
        }
        rows
    }
}

pub struct PopupList {
    id: SharedString,
    focus: FocusHandle,
    rows: Vec<Row>,
    highlighted: Option<usize>,
    typed: String,
    typed_at: Option<std::time::Instant>,
    position: Point<Pixels>,
    corner: Anchor,
    width: f32,
}

const TYPEAHEAD_RESET: std::time::Duration = std::time::Duration::from_millis(700);

impl EventEmitter<Dismissed> for PopupList {}

impl Focusable for PopupList {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PopupList {
    pub fn open(
        id: impl Into<SharedString>,
        content: MenuContent,
        placement: Placement,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<PopupList> {
        let Placement {
            position,
            anchor: corner,
            width,
        } = placement;
        let highlight_first = window.last_input_was_keyboard();
        let list = cx.new(|cx| {
            let mut list = PopupList {
                id: id.into(),
                focus: cx.focus_handle(),
                rows: content.into_rows(),
                highlighted: None,
                typed: String::new(),
                typed_at: None,
                position,
                corner,
                width,
            };
            if highlight_first {
                let first = list.entries().next();
                list.highlighted = first;
            }
            list
        });
        let focus = list.read(cx).focus.clone();
        window.focus(&focus, cx);
        list
    }

    fn entries(&self) -> impl Iterator<Item = usize> + '_ {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| matches!(row, Row::Entry(entry) if !entry.disabled))
            .map(|(index, _)| index)
    }

    pub fn labels(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter_map(|row| match row {
                Row::Entry(entry) => Some(match &entry.label {
                    Label::Text(text) | Label::Urdu(text) => text.to_string(),
                }),
                Row::Label(..) | Row::Header(..) | Row::Switch { .. } | Row::Separator => None,
            })
            .collect()
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let entries: Vec<usize> = self.entries().collect();
        if entries.is_empty() {
            return;
        }
        let count = entries.len() as isize;
        let current = self
            .highlighted
            .and_then(|row| entries.iter().position(|entry| *entry == row));
        let next = match current {
            Some(position) => (position as isize + delta).rem_euclid(count),
            None if delta > 0 => 0,
            None => count - 1,
        };
        self.highlighted = Some(entries[next as usize]);
        cx.notify();
    }

    fn run(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Row::Entry(entry)) = self.rows.get_mut(row) else {
            return;
        };
        if entry.disabled {
            return;
        }
        let action = entry.action.clone();
        let keep_open = entry.keep_open;
        if keep_open && let Some(checked) = entry.check.as_mut() {
            *checked = !*checked;
        }
        if !keep_open {
            cx.emit(Dismissed);
        }
        cx.notify();
        action(window, cx);
    }

    fn choose_option(
        &mut self,
        row: usize,
        option: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Row::Switch {
            options, selected, ..
        }) = self.rows.get_mut(row)
        else {
            return;
        };
        let Some(action) = options.get(option).map(|entry| entry.action.clone()) else {
            return;
        };
        if *selected == option {
            return;
        }
        *selected = option;
        cx.notify();
        action(window, cx);
    }

    fn shift_switch(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some((row, count, selected)) =
            self.rows
                .iter()
                .enumerate()
                .find_map(|(index, row)| match row {
                    Row::Switch {
                        options, selected, ..
                    } => Some((index, options.len() as isize, *selected as isize)),
                    _ => None,
                })
        else {
            return false;
        };
        let next = (selected + delta).rem_euclid(count) as usize;
        self.choose_option(row, next, window, cx);
        true
    }

    fn switch_row(
        &self,
        row: usize,
        text: &SharedString,
        options: &[MenuSwitchOption],
        selected: usize,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = options
            .iter()
            .enumerate()
            .map(|(option, entry)| IconSegment {
                id: entry.id.clone().into(),
                icon: entry.icon,
                label: entry.label.clone(),
                on_select: Rc::new(cx.listener(move |this, _, window, cx| {
                    this.choose_option(row, option, window, cx)
                })),
            })
            .collect();
        gpui_kit::component::h_flex()
            .h(px(MENU_ITEM + 4.))
            .px(px(8.))
            .items_center()
            .justify_between()
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(palette.ink)
            .child(text.clone())
            .child(icon_segmented(items, selected, palette))
            .into_any_element()
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "left" if self.shift_switch(-1, window, cx) => {}
            "right" if self.shift_switch(1, window, cx) => {}
            "down" => self.step(1, cx),
            "up" => self.step(-1, cx),
            "home" => {
                self.highlighted = None;
                self.step(1, cx);
            }
            "end" => {
                self.highlighted = None;
                self.step(-1, cx);
            }
            "space" if self.typeahead_active() => {
                if !self.typeahead(' ', cx) {
                    return;
                }
            }
            "enter" | "space" => {
                if let Some(row) = self.highlighted {
                    self.run(row, window, cx);
                }
            }
            "escape" | "tab" | "f10" => cx.emit(Dismissed),
            other => {
                let Some(letter) = other.chars().next().filter(|_| {
                    other.chars().count() == 1 && !event.keystroke.modifiers.modified()
                }) else {
                    return;
                };
                if !self.typeahead(letter, cx) {
                    return;
                }
            }
        }
        cx.stop_propagation();
    }

    fn typeahead_active(&self) -> bool {
        !self.typed.is_empty()
            && self
                .typed_at
                .is_some_and(|at| at.elapsed() <= TYPEAHEAD_RESET)
    }

    fn typeahead(&mut self, letter: char, cx: &mut Context<Self>) -> bool {
        if self
            .typed_at
            .is_none_or(|at| at.elapsed() > TYPEAHEAD_RESET)
        {
            self.typed.clear();
        }
        self.typed_at = Some(std::time::Instant::now());
        self.typed.extend(letter.to_lowercase());
        let first = self.typed.chars().next();
        let repeated = self.typed.chars().all(|typed| Some(typed) == first);
        let query = if repeated {
            self.typed.chars().take(1).collect::<String>()
        } else {
            self.typed.clone()
        };
        let entries: Vec<usize> = self.entries().collect();
        if entries.is_empty() {
            return false;
        }
        let position = self
            .highlighted
            .and_then(|row| entries.iter().position(|entry| *entry == row));
        let start = match position {
            Some(position) if repeated => position + 1,
            Some(position) => position,
            None => 0,
        };
        let found = (0..entries.len())
            .map(|step| entries[(start + step) % entries.len()])
            .find(|row| match &self.rows[*row] {
                Row::Entry(entry) => match &entry.label {
                    Label::Text(text) | Label::Urdu(text) => {
                        text.to_lowercase().starts_with(&query)
                    }
                },
                Row::Label(..) | Row::Header(..) | Row::Switch { .. } | Row::Separator => false,
            });
        let Some(row) = found else {
            return false;
        };
        self.highlighted = Some(row);
        cx.notify();
        true
    }

    fn item(&self, index: usize, entry: &MenuEntry, cx: &mut Context<Self>) -> AnyElement {
        let dot = entry.dot.map(|color| {
            div()
                .flex_none()
                .size(px(crate::ui::status::DOT))
                .rounded_full()
                .bg(color)
        });
        let content: AnyElement = match &entry.label {
            Label::Text(text) => div().child(text.clone()).into_any_element(),
            Label::Urdu(text) => gpui_kit::component::h_flex()
                .flex_row_reverse()
                .items_center()
                .gap(px(8.))
                .children(dot)
                .child(
                    div()
                        .mt(px(-2.))
                        .child(ur_label(text.clone(), UrRole::Body)),
                )
                .into_any_element(),
        };
        let mut item = match &entry.label {
            Label::Text(text) => menu_item(entry.id.clone(), text.clone()),
            Label::Urdu(_) => menu_item_element(entry.id.clone(), content).rtl(),
        };
        if let Some(icon) = entry.icon {
            item = item.icon(icon);
        }
        if let Some(checked) = entry.check {
            item = item.check(checked);
        }
        if let Some(keys) = &entry.keys {
            item = item.keys(keys.clone());
        }
        if let Some(count) = entry.count {
            item = item.trailing_count(count);
        }
        if entry.danger {
            item = item.danger();
        }
        item = item.disabled(entry.disabled);
        let weak = cx.weak_entity();
        item.highlighted(self.highlighted == Some(index))
            .on_hover(move |_, cx| {
                weak.update(cx, |this, cx| {
                    if this.highlighted != Some(index) {
                        this.highlighted = Some(index);
                        cx.notify();
                    }
                })
                .ok();
            })
            .on_click(cx.listener(move |this, _, window, cx| this.run(index, window, cx)))
            .into_any_element()
    }
}

impl Render for PopupList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let rows: Vec<Row> = self.rows.clone();
        let children: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| match row {
                Row::Entry(entry) => self.item(index, entry, cx),
                Row::Label(text) => menu_group_label().child(text.clone()).into_any_element(),
                Row::Header(title, detail) => menu_header()
                    .child(title.clone())
                    .children(
                        detail
                            .clone()
                            .map(|detail| menu_header_detail().child(detail)),
                    )
                    .into_any_element(),
                Row::Switch {
                    label,
                    options,
                    selected,
                } => self.switch_row(index, label, options, *selected, &palette, cx),
                Row::Separator => menu_separator().into_any_element(),
            })
            .collect();
        let rises = matches!(self.corner, Anchor::BottomLeft | Anchor::BottomRight);
        deferred(
            anchored()
                .position(self.position)
                .anchor(self.corner)
                .snap_to_window_with_margin(px(8.))
                .child(
                    menu_popup()
                        .id(self.id.clone())
                        .key_context("PopupList TextInput")
                        .on_action(cx.listener(|_, _: &BlurFocus, _, cx| cx.emit(Dismissed)))
                        .on_action(cx.listener(|_, _: &PreviewClose, _, cx| cx.emit(Dismissed)))
                        .track_focus(&self.focus)
                        .w(px(self.width))
                        .occlude()
                        .on_key_down(cx.listener(Self::on_key))
                        .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(Dismissed)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .children(children)
                        .with_animation(
                            SharedString::from(format!("{}-enter", self.id)),
                            motion_fast(),
                            move |this, delta| {
                                let shift = 4. * (1. - delta);
                                this.opacity(delta)
                                    .mt(px(if rises { shift } else { -shift }))
                            },
                        ),
                ),
        )
        // Above dialogs (10 + layer) so a menu opened inside one is visible.
        .with_priority(100)
    }
}

pub trait MenuTrigger: IntoElement + 'static {
    fn open(self, open: bool) -> Self;

    fn on_activate(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self;
}

impl MenuTrigger for IconButton {
    fn open(self, open: bool) -> Self {
        self.toggled(open)
    }

    fn on_activate(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click(handler)
    }
}

impl MenuTrigger for ActionButton {
    fn open(self, open: bool) -> Self {
        if open { self.show_hover() } else { self }
    }

    fn on_activate(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click(handler)
    }
}

type ContentBuilder = Rc<dyn Fn(MenuContent, &mut Window, &mut App) -> MenuContent>;

struct MenuState {
    list: Option<Entity<PopupList>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    swallow_click: bool,
}

#[derive(IntoElement)]
pub struct Menu<T: MenuTrigger> {
    id: SharedString,
    trigger: Option<T>,
    content: Option<ContentBuilder>,
    anchor: Anchor,
    width: f32,
}

pub fn menu<T: MenuTrigger>(id: impl Into<SharedString>) -> Menu<T> {
    Menu {
        id: id.into(),
        trigger: None,
        content: None,
        anchor: Anchor::TopRight,
        width: 224.,
    }
}

impl<T: MenuTrigger> Menu<T> {
    pub fn trigger(mut self, trigger: T) -> Self {
        self.trigger = Some(trigger);
        self
    }

    pub fn content(
        mut self,
        content: impl Fn(MenuContent, &mut Window, &mut App) -> MenuContent + 'static,
    ) -> Self {
        self.content = Some(Rc::new(content));
        self
    }

    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl<T: MenuTrigger> RenderOnce for Menu<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(
            SharedString::from(format!("{}-state", self.id)),
            cx,
            |_, _| MenuState {
                list: None,
                bounds: Rc::new(Cell::new(Bounds::default())),
                swallow_click: false,
            },
        );
        let (list, bounds) = {
            let state = state.read(cx);
            (state.list.clone(), state.bounds.clone())
        };
        let painted = bounds.clone();
        let Menu {
            id,
            trigger,
            content,
            anchor,
            width,
        } = self;
        let press = state.clone();
        let click = state.clone();
        let close = |state: &Entity<MenuState>, cx: &mut App| {
            state.update(cx, |state, cx| {
                state.list = None;
                cx.notify();
            })
        };
        let trigger = trigger.map(|trigger| {
            trigger
                .open(list.is_some())
                .on_activate(move |_, window, cx| {
                    let swallow = click.update(cx, |state, _| {
                        std::mem::take(&mut state.swallow_click) || state.list.is_some()
                    });
                    if swallow {
                        close(&click, cx);
                        return;
                    }
                    let Some(content) = content.clone() else {
                        return;
                    };
                    let area = bounds.get();
                    let position = match anchor {
                        Anchor::TopRight => point(area.right(), area.bottom() + px(4.)),
                        Anchor::BottomLeft => point(area.left(), area.top() - px(4.)),
                        Anchor::BottomRight => point(area.right(), area.top() - px(4.)),
                        _ => point(area.left(), area.bottom() + px(4.)),
                    };
                    let rows = content(MenuContent::new(), window, cx);
                    let list = PopupList::open(
                        format!("{id}-menu"),
                        rows,
                        Placement {
                            position,
                            anchor,
                            width,
                        },
                        window,
                        cx,
                    );
                    let dismiss = click.clone();
                    cx.subscribe(&list, move |_, _: &Dismissed, cx| close(&dismiss, cx))
                        .detach();
                    click.update(cx, |state, cx| {
                        state.list = Some(list);
                        cx.notify();
                    });
                })
        });
        div()
            .relative()
            .flex_none()
            .capture_any_mouse_down(move |_, _, cx| {
                press.update(cx, |state, _| state.swallow_click = state.list.is_some());
            })
            .children(trigger)
            .child(
                canvas(move |area, _, _| painted.set(area), |_, _, _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
            .children(list)
    }
}
