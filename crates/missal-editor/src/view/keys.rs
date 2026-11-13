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

fn key(keys: &str, action: impl gpui_kit::Action, cx: &App) -> KeyBinding {
    let predicate = gpui_kit::KeyBindingContextPredicate::parse(KEY_CONTEXT)
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
    ];
    cx.bind_keys(bindings);
    if !cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("ctrl-up", ParagraphUp, c),
            KeyBinding::new("ctrl-down", ParagraphDown, c),
            KeyBinding::new("ctrl-shift-up", ParagraphUp, c),
            KeyBinding::new("ctrl-shift-down", ParagraphDown, c),
        ]);
    }
}
