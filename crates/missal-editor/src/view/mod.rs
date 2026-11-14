mod element;
pub mod keys;
mod theme;
mod toolbar;

use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{
    App, AppContext, Bounds, ClipboardEntry, ClipboardItem, Context, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, MouseDownEvent,
    ParentElement, Pixels, Point, Render, ScrollEvent, Styled, Subscription, Task, UTF16Selection,
    Window, div, prelude::FluentBuilder, px,
};
use missal_core::document::{Align, ListType, TextFormat};
use missal_core::{
    DisplayMode, Document, FieldCatalog, FieldMarkers, FieldPresentation, FieldReference, FirValues,
};
use missal_page::{
    Affinity, BlockPath, DocSource, HorizontalDirection, Packet, PageEngine, Point as PagePoint,
    Position, Rect, VerticalDirection,
};
use missal_text::TextInput;
use missal_text::input as text_actions;

use crate::model::boundary;
use crate::model::selection::{document_end, document_start, effective_range, end_of, pos};
use crate::model::{Change, EditorCore, Fragment, Selection, clip};

pub use element::PagesElement;
pub use keys::KEY_CONTEXT;
pub use theme::EditorTheme;

pub const PAGE_GAP: f32 = 24.0;
pub const CANVAS_PAD: f32 = 24.0;
pub const CANVAS_TOP: f32 = 20.0;
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_FIT_ZOOM: f32 = 1.0;
pub const MAX_ZOOM: f32 = 3.0;

#[derive(Clone)]
pub struct EditorFields {
    pub catalog: Rc<FieldCatalog>,
    pub values: Option<Rc<FirValues>>,
    pub mode: DisplayMode,
    pub markers: FieldMarkers,
}

impl EditorFields {
    pub fn labels(catalog: Rc<FieldCatalog>, markers: FieldMarkers) -> EditorFields {
        EditorFields {
            catalog,
            values: None,
            mode: DisplayMode::Labels,
            markers,
        }
    }

    pub fn presentation(&self) -> FieldPresentation<'_> {
        match &self.values {
            Some(values) => FieldPresentation::for_fir(&self.catalog, values, self.mode),
            None => FieldPresentation::labels(&self.catalog),
        }
    }
}

