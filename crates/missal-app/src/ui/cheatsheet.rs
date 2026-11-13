use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{StyledExt, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, IntoElement, ParentElement, Styled, Window, div, px};

use crate::shortcuts::{Group, SHORTCUTS};
use crate::theme::Palette;
use crate::ui::key_hint;

pub fn content(palette: &Palette) -> impl IntoElement {
    let column = |groups: &[Group]| {
        v_flex().flex_1().min_w_0().gap_4().children(groups.iter().map(|group| {
            v_flex()
                .gap_1()
                .child(
                    div()
                        .pb_1()
                        .text_size(px(12.))
                        .font_medium()
                        .text_color(palette.muted_foreground)
                        .child(group.title()),
                )
                .children(SHORTCUTS.iter().filter(|shortcut| shortcut.group == *group).map(|shortcut| {
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .h(px(26.))
                        .child(div().text_size(px(13.)).child(shortcut.label))
                        .child(key_hint(palette, shortcut.keys))
                }))
        }))
    };
    v_flex()
        .gap_4()
        .pb_2()
        .child(
            h_flex()
                .gap_8()
                .items_start()
                .child(column(&[Group::General, Group::Navigation]))
                .child(column(&[Group::Firs, Group::Documents, Group::Templates])),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(palette.muted_foreground)
                .child("Single-key shortcuts work when no text field or document is focused. Press Escape to leave one."),
        )
}

pub fn open(window: &mut Window, cx: &mut App) {
    if window.has_active_dialog(cx) {
        return;
    }
    window.open_dialog(cx, |dialog: Dialog, _, cx| {
        let palette = *Palette::get(cx);
        dialog
            .w(px(700.))
            .title(div().text_size(px(16.)).font_semibold().child("Keyboard shortcuts"))
            .child(content(&palette))
    });
}
