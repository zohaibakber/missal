use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Selectable, h_flex, v_flex};
use gpui_kit::prelude::*;

use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Task, WeakEntity, Window, div, px,
};
use missal_core::{DisplayMode, FirDocumentId, FirId, FirValues, Revision, TemplateId};
use missal_store::{FirDocumentSummary, FirWorkspace};
use missal_text::{TextInput, TextInputEvent};

use crate::app::{navigate, navigate_replace, open_print_preview};
use crate::editor::{DocumentEditor, EditorEvent, EditorFactory, FieldContext, LoadRequest};
use crate::screens::fields_panel::{
    AUTO_HIDE_BELOW, FieldsPanel, FieldsPanelEvent, FieldsTab, take_pending_fields,
};
use crate::shortcuts::*;
use crate::state::catalog::CatalogEvent;
use crate::state::firs::FirsEvent;
use crate::state::rows::fold_for_search;
use crate::state::{Models, Route, run_db_in, toast_error, toast_success, toast_warning};
use crate::theme::{ICON_SM, Palette, RAIL_WIDTH};
use crate::ui::dialogs::{Choice, Tone, confirm_delete, open_choice};
use crate::ui::input::{Kind, field_input};
use crate::ui::menu::{menu_item, menu_panel, menu_separator};
use crate::ui::print_preview::{PreviewDocument, PrintRequest};
use crate::ui::{
    IconButton, SaveState, UrRole, button, checkbox, dirty_dot, empty_state, icon_button, keycap,
    save_state, section_label, sized_icon, skeleton_bar, ur_label,
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WorkspaceState {
    Loading,
    NotFound,
    Failed(String),
    Ready,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DocumentState {
    Idle,
    Loading,
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Menu {
    More,
    Document(FirDocumentId),
}

type Continuation =
    Box<dyn FnOnce(&mut WorkspaceScreen, &mut Window, &mut Context<WorkspaceScreen>)>;

#[derive(IntoElement)]
struct Trigger {
    button: IconButton,
    open: bool,
}

impl Selectable for Trigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for Trigger {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.button.toggled(self.open)
    }
}

#[derive(IntoElement)]
struct AddRow {
    open: bool,
}

impl Selectable for AddRow {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for AddRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let ink = palette.ink;
        h_flex()
            .id("add-templates-trigger")
            .h(px(32.))
            .mx(px(8.))
            .my(px(4.))
            .px(px(10.))
            .gap(px(8.))
            .items_center()
            .rounded(px(6.))
            .text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if self.open {
                palette.ink
            } else {
                palette.ink_muted
            })
            .cursor_pointer()
            .when(self.open, |this| this.bg(palette.pressed))
            .hover(move |s| s.bg(palette.hover).text_color(ink))
            .role(gpui_kit::accesskit::Role::Button)
            .aria_label("Add document")
            .child(sized_icon(IconName::Plus, ICON_SM, palette.ink_muted))
            .child(div().flex_1().child("Add document"))
            .child(keycap(&palette, "A"))
            .test_support()
    }
}

