use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;

use gpui_kit::{
    Anchor, App, AppContext, Bounds, Context, Entity, ExternalPaths, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Render, ScrollStrategy,
    SharedString, StatefulInteractiveElement, Styled, Subscription, UniformListScrollHandle,
    Window, canvas, div, point, px, uniform_list,
};
use missal_core::{DisplayMode, Document, Revision, TemplateId};
use missal_editor::{Editor, EditorEvent, EditorFields};
use missal_store::{DraftKey, TemplateSummary};
use missal_text::{
    Attr, AttributedText, FontFamily, Label, TextInput, TextInputEvent, TextStyle, label, to_rgba,
};

use crate::app::{import_word_file, navigate, navigate_replace, open_print_preview};
use crate::autosave::{self, Autosave, Autosaving};
use crate::editor;
use crate::library::LibraryModel;
use crate::shortcuts::*;
use crate::state::rows::fold_for_search;
use crate::state::templates::TemplatesEvent;
use crate::state::{
    Load, Models, Route, ToastKind, run_db_in, toast_detailed, toast_error, toast_success,
};
use crate::theme::Palette;
use crate::ui::dialogs::{Choice, Tone, confirm_delete, open_choice};
use crate::ui::input::{Kind, field, field_input, search_field};
use crate::ui::library::open_library;
use crate::ui::popup_menu::{Dismissed, MenuContent, MenuEntry, Placement, PopupList, menu};
use crate::ui::print_preview::{PreviewDocument, PrintRequest};
use crate::ui::titlebar::HeaderContent;
use crate::ui::{
    Intent, SaveState, alert, empty_state, empty_state_body, ghost_sm, icon_button, isolate_urdu,
    primary_sm, save_state, section_label,
};
use crate::word::{WordImport, is_docx};

type Continuation = Box<dyn FnOnce(&mut Window, &mut App)>;

const LIST_WIDTH: f32 = crate::theme::RAIL_WIDTH;
const ROW_HEIGHT: f32 = 44.0;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PageState {
    Loading,
    Ready,
    NotFound,
    Failed(String),
}

pub struct TemplatePage {
    models: Models,
    id: Option<TemplateId>,
    title: Entity<TextInput>,
    initial_name: String,
    revision: Option<Revision>,
    state: PageState,
    editor: Entity<Editor>,
    editor_dirty: bool,
    saving: bool,
    autosave: Autosave,
    picking: bool,
    importing: bool,
    renaming: bool,
    review: Option<ImportReview>,
    _subscriptions: Vec<Subscription>,
}

struct ImportReview {
    file_name: String,
    field_count: usize,
    unrecognised: Vec<String>,
    notices: Vec<&'static str>,
}

impl TemplatePage {
    pub fn new(
        models: Models,
        id: Option<TemplateId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TemplatePage {
        let title = field_input(window, cx, Kind::Urdu, "Template name", "");
        let editor = crate::editor::shared(window, cx);
        let editor_subscription =
            cx.subscribe(&editor, |this, _, event: &EditorEvent, cx| match event {
                EditorEvent::DirtyChanged(dirty) => {
                    this.editor_dirty = *dirty;
                    cx.notify();
                }
                EditorEvent::Edited => autosave::edited(this, cx),
                _ => {}
            });
        let title_subscription = cx.subscribe_in(
            &title,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Change => cx.notify(),
                TextInputEvent::Submit => this.commit_rename(true, window, cx),
                TextInputEvent::Blur => this.commit_rename(false, window, cx),
                TextInputEvent::Focus => {}
            },
        );
        let mut page = TemplatePage {
            models,
            id,
            title,
            initial_name: String::new(),
            revision: None,
            state: PageState::Loading,
            editor,
            editor_dirty: false,
            saving: false,
            autosave: Autosave::default(),
            picking: false,
            importing: false,
            renaming: id.is_none(),
            review: None,
            _subscriptions: vec![editor_subscription, title_subscription],
        };
        let weak = cx.weak_entity();
        page.editor.update(cx, |editor, cx| {
            editor.set_toolbar_end(
                move |_, cx| {
                    weak.update(cx, |page, cx| page.name_pill(cx))
                        .ok()
                        .flatten()
                },
                cx,
            )
        });
        page.load(window, cx);
        page
    }

    fn field_context(&self, cx: &App) -> EditorFields {
        let catalog = self.models.catalog.read(cx);
        EditorFields::labels(
            catalog.field_catalog(),
            catalog.settings.field_markers.clone(),
        )
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.id {
            None => {
                self.state = PageState::Ready;
                let fields = self.field_context(cx);
                self.editor.update(cx, |editor, cx| {
                    editor.load(Document::empty(), fields, false, cx)
                });
                self.title.update(cx, |input, cx| input.focus(window, cx));
            }
            Some(id) => {
                let key = DraftKey::Template(id);
                run_db_in(
                    window,
                    cx,
                    move |store| Ok((store.template(id)?, store.draft(key)?)),
                    move |this, result, window, cx| {
                        match result {
                            Ok((record, draft)) => {
                                this.revision = Some(record.revision);
                                this.initial_name = record.name.clone();
                                this.title.update(cx, |input, cx| {
                                    input.set_text(record.name.clone(), cx)
                                });
                                let fields = this.field_context(cx);
                                this.autosave.track(key, record.revision, draft.is_some());
                                let recovered = draft.is_some();
                                this.editor.update(cx, |editor, cx| match draft {
                                    Some(draft) => {
                                        editor.restore(record.document, draft.document, fields, cx)
                                    }
                                    None => editor.load(record.document, fields, false, cx),
                                });
                                this.editor_dirty = this.editor.read(cx).is_dirty();
                                this.state = PageState::Ready;
                                if recovered {
                                    let weak = cx.entity().downgrade();
                                    autosave::announce_recovered(window, cx, move |_, cx| {
                                        weak.update(cx, |this, cx| this.discard_recovered(cx)).ok();
                                    });
                                }
                            }
                            Err(missal_core::RepositoryError::NotFound { .. }) => {
                                this.state = PageState::NotFound
                            }
                            Err(error) => this.state = PageState::Failed(error.message()),
                        }
                        cx.notify();
                    },
                );
            }
        }
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.state == PageState::Ready
            && (self.title.read(cx).text() != self.initial_name
                || self.editor_dirty
                || self.editor.read(cx).is_dirty())
    }

