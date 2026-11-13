use std::rc::Rc;

use gpui_kit::{Action, App, KeyBinding, KeyBindingContextPredicate, actions};

use crate::platform::primary_modifier_label;

actions!(
    missal,
    [
        ToggleCommandPalette,
        ShowShortcuts,
        ToggleSidebar,
        FocusSearch,
        Save,
        BlurFocus,
        ConfirmSelection,
        GoFirs,
        GoTemplates,
        GoSettings,
        HistoryBack,
        HistoryForward,
        NewFir,
        NextRow,
        PreviousRow,
        ToggleRow,
        OpenRow,
        EditFir,
        Print,
        NextDocument,
        PreviousDocument,
        AddTemplates,
        ToggleFieldNames,
        NewTemplate,
        ImportWord,
        NextTemplate,
        PreviousTemplate,
    ]
);

pub const SHELL: &str = "Missal";
pub const FIR_LIST: &str = "FirList";
pub const WORKSPACE: &str = "Workspace";
pub const TEMPLATES: &str = "Templates";
pub const TEXT_INPUT: &str = "MissalTextInput";
pub const NOT_TYPING: &str = "Missal && !MissalTextInput";
const FIR_LIST_NOT_TYPING: &str = "FirList && !MissalTextInput";
const WORKSPACE_NOT_TYPING: &str = "Workspace && !MissalTextInput";
const TEMPLATES_NOT_TYPING: &str = "Templates && !MissalTextInput";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    General,
    Navigation,
    Firs,
    Documents,
    Templates,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::General,
        Group::Navigation,
        Group::Firs,
        Group::Documents,
        Group::Templates,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Group::General => "General",
            Group::Navigation => "Navigation",
            Group::Firs => "FIRs",
            Group::Documents => "Documents",
            Group::Templates => "Templates",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Shortcut {
    pub keys: &'static str,
    pub label: &'static str,
    pub group: Group,
}

pub const SHORTCUTS: &[Shortcut] = &[
    Shortcut { keys: "Mod+K", label: "Open command menu", group: Group::General },
    Shortcut { keys: "?", label: "Show keyboard shortcuts", group: Group::General },
    Shortcut { keys: "Mod+B", label: "Toggle sidebar", group: Group::General },
    Shortcut { keys: "/", label: "Search this page", group: Group::General },
    Shortcut { keys: "Mod+S", label: "Save", group: Group::General },
    Shortcut { keys: "Escape", label: "Leave the field", group: Group::General },
    Shortcut { keys: "Mod+Enter", label: "Confirm a selection", group: Group::General },
    Shortcut { keys: "G then H", label: "Go to FIRs", group: Group::Navigation },
    Shortcut { keys: "G then T", label: "Go to templates", group: Group::Navigation },
    Shortcut { keys: "G then S", label: "Go to settings", group: Group::Navigation },
    Shortcut { keys: "Alt+Left", label: "Back", group: Group::Navigation },
    Shortcut { keys: "Alt+Right", label: "Forward", group: Group::Navigation },
    Shortcut { keys: "N", label: "New FIR", group: Group::Firs },
    Shortcut { keys: "J", label: "Next row", group: Group::Firs },
    Shortcut { keys: "K", label: "Previous row", group: Group::Firs },
    Shortcut { keys: "X", label: "Select row", group: Group::Firs },
    Shortcut { keys: "Enter", label: "Open FIR", group: Group::Firs },
    Shortcut { keys: "E", label: "Edit FIR details", group: Group::Firs },
    Shortcut { keys: "Mod+P", label: "Print preview", group: Group::Documents },
    Shortcut { keys: "Alt+Down", label: "Next document", group: Group::Documents },
    Shortcut { keys: "Alt+Up", label: "Previous document", group: Group::Documents },
    Shortcut { keys: "A", label: "Add templates to FIR", group: Group::Documents },
    Shortcut { keys: "Mod+Shift+L", label: "Show field names", group: Group::Documents },
    Shortcut { keys: "Shift+N", label: "New template", group: Group::Templates },
    Shortcut { keys: "Mod+O", label: "Upload Word", group: Group::Templates },
    Shortcut { keys: "Alt+Down", label: "Next template", group: Group::Templates },
    Shortcut { keys: "Alt+Up", label: "Previous template", group: Group::Templates },
];

pub fn key_caps(keys: &str) -> Vec<Vec<String>> {
    keys.split(" then ")
        .map(|step| {
            step.split('+')
                .map(|part| {
                    if part == "Mod" {
                        primary_modifier_label().to_owned()
                    } else {
                        part.to_owned()
                    }
                })
                .collect()
        })
        .collect()
}

pub fn bind_keys(cx: &mut App) {
    let bindings = [
        binding(cx, "secondary-k", ToggleCommandPalette, Some(SHELL)),
        binding(cx, "?", ShowShortcuts, Some(NOT_TYPING)),
        binding(cx, "shift-/", ShowShortcuts, Some(NOT_TYPING)),
        binding(cx, "secondary-b", ToggleSidebar, Some(NOT_TYPING)),
        binding(cx, "/", FocusSearch, Some(NOT_TYPING)),
        binding(cx, "secondary-s", Save, None),
        binding(cx, "escape", BlurFocus, Some(SHELL)),
        binding(cx, "secondary-enter", ConfirmSelection, None),
        binding(cx, "g h", GoFirs, Some(NOT_TYPING)),
        binding(cx, "g t", GoTemplates, Some(NOT_TYPING)),
        binding(cx, "g s", GoSettings, Some(NOT_TYPING)),
        binding(cx, "alt-left", HistoryBack, Some(SHELL)),
        binding(cx, "alt-right", HistoryForward, Some(SHELL)),
        binding(cx, "n", NewFir, Some(NOT_TYPING)),
        binding(cx, "j", NextRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "down", NextRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "k", PreviousRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "up", PreviousRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "x", ToggleRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "space", ToggleRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "enter", OpenRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "e", EditFir, Some(NOT_TYPING)),
        binding(cx, "secondary-p", Print, Some(SHELL)),
        binding(cx, "alt-down", NextDocument, Some(WORKSPACE)),
        binding(cx, "alt-up", PreviousDocument, Some(WORKSPACE)),
        binding(cx, "a", AddTemplates, Some(WORKSPACE_NOT_TYPING)),
        binding(cx, "secondary-shift-l", ToggleFieldNames, Some(WORKSPACE)),
        binding(cx, "shift-n", NewTemplate, Some(NOT_TYPING)),
        binding(cx, "secondary-o", ImportWord, Some(TEMPLATES)),
        binding(cx, "alt-down", NextTemplate, Some(TEMPLATES)),
        binding(cx, "alt-up", PreviousTemplate, Some(TEMPLATES)),
        binding(cx, "down", NextTemplate, Some(TEMPLATES_NOT_TYPING)),
        binding(cx, "up", PreviousTemplate, Some(TEMPLATES_NOT_TYPING)),
    ];
    cx.bind_keys(bindings);
}

fn binding<A: Action>(cx: &App, keys: &str, action: A, context: Option<&str>) -> KeyBinding {
    let predicate = context.map(|context| {
        Rc::new(KeyBindingContextPredicate::parse(context).expect("a valid key context"))
    });
    KeyBinding::load(keys, Box::new(action), predicate, false, None, cx).expect("a valid keystroke")
}
