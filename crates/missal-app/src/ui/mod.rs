pub mod cheatsheet;
pub mod components;
pub mod dialogs;
pub mod input;
pub mod menu;
pub mod nav_menu;
pub mod palette;
pub mod print_preview;
pub mod status;
pub mod titlebar;
pub mod toast;
pub mod window_controls;

use std::rc::Rc;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Colorize, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AnyView, App, ClickEvent, Context, Div, ElementId, FocusHandle, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, RenderOnce, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use missal_text::{Align, Label, LineHeight, label};

use crate::shortcuts::key_caps;
use crate::theme::{
    BUTTON_MD, BUTTON_SM, CHECKBOX, ICON, ICON_BUTTON, ICON_BUTTON_DENSE, ICON_SM, ICON_XS, KEYCAP,
    PANE_HEADER_HEIGHT, Palette, RADIUS, RADIUS_LG, RADIUS_PILL, RADIUS_SM, RAIL_HEADER_HEIGHT,
    SEGMENTED_ITEM, SEGMENTED_TRACK, STATUS_BAR_HEIGHT, TOOLTIP_DELAY,
};

pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub fn icon(name: IconName) -> Icon {
    Icon::new(name)
}

pub fn sized_icon(name: IconName, size: f32, color: Hsla) -> Icon {
    Icon::new(name).size(px(size)).text_color(color)
}

