use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, div, px,
};
use missal_text::{Align, LineHeight, label};

use crate::theme::{
    ICON, ICON_XS, KEYCAP, Palette, RADIUS, RADIUS_2XL, RADIUS_CHIP, RADIUS_SM, RADIUS_XL,
    SIDEBAR_ITEM, TABLE_FOOT, TABLE_HEAD,
};
use crate::ui::part::{Part, part};
use crate::ui::{sized_icon, tabular};

pub fn card() -> Part {
    part(|base, palette| {
        base.rounded(px(RADIUS_2XL))
            .bg(palette.surface)
            .shadow(palette.shadow_xs())
    })
}

pub fn card_header() -> Part {
    part(|base, palette| base.v_flex().gap(px(4.)).text_color(palette.ink))
}

pub fn card_title() -> Part {
    part(|base, palette| {
        base.h_flex()
            .gap(px(8.))
            .text_size(px(18.))
            .line_height(px(28.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.ink)
    })
}

pub fn card_description() -> Part {
    part(|base, palette| {
        base.text_size(px(14.))
            .line_height(px(20.))
            .text_color(palette.ink_muted)
    })
}

pub fn frame() -> Part {
    part(|base, palette| {
        base.v_flex()
            .p(px(4.))
            .rounded(px(RADIUS_2XL))
            .bg(palette.hover.opacity(0.72))
    })
}

pub fn frame_panel() -> Part {
    part(|base, palette| {
        base.rounded(px(RADIUS_XL))
            .bg(palette.surface)
            .shadow(palette.shadow_xs())
    })
}

pub fn toolbar() -> Part {
    part(|base, palette| {
        base.h_flex()
            .flex_none()
            .p(px(4.))
            .gap(px(2.))
            .rounded(px(RADIUS_XL))
            .bg(palette.raised)
            .shadow(palette.shadow_lg())
            .occlude()
    })
}

pub fn toolbar_separator() -> Part {
    part(|base, palette| {
        base.flex_none()
            .w(px(1.))
            .h(px(20.))
            .mx(px(4.))
            .bg(palette.line)
    })
}

pub fn floating_bar() -> Part {
    part(|base, palette| {
        base.h_flex()
            .flex_none()
            .h(px(36.))
            .p(px(4.))
            .gap(px(2.))
            .rounded(px(RADIUS_XL))
            .bg(palette.raised)
            .shadow(palette.shadow_lg())
            .occlude()
            .text_size(px(14.))
            .text_color(palette.ink_muted)
            .font_features(tabular())
    })
}

pub fn separator() -> Part {
    part(|base, palette| base.flex_none().h(px(1.)).w_full().bg(palette.line))
}

pub fn skeleton() -> Part {
    part(|base, palette| base.rounded(px(RADIUS_SM)).bg(palette.hover))
}

pub fn skeleton_bar(width: f32, height: f32) -> Part {
    skeleton().w(px(width)).h(px(height))
}

pub fn table() -> Part {
    part(|base, palette| {
        base.v_flex()
            .size_full()
            .p(px(4.))
            .pt(px(0.))
            .rounded(px(RADIUS_2XL))
            .bg(palette.hover.opacity(0.72))
    })
}

pub fn table_header() -> Part {
    part(|base, palette| {
        base.h_flex()
            .flex_none()
            .h(px(TABLE_HEAD))
            .text_size(px(14.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.ink_muted)
    })
}

pub fn table_body() -> Part {
    part(|base, palette| {
        base.flex_1()
            .min_h_0()
            .overflow_hidden()
            .rounded(px(RADIUS_XL))
            .bg(palette.surface)
            .shadow(palette.shadow_xs())
    })
}

pub fn table_footer() -> Part {
    part(|base, palette| {
        base.h_flex()
            .flex_none()
            .h(px(TABLE_FOOT))
            .px(px(10.))
            .pt(px(4.))
            .gap(px(12.))
            .items_center()
            .justify_between()
            .text_size(px(13.))
            .line_height(px(18.))
            .text_color(palette.ink_muted)
            .font_features(tabular())
    })
}

pub fn table_head() -> Part {
    part(|base, _| base.h_flex().h_full().px(px(10.)).whitespace_nowrap())
}

pub fn table_cell() -> Part {
    part(|base, _| base.h_flex().h_full().px(px(10.)).min_w_0())
}

pub fn empty() -> Part {
    part(|base, _| {
        base.v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .px(px(24.))
            .py(px(48.))
    })
}

pub fn empty_header() -> Part {
    part(|base, _| base.v_flex().items_center().max_w(px(384.)))
}

pub fn empty_content() -> Part {
    part(|base, _| base.v_flex().items_center().gap(px(8.)).mt(px(24.)))
}

pub fn empty_media(icon: IconName) -> Part {
    part(move |base, palette| {
        let tile = |size: f32| {
            div()
                .size(px(size))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(RADIUS))
                .bg(palette.surface)
                .shadow(palette.shadow_sm())
        };
        base.relative()
            .mb(px(24.))
            .w(px(72.))
            .h(px(40.))
            .child(
                div()
                    .absolute()
                    .left(px(10.))
                    .top(px(5.))
                    .child(tile(30.).opacity(0.84)),
            )
            .child(
                div()
                    .absolute()
                    .right(px(10.))
                    .top(px(5.))
                    .child(tile(30.).opacity(0.84)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(18.))
                    .top(px(0.))
                    .child(tile(36.).child(sized_icon(icon, 18., palette.ink))),
            )
    })
}

pub fn empty_title(text: impl Into<SharedString>) -> Part {
    part(|base, palette| {
        base.text_size(px(20.))
            .line_height(px(28.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.ink)
    })
    .child(
        label(text.into())
            .align(Align::Center)
            .leading(LineHeight::Px(28.)),
    )
}

pub fn empty_description(text: impl Into<SharedString>) -> Part {
    part(|base, palette| {
        base.mt(px(4.))
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(palette.ink_muted)
    })
    .child(
        label(text.into())
            .align(Align::Center)
            .leading(LineHeight::Px(20.)),
    )
}

pub fn empty_state_body(
    icon: IconName,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    action: Option<AnyElement>,
) -> Part {
    empty_header()
        .child(empty_media(icon))
        .child(empty_title(title))
        .children(description.map(empty_description))
        .children(action.map(|action| empty_content().child(action)))
}

pub fn empty_state(
    icon: IconName,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    action: Option<AnyElement>,
) -> Part {
    empty().child(empty_state_body(icon, title, description, action))
}

pub fn sidebar_group() -> Part {
    part(|base, _| base.v_flex().w_full().gap(px(2.)))
}

pub fn sidebar_group_label() -> Part {
    part(|base, palette| {
        base.h_flex()
            .h(px(SIDEBAR_ITEM))
            .px(px(8.))
            .gap(px(6.))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.sidebar_ink)
    })
}

pub fn section_label(text: impl Into<SharedString>) -> Part {
    sidebar_group_label().child(text.into())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intent {
    Neutral,
    Warning,
    Info,
    Success,
    Danger,
}

impl Intent {
    fn colors(self, palette: &Palette) -> (Hsla, Hsla) {
        match self {
            Intent::Neutral => (palette.ink_muted, palette.ink_muted),
            Intent::Warning => (palette.notice, palette.notice_text),
            Intent::Info => (palette.st_investigating, palette.accent),
            Intent::Success => (palette.success, palette.success_text),
            Intent::Danger => (palette.danger, palette.danger_text),
        }
    }

    fn icon(self) -> IconName {
        match self {
            Intent::Neutral | Intent::Info => IconName::Info,
            Intent::Warning | Intent::Danger => IconName::TriangleAlert,
            Intent::Success => IconName::CircleCheck,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BadgeVariant {
    Outline,
    Secondary,
    Intent(Intent),
}

pub fn badge(variant: BadgeVariant) -> Part {
    part(move |base, palette| {
        let clear = gpui_kit::transparent_black();
        let (fill, text, border) = match variant {
            BadgeVariant::Outline => (palette.control_fill(), palette.ink, palette.line_input),
            BadgeVariant::Secondary => (palette.hover, palette.ink, clear),
            BadgeVariant::Intent(Intent::Neutral) => (palette.hover, palette.ink_muted, clear),
            BadgeVariant::Intent(intent) => {
                let (tint, text) = intent.colors(palette);
                (
                    tint.opacity(if palette.dark { 0.16 } else { 0.08 }),
                    text,
                    clear,
                )
            }
        };
        base.h_flex()
            .flex_none()
            .h(px(18.))
            .min_w(px(18.))
            .px(px(3.))
            .gap(px(4.))
            .justify_center()
            .rounded(px(RADIUS_SM))
            .border_1()
            .border_color(border)
            .bg(fill)
            .text_color(text)
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .font_features(tabular())
            .whitespace_nowrap()
    })
}

pub fn count_badge(count: usize) -> Part {
    badge(BadgeVariant::Intent(Intent::Neutral)).child(count.to_string())
}

pub fn pill() -> Part {
    badge(BadgeVariant::Outline)
        .h(px(22.))
        .px(px(7.))
        .gap(px(6.))
}

pub fn alert(intent: Intent) -> Part {
    part(move |base, palette| {
        let (tint, _) = intent.colors(palette);
        let (border, fill) = match intent {
            Intent::Neutral => (palette.line, palette.surface),
            _ => (tint.opacity(0.32), tint.opacity(0.04)),
        };
        base.relative()
            .v_flex()
            .gap(px(2.))
            .pl(px(38.))
            .pr(px(14.))
            .py(px(12.))
            .rounded(px(RADIUS_XL))
            .border_1()
            .border_color(border)
            .bg(fill)
            .text_color(palette.ink)
            .text_size(px(14.))
            .line_height(px(20.))
            .child(
                div()
                    .absolute()
                    .left(px(13.))
                    .top(px(11.))
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .child(sized_icon(intent.icon(), ICON, tint)),
            )
    })
}

pub fn alert_title() -> Part {
    part(|base, palette| base.font_weight(FontWeight::MEDIUM).text_color(palette.ink))
}

pub fn alert_description() -> Part {
    part(|base, palette| base.text_color(palette.ink_muted))
}

pub fn dirty_dot(dirty: bool) -> Part {
    part(move |base, palette| {
        base.flex_none().size(px(6.)).rounded_full().bg(if dirty {
            palette.notice
        } else {
            palette.ink_faint.opacity(0.4)
        })
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaveState {
    Saved,
    Unsaved,
    Saving,
}

pub fn save_state(state: SaveState) -> Part {
    match state {
        SaveState::Saved => part(|base, _| base.h_flex().flex_none()),
        SaveState::Unsaved => badge(BadgeVariant::Intent(Intent::Warning))
            .h(px(22.))
            .px(px(7.))
            .gap(px(6.))
            .child(dirty_dot(true))
            .child("Unsaved"),
        SaveState::Saving => part(|base, palette| {
            base.h_flex()
                .flex_none()
                .px(px(4.))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(palette.ink_faint)
        })
        .child("Saving…"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyTone {
    Plain,
    Bare,
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

pub fn kbd(text: impl Into<SharedString>) -> Part {
    kbd_toned(text, KeyTone::Plain)
}

fn kbd_toned(text: impl Into<SharedString>, tone: KeyTone) -> Part {
    part(move |base, palette| {
        let (fill, color) = match tone {
            KeyTone::Plain => (Some(palette.hover), palette.ink_muted),
            KeyTone::Bare => (None, palette.ink_faint),
        };
        base.flex_none()
            .h(px(KEYCAP))
            .min_w(px(KEYCAP))
            .px(px(4.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(RADIUS_CHIP))
            .when_some(fill, |this, fill| this.bg(fill))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(color)
    })
    .child(text.into())
}

pub struct KbdGroup {
    keys: SharedString,
    tone: KeyTone,
    chord: bool,
}

pub fn kbd_group(keys: impl Into<SharedString>) -> KbdGroup {
    KbdGroup {
        keys: keys.into(),
        tone: KeyTone::Plain,
        chord: false,
    }
}

impl KbdGroup {
    pub fn tone(mut self, tone: KeyTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn chord(mut self) -> Self {
        self.chord = true;
        self
    }
}

impl IntoElement for KbdGroup {
    type Element = Part;

    fn into_element(self) -> Part {
        let caps = crate::shortcuts::key_caps(&self.keys);
        if self.tone == KeyTone::Bare {
            let text = caps
                .into_iter()
                .map(|caps| {
                    caps.iter()
                        .map(|cap| match display_key(cap) {
                            key if key == "↵" => "Enter".to_owned(),
                            key => key,
                        })
                        .collect::<Vec<_>>()
                        .join("+")
                })
                .collect::<Vec<_>>()
                .join(", ");
            return part(|base, palette| {
                base.flex_none()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_faint)
            })
            .child(text);
        }
        let tone = self.tone;
        let chord = self.chord;
        part(|base, _| base.h_flex().flex_none().gap(px(3.))).children(
            caps.into_iter().enumerate().flat_map(move |(step, caps)| {
                let mut items: Vec<AnyElement> = Vec::new();
                if chord && step > 0 {
                    items.push(
                        part(|base, palette| {
                            base.child(sized_icon(
                                IconName::ChevronRight,
                                ICON_XS,
                                palette.ink_faint,
                            ))
                        })
                        .into_any_element(),
                    );
                }
                items.extend(
                    caps.into_iter()
                        .map(|cap| kbd_toned(display_key(&cap), tone).into_any_element()),
                );
                items
            }),
        )
    }
}