impl Default for EditorFields {
    fn default() -> EditorFields {
        EditorFields::labels(Rc::new(FieldCatalog::default()), FieldMarkers::default())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    DirtyChanged(bool),
    SaveRequested,
    ModeRequested(DisplayMode),
}

#[derive(Clone, Copy, Debug, Default)]
struct SessionZoom(Option<f32>);

impl gpui_kit::Global for SessionZoom {}

#[derive(Clone, Copy, Debug, Default)]
pub struct Timings {
    pub load: Duration,
    pub edit: Duration,
    pub relayout: Duration,
    pub blocks_measured: usize,
    pub pages_rebuilt: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Placement {
    pub zoom: f32,
    pub pages: Vec<Rect>,
    pub width: f32,
    pub height: f32,
}

impl Placement {
    pub fn page_at(&self, y: f32) -> Option<usize> {
        if self.pages.is_empty() {
            return None;
        }
        let index = self.pages.partition_point(|r| r.bottom() < y);
        if index >= self.pages.len() {
            return Some(self.pages.len() - 1);
        }
        let page = self.pages[index];
        if y < page.y && index > 0 {
            let previous = self.pages[index - 1];
            if y - previous.bottom() < page.y - y {
                return Some(index - 1);
            }
        }
        Some(index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Granularity {
    Character,
    Word,
    Block,
}

#[derive(Clone, Debug)]
struct Drag {
    granularity: Granularity,
    origin: (Position, Position),
    last: Point<Pixels>,
}

pub struct Editor {
    focus: FocusHandle,
    core: EditorCore,
    fields: EditorFields,
    engine: PageEngine,
    packet: Packet,
    key: String,
    read_only: bool,
    mode_toggle: bool,
    zoom_override: Option<f32>,
    pub(crate) scroll: PagePoint,
    pub(crate) viewport: Option<Bounds<Pixels>>,
    pub(crate) placement: Placement,
    autoscroll: bool,
    goal_x: Option<f32>,
    caret_visible: bool,
    blink_epoch: usize,
    blink_task: Option<Task<()>>,
    drag: Option<Drag>,
    autoscroll_task: Option<Task<()>>,
    dirty: bool,
    timings: Timings,
    field_menu_open: bool,
    field_query: String,
    field_search: Entity<TextInput>,
    menu_selection: Option<Selection>,
    clipboard: Option<(String, Fragment)>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<EditorEvent> for Editor {}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn utf16_to_byte(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        if units >= offset {
            return index;
        }
        units += c.len_utf16();
    }
    text.len()
}

fn byte_to_utf16(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .chars()
        .map(char::len_utf16)
        .sum()
}

impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Editor {
        let focus = cx.focus_handle();
        let theme = EditorTheme::get(cx);
        let field_search = cx.new(|cx| {
            TextInput::new(window, cx)
                .bare()
                .placeholder("تلاش")
                .placeholder_color(theme.ink_faint)
                .direction(missal_text::Direction::Rtl)
                .align(missal_text::Align::Right)
                .text_size(16.0)
        });
        let zoom_override = cx.try_global::<SessionZoom>().and_then(|zoom| zoom.0);
        let subscriptions = vec![
            cx.on_focus(&focus, window, |this, _, cx| {
                this.restart_blink(cx);
                cx.notify();
            }),
            cx.on_blur(&focus, window, |this, window, cx| {
                this.blink_epoch += 1;
                this.blink_task = None;
                this.drag = None;
                if this.core.marked().is_some() {
                    let change = this.core.unmark();
                    this.apply(change, window, cx);
                }
                cx.notify();
            }),
            cx.subscribe_in(
                &field_search,
                window,
                |this, input, event: &missal_text::TextInputEvent, window, cx| match event {
                    missal_text::TextInputEvent::Change => {
                        this.field_query = input.read(cx).text().to_owned();
                        cx.notify();
                    }
                    missal_text::TextInputEvent::Submit => {
                        if let Some((_, reference)) = this
                            .field_choices()
                            .into_iter()
                            .flat_map(|(_, items)| items)
                            .next()
                        {
                            this.choose_field(reference, window, cx);
                        }
                    }
                    _ => {}
                },
            ),
        ];
        let fields = EditorFields::default();
        let core = EditorCore::new(
            Document::empty(),
            fields.catalog.clone(),
            fields.markers.clone(),
        );
        let mut engine = PageEngine::new();
        let packet = {
            let presentation = fields.presentation();
            engine.layout(&[DocSource::new(core.document(), &presentation)])
        };
        Editor {
            focus,
            core,
            fields,
            engine,
            packet,
            key: String::new(),
            read_only: false,
            mode_toggle: false,
            zoom_override,
            scroll: PagePoint::default(),
            viewport: None,
            placement: Placement::default(),
            autoscroll: false,
            goal_x: None,
            caret_visible: true,
            blink_epoch: 0,
            blink_task: None,
            drag: None,
            autoscroll_task: None,
            dirty: false,
            timings: Timings::default(),
            field_menu_open: false,
            field_query: String::new(),
            field_search,
            menu_selection: None,
            clipboard: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn load(
        &mut self,
        key: String,
        document: Document,
        fields: EditorFields,
        dirty: bool,
        cx: &mut Context<Self>,
    ) {
        let started = Instant::now();
        self.key = key;
        self.fields = fields;
        self.core
            .set_fields(self.fields.catalog.clone(), self.fields.markers.clone());
        if dirty {
            self.core.load_dirty(document);
        } else {
            self.core.load(document);
        }
        self.relayout_all();
        self.timings.load = started.elapsed();
        self.scroll = PagePoint::default();
        self.goal_x = None;
        self.drag = None;
        self.field_menu_open = false;
        self.refresh_dirty(cx);
        cx.notify();
    }

    fn relayout_all(&mut self) {
        let presentation = self.fields.presentation();
        self.packet = self
            .engine
            .layout(&[DocSource::new(self.core.document(), &presentation)]);
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn core(&self) -> &EditorCore {
        &self.core
    }

    pub fn packet(&self) -> &Packet {
        &self.packet
    }

    pub fn timings(&self) -> Timings {
        self.timings
    }

    pub fn fields(&self) -> &EditorFields {
        &self.fields
    }

    pub fn document(&self) -> &Document {
        self.core.document()
    }

    pub fn selection(&self) -> &Selection {
        self.core.selection()
    }

    pub fn zoom(&self) -> f32 {
        self.placement.zoom
    }

    pub fn scroll_offset(&self) -> (f32, f32) {
        (self.scroll.x, self.scroll.y)
    }

    pub fn viewport_bounds(&self) -> Option<Bounds<Pixels>> {
        self.viewport
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    pub fn set_mode_toggle(&mut self, shown: bool, cx: &mut Context<Self>) {
        self.mode_toggle = shown;
        cx.notify();
    }

    pub fn has_mode_toggle(&self) -> bool {
        self.mode_toggle
    }

    pub fn request_mode(&mut self, mode: DisplayMode, cx: &mut Context<Self>) {
        if self.fields.mode != mode {
            cx.emit(EditorEvent::ModeRequested(mode));
        }
    }

    pub fn is_fit_zoom(&self) -> bool {
        self.zoom_override.is_none()
    }

    pub fn field_positions(&self, reference: &FieldReference) -> Vec<Position> {
        let document = self.core.document();
        let mut found = Vec::new();
        for path in crate::model::tree::all_text_paths(document) {
            let Some(inlines) = crate::model::tree::inlines(document, &path) else {
                continue;
            };
            let mut offset = 0;
            for inline in inlines.iter() {
                if let missal_core::document::Inline::Field(field) = inline
                    && field.reference == *reference
                {
                    found.push(pos(path.clone(), offset));
                }
                offset += crate::model::inline::inline_len(inline);
            }
        }
        found
    }

    pub fn reveal_reference(
        &mut self,
        reference: &FieldReference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(first) = self.field_positions(reference).into_iter().next() else {
            return false;
        };
        self.select(Selection::caret(first), window, cx);
        if let Some(bounds) = self.viewport
            && let Some(rect) = self.caret_canvas_rect(&self.core.selection().head)
        {
            self.autoscroll = false;
            self.scroll.y = rect.y - f32::from(bounds.size.height) / 3.0;
            self.clamp_scroll();
        }
        window.focus(&self.focus, cx);
        cx.notify();
        true
    }

    pub fn set_fields(&mut self, fields: EditorFields, cx: &mut Context<Self>) {
        self.fields = fields;
        self.core
            .set_fields(self.fields.catalog.clone(), self.fields.markers.clone());
        let started = Instant::now();
        let presentation = self.fields.presentation();
        self.engine.refresh_fields(
            &mut self.packet,
            &[DocSource::new(self.core.document(), &presentation)],
        );
        self.timings.relayout = started.elapsed();
        cx.notify();
    }

    pub fn is_dirty(&self) -> bool {
        self.core.is_dirty()
    }

    pub fn snapshot(&self) -> (Document, u64) {
        self.core.snapshot()
    }

    pub fn mark_saved(&mut self, revision: u64, cx: &mut Context<Self>) {
        self.core.mark_saved(revision);
        self.refresh_dirty(cx);
        cx.notify();
    }

    pub fn discard(&mut self, cx: &mut Context<Self>) {
        self.core.discard();
        self.relayout_all();
        self.refresh_dirty(cx);
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn focus_handle_ref(&self) -> &FocusHandle {
        &self.focus
    }

    fn refresh_dirty(&mut self, cx: &mut Context<Self>) {
        let dirty = self.core.is_dirty();
        if dirty != self.dirty {
            self.dirty = dirty;
            cx.emit(EditorEvent::DirtyChanged(dirty));
        }
    }

    pub fn apply(&mut self, change: Option<Change>, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(change) = change {
            let started = Instant::now();
            let presentation = self.fields.presentation();
            let total = self.core.document().blocks.len();
            let edit = match change.edit {
                missal_page::Edit::Splice { inserted, .. }
                    if inserted > 48 && inserted * 2 > total =>
                {
                    missal_page::Edit::Document
                }
                edit => edit,
            };
            self.engine.update(
                &mut self.packet,
                0,
                &DocSource::new(self.core.document(), &presentation),
                &edit,
            );
            let stats = self.engine.last_stats();
            self.timings.relayout = started.elapsed();
            self.timings.blocks_measured = stats.blocks_measured;
            self.timings.pages_rebuilt = stats.pages_rebuilt;
            self.refresh_dirty(cx);
        }
        self.moved(cx);
    }

    fn moved(&mut self, cx: &mut Context<Self>) {
        self.autoscroll = true;
        self.restart_blink(cx);
        cx.notify();
    }

    pub fn run<R>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut EditorCore) -> R,
    ) -> R
    where
        R: Into<Option<Change>> + Clone,
    {
        let started = Instant::now();
        let result = f(&mut self.core);
        let change: Option<Change> = result.clone().into();
        self.apply(change, window, cx);
        self.timings.edit = started.elapsed();
        result
    }

    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut EditorCore) -> Option<Change>,
    ) {
        if self.read_only {
            return;
        }
        let started = Instant::now();
        let change = f(&mut self.core);
        self.goal_x = None;
        self.apply(change, window, cx);
        self.timings.edit = started.elapsed();
    }

    pub fn set_selection(
        &mut self,
        selection: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.goal_x = None;
        self.select(selection, window, cx);
    }

    fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        let change = self.core.set_selection(selection);
        self.apply(change, window, cx);
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

    pub(crate) fn caret_shown(&self, window: &Window) -> bool {
        self.caret_visible
            && !self.read_only
            && self.focus.is_focused(window)
            && self.core.selection().is_collapsed()
    }

    pub fn fit_zoom(&self, viewport_width: f32) -> f32 {
        let page_width = self.packet.page(0).map(|p| p.size.width).unwrap_or(794.0);
        ((viewport_width - 2.0 * CANVAS_PAD) / page_width).clamp(MIN_ZOOM, MAX_FIT_ZOOM)
    }

    pub(crate) fn layout_frame(&mut self, bounds: Bounds<Pixels>) {
        self.viewport = Some(bounds);
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let zoom = self.zoom_override.unwrap_or_else(|| self.fit_zoom(width));
        let mut pages = Vec::with_capacity(self.packet.page_count());
        let widest = self
            .packet
            .pages()
            .map(|p| p.size.width * zoom)
            .fold(0.0f32, f32::max);
        let content_width = width.max(widest + 2.0 * CANVAS_PAD);
        let mut y = CANVAS_TOP;
        for page in self.packet.pages() {
            let w = page.size.width * zoom;
            let h = page.size.height * zoom;
            pages.push(Rect::new((content_width - w) / 2.0, y, w, h));
            y += h + PAGE_GAP;
        }
        let content_height = (y - PAGE_GAP + CANVAS_PAD).max(height);
        self.placement = Placement {
            zoom,
            pages,
            width: content_width,
            height: content_height,
        };
        if self.autoscroll {
            self.autoscroll = false;
            self.scroll_to_head(width, height);
        }
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let Some(bounds) = self.viewport else { return };
        let max_x = (self.placement.width - f32::from(bounds.size.width)).max(0.0);
        let max_y = (self.placement.height - f32::from(bounds.size.height)).max(0.0);
        self.scroll.x = self.scroll.x.clamp(0.0, max_x);
        self.scroll.y = self.scroll.y.clamp(0.0, max_y);
    }

    pub(crate) fn page_rect_to_canvas(&self, page: usize, rect: Rect) -> Option<Rect> {
        let origin = self.placement.pages.get(page)?;
        let zoom = self.placement.zoom;
        Some(Rect::new(
            origin.x + rect.x * zoom,
            origin.y + rect.y * zoom,
            rect.width * zoom,
            rect.height * zoom,
        ))
    }

    pub fn caret_canvas_rect(&self, position: &Position) -> Option<Rect> {
        let caret = self.packet.caret(position)?;
        self.page_rect_to_canvas(caret.page, caret.rect)
    }

    fn scroll_to_head(&mut self, width: f32, height: f32) {
        let head = self.core.selection().head.clone();
        let Some(rect) = self.caret_canvas_rect(&head) else {
            return;
        };
        let margin = (rect.height * 1.5).min(height / 3.0);
        if rect.y < self.scroll.y + margin {
            self.scroll.y = rect.y - margin;
        } else if rect.bottom() > self.scroll.y + height - margin {
            self.scroll.y = rect.bottom() - height + margin;
        }
        if rect.x < self.scroll.x + 16.0 {
            self.scroll.x = rect.x - 16.0;
        } else if rect.x > self.scroll.x + width - 16.0 {
            self.scroll.x = rect.x - width + 16.0;
        }
    }

    pub(crate) fn screen_to_page(&self, point: Point<Pixels>) -> Option<(usize, PagePoint)> {
        let bounds = self.viewport?;
        let cx = f32::from(point.x - bounds.origin.x) + self.scroll.x;
        let cy = f32::from(point.y - bounds.origin.y) + self.scroll.y;
        let page = self.placement.page_at(cy)?;
        let rect = self.placement.pages[page];
        let zoom = self.placement.zoom;
        Some((
            page,
            PagePoint::new((cx - rect.x) / zoom, (cy - rect.y) / zoom),
        ))
    }

    pub(crate) fn canvas_to_screen(&self, rect: Rect) -> Option<Bounds<Pixels>> {
        let bounds = self.viewport?;
        Some(Bounds::new(
            gpui_kit::point(
                bounds.origin.x + px(rect.x - self.scroll.x),
                bounds.origin.y + px(rect.y - self.scroll.y),
            ),
            gpui_kit::size(px(rect.width.max(1.0)), px(rect.height)),
        ))
    }

    pub fn page_to_screen(&self, page: usize, rect: Rect) -> Option<Bounds<Pixels>> {
        self.canvas_to_screen(self.page_rect_to_canvas(page, rect)?)
    }

    pub fn hit(&self, point: Point<Pixels>) -> Option<Position> {
        let (page, local) = self.screen_to_page(point)?;
        self.packet.hit_test(page, local)
    }

    pub fn position_screen_bounds(&self, position: &Position) -> Option<Bounds<Pixels>> {
        self.canvas_to_screen(self.caret_canvas_rect(position)?)
    }

    pub(crate) fn field_under(&self, point: Point<Pixels>, hit: &Position) -> Option<usize> {
        let (page, local) = self.screen_to_page(point)?;
        let candidates = [
            Some(hit.offset),
            hit.offset.checked_sub(missal_page::OBJECT_LEN),
        ];
        for offset in candidates.into_iter().flatten() {
            if self.core.field_at(&hit.path, offset).is_none() {
                continue;
            }
            let rects = self.packet.selection(
                &pos(hit.path.clone(), offset),
                &pos(hit.path.clone(), offset + missal_page::OBJECT_LEN),
            );
            if rects.iter().any(|(p, r)| *p == page && r.contains(local)) {
                return Some(offset);
            }
        }
        None
    }

    fn scroll_by(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        self.scroll.x += dx;
        self.scroll.y += dy;
        self.clamp_scroll();
        cx.notify();
    }

    pub(crate) fn on_scroll(&mut self, event: &ScrollEvent, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(20.));
        if event.modifiers.secondary(cx) {
            let factor = if f32::from(delta.y) > 0.0 {
                1.1
            } else {
                1.0 / 1.1
            };
            self.set_zoom(Some(self.placement.zoom * factor), cx);
            return;
        }
        self.scroll_by(-f32::from(delta.x), -f32::from(delta.y), cx);
    }

    pub fn set_zoom(&mut self, zoom: Option<f32>, cx: &mut Context<Self>) {
        let anchor = self
            .viewport
            .map(|b| f32::from(b.size.height) / 2.0)
            .unwrap_or(0.0);
        let fraction = if self.placement.height > 0.0 {
            (self.scroll.y + anchor) / self.placement.height
        } else {
            0.0
        };
        self.zoom_override = zoom.map(|z| z.clamp(MIN_ZOOM, MAX_ZOOM));
        cx.set_global(SessionZoom(self.zoom_override));
        if let Some(bounds) = self.viewport {
            self.layout_frame(bounds);
            self.scroll.y = fraction * self.placement.height - anchor;
            self.clamp_scroll();
        }
        cx.notify();
    }

    pub(crate) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.field_menu_open = false;
        let Some(hit) = self.hit(event.position) else {
            return;
        };
        self.goal_x = None;
        self.core.break_coalescing();
        match event.click_count {
            0 | 1 => {
                let selection = if event.modifiers.shift {
                    Selection::new(self.core.selection().anchor.clone(), hit.clone())
                } else {
                    Selection::caret(hit.clone())
                };
                self.select(selection, window, cx);
                self.drag = Some(Drag {
                    granularity: Granularity::Character,
                    origin: (hit.clone(), hit),
                    last: event.position,
                });
            }
            2 => {
                if !self.read_only
                    && let Some(offset) = self.field_under(event.position, &hit)
                {
                    let path = hit.path.clone();
                    self.edit(window, cx, |core| core.reveal_field(&path, offset));
                    self.drag = None;
                    return;
                }
                let (a, b) = self.word_range(&hit);
                self.select(Selection::new(a.clone(), b.clone()), window, cx);
                self.drag = Some(Drag {
                    granularity: Granularity::Word,
                    origin: (a, b),
                    last: event.position,
                });
            }
            _ => {
                let a = pos(hit.path.clone(), 0);
                let b = end_of(self.core.document(), hit.path.clone());
                self.select(Selection::new(a.clone(), b.clone()), window, cx);
                self.drag = Some(Drag {
                    granularity: Granularity::Block,
                    origin: (a, b),
                    last: event.position,
                });
            }
        }
    }

    fn word_range(&self, hit: &Position) -> (Position, Position) {
        let text = self.core.block_text(&hit.path).unwrap_or_default();
        let range = boundary::word_at(&text, hit.offset);
        (
            pos(hit.path.clone(), range.start),
            pos(hit.path.clone(), range.end).with_affinity(Affinity::Upstream),
        )
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub(crate) fn drag_to(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        drag.last = point;
        let drag = drag.clone();
        let Some(hit) = self.hit(point) else {
            return;
        };
        let selection = match drag.granularity {
            Granularity::Character => Selection::new(self.core.selection().anchor.clone(), hit),
            Granularity::Word | Granularity::Block => {
                let (a, b) = if drag.granularity == Granularity::Word {
                    self.word_range(&hit)
                } else {
                    (
                        pos(hit.path.clone(), 0),
                        end_of(self.core.document(), hit.path.clone()),
                    )
                };
                let before =
                    crate::model::selection::order(&a, &drag.origin.0) == std::cmp::Ordering::Less;
                if before {
                    Selection::new(drag.origin.1.clone(), a)
                } else {
                    Selection::new(drag.origin.0.clone(), b)
                }
            }
        };
        if &selection != self.core.selection() {
            let change = self.core.set_selection(selection);
            self.apply(change, window, cx);
        }
        self.ensure_autoscroll(cx);
    }

    fn ensure_autoscroll(&mut self, cx: &mut Context<Self>) {
        if self.autoscroll_task.is_some() {
            return;
        }
        self.autoscroll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let keep = this.update(cx, |this, cx| this.autoscroll_tick(cx));
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        }));
    }

