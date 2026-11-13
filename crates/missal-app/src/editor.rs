use std::rc::Rc;

use gpui_kit::component::{StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyView, App, AppContext, Context, Entity, EventEmitter, Global, IntoElement, ParentElement,
    Render, Styled, Subscription, Window, div, px,
};
use missal_core::document::{Block, Inline, Paragraph, Text};
use missal_core::{
    DisplayMode, Document, FieldCatalog, FieldMarkers, FieldPresentation, FirValues,
};
use missal_text::{Direction, TextInput, TextInputEvent, label};

use crate::theme::Palette;

#[derive(Clone)]
pub struct FieldContext {
    pub catalog: Rc<FieldCatalog>,
    pub values: Option<Rc<FirValues>>,
    pub mode: DisplayMode,
    pub markers: FieldMarkers,
}

impl FieldContext {
    pub fn labels(catalog: Rc<FieldCatalog>, markers: FieldMarkers) -> FieldContext {
        FieldContext {
            catalog,
            values: None,
            mode: DisplayMode::Labels,
            markers,
        }
    }

    pub fn for_fir(
        catalog: Rc<FieldCatalog>,
        values: Rc<FirValues>,
        mode: DisplayMode,
        markers: FieldMarkers,
    ) -> FieldContext {
        FieldContext {
            catalog,
            values: Some(values),
            mode,
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

pub struct LoadRequest {
    pub key: String,
    pub document: Document,
    pub fields: FieldContext,
}

#[derive(Clone)]
pub struct Snapshot {
    pub document: Document,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    DirtyChanged(bool),
    SaveRequested,
}

pub trait DocumentEditor {
    fn view(&self) -> AnyView;
    fn load(&self, request: LoadRequest, window: &mut Window, cx: &mut App);
    fn set_fields(&self, fields: FieldContext, cx: &mut App);
    fn is_dirty(&self, cx: &App) -> bool;
    fn snapshot(&self, cx: &App) -> Snapshot;
    fn mark_saved(&self, revision: u64, cx: &mut App);
    fn discard(&self, window: &mut Window, cx: &mut App);
    fn focus(&self, window: &mut Window, cx: &mut App);
    fn on_event(&self, cx: &mut App, listener: Box<dyn Fn(&EditorEvent, &mut App)>) -> Subscription;
}

#[derive(Clone)]
pub struct EditorFactory(pub Rc<dyn Fn(&mut Window, &mut App) -> Rc<dyn DocumentEditor>>);

impl Global for EditorFactory {}

impl EditorFactory {
    pub fn stand_in() -> EditorFactory {
        EditorFactory(Rc::new(|window, cx| {
            let editor = cx.new(|cx| StandInEditor::new(window, cx));
            Rc::new(StandInHandle { editor })
        }))
    }

    pub fn create(cx: &App) -> EditorFactory {
        cx.try_global::<EditorFactory>()
            .cloned()
            .unwrap_or_else(EditorFactory::stand_in)
    }

    pub fn build(&self, window: &mut Window, cx: &mut App) -> Rc<dyn DocumentEditor> {
        (self.0)(window, cx)
    }
}

pub struct StandInEditor {
    key: String,
    original: Option<Document>,
    fields: Option<FieldContext>,
    note: Entity<TextInput>,
    revision: u64,
    saved_revision: u64,
    dirty: bool,
    loading: bool,
    _subscription: Subscription,
}

impl EventEmitter<EditorEvent> for StandInEditor {}

impl StandInEditor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> StandInEditor {
        let note = cx.new(|cx| {
            TextInput::new(window, cx)
                .multiline(2, 4)
                .direction(Direction::Rtl)
                .text_size(16.0)
                .placeholder("اس خانے میں لکھنے سے دستاویز میں تبدیلی کا عمل آزمایا جا سکتا ہے")
        });
        let subscription = cx.subscribe(&note, |this, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Change && !this.loading {
                this.revision += 1;
                this.refresh_dirty(cx);
                cx.notify();
            }
        });
        StandInEditor {
            key: String::new(),
            original: None,
            fields: None,
            note,
            revision: 0,
            saved_revision: 0,
            dirty: false,
            loading: false,
            _subscription: subscription,
        }
    }

    fn refresh_dirty(&mut self, cx: &mut Context<Self>) {
        let dirty = self.revision != self.saved_revision;
        if dirty != self.dirty {
            self.dirty = dirty;
            cx.emit(EditorEvent::DirtyChanged(dirty));
        }
    }

    pub fn load(&mut self, request: LoadRequest, cx: &mut Context<Self>) {
        self.loading = true;
        self.key = request.key;
        self.original = Some(request.document);
        self.fields = Some(request.fields);
        self.revision = 0;
        self.saved_revision = 0;
        self.note.update(cx, |note, cx| note.set_text("", cx));
        self.loading = false;
        self.refresh_dirty(cx);
        cx.notify();
    }

    pub fn snapshot(&self, cx: &App) -> Snapshot {
        let mut document = self.original.clone().unwrap_or_default();
        let note = self.note.read(cx).text().trim().to_owned();
        if !note.is_empty() {
            document.blocks.push(paragraph_block(&note));
        }
        Snapshot {
            document,
            revision: self.revision,
        }
    }

    pub fn mark_saved(&mut self, revision: u64, cx: &mut Context<Self>) {
        self.saved_revision = revision;
        self.refresh_dirty(cx);
        cx.notify();
    }

    pub fn discard(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.note.update(cx, |note, cx| note.set_text("", cx));
        self.loading = false;
        self.revision = 0;
        self.saved_revision = 0;
        self.refresh_dirty(cx);
        cx.notify();
    }

    pub fn set_fields(&mut self, fields: FieldContext, cx: &mut Context<Self>) {
        self.fields = Some(fields);
        cx.notify();
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Render for StandInEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let document = self.original.as_ref();
        let projections = document.map(Document::projections).unwrap_or_default();
        let paragraphs = document.map_or(0, |document| document.blocks.len());
        let fields: Vec<(String, bool)> = match (&self.fields, document) {
            (Some(fields), Some(document)) => {
                let presentation = fields.presentation();
                document
                    .field_references()
                    .iter()
                    .take(8)
                    .map(|reference| {
                        let shown = presentation.display_text(reference);
                        (shown.text, shown.unresolved)
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        let mode = match self.fields.as_ref().map(|fields| (fields.values.is_some(), fields.mode)) {
            Some((true, DisplayMode::Values)) => "values",
            _ => "field names",
        };
        let preview: String = projections.preview_text.chars().take(160).collect();
        v_flex()
            .id("editor-placeholder")
            .size_full()
            .p_6()
            .bg(palette.canvas)
            .child(
                v_flex()
                    .size_full()
                    .gap_3()
                    .p_6()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(palette.input)
                    .bg(palette.paper)
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_semibold()
                            .child("Document editor mounts here"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(palette.muted_foreground)
                            .child(format!(
                                "Stand-in for the rich editor. {paragraphs} blocks, {} fields shown as {mode}.",
                                projections.field_count
                            )),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_size(px(16.))
                            .text_color(palette.paper_foreground)
                            .child(label(if preview.is_empty() { "خالی".to_owned() } else { preview }).rtl()),
                    )
                    .child(
                        h_flex().flex_wrap().gap_2().children(fields.into_iter().map(
                            |(text, unresolved)| {
                                div()
                                    .px_2()
                                    .rounded(px(5.))
                                    .bg(palette.muted)
                                    .text_size(px(14.))
                                    .text_color(if unresolved { palette.muted_foreground } else { palette.field })
                                    .child(label(text).rtl())
                            },
                        )),
                    )
                    .child(div().w_full().child(self.note.clone())),
            )
    }
}

fn paragraph_block(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(vec![Inline::Text(Text::new(text))]))
}

pub struct StandInHandle {
    editor: Entity<StandInEditor>,
}

impl DocumentEditor for StandInHandle {
    fn view(&self) -> AnyView {
        self.editor.clone().into()
    }

    fn load(&self, request: LoadRequest, _: &mut Window, cx: &mut App) {
        self.editor.update(cx, |editor, cx| editor.load(request, cx));
    }

    fn set_fields(&self, fields: FieldContext, cx: &mut App) {
        self.editor.update(cx, |editor, cx| editor.set_fields(fields, cx));
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.editor.read(cx).is_dirty()
    }

    fn snapshot(&self, cx: &App) -> Snapshot {
        self.editor.read(cx).snapshot(cx)
    }

    fn mark_saved(&self, revision: u64, cx: &mut App) {
        self.editor.update(cx, |editor, cx| editor.mark_saved(revision, cx));
    }

    fn discard(&self, _: &mut Window, cx: &mut App) {
        self.editor.update(cx, |editor, cx| editor.discard(cx));
    }

    fn focus(&self, window: &mut Window, cx: &mut App) {
        let note = self.editor.read(cx).note.clone();
        note.update(cx, |note, cx| note.focus(window, cx));
    }

    fn on_event(&self, cx: &mut App, listener: Box<dyn Fn(&EditorEvent, &mut App)>) -> Subscription {
        cx.subscribe(&self.editor, move |_, event: &EditorEvent, cx| listener(event, cx))
    }
}