    pub fn title_input(&self) -> &Entity<TextInput> {
        &self.title
    }

    pub fn editor(&self) -> &Entity<Editor> {
        &self.editor
    }

    pub fn state(&self) -> &PageState {
        &self.state
    }

    pub fn is_importing(&self) -> bool {
        self.importing
    }

    pub fn open_print_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state != PageState::Ready {
            return;
        }
        let name = self.title.read(cx).text().trim().to_owned();
        let title = if name.is_empty() {
            "Untitled template".to_owned()
        } else {
            name
        };
        let request = PrintRequest {
            export_name: title.clone(),
            title,
            note: Some("Placeholders show their names".to_owned()),
            catalog: self.models.catalog.read(cx).field_catalog(),
            values: None,
            mode: DisplayMode::Labels,
            documents: vec![PreviewDocument::Content(std::sync::Arc::new(
                self.editor.read(cx).snapshot().0,
            ))],
        };
        open_print_preview(request, window, cx);
    }

    pub fn request_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picking || self.importing || self.saving || self.state != PageState::Ready {
            return;
        }
        self.picking = true;
        let chosen = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a Word document (.docx)".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = chosen.await;
            this.update_in(cx, |this, window, cx| {
                this.picking = false;
                match outcome {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.choose_import(path, window, cx);
                        }
                    }
                    Ok(Err(error)) => toast_detailed(
                        window,
                        cx,
                        ToastKind::Error,
                        "Could not import Word document",
                        format!("The file dialog could not be opened ({error})."),
                    ),
                    _ => {}
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn choose_import(
        &mut self,
        path: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.importing || self.saving || self.state != PageState::Ready {
            return;
        }
        if !is_docx(&path) {
            toast_detailed(
                window,
                cx,
                ToastKind::Error,
                "Could not import Word document",
                "Choose a Word document with the .docx extension.",
            );
            return;
        }
        let projections = self.editor.read(cx).document().projections();
        let has_content = self.id.is_some()
            || self.editor_dirty
            || self.editor.read(cx).is_dirty()
            || !projections.plain_text.trim().is_empty()
            || projections.field_count > 0;
        if !has_content {
            self.run_import(path, window, cx);
            return;
        }
        let weak = cx.entity().downgrade();
        open_choice(
            window,
            cx,
            "Replace template content?",
            "Importing replaces all text and formatting in this editor. The saved template stays unchanged until you save.",
            vec![
                Choice::new("cancel", "Cancel", Tone::Neutral, |_, _| {}),
                Choice::new(
                    "import-replace",
                    "Import and replace",
                    Tone::Primary,
                    move |window, cx| {
                        let path = path.clone();
                        weak.update(cx, |this, cx| this.run_import(path, window, cx))
                            .ok();
                    },
                ),
            ],
        );
    }

    fn run_import(
        &mut self,
        path: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.importing = true;
        cx.notify();
        let catalog = self.models.catalog.read(cx);
        let fields = catalog.field_catalog();
        let markers = catalog.settings.field_markers.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::word::import_file(path, &fields, &markers) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.finish_import(result, window, cx)
            })
            .ok();
        })
        .detach();
    }

    fn finish_import(
        &mut self,
        result: Result<WordImport, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.importing = false;
        match result {
            Ok(imported) => {
                let fields = self.field_context(cx);
                self.editor.update(cx, |editor, cx| {
                    editor.load(imported.document, fields, true, cx)
                });
                self.editor_dirty = false;
                if self.title.read(cx).text().trim().is_empty()
                    && let Some(name) = imported.name
                {
                    self.title.update(cx, |input, cx| input.set_text(name, cx));
                }
                self.renaming = self.id.is_none();
                self.review = Some(ImportReview {
                    file_name: imported.file_name,
                    field_count: imported.field_count,
                    unrecognised: imported.unrecognised,
                    notices: imported.notices,
                });
            }
            Err(message) => toast_detailed(
                window,
                cx,
                ToastKind::Error,
                "Could not import Word document",
                message,
            ),
        }
        cx.notify();
    }

    pub fn is_reviewing(&self) -> bool {
        self.review.is_some()
    }

    pub fn discard_draft(&mut self, cx: &mut Context<Self>) {
        self.autosave.discard(cx);
    }

    fn discard_recovered(&mut self, cx: &mut Context<Self>) {
        if self.state != PageState::Ready || self.saving || self.review.is_some() {
            return;
        }
        self.editor.update(cx, |editor, cx| editor.discard(cx));
        self.editor_dirty = false;
        self.autosave.discard(cx);
        cx.notify();
    }

    pub fn discard_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.review.take().is_none() {
            return;
        }
        self.autosave.discard(cx);
        self.editor_dirty = false;
        if self.id.is_none() {
            self.title.update(cx, |input, cx| input.set_text("", cx));
        }
        self.load(window, cx);
        cx.notify();
    }

    pub fn is_renaming(&self) -> bool {
        self.renaming && self.id.is_some()
    }

    pub fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state != PageState::Ready || self.id.is_none() || self.importing || self.saving {
            return;
        }
        self.renaming = true;
        self.title.update(cx, |input, cx| {
            input.focus(window, cx);
            let length = input.text().len();
            input.select(0..length, cx);
        });
        cx.notify();
    }

    pub fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_renaming() {
            return;
        }
        self.renaming = false;
        let initial = self.initial_name.clone();
        self.title
            .update(cx, |input, cx| input.set_text(initial, cx));
        editor::focus(&self.editor, window, cx);
        cx.notify();
    }

    fn commit_rename(&mut self, submitted: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.id.is_none() {
            if submitted {
                editor::focus(&self.editor, window, cx);
            }
            return;
        }
        if !self.renaming || self.saving {
            return;
        }
        self.renaming = false;
        let name = self.title.read(cx).text().trim().to_owned();
        if name.is_empty() || name == self.initial_name {
            let initial = self.initial_name.clone();
            self.title
                .update(cx, |input, cx| input.set_text(initial, cx));
        } else {
            self.save(window, cx);
        }
        if submitted {
            editor::focus(&self.editor, window, cx);
        }
        cx.notify();
    }

    pub fn duplicate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state != PageState::Ready || self.id.is_none() || self.saving {
            return;
        }
        let name = format!("{} کاپی", self.title.read(cx).text().trim());
        let document = self.editor.read(cx).snapshot().0;
        run_db_in(
            window,
            cx,
            move |store| store.create_template(&name, &document),
            move |this, result, window, cx| match result {
                Ok(summary) => {
                    let id = summary.id;
                    this.models
                        .templates
                        .update(cx, |templates, cx| templates.upsert(summary, cx));
                    navigate(Route::Template(id), window, cx);
                }
                Err(error) => toast_error(window, cx, error.message()),
            },
        );
    }

    pub fn publish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(id), Some(library)) = (self.id, LibraryModel::get(cx)) else {
            return;
        };
        if self.state != PageState::Ready || self.saving || self.review.is_some() {
            return;
        }
        let publish: Continuation = Box::new(move |window, cx| {
            library.update(cx, |library, cx| library.publish(id, window, cx));
        });
        if self.is_dirty(cx) {
            self.save_then(Some(publish), window, cx);
        } else {
            publish(window, cx);
        }
    }

    fn dropped(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = paths.paths().first().cloned() else {
            return;
        };
        self.choose_import(path, window, cx);
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_then(None, window, cx);
    }

    pub fn save_then(
        &mut self,
        then: Option<Continuation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = self.title.read(cx).text().trim().to_owned();
        if self.saving || name.is_empty() || self.state != PageState::Ready {
            return;
        }
        if self.id.is_some() && !self.is_dirty(cx) {
            return;
        }
        let (document, revision_token) = self.editor.read(cx).snapshot();
        self.saving = true;
        cx.notify();
        let id = self.id;
        let revision = self.revision;
        let job_name = name.clone();
        run_db_in(
            window,
            cx,
            move |store| match (id, revision) {
                (Some(id), Some(revision)) => {
                    let ack = store.save_template(id, revision, &job_name, &document)?;
                    Ok((None, Some(ack)))
                }
                _ => Ok((Some(store.create_template(&job_name, &document)?), None)),
            },
            move |this, result, window, cx| {
                this.saving = false;
                match result {
                    Ok((Some(summary), _)) => {
                        let new_id = summary.id;
                        this.models
                            .templates
                            .update(cx, |templates, cx| templates.upsert(summary, cx));
                        this.editor
                            .update(cx, |editor, cx| editor.mark_saved(revision_token, cx));
                        this.initial_name = name.clone();
                        this.review = None;
                        navigate_replace(Route::Template(new_id), cx);
                    }
                    Ok((None, Some(ack))) => {
                        this.revision = Some(ack.revision);
                        this.initial_name = name.clone();
                        this.editor
                            .update(cx, |editor, cx| editor.mark_saved(revision_token, cx));
                        this.editor_dirty = this.editor.read(cx).is_dirty();
                        this.autosave.saved(ack.revision);
                        if this.editor_dirty {
                            autosave::edited(this, cx);
                        } else {
                            this.autosave.cancel();
                        }
                        let created_at = this.id.and_then(|id| {
                            this.models
                                .templates
                                .read(cx)
                                .get(id)
                                .map(|item| item.created_at)
                        });
                        if let (Some(id), Some(created_at)) = (this.id, created_at) {
                            let summary = TemplateSummary {
                                id,
                                name: name.clone(),
                                revision: ack.revision,
                                preview_text: ack.preview_text,
                                field_count: ack.field_count,
                                created_at,
                                updated_at: ack.updated_at,
                            };
                            this.models
                                .templates
                                .update(cx, |templates, cx| templates.upsert(summary, cx));
                        }
                        this.review = None;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        autosave::resume(this, cx);
                        toast_error(window, cx, error.message());
                        cx.notify();
                        return;
                    }
                }
                cx.notify();
                if let Some(then) = then {
                    then(window, cx);
                }
            },
        );
    }

    fn request_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.id else {
            return;
        };
        let name = self.title.read(cx).text().trim().to_owned();
        let weak = cx.entity().downgrade();
        confirm_delete(
            window,
            cx,
            format!("Delete «{}»?", isolate_urdu(&name)),
            "This can't be undone. Templates that an FIR's documents use can't be deleted.",
            "Delete template",
            move |window, cx| {
                weak.update(cx, |this, cx| this.delete(id, window, cx)).ok();
            },
        );
    }

    fn delete(&mut self, id: TemplateId, window: &mut Window, cx: &mut Context<Self>) {
        run_db_in(
            window,
            cx,
            move |store| store.remove_template(id),
            move |this, result, window, cx| match result {
                Ok(()) => {
                    this.models
                        .templates
                        .update(cx, |templates, cx| templates.remove(id, cx));
                    this.initial_name = this.title.read(cx).text().to_owned();
                    this.review = None;
                    navigate_replace(Route::Templates, cx);
                    toast_success(window, cx, "Template deleted");
                }
                Err(error) => toast_error(window, cx, error.message()),
            },
        );
    }

    fn title_field(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        div()
            .id("template-title")
            .occlude()
            .w(px(280.))
            .on_action(cx.listener(|this, _: &BlurFocus, window, cx| {
                if this.is_renaming() {
                    this.cancel_rename(window, cx);
                } else {
                    cx.propagate();
                }
            }))
            .child(field("template-title-field", &self.title, Kind::Urdu).width(px(280.)))
            .into_any_element()
    }

    fn name_pill(&self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let palette = *Palette::get(cx);
        if self.state != PageState::Ready {
            return None;
        }
        if self.id.is_none() || self.renaming {
            return Some(self.title_field(cx));
        }
        let name = self.title.read(cx).text().trim().to_owned();
        let shown = if name.is_empty() {
            "Untitled template".to_owned()
        } else {
            name
        };
        Some(
            div()
                .id("template-name")
                .max_w(px(280.))
                .h(px(32.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded(px(crate::theme::RADIUS_LG))
                .hover(move |style| style.bg(palette.hover))
                .active(move |s| s.bg(palette.pressed))
                .on_click(cx.listener(|this, _, window, cx| this.begin_rename(window, cx)))
                .child(
                    label(shown)
                        .text_size(px(18.))
                        .text_color(palette.ink)
                        .rtl()
                        .leading(missal_text::LineHeight::Px(30.))
                        .truncate(),
                )
                .into_any_element(),
        )
    }

    pub fn header(&mut self, cx: &mut Context<Self>) -> HeaderContent {
        if self.state != PageState::Ready {
            return HeaderContent::new("Template");
        }
        let name = self.title.read(cx).text().trim().to_owned();
        let is_new = self.id.is_none();
        let dirty = self.is_dirty(cx);
        let weak = cx.entity().downgrade();
        if let Some(review) = &self.review {
            let summary = match review.unrecognised.len() {
                0 => format!("{} fields found", review.field_count),
                count => format!(
                    "{} fields found, {count} not recognised",
                    review.field_count
                ),
            };
            let file = if review.file_name.is_ascii() {
                review.file_name.clone()
            } else {
                match review.file_name.rsplit_once('.') {
                    Some((stem, extension)) => format!("{}.{extension}", isolate_urdu(stem)),
                    None => isolate_urdu(&review.file_name),
                }
            };
            let save_enabled = !name.is_empty() && !self.saving && !self.importing;
            return HeaderContent::element(
                div()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .whitespace_nowrap()
                    .child(label(format!("Import: {file}")).ltr().truncate()),
            )
            .meta(summary)
            .action(
                ghost_sm("template-discard", "Discard")
                    .on_click(cx.listener(|this, _, window, cx| this.discard_import(window, cx))),
            )
            .action(
                primary_sm("template-save", "Save template")
                    .disabled(!save_enabled)
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
            );
        }
        let mut content = HeaderContent::new(if is_new { "New template" } else { "Template" });
        if is_new {
            let save_enabled = !name.is_empty() && !self.saving && !self.importing;
            content = content
                .action(
                    icon_button("template-print", IconName::Printer, "Preview")
                        .keys("Mod+P")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_print_preview(window, cx)),
                        ),
                )
                .action(
                    primary_sm("template-save", "Create template")
                        .keys("Mod+S")
                        .disabled(!save_enabled)
                        .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                );
            return content;
        }
        let state = if self.saving {
            SaveState::Saving
        } else if dirty {
            SaveState::Unsaved
        } else {
            SaveState::Saved
        };
        content
            .action(save_state(state))
            .action(
                icon_button("template-print", IconName::Printer, "Preview")
                    .keys("Mod+P")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_print_preview(window, cx)),
                    ),
            )
            .action(
                menu("template-more")
                    .trigger(icon_button(
                        "template-more",
                        IconName::Ellipsis,
                        "More actions",
                    ))
                    .content(move |menu, _, cx| {
                        let template = weak.upgrade().and_then(|page| page.read(cx).id);
                        let library = LibraryModel::get(cx)
                            .map(|library| library.read(cx))
                            .filter(|library| library.can_publish(cx));
                        let publishing = match (library, template) {
                            (Some(library), Some(id)) if library.is_publishing(id) => {
                                Some(("Publishing to library…", false))
                            }
                            (Some(library), Some(id)) => match library.link_for_template(id) {
                                None => Some(("Publish to library", true)),
                                Some(_) if library.has_unpublished_changes(id, cx) => {
                                    Some(("Publish changes to library", true))
                                }
                                Some(_) => Some(("Published to library", false)),
                            },
                            _ => None,
                        };
                        let action = |id: &'static str,
                                      text: &'static str,
                                      icon: IconName,
                                      run: fn(
                            &mut TemplatePage,
                            &mut Window,
                            &mut Context<TemplatePage>,
                        )| {
                            let weak = weak.clone();
                            MenuEntry::new(id, text, move |window, cx| {
                                weak.update(cx, |this, cx| run(this, window, cx)).ok();
                            })
                            .icon(icon)
                        };
                        menu.when_some(publishing, |menu, (text, enabled)| {
                            menu.item(
                                action(
                                    "template-publish",
                                    text,
                                    IconName::CloudUpload,
                                    |this, window, cx| this.publish(window, cx),
                                )
                                .disabled(!enabled),
                            )
                            .separator()
                        })
                        .item(action(
                            "template-replace",
                            "Replace from Word file…",
                            IconName::FileUp,
                            |this, window, cx| this.request_import(window, cx),
                        ))
                        .item(action(
                            "template-duplicate",
                            "Duplicate",
                            IconName::Copy,
                            |this, window, cx| this.duplicate(window, cx),
                        ))
                        .separator()
                        .item(
                            action(
                                "template-delete",
                                "Delete",
                                IconName::Trash,
                                |this, window, cx| this.request_delete(window, cx),
                            )
                            .danger(),
                        )
                    }),
            )
    }

    fn review_notes(&self) -> Option<gpui_kit::AnyElement> {
        let review = self.review.as_ref()?;
        if review.unrecognised.is_empty() && review.notices.is_empty() {
            return None;
        }
        let mut lines: Vec<String> = Vec::new();
        if !review.unrecognised.is_empty() {
            let count = review.unrecognised.len();
            let shown: Vec<String> = review
                .unrecognised
                .iter()
                .take(4)
                .map(|token| isolate_urdu(token))
                .collect();
            let more = if count > shown.len() {
                format!(" and {} more", count - shown.len())
            } else {
                String::new()
            };
            lines.push(format!(
                "{count} {} recognised: {}{more}. They will stay as plain text.",
                if count == 1 {
                    "marker wasn't"
                } else {
                    "markers weren't"
                },
                shown.join(", ")
            ));
        }
        lines.extend(review.notices.iter().map(|notice| (*notice).to_owned()));
        Some(
            div()
                .id("import-notes")
                .flex_shrink_0()
                .px_4()
                .pt_3()
                .pb_1()
                .child(alert(Intent::Warning).child(
                    v_flex().gap(px(2.)).children(
                        lines.into_iter().map(|line| {
                            label(line).ltr().leading(missal_text::LineHeight::Px(20.))
                        }),
                    ),
                ))
                .into_any_element(),
        )
    }
}