    fn autoscroll_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let (Some(drag), Some(bounds)) = (self.drag.clone(), self.viewport) else {
            self.autoscroll_task = None;
            return false;
        };
        let y = f32::from(drag.last.y);
        let top = f32::from(bounds.origin.y);
        let bottom = top + f32::from(bounds.size.height);
        let delta = if y < top {
            -((top - y).min(200.0) / 4.0 + 2.0)
        } else if y > bottom {
            (y - bottom).min(200.0) / 4.0 + 2.0
        } else {
            self.autoscroll_task = None;
            return false;
        };
        let before = self.scroll.y;
        self.scroll_by(0.0, delta, cx);
        if (self.scroll.y - before).abs() < 0.01 {
            self.autoscroll_task = None;
            return false;
        }
        let clamped = gpui_kit::point(drag.last.x, px(y.clamp(top + 1.0, bottom - 1.0)));
        if let Some(hit) = self.hit(clamped)
            && drag.granularity == Granularity::Character
        {
            let selection = Selection::new(self.core.selection().anchor.clone(), hit);
            self.core.set_selection_keep_marked(selection);
        }
        cx.notify();
        true
    }

    pub(crate) fn end_drag(&mut self) {
        self.drag = None;
        self.autoscroll_task = None;
    }

    fn head(&self) -> Position {
        self.core.selection().head.clone()
    }

