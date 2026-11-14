use std::ops::Range;
use std::rc::Rc;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::{StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;

use gpui_kit::{
    App, AppContext, Context, Entity, ExternalPaths, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollStrategy, StatefulInteractiveElement, Styled,
    Subscription, UniformListScrollHandle, Window, div, px, uniform_list,
};
use missal_core::{DisplayMode, Document, Revision, TemplateId};
use missal_store::TemplateSummary;
use missal_text::{
    Attr, AttributedText, FontFamily, Label, TextInput, TextInputEvent, TextStyle, label, to_rgba,
};

use crate::app::{import_word_file, navigate, navigate_replace, open_print_preview};
use crate::editor::{DocumentEditor, EditorEvent, EditorFactory, FieldContext, LoadRequest};
use crate::shortcuts::*;
use crate::state::rows::fold_for_search;
use crate::state::templates::TemplatesEvent;
use crate::state::{
    Load, Models, Route, ToastKind, run_db_in, toast_detailed, toast_error, toast_success,
};
use crate::theme::{Palette, RADIUS};
use crate::ui::dialogs::{Choice, Tone, confirm_delete, open_choice};
use crate::ui::input::{Kind, field, field_input, search_field};
use crate::ui::print_preview::{PreviewDocument, PrintRequest};
use crate::ui::titlebar::HeaderContent;
use crate::ui::{
    SaveState, empty_state, empty_state_body, ghost_sm, icon_button, isolate_urdu, kit_icon_button,
    primary_sm, save_state, warn_box,
};
use crate::word::{WordImport, is_docx};

const LIST_WIDTH: f32 = 320.0;
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
    editor: Rc<dyn DocumentEditor>,
    editor_dirty: bool,
    saving: bool,
    picking: bool,
    importing: bool,
    imports: u64,
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
        let editor = EditorFactory::create(cx).build(window, cx);
        let weak = cx.entity().downgrade();
        let editor_subscription = editor.on_event(cx, {
            let weak = weak.clone();
            Box::new(move |event, cx| {
                let event = event.clone();
                weak.update(cx, |this, cx| {
                    if let EditorEvent::DirtyChanged(dirty) = event {
                        this.editor_dirty = dirty;
                        cx.notify();
                    }
                })
                .ok();
            })
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
            picking: false,
            importing: false,
            imports: 0,
            renaming: id.is_none(),
            review: None,
            _subscriptions: vec![editor_subscription, title_subscription],
        };
        page.load(window, cx);
        page
    }

    fn field_context(&self, cx: &App) -> FieldContext {
        let catalog = self.models.catalog.read(cx);
        FieldContext::labels(
            catalog.field_catalog(),
            catalog.settings.field_markers.clone(),
        )
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.id {
            None => {
                self.state = PageState::Ready;
                let request = LoadRequest {
                    key: format!("template:new:{}", self.imports),
                    document: Document::empty(),
                    fields: self.field_context(cx),
                    dirty: false,
                };
                self.editor.load(request, window, cx);
                self.title.update(cx, |input, cx| input.focus(window, cx));
            }
            Some(id) => {
                run_db_in(
                    window,
                    cx,
                    move |store| store.template(id),
                    move |this, result, window, cx| {
                        match result {
                            Ok(record) => {
                                this.revision = Some(record.revision);
                                this.initial_name = record.name.clone();
                                this.title.update(cx, |input, cx| {
                                    input.set_text(record.name.clone(), cx)
                                });
                                let request = LoadRequest {
                                    key: format!(
                                        "template:{}:{}:{}",
                                        record.id, record.revision, this.imports
                                    ),
                                    document: record.document,
                                    fields: this.field_context(cx),
                                    dirty: false,
                                };
                                this.editor.load(request, window, cx);
                                this.state = PageState::Ready;
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
                || self.editor.is_dirty(cx))
    }

    pub fn title_input(&self) -> &Entity<TextInput> {
        &self.title
    }

    pub fn editor(&self) -> &Rc<dyn DocumentEditor> {
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
            catalog: (*self.models.catalog.read(cx).field_catalog()).clone(),
            values: None,
            mode: DisplayMode::Labels,
            documents: vec![PreviewDocument::Content(Box::new(
                self.editor.snapshot(cx).document,
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
        let projections = self.editor.snapshot(cx).document.projections();
        let has_content = self.id.is_some()
            || self.editor_dirty
            || self.editor.is_dirty(cx)
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
        let fields = (*catalog.field_catalog()).clone();
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
                self.imports += 1;
                let key = match self.id {
                    Some(id) => format!("template:{id}:import:{}", self.imports),
                    None => format!("template:new:import:{}", self.imports),
                };
                let request = LoadRequest {
                    key,
                    document: imported.document,
                    fields: self.field_context(cx),
                    dirty: true,
                };
                self.editor.load(request, window, cx);
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

    pub fn discard_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.review.take().is_none() {
            return;
        }
        self.imports += 1;
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
        self.editor.focus(window, cx);
        cx.notify();
    }

    fn commit_rename(&mut self, submitted: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.id.is_none() {
            if submitted {
                self.editor.focus(window, cx);
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
            self.editor.focus(window, cx);
        }
        cx.notify();
    }

    pub fn duplicate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state != PageState::Ready || self.id.is_none() || self.saving {
            return;
        }
        let name = format!("{} کاپی", self.title.read(cx).text().trim());
        let document = self.editor.snapshot(cx).document;
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

    fn dropped(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = paths.paths().first().cloned() else {
            return;
        };
        self.choose_import(path, window, cx);
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.title.read(cx).text().trim().to_owned();
        if self.saving || name.is_empty() || self.state != PageState::Ready {
            return;
        }
        if self.id.is_some() && !self.is_dirty(cx) {
            return;
        }
        let snapshot = self.editor.snapshot(cx);
        self.saving = true;
        cx.notify();
        let revision_token = snapshot.revision;
        let document = snapshot.document;
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
                        this.editor.mark_saved(revision_token, cx);
                        this.initial_name = name.clone();
                        this.review = None;
                        navigate_replace(Route::Template(new_id), cx);
                    }
                    Ok((None, Some(ack))) => {
                        this.revision = Some(ack.revision);
                        this.initial_name = name.clone();
                        this.editor.mark_saved(revision_token, cx);
                        let created_at = this.id.and_then(|id| {
                            this.models
                                .templates
                                .read(cx)
                                .get(id)
                                .map(|item| item.created_at.clone())
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
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
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
            .w(px(360.))
            .on_action(cx.listener(|this, _: &BlurFocus, window, cx| {
                if this.is_renaming() {
                    this.cancel_rename(window, cx);
                } else {
                    cx.propagate();
                }
            }))
            .child(field("template-title-field", &self.title, Kind::Urdu).width(px(360.)))
            .into_any_element()
    }

    pub fn header(&mut self, cx: &mut Context<Self>) -> HeaderContent {
        let palette = *Palette::get(cx);
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
                    .text_size(px(15.))
                    .line_height(px(20.))
                    .font_semibold()
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
        let mut content = if is_new || self.renaming {
            HeaderContent::element(self.title_field(cx))
        } else if name.is_empty() {
            HeaderContent::new("Untitled template")
        } else {
            HeaderContent::urdu(name.clone())
        };
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
            .action(save_state(&palette, state))
            .action(
                icon_button("template-rename", IconName::Pencil, "Rename")
                    .keys("F2")
                    .toggled(self.renaming)
                    .on_click(cx.listener(|this, _, window, cx| this.begin_rename(window, cx))),
            )
            .action(
                icon_button("template-print", IconName::Printer, "Preview")
                    .keys("Mod+P")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_print_preview(window, cx)),
                    ),
            )
            .action(
                kit_icon_button("template-more", IconName::Ellipsis)
                    .tooltip("More actions")
                    .dropdown_menu(move |menu, _, _| {
                        let replace = weak.clone();
                        let duplicate = weak.clone();
                        let delete = weak.clone();
                        menu.item(
                            PopupMenuItem::new("Replace from Word file…")
                                .icon(IconName::FileUp)
                                .on_click(move |_, window, cx| {
                                    replace
                                        .update(cx, |this, cx| this.request_import(window, cx))
                                        .ok();
                                }),
                        )
                        .item(
                            PopupMenuItem::new("Duplicate")
                                .icon(IconName::Copy)
                                .on_click(move |_, window, cx| {
                                    duplicate
                                        .update(cx, |this, cx| this.duplicate(window, cx))
                                        .ok();
                                }),
                        )
                        .item(PopupMenuItem::separator())
                        .item(
                            PopupMenuItem::new("Delete").icon(IconName::Trash).on_click(
                                move |_, window, cx| {
                                    delete
                                        .update(cx, |this, cx| this.request_delete(window, cx))
                                        .ok();
                                },
                            ),
                        )
                    }),
            )
    }

    fn review_notes(&self, palette: &Palette) -> Option<gpui_kit::AnyElement> {
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
                .child(warn_box(
                    palette,
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

impl Render for TemplatePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        match &self.state {
            PageState::NotFound => {
                return empty_state(
                    &palette,
                    IconName::FileText,
                    "Template not found",
                    None,
                    None,
                )
                .into_any_element();
            }
            PageState::Failed(message) => {
                return empty_state(
                    &palette,
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
            .children(self.review_notes(&palette))
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
                    .child(self.editor.view()),
            )
            .into_any_element()
    }
}

pub struct TemplatesScreen {
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
        family: FontFamily::Document,
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
        .document_font()
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
        self.models
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
            .collect()
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

    fn list_header(&self, palette: &Palette, total: usize) -> gpui_kit::Div {
        h_flex()
            .flex_shrink_0()
            .h(px(44.))
            .items_center()
            .gap(px(8.))
            .pl(px(16.))
            .pr(px(12.))
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .font_medium()
                    .child("Templates"),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(palette.ink_muted)
                    .child(total.to_string()),
            )
            .child(div().flex_1())
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
                                    .test_support()
                                    .size_full()
                                    .flex_row_reverse()
                                    .items_center()
                                    .gap(px(10.))
                                    .px(px(10.))
                                    .rounded(px(RADIUS))
                                    .cursor_pointer()
                                    .when(is_active, |this| this.bg(palette.accent_wash))
                                    .when(!is_active, |this| {
                                        this.hover(move |style| style.bg(palette.hover))
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
                                            .text_color(if is_active {
                                                palette.ink_muted
                                            } else {
                                                palette.ink_faint
                                            })
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
                        .rounded(px(RADIUS))
                        .bg(palette.hover)
                }))
                .into_any_element(),
            Load::Failed(message) => empty_state(
                &palette,
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
                        &palette,
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
            .border_r_1()
            .border_color(palette.line)
            .child(self.list_header(&palette, total))
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
        let main: gpui_kit::AnyElement = match &self.page {
            Some(page) => page.clone().into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(palette.ink_muted)
                        .child("Choose a template from the list."),
                )
                .into_any_element(),
        };
        let body = if nothing_at_all {
            empty_state(
                &palette,
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
                .child(list)
                .child(div().flex_1().min_w_0().h_full().child(main))
                .into_any_element()
        };
        div()
            .id("templates")
            .key_context(TEMPLATES)
            .track_focus(&self.focus)
            .size_full()
            .bg(palette.surface)
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
                    editor.focus(window, cx);
                }
            }))
            .child(body)
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