impl Autosaving for TemplatePage {
    fn autosave(&mut self) -> &mut Autosave {
        &mut self.autosave
    }

    fn autosave_ready(&self, _: &App) -> bool {
        self.state == PageState::Ready && !self.saving && !self.importing
    }

    fn autosave_editor(&self) -> &Entity<Editor> {
        &self.editor
    }
}

impl Render for TemplatePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        match &self.state {
            PageState::NotFound => {
                return empty_state(IconName::FileText, "Template not found", None, None)
                    .into_any_element();
            }
            PageState::Failed(message) => {
                return empty_state(
                    IconName::TriangleAlert,
                    "Could not load this template",
                    Some(message.clone().into()),
                    None,
                )
                .into_any_element();
            }
            _ => {}
        }
        v_flex()
            .size_full()
            .bg(palette.sunken)
            .children(self.review_notes())
            .child(
                div()
                    .id("template-editor")
                    .flex_1()
                    .min_h_0()
                    .drag_over::<ExternalPaths>(|style, _, _, cx| {
                        style.bg(Palette::get(cx).accent_wash)
                    })
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                        this.dropped(paths, window, cx)
                    }))
                    .child(self.editor.clone()),
            )
            .into_any_element()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TemplateSort {
    Updated,
    Created,
    Name,
    Fields,
}

