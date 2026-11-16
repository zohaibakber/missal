use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::{Button as BaseButton, Checkbox as BaseCheckbox, Switch as BaseSwitch, Tab};
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, InteractiveElement, Interactivity,
    IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, px,
};

use crate::theme::{
    CHECKBOX, ICON_XS, Palette, RADIUS, RADIUS_CHIP, RADIUS_LG, SEGMENTED_ITEM, SIDEBAR_ITEM,
    hex_alpha,
};
use crate::ui::part::{Part, part, underlay};
use crate::ui::{ClickHandler, inset_shadow, sized_icon};

#[derive(IntoElement)]
pub struct Themed<T: IntoElement + Styled + 'static> {
    inner: T,
    theme: fn(T, &Palette) -> T,
}

impl<T: IntoElement + Styled + 'static> Themed<T> {
    fn new(inner: T, theme: fn(T, &Palette) -> T) -> Self {
        Themed { inner, theme }
    }

    fn map(self, f: impl FnOnce(T) -> T) -> Self {
        Themed {
            inner: f(self.inner),
            theme: self.theme,
        }
    }
}

impl<T: IntoElement + Styled + 'static> RenderOnce for Themed<T> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let theme = self.theme;
        underlay(self.inner, |inner| theme(inner, &palette))
    }
}

impl<T: IntoElement + Styled + 'static> Styled for Themed<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl<T: IntoElement + Styled + ParentElement + 'static> ParentElement for Themed<T> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.inner.extend(elements)
    }
}

impl<T: IntoElement + Styled + InteractiveElement + 'static> InteractiveElement for Themed<T> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.inner.interactivity()
    }
}

impl<T: IntoElement + Styled + StatefulInteractiveElement + 'static> StatefulInteractiveElement
    for Themed<T>
{
}

pub type Checkbox = Themed<BaseCheckbox>;

pub fn checkbox(id: impl Into<ElementId>, checked: bool) -> Checkbox {
    Themed::new(
        BaseCheckbox::new(id)
            .checked(checked)
            .flex_none()
            .size(px(CHECKBOX))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(RADIUS_CHIP))
            .border_1()
            .cursor_pointer()
            .when(checked, |this| this.child(check_mark())),
        |checkbox, palette| {
            let palette = *palette;
            let focus = palette.focus_ring();
            checkbox
                .border_color(palette.line_input)
                .bg(palette.control_fill())
                .shadow({
                    let mut shadows = vec![palette.edge()];
                    shadows.extend(palette.elevation_xs());
                    shadows
                })
                .focus_visible(move |style| style.shadow(focus))
                .styles(|styles| {
                    styles
                        .checked(|style| {
                            style
                                .bg(palette.action)
                                .border_color(palette.action)
                                .shadow(Vec::new())
                        })
                        .disabled(|style| style.opacity(0.64))
                })
        },
    )
}

impl Checkbox {
    pub fn accessibility_label(self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.map(|checkbox| checkbox.accessibility_label(label))
    }

    pub fn tab_stop(self, tab_stop: bool) -> Self {
        self.map(|checkbox| checkbox.tab_stop(tab_stop))
    }

    pub fn disabled(self, disabled: bool) -> Self {
        self.map(|checkbox| checkbox.disabled(disabled))
    }
}

fn check_mark() -> Part {
    part(|base, palette| {
        base.flex()
            .items_center()
            .justify_center()
            .child(sized_icon(IconName::Check, ICON_XS, palette.on_action))
    })
}

fn switch_thumb() -> Part {
    part(|base, palette| {
        base.size(px(THUMB))
            .rounded_full()
            .bg(palette.surface)
            .shadow(palette.elevation_sm())
    })
}

pub type Switch = Themed<BaseSwitch>;

const THUMB: f32 = 16.;

