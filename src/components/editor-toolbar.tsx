import { useEffect, useState, type ComponentProps } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isRangeSelection,
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
} from "@hugeicons/core-free-icons";
import { Tooltip, TooltipTrigger, TooltipContent } from "#/components/ui/tooltip";
import { Kbd, KbdGroup } from "#/components/ui/kbd";
import { formatForDisplay, type Hotkey } from "@tanstack/react-hotkeys";

type ToolbarIcon = ComponentProps<typeof HugeiconsIcon>["icon"];
const formats = [
  { value: "bold", label: "Bold", keys: "Mod+B", icon: TextBoldIcon },
  { value: "italic", label: "Italic", keys: "Mod+I", icon: TextItalicIcon },
  { value: "underline", label: "Underline", keys: "Mod+U", icon: TextUnderlineIcon },
] satisfies { value: TextFormatType; label: string; keys: Hotkey; icon: ToolbarIcon }[];

function TooltipLabel({ label, keys }: { label: string; keys?: Hotkey }) {
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
}: ComponentProps<typeof Button> & { label: string; keys?: Hotkey; icon: ToolbarIcon }) {
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

export function EditorToolbar() {
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
    </div>
  );
}