impl TemplateSort {
    const ALL: [TemplateSort; 4] = [
        TemplateSort::Updated,
        TemplateSort::Created,
        TemplateSort::Name,
        TemplateSort::Fields,
    ];

    fn title(self) -> &'static str {
        match self {
            TemplateSort::Updated => "Recently updated",
            TemplateSort::Created => "Recently created",
            TemplateSort::Name => "Name",
            TemplateSort::Fields => "Most fields",
        }
    }
}

pub struct TemplatesScreen {
    sort: TemplateSort,
    menu: Option<Entity<PopupList>>,
    sort_bounds: Rc<Cell<Bounds<Pixels>>>,
    models: Models,
    focus: FocusHandle,
    search: Entity<TextInput>,
    query: String,
    route: Route,
    page: Option<Entity<TemplatePage>>,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

fn match_ranges(name: &str, words: &[String]) -> Vec<Range<usize>> {
    let folded = fold_for_search(name);
    if folded.len() != name.len() {
        return Vec::new();
    }
    words
        .iter()
        .filter_map(|word| {
            let start = folded.find(word.as_str())?;
            let end = start + word.len();
            (name.is_char_boundary(start) && name.is_char_boundary(end)).then_some(start..end)
        })
        .collect()
}

fn name_label(palette: &Palette, name: &str, ranges: Vec<Range<usize>>) -> Label {
    let style = TextStyle {
        family: FontFamily::Interface,
        size: 16.0,
        weight: 400,
        italic: false,
        underline: false,
        strikethrough: false,
        color: to_rgba(palette.ink),
        background: None,
    };
    let mut text = AttributedText::new(name, style);
    let wash = to_rgba(palette.notice.opacity(0.3));
    for range in ranges {
        text.push(range, Attr::Background(wash));
    }
    Label::rich(text)
        .text_size(px(16.))
        .text_color(palette.ink)
        .rtl()
        .leading(missal_text::LineHeight::Px(30.))
}

impl TemplatesScreen {
    pub fn new(
        models: Models,
        route: Route,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TemplatesScreen {
        let search = field_input(window, cx, Kind::SearchRtl, "تلاش", "");
        let subscriptions = vec![
            cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                if *event == TextInputEvent::Change {
                    this.query = input.read(cx).text().to_owned();
                    cx.notify();
                }
            }),
            cx.subscribe(&models.templates, |_, _, _: &TemplatesEvent, cx| {
                cx.notify()
            }),
        ];
        let mut screen = TemplatesScreen {
            sort: TemplateSort::Updated,
            menu: None,
            sort_bounds: Rc::new(Cell::new(Bounds::default())),
            models,
            focus: cx.focus_handle(),
            search,
            query: String::new(),
            route,
            page: None,
            scroll: UniformListScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        screen.build_page(window, cx);
        screen
    }