pub fn switch(id: impl Into<ElementId>, checked: bool) -> Switch {
    Themed::new(
        BaseSwitch::new(id)
            .checked(checked)
            .flex_none()
            .h(px(THUMB + 2.))
            .w(px(THUMB * 2. - 2.))
            .p(px(1.))
            .flex()
            .items_center()
            .when(checked, |this| this.justify_end())
            .rounded_full()
            .cursor_pointer()
            .child(switch_thumb()),
        |switch, palette| {
            let palette = *palette;
            let focus = palette.focus_ring();
            switch
                .bg(palette.line_input)
                .shadow(vec![inset_shadow(hex_alpha(0x000000, 0.04), 1.)])
                .focus_visible(move |style| style.shadow(focus))
                .styles(|styles| {
                    styles
                        .checked(|style| style.bg(palette.action))
                        .disabled(|style| style.opacity(0.64))
                })
        },
    )
}

impl Switch {
    pub fn accessibility_label(self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.map(|switch| switch.accessibility_label(label))
    }

    pub fn disabled(self, disabled: bool) -> Self {
        self.map(|switch| switch.disabled(disabled))
    }
}

pub fn tab_list() -> Part {
    part(|base, palette| {
        base.h_flex()
            .flex_none()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(RADIUS_LG))
            .bg(palette.hover)
    })
}

pub type TabItem = Themed<Tab>;

pub fn tab(id: impl Into<ElementId>, selected: bool) -> TabItem {
    Themed::new(
        Tab::new(id)
            .selected(selected)
            .h(px(SEGMENTED_ITEM))
            .px(px(9.))
            .gap(px(6.))
            .items_center()
            .rounded(px(RADIUS))
            .text_size(px(14.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .cursor_pointer(),
        |tab, palette| {
            let palette = *palette;
            let muted = palette.ink_muted;
            let focus = palette.focus_ring();
            let active = if palette.dark {
                palette.line_input
            } else {
                palette.surface
            };
            tab.text_color(palette.ink_faint)
                .hover(move |style| style.text_color(muted))
                .focus_visible(move |style| style.shadow(focus))
                .styles(|styles| {
                    styles
                        .selected(|style| {
                            style
                                .bg(active)
                                .text_color(palette.ink)
                                .shadow(palette.elevation_sm())
                        })
                        .disabled(|style| style.opacity(0.64))
                })
        },
    )
}

pub struct SegmentItem {
    pub id: ElementId,
    pub label: SharedString,
    pub on_select: ClickHandler,
}

impl SegmentItem {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        on_select: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> SegmentItem {
        SegmentItem {
            id: id.into(),
            label: label.into(),
            on_select: Rc::new(on_select),
        }
    }
}

pub fn segmented(items: Vec<SegmentItem>, selected: usize) -> Part {
    tab_list().children(items.into_iter().enumerate().map(|(index, item)| {
        let handler = item.on_select.clone();
        tab(item.id, index == selected)
            .on_click(move |event, window, cx| handler(event, window, cx))
            .child(item.label)
    }))
}

pub type SidebarMenuButton = Themed<BaseButton>;

pub fn sidebar_menu_button(id: impl Into<ElementId>, active: bool) -> SidebarMenuButton {
    Themed::new(
        BaseButton::new(id)
            .selected(active)
            .w_full()
            .h(px(SIDEBAR_ITEM))
            .px(px(8.))
            .gap(px(8.))
            .justify_start()
            .rounded(px(RADIUS_LG))
            .text_size(px(14.))
            .line_height(px(20.))
            .overflow_hidden()
            .cursor_pointer(),
        |button, palette| {
            let palette = *palette;
            let hover = palette.hover;
            let focus = palette.focus_ring();
            button
                .text_color(palette.sidebar_ink)
                .hover(move |style| style.bg(hover).text_color(palette.ink))
                .active(move |style| style.bg(hover).text_color(palette.ink))
                .focus_visible(move |style| style.shadow(focus))
                .styles(|styles| {
                    styles.selected(|style| {
                        style
                            .bg(hover)
                            .text_color(palette.ink)
                            .font_weight(FontWeight::MEDIUM)
                    })
                })
        },
    )
}