    fn collapse_or_move(
        &mut self,
        select: bool,
        target: Position,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let anchor = if select {
            self.core.selection().anchor.clone()
        } else {
            target.clone()
        };
        self.select(Selection::new(anchor, target), window, cx);
    }

    fn horizontal(
        &mut self,
        direction: HorizontalDirection,
        select: bool,
        word: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.goal_x = None;
        let selection = self.core.selection().clone();
        if !select && !word && !selection.is_collapsed() {
            let (s, e) = effective_range(self.core.document(), &selection);
            let rtl = self.packet.caret(&s).map(|c| c.rtl).unwrap_or(true);
            let forward = (direction == HorizontalDirection::Left) == rtl;
            let target = if forward { e } else { s };
            self.collapse_or_move(false, target, window, cx);
            return;
        }
        let head = self.head();
        let target = if word {
            self.word_move(&head, direction)
        } else {
            self.visual_step(&head, direction)
        };
        self.collapse_or_move(select, target, window, cx);
    }

    fn visual_step(&self, head: &Position, direction: HorizontalDirection) -> Position {
        if let Some(node) = self.packet.text_block(0, &head.path) {
            let layout = node.layout();
            let mut current =
                layout.normalize(node.offsets().to_layout(head.offset, head.affinity));
            for _ in 0..64 {
                let moved = layout.move_visual(current, direction);
                if moved == current {
                    break;
                }
                let (offset, affinity) = node.offsets().to_logical(moved);
                if offset != head.offset {
                    return Position {
                        document: 0,
                        path: head.path.clone(),
                        offset,
                        affinity,
                    };
                }
                current = moved;
            }
        }
        let next = self.packet.move_horizontal(head, direction);
        if next.path != head.path || next.offset != head.offset {
            next
        } else {
            head.clone()
        }
    }

