use gpui_kit::{App, KeyBinding, actions};

actions!(
    missal_editor,
    [
        NextCell,
        PreviousCell,
        PageUp,
        PageDown,
        SelectPageUp,
        SelectPageDown,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        AlignLeft,
        AlignCenter,
        AlignRight,
        AlignJustify,
        BulletList,
        NumberedList,
        IndentList,
        OutdentList,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        InsertField,
        ParagraphUp,
        ParagraphDown,
    ]
);

pub const KEY_CONTEXT: &str = "MissalEditor";

pub fn bind_keys(cx: &mut App) {
    let c = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("tab", NextCell, c),
        KeyBinding::new("shift-tab", PreviousCell, c),
        KeyBinding::new("pageup", PageUp, c),
        KeyBinding::new("pagedown", PageDown, c),
        KeyBinding::new("shift-pageup", SelectPageUp, c),
        KeyBinding::new("shift-pagedown", SelectPageDown, c),
        KeyBinding::new("secondary-b", ToggleBold, c),
        KeyBinding::new("secondary-i", ToggleItalic, c),
        KeyBinding::new("secondary-u", ToggleUnderline, c),
        KeyBinding::new("secondary-l", AlignLeft, c),
        KeyBinding::new("secondary-e", AlignCenter, c),
        KeyBinding::new("secondary-r", AlignRight, c),
        KeyBinding::new("secondary-j", AlignJustify, c),
        KeyBinding::new("secondary-shift-8", BulletList, c),
        KeyBinding::new("secondary-shift-7", NumberedList, c),
        KeyBinding::new("secondary-]", IndentList, c),
        KeyBinding::new("secondary-[", OutdentList, c),
        KeyBinding::new("secondary-=", ZoomIn, c),
        KeyBinding::new("secondary-+", ZoomIn, c),
        KeyBinding::new("secondary-shift-=", ZoomIn, c),
        KeyBinding::new("secondary--", ZoomOut, c),
        KeyBinding::new("secondary-0", ZoomReset, c),
        KeyBinding::new("secondary-shift-f", InsertField, c),
    ]);
    if !cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("ctrl-up", ParagraphUp, c),
            KeyBinding::new("ctrl-down", ParagraphDown, c),
            KeyBinding::new("ctrl-shift-up", ParagraphUp, c),
            KeyBinding::new("ctrl-shift-down", ParagraphDown, c),
        ]);
    }
}