    fn build_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = match self.route {
            Route::NewTemplate => {
                Some(cx.new(|cx| TemplatePage::new(self.models.clone(), None, window, cx)))
            }
            Route::Template(id) => {
                Some(cx.new(|cx| TemplatePage::new(self.models.clone(), Some(id), window, cx)))
            }
            _ => None,
        };
        if let Some(page) = &self.page {
            cx.observe(page, |_, _, cx| cx.notify()).detach();
        }
        if let Route::Template(id) = self.route {
            let visible = self.visible(cx);
            if let Some(position) = visible.iter().position(|item| item.id == id) {
                self.scroll.scroll_to_item(position, ScrollStrategy::Center);
            }
        }
    }

    pub fn route_changed(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.route = route;
        self.build_page(window, cx);
        self.focus(window, cx);
        cx.notify();
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.page
            .as_ref()
            .is_some_and(|page| page.read(cx).is_dirty(cx))
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        match (&self.page, self.route) {
            (Some(page), Route::NewTemplate) => {
                let title = page.read(cx).title.clone();
                title.update(cx, |input, cx| input.focus(window, cx));
            }
            _ => window.focus(&self.focus, cx),
        }
    }

    pub fn save(&mut self, window: &mut Window, then: Continuation, cx: &mut Context<Self>) {
        match &self.page {
            Some(page) => page.update(cx, |page, cx| page.save_then(Some(then), window, cx)),
            None => then(window, cx),
        }
    }

