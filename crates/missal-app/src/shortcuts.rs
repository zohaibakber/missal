use std::rc::Rc;

use gpui_kit::{Action, App, KeyBinding, KeyBindingContextPredicate, actions};

use crate::platform::primary_modifier_label;

actions!(
    missal,
    [
        ToggleCommandPalette,
        ShowShortcuts,
        ToggleNavMenu,
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
        DeleteSelection,
        Print,
        SavePdf,
        NextDocument,
        PreviousDocument,
        MoveDocumentUp,
        MoveDocumentDown,
        AddTemplates,
        ToggleFieldNames,
        ToggleFieldsPanel,
        NewTemplate,
        ImportWord,
        RenameTemplate,
        NextTemplate,
        PreviousTemplate,
        PreviewNextPage,
        PreviewPreviousPage,
        PreviewZoomIn,
        PreviewZoomOut,
        PreviewZoomFit,
        PreviewClose,
    ]
);

pub const SHELL: &str = "Missal";
pub const FIR_LIST: &str = "FirList";
pub const WORKSPACE: &str = "Workspace";
pub const TEMPLATES: &str = "Templates";
pub const TEXT_INPUT: &str = "MissalTextInput";
pub const PRINT_PREVIEW: &str = "PrintPreview";
pub const PRINT_PREVIEW_CONTEXT: &str = "PrintPreview MissalTextInput";
pub const NOT_TYPING: &str = "Missal && !MissalTextInput && !NavMenu";
const FIR_LIST_NOT_TYPING: &str = "FirList && !MissalTextInput";
const WORKSPACE_NOT_TYPING: &str = "Workspace && !MissalTextInput";
const TEMPLATES_NOT_TYPING: &str = "Templates && !MissalTextInput";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    General,
    FirList,
    Workspace,
    Editor,
    Templates,
    Print,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::General,
        Group::FirList,
        Group::Workspace,
        Group::Editor,
        Group::Templates,
        Group::Print,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Group::General => "General",
            Group::FirList => "FIR list",
            Group::Workspace => "FIR workspace",
            Group::Editor => "Editor",
            Group::Templates => "Templates",
            Group::Print => "Print",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Shortcut {
    pub keys: &'static str,
    pub label: &'static str,
    pub group: Group,
}

const fn shortcut(keys: &'static str, label: &'static str, group: Group) -> Shortcut {
    Shortcut { keys, label, group }
}

