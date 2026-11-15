use std::rc::Rc;

use gpui_kit::AnimationExt;
use gpui_kit::assets::IconName;
use gpui_kit::{
    Anchor, AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels, Point,
    Render, SharedString, Styled, Window, anchored, deferred, div, px,
};

use crate::shortcuts::{BlurFocus, PreviewClose};
use crate::theme::{Palette, motion_fast};
use crate::ui::menu::{menu_item, menu_item_element, menu_panel, menu_separator};
use crate::ui::{UrRole, ur_inline};

pub struct Dismissed;

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
enum Label {
    Text(SharedString),
    Urdu(SharedString),
}

#[derive(Clone)]
pub struct PopupEntry {
    id: SharedString,
    label: Label,
    icon: Option<IconName>,
    check: Option<bool>,
    keys: Option<SharedString>,
    danger: bool,
    keep_open: bool,
    action: Action,
}

impl PopupEntry {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> PopupEntry {
        PopupEntry {
            id: id.into(),
            label: Label::Text(label.into()),
            icon: None,
            check: None,
            keys: None,
            danger: false,
            keep_open: false,
            action: Rc::new(action),
        }
    }

    pub fn urdu(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> PopupEntry {
        PopupEntry {
            label: Label::Urdu(label.into()),
            ..PopupEntry::new(id, "", action)
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

    pub fn keep_open(mut self) -> Self {
        self.keep_open = true;
        self
    }
}

#[derive(Clone, Copy)]
pub struct Placement {
    pub position: Point<Pixels>,
    pub anchor: Anchor,
    pub width: f32,
}

#[derive(Clone)]
pub enum PopupRow {
    Entry(PopupEntry),
    Separator,
}

pub struct PopupList {
    id: SharedString,
    focus: FocusHandle,
    rows: Vec<PopupRow>,
    highlighted: Option<usize>,
    position: Point<Pixels>,
    corner: Anchor,
    width: f32,
}

impl EventEmitter<Dismissed> for PopupList {}

impl Focusable for PopupList {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PopupList {
    pub fn open(
        id: impl Into<SharedString>,
        rows: Vec<PopupRow>,
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
                rows,
                highlighted: None,
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
            .filter(|(_, row)| matches!(row, PopupRow::Entry(_)))
            .map(|(index, _)| index)
    }

    pub fn labels(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter_map(|row| match row {
                PopupRow::Entry(entry) => Some(match &entry.label {
                    Label::Text(text) | Label::Urdu(text) => text.to_string(),
                }),
                PopupRow::Separator => None,
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
        let Some(PopupRow::Entry(entry)) = self.rows.get_mut(row) else {
            return;
        };
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

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
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
                let entries: Vec<usize> = self.entries().collect();
                let start = self
                    .highlighted
                    .and_then(|row| entries.iter().position(|entry| *entry == row))
                    .map_or(0, |position| position + 1);
                let found = (0..entries.len())
                    .map(|step| entries[(start + step) % entries.len()])
                    .find(|row| match &self.rows[*row] {
                        PopupRow::Entry(entry) => match &entry.label {
                            Label::Text(text) | Label::Urdu(text) => {
                                text.chars().next().is_some_and(|first| {
                                    first.to_lowercase().eq(letter.to_lowercase())
                                })
                            }
                        },
                        PopupRow::Separator => false,
                    });
                if let Some(row) = found {
                    self.highlighted = Some(row);
                    cx.notify();
                } else {
                    return;
                }
            }
        }
        cx.stop_propagation();
    }

    fn item(&self, index: usize, entry: &PopupEntry, cx: &mut Context<Self>) -> AnyElement {
        let content: AnyElement = match &entry.label {
            Label::Text(text) => div().child(text.clone()).into_any_element(),
            Label::Urdu(text) => div()
                .mt(px(-2.))
                .child(ur_inline(text.clone(), UrRole::Body))
                .into_any_element(),
        };
        let mut item = match &entry.label {
            Label::Text(text) => menu_item(entry.id.clone(), text.clone()),
            Label::Urdu(_) => menu_item_element(entry.id.clone(), content),
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
        if entry.danger {
            item = item.danger();
        }
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
        let rows: Vec<PopupRow> = self.rows.clone();
        let children: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| match row {
                PopupRow::Entry(entry) => self.item(index, entry, cx),
                PopupRow::Separator => menu_separator(&palette).into_any_element(),
            })
            .collect();
        let rises = matches!(self.corner, Anchor::BottomLeft | Anchor::BottomRight);
        deferred(
            anchored()
                .position(self.position)
                .anchor(self.corner)
                .snap_to_window_with_margin(px(8.))
                .child(
                    menu_panel(&palette)
                        .id(self.id.clone())
                        .key_context("PopupList MissalTextInput")
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
        .with_priority(1)
    }
}