pub struct WorkspaceScreen {
    models: Models,
    id: FirId,
    focus: FocusHandle,
    rail_focus: FocusHandle,
    state: WorkspaceState,
    values: Option<Rc<FirValues>>,
    documents: Vec<FirDocumentSummary>,
    active: Option<FirDocumentId>,
    loaded: Option<(FirDocumentId, Revision)>,
    document_state: DocumentState,
    mode: DisplayMode,
    editor: Rc<dyn DocumentEditor>,
    editor_dirty: bool,
    saving: bool,
    slow: bool,
    add_open: bool,
    add_selected: HashSet<TemplateId>,
    add_highlight: Option<usize>,
    add_search: Entity<TextInput>,
    add_query: String,
    menu: Option<Menu>,
    panel: Entity<FieldsPanel>,
    panel_choice: Option<bool>,
    references_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceScreen {
    pub fn new(
        models: Models,
        id: FirId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> WorkspaceScreen {
        let editor = EditorFactory::create(cx).build(window, cx);
        editor.set_mode_toggle(true, cx);
        let weak = cx.entity().downgrade();
        let editor_subscription = editor.on_event(cx, {
            let weak = weak.clone();
            Box::new(move |event, cx| {
                let event = event.clone();
                weak.update(cx, |this, cx| match event {
                    EditorEvent::DirtyChanged(dirty) => {
                        this.editor_dirty = dirty;
                        this.refresh_references(cx);
                        cx.notify();
                    }
                    EditorEvent::ModeRequested(mode) => {
                        if this.mode != mode {
                            this.toggle_field_names(cx);
                        }
                    }
                    EditorEvent::SaveRequested => {}
                })
                .ok();
            })
        });
        let add_search = field_input(window, cx, Kind::SearchRtl, "تلاش", "");
        let panel = cx.new(|cx| FieldsPanel::new(models.clone(), id, window, cx));
        let mut subscriptions = vec![
            editor_subscription,
            cx.subscribe(&add_search, |this, input, event: &TextInputEvent, cx| {
                if *event == TextInputEvent::Change {
                    this.add_query = input.read(cx).text().to_owned();
                    this.add_highlight = None;
                    cx.notify();
                }
            }),
            cx.subscribe_in(
                &models.firs,
                window,
                |this, _, _: &FirsEvent, window, cx| {
                    if this.state == WorkspaceState::Ready {
                        this.refresh_values(window, cx);
                    }
                },
            ),
            cx.subscribe(&models.catalog, |this, catalog, _: &CatalogEvent, cx| {
                let shared = catalog.read(cx).settings.shared_placeholders.clone();
                if let Some(values) = this.values.as_ref()
                    && values.shared_settings != shared
                {
                    let mut updated = (**values).clone();
                    updated.shared_settings = shared;
                    this.set_values(Rc::new(updated), cx);
                } else {
                    this.push_fields(cx);
                }
                cx.notify();
            }),
            cx.observe(&models.templates, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &panel,
                window,
                |this, _, event: &FieldsPanelEvent, window, cx| match event {
                    FieldsPanelEvent::ValuesChanged(values) => {
                        this.set_values(values.clone(), cx);
                    }
                    FieldsPanelEvent::Reveal(reference, focus) => {
                        this.editor.reveal_field(reference, window, cx);
                        if !focus && let Some(input) = this.panel.read(cx).input_for(reference) {
                            input.update(cx, |input, cx| input.focus(window, cx));
                        }
                    }
                    FieldsPanelEvent::EditDetails => this.edit_details(window, cx),
                },
            ),
        ];
        if let Some(entity) = editor.editor_entity() {
            subscriptions.push(cx.observe(&entity, |this, _, cx| this.schedule_references(cx)));
        }
        let mut screen = WorkspaceScreen {
            models,
            id,
            focus: cx.focus_handle(),
            rail_focus: cx.focus_handle(),
            state: WorkspaceState::Loading,
            values: None,
            documents: Vec::new(),
            active: None,
            loaded: None,
            document_state: DocumentState::Idle,
            mode: DisplayMode::Values,
            editor,
            editor_dirty: false,
            saving: false,
            slow: false,
            add_open: false,
            add_selected: HashSet::new(),
            add_highlight: None,
            add_search,
            add_query: String::new(),
            menu: None,
            panel,
            panel_choice: None,
            references_task: None,
            _subscriptions: subscriptions,
        };
        screen.load_workspace(window, cx);
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            this.update(cx, |this, cx| {
                this.slow = true;
                cx.notify();
            })
            .ok();
        })
        .detach();
        screen
    }

    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if self.state == WorkspaceState::Ready {
            self.reload_order(window, cx);
            if crate::ui::print_preview::take_fill_request(self.id, cx) {
                self.show_fields(window, cx);
            }
            self.take_pending_fields(window, cx);
        }
    }

    fn reload_order(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fir = self.id;
        run_db_in(
            window,
            cx,
            move |store| store.fir_documents(fir),
            |this, result, _, cx| {
                if let Ok(documents) = result
                    && documents
                        .iter()
                        .map(|d| d.id)
                        .ne(this.documents.iter().map(|d| d.id))
                {
                    let mut documents = documents;
                    for document in &mut documents {
                        if let Some(current) = this.documents.iter().find(|d| d.id == document.id) {
                            *document = current.clone();
                        }
                    }
                    this.documents = documents;
                    cx.notify();
                }
            },
        );
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.editor_dirty || self.editor.is_dirty(cx)
    }

    pub fn state(&self) -> &WorkspaceState {
        &self.state
    }

    pub fn id(&self) -> FirId {
        self.id
    }

    pub fn documents(&self) -> &[FirDocumentSummary] {
        &self.documents
    }

    pub fn active(&self) -> Option<FirDocumentId> {
        self.active
    }

    pub fn editor(&self) -> &Rc<dyn DocumentEditor> {
        &self.editor
    }

    pub fn mode(&self) -> DisplayMode {
        self.mode
    }

    pub fn values(&self) -> Option<&Rc<FirValues>> {
        self.values.as_ref()
    }

    pub fn panel(&self) -> &Entity<FieldsPanel> {
        &self.panel
    }

    pub fn is_panel_open(&self, window: &Window) -> bool {
        self.panel_choice
            .unwrap_or_else(|| f32::from(window.viewport_size().width) >= AUTO_HIDE_BELOW)
    }

    pub fn toggle_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.is_panel_open(window);
        self.panel_choice = Some(!open);
        cx.notify();
    }

    pub fn show_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.panel_choice = Some(true);
        self.refresh_references(cx);
        self.panel
            .update(cx, |panel, cx| panel.focus_first_empty(window, cx));
        cx.notify();
    }

    pub fn take_pending_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state == WorkspaceState::Ready
            && self.document_state == DocumentState::Idle
            && take_pending_fields(self.id, cx)
        {
            self.show_fields(window, cx);
        }
    }

    fn field_context(&self, cx: &App) -> Option<FieldContext> {
        let values = self.values.clone()?;
        let catalog = self.models.catalog.read(cx);
        Some(FieldContext::for_fir(
            catalog.field_catalog(),
            values,
            self.mode,
            catalog.settings.field_markers.clone(),
        ))
    }

    fn push_fields(&self, cx: &mut App) {
        if let Some(fields) = self.field_context(cx) {
            self.editor.set_fields(fields, cx);
        }
    }

    fn set_values(&mut self, values: Rc<FirValues>, cx: &mut Context<Self>) {
        self.values = Some(values.clone());
        self.panel
            .update(cx, |panel, cx| panel.set_values(Some(values), cx));
        self.push_fields(cx);
        cx.notify();
    }

    fn refresh_references(&mut self, cx: &mut Context<Self>) {
        let loaded = self.document_state == DocumentState::Idle && self.loaded.is_some();
        let references = if loaded {
            self.editor.field_references(cx)
        } else {
            Vec::new()
        };
        self.panel
            .update(cx, |panel, cx| panel.set_references(references, loaded, cx));
    }

    fn schedule_references(&mut self, cx: &mut Context<Self>) {
        if self.references_task.is_some() {
            return;
        }
        self.references_task = Some(cx.spawn(async move |this: WeakEntity<Self>, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            this.update(cx, |this, cx| {
                this.references_task = None;
                this.refresh_references(cx);
            })
            .ok();
        }));
    }

    fn load_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.id;
        run_db_in(
            window,
            cx,
            move |store| store.fir_workspace(id),
            |this, result, window, cx| {
                match result {
                    Ok(workspace) => this.apply_workspace(workspace, window, cx),
                    Err(missal_core::RepositoryError::NotFound { .. }) => {
                        this.state = WorkspaceState::NotFound
                    }
                    Err(error) => this.state = WorkspaceState::Failed(error.message()),
                }
                cx.notify();
            },
        );
    }

    fn apply_workspace(
        &mut self,
        workspace: FirWorkspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let values = Rc::new(workspace.values);
        self.values = Some(values.clone());
        self.panel
            .update(cx, |panel, cx| panel.set_values(Some(values), cx));
        self.documents = workspace.documents;
        self.state = WorkspaceState::Ready;
        let first = self.documents.first().map(|document| document.id);
        match first {
            Some(first) => self.open_document(first, window, cx),
            None => {
                self.active = None;
                self.panel
                    .update(cx, |panel, cx| panel.set_tab(FieldsTab::Details, cx));
                self.refresh_references(cx);
            }
        }
    }

    fn refresh_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.id;
        run_db_in(
            window,
            cx,
            move |store| store.fir_values(id),
            |this, result, _, cx| {
                if let Ok(values) = result {
                    this.set_values(Rc::new(values), cx);
                }
            },
        );
    }

    fn open_document(&mut self, id: FirDocumentId, window: &mut Window, cx: &mut Context<Self>) {
        self.active = Some(id);
        self.document_state = DocumentState::Loading;
        cx.notify();
        run_db_in(
            window,
            cx,
            move |store| store.fir_document(id),
            move |this, result, window, cx| {
                if this.active != Some(id) {
                    return;
                }
                match result {
                    Ok(record) => {
                        let Some(fields) = this.field_context(cx) else {
                            return;
                        };
                        this.loaded = Some((record.id, record.revision));
                        this.document_state = DocumentState::Idle;
                        this.editor.load(
                            LoadRequest {
                                key: format!("fir-document:{}:{}", record.id, record.revision),
                                document: record.document,
                                fields,
                                dirty: false,
                            },
                            window,
                            cx,
                        );
                        this.editor_dirty = false;
                        this.refresh_references(cx);
                        this.take_pending_fields(window, cx);
                    }
                    Err(error) => this.document_state = DocumentState::Failed(error.message()),
                }
                cx.notify();
            },
        );
    }

    pub fn select_document(
        &mut self,
        id: FirDocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active == Some(id) {
            return;
        }
        if !self.is_dirty(cx) {
            self.open_document(id, window, cx);
            return;
        }
        let weak = cx.entity().downgrade();
        let discard = weak.clone();
        let save = weak;
        open_choice(
            window,
            cx,
            "Save changes first?",
            "This document has unsaved changes.",
            vec![
                Choice::new("discard", "Don't save", Tone::Ghost, move |window, cx| {
                    discard
                        .update(cx, |this, cx| {
                            this.editor.discard(window, cx);
                            this.editor_dirty = false;
                            this.open_document(id, window, cx);
                        })
                        .ok();
                }),
                Choice::new("cancel", "Cancel", Tone::Neutral, |_, _| {}),
                Choice::new("save", "Save", Tone::Primary, move |window, cx| {
                    save.update(cx, |this, cx| {
                        this.save_active(
                            window,
                            cx,
                            Some(Box::new(move |this, window, cx| {
                                this.open_document(id, window, cx)
                            })),
                        );
                    })
                    .ok();
                }),
            ],
        );
    }

    fn step_document(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(position) = self
            .documents
            .iter()
            .position(|document| Some(document.id) == self.active)
        else {
            return;
        };
        let next = position as isize + delta;
        if next >= 0 && (next as usize) < self.documents.len() {
            let id = self.documents[next as usize].id;
            self.select_document(id, window, cx);
        }
    }

    pub fn save_active(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: Option<Continuation>,
    ) {
        let Some((id, revision)) = self.loaded else {
            return;
        };
        if self.saving || Some(id) != self.active {
            return;
        }
        if !self.is_dirty(cx) {
            if let Some(then) = then {
                then(self, window, cx);
            }
            return;
        }
        let snapshot = self.editor.snapshot(cx);
        self.saving = true;
        cx.notify();
        let token = snapshot.revision;
        let document = snapshot.document;
        run_db_in(
            window,
            cx,
            move |store| store.save_fir_document(id, revision, &document),
            move |this, result, window, cx| {
                this.saving = false;
                match result {
                    Ok(ack) => {
                        this.loaded = Some((id, ack.revision));
                        this.editor.mark_saved(token, cx);
                        this.editor_dirty = this.editor.is_dirty(cx);
                        if let Some(summary) =
                            this.documents.iter_mut().find(|document| document.id == id)
                        {
                            summary.revision = ack.revision;
                            summary.preview_text = ack.preview_text;
                            summary.field_count = ack.field_count;
                            summary.updated_at = ack.updated_at;
                        }
                        if let Some(then) = then {
                            then(this, window, cx);
                        }
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    pub fn toggle_field_names(&mut self, cx: &mut Context<Self>) {
        self.mode = match self.mode {
            DisplayMode::Values => DisplayMode::Labels,
            DisplayMode::Labels => DisplayMode::Values,
        };
        self.push_fields(cx);
        cx.notify();
    }

    pub fn move_document(
        &mut self,
        id: FirDocumentId,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(position) = self.documents.iter().position(|document| document.id == id) else {
            return;
        };
        let target = position as isize + delta;
        if target < 0 || target as usize >= self.documents.len() {
            return;
        }
        let mut ids: Vec<FirDocumentId> =
            self.documents.iter().map(|document| document.id).collect();
        ids.swap(position, target as usize);
        let fir_id = self.id;
        run_db_in(
            window,
            cx,
            move |store| store.reorder_fir_documents(fir_id, &ids),
            |this, result, window, cx| {
                match result {
                    Ok(documents) => this.documents = documents,
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    pub fn remove_document(
        &mut self,
        id: FirDocumentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active == Some(id) && self.is_dirty(cx) {
            toast_warning(
                window,
                cx,
                "Save or discard your changes before removing this document",
            );
            return;
        }
        run_db_in(
            window,
            cx,
            move |store| store.remove_fir_document(id),
            move |this, result, window, cx| {
                match result {
                    Ok(()) => {
                        let position = this.documents.iter().position(|document| document.id == id);
                        this.documents.retain(|document| document.id != id);
                        if this.active == Some(id) {
                            let next = position
                                .and_then(|position| {
                                    this.documents
                                        .get(position.min(this.documents.len().saturating_sub(1)))
                                })
                                .map(|document| document.id);
                            this.active = None;
                            this.loaded = None;
                            match next {
                                Some(next) => this.open_document(next, window, cx),
                                None => {
                                    this.editor_dirty = false;
                                    this.refresh_references(cx);
                                }
                            }
                        }
                        toast_success(window, cx, "Document removed");
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    pub fn add_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<TemplateId> = self.add_selected.iter().copied().collect();
        if ids.is_empty() {
            return;
        }
        let fir_id = self.id;
        let before: HashSet<FirDocumentId> =
            self.documents.iter().map(|document| document.id).collect();
        self.add_open = false;
        run_db_in(
            window,
            cx,
            move |store| store.add_templates_to_fir(fir_id, &ids),
            move |this, result, window, cx| {
                match result {
                    Ok(documents) => {
                        let added: Vec<FirDocumentId> = documents
                            .iter()
                            .map(|d| d.id)
                            .filter(|id| !before.contains(id))
                            .collect();
                        let names: Vec<String> = documents
                            .iter()
                            .filter(|d| added.contains(&d.id))
                            .map(|d| d.title.clone())
                            .collect();
                        this.documents = documents;
                        this.add_selected.clear();
                        this.add_search
                            .update(cx, |input, cx| input.set_text("", cx));
                        let message = match names.as_slice() {
                            [one] => format!("Added {}", crate::ui::isolate_urdu(one)),
                            _ => format!("{} documents added", added.len()),
                        };
                        toast_success(window, cx, message);
                        if let Some(first) = added.first() {
                            this.select_document(*first, window, cx);
                        }
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    pub fn open_add_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_open = true;
        self.add_highlight = None;
        self.add_search
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    fn close_add_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_open = false;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub fn is_add_open(&self) -> bool {
        self.add_open
    }

    pub fn toggle_template(&mut self, id: TemplateId, cx: &mut Context<Self>) {
        if !self.add_selected.remove(&id) {
            self.add_selected.insert(id);
        }
        cx.notify();
    }

    pub fn edit_details(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        navigate(Route::EditFir(self.id), window, cx);
    }

    pub fn print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_print_preview(window, cx);
    }

    fn request_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(values) = &self.values else {
            return;
        };
        let id = self.id;
        let weak = cx.entity().downgrade();
        let count = self.documents.len();
        let body = match count {
            0 => "This can't be undone.".to_owned(),
            1 => "Its document is deleted too. This can't be undone.".to_owned(),
            n => format!("Its {n} documents are deleted too. This can't be undone."),
        };
        confirm_delete(
            window,
            cx,
            format!("Delete FIR {}?", values.fir.fir.fir_no),
            body,
            "Delete FIR",
            move |window, cx| {
                weak.update(cx, |this, cx| this.delete_fir(id, window, cx))
                    .ok();
            },
        );
    }

    fn delete_fir(&mut self, id: FirId, window: &mut Window, cx: &mut Context<Self>) {
        run_db_in(
            window,
            cx,
            move |store| store.remove_fir(id),
            move |this, result, window, cx| match result {
                Ok(()) => {
                    this.models
                        .firs
                        .update(cx, |firs, cx| firs.remove(&[id], cx));
                    this.editor_dirty = false;
                    this.editor.discard(window, cx);
                    navigate_replace(Route::Firs, cx);
                    toast_success(window, cx, "FIR deleted");
                }
                Err(error) => toast_error(window, cx, error.message()),
            },
        );
    }

    pub fn open_print_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state != WorkspaceState::Ready {
            return;
        }
        let excluded = crate::ui::print_preview::excluded_documents(self.id, cx);
        let included: Vec<FirDocumentId> = self
            .documents
            .iter()
            .map(|document| document.id)
            .filter(|id| !excluded.contains(id))
            .collect();
        let Some(values) = self.values.clone() else {
            return;
        };
        let live = self.active.filter(|id| {
            self.document_state == DocumentState::Idle
                && self.loaded.is_some_and(|(loaded, _)| loaded == *id)
        });
        let documents = included
            .iter()
            .map(|id| {
                if Some(*id) == live {
                    PreviewDocument::Content(Box::new(self.editor.snapshot(cx).document))
                } else {
                    PreviewDocument::Stored(*id)
                }
            })
            .collect();
        let title = format!("FIR {}", values.fir.fir.fir_no);
        let request = PrintRequest {
            export_name: title.clone(),
            title,
            note: None,
            catalog: (*self.models.catalog.read(cx).field_catalog()).clone(),
            values: Some((*values).clone()),
            mode: DisplayMode::Values,
            documents,
        };
        open_print_preview(request, window, cx);
    }

    fn cycle_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel_focus = self.panel.read(cx).focus_handle().clone();
        let editor_focused = self.editor.editor_entity().is_some_and(|editor| {
            editor
                .read(cx)
                .focus_handle_ref()
                .contains_focused(window, cx)
        });
        if self.rail_focus.is_focused(window) {
            self.editor.focus(window, cx);
        } else if editor_focused {
            if self.is_panel_open(window) {
                window.focus(&panel_focus, cx);
            } else {
                window.focus(&self.focus, cx);
            }
        } else if panel_focus.contains_focused(window, cx) {
            window.focus(&self.focus, cx);
        } else {
            window.focus(&self.rail_focus, cx);
        }
        cx.notify();
    }

    fn document_menu(&self, id: FirDocumentId, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let count = self.documents.len();
        let first = index == 0;
        let last = index + 1 == count;
        let can_remove = !(self.active == Some(id) && self.is_dirty(cx));
        let close = |this: &mut Self, cx: &mut Context<Self>| {
            this.menu = None;
            cx.notify();
        };
        menu_panel(Palette::get(cx))
            .w(px(200.))
            .child(
                menu_item(("document-open", index), "Open")
                    .keys("Enter")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        close(this, cx);
                        this.select_document(id, window, cx);
                    })),
            )
            .child(menu_separator(Palette::get(cx)))
            .child(
                menu_item(("document-up", index), "Move up")
                    .icon(IconName::ArrowUp)
                    .keys("Alt+Shift+Up")
                    .disabled(first)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        close(this, cx);
                        this.move_document(id, -1, window, cx);
                    })),
            )
            .child(
                menu_item(("document-down", index), "Move down")
                    .icon(IconName::ArrowDown)
                    .keys("Alt+Shift+Down")
                    .disabled(last)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        close(this, cx);
                        this.move_document(id, 1, window, cx);
                    })),
            )
            .child(menu_separator(Palette::get(cx)))
            .child(
                menu_item(("document-remove", index), "Remove from FIR")
                    .icon(IconName::Trash)
                    .danger()
                    .disabled(!can_remove)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        close(this, cx);
                        this.remove_document(id, window, cx);
                    })),
            )
            .into_any_element()
    }

    fn documents_rail(
        &self,
        palette: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = *palette;
        let count = self.documents.len();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let active_dirty = self.is_dirty(cx);
        let rail_focused = self.rail_focus.is_focused(window);
        let rows: Vec<AnyElement> = self
            .documents
            .iter()
            .enumerate()
            .map(|(index, document)| {
                let id = document.id;
                let active = self.active == Some(id);
                let title = document.title.clone();
                let menu_open = self.menu == Some(Menu::Document(id));
                let context = {
                    let weak = weak.clone();
                    let first = index == 0;
                    let last = index + 1 == count;
                    move |menu: PopupMenu| {
                        let (select, up, down, remove) =
                            (weak.clone(), weak.clone(), weak.clone(), weak.clone());
                        menu.item(PopupMenuItem::new("Open").on_click(move |_, window, cx| {
                            select
                                .update(cx, |this, cx| this.select_document(id, window, cx))
                                .ok();
                        }))
                        .item(PopupMenuItem::separator())
                        .item(PopupMenuItem::new("Move up").disabled(first).on_click(
                            move |_, window, cx| {
                                up.update(cx, |this, cx| this.move_document(id, -1, window, cx))
                                    .ok();
                            },
                        ))
                        .item(PopupMenuItem::new("Move down").disabled(last).on_click(
                            move |_, window, cx| {
                                down.update(cx, |this, cx| this.move_document(id, 1, window, cx))
                                    .ok();
                            },
                        ))
                        .item(PopupMenuItem::separator())
                        .item(
                            PopupMenuItem::new("Remove from FIR").on_click(move |_, window, cx| {
                                remove
                                    .update(cx, |this, cx| this.remove_document(id, window, cx))
                                    .ok();
                            }),
                        )
                    }
                };
                let menu_screen = cx.entity();
                let more = Popover::new(("document-menu", index))
                    .appearance(false)
                    .anchor(gpui_kit::Anchor::TopLeft)
                    .trigger(Trigger {
                        button: icon_button(
                            ("document-actions", index),
                            IconName::Ellipsis,
                            "More actions",
                        )
                        .dense(),
                        open: menu_open,
                    })
                    .open(menu_open)
                    .on_open_change({
                        let weak = weak.clone();
                        move |open, _, cx| {
                            let open = *open;
                            weak.update(cx, |this, cx| {
                                this.menu = open.then_some(Menu::Document(id));
                                cx.notify();
                            })
                            .ok();
                        }
                    })
                    .content(move |_, _, cx| {
                        menu_screen.update(cx, |this, cx| this.document_menu(id, index, cx))
                    });
                let hover = palette.hover;
                h_flex()
                    .id(("document-row", index))
                    .test_support()
                    .group("document-row")
                    .flex_row_reverse()
                    .h(px(40.))
                    .mx(px(8.))
                    .px(px(10.))
                    .gap(px(8.))
                    .items_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(active, |this| this.bg(palette.accent_wash))
                    .when(!active, |this| this.hover(move |style| style.bg(hover)))
                    .when(active && rail_focused, |this| {
                        this.border_r_2().border_color(palette.accent)
                    })
                    .role(gpui_kit::accesskit::Role::ListItem)
                    .aria_label(SharedString::from(title.clone()))
                    .on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.select_document(id, window, cx)
                        }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(14.))
                            .pt(px(2.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(palette.ink_faint)
                            .text_right()
                            .child((index + 1).to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(palette.ink)
                            .child(ur_label(title, UrRole::Body).truncate()),
                    )
                    .children((active && active_dirty).then(|| {
                        div()
                            .flex_none()
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .group_hover("document-row", |style| style.invisible())
                            .child(dirty_dot(&palette, true))
                    }))
                    .child(
                        div()
                            .flex_none()
                            .when(!menu_open, |this| {
                                this.invisible()
                                    .group_hover("document-row", |style| style.visible())
                            })
                            .when(active && active_dirty, |this| this.absolute().left(px(8.)))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(more),
                    )
                    .context_menu(move |popup, _, _| context(popup))
                    .into_any_element()
            })
            .collect();
        let add = self.add_popover(&palette, cx);
        v_flex()
            .id("workspace-rail")
            .key_context("DocumentRail")
            .track_focus(&self.rail_focus)
            .w(px(RAIL_WIDTH))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(palette.line)
            .bg(palette.surface)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !this.rail_focus.is_focused(window) {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "down" => this.step_document(1, window, cx),
                    "up" => this.step_document(-1, window, cx),
                    "enter" => this.editor.focus(window, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                h_flex()
                    .flex_none()
                    .h(px(40.))
                    .pl(px(16.))
                    .pr(px(8.))
                    .gap(px(6.))
                    .items_center()
                    .child(section_label(&palette, "Documents"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(palette.ink_faint)
                            .child(count.to_string()),
                    ),
            )
            .when(self.state == WorkspaceState::Loading, |this| {
                this.when(self.slow, |this| {
                    this.child(
                        v_flex()
                            .px(px(18.))
                            .gap(px(26.))
                            .pt(px(14.))
                            .items_end()
                            .child(skeleton_bar(&palette, 160., 12.))
                            .child(skeleton_bar(&palette, 132., 12.))
                            .child(skeleton_bar(&palette, 148., 12.)),
                    )
                })
            })
            .when(self.state == WorkspaceState::Ready, |this| {
                this.child(
                    v_flex()
                        .id("workspace-rail-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(v_flex().gap(px(2.)).children(rows))
                        .child(add),
                )
            })
            .into_any_element()
    }

    fn add_popover(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let screen = cx.entity();
        let templates = self.models.templates.read(cx);
        let attached: HashSet<TemplateId> = self
            .documents
            .iter()
            .map(|document| document.template_id)
            .collect();
        let words: Vec<String> = fold_for_search(&self.add_query)
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let available: Vec<(TemplateId, String, bool)> = templates
            .items
            .iter()
            .filter(|item| {
                let haystack = fold_for_search(&item.name);
                words.iter().all(|word| haystack.contains(word.as_str()))
            })
            .map(|item| (item.id, item.name.clone(), attached.contains(&item.id)))
            .collect();
        let no_templates = templates.items.is_empty();
        let search = self.add_search.clone();
        let highlight = self.add_highlight;
        Popover::new("add-templates")
            .appearance(false)
            .trigger(AddRow {
                open: self.add_open,
            })
            .trigger_style(gpui_kit::StyleRefinement::default().w_full())
            .anchor(gpui_kit::Anchor::TopLeft)
            .open(self.add_open)
            .on_open_change({
                let weak = weak.clone();
                move |open, window, cx| {
                    let open = *open;
                    weak.update(cx, |this, cx| {
                        if open {
                            this.open_add_templates(window, cx);
                        } else {
                            this.close_add_templates(window, cx);
                        }
                    })
                    .ok();
                }
            })
            .content(move |_, _, cx| {
                let state = screen.read(cx);
                let selected = state.add_selected.clone();
                let selected_count = selected.len();
                let screen = screen.clone();
                let selectable: Vec<usize> = available
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, _, added))| !added)
                    .map(|(index, _)| index)
                    .collect();
                let keys_screen = screen.clone();
                let ids: Vec<(TemplateId, bool)> = available
                    .iter()
                    .map(|(id, _, added)| (*id, *added))
                    .collect();
                v_flex()
                    .id("add-document-card")
                    .w(px(360.))
                    .bg(palette.raised)
                    .rounded(px(8.))
                    .shadow(palette.shadow_md())
                    .text_color(palette.ink)
                    .font_family(missal_text::UI_FAMILY)
                    .on_action({
                        let screen = screen.clone();
                        cx.listener(move |_, _: &ConfirmSelection, window, cx| {
                            screen.update(cx, |this, cx| this.add_templates(window, cx));
                        })
                    })
                    .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.as_str();
                        let modified = event.keystroke.modifiers.modified();
                        match key {
                            "down" | "up" if !modified => {
                                keys_screen.update(cx, |this, cx| {
                                    if selectable.is_empty() {
                                        return;
                                    }
                                    let current = this
                                        .add_highlight
                                        .and_then(|row| selectable.iter().position(|i| *i == row));
                                    let next = match (current, key) {
                                        (None, "down") => 0,
                                        (None, _) => selectable.len() - 1,
                                        (Some(p), "down") => (p + 1).min(selectable.len() - 1),
                                        (Some(p), _) => p.saturating_sub(1),
                                    };
                                    this.add_highlight = Some(selectable[next]);
                                    cx.notify();
                                });
                                cx.stop_propagation();
                            }
                            "space" if !modified => {
                                let toggled = keys_screen.update(cx, |this, cx| {
                                    let Some(row) = this.add_highlight else {
                                        return false;
                                    };
                                    if let Some((id, false)) = ids.get(row).copied() {
                                        this.toggle_template(id, cx);
                                        return true;
                                    }
                                    false
                                });
                                if toggled {
                                    cx.stop_propagation();
                                }
                            }
                            "escape" => {
                                keys_screen
                                    .update(cx, |this, cx| this.close_add_templates(window, cx));
                                cx.stop_propagation();
                            }
                            _ => {}
                        }
                    })
                    .child(
                        h_flex()
                            .h(px(48.))
                            .px(px(14.))
                            .gap(px(8.))
                            .items_center()
                            .border_b_1()
                            .border_color(palette.line)
                            .child(keycap(&palette, "Esc"))
                            .child(div().flex_1().min_w_0().mt(px(-1.)).child(search.clone()))
                            .child(sized_icon(IconName::Search, ICON_SM, palette.ink_faint)),
                    )
                    .child(
                        v_flex()
                            .id("add-template-list")
                            .max_h(px(280.))
                            .overflow_y_scroll()
                            .p(px(4.))
                            .children(available.iter().enumerate().map(
                                |(row, (id, name, added))| {
                                    let (id, added) = (*id, *added);
                                    let checked = selected.contains(&id);
                                    let screen = screen.clone();
                                    let highlighted = highlight == Some(row);
                                    h_flex()
                                        .id(("add-template", id.get() as usize))
                                        .test_support()
                                        .flex_row_reverse()
                                        .h(px(40.))
                                        .px(px(10.))
                                        .gap(px(10.))
                                        .items_center()
                                        .rounded(px(6.))
                                        .when(highlighted, |this| this.bg(palette.hover))
                                        .when(!added, |this| {
                                            this.cursor_pointer()
                                                .hover(move |style| style.bg(palette.hover))
                                                .on_click(cx.listener(move |_, _, _, cx| {
                                                    screen.update(cx, |this, cx| {
                                                        this.toggle_template(id, cx)
                                                    });
                                                }))
                                        })
                                        .child(if added {
                                            checkbox(
                                                &palette,
                                                ("add-check", id.get() as usize),
                                                true,
                                            )
                                            .opacity(0.5)
                                            .into_any_element()
                                        } else {
                                            checkbox(
                                                &palette,
                                                ("add-check", id.get() as usize),
                                                checked,
                                            )
                                            .into_any_element()
                                        })
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_color(if added {
                                                    palette.ink_muted
                                                } else {
                                                    palette.ink
                                                })
                                                .child(
                                                    ur_label(name.clone(), UrRole::Body).truncate(),
                                                ),
                                        )
                                        .children(added.then(|| {
                                            div()
                                                .flex_none()
                                                .text_size(px(12.))
                                                .line_height(px(16.))
                                                .text_color(palette.ink_faint)
                                                .child("Added")
                                        }))
                                },
                            ))
                            .when(available.is_empty(), |this| {
                                this.child(
                                    div()
                                        .px(px(12.))
                                        .py(px(10.))
                                        .text_size(px(13.))
                                        .text_color(palette.ink_muted)
                                        .child(if no_templates {
                                            "No templates exist yet."
                                        } else {
                                            "No templates match."
                                        }),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .h(px(48.))
                            .px(px(12.))
                            .gap(px(8.))
                            .items_center()
                            .border_t_1()
                            .border_color(palette.line)
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .line_height(px(16.))
                                    .text_color(palette.ink_muted)
                                    .child(format!("{selected_count} selected")),
                            )
                            .child(
                                button(
                                    "add-templates-confirm",
                                    match selected_count {
                                        1 => "Add 1 document".to_owned(),
                                        n => format!("Add {n} documents"),
                                    },
                                )
                                .primary()
                                .sm()
                                .keys("Mod+Enter")
                                .disabled(selected_count == 0)
                                .on_click({
                                    let screen = screen.clone();
                                    move |_, window, cx| {
                                        screen
                                            .update(cx, |this, cx| this.add_templates(window, cx));
                                    }
                                }),
                            ),
                    )
            })
            .into_any_element()
    }

    fn canvas(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        if self.state == WorkspaceState::Loading {
            return div().size_full().bg(palette.sunken).into_any_element();
        }
        if self.documents.is_empty() {
            return div()
                .size_full()
                .bg(palette.sunken)
                .child(empty_state(
                    &palette,
                    IconName::FileText,
                    "No documents yet",
                    Some("Add a template to start writing for this FIR.".into()),
                    Some(
                        button("empty-add-templates", "Add document")
                            .primary()
                            .icon(IconName::Plus)
                            .keys("A")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_add_templates(window, cx)
                            }))
                            .into_any_element(),
                    ),
                ))
                .into_any_element();
        }
        match &self.document_state {
            DocumentState::Loading => div().size_full().bg(palette.sunken).into_any_element(),
            DocumentState::Failed(message) => div()
                .size_full()
                .bg(palette.sunken)
                .child(empty_state(
                    &palette,
                    IconName::TriangleAlert,
                    "Couldn't open this document",
                    Some(message.clone().into()),
                    Some(
                        button("document-retry", "Try again")
                            .outline()
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(active) = this.active {
                                    this.open_document(active, window, cx);
                                }
                            }))
                            .into_any_element(),
                    ),
                ))
                .into_any_element(),
            DocumentState::Idle => div()
                .size_full()
                .child(self.editor.view())
                .into_any_element(),
        }
    }

    fn more_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = *Palette::get(cx);
        let names = self.mode == DisplayMode::Labels;
        let close = |this: &mut Self, cx: &mut Context<Self>| {
            this.menu = None;
            cx.notify();
        };
        menu_panel(&palette)
            .w(px(232.))
            .child(
                menu_item("more-names", "Show field names")
                    .check(names)
                    .keys("Mod+Shift+L")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        close(this, cx);
                        this.toggle_field_names(cx);
                    })),
            )
            .child(
                menu_item("more-case-details", "Case details")
                    .icon(IconName::PanelRight)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        close(this, cx);
                        this.panel_choice = Some(true);
                        this.panel
                            .update(cx, |panel, cx| panel.set_tab(FieldsTab::Details, cx));
                    })),
            )
            .child(menu_separator(&palette))
            .child(
                menu_item("more-delete", "Delete FIR")
                    .icon(IconName::Trash)
                    .danger()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        close(this, cx);
                        this.request_delete(window, cx);
                    })),
            )
            .into_any_element()
    }
}