pub const SHORTCUTS: &[Shortcut] = &[
    shortcut("Mod+K", "Search and commands", Group::General),
    shortcut("F10", "Open the menu", Group::General),
    shortcut("?", "Keyboard shortcuts", Group::General),
    shortcut("G then H", "Go to FIRs", Group::General),
    shortcut("G then T", "Go to templates", Group::General),
    shortcut("G then S", "Go to settings", Group::General),
    shortcut("Alt+Left", "Back", Group::General),
    shortcut("Alt+Right", "Forward", Group::General),
    shortcut("N", "New FIR", Group::General),
    shortcut("Mod+O", "Import Word file", Group::General),
    shortcut("Mod+S", "Save", Group::General),
    shortcut("Escape", "Leave the field or close", Group::General),
    shortcut("/", "Search this page", Group::FirList),
    shortcut("J", "Next row", Group::FirList),
    shortcut("K", "Previous row", Group::FirList),
    shortcut("Enter", "Open FIR", Group::FirList),
    shortcut("X", "Select row", Group::FirList),
    shortcut("Mod+A", "Select all visible", Group::FirList),
    shortcut("E", "Edit details", Group::FirList),
    shortcut("Delete", "Delete FIRs", Group::FirList),
    shortcut("Shift+F10", "Row actions", Group::FirList),
    shortcut("Alt+Down", "Next document", Group::Workspace),
    shortcut("Alt+Up", "Previous document", Group::Workspace),
    shortcut("Alt+Shift+Up", "Move document up", Group::Workspace),
    shortcut("Alt+Shift+Down", "Move document down", Group::Workspace),
    shortcut("A", "Add document", Group::Workspace),
    shortcut("E", "Edit details", Group::Workspace),
    shortcut("Mod+.", "Fields panel", Group::Workspace),
    shortcut("Mod+Shift+L", "Show field names", Group::Workspace),
    shortcut("Mod+P", "Print", Group::Workspace),
    shortcut("Mod+Z", "Undo", Group::Editor),
    shortcut("Mod+Y", "Redo", Group::Editor),
    shortcut("Mod+B", "Bold", Group::Editor),
    shortcut("Mod+I", "Italic", Group::Editor),
    shortcut("Mod+U", "Underline", Group::Editor),
    shortcut("Mod+L", "Align left", Group::Editor),
    shortcut("Mod+E", "Center", Group::Editor),
    shortcut("Mod+R", "Align right", Group::Editor),
    shortcut("Mod+J", "Justify", Group::Editor),
    shortcut("Mod+Shift+F", "Insert field", Group::Editor),
    shortcut("Shift+N", "New template", Group::Templates),
    shortcut("F2", "Rename template", Group::Templates),
    shortcut("Alt+Down", "Next template", Group::Templates),
    shortcut("Alt+Up", "Previous template", Group::Templates),
    shortcut("Mod+P", "Preview", Group::Templates),
    shortcut("Mod+P", "Print", Group::Print),
    shortcut("Mod+Shift+S", "Save as PDF", Group::Print),
    shortcut("PageDown", "Next page", Group::Print),
    shortcut("PageUp", "Previous page", Group::Print),
    shortcut("Mod+=", "Zoom in", Group::Print),
    shortcut("Mod+-", "Zoom out", Group::Print),
    shortcut("Mod+0", "Fit page", Group::Print),
    shortcut("Escape", "Back", Group::Print),
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
        binding(cx, "f10", ToggleNavMenu, Some(SHELL)),
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
        binding(cx, "secondary-o", ImportWord, Some(SHELL)),
        binding(cx, "j", NextRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "down", NextRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "k", PreviousRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "up", PreviousRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "x", ToggleRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "space", ToggleRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "enter", OpenRow, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "delete", DeleteSelection, Some(FIR_LIST_NOT_TYPING)),
        binding(cx, "e", EditFir, Some(NOT_TYPING)),
        binding(cx, "secondary-p", Print, Some(SHELL)),
        binding(cx, "secondary-shift-s", SavePdf, Some(SHELL)),
        binding(cx, "alt-down", NextDocument, Some(WORKSPACE)),
        binding(cx, "alt-up", PreviousDocument, Some(WORKSPACE)),
        binding(cx, "alt-shift-up", MoveDocumentUp, Some(WORKSPACE)),
        binding(cx, "alt-shift-down", MoveDocumentDown, Some(WORKSPACE)),
        binding(cx, "a", AddTemplates, Some(WORKSPACE_NOT_TYPING)),
        binding(cx, "secondary-shift-l", ToggleFieldNames, Some(WORKSPACE)),
        binding(cx, "secondary-.", ToggleFieldsPanel, Some(WORKSPACE)),
        binding(cx, "shift-n", NewTemplate, Some(NOT_TYPING)),
        binding(cx, "f2", RenameTemplate, Some(TEMPLATES)),
        binding(cx, "alt-down", NextTemplate, Some(TEMPLATES)),
        binding(cx, "alt-up", PreviousTemplate, Some(TEMPLATES)),
        binding(cx, "pagedown", PreviewNextPage, Some(PRINT_PREVIEW)),
        binding(cx, "pageup", PreviewPreviousPage, Some(PRINT_PREVIEW)),
        binding(cx, "secondary-=", PreviewZoomIn, Some(PRINT_PREVIEW)),
        binding(cx, "secondary-+", PreviewZoomIn, Some(PRINT_PREVIEW)),
        binding(cx, "secondary-shift-=", PreviewZoomIn, Some(PRINT_PREVIEW)),
        binding(cx, "secondary--", PreviewZoomOut, Some(PRINT_PREVIEW)),
        binding(cx, "secondary-0", PreviewZoomFit, Some(PRINT_PREVIEW)),
        binding(cx, "escape", PreviewClose, Some(PRINT_PREVIEW)),
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