    pub fn discard_draft(&mut self, cx: &mut Context<Self>) {
        if let Some(page) = &self.page {
            page.update(cx, |page, cx| page.discard_draft(cx));
        }
    }

    pub fn page(&self) -> Option<&Entity<TemplatePage>> {
        self.page.as_ref()
    }

    pub fn search_input(&self) -> &Entity<TextInput> {
        &self.search
    }

    fn query_words(&self) -> Vec<String> {
        fold_for_search(&self.query)
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }

    pub fn visible(&self, cx: &App) -> Vec<TemplateSummary> {
        let words = self.query_words();
        let mut items: Vec<TemplateSummary> = self
            .models
            .templates
            .read(cx)
            .items
            .iter()
            .filter(|item| {
                words.is_empty() || {
                    let haystack = fold_for_search(&format!("{} {}", item.name, item.preview_text));
                    words.iter().all(|word| haystack.contains(word.as_str()))
                }
            })
            .cloned()
            .collect();
        match self.sort {
            TemplateSort::Updated => items.sort_by_key(|item| std::cmp::Reverse(item.updated_at)),
            TemplateSort::Created => items.sort_by_key(|item| std::cmp::Reverse(item.created_at)),
            TemplateSort::Name => items.sort_by_cached_key(|item| fold_for_search(&item.name)),
            TemplateSort::Fields => items.sort_by_key(|item| std::cmp::Reverse(item.field_count)),
        }
        items
    }

    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        if visible.is_empty() {
            return;
        }
        let current = match self.route {
            Route::Template(id) => visible.iter().position(|item| item.id == id),
            _ => None,
        };
        let next = match current {
            None => {
                if delta >= 0 {
                    0
                } else {
                    visible.len() - 1
                }
            }
            Some(position) => {
                (position as isize + delta).clamp(0, visible.len() as isize - 1) as usize
            }
        };
        if Some(next) != current {
            self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
            navigate(Route::Template(visible[next].id), window, cx);
        }
    }

    fn open_sort_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let current = self.sort;
        let rows = MenuContent::new().items(TemplateSort::ALL.into_iter().map(|sort| {
            let weak = weak.clone();
            MenuEntry::new(
                SharedString::from(format!("sort-{}", sort as usize)),
                sort.title(),
                move |_, cx| {
                    weak.update(cx, |this, cx| {
                        this.sort = sort;
                        cx.notify();
                    })
                    .ok();
                },
            )
            .check(sort == current)
        }));
        let bounds = self.sort_bounds.get();
        let menu = PopupList::open(
            "template-sort-menu",
            rows,
            Placement {
                position: point(bounds.left(), bounds.bottom() + px(4.)),
                anchor: Anchor::TopLeft,
                width: 200.,
            },
            window,
            cx,
        );
        cx.subscribe_in(&menu, window, |this, _, _: &Dismissed, window, cx| {
            if this.menu.take().is_some() {
                this.focus(window, cx);
                cx.notify();
            }
        })
        .detach();
        self.menu = Some(menu);
        cx.notify();
    }

    fn list_header(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
        let palette = *Palette::get(cx);
        let total = self.models.templates.read(cx).items.len();
        let library = LibraryModel::get(cx).is_some_and(|library| library.read(cx).available(cx));
        let sort_bounds = self.sort_bounds.clone();
        let open = self.menu.is_some();
        h_flex()
            .flex_shrink_0()
            .h(px(44.))
            .items_center()
            .gap(px(2.))
            .pl(px(8.))
            .pr(px(8.))
            .border_b_1()
            .border_color(palette.line)
            .mb(px(8.))
            .child(section_label("Templates"))
            .child(
                div()
                    .ml(px(4.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_faint)
                    .child(total.to_string()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(
                        canvas(move |bounds, _, _| sort_bounds.set(bounds), |_, _, _, _| {})
                            .absolute()
                            .size_full(),
                    )
                    .child(
                        icon_button("template-sort", IconName::ArrowUpDown, "Sort")
                            .toggled(open)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_sort_menu(window, cx)),
                            ),
                    ),
            )
            .children(library.then(|| {
                icon_button("template-library", IconName::Library, "Template library")
                    .on_click(|_, window, cx| open_library(window, cx))
            }))
            .child(
                icon_button("template-import", IconName::FileUp, "Import Word file")
                    .keys("Mod+O")
                    .on_click(|_, window, cx| import_word_file(window, cx)),
            )
            .child(
                icon_button("new-template", IconName::Plus, "New template")
                    .keys("Shift+N")
                    .on_click(|_, window, cx| navigate(Route::NewTemplate, window, cx)),
            )
    }

    fn rows(
        &self,
        palette: Palette,
        visible: Vec<TemplateSummary>,
        active: Option<TemplateId>,
    ) -> gpui_kit::AnyElement {
        let words = self.query_words();
        let items = Rc::new(visible);
        uniform_list("template-rail", items.len(), {
            let items = items.clone();
            move |range, _, _| {
                range
                    .filter_map(|index| {
                        let item = items.get(index)?.clone();
                        let is_active = active == Some(item.id);
                        let id = item.id;
                        let ranges = match_ranges(&item.name, &words);
                        let fields = if item.field_count == 1 {
                            "1 field".to_owned()
                        } else {
                            format!("{} fields", item.field_count)
                        };
                        Some(
                            div().h(px(ROW_HEIGHT)).px(px(8.)).child(
                                h_flex()
                                    .id(("template-item", index))
                                    .group("template-item")
                                    .test_support()
                                    .size_full()
                                    .flex_row_reverse()
                                    .items_center()
                                    .gap(px(10.))
                                    .px(px(10.))
                                    .rounded(px(crate::theme::RADIUS_LG))
                                    .when(is_active, |this| {
                                        this.bg(palette.hover).font_weight(FontWeight::MEDIUM)
                                    })
                                    .when(!is_active, |this| {
                                        this.hover(move |style| style.bg(palette.hover))
                                            .active(move |s| s.bg(palette.pressed))
                                    })
                                    .on_click(move |_, window, cx| {
                                        navigate(Route::Template(id), window, cx)
                                    })
                                    .child(
                                        div().flex_1().min_w_0().mt(px(-2.)).child(
                                            name_label(&palette, &item.name, ranges).truncate(),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_size(px(12.))
                                            .line_height(px(16.))
                                            .text_color(palette.ink_faint)
                                            .font_features(crate::ui::tabular())
                                            .child(fields),
                                    ),
                            ),
                        )
                    })
                    .collect::<Vec<_>>()
            }
        })
        .track_scroll(&self.scroll)
        .flex_1()
        .min_h_0()
        .into_any_element()
    }

    fn list(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let palette = *Palette::get(cx);
        let load = self.models.templates.read(cx).load.clone();
        let visible = self.visible(cx);
        let total = self.models.templates.read(cx).items.len();
        let filtering = !self.query_words().is_empty();
        let active = match self.route {
            Route::Template(id) => Some(id),
            _ => None,
        };
        let body: gpui_kit::AnyElement = match load {
            Load::Loading => v_flex()
                .gap(px(4.))
                .px(px(16.))
                .pt(px(8.))
                .children((0..6usize).map(|index| {
                    div()
                        .id(("template-skeleton", index))
                        .h(px(ROW_HEIGHT - 12.))
                        .rounded(px(crate::theme::RADIUS_LG))
                        .bg(palette.hover)
                }))
                .into_any_element(),
            Load::Failed(message) => empty_state(
                IconName::TriangleAlert,
                "Could not load templates",
                Some(message.into()),
                None,
            )
            .into_any_element(),
            Load::Ready if visible.is_empty() => div()
                .flex_1()
                .pt(px(48.))
                .child(
                    v_flex().items_center().child(empty_state_body(
                        IconName::Search,
                        format!("No templates match “{}”", isolate_urdu(self.query.trim())),
                        None,
                        Some(
                            ghost_sm("clear-template-search", "Clear search")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.search.update(cx, |input, cx| input.set_text("", cx));
                                }))
                                .into_any_element(),
                        ),
                    )),
                )
                .into_any_element(),
            Load::Ready => self.rows(palette, visible.clone(), active),
        };
        let count_line = (filtering && matches!(self.models.templates.read(cx).load, Load::Ready))
            .then(|| {
                div()
                    .flex_shrink_0()
                    .px(px(18.))
                    .pt(px(2.))
                    .pb(px(6.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_faint)
                    .child(format!("{} of {}", visible.len(), total))
            });
        v_flex()
            .id("template-rail-panel")
            .w(px(LIST_WIDTH))
            .flex_shrink_0()
            .h_full()
            .child(self.list_header(cx))
            .child(
                div()
                    .id("template-search")
                    .flex_shrink_0()
                    .mx(px(12.))
                    .mb(px(8.))
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.as_str();
                        let plain = !event.keystroke.modifiers.modified();
                        if key == "escape" && plain {
                            if this.query.is_empty() {
                                this.focus(window, cx);
                            } else {
                                this.search.update(cx, |input, cx| input.set_text("", cx));
                            }
                            cx.stop_propagation();
                        } else if key == "enter" && plain {
                            if let Some(first) = this.visible(cx).first() {
                                navigate(Route::Template(first.id), window, cx);
                            }
                            cx.stop_propagation();
                        } else if key == "down" && plain {
                            this.focus(window, cx);
                            this.step(1, window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(search_field(
                        "template-search-field",
                        &self.search,
                        Kind::SearchRtl,
                    )),
            )
            .children(count_line)
            .child(body)
            .into_any_element()
    }

    fn rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = &self.page {
            page.update(cx, |page, cx| page.begin_rename(window, cx));
        }
    }
}