    fn word_move(&self, head: &Position, direction: HorizontalDirection) -> Position {
        let Some(node) = self.packet.text_block(0, &head.path) else {
            return head.clone();
        };
        let layout = node.layout();
        let current = node.offsets().to_layout(head.offset, head.affinity);
        let moved = match direction {
            HorizontalDirection::Left => layout.word_left(current),
            HorizontalDirection::Right => layout.word_right(current),
        };
        let (offset, affinity) = node.offsets().to_logical(moved);
        if offset != head.offset {
            return Position {
                document: 0,
                path: head.path.clone(),
                offset,
                affinity,
            };
        }
        self.visual_step(head, direction)
    }

    fn vertical(
        &mut self,
        direction: VerticalDirection,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selection = self.core.selection().clone();
        if !select && !selection.is_collapsed() {
            let (s, e) = effective_range(self.core.document(), &selection);
            let target = if direction == VerticalDirection::Up {
                s
            } else {
                e
            };
            self.collapse_or_move(false, target, window, cx);
            return;
        }
        let head = self.head();
        let goal = self.goal_x;
        let target = match self.packet.move_vertical(&head, direction, goal) {
            Some((target, x)) => {
                self.goal_x = Some(x);
                target
            }
            None => {
                let edge = match direction {
                    VerticalDirection::Up => document_start(self.core.document()),
                    VerticalDirection::Down => document_end(self.core.document()),
                };
                edge.unwrap_or(head)
            }
        };
        let goal = self.goal_x;
        self.collapse_or_move(select, target, window, cx);
        self.goal_x = goal;
    }