pub fn small_icon(name: IconName, color: Hsla) -> Icon {
    sized_icon(name, ICON, color)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UrRole {
    Label,
    Body,
    InputLabel,
    Title,
    Display,
}

impl UrRole {
    pub const fn size(self) -> f32 {
        match self {
            UrRole::Label => 14.0,
            UrRole::Body => 16.0,
            UrRole::InputLabel => 15.0,
            UrRole::Title => 18.0,
            UrRole::Display => 22.0,
        }
    }

    pub const fn line_height(self) -> f32 {
        match self {
            UrRole::Label => 26.0,
            UrRole::Body => 30.0,
            UrRole::InputLabel => 28.0,
            UrRole::Title => 34.0,
            UrRole::Display => 40.0,
        }
    }
}

pub fn ur_label(text: impl Into<SharedString>, role: UrRole) -> Label {
    label(text)
        .rtl()
        .leading(LineHeight::Px(role.line_height()))
        .text_size(px(role.size()))
}

pub fn ur_inline(text: impl Into<SharedString>, role: UrRole) -> Label {
    label(text)
        .rtl()
        .align(Align::Left)
        .leading(LineHeight::Px(role.line_height()))
        .text_size(px(role.size()))
}

pub fn isolate_urdu(text: &str) -> String {
    format!("\u{2067}{text}\u{2069}")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyTone {
    Plain,
    Bare,
    OnAction,
    OnTooltip,
}

fn display_key(key: &str) -> String {
    match key {
        "Enter" | "enter" => "↵".into(),
        "Up" | "up" => "↑".into(),
        "Down" | "down" => "↓".into(),
        "Left" | "left" => "←".into(),
        "Right" | "right" => "→".into(),
        "Escape" | "escape" => "Esc".into(),
        "Delete" => "Del".into(),
        "PageUp" => "PgUp".into(),
        "PageDown" => "PgDn".into(),
        other => other.into(),
    }
}

pub fn keycap(palette: &Palette, text: impl Into<SharedString>) -> Div {
    keycap_toned(palette, text, KeyTone::Plain)
}

pub fn keycap_toned(palette: &Palette, text: impl Into<SharedString>, tone: KeyTone) -> Div {
    let (fill, border, color) = match tone {
        KeyTone::Plain => (Some(palette.hover), palette.line, palette.ink_muted),
        KeyTone::Bare => (None, palette.line, palette.ink_muted),
        KeyTone::OnAction => (
            None,
            palette.on_action.opacity(0.35),
            palette.on_action.opacity(0.8),
        ),
        KeyTone::OnTooltip => (None, palette.surface.opacity(0.35), palette.surface),
    };
    div()
        .flex_none()
        .h(px(KEYCAP))
        .min_w(px(KEYCAP))
        .px(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(border)
        .when_some(fill, |this, fill| this.bg(fill))
        .text_size(px(11.))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(text.into())
}

pub fn keycaps(palette: &Palette, keys: &str) -> Div {
    keycaps_toned(palette, keys, KeyTone::Plain)
}

pub fn keycaps_toned(palette: &Palette, keys: &str, tone: KeyTone) -> Div {
    h_flex().flex_none().items_center().gap(px(3.)).children(
        key_caps(keys)
            .into_iter()
            .flatten()
            .map(|cap| keycap_toned(palette, display_key(&cap), tone)),
    )
}

pub fn keycaps_chord(palette: &Palette, keys: &str) -> Div {
    h_flex().flex_none().items_center().gap(px(3.)).children(
        key_caps(keys)
            .into_iter()
            .enumerate()
            .flat_map(|(step, caps)| {
                let mut items: Vec<AnyElement> = Vec::new();
                if step > 0 {
                    items.push(
                        sized_icon(IconName::ChevronRight, ICON_XS, palette.ink_faint)
                            .into_any_element(),
                    );
                }
                items.extend(
                    caps.into_iter()
                        .map(|cap| keycap(palette, display_key(&cap)).into_any_element()),
                );
                items
            }),
    )
}

pub struct TooltipView {
    label: SharedString,
    keys: Option<SharedString>,
}

impl Render for TooltipView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        div().pl(px(4.)).pt(px(4.)).child(
            h_flex()
                .items_center()
                .gap(px(8.))
                .py(px(4.))
                .px(px(8.))
                .rounded(px(RADIUS))
                .bg(palette.ink)
                .text_color(palette.surface)
                .font_family(missal_text::UI_FAMILY)
                .text_size(px(12.))
                .line_height(px(18.))
                .child(self.label.clone())
                .children(
                    self.keys
                        .as_ref()
                        .map(|keys| keycaps_toned(&palette, keys, KeyTone::OnTooltip)),
                ),
        )
    }
}

pub fn tooltip(
    label: impl Into<SharedString>,
    keys: Option<&str>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label: SharedString = label.into();
    let keys: Option<SharedString> = keys.map(|keys| SharedString::from(keys.to_owned()));
    move |_, cx| {
        cx.new(|_| TooltipView {
            label: label.clone(),
            keys: keys.clone(),
        })
        .into()
    }
}

pub fn tooltip_with_keys(
    text: &'static str,
    keys: Option<&'static str>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    tooltip(text, keys)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonKind {
    Primary,
    Outline,
    Ghost,
    GhostMuted,
    Danger,
    DangerOutline,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    Sm,
    Md,
}

#[derive(IntoElement)]
pub struct ActionButton {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    keys: Option<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
    disabled: bool,
    show_hover: bool,
    show_focus: bool,
    focus: Option<FocusHandle>,
    tooltip: Option<(SharedString, Option<SharedString>)>,
    on_click: Option<ClickHandler>,
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ActionButton {
    ActionButton {
        id: id.into(),
        label: label.into(),
        icon: None,
        keys: None,
        kind: ButtonKind::Outline,
        size: ButtonSize::Md,
        disabled: false,
        show_hover: false,
        show_focus: false,
        focus: None,
        tooltip: None,
        on_click: None,
    }
}

pub fn primary_sm(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ActionButton {
    button(id, label).kind(ButtonKind::Primary).sm()
}

pub fn ghost_sm(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ActionButton {
    button(id, label).kind(ButtonKind::Ghost).sm()
}

pub fn danger_outline(id: impl Into<ElementId>, label: impl Into<SharedString>) -> ActionButton {
    button(id, label).kind(ButtonKind::DangerOutline).sm()
}

impl ActionButton {
    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn primary(self) -> Self {
        self.kind(ButtonKind::Primary)
    }

    pub fn outline(self) -> Self {
        self.kind(ButtonKind::Outline)
    }

    pub fn ghost(self) -> Self {
        self.kind(ButtonKind::Ghost)
    }

    pub fn danger(self) -> Self {
        self.kind(ButtonKind::Danger)
    }

    pub fn sm(mut self) -> Self {
        self.size = ButtonSize::Sm;
        self
    }

    pub fn md(mut self) -> Self {
        self.size = ButtonSize::Md;
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub fn tooltip(mut self, label: impl Into<SharedString>, keys: Option<&str>) -> Self {
        self.tooltip = Some((
            label.into(),
            keys.map(|keys| SharedString::from(keys.to_owned())),
        ));
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn show_hover(mut self) -> Self {
        self.show_hover = true;
        self
    }

    pub fn show_focus(mut self) -> Self {
        self.show_focus = true;
        self
    }
}

impl RenderOnce for ActionButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let (fill, hover_fill, text, border, shadow) = match self.kind {
            ButtonKind::Primary => (
                Some(palette.action),
                Some(palette.action_hover),
                palette.on_action,
                None,
                true,
            ),
            ButtonKind::Outline => (
                Some(palette.surface),
                Some(palette.hover),
                palette.ink,
                Some(palette.line_strong.mix_oklab(palette.surface, 0.7)),
                false,
            ),
            ButtonKind::Ghost => (None, Some(palette.hover), palette.ink, None, false),
            ButtonKind::GhostMuted => (None, Some(palette.hover), palette.ink_muted, None, false),
            ButtonKind::Danger => (
                Some(palette.danger),
                Some(palette.danger.mix_oklab(palette.ink, 0.88)),
                crate::theme::hex(0xffffff),
                None,
                true,
            ),
            ButtonKind::DangerOutline => (
                Some(palette.surface),
                Some(palette.danger_wash),
                palette.danger_text,
                Some(palette.danger_text.mix_oklab(palette.surface, 0.45)),
                false,
            ),
        };
        let height = match self.size {
            ButtonSize::Sm => BUTTON_SM,
            ButtonSize::Md => BUTTON_MD,
        };
        let pad = match self.size {
            ButtonSize::Sm => 10.,
            ButtonSize::Md => 12.,
        };
        let key_tone = if self.kind == ButtonKind::Primary {
            KeyTone::OnAction
        } else {
            KeyTone::Bare
        };
        let focused = self.show_focus
            || self
                .focus
                .as_ref()
                .is_some_and(|focus| focus.is_focused(window) && window.last_input_was_keyboard());
        let mut shadows = Vec::new();
        if focused {
            shadows.extend(palette.focus_ring());
        }
        if shadow && !self.disabled {
            shadows.extend(palette.shadow_sm());
        }
        let on_click = self.on_click.clone();
        let disabled = self.disabled;
        h_flex()
            .id(self.id)
            .flex_none()
            .h(px(height))
            .px(px(pad))
            .gap(px(6.))
            .items_center()
            .justify_center()
            .rounded(px(RADIUS))
            .border_1()
            .border_color(border.unwrap_or(gpui_kit::transparent_black()))
            .when_some(fill, |this, fill| this.bg(fill))
            .when(self.show_hover, |this| {
                this.when_some(hover_fill, |this, fill| this.bg(fill))
            })
            .text_color(text)
            .text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .when(!shadows.is_empty(), |this| this.shadow(shadows))
            .when(disabled, |this| this.opacity(0.45))
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .when_some(hover_fill, |this, fill| this.hover(move |s| s.bg(fill)))
            })
            .when_some(self.focus.as_ref(), |this, focus| this.track_focus(focus))
            .role(gpui_kit::accesskit::Role::Button)
            .aria_label(self.label.clone())
            .when_some(self.tooltip.clone(), |this, (label, keys)| {
                this.tooltip(tooltip(label, keys.as_deref()))
                    .tooltip_show_delay(TOOLTIP_DELAY)
            })
            .when_some(on_click.filter(|_| !disabled), |this, handler| {
                this.on_click(move |event, window, cx| handler(event, window, cx))
            })
            .when_some(self.icon, |this, name| {
                this.child(sized_icon(name, ICON_SM, text))
            })
            .child(self.label)
            .when_some(self.keys, |this, keys| {
                this.child(
                    div()
                        .ml(px(2.))
                        .child(keycaps_toned(&palette, &keys, key_tone)),
                )
            })
            .test_support()
    }
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    label: SharedString,
    keys: Option<SharedString>,
    dense: bool,
    toggled: bool,
    disabled: bool,
    show_hover: bool,
    show_focus: bool,
    color: Option<Hsla>,
    focus: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
}

pub fn icon_button(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
) -> IconButton {
    IconButton {
        id: id.into(),
        icon,
        label: label.into(),
        keys: None,
        dense: false,
        toggled: false,
        disabled: false,
        show_hover: false,
        show_focus: false,
        color: None,
        focus: None,
        on_click: None,
    }
}

impl IconButton {
    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    pub fn dense(mut self) -> Self {
        self.dense = true;
        self
    }

    pub fn toggled(mut self, toggled: bool) -> Self {
        self.toggled = toggled;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn show_hover(mut self) -> Self {
        self.show_hover = true;
        self
    }

    pub fn show_focus(mut self) -> Self {
        self.show_focus = true;
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let size = if self.dense {
            ICON_BUTTON_DENSE
        } else {
            ICON_BUTTON
        };
        let glyph = if self.dense { ICON_SM } else { ICON };
        let active = self.toggled || self.show_hover;
        let idle = self.color.unwrap_or(palette.ink_muted);
        let color = if active { palette.ink } else { idle };
        let focused = self.show_focus
            || self
                .focus
                .as_ref()
                .is_some_and(|focus| focus.is_focused(window) && window.last_input_was_keyboard());
        let disabled = self.disabled;
        let on_click = self.on_click.clone();
        let ink = palette.ink;
        let hover = palette.hover;
        div()
            .id(self.id)
            .flex_none()
            .size(px(size))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(RADIUS))
            .text_color(color)
            .when(self.toggled, |this| this.bg(palette.pressed))
            .when(self.show_hover && !self.toggled, |this| this.bg(hover))
            .when(focused, |this| {
                this.bg(palette.surface).shadow(vec![gpui_kit::BoxShadow {
                    color: palette.accent,
                    offset: gpui_kit::point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(2.),
                    inset: false,
                }])
            })
            .when(disabled, |this| this.opacity(0.4))
            .when(!disabled && !self.toggled, |this| {
                this.hover(move |s| s.bg(hover).text_color(ink))
            })
            .when_some(self.focus.as_ref(), |this, focus| this.track_focus(focus))
            .occlude()
            .role(gpui_kit::accesskit::Role::Button)
            .aria_label(self.label.clone())
            .when(self.toggled, |this| {
                this.aria_toggled(gpui_kit::accesskit::Toggled::True)
            })
            .tooltip(tooltip(self.label.clone(), self.keys.as_deref()))
            .tooltip_show_delay(crate::theme::TOOLTIP_DELAY)
            .when_some(on_click.filter(|_| !disabled), |this, handler| {
                this.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx)
                })
            })
            .child(sized_icon(self.icon, glyph, color))
            .test_support()
    }
}

