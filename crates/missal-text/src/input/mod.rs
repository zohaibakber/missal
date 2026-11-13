mod element;
mod history;

use std::{ops::Range, sync::Arc, time::Duration};

use gpui_kit::component::{ActiveTheme, Colorize, ThemeStyled};
use gpui_kit::{
    App, Bounds, ClipboardItem, Context, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Point, Render, SharedString, Styled, Subscription, Task, UTF16Selection, Window,
    actions, div, prelude::FluentBuilder, px,
};

pub use element::TextInputElement;
use history::{EditKind, History, Snapshot};

use crate::{
    fonts::FontFamily,
    layout::{
        Affinity, Align, AttributedText, Direction, HorizontalDirection, LineHeight,
        ParagraphStyle, TextLayout, TextPosition, TextStyle, next_cluster_boundary, next_word_end,
        paragraph_range_at, previous_char_boundary, previous_word_start, word_range_at,
    },
    system::{TextSystem, to_rgba},
};

actions!(
    missal_text_input,
    [
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        LineStart,
        LineEnd,
        SelectLineStart,
        SelectLineEnd,
        DocumentStart,
        DocumentEnd,
        SelectDocumentStart,
        SelectDocumentEnd,
        Backspace,
        Delete,
        DeleteWordBackward,
        DeleteWordForward,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Enter,
        InsertNewline,
    ]
);

pub const KEY_CONTEXT: &str = "MissalTextInput";

