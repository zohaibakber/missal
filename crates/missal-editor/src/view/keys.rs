use gpui_kit::{App, KeyBinding, actions};

use super::find::{FIND_CONTEXT, QUERY_CONTEXT};

actions!(
    editor,
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
        Find,
        FindReplace,
        FindNext,
        FindPrevious,
        CloseFind,
        ReplaceAll,
        ToggleMatchCase,
    ]
);

pub const KEY_CONTEXT: &str = "Editor";

fn key(keys: &str, action: impl gpui_kit::Action, cx: &App) -> KeyBinding {
    key_in(keys, action, KEY_CONTEXT, cx)
}

fn key_in(keys: &str, action: impl gpui_kit::Action, context: &str, cx: &App) -> KeyBinding {
    let predicate = gpui_kit::KeyBindingContextPredicate::parse(context)
        .ok()
        .map(std::rc::Rc::new);
    KeyBinding::load(keys, Box::new(action), predicate, false, None, cx).expect("a valid keystroke")
}

pub fn bind_keys(cx: &mut App) {
    let c = Some(KEY_CONTEXT);
    let bindings = [
        KeyBinding::new("tab", NextCell, c),
        KeyBinding::new("shift-tab", PreviousCell, c),
        KeyBinding::new("pageup", PageUp, c),
        KeyBinding::new("pagedown", PageDown, c),
        KeyBinding::new("shift-pageup", SelectPageUp, c),
        KeyBinding::new("shift-pagedown", SelectPageDown, c),
        key("secondary-b", ToggleBold, cx),
        key("secondary-i", ToggleItalic, cx),
        key("secondary-u", ToggleUnderline, cx),
        key("secondary-l", AlignLeft, cx),
        key("secondary-e", AlignCenter, cx),
        key("secondary-r", AlignRight, cx),
        key("secondary-j", AlignJustify, cx),
        key("secondary-shift-8", BulletList, cx),
        key("secondary-shift-7", NumberedList, cx),
        key("secondary-]", IndentList, cx),
        key("secondary-[", OutdentList, cx),
        key("secondary-=", ZoomIn, cx),
        key("secondary-+", ZoomIn, cx),
        key("secondary-shift-=", ZoomIn, cx),
        key("secondary--", ZoomOut, cx),
        key("secondary-0", ZoomReset, cx),
        key("secondary-shift-f", InsertField, cx),
        key("secondary-f", Find, cx),
        key("f3", FindNext, cx),
        key("shift-f3", FindPrevious, cx),
        key("secondary-g", FindNext, cx),
        key("secondary-shift-g", FindPrevious, cx),
        key_in("escape", CloseFind, FIND_CONTEXT, cx),
        key_in("shift-enter", FindPrevious, QUERY_CONTEXT, cx),
        key_in("secondary-alt-enter", ReplaceAll, FIND_CONTEXT, cx),
        key_in("alt-c", ToggleMatchCase, FIND_CONTEXT, cx),
    ];
    cx.bind_keys(bindings);
    if cfg!(target_os = "macos") {
        cx.bind_keys([key("cmd-alt-f", FindReplace, cx)]);
    } else {
        cx.bind_keys([key("ctrl-h", FindReplace, cx)]);
    }
    if !cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("ctrl-up", ParagraphUp, c),
            KeyBinding::new("ctrl-down", ParagraphDown, c),
            KeyBinding::new("ctrl-shift-up", ParagraphUp, c),
            KeyBinding::new("ctrl-shift-down", ParagraphDown, c),
        ]);
    }
    super::slash::bind_keys(cx);
}