    fn page_move(
        &mut self,
        direction: VerticalDirection,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.viewport else { return };
        let height = f32::from(bounds.size.height) * 0.9;
        let head = self.head();
        let Some(rect) = self.caret_canvas_rect(&head) else {
            return;
        };
        let x = self
            .goal_x
            .map(|g| g * self.placement.zoom)
            .unwrap_or(rect.x);
        let dy = if direction == VerticalDirection::Up {
            -height
        } else {
            height
        };
        let target_y = rect.y + rect.height / 2.0 + dy;
        let screen = gpui_kit::point(
            bounds.origin.x + px(x - self.scroll.x),
            bounds.origin.y + px(target_y - self.scroll.y),
        );
        let target = self.hit(screen).unwrap_or(head);
        self.scroll_by(0.0, dy, cx);
        let goal = self.goal_x;
        self.collapse_or_move(select, target, window, cx);
        self.goal_x = goal;
    }

    fn line_edge(&mut self, end: bool, select: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        let head = self.head();
        let Some(node) = self.packet.text_block(0, &head.path) else {
            return;
        };
        let layout = node.layout();
        let current = node.offsets().to_layout(head.offset, head.affinity);
        let moved = if end {
            layout.line_end(current)
        } else {
            layout.line_start(current)
        };
        let (offset, affinity) = node.offsets().to_logical(moved);
        let target = Position {
            document: 0,
            path: head.path,
            offset,
            affinity,
        };
        self.collapse_or_move(select, target, window, cx);
    }

    fn document_edge(
        &mut self,
        end: bool,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.goal_x = None;
        let target = if end {
            document_end(self.core.document())
        } else {
            document_start(self.core.document())
        };
        if let Some(target) = target {
            self.collapse_or_move(select, target, window, cx);
        }
    }

    fn paragraph_move(
        &mut self,
        down: bool,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let head = self.head();
        let doc = self.core.document();
        let target = if down {
            match crate::model::tree::next_text(doc, &head.path) {
                Some(path) => pos(path, 0),
                None => end_of(doc, head.path.clone()),
            }
        } else if head.offset > 0 {
            pos(head.path.clone(), 0)
        } else {
            match crate::model::tree::prev_text(doc, &head.path) {
                Some(path) => pos(path, 0),
                None => head.clone(),
            }
        };
        self.collapse_or_move(select, target, window, cx);
    }

    fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(fragment) = self.core.copy() else {
            return false;
        };
        let presentation = self.fields.presentation();
        let text = clip::plain_text(&fragment, &|reference: &FieldReference| {
            presentation.display_text(reference)
        });
        let encoded = fragment.encode();
        cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
            text.clone(),
            encoded,
        ));
        self.clipboard = Some((text, fragment));
        true
    }

    pub fn paste_item(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        let mut text = None;
        let mut fragment = None;
        for entry in item.entries() {
            if let ClipboardEntry::String(string) = entry {
                if fragment.is_none()
                    && let Some(metadata) = string.metadata.as_deref()
                {
                    fragment = Fragment::decode(metadata);
                }
                if text.is_none() {
                    text = Some(string.text.clone());
                }
            }
        }
        if fragment.is_none()
            && let (Some(text), Some((copied, internal))) = (&text, &self.clipboard)
            && text == copied
        {
            fragment = Some(internal.clone());
        }
        match (fragment, text) {
            (Some(fragment), _) => self.edit(window, cx, |core| core.paste_fragment(fragment)),
            (None, Some(text)) => self.edit(window, cx, |core| core.paste_text(&text)),
            _ => {}
        }
    }

    pub fn insert_text_command(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |core| core.insert_text(text));
    }

    pub fn toggle_format(&mut self, flag: TextFormat, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |core| core.toggle_flag(flag));
        cx.notify();
    }

    pub fn align(&mut self, align: Align, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |core| core.set_align(align));
    }

    pub fn list(&mut self, kind: ListType, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |core| core.toggle_list(kind));
    }

    pub fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |core| core.undo());
    }

    pub fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |core| core.redo());
    }

    pub fn open_field_menu(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.field_menu_open = open;
        if open {
            self.menu_selection = Some(self.core.selection().clone());
            self.field_query.clear();
            self.field_search.update(cx, |input, cx| {
                input.set_text("", cx);
                input.focus(window, cx);
            });
        } else {
            self.menu_selection = None;
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    pub fn field_menu_is_open(&self) -> bool {
        self.field_menu_open
    }

    pub fn field_choices(&self) -> Vec<(&'static str, Vec<(String, FieldReference)>)> {
        let query = self.field_query.trim().to_lowercase();
        let matches = |label: &str| query.is_empty() || label.to_lowercase().contains(&query);
        let fir: Vec<(String, FieldReference)> = missal_core::FIR_FIELDS
            .iter()
            .filter(|f| matches(f.label))
            .map(|f| (f.label.to_owned(), FieldReference::Fir(f.field)))
            .collect();
        let mut global = Vec::new();
        let mut custom = Vec::new();
        for placeholder in self.fields.catalog.placeholders() {
            if !matches(&placeholder.label) {
                continue;
            }
            let entry = (
                placeholder.label.clone(),
                FieldReference::Catalog(placeholder.id),
            );
            match placeholder.source {
                missal_core::PlaceholderSource::Custom => custom.push(entry),
                missal_core::PlaceholderSource::SharedSetting { .. } => global.push(entry),
            }
        }
        [("FIR", fir), ("Global", global), ("Custom", custom)]
            .into_iter()
            .filter(|(_, items)| !items.is_empty())
            .collect()
    }

    pub fn choose_field(
        &mut self,
        reference: FieldReference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(selection) = self.menu_selection.take() {
            let change = self.core.set_selection(selection);
            self.apply(change, window, cx);
        }
        self.field_menu_open = false;
        self.edit(window, cx, |core| core.insert_field(reference));
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn head_text(&self) -> Option<(BlockPath, String)> {
        let path = self.core.selection().head.path.clone();
        let text = self.core.block_text(&path)?;
        Some((path, text))
    }
}

macro_rules! handlers {
    ($($action:ty => |$this:ident, $window:ident, $cx:ident| $body:expr;)*) => {
        impl Editor {
            pub(crate) fn register_actions(&self, root: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
                root
                $(
                    .on_action(cx.listener(|$this: &mut Editor, _: &$action, $window: &mut Window, $cx: &mut Context<Editor>| { $body; }))
                )*
            }
        }
    };
}

handlers! {
    text_actions::MoveLeft => |this, window, cx| this.horizontal(HorizontalDirection::Left, false, false, window, cx);
    text_actions::MoveRight => |this, window, cx| this.horizontal(HorizontalDirection::Right, false, false, window, cx);
    text_actions::SelectLeft => |this, window, cx| this.horizontal(HorizontalDirection::Left, true, false, window, cx);
    text_actions::SelectRight => |this, window, cx| this.horizontal(HorizontalDirection::Right, true, false, window, cx);
    text_actions::WordLeft => |this, window, cx| this.horizontal(HorizontalDirection::Left, false, true, window, cx);
    text_actions::WordRight => |this, window, cx| this.horizontal(HorizontalDirection::Right, false, true, window, cx);
    text_actions::SelectWordLeft => |this, window, cx| this.horizontal(HorizontalDirection::Left, true, true, window, cx);
    text_actions::SelectWordRight => |this, window, cx| this.horizontal(HorizontalDirection::Right, true, true, window, cx);
    text_actions::MoveUp => |this, window, cx| this.vertical(VerticalDirection::Up, false, window, cx);
    text_actions::MoveDown => |this, window, cx| this.vertical(VerticalDirection::Down, false, window, cx);
    text_actions::SelectUp => |this, window, cx| this.vertical(VerticalDirection::Up, true, window, cx);
    text_actions::SelectDown => |this, window, cx| this.vertical(VerticalDirection::Down, true, window, cx);
    text_actions::LineStart => |this, window, cx| this.line_edge(false, false, window, cx);
    text_actions::LineEnd => |this, window, cx| this.line_edge(true, false, window, cx);
    text_actions::SelectLineStart => |this, window, cx| this.line_edge(false, true, window, cx);
    text_actions::SelectLineEnd => |this, window, cx| this.line_edge(true, true, window, cx);
    text_actions::DocumentStart => |this, window, cx| this.document_edge(false, false, window, cx);
    text_actions::DocumentEnd => |this, window, cx| this.document_edge(true, false, window, cx);
    text_actions::SelectDocumentStart => |this, window, cx| this.document_edge(false, true, window, cx);
    text_actions::SelectDocumentEnd => |this, window, cx| this.document_edge(true, true, window, cx);
    text_actions::Backspace => |this, window, cx| this.edit(window, cx, |core| core.delete(false, false));
    text_actions::Delete => |this, window, cx| this.edit(window, cx, |core| core.delete(true, false));
    text_actions::DeleteWordBackward => |this, window, cx| this.edit(window, cx, |core| core.delete(false, true));
    text_actions::DeleteWordForward => |this, window, cx| this.edit(window, cx, |core| core.delete(true, true));
    text_actions::SelectAll => |this, window, cx| {
        let change = this.core.select_all();
        this.apply(change, window, cx)
    };
    text_actions::Copy => |this, _window, cx| { this.copy_selection(cx); };
    text_actions::Cut => |this, window, cx| {
        if !this.read_only && this.copy_selection(cx) {
            this.edit(window, cx, |core| core.delete(false, false));
        }
    };
    text_actions::Paste => |this, window, cx| {
        if let Some(item) = cx.read_from_clipboard() {
            this.paste_item(item, window, cx);
        }
    };
    text_actions::Undo => |this, window, cx| this.undo(window, cx);
    text_actions::Redo => |this, window, cx| this.redo(window, cx);
    text_actions::Enter => |this, window, cx| this.edit(window, cx, |core| core.split());
    text_actions::InsertNewline => |this, window, cx| this.edit(window, cx, |core| core.line_break());
    keys::NextCell => |this, window, cx| {
        if this.core.in_list() {
            this.edit(window, cx, |core| core.indent(false));
        } else if let Some(selection) = this.core.cell_step(false) {
            this.select(selection, window, cx);
        }
    };
    keys::PreviousCell => |this, window, cx| {
        if this.core.in_list() {
            this.edit(window, cx, |core| core.indent(true));
        } else if let Some(selection) = this.core.cell_step(true) {
            this.select(selection, window, cx);
        }
    };
    keys::PageUp => |this, window, cx| this.page_move(VerticalDirection::Up, false, window, cx);
    keys::PageDown => |this, window, cx| this.page_move(VerticalDirection::Down, false, window, cx);
    keys::SelectPageUp => |this, window, cx| this.page_move(VerticalDirection::Up, true, window, cx);
    keys::SelectPageDown => |this, window, cx| this.page_move(VerticalDirection::Down, true, window, cx);
    keys::ToggleBold => |this, window, cx| this.toggle_format(TextFormat::BOLD, window, cx);
    keys::ToggleItalic => |this, window, cx| this.toggle_format(TextFormat::ITALIC, window, cx);
    keys::ToggleUnderline => |this, window, cx| this.toggle_format(TextFormat::UNDERLINE, window, cx);
    keys::AlignLeft => |this, window, cx| this.align(Align::Left, window, cx);
    keys::AlignCenter => |this, window, cx| this.align(Align::Center, window, cx);
    keys::AlignRight => |this, window, cx| this.align(Align::Right, window, cx);
    keys::AlignJustify => |this, window, cx| this.align(Align::Justify, window, cx);
    keys::BulletList => |this, window, cx| this.list(ListType::Bullet, window, cx);
    keys::NumberedList => |this, window, cx| this.list(ListType::Number, window, cx);
    keys::IndentList => |this, window, cx| this.edit(window, cx, |core| core.indent(false));
    keys::OutdentList => |this, window, cx| this.edit(window, cx, |core| core.indent(true));
    keys::ZoomIn => |this, _window, cx| this.set_zoom(Some(this.placement.zoom * 1.1), cx);
    keys::ZoomOut => |this, _window, cx| this.set_zoom(Some(this.placement.zoom / 1.1), cx);
    keys::ZoomReset => |this, _window, cx| this.set_zoom(None, cx);
    keys::InsertField => |this, window, cx| this.open_field_menu(true, window, cx);
    keys::ParagraphUp => |this, window, cx| this.paragraph_move(false, false, window, cx);
    keys::ParagraphDown => |this, window, cx| this.paragraph_move(true, false, window, cx);
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let (_, text) = self.head_text()?;
        let start = utf16_to_byte(&text, range_utf16.start);
        let end = utf16_to_byte(&text, range_utf16.end).max(start);
        adjusted_range.replace(byte_to_utf16(&text, start)..byte_to_utf16(&text, end));
        Some(text[start..end].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (path, text) = self.head_text()?;
        let selection = self.core.selection();
        let head = byte_to_utf16(&text, selection.head.offset);
        if selection.anchor.path != path {
            return Some(UTF16Selection {
                range: head..head,
                reversed: false,
            });
        }
        let anchor = byte_to_utf16(&text, selection.anchor.offset);
        Some(UTF16Selection {
            range: anchor.min(head)..anchor.max(head),
            reversed: head < anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let marked = self.core.marked()?;
        let text = self.core.block_text(&marked.path)?;
        Some(byte_to_utf16(&text, marked.range.start)..byte_to_utf16(&text, marked.range.end))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let change = self.core.unmark();
        self.apply(change, window, cx);
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        if text == "\n" || text == "\r" || text == "\r\n" {
            if let Some(marked) = self.core.marked().cloned() {
                let _ = marked;
                let change = self.core.commit("");
                self.apply(change, window, cx);
            }
            self.edit(window, cx, |core| core.split());
            return;
        }
        if self.core.marked().is_some() {
            if let Some(range) = range_utf16
                && let Some((path, block)) = self.head_text()
            {
                let start = utf16_to_byte(&block, range.start);
                let end = utf16_to_byte(&block, range.end).max(start);
                self.core.set_marked(Some(crate::model::Marked {
                    path,
                    range: start..end,
                }));
            }
            let text = text.to_owned();
            self.edit(window, cx, |core| core.commit(&text));
            return;
        }
        if let Some(range) = range_utf16
            && let Some((path, block)) = self.head_text()
        {
            let start = utf16_to_byte(&block, range.start);
            let end = utf16_to_byte(&block, range.end).max(start);
            self.core.set_selection_keep_marked(Selection::new(
                pos(path.clone(), start),
                pos(path, end),
            ));
        }
        let text = text.to_owned();
        self.edit(window, cx, |core| core.insert_text(&text));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        if let Some(range) = range_utf16
            && let Some((path, block)) = self.head_text()
        {
            let start = utf16_to_byte(&block, range.start);
            let end = utf16_to_byte(&block, range.end).max(start);
            self.core.set_marked(Some(crate::model::Marked {
                path,
                range: start..end,
            }));
        }
        let text = new_text.to_owned();
        self.edit(window, cx, |core| {
            core.compose(&text, new_selected_range_utf16)
        });
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (path, text) = self.head_text()?;
        let start = utf16_to_byte(&text, range_utf16.start);
        let end = utf16_to_byte(&text, range_utf16.end).max(start);
        if end > start {
            let rects = self
                .packet
                .selection(&pos(path.clone(), start), &pos(path.clone(), end));
            if let Some((page, rect)) = rects.first() {
                return self.canvas_to_screen(self.page_rect_to_canvas(*page, *rect)?);
            }
        }
        self.position_screen_bounds(&pos(path, start))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let hit = self.hit(point)?;
        let (path, text) = self.head_text()?;
        (hit.path == path).then(|| byte_to_utf16(&text, hit.offset))
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        !self.read_only
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let canvas = EditorTheme::get(cx).canvas;
        let root = div()
            .id("missal-editor")
            .key_context("MissalEditor MissalTextInput")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(canvas);
        let root = self.register_actions(root, cx);
        let toolbar = (!self.read_only).then(|| self.render_toolbar(window, cx));
        root.children(toolbar).child(
            div()
                .id("missal-editor-canvas")
                .flex_1()
                .min_h_0()
                .w_full()
                .when(self.read_only, |this| this.opacity(1.0))
                .child(PagesElement::new(entity)),
        )
    }
}

pub fn init(cx: &mut App) {
    keys::bind_keys(cx);
}
