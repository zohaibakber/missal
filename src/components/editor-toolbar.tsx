import { useEffect, useState, type ComponentProps } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isRangeSelection,
  $setSelection,
  type BaseSelection,
  CAN_UNDO_COMMAND,
  CAN_REDO_COMMAND,
  COMMAND_PRIORITY_LOW,
  FORMAT_TEXT_COMMAND,
  FORMAT_ELEMENT_COMMAND,
  UNDO_COMMAND,
  REDO_COMMAND,
  SELECTION_CHANGE_COMMAND,
  TEXT_TYPE_TO_FORMAT,
  type TextFormatType,
  type ElementFormatType,
} from "lexical";
import { INSERT_UNORDERED_LIST_COMMAND, INSERT_ORDERED_LIST_COMMAND } from "@lexical/list";
import { mergeRegister } from "@lexical/utils";
import { Button } from "#/components/ui/button";
import { Separator } from "#/components/ui/separator";
import { ToggleGroup, ToggleGroupItem } from "#/components/ui/toggle-group";

import { HugeiconsIcon } from "@hugeicons/react";
import {
  TextBoldIcon,
  TextItalicIcon,
  TextUnderlineIcon,
  TextAlignLeftIcon,
  TextAlignCenterIcon,
  TextAlignRightIcon,
  TextAlignJustifyIcon,
  UndoIcon,
  RedoIcon,
  LeftToRightListBulletIcon,
  LeftToRightListNumberIcon,
  BracesIcon,
  ArrowDown01Icon,
} from "@hugeicons/core-free-icons";
import { useAtomValue } from "@effect/atom-react";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "#/components/ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "#/components/ui/popover";
import { atoms } from "#/state/atoms";
import { Tooltip, TooltipTrigger, TooltipContent } from "#/components/ui/tooltip";
import { Kbd, KbdGroup } from "#/components/ui/kbd";
import { formatForDisplay, type HotkeyChord } from "@tanstack/react-hotkeys";

type ToolbarIcon = ComponentProps<typeof HugeiconsIcon>["icon"];
const formats = [
  { value: "bold", label: "Bold", keys: "Mod+B", icon: TextBoldIcon },
  { value: "italic", label: "Italic", keys: "Mod+I", icon: TextItalicIcon },
  { value: "underline", label: "Underline", keys: "Mod+U", icon: TextUnderlineIcon },
] satisfies { value: TextFormatType; label: string; keys: HotkeyChord; icon: ToolbarIcon }[];

function TooltipLabel({ label, keys }: { label: string; keys?: HotkeyChord }) {
  return (
    <>
      {label}
      {keys ? (
        <KbdGroup>
          {formatForDisplay(keys, { parts: true }).map((part) => (
            <Kbd key={part}>{part}</Kbd>
          ))}
        </KbdGroup>
      ) : null}
    </>
  );
}
const alignments = [
  { value: "left", label: "Align left", icon: TextAlignLeftIcon },
  { value: "center", label: "Align center", icon: TextAlignCenterIcon },
  { value: "right", label: "Align right", icon: TextAlignRightIcon },
  { value: "justify", label: "Justify", icon: TextAlignJustifyIcon },
] satisfies { value: ElementFormatType; label: string; icon: ToolbarIcon }[];

function ToolbarButton({
  label,
  keys,
  icon,
  ...props
}: ComponentProps<typeof Button> & { label: string; keys?: HotkeyChord; icon: ToolbarIcon }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label={label}
            onMouseDown={(event) => event.preventDefault()}
            {...props}
          />
        }
      >
        <HugeiconsIcon icon={icon} strokeWidth={2} />
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <TooltipLabel label={label} keys={keys} />
      </TooltipContent>
    </Tooltip>
  );
}

export type InsertableFieldGroup = {
  readonly heading: string;
  readonly labels: readonly string[];
};

