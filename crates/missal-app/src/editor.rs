use gpui_kit::{App, Entity, Window};
use missal_editor::{Editor, EditorTheme};

use crate::theme::Palette;

pub fn focus(editor: &Entity<Editor>, window: &mut Window, cx: &mut App) {
    let handle = editor.read(cx).focus_handle_ref().clone();
    window.focus(&handle, cx);
}

pub fn install(cx: &mut App) {
    missal_editor::init(cx);
    sync_theme(cx);
    cx.observe_global::<Palette>(sync_theme).detach();
}

fn sync_theme(cx: &mut App) {
    let Some(palette) = cx.try_global::<Palette>().copied() else {
        return;
    };
    EditorTheme {
        surface: palette.surface,
        raised: palette.raised,
        canvas: palette.sunken,
        paper: palette.paper,
        ink: palette.ink,
        ink_muted: palette.ink_muted,
        ink_faint: palette.ink_faint,
        line: palette.line,
        line_strong: palette.line_strong,
        hover: palette.hover,
        pressed: palette.pressed,
        accent: palette.accent,
        field_wash: palette.field_wash,
        notice: palette.notice,
        notice_text: palette.notice_text,
        notice_wash: palette.notice_wash,
        selection: palette
            .accent
            .opacity(if palette.dark { 0.40 } else { 0.28 }),
        shadow: if palette.dark {
            gpui_kit::black().opacity(0.35)
        } else {
            crate::theme::hex(0x14110e).opacity(0.05)
        },
    }
    .install(cx);
}