impl Render for WorkspaceScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        match &self.state {
            WorkspaceState::NotFound => {
                return empty_state(
                    &palette,
                    IconName::Folder,
                    "FIR not found",
                    Some("This FIR doesn't exist or was deleted.".into()),
                    Some(
                        button("back-to-firs", "Back to FIRs")
                            .outline()
                            .on_click(|_, window, cx| navigate(Route::Firs, window, cx))
                            .into_any_element(),
                    ),
                )
                .into_any_element();
            }
            WorkspaceState::Failed(message) => {
                return empty_state(
                    &palette,
                    IconName::TriangleAlert,
                    "Couldn't open this FIR",
                    Some(format!("Your saved documents are safe. ({message})").into()),
                    Some(
                        button("reload-workspace", "Try again")
                            .outline()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.state = WorkspaceState::Loading;
                                this.load_workspace(window, cx);
                                cx.notify();
                            }))
                            .into_any_element(),
                    ),
                )
                .into_any_element();
            }
            _ => {}
        }
        let panel_open = self.is_panel_open(window) && self.state == WorkspaceState::Ready;
        let rail = self.documents_rail(&palette, window, cx);
        let canvas = self.canvas(&palette, cx);
        h_flex()
            .id("workspace")
            .key_context(WORKSPACE)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save_active(window, cx, None)))
            .on_action(cx.listener(|this, _: &EditFir, window, cx| this.edit_details(window, cx)))
            .on_action(
                cx.listener(|this, _: &AddTemplates, window, cx| {
                    this.open_add_templates(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &NextDocument, window, cx| this.step_document(1, window, cx)),
            )
            .on_action(cx.listener(|this, _: &PreviousDocument, window, cx| {
                this.step_document(-1, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleFieldNames, _, cx| this.toggle_field_names(cx)))
            .on_action(
                cx.listener(|this, _: &ToggleFieldsPanel, window, cx| {
                    this.toggle_panel(window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &MoveDocumentUp, window, cx| {
                if let Some(active) = this.active() {
                    this.move_document(active, -1, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &MoveDocumentDown, window, cx| {
                if let Some(active) = this.active() {
                    this.move_document(active, 1, window, cx);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "f6" && !event.keystroke.modifiers.modified() {
                    this.cycle_focus(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(rail)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .bg(palette.sunken)
                    .child(canvas),
            )
            .when(panel_open, |this| this.child(self.panel.clone()))
            .into_any_element()
    }
}

impl crate::ui::titlebar::HeaderSource for WorkspaceScreen {
    fn header(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::ui::titlebar::HeaderContent {
        let palette = *Palette::get(cx);
        let row = self.models.firs.read(cx).get(self.id).cloned();
        let fir = self.values.as_ref().map(|values| values.fir.fir.clone());
        let number = fir
            .as_ref()
            .map(|fir| fir.fir_no.clone())
            .or_else(|| row.as_ref().map(|row| row.no.to_string()));
        let status = fir
            .as_ref()
            .map(|fir| fir.status)
            .or_else(|| row.as_ref().map(|row| row.status));
        let offence = fir
            .as_ref()
            .map(|fir| fir.offence.clone())
            .or_else(|| row.as_ref().map(|row| row.offence.to_string()));
        let title = h_flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .flex_none()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(
                        missal_text::label(match number {
                            Some(number) => format!("FIR {number}"),
                            None => "FIR".to_owned(),
                        })
                        .ltr(),
                    ),
            )
            .children(status.map(|status| crate::ui::status::status_pill(&palette, status)));
        let mut content = crate::ui::titlebar::HeaderContent::element(title);
        if let Some(offence) = offence {
            content = content.meta_element(
                div()
                    .min_w_0()
                    .mt(px(-2.))
                    .text_color(palette.ink_muted)
                    .child(crate::ui::ur_inline(offence, UrRole::Title).truncate()),
            );
        }
        let state = if self.saving {
            SaveState::Saving
        } else if self.is_dirty(cx) {
            SaveState::Unsaved
        } else {
            SaveState::Saved
        };
        let panel_open = self.is_panel_open(window);
        let more_open = self.menu == Some(Menu::More);
        let more_screen = cx.entity();
        let weak = cx.entity().downgrade();
        let more = Popover::new("workspace-more")
            .appearance(false)
            .anchor(gpui_kit::Anchor::TopRight)
            .trigger(Trigger {
                button: icon_button("header-more", IconName::Ellipsis, "More actions"),
                open: more_open,
            })
            .open(more_open)
            .on_open_change(move |open, _, cx| {
                let open = *open;
                weak.update(cx, |this, cx| {
                    this.menu = open.then_some(Menu::More);
                    cx.notify();
                })
                .ok();
            })
            .content(move |_, _, cx| more_screen.update(cx, |this, cx| this.more_menu(cx)));
        content
            .action(div().mr(px(6.)).child(save_state(&palette, state)))
            .action(
                icon_button("header-edit-details", IconName::Pencil, "Edit details")
                    .keys("E")
                    .on_click(cx.listener(|this, _, window, cx| this.edit_details(window, cx))),
            )
            .action(
                icon_button("header-print", IconName::Printer, "Print")
                    .keys("Mod+P")
                    .on_click(cx.listener(|this, _, window, cx| this.print(window, cx))),
            )
            .action(
                icon_button("header-fields-panel", IconName::PanelRight, "Fields panel")
                    .keys("Mod+.")
                    .toggled(panel_open)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_panel(window, cx))),
            )
            .action(more)
    }
}
