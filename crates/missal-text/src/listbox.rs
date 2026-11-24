use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Anchor, AnchoredPositionMode, AnyElement, App, Bounds, BoxShadow, Deferred, ElementId,
    FontWeight, Hsla, InteractiveElement, IntoElement, KeyBinding, MouseButton, ParentElement,
    Pixels, RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window,
    anchored, deferred, div, point, px,
};

use crate::fonts::UI_FAMILY;

pub const POPOVER_GAP: f32 = 4.0;
pub const POPOVER_MARGIN: f32 = 8.0;
pub const POPOVER_PRIORITY: usize = 2;
const PANEL_PADDING: f32 = 4.0;
const HEADER_HEIGHT: f32 = 28.0;

gpui_kit::actions!(
    listbox,
    [
        SelectNext,
        SelectPrevious,
        SelectPageDown,
        SelectPageUp,
        Confirm,
        Complete,
        Dismiss,
        Reveal,
    ]
);

pub fn bind_keys(cx: &mut App, context: &str) {
    let c = Some(context);
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, c),
        KeyBinding::new("up", SelectPrevious, c),
        KeyBinding::new("ctrl-n", SelectNext, c),
        KeyBinding::new("ctrl-p", SelectPrevious, c),
        KeyBinding::new("pagedown", SelectPageDown, c),
        KeyBinding::new("pageup", SelectPageUp, c),
        KeyBinding::new("enter", Confirm, c),
        KeyBinding::new("tab", Complete, c),
        KeyBinding::new("escape", Dismiss, c),
    ]);
}