impl Render for TemplatesScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let templates = self.models.templates.read(cx);
        let nothing_at_all =
            templates.load.is_ready() && templates.items.is_empty() && self.page.is_none();
        let library = LibraryModel::get(cx).is_some_and(|library| library.read(cx).available(cx));
        let main: gpui_kit::AnyElement = match &self.page {
            Some(page) => page.clone().into_any_element(),
            None => empty_state(
                IconName::FileText,
                "No template open",
                Some("Pick one from the list, or start a new one.".into()),
                Some(
                    h_flex()
                        .gap(px(8.))
                        .child(
                            primary_sm("open-new-template", "New template")
                                .icon(IconName::Plus)
                                .keys("Shift+N")
                                .on_click(|_, window, cx| navigate(Route::NewTemplate, window, cx)),
                        )
                        .child(
                            ghost_sm("open-import-template", "Import Word file")
                                .icon(IconName::FileUp)
                                .on_click(|_, window, cx| import_word_file(window, cx)),
                        )
                        .into_any_element(),
                ),
            )
            .into_any_element(),
        };
        let body = if nothing_at_all {
            empty_state(
                IconName::FileText,
                "No templates yet",
                Some("Import a Word document to make your first template.".into()),
                Some(
                    h_flex()
                        .gap(px(8.))
                        .child(
                            primary_sm("empty-import-template", "Import Word file")
                                .icon(IconName::FileUp)
                                .on_click(|_, window, cx| import_word_file(window, cx)),
                        )
                        .children(library.then(|| {
                            ghost_sm("empty-template-library", "Browse library")
                                .icon(IconName::Library)
                                .on_click(|_, window, cx| open_library(window, cx))
                        }))
                        .child(
                            ghost_sm("empty-new-template", "New blank template")
                                .on_click(|_, window, cx| navigate(Route::NewTemplate, window, cx)),
                        )
                        .into_any_element(),
                ),
            )
            .into_any_element()
        } else {
            let list = self.list(cx);
            h_flex()
                .size_full()
                .rounded(px(crate::theme::RADIUS_XL))
                .overflow_hidden()
                .bg(palette.surface)
                .shadow(palette.shadow_xs())
                .child(list)
                .child(crate::ui::pane_divider())
                .child(div().flex_1().min_w_0().h_full().child(main))
                .into_any_element()
        };
        let body = if nothing_at_all {
            crate::ui::card().size_full().child(body).into_any_element()
        } else {
            body
        };
        div()
            .id("templates")
            .key_context(TEMPLATES)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &NextTemplate, window, cx| this.step(1, window, cx)))
            .on_action(
                cx.listener(|this, _: &PreviousTemplate, window, cx| this.step(-1, window, cx)),
            )
            .on_action(cx.listener(|this, _: &RenameTemplate, window, cx| this.rename(window, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |input, cx| {
                    input.focus(window, cx);
                    let length = input.text().len();
                    input.select(0..length, cx);
                });
            }))
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                if let Some(page) = &this.page {
                    page.update(cx, |page, cx| page.save(window, cx));
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "enter"
                    && !event.keystroke.modifiers.modified()
                    && this.focus.is_focused(window)
                    && let Some(page) = &this.page
                {
                    let editor = page.read(cx).editor.clone();
                    editor::focus(&editor, window, cx);
                }
            }))
            .child(body)
            .children(self.menu.clone().map(|menu| menu.into_any_element()))
    }
}

impl crate::ui::titlebar::HeaderSource for TemplatesScreen {
    fn header(&mut self, _: &mut Window, cx: &mut Context<Self>) -> HeaderContent {
        match &self.page {
            Some(page) => page.update(cx, |page, cx| page.header(cx)),
            None => HeaderContent::new("Templates"),
        }
    }
}
