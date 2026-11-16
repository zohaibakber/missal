use gpui_kit::{App, AppContext, Entity, Global, Window};
use missal_editor::{Editor, EditorTheme};

use crate::theme::Palette;

struct SharedEditor(Entity<Editor>);

impl Global for SharedEditor {}

pub fn shared(window: &mut Window, cx: &mut App) -> Entity<Editor> {
    if let Some(shared) = cx.try_global::<SharedEditor>() {
        let editor = shared.0.clone();
        editor.update(cx, |editor, cx| editor.reset(cx));
        return editor;
    }
    let editor = cx.new(|cx| Editor::new(window, cx));
    cx.set_global(SharedEditor(editor.clone()));
    editor
}

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
            .opacity(if palette.dark { 0.36 } else { 0.22 }),
        shadow: if palette.dark {
            gpui_kit::black().opacity(0.35)
        } else {
            gpui_kit::black().opacity(0.06)
        },
    }
    .install(cx);
}