pub fn kit_icon_button(id: impl Into<ElementId>, name: IconName) -> Button {
    Button::new(id).icon(name).ghost().small()
}

pub fn section_label(palette: &Palette, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.ink_muted)
        .child(text.into())
}

pub fn caption(palette: &Palette, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.))
        .line_height(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.ink_faint)
        .child(text.into().to_uppercase())
}

pub fn empty_state(
    palette: &Palette,
    name: IconName,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    action: Option<AnyElement>,
) -> Div {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .p_6()
        .child(empty_state_body(palette, name, title, description, action))
}

pub fn empty_state_body(
    palette: &Palette,
    name: IconName,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    action: Option<AnyElement>,
) -> Div {
    v_flex()
        .items_center()
        .child(
            div()
                .size(px(36.))
                .mb(px(10.))
                .rounded(px(RADIUS_LG))
                .bg(palette.hover)
                .flex()
                .items_center()
                .justify_center()
                .child(small_icon(name, palette.ink_muted)),
        )
        .child(
            div()
                .text_size(px(14.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.ink)
                .child(
                    label(title.into())
                        .align(Align::Center)
                        .leading(LineHeight::Px(20.)),
                ),
        )
        .children(description.map(|description| {
            div()
                .mt(px(2.))
                .max_w(px(420.))
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(palette.ink_muted)
                .child(
                    label(description)
                        .align(Align::Center)
                        .leading(LineHeight::Px(20.)),
                )
        }))
        .children(action.map(|action| div().mt(px(12.)).child(action)))
}

pub fn skeleton_bar(palette: &Palette, width: f32, height: f32) -> Div {
    div()
        .w(px(width))
        .h(px(height))
        .rounded(px(RADIUS_SM))
        .bg(palette.hover)
}

pub fn count_badge(palette: &Palette, count: usize) -> Div {
    div()
        .flex_none()
        .h(px(18.))
        .min_w(px(18.))
        .px(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(9.))
        .bg(palette.notice_wash)
        .text_color(palette.notice_text)
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(count.to_string())
}

pub fn warn_box(palette: &Palette, content: impl IntoElement) -> Div {
    h_flex()
        .items_start()
        .gap(px(8.))
        .p(px(12.))
        .rounded(px(RADIUS_LG))
        .bg(palette.notice_wash)
        .text_color(palette.notice_text)
        .text_size(px(13.))
        .line_height(px(20.))
        .child(
            div()
                .pt(px(3.))
                .child(sized_icon(IconName::TriangleAlert, ICON_SM, palette.notice)),
        )
        .child(div().flex_1().min_w_0().child(content))
}

pub fn checkbox(palette: &Palette, id: impl Into<ElementId>, checked: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(px(CHECKBOX))
        .rounded(px(RADIUS_SM))
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .role(gpui_kit::accesskit::Role::CheckBox)
        .map(|this| {
            if checked {
                this.bg(palette.action)
                    .border_color(palette.action)
                    .child(sized_icon(IconName::Check, ICON_XS, palette.on_action))
            } else {
                this.bg(palette.surface).border_color(palette.line_strong)
            }
        })
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

pub fn segmented(palette: &Palette, items: Vec<SegmentItem>, selected: usize) -> Div {
    let ink = palette.ink;
    h_flex()
        .flex_none()
        .h(px(SEGMENTED_TRACK))
        .p(px(2.))
        .gap(px(2.))
        .rounded(px(RADIUS_LG))
        .bg(palette.hover)
        .children(items.into_iter().enumerate().map(|(index, item)| {
            let on = index == selected;
            let handler = item.on_select.clone();
            div()
                .id(item.id)
                .h(px(SEGMENTED_ITEM))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(RADIUS))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .role(gpui_kit::accesskit::Role::RadioButton)
                .when(on, |this| {
                    this.bg(palette.surface)
                        .text_color(palette.ink)
                        .shadow(palette.shadow_sm())
                })
                .when(!on, |this| {
                    this.text_color(palette.ink_muted)
                        .hover(move |s| s.text_color(ink))
                })
                .on_click(move |event, window, cx| handler(event, window, cx))
                .child(item.label)
        }))
}

pub fn pill(palette: &Palette) -> Div {
    h_flex()
        .flex_none()
        .h(px(24.))
        .px(px(8.))
        .gap(px(6.))
        .items_center()
        .rounded(px(RADIUS_PILL))
        .border_1()
        .border_color(palette.line)
}

pub fn hairline(palette: &Palette) -> Div {
    div().h(px(1.)).w_full().bg(palette.line)
}

pub fn vertical_rule(palette: &Palette) -> Div {
    div().w(px(1.)).h(px(20.)).bg(palette.line)
}

pub fn pane_header(palette: &Palette) -> Div {
    h_flex()
        .flex_shrink_0()
        .h(px(PANE_HEADER_HEIGHT))
        .items_center()
        .gap_2()
        .pl_4()
        .pr_2()
        .border_b_1()
        .border_color(palette.line)
}

pub fn rail_header(palette: &Palette, title: &'static str) -> Div {
    h_flex()
        .flex_shrink_0()
        .h(px(RAIL_HEADER_HEIGHT))
        .items_center()
        .gap_2()
        .pl_4()
        .pr_1p5()
        .child(section_label(palette, title).flex_1().min_w_0())
}

pub fn status_bar(palette: &Palette) -> Div {
    h_flex()
        .flex_shrink_0()
        .h(px(STATUS_BAR_HEIGHT))
        .items_center()
        .gap_3()
        .px_4()
        .border_t_1()
        .border_color(palette.line)
        .text_size(px(12.))
        .text_color(palette.ink_muted)
}

pub fn dirty_dot(palette: &Palette, dirty: bool) -> Div {
    div().size(px(6.)).rounded_full().bg(if dirty {
        palette.notice_text
    } else {
        palette.ink_faint.opacity(0.4)
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaveState {
    Saved,
    Unsaved,
    Saving,
}

pub fn save_state(palette: &Palette, state: SaveState) -> Div {
    let row = h_flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .px(px(4.))
        .text_size(px(12.))
        .line_height(px(16.));
    match state {
        SaveState::Saved => row
            .text_color(palette.ink_faint)
            .child(sized_icon(IconName::Check, ICON_SM, palette.ink_faint))
            .child("Saved"),
        SaveState::Unsaved => row
            .text_color(palette.ink)
            .child(dirty_dot(palette, true))
            .child("Unsaved"),
        SaveState::Saving => row.text_color(palette.ink_faint).child("Saving…"),
    }
}

pub fn save_status(palette: &Palette, dirty: bool) -> impl IntoElement {
    save_state(
        palette,
        if dirty {
            SaveState::Unsaved
        } else {
            SaveState::Saved
        },
    )
}

pub fn key_cap(palette: &Palette, text: impl Into<SharedString>) -> Div {
    keycap(palette, text)
}

pub fn key_hint(palette: &Palette, keys: &str) -> Div {
    keycaps(palette, keys)
}

pub fn spinner_text(palette: &Palette, text: &'static str) -> Div {
    div()
        .text_size(px(12.))
        .text_color(palette.ink_muted)
        .child(text)
}
