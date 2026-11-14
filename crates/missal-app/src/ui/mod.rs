pub mod cheatsheet;
pub mod dialogs;
pub mod input;
pub mod palette;
pub mod print_preview;
pub mod sidebar;
pub mod status;
pub mod titlebar;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::{AnyElement, Div, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px};
use missal_text::label;

use crate::platform::primary_modifier_label;
use crate::shortcuts::key_caps;
use crate::theme::{PANE_HEADER_HEIGHT, Palette, RAIL_HEADER_HEIGHT, STATUS_BAR_HEIGHT};

pub fn icon(name: IconName) -> Icon {
    Icon::new(name)
}

pub fn small_icon(name: IconName, color: Hsla) -> Icon {
    Icon::new(name).size_4().text_color(color)
}

pub fn icon_button(id: impl Into<gpui_kit::ElementId>, name: IconName) -> Button {
    Button::new(id).icon(name).ghost().small()
}

pub fn tooltip_with_keys(
    text: &'static str,
    keys: Option<&'static str>,
) -> impl Fn(&mut gpui_kit::Window, &mut gpui_kit::App) -> gpui_kit::AnyView + 'static {
    move |window, cx| {
        let text = match keys {
            Some(keys) => format!(
                "{text}  ({})",
                keys.replace("Mod", primary_modifier_label())
            ),
            None => text.to_owned(),
        };
        Tooltip::new(text).build(window, cx)
    }
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
        .border_color(palette.border)
}

pub fn rail_header(palette: &Palette, title: &'static str) -> Div {
    h_flex()
        .flex_shrink_0()
        .h(px(RAIL_HEADER_HEIGHT))
        .items_center()
        .gap_2()
        .pl_4()
        .pr_1p5()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.))
                .font_medium()
                .text_color(palette.muted_foreground)
                .child(title),
        )
}

pub fn status_bar(palette: &Palette) -> Div {
    h_flex()
        .flex_shrink_0()
        .h(px(STATUS_BAR_HEIGHT))
        .items_center()
        .gap_3()
        .px_4()
        .border_t_1()
        .border_color(palette.border)
        .text_size(px(12.))
        .text_color(palette.muted_foreground)
}

pub fn dirty_dot(palette: &Palette, dirty: bool) -> Div {
    div().size(px(6.)).rounded_full().bg(if dirty {
        palette.status_open
    } else {
        palette.muted_foreground.opacity(0.4)
    })
}

pub fn save_status(palette: &Palette, dirty: bool) -> impl IntoElement {
    h_flex()
        .items_center()
        .gap_1p5()
        .px_2()
        .text_size(px(12.))
        .text_color(if dirty {
            palette.foreground
        } else {
            palette.muted_foreground
        })
        .child(dirty_dot(palette, dirty))
        .child(if dirty { "Unsaved" } else { "Saved" })
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
        .gap_3()
        .p_6()
        .child(
            div()
                .size(px(40.))
                .rounded(px(10.))
                .bg(palette.muted)
                .flex()
                .items_center()
                .justify_center()
                .child(small_icon(name, palette.muted_foreground)),
        )
        .child(
            v_flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.))
                        .font_medium()
                        .child(label(title.into())),
                )
                .children(description.map(|description| {
                    div()
                        .max_w(px(360.))
                        .text_size(px(13.))
                        .text_color(palette.muted_foreground)
                        .child(label(description).align(missal_text::Align::Center))
                })),
        )
        .children(action)
}

pub fn key_cap(palette: &Palette, text: impl Into<SharedString>) -> Div {
    div()
        .px(px(5.))
        .min_w(px(20.))
        .h(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(palette.border)
        .bg(palette.muted)
        .text_size(px(11.))
        .font_medium()
        .text_color(palette.muted_foreground)
        .child(text.into())
}

pub fn key_hint(palette: &Palette, keys: &str) -> Div {
    h_flex()
        .items_center()
        .gap_1()
        .children(
            key_caps(keys)
                .into_iter()
                .enumerate()
                .flat_map(|(step, caps)| {
                    let mut items: Vec<AnyElement> = Vec::new();
                    if step > 0 {
                        items.push(
                            div()
                                .text_size(px(11.))
                                .text_color(palette.muted_foreground)
                                .child("then")
                                .into_any_element(),
                        );
                    }
                    items.extend(
                        caps.into_iter()
                            .map(|cap| key_cap(palette, cap).into_any_element()),
                    );
                    items
                }),
        )
}

pub fn skeleton_bar(palette: &Palette, width: f32, height: f32) -> Div {
    div()
        .w(px(width))
        .h(px(height))
        .rounded(px(5.))
        .bg(palette.muted)
}

pub fn spinner_text(palette: &Palette, text: &'static str) -> Div {
    div()
        .text_size(px(12.))
        .text_color(palette.muted_foreground)
        .child(text)
}