pub(crate) fn bind_keys(cx: &mut App) {
    let c = Some(KEY_CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("left", MoveLeft, c),
        KeyBinding::new("right", MoveRight, c),
        KeyBinding::new("up", MoveUp, c),
        KeyBinding::new("down", MoveDown, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("home", LineStart, c),
        KeyBinding::new("end", LineEnd, c),
        KeyBinding::new("shift-home", SelectLineStart, c),
        KeyBinding::new("shift-end", SelectLineEnd, c),
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("enter", Enter, c),
        KeyBinding::new("shift-enter", InsertNewline, c),
    ];
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("alt-left", WordLeft, c),
            KeyBinding::new("alt-right", WordRight, c),
            KeyBinding::new("alt-shift-left", SelectWordLeft, c),
            KeyBinding::new("alt-shift-right", SelectWordRight, c),
            KeyBinding::new("cmd-left", LineStart, c),
            KeyBinding::new("cmd-right", LineEnd, c),
            KeyBinding::new("cmd-shift-left", SelectLineStart, c),
            KeyBinding::new("cmd-shift-right", SelectLineEnd, c),
            KeyBinding::new("cmd-up", DocumentStart, c),
            KeyBinding::new("cmd-down", DocumentEnd, c),
            KeyBinding::new("cmd-shift-up", SelectDocumentStart, c),
            KeyBinding::new("cmd-shift-down", SelectDocumentEnd, c),
            KeyBinding::new("alt-backspace", DeleteWordBackward, c),
            KeyBinding::new("alt-delete", DeleteWordForward, c),
            KeyBinding::new("cmd-a", SelectAll, c),
            KeyBinding::new("cmd-c", Copy, c),
            KeyBinding::new("cmd-x", Cut, c),
            KeyBinding::new("cmd-v", Paste, c),
            KeyBinding::new("cmd-z", Undo, c),
            KeyBinding::new("cmd-shift-z", Redo, c),
        ]);
    } else {
        bindings.extend([
            KeyBinding::new("ctrl-left", WordLeft, c),
            KeyBinding::new("ctrl-right", WordRight, c),
            KeyBinding::new("ctrl-shift-left", SelectWordLeft, c),
            KeyBinding::new("ctrl-shift-right", SelectWordRight, c),
            KeyBinding::new("ctrl-home", DocumentStart, c),
            KeyBinding::new("ctrl-end", DocumentEnd, c),
            KeyBinding::new("ctrl-shift-home", SelectDocumentStart, c),
            KeyBinding::new("ctrl-shift-end", SelectDocumentEnd, c),
            KeyBinding::new("ctrl-backspace", DeleteWordBackward, c),
            KeyBinding::new("ctrl-delete", DeleteWordForward, c),
            KeyBinding::new("ctrl-a", SelectAll, c),
            KeyBinding::new("ctrl-c", Copy, c),
            KeyBinding::new("ctrl-insert", Copy, c),
            KeyBinding::new("ctrl-x", Cut, c),
            KeyBinding::new("shift-delete", Cut, c),
            KeyBinding::new("ctrl-v", Paste, c),
            KeyBinding::new("shift-insert", Paste, c),
            KeyBinding::new("ctrl-z", Undo, c),
            KeyBinding::new("ctrl-y", Redo, c),
            KeyBinding::new("ctrl-shift-z", Redo, c),
        ]);
    }
    cx.bind_keys(bindings);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextInputEvent {
    Change,
    Submit,
    Focus,
    Blur,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Granularity {
    Character,
    Word,
    Line,
}

#[derive(Clone, Debug)]
struct DragState {
    granularity: Granularity,
    origin: Range<usize>,
}

#[derive(Clone, Default)]
pub(crate) struct Frame {
    pub(crate) bounds: Option<Bounds<Pixels>>,
    pub(crate) wrap_width: Option<f32>,
    pub(crate) scroll: Point<f32>,
    pub(crate) layout: Option<Arc<TextLayout>>,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    text: String,
    anchor: TextPosition,
    head: TextPosition,
    marked: Option<Range<usize>>,
    multiline: bool,
    min_rows: usize,
    max_rows: usize,
    placeholder: SharedString,
    disabled: bool,
    read_only: bool,
    family: FontFamily,
    font_size: Option<f32>,
    direction: Direction,
    align: Align,
    history: History,
    sticky_x: Option<f32>,
    caret_visible: bool,
    blink_epoch: usize,
    blink_task: Option<Task<()>>,
    drag: Option<DragState>,
    pub(crate) frame: Frame,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus(&focus_handle, window, |this, _, cx| {
                this.restart_blink(cx);
                cx.emit(TextInputEvent::Focus);
                cx.notify();
            }),
            cx.on_blur(&focus_handle, window, |this, _, cx| {
                this.blink_epoch += 1;
                this.blink_task = None;
                this.marked = None;
                this.drag = None;
                cx.emit(TextInputEvent::Blur);
                cx.notify();
            }),
        ];
        Self {
            focus_handle,
            text: String::new(),
            anchor: TextPosition::default(),
            head: TextPosition::default(),
            marked: None,
            multiline: false,
            min_rows: 1,
            max_rows: 1,
            placeholder: SharedString::default(),
            disabled: false,
            read_only: false,
            family: FontFamily::Interface,
            font_size: None,
            direction: Direction::Auto,
            align: Align::Start,
            history: History::default(),
            sticky_x: None,
            caret_visible: true,
            blink_epoch: 0,
            blink_task: None,
            drag: None,
            frame: Frame::default(),
            _subscriptions: subscriptions,
        }
    }

    pub fn multiline(mut self, min_rows: usize, max_rows: usize) -> Self {
        self.multiline = true;
        self.min_rows = min_rows.max(1);
        self.max_rows = max_rows.max(self.min_rows);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn default_text(mut self, text: impl Into<String>) -> Self {
        self.text = self.sanitize(&text.into());
        let end = TextPosition::upstream(self.text.len());
        self.anchor = end;
        self.head = end;
        self
    }

    pub fn family(mut self, family: FontFamily) -> Self {
        self.family = family;
        self
    }

    pub fn document_font(self) -> Self {
        self.family(FontFamily::Document)
    }

    pub fn text_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    pub fn selected_range(&self) -> Range<usize> {
        let (a, b) = (self.anchor.index, self.head.index);
        a.min(b)..a.max(b)
    }

    pub fn selection_reversed(&self) -> bool {
        self.head.index < self.anchor.index
    }

    pub fn caret(&self) -> TextPosition {
        self.head
    }

    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn frame_bounds(&self) -> Option<Bounds<Pixels>> {
        self.frame.bounds
    }

    pub fn frame_layout(&self) -> Option<Arc<TextLayout>> {
        self.frame.layout.clone()
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.text = self.sanitize(&text.into());
        let end = TextPosition::upstream(self.text.len());
        self.anchor = end;
        self.head = end;
        self.marked = None;
        self.history.clear();
        cx.emit(TextInputEvent::Change);
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub fn select(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let len = self.text.len();
        let clamp = |i: usize| {
            let mut i = i.min(len);
            while !self.text.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        self.anchor = TextPosition::downstream(clamp(range.start));
        self.head = TextPosition::upstream(clamp(range.end));
        self.sticky_x = None;
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle, cx);
    }

    fn editable(&self) -> bool {
        !self.disabled && !self.read_only
    }

    fn sanitize(&self, text: &str) -> String {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.multiline {
            text
        } else {
            text.replace('\n', " ")
        }
    }

    pub(crate) fn font_size(&self, cx: &App) -> f32 {
        self.font_size
            .unwrap_or_else(|| f32::from(cx.theme().font_size) * 0.875)
    }

    pub(crate) fn row_height(&self, cx: &mut App) -> f32 {
        let size = self.font_size(cx);
        let probe = AttributedText::new("Aب", TextStyle::new(self.family.clone(), size));
        TextSystem::layout(cx, &probe, &ParagraphStyle::default(), None).height()
    }

    pub(crate) fn text_color(&self, cx: &App) -> gpui_kit::Hsla {
        if self.disabled {
            cx.theme().muted_foreground
        } else {
            cx.theme().foreground
        }
    }

    fn paragraph(&self, row_height: f32) -> ParagraphStyle {
        ParagraphStyle {
            direction: self.direction,
            align: self.align,
            line_height: LineHeight::Px(row_height),
            wrap: self.multiline,
        }
    }

    pub(crate) fn build_layout(
        &self,
        text: &str,
        color: gpui_kit::Hsla,
        width: Option<f32>,
        cx: &mut App,
    ) -> Arc<TextLayout> {
        let row_height = self.row_height(cx);
        let style = TextStyle {
            color: to_rgba(color),
            ..TextStyle::new(self.family.clone(), self.font_size(cx))
        };
        let mut paragraph = self.paragraph(row_height);
        if text.is_empty() && self.direction == Direction::Auto && !self.placeholder.is_empty() {
            paragraph.direction =
                if crate::layout::first_strong_is_rtl(&self.placeholder).unwrap_or(true) {
                    Direction::Rtl
                } else {
                    Direction::Ltr
                };
        }
        TextSystem::layout(cx, &AttributedText::new(text, style), &paragraph, width)
    }

    pub(crate) fn layout(&self, cx: &mut App) -> Arc<TextLayout> {
        let color = self.text_color(cx);
        self.build_layout(&self.text.clone(), color, self.frame.wrap_width, cx)
    }

    fn restart_blink(&mut self, cx: &mut Context<Self>) {
        self.caret_visible = true;
        self.blink_epoch += 1;
        let epoch = self.blink_epoch;
        self.blink_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(530))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    if this.blink_epoch != epoch {
                        return false;
                    }
                    this.caret_visible = !this.caret_visible;
                    cx.notify();
                    true
                });
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
        }));
    }

    pub(crate) fn caret_visible(&self, window: &Window) -> bool {
        self.caret_visible
            && self.focus_handle.is_focused(window)
            && !self.disabled
            && self.anchor.index == self.head.index
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            anchor: self.anchor,
            head: self.head,
        }
    }

    fn restore(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        self.text = snapshot.text;
        self.anchor = snapshot.anchor;
        self.head = snapshot.head;
        self.marked = None;
        self.sticky_x = None;
        cx.emit(TextInputEvent::Change);
        self.restart_blink(cx);
        cx.notify();
    }

    fn replace(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        kind: EditKind,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() {
            return;
        }
        let new_text = self.sanitize(new_text);
        let range = self.clamp_range(range);
        if range.is_empty() && new_text.is_empty() {
            return;
        }
        let snapshot = self.snapshot();
        self.history.record(snapshot, kind);
        self.text.replace_range(range.clone(), &new_text);
        let caret = TextPosition::new(
            range.start + new_text.len(),
            if new_text.is_empty() {
                Affinity::Downstream
            } else {
                Affinity::Upstream
            },
        );
        self.anchor = caret;
        self.head = caret;
        self.marked = None;
        self.sticky_x = None;
        cx.emit(TextInputEvent::Change);
        self.restart_blink(cx);
        cx.notify();
    }

    fn clamp_range(&self, range: Range<usize>) -> Range<usize> {
        let len = self.text.len();
        let fix = |i: usize| {
            let mut i = i.min(len);
            while !self.text.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        let (a, b) = (fix(range.start), fix(range.end));
        a.min(b)..a.max(b)
    }

    fn move_to(&mut self, position: TextPosition, select: bool, cx: &mut Context<Self>) {
        self.head = position;
        if !select {
            self.anchor = position;
        }
        self.marked = None;
        self.history.break_coalescing();
        self.restart_blink(cx);
        cx.notify();
    }

    fn horizontal(
        &mut self,
        direction: HorizontalDirection,
        select: bool,
        word: bool,
        cx: &mut Context<Self>,
    ) {
        let layout = self.layout(cx);
        self.sticky_x = None;
        if !select && !word && self.anchor.index != self.head.index {
            let a = layout.caret(self.anchor);
            let h = layout.caret(self.head);
            let pick_head = if a.line == h.line {
                match direction {
                    HorizontalDirection::Left => h.x < a.x,
                    HorizontalDirection::Right => h.x > a.x,
                }
            } else {
                let forward = (direction == HorizontalDirection::Left) == layout.is_rtl();
                (self.head.index > self.anchor.index) == forward
            };
            let target = if pick_head { self.head } else { self.anchor };
            self.move_to(target, false, cx);
            return;
        }
        let target = match (word, direction) {
            (false, d) => layout.move_visual(self.head, d),
            (true, HorizontalDirection::Left) => layout.word_left(self.head),
            (true, HorizontalDirection::Right) => layout.word_right(self.head),
        };
        self.move_to(target, select, cx);
    }

    fn vertical(&mut self, lines: isize, select: bool, cx: &mut Context<Self>) {
        let layout = self.layout(cx);
        if !self.multiline {
            let target = if lines < 0 {
                TextPosition::downstream(0)
            } else {
                layout.end_position()
            };
            self.move_to(target, select, cx);
            return;
        }
        if !select && self.anchor.index != self.head.index {
            let start = self.selected_range().start;
            let end = self.selected_range().end;
            let index = if lines < 0 { start } else { end };
            self.sticky_x = None;
            self.move_to(TextPosition::downstream(index), false, cx);
            return;
        }
        let (target, x) = layout.move_vertical(self.head, lines, self.sticky_x);
        self.move_to(target, select, cx);
        self.sticky_x = Some(x);
    }

    fn line_edge(&mut self, end: bool, select: bool, cx: &mut Context<Self>) {
        let layout = self.layout(cx);
        let target = if end {
            layout.line_end(self.head)
        } else {
            layout.line_start(self.head)
        };
        self.sticky_x = None;
        self.move_to(target, select, cx);
    }

    fn document_edge(&mut self, end: bool, select: bool, cx: &mut Context<Self>) {
        let target = if end {
            self.layout(cx).end_position()
        } else {
            TextPosition::downstream(0)
        };
        self.sticky_x = None;
        self.move_to(target, select, cx);
    }

    fn delete_backward(&mut self, word: bool, cx: &mut Context<Self>) {
        let range = self.selected_range();
        if !range.is_empty() {
            self.replace(range, "", EditKind::Other, cx);
            return;
        }
        let head = self.head.index;
        let start = if word {
            previous_word_start(&self.text, head)
        } else {
            previous_char_boundary(&self.text, head)
        };
        self.replace(start..head, "", EditKind::Delete, cx);
    }

    fn delete_forward(&mut self, word: bool, cx: &mut Context<Self>) {
        let range = self.selected_range();
        if !range.is_empty() {
            self.replace(range, "", EditKind::Other, cx);
            return;
        }
        let head = self.head.index;
        let end = if word {
            next_word_end(&self.text, head)
        } else {
            let layout = self.layout(cx);
            next_cluster_boundary(&layout, head)
        };
        self.replace(head..end, "", EditKind::Delete, cx);
    }

    fn on_action_move_left(&mut self, _: &MoveLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(HorizontalDirection::Left, false, false, cx);
    }
    fn on_action_move_right(&mut self, _: &MoveRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(HorizontalDirection::Right, false, false, cx);
    }
    fn on_action_select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(HorizontalDirection::Left, true, false, cx);
    }
    fn on_action_select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(HorizontalDirection::Right, true, false, cx);
    }
    fn on_action_word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(HorizontalDirection::Left, false, true, cx);
    }
    fn on_action_word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(HorizontalDirection::Right, false, true, cx);
    }
    fn on_action_select_word_left(
        &mut self,
        _: &SelectWordLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.horizontal(HorizontalDirection::Left, true, true, cx);
    }
    fn on_action_select_word_right(
        &mut self,
        _: &SelectWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.horizontal(HorizontalDirection::Right, true, true, cx);
    }
    fn on_action_move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, false, cx);
    }
    fn on_action_move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, false, cx);
    }
    fn on_action_select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1, true, cx);
    }
    fn on_action_select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1, true, cx);
    }
    fn on_action_line_start(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(false, false, cx);
    }
    fn on_action_line_end(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(true, false, cx);
    }
    fn on_action_select_line_start(
        &mut self,
        _: &SelectLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.line_edge(false, true, cx);
    }
    fn on_action_select_line_end(
        &mut self,
        _: &SelectLineEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.line_edge(true, true, cx);
    }
    fn on_action_document_start(
        &mut self,
        _: &DocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.document_edge(false, false, cx);
    }
    fn on_action_document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.document_edge(true, false, cx);
    }
    fn on_action_select_document_start(
        &mut self,
        _: &SelectDocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.document_edge(false, true, cx);
    }
    fn on_action_select_document_end(
        &mut self,
        _: &SelectDocumentEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.document_edge(true, true, cx);
    }
    fn on_action_backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_backward(false, cx);
    }
    fn on_action_delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_forward(false, cx);
    }
    fn on_action_delete_word_backward(
        &mut self,
        _: &DeleteWordBackward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_backward(true, cx);
    }
    fn on_action_delete_word_forward(
        &mut self,
        _: &DeleteWordForward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_forward(true, cx);
    }
    fn on_action_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.anchor = TextPosition::downstream(0);
        self.head = TextPosition::upstream(self.text.len());
        self.history.break_coalescing();
        cx.notify();
    }
    fn on_action_copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.selected_range();
        if !range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.text[range].to_string()));
        }
    }
    fn on_action_cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.selected_range();
        if range.is_empty() || !self.editable() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.text[range.clone()].to_string(),
        ));
        self.replace(range, "", EditKind::Other, cx);
    }
    fn on_action_paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let range = self.selected_range();
            self.replace(range, &text, EditKind::Other, cx);
        }
    }
    fn on_action_undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        let current = self.snapshot();
        if let Some(previous) = self.history.undo(current) {
            self.restore(previous, cx);
        }
    }
    fn on_action_redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        let current = self.snapshot();
        if let Some(next) = self.history.redo(current) {
            self.restore(next, cx);
        }
    }
    fn on_action_enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            let range = self.selected_range();
            self.replace(range, "\n", EditKind::Other, cx);
        } else {
            cx.emit(TextInputEvent::Submit);
        }
    }
    fn on_action_insert_newline(
        &mut self,
        _: &InsertNewline,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.multiline {
            let range = self.selected_range();
            self.replace(range, "\n", EditKind::Other, cx);
        } else {
            cx.emit(TextInputEvent::Submit);
        }
    }

    fn content_point(&self, position: Point<Pixels>) -> Option<(f32, f32)> {
        let bounds = self.frame.bounds?;
        Some((
            f32::from(position.x - bounds.origin.x) - self.frame.scroll.x,
            f32::from(position.y - bounds.origin.y) - self.frame.scroll.y,
        ))
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        window.focus(&self.focus_handle, cx);
        let Some((x, y)) = self.content_point(event.position) else {
            return;
        };
        let layout = self.layout(cx);
        let hit = layout.hit_test(x, y);
        self.sticky_x = None;
        self.history.break_coalescing();
        self.marked = None;
        match event.click_count {
            0 | 1 => {
                if event.modifiers.shift {
                    self.head = hit;
                } else {
                    self.anchor = hit;
                    self.head = hit;
                }
                self.drag = Some(DragState {
                    granularity: Granularity::Character,
                    origin: self.anchor.index..self.anchor.index,
                });
            }
            2 => {
                let range = word_range_at(&self.text, hit.index);
                self.set_range(range.clone());
                self.drag = Some(DragState {
                    granularity: Granularity::Word,
                    origin: range,
                });
            }
            _ => {
                let range = if self.multiline {
                    paragraph_range_at(&self.text, hit.index)
                } else {
                    0..self.text.len()
                };
                self.set_range(range.clone());
                self.drag = Some(DragState {
                    granularity: Granularity::Line,
                    origin: range,
                });
            }
        }
        self.restart_blink(cx);
        cx.notify();
    }

    fn set_range(&mut self, range: Range<usize>) {
        self.anchor = TextPosition::downstream(range.start);
        self.head = TextPosition::upstream(range.end);
    }

    pub(crate) fn on_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.clone() else {
            return;
        };
        let Some((x, y)) = self.content_point(position) else {
            return;
        };
        let layout = self.layout(cx);
        let hit = layout.hit_test(x, y);
        match drag.granularity {
            Granularity::Character => {
                self.head = hit;
            }
            Granularity::Word | Granularity::Line => {
                let unit = if drag.granularity == Granularity::Word {
                    word_range_at(&self.text, hit.index)
                } else if self.multiline {
                    paragraph_range_at(&self.text, hit.index)
                } else {
                    0..self.text.len()
                };
                if unit.start < drag.origin.start {
                    self.anchor = TextPosition::upstream(drag.origin.end);
                    self.head = TextPosition::downstream(unit.start);
                } else {
                    self.anchor = TextPosition::downstream(drag.origin.start);
                    self.head = TextPosition::upstream(unit.end.max(drag.origin.end));
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn end_drag(&mut self) {
        self.drag = None;
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub(crate) fn render_state(&self) -> (TextPosition, TextPosition, Option<Range<usize>>) {
        (self.anchor, self.head, self.marked.clone())
    }

    pub(crate) fn placeholder_text(&self) -> &SharedString {
        &self.placeholder
    }

    pub(crate) fn rows(&self) -> (usize, usize) {
        if self.multiline {
            (self.min_rows, self.max_rows)
        } else {
            (1, 1)
        }
    }

    fn utf16_to_byte(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        for (i, c) in self.text.char_indices() {
            if utf16 >= offset {
                return i;
            }
            utf16 += c.len_utf16();
        }
        self.text.len()
    }

    fn byte_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.utf16_to_byte(range.start)..self.utf16_to_byte(range.end)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.byte_to_utf16(range.start)..self.byte_to_utf16(range.end)
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        adjusted_range.replace(self.range_to_utf16(&range));
        Some(self.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range()),
            reversed: self.selection_reversed(),
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or_else(|| self.selected_range());
        let kind = if self.marked.is_some() {
            EditKind::Compose
        } else {
            EditKind::Typing
        };
        self.replace(range, new_text, kind, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or_else(|| self.selected_range());
        let range = self.clamp_range(range);
        let new_text = self.sanitize(new_text);
        let snapshot = self.snapshot();
        self.history.record(snapshot, EditKind::Compose);
        self.text.replace_range(range.clone(), &new_text);
        self.marked = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let selection = new_selected_range_utf16
            .map(|r| {
                let base = self.byte_to_utf16(range.start);
                self.range_from_utf16(&(base + r.start..base + r.end))
            })
            .unwrap_or(range.start + new_text.len()..range.start + new_text.len());
        self.anchor = TextPosition::downstream(selection.start);
        self.head = TextPosition::upstream(selection.end);
        self.sticky_x = None;
        cx.emit(TextInputEvent::Change);
        self.restart_blink(cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let layout = self.layout(cx);
        let origin = self
            .frame
            .bounds
            .map(|b| b.origin)
            .unwrap_or(element_bounds.origin)
            + gpui_kit::point(px(self.frame.scroll.x), px(self.frame.scroll.y));
        let rect = layout
            .selection_rects(range.clone())
            .into_iter()
            .next()
            .unwrap_or_else(|| layout.caret_rect(TextPosition::downstream(range.start), 1.0));
        Some(Bounds::new(
            gpui_kit::point(origin.x + px(rect.x), origin.y + px(rect.y)),
            gpui_kit::size(px(rect.width.max(1.0)), px(rect.height)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let (x, y) = self.content_point(point)?;
        let layout = self.layout(cx);
        let hit = layout.hit_test(x, y);
        Some(self.byte_to_utf16(hit.index))
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        self.editable()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let focused = !self.disabled && self.focus_handle.is_focused(window);
        let background = if self.disabled {
            theme.input.mix_oklab(theme.transparent, 0.8).opacity(0.5)
        } else {
            theme.input_background()
        };
        let border = theme.input;
        let ring = theme.ring;
        let radius = theme.radius;
        let entity = cx.entity();
        div()
            .id(("missal-text-input", cx.entity_id()))
            .key_context(KEY_CONTEXT)
            .when(!self.disabled, |this| this.track_focus(&self.focus_handle))
            .cursor_text()
            .w_full()
            .px(px(10.))
            .py(px(6.))
            .bg(background)
            .border_1()
            .border_color(border)
            .rounded(radius)
            .when(focused, |this| {
                this.border_color(ring).focus_ring_style(window, cx)
            })
            .on_action(cx.listener(Self::on_action_move_left))
            .on_action(cx.listener(Self::on_action_move_right))
            .on_action(cx.listener(Self::on_action_move_up))
            .on_action(cx.listener(Self::on_action_move_down))
            .on_action(cx.listener(Self::on_action_select_left))
            .on_action(cx.listener(Self::on_action_select_right))
            .on_action(cx.listener(Self::on_action_select_up))
            .on_action(cx.listener(Self::on_action_select_down))
            .on_action(cx.listener(Self::on_action_word_left))
            .on_action(cx.listener(Self::on_action_word_right))
            .on_action(cx.listener(Self::on_action_select_word_left))
            .on_action(cx.listener(Self::on_action_select_word_right))
            .on_action(cx.listener(Self::on_action_line_start))
            .on_action(cx.listener(Self::on_action_line_end))
            .on_action(cx.listener(Self::on_action_select_line_start))
            .on_action(cx.listener(Self::on_action_select_line_end))
            .on_action(cx.listener(Self::on_action_document_start))
            .on_action(cx.listener(Self::on_action_document_end))
            .on_action(cx.listener(Self::on_action_select_document_start))
            .on_action(cx.listener(Self::on_action_select_document_end))
            .on_action(cx.listener(Self::on_action_backspace))
            .on_action(cx.listener(Self::on_action_delete))
            .on_action(cx.listener(Self::on_action_delete_word_backward))
            .on_action(cx.listener(Self::on_action_delete_word_forward))
            .on_action(cx.listener(Self::on_action_select_all))
            .on_action(cx.listener(Self::on_action_copy))
            .on_action(cx.listener(Self::on_action_cut))
            .on_action(cx.listener(Self::on_action_paste))
            .on_action(cx.listener(Self::on_action_undo))
            .on_action(cx.listener(Self::on_action_redo))
            .on_action(cx.listener(Self::on_action_enter))
            .on_action(cx.listener(Self::on_action_insert_newline))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .child(TextInputElement::new(entity))
    }
}
