import type { Hotkey } from "@tanstack/react-hotkeys";

export type Shortcut = {
  keys: Hotkey | readonly Hotkey[];
  label: string;
  group: ShortcutGroup;
};

export type ShortcutGroup = "General" | "Navigation" | "FIRs" | "Documents" | "Templates";

export const SHORTCUT_GROUPS: readonly ShortcutGroup[] = [
  "General",
  "Navigation",
  "FIRs",
  "Documents",
  "Templates",
];

export const shortcuts = {
  commandMenu: { keys: "Mod+K", label: "Open command menu", group: "General" },
  cheatsheet: { keys: "?", label: "Show keyboard shortcuts", group: "General" },
  toggleSidebar: { keys: "Mod+B", label: "Toggle sidebar", group: "General" },
  search: { keys: "/", label: "Search this page", group: "General" },
  save: { keys: "Mod+S", label: "Save", group: "General" },
  blurEditor: { keys: "Escape", label: "Leave the editor or field", group: "General" },
  confirm: { keys: "Mod+Enter", label: "Confirm a selection", group: "General" },

  goHome: { keys: ["G", "H"], label: "Go to FIRs", group: "Navigation" },
  goTemplates: { keys: ["G", "T"], label: "Go to templates", group: "Navigation" },
  goPlaceholders: { keys: ["G", "P"], label: "Go to placeholders", group: "Navigation" },
  goSettings: { keys: ["G", "S"], label: "Go to settings", group: "Navigation" },
  goBack: { keys: "Alt+ArrowLeft", label: "Back", group: "Navigation" },
  goForward: { keys: "Alt+ArrowRight", label: "Forward", group: "Navigation" },

  newFir: { keys: "N", label: "New FIR", group: "FIRs" },
  nextRow: { keys: "J", label: "Next row", group: "FIRs" },
  previousRow: { keys: "K", label: "Previous row", group: "FIRs" },
  selectRow: { keys: "X", label: "Select row", group: "FIRs" },
  openRow: { keys: "Enter", label: "Open FIR", group: "FIRs" },
  editFir: { keys: "E", label: "Edit FIR details", group: "FIRs" },

  print: { keys: "Mod+P", label: "Print preview", group: "Documents" },
  nextDocument: { keys: "Alt+ArrowDown", label: "Next document", group: "Documents" },
  previousDocument: { keys: "Alt+ArrowUp", label: "Previous document", group: "Documents" },
  addTemplates: { keys: "A", label: "Add templates to FIR", group: "Documents" },
  toggleFieldNames: { keys: "Mod+Shift+L", label: "Show field names", group: "Documents" },

  newTemplate: { keys: "Shift+N", label: "New template", group: "Templates" },
  importDocx: { keys: "Mod+O", label: "Import Word document", group: "Templates" },
  nextTemplate: { keys: "Alt+ArrowDown", label: "Next template", group: "Templates" },
  previousTemplate: { keys: "Alt+ArrowUp", label: "Previous template", group: "Templates" },
} as const satisfies Record<string, Shortcut>;

export type ShortcutId = keyof typeof shortcuts;
