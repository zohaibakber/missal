use std::rc::Rc;

use gpui_kit::{AnyView, App, AppContext, Entity, Global, Subscription, Window};
use missal_core::{DisplayMode, Document, FieldCatalog, FieldMarkers, FieldPresentation, FirValues};
use missal_editor::{Editor, EditorFields};

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
    pub dirty: bool,
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
    fn on_event(&self, cx: &mut App, listener: EditorListener) -> Subscription;
    fn editor_entity(&self) -> Option<Entity<Editor>> {
        None
    }
}

pub type EditorBuilder = Rc<dyn Fn(&mut Window, &mut App) -> Rc<dyn DocumentEditor>>;
pub type EditorListener = Box<dyn Fn(&EditorEvent, &mut App)>;

#[derive(Clone)]
pub struct EditorFactory(pub EditorBuilder);

impl Global for EditorFactory {}

impl EditorFactory {
    pub fn rich() -> EditorFactory {
        EditorFactory(Rc::new(|window, cx| {
            let editor = cx.new(|cx| Editor::new(window, cx));
            Rc::new(RichEditor { editor })
        }))
    }

    pub fn create(cx: &App) -> EditorFactory {
        cx.try_global::<EditorFactory>()
            .cloned()
            .unwrap_or_else(EditorFactory::rich)
    }

    pub fn build(&self, window: &mut Window, cx: &mut App) -> Rc<dyn DocumentEditor> {
        (self.0)(window, cx)
    }
}

pub fn install(cx: &mut App) {
    missal_editor::init(cx);
    cx.set_global(EditorFactory::rich());
}

fn editor_fields(fields: FieldContext) -> EditorFields {
    EditorFields {
        catalog: fields.catalog,
        values: fields.values,
        mode: fields.mode,
        markers: fields.markers,
    }
}

pub struct RichEditor {
    editor: Entity<Editor>,
}

impl RichEditor {
    pub fn entity(&self) -> &Entity<Editor> {
        &self.editor
    }
}

impl DocumentEditor for RichEditor {
    fn view(&self) -> AnyView {
        self.editor.clone().into()
    }

    fn load(&self, request: LoadRequest, _: &mut Window, cx: &mut App) {
        let fields = editor_fields(request.fields);
        self.editor
            .update(cx, |editor, cx| editor.load(request.key, request.document, fields, request.dirty, cx));
    }

    fn set_fields(&self, fields: FieldContext, cx: &mut App) {
        let fields = editor_fields(fields);
        self.editor.update(cx, |editor, cx| editor.set_fields(fields, cx));
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.editor.read(cx).is_dirty()
    }

    fn snapshot(&self, cx: &App) -> Snapshot {
        let (document, revision) = self.editor.read(cx).snapshot();
        Snapshot { document, revision }
    }

    fn mark_saved(&self, revision: u64, cx: &mut App) {
        self.editor.update(cx, |editor, cx| editor.mark_saved(revision, cx));
    }

    fn discard(&self, _: &mut Window, cx: &mut App) {
        self.editor.update(cx, |editor, cx| editor.discard(cx));
    }

    fn focus(&self, window: &mut Window, cx: &mut App) {
        let handle = self.editor.read(cx).focus_handle_ref().clone();
        window.focus(&handle, cx);
    }

    fn editor_entity(&self) -> Option<Entity<Editor>> {
        Some(self.editor.clone())
    }

    fn on_event(&self, cx: &mut App, listener: EditorListener) -> Subscription {
        cx.subscribe(&self.editor, move |_, event: &missal_editor::EditorEvent, cx| {
            let event = match event {
                missal_editor::EditorEvent::DirtyChanged(dirty) => EditorEvent::DirtyChanged(*dirty),
                missal_editor::EditorEvent::SaveRequested => EditorEvent::SaveRequested,
            };
            listener(&event, cx)
        })
    }
}
