use std::rc::Rc;

use gpui_kit::{AnyView, App, AppContext, Entity, Global, Subscription, Window};
use missal_core::{
    DisplayMode, Document, FieldCatalog, FieldMarkers, FieldPresentation, FieldReference, FirValues,
};
use missal_editor::{Editor, EditorFields, EditorTheme};

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
    ModeRequested(DisplayMode),
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
    fn set_mode_toggle(&self, _shown: bool, _cx: &mut App) {}
    fn field_references(&self, _cx: &App) -> Vec<FieldReference> {
        Vec::new()
    }
    fn reveal_field(
        &self,
        _reference: &FieldReference,
        _window: &mut Window,
        _cx: &mut App,
    ) -> bool {
        false
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
    sync_theme(cx);
    cx.observe_global::<Palette>(sync_theme).detach();
}

fn sync_theme(cx: &mut App) {
    let Some(palette) = cx.try_global::<Palette>().copied() else {
        return;
    };
    EditorTheme {
        surface: palette.surface,
        raised: palette.raised,
        canvas: palette.sunken,
        paper: palette.paper,
        ink: palette.ink,
        ink_muted: palette.ink_muted,
        ink_faint: palette.ink_faint,
        line: palette.line,
        line_strong: palette.line_strong,
        hover: palette.hover,
        pressed: palette.pressed,
        accent: palette.accent,
        field_wash: palette.field_wash,
        notice: palette.notice,
        notice_text: palette.notice_text,
        notice_wash: palette.notice_wash,
        selection: palette
            .accent
            .opacity(if palette.dark { 0.40 } else { 0.28 }),
        shadow: if palette.dark {
            gpui_kit::black().opacity(0.35)
        } else {
            crate::theme::hex(0x14110e).opacity(0.05)
        },
    }
    .install(cx);
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
        self.editor.update(cx, |editor, cx| {
            editor.load(request.key, request.document, fields, request.dirty, cx)
        });
    }

    fn set_fields(&self, fields: FieldContext, cx: &mut App) {
        let fields = editor_fields(fields);
        self.editor
            .update(cx, |editor, cx| editor.set_fields(fields, cx));
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.editor.read(cx).is_dirty()
    }

    fn snapshot(&self, cx: &App) -> Snapshot {
        let (document, revision) = self.editor.read(cx).snapshot();
        Snapshot { document, revision }
    }

    fn mark_saved(&self, revision: u64, cx: &mut App) {
        self.editor
            .update(cx, |editor, cx| editor.mark_saved(revision, cx));
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

    fn set_mode_toggle(&self, shown: bool, cx: &mut App) {
        self.editor
            .update(cx, |editor, cx| editor.set_mode_toggle(shown, cx));
    }

    fn field_references(&self, cx: &App) -> Vec<FieldReference> {
        self.editor.read(cx).document().field_references()
    }

    fn reveal_field(&self, reference: &FieldReference, window: &mut Window, cx: &mut App) -> bool {
        self.editor.update(cx, |editor, cx| {
            editor.reveal_reference(reference, window, cx)
        })
    }

    fn on_event(&self, cx: &mut App, listener: EditorListener) -> Subscription {
        cx.subscribe(
            &self.editor,
            move |_, event: &missal_editor::EditorEvent, cx| {
                let event = match event {
                    missal_editor::EditorEvent::DirtyChanged(dirty) => {
                        EditorEvent::DirtyChanged(*dirty)
                    }
                    missal_editor::EditorEvent::SaveRequested => EditorEvent::SaveRequested,
                    missal_editor::EditorEvent::ModeRequested(mode) => {
                        EditorEvent::ModeRequested(*mode)
                    }
                };
                listener(&event, cx)
            },
        )
    }
}