pub fn bind_reveal_keys(cx: &mut App, context: &str) {
    let c = Some(context);
    cx.bind_keys([
        KeyBinding::new("alt-down", Reveal, c),
        KeyBinding::new("ctrl-space", Reveal, c),
    ]);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListNav {
    len: usize,
    active: Option<usize>,
}

impl ListNav {
    pub const fn new(len: usize) -> ListNav {
        ListNav { len, active: None }
    }

    pub fn with_active(len: usize, active: Option<usize>) -> ListNav {
        ListNav {
            len,
            active: active.filter(|index| *index < len),
        }
    }

    pub const fn len(self) -> usize {
        self.len
    }

    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub const fn active(self) -> Option<usize> {
        self.active
    }

    pub fn set_active(&mut self, active: Option<usize>) -> bool {
        let active = active.filter(|index| *index < self.len);
        let changed = active != self.active;
        self.active = active;
        changed
    }

    pub fn step(&mut self, delta: isize) -> bool {
        if self.len == 0 || delta == 0 {
            return false;
        }
        let len = self.len as isize;
        let next = match self.active {
            None if delta > 0 => delta - 1,
            None => len + delta,
            Some(index) => index as isize + delta,
        };
        self.set_active(Some(next.rem_euclid(len) as usize))
    }

    pub fn page(&mut self, delta: isize, size: usize) -> bool {
        if self.len == 0 {
            return false;
        }
        let size = size.max(1) as isize;
        let from = self.active.map_or(-1, |index| index as isize);
        let next = (from + delta.signum() * size).clamp(0, self.len as isize - 1);
        self.set_active(Some(next as usize))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListboxTheme {
    pub panel: Hsla,
    pub ink: Hsla,
    pub ink_muted: Hsla,
    pub ink_faint: Hsla,
    pub active: Hsla,
    pub line: Hsla,
    pub shadow: Vec<BoxShadow>,
    pub radius: f32,
    pub row_radius: f32,
}

pub struct ListboxRow {
    lead: Option<AnyElement>,
    body: AnyElement,
    trail: Option<AnyElement>,
}

pub fn listbox_row(body: impl IntoElement) -> ListboxRow {
    ListboxRow {
        lead: None,
        body: body.into_any_element(),
        trail: None,
    }
}

impl ListboxRow {
    pub fn lead(mut self, lead: impl IntoElement) -> Self {
        self.lead = Some(lead.into_any_element());
        self
    }

    pub fn trail(mut self, trail: impl IntoElement) -> Self {
        self.trail = Some(trail.into_any_element());
        self
    }
}

pub struct ListboxSection {
    title: Option<SharedString>,
    rows: Vec<ListboxRow>,
}

pub fn listbox_section(title: Option<SharedString>, rows: Vec<ListboxRow>) -> ListboxSection {
    ListboxSection { title, rows }
}

type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Listbox {
    id: ElementId,
    theme: ListboxTheme,
    sections: Vec<ListboxSection>,
    active: Option<usize>,
    row_height: f32,
    width: Option<Pixels>,
    max_rows: usize,
    scroll: Option<ScrollHandle>,
    reveal: bool,
    rtl: bool,
    empty: Option<AnyElement>,
    footer: Option<AnyElement>,
    on_hover: Option<IndexHandler>,
    on_pick: Option<IndexHandler>,
}

pub fn listbox(id: impl Into<ElementId>, theme: ListboxTheme) -> Listbox {
    Listbox {
        id: id.into(),
        theme,
        sections: Vec::new(),
        active: None,
        row_height: 32.0,
        width: None,
        max_rows: 8,
        scroll: None,
        reveal: false,
        rtl: false,
        empty: None,
        footer: None,
        on_hover: None,
        on_pick: None,
    }
}

fn listbox_height(sections: &[(bool, usize)], row_height: f32, max_rows: usize) -> f32 {
    let mut height = 0.0;
    let mut rows = 0;
    for (titled, count) in sections {
        if *titled {
            height += HEADER_HEIGHT;
        }
        let take = (*count).min(max_rows.saturating_sub(rows));
        height += take as f32 * row_height;
        rows += take;
    }
    height + 2.0 * PANEL_PADDING
}

impl Listbox {
    pub fn section(mut self, section: ListboxSection) -> Self {
        if !section.rows.is_empty() {
            self.sections.push(section);
        }
        self
    }

    pub fn rows(self, rows: Vec<ListboxRow>) -> Self {
        self.section(listbox_section(None, rows))
    }

    pub fn active(mut self, active: Option<usize>) -> Self {
        self.active = active;
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height;
        self
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn max_rows(mut self, rows: usize) -> Self {
        self.max_rows = rows.max(1);
        self
    }

    pub fn track_scroll(mut self, scroll: &ScrollHandle, reveal: bool) -> Self {
        self.scroll = Some(scroll.clone());
        self.reveal = reveal;
        self
    }

    pub fn rtl(mut self, rtl: bool) -> Self {
        self.rtl = rtl;
        self
    }

    pub fn empty(mut self, empty: impl IntoElement) -> Self {
        self.empty = Some(empty.into_any_element());
        self
    }

    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    pub fn on_hover(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }

    pub fn on_pick(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Listbox {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let theme = self.theme;
        let mut children: Vec<AnyElement> = Vec::new();
        let mut index = 0usize;
        let mut active_child = None;
        let shape: Vec<(bool, usize)> = self
            .sections
            .iter()
            .map(|section| (section.title.is_some(), section.rows.len()))
            .collect();
        let max_height = listbox_height(&shape, self.row_height, self.max_rows);
        let id = self.id.clone();
        for (section_index, section) in self.sections.into_iter().enumerate() {
            if let Some(title) = section.title {
                children.push(
                    div()
                        .id((SharedString::from("listbox-section"), section_index))
                        .flex_none()
                        .h(px(HEADER_HEIGHT))
                        .px(px(8.))
                        .pt(px(8.))
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.ink_faint)
                        .when(self.rtl, |this| this.flex().justify_end())
                        .child(title)
                        .into_any_element(),
                );
            }
            for row in section.rows {
                let selected = self.active == Some(index);
                if selected {
                    active_child = Some(children.len());
                }
                let hover = self.on_hover.clone();
                let pick = self.on_pick.clone();
                children.push(
                    div()
                        .id((SharedString::from("listbox-option"), index))
                        .flex_none()
                        .flex()
                        .when(self.rtl, gpui_kit::Styled::flex_row_reverse)
                        .items_center()
                        .gap(px(8.))
                        .h(px(self.row_height))
                        .px(px(8.))
                        .rounded(px(theme.row_radius))
                        .cursor_pointer()
                        .role(gpui_kit::accesskit::Role::ListBoxOption)
                        .when(selected, |this| this.bg(theme.active))
                        .on_mouse_move(move |_, window, cx| {
                            if !selected && let Some(hover) = hover.as_ref() {
                                hover(index, window, cx);
                            }
                        })
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cx.stop_propagation();
                            window.prevent_default();
                            if let Some(pick) = pick.as_ref() {
                                pick(index, window, cx);
                            }
                        })
                        .children(row.lead.map(|lead| {
                            div()
                                .flex_none()
                                .flex()
                                .items_center()
                                .text_color(theme.ink_muted)
                                .child(lead)
                        }))
                        .child(div().flex_1().min_w_0().child(row.body))
                        .children(row.trail.map(|trail| {
                            div()
                                .flex_none()
                                .flex()
                                .items_center()
                                .text_size(px(12.))
                                .text_color(theme.ink_faint)
                                .child(trail)
                        }))
                        .into_any_element(),
                );
                index += 1;
            }
        }
        if self.reveal
            && let (Some(scroll), Some(child)) = (self.scroll.as_ref(), active_child)
        {
            scroll.scroll_to_item(child);
        }
        let empty = children.is_empty();
        let list = div()
            .id("listbox-scroll")
            .flex()
            .flex_col()
            .p(px(PANEL_PADDING))
            .max_h(px(max_height))
            .overflow_y_scroll()
            .role(gpui_kit::accesskit::Role::ListBox)
            .when_some(self.scroll.as_ref(), |this, scroll| {
                this.track_scroll(scroll)
            })
            .children(children);
        div()
            .id(id)
            .occlude()
            .flex()
            .flex_col()
            .when_some(self.width, gpui_kit::Styled::w)
            .rounded(px(theme.radius))
            .bg(theme.panel)
            .shadow(theme.shadow.clone())
            .text_color(theme.ink)
            .font_family(UI_FAMILY)
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
            })
            .when(!empty, |this| this.child(list))
            .when(empty, |this| this.children(self.empty))
            .children(
                self.footer
                    .map(|footer| div().border_t_1().border_color(theme.line).child(footer)),
            )
    }
}

pub fn popover(
    target: Bounds<Pixels>,
    mode: AnchoredPositionMode,
    align_end: bool,
    child: impl IntoElement,
) -> Deferred {
    let (corner, anchor) = if align_end {
        (Anchor::BottomRight, Anchor::TopRight)
    } else {
        (Anchor::BottomLeft, Anchor::TopLeft)
    };
    deferred(
        anchored()
            .position_mode(mode)
            .attach(target, corner)
            .anchor(anchor)
            .offset(point(px(0.), px(POPOVER_GAP)))
            .snap_to_window_with_margin(px(POPOVER_MARGIN))
            .child(child),
    )
    .with_priority(POPOVER_PRIORITY)
}