function InsertFieldMenu({ groups }: { groups: readonly InsertableFieldGroup[] }) {
  const [editor] = useLexicalComposerContext();
  const { open: openMarker, close: closeMarker } = useAtomValue(atoms.fieldMarkersAtom);
  const [open, setOpen] = useState(false);
  const [saved, setSaved] = useState<BaseSelection | null>(null);

  function insert(label: string) {
    setOpen(false);
    editor.update(() => {
      if (saved) $setSelection(saved.clone());
      const selection = $getSelection();
      if ($isRangeSelection(selection)) selection.insertText(`${openMarker}${label}${closeMarker}`);
    });
    editor.focus();
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        if (next) setSaved(editor.getEditorState().read(() => $getSelection()?.clone() ?? null));
        setOpen(next);
      }}
    >
      <Tooltip>
        <TooltipTrigger
          render={
            <PopoverTrigger
              render={
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onMouseDown={(event) => event.preventDefault()}
                />
              }
            />
          }
        >
          <HugeiconsIcon icon={BracesIcon} strokeWidth={2} data-icon="inline-start" />
          Insert field
          <HugeiconsIcon icon={ArrowDown01Icon} strokeWidth={2} data-icon="inline-end" />
        </TooltipTrigger>
        <TooltipContent side="bottom">
          Or type {openMarker}name{closeMarker}
        </TooltipContent>
      </Tooltip>
      <PopoverContent align="start" className="w-72 p-0">
        <Command>
          <CommandInput placeholder="Search fields" />
          <CommandList className="max-h-80">
            <CommandEmpty>No fields found</CommandEmpty>
            {groups.map((group) =>
              group.labels.length ? (
                <CommandGroup key={group.heading} heading={group.heading}>
                  {group.labels.map((label) => (
                    <CommandItem key={label} value={label} onSelect={() => insert(label)}>
                      <span lang="ur" dir="rtl" className="min-w-0 flex-1 truncate text-ur">
                        {label}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              ) : null,
            )}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}

export function EditorToolbar({ fieldGroups }: { fieldGroups: readonly InsertableFieldGroup[] }) {
  const [editor] = useLexicalComposerContext();
  const [format, setFormat] = useState(0);
  const [alignment, setAlignment] = useState<string>("right");
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);

  useEffect(() => {
    function updateSelection() {
      const selection = $getSelection();
      if (!$isRangeSelection(selection)) return;
      setFormat(selection.format);
      const node = selection.anchor.getNode();
      const element = node.getTopLevelElement();
      setAlignment(element?.getFormatType() || "right");
    }
    return mergeRegister(
      editor.registerUpdateListener(({ editorState }) => editorState.read(updateSelection)),
      editor.registerCommand(
        SELECTION_CHANGE_COMMAND,
        () => {
          updateSelection();
          return false;
        },
        COMMAND_PRIORITY_LOW,
      ),
      editor.registerCommand(
        CAN_UNDO_COMMAND,
        (value) => {
          setCanUndo(value);
          return false;
        },
        COMMAND_PRIORITY_LOW,
      ),
      editor.registerCommand(
        CAN_REDO_COMMAND,
        (value) => {
          setCanRedo(value);
          return false;
        },
        COMMAND_PRIORITY_LOW,
      ),
    );
  }, [editor]);

  return (
    <div
      aria-label="Document formatting"
      role="toolbar"
      className="flex h-10 shrink-0 items-center gap-0.5 overflow-x-auto border-b px-2"
      dir="ltr"
    >
      <ToolbarButton
        label="Undo"
        keys="Mod+Z"
        icon={UndoIcon}
        disabled={!canUndo}
        onClick={() => editor.dispatchCommand(UNDO_COMMAND, undefined)}
      />
      <ToolbarButton
        label="Redo"
        keys="Mod+Shift+Z"
        icon={RedoIcon}
        disabled={!canRedo}
        onClick={() => editor.dispatchCommand(REDO_COMMAND, undefined)}
      />
      <Separator
        orientation="vertical"
        className="mx-1.5 data-vertical:h-4 data-vertical:self-auto"
      />
      <ToggleGroup
        multiple
        value={formats
          .filter((item) => format & TEXT_TYPE_TO_FORMAT[item.value])
          .map((item) => item.value)}
        aria-label="Text style"
        size="sm"
        spacing={0}
      >
        {formats.map((format) => (
          <Tooltip key={format.value}>
            <TooltipTrigger
              render={
                <ToggleGroupItem
                  value={format.value}
                  aria-label={format.label}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => editor.dispatchCommand(FORMAT_TEXT_COMMAND, format.value)}
                />
              }
            >
              <HugeiconsIcon icon={format.icon} strokeWidth={2} />
            </TooltipTrigger>
            <TooltipContent side="bottom">
              <TooltipLabel label={format.label} keys={format.keys} />
            </TooltipContent>
          </Tooltip>
        ))}
      </ToggleGroup>
      <Separator
        orientation="vertical"
        className="mx-1.5 data-vertical:h-4 data-vertical:self-auto"
      />
      <ToggleGroup value={[alignment]} aria-label="Paragraph alignment" size="sm" spacing={0}>
        {alignments.map((align) => (
          <Tooltip key={align.value}>
            <TooltipTrigger
              render={
                <ToggleGroupItem
                  value={align.value}
                  aria-label={align.label}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, align.value)}
                />
              }
            >
              <HugeiconsIcon icon={align.icon} strokeWidth={2} />
            </TooltipTrigger>
            <TooltipContent side="bottom">{align.label}</TooltipContent>
          </Tooltip>
        ))}
      </ToggleGroup>
      <Separator
        orientation="vertical"
        className="mx-1.5 data-vertical:h-4 data-vertical:self-auto"
      />
      <ToolbarButton
        label="Bulleted list"
        icon={LeftToRightListBulletIcon}
        onClick={() => editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, undefined)}
      />
      <ToolbarButton
        label="Numbered list"
        icon={LeftToRightListNumberIcon}
        onClick={() => editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND, undefined)}
      />
      <Separator
        orientation="vertical"
        className="mx-1.5 data-vertical:h-4 data-vertical:self-auto"
      />
      <InsertFieldMenu groups={fieldGroups} />
    </div>
  );
}
