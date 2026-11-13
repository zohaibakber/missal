use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::menu::DropdownMenu as _;

use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription,
    WeakEntity, Window, div, px,
};
use missal_core::{
    DisplayMode, FirDocumentId, FirId, FirValues, Revision, TemplateId, format_date,
};
use missal_store::{FirDocumentSummary, FirWorkspace};
use missal_text::{TextInput, TextInputEvent, label};

use crate::app::navigate_replace;
use crate::editor::{DocumentEditor, EditorEvent, EditorFactory, FieldContext, LoadRequest};
use crate::screens::custom_values::open_custom_values;
use crate::screens::edit_sheet::open_edit_sheet;
use crate::shortcuts::*;
use crate::state::catalog::CatalogEvent;
use crate::state::firs::FirsEvent;
use crate::state::rows::fold_for_search;
use crate::state::{Models, Route, not_available_yet, run_db_in, toast_error, toast_success, toast_warning};
use crate::theme::{DETAILS_RAIL_WIDTH, Palette};
use crate::ui::dialogs::{Choice, Tone, confirm_delete, open_choice};
use crate::ui::input::{Kind, text_input};
use crate::ui::status::status_icon;
use crate::ui::{empty_state, icon_button, pane_header, rail_header, small_icon, status_bar};

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

type Continuation = Box<dyn FnOnce(&mut WorkspaceScreen, &mut Window, &mut Context<WorkspaceScreen>)>;

pub struct WorkspaceScreen {
    models: Models,
    id: FirId,
    focus: FocusHandle,
    state: WorkspaceState,
    values: Option<Rc<FirValues>>,
    documents: Vec<FirDocumentSummary>,
    active: Option<FirDocumentId>,
    loaded: Option<(FirDocumentId, Revision)>,
    document_state: DocumentState,
    excluded: HashSet<FirDocumentId>,
    mode: DisplayMode,
    editor: Rc<dyn DocumentEditor>,
    editor_dirty: bool,
    saving: bool,
    add_open: bool,
    add_selected: HashSet<TemplateId>,
    add_search: Entity<TextInput>,
    add_query: String,
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceScreen {
    pub fn new(models: Models, id: FirId, window: &mut Window, cx: &mut Context<Self>) -> WorkspaceScreen {
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
        let add_search = text_input(window, cx, Kind::Urdu, "Search templates", "");
        let subscriptions = vec![
            editor_subscription,
            cx.subscribe(&add_search, |this, input, event: &TextInputEvent, cx| {
                if *event == TextInputEvent::Change {
                    this.add_query = input.read(cx).text().to_owned();
                    cx.notify();
                }
            }),
            cx.subscribe_in(&models.firs, window, |this, _, _: &FirsEvent, window, cx| {
                if this.state == WorkspaceState::Ready {
                    this.refresh_values(window, cx);
                }
            }),
            cx.subscribe(&models.catalog, |this, _, _: &CatalogEvent, cx| {
                this.push_fields(cx);
                cx.notify();
            }),
            cx.observe(&models.templates, |_, _, cx| cx.notify()),
        ];
        let mut screen = WorkspaceScreen {
            models,
            id,
            focus: cx.focus_handle(),
            state: WorkspaceState::Loading,
            values: None,
            documents: Vec::new(),
            active: None,
            loaded: None,
            document_state: DocumentState::Idle,
            excluded: HashSet::new(),
            mode: DisplayMode::Values,
            editor,
            editor_dirty: false,
            saving: false,
            add_open: false,
            add_selected: HashSet::new(),
            add_search,
            add_query: String::new(),
            _subscriptions: subscriptions,
        };
        screen.load_workspace(window, cx);
        screen
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.editor_dirty || self.editor.is_dirty(cx)
    }

    pub fn state(&self) -> &WorkspaceState {
        &self.state
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

    pub fn excluded_count(&self) -> usize {
        self.excluded.len()
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

    fn load_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.id;
        run_db_in(window, cx, move |store| store.fir_workspace(id), |this, result, window, cx| {
            match result {
                Ok(workspace) => this.apply_workspace(workspace, window, cx),
                Err(missal_core::RepositoryError::NotFound { .. }) => this.state = WorkspaceState::NotFound,
                Err(error) => this.state = WorkspaceState::Failed(error.message()),
            }
            cx.notify();
        });
    }

    fn apply_workspace(&mut self, workspace: FirWorkspace, window: &mut Window, cx: &mut Context<Self>) {
        self.values = Some(Rc::new(workspace.values));
        self.documents = workspace.documents;
        self.state = WorkspaceState::Ready;
        let first = self.documents.first().map(|document| document.id);
        match first {
            Some(first) => self.open_document(first, window, cx),
            None => self.active = None,
        }
    }

    fn refresh_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.id;
        run_db_in(window, cx, move |store| store.fir_values(id), |this, result, _, cx| {
            if let Ok(values) = result {
                this.values = Some(Rc::new(values));
                this.push_fields(cx);
                cx.notify();
            }
        });
    }

    fn open_document(&mut self, id: FirDocumentId, window: &mut Window, cx: &mut Context<Self>) {
        self.active = Some(id);
        self.document_state = DocumentState::Loading;
        cx.notify();
        run_db_in(window, cx, move |store| store.fir_document(id), move |this, result, window, cx| {
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
                        },
                        window,
                        cx,
                    );
                    this.editor_dirty = false;
                }
                Err(error) => this.document_state = DocumentState::Failed(error.message()),
            }
            cx.notify();
        });
    }

    pub fn select_document(&mut self, id: FirDocumentId, window: &mut Window, cx: &mut Context<Self>) {
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
                Choice::new("cancel", "Cancel", Tone::Neutral, |_, _| {}),
                Choice::new("discard", "Discard", Tone::Danger, move |window, cx| {
                    discard
                        .update(cx, |this, cx| {
                            this.editor.discard(window, cx);
                            this.editor_dirty = false;
                            this.open_document(id, window, cx);
                        })
                        .ok();
                }),
                Choice::new("save", "Save", Tone::Primary, move |window, cx| {
                    save.update(cx, |this, cx| {
                        this.save_active(window, cx, Some(Box::new(move |this, window, cx| this.open_document(id, window, cx))));
                    })
                    .ok();
                }),
            ],
        );
    }

    fn step_document(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(position) = self.documents.iter().position(|document| Some(document.id) == self.active) else {
            return;
        };
        let next = position as isize + delta;
        if next >= 0 && (next as usize) < self.documents.len() {
            let id = self.documents[next as usize].id;
            self.select_document(id, window, cx);
        }
    }

    pub fn save_active(&mut self, window: &mut Window, cx: &mut Context<Self>, then: Option<Continuation>) {
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
                        if let Some(summary) = this.documents.iter_mut().find(|document| document.id == id) {
                            summary.revision = ack.revision;
                            summary.preview_text = ack.preview_text;
                            summary.field_count = ack.field_count;
                            summary.updated_at = ack.updated_at;
                        }
                        toast_success(window, cx, "Document saved");
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

    pub fn toggle_included(&mut self, id: FirDocumentId, cx: &mut Context<Self>) {
        if !self.excluded.remove(&id) {
            self.excluded.insert(id);
        }
        cx.notify();
    }

    pub fn move_document(&mut self, id: FirDocumentId, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(position) = self.documents.iter().position(|document| document.id == id) else {
            return;
        };
        let target = position as isize + delta;
        if target < 0 || target as usize >= self.documents.len() {
            return;
        }
        let mut ids: Vec<FirDocumentId> = self.documents.iter().map(|document| document.id).collect();
        ids.swap(position, target as usize);
        let fir_id = self.id;
        run_db_in(window, cx, move |store| store.reorder_fir_documents(fir_id, &ids), |this, result, window, cx| {
            match result {
                Ok(documents) => this.documents = documents,
                Err(error) => toast_error(window, cx, error.message()),
            }
            cx.notify();
        });
    }

    pub fn remove_document(&mut self, id: FirDocumentId, window: &mut Window, cx: &mut Context<Self>) {
        if self.active == Some(id) && self.is_dirty(cx) {
            toast_warning(window, cx, "Save or discard your changes before removing this document");
            return;
        }
        run_db_in(window, cx, move |store| store.remove_fir_document(id), move |this, result, window, cx| {
            match result {
                Ok(()) => {
                    let position = this.documents.iter().position(|document| document.id == id);
                    this.documents.retain(|document| document.id != id);
                    this.excluded.remove(&id);
                    if this.active == Some(id) {
                        let next = position
                            .and_then(|position| this.documents.get(position.min(this.documents.len().saturating_sub(1))))
                            .map(|document| document.id);
                        this.active = None;
                        this.loaded = None;
                        match next {
                            Some(next) => this.open_document(next, window, cx),
                            None => this.editor_dirty = false,
                        }
                    }
                    toast_success(window, cx, "Document removed");
                }
                Err(error) => toast_error(window, cx, error.message()),
            }
            cx.notify();
        });
    }

    pub fn add_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<TemplateId> = self.add_selected.iter().copied().collect();
        if ids.is_empty() {
            return;
        }
        let fir_id = self.id;
        let before: HashSet<FirDocumentId> = self.documents.iter().map(|document| document.id).collect();
        self.add_open = false;
        run_db_in(window, cx, move |store| store.add_templates_to_fir(fir_id, &ids), move |this, result, window, cx| {
            match result {
                Ok(documents) => {
                    let added: Vec<FirDocumentId> = documents.iter().map(|d| d.id).filter(|id| !before.contains(id)).collect();
                    this.documents = documents;
                    this.add_selected.clear();
                    this.add_search.update(cx, |input, cx| input.set_text("", cx));
                    let message = if added.len() == 1 { "Document added".to_owned() } else { format!("{} documents added", added.len()) };
                    toast_success(window, cx, message);
                    if let Some(first) = added.first() {
                        this.select_document(*first, window, cx);
                    }
                }
                Err(error) => toast_error(window, cx, error.message()),
            }
            cx.notify();
        });
    }

    pub fn open_add_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_open = true;
        self.add_search.update(cx, |input, cx| input.focus(window, cx));
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
        if let Some(values) = &self.values {
            open_edit_sheet(values.fir.clone(), window, cx);
        }
    }

    pub fn edit_custom_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(values) = &self.values else {
            return;
        };
        let weak = cx.entity().downgrade();
        open_custom_values(
            self.id,
            values.fir.fir.fir_no.clone(),
            values.overrides.clone(),
            Rc::new(move |overrides, window, cx| {
                weak.update(cx, |this, cx| {
                    if let Some(values) = this.values.as_ref() {
                        let mut updated = (**values).clone();
                        updated.overrides = overrides;
                        this.values = Some(Rc::new(updated));
                        this.push_fields(cx);
                        cx.notify();
                    }
                    let _ = window;
                })
                .ok();
            }),
            window,
            cx,
        );
    }

    fn request_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(values) = &self.values else {
            return;
        };
        let id = self.id;
        let weak = cx.entity().downgrade();
        confirm_delete(
            window,
            cx,
            format!("Delete FIR {}?", values.fir.fir.fir_no),
            "This removes the FIR and every document created for it. This can't be undone.",
            "Delete FIR",
            move |window, cx| {
                weak.update(cx, |this, cx| this.delete_fir(id, window, cx)).ok();
            },
        );
    }

    fn delete_fir(&mut self, id: FirId, window: &mut Window, cx: &mut Context<Self>) {
        run_db_in(window, cx, move |store| store.remove_fir(id), move |this, result, window, cx| match result {
            Ok(()) => {
                this.models.firs.update(cx, |firs, cx| firs.remove(&[id], cx));
                this.editor_dirty = false;
                this.editor.discard(window, cx);
                navigate_replace(Route::Firs, cx);
                toast_success(window, cx, "FIR deleted");
            }
            Err(error) => toast_error(window, cx, error.message()),
        });
    }

    fn included_count(&self) -> usize {
        self.documents.iter().filter(|document| !self.excluded.contains(&document.id)).count()
    }

    fn details(&self, palette: &Palette, values: &FirValues, cx: &mut Context<Self>) -> AnyElement {
        let fir = &values.fir.fir;
        let palette = *palette;
        let field = |title: &'static str, body: AnyElement| {
            v_flex()
                .gap_0p5()
                .child(
                    div()
                        .w_full()
                        .text_size(px(12.))
                        .text_color(palette.muted_foreground)
                        .child(label(title).rtl().w_full()),
                )
                .child(div().w_full().text_size(px(15.)).child(body))
        };
        let text = |value: &str| label(value.to_owned()).rtl().w_full().into_any_element();
        let dated = |value: &str| {
            let shown = if value.trim().is_empty() { "—".to_owned() } else { format_date(value) };
            label(shown).rtl().w_full().into_any_element()
        };
        let list = |items: &[String]| {
            v_flex()
                .gap_1()
                .children(items.iter().enumerate().map(|(index, item)| {
                    h_flex()
                        .flex_row_reverse()
                        .items_start()
                        .gap_2()
                        .child(div().w(px(18.)).flex_shrink_0().text_size(px(12.)).text_color(palette.muted_foreground).text_right().child((index + 1).to_string()))
                        .child(div().flex_1().min_w_0().child(label(item.clone()).rtl().w_full()))
                }))
                .into_any_element()
        };
        let optional = |title: &'static str, value: &str| (!value.trim().is_empty()).then(|| field(title, text(value)).into_any_element());
        let mut items: Vec<AnyElement> = vec![
            field(
                "حالت",
                h_flex()
                    .flex_row_reverse()
                    .items_center()
                    .gap_2()
                    .child(status_icon(fir.status, palette.status(fir.status), px(14.)))
                    .child(label(fir.status.label()).rtl())
                    .into_any_element(),
            )
            .into_any_element(),
        ];
        items.extend(optional("مدعی مقدمہ", &fir.complainant));
        items.push(field("تاریخ ایف آئی آر", dated(&fir.date)).into_any_element());
        items.push(field("تاریخ وقوعہ", dated(&fir.incident_date)).into_any_element());
        items.push(field("تاریخ گرفتاری", dated(&fir.arrest_date)).into_any_element());
        items.push(field("ملزمان", list(&fir.accused)).into_any_element());
        if !fir.witness.is_empty() {
            items.push(field("گواہان", list(&fir.witness)).into_any_element());
        }
        if !fir.zimni.is_empty() {
            items.push(field("ضمنی", list(&fir.zimni)).into_any_element());
        }
        items.extend(optional("تفتیشی افسر", &fir.investigation_officer));
        items.extend(optional("تحریر کنندہ", &fir.writer));
        items.extend(optional("برآمدگی", &fir.recovery));
        items.extend(optional("متن", &fir.body));
        items.extend(optional("ایمی نمبر 1", &fir.imei_1));
        items.extend(optional("ایمی نمبر 2", &fir.imei_2));
        items.extend(optional("سم نمبر 1", &fir.sim_1));
        items.extend(optional("سم نمبر 2", &fir.sim_2));
        let _ = cx;
        v_flex().gap_3().px_4().pb_3().children(items)
            .into_any_element()
    }

    fn custom_section(&self, palette: &Palette, values: &FirValues, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = *palette;
        let catalog = self.models.catalog.read(cx);
        let customs: Vec<_> = catalog.custom_fields().collect();
        if customs.is_empty() {
            return None;
        }
        let filled: Vec<(String, String)> = customs
            .iter()
            .filter_map(|field| {
                values
                    .overrides
                    .iter()
                    .find(|value| value.placeholder_id == field.id && !value.value.is_empty())
                    .map(|value| (field.label.clone(), value.value.clone()))
            })
            .collect();
        let total = customs.len();
        Some(
            v_flex()
                .child(
                    rail_header(&palette, "Custom fields").child(
                        icon_button("edit-custom-values", IconName::Pencil)
                            .tooltip("Edit custom field values")
                            .on_click(cx.listener(|this, _, window, cx| this.edit_custom_values(window, cx))),
                    ),
                )
                .child(
                    v_flex()
                        .gap_3()
                        .px_4()
                        .pb_3()
                        .children(filled.iter().map(|(name, value)| {
                            v_flex()
                                .gap_0p5()
                                .child(div().w_full().text_size(px(12.)).text_color(palette.muted_foreground).child(label(name.clone()).rtl().w_full()))
                                .child(div().w_full().text_size(px(15.)).child(label(value.clone()).rtl().w_full()))
                        }))
                        .when(filled.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(palette.muted_foreground)
                                    .child(format!("No values yet. {total} custom fields can be filled for this FIR.")),
                            )
                        }),
                )
                .into_any_element(),
        )
    }

    fn documents_rail(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let count = self.documents.len();
        let multiple = count > 1;
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let screen = cx.entity();
        let active_dirty = self.is_dirty(cx);
        let rows = self.documents.iter().enumerate().map(|(index, document)| {
            let id = document.id;
            let active = self.active == Some(id);
            let included = !self.excluded.contains(&id);
            let title = document.title.clone();
            let first = index == 0;
            let last = index + 1 == count;
            let can_remove = !(active && active_dirty);
            let menu = {
                let weak = weak.clone();
                move |menu: PopupMenu| {
                    let (select, up, down, include, remove) = (weak.clone(), weak.clone(), weak.clone(), weak.clone(), weak.clone());
                    menu.item(PopupMenuItem::new("Open").on_click(move |_, window, cx| {
                        select.update(cx, |this, cx| this.select_document(id, window, cx)).ok();
                    }))
                    .item(PopupMenuItem::new("Include when printing").checked(included).on_click(move |_, _, cx| {
                        include.update(cx, |this, cx| this.toggle_included(id, cx)).ok();
                    }))
                    .item(PopupMenuItem::separator())
                    .item(PopupMenuItem::new("Move up").disabled(first).on_click(move |_, window, cx| {
                        up.update(cx, |this, cx| this.move_document(id, -1, window, cx)).ok();
                    }))
                    .item(PopupMenuItem::new("Move down").disabled(last).on_click(move |_, window, cx| {
                        down.update(cx, |this, cx| this.move_document(id, 1, window, cx)).ok();
                    }))
                    .item(PopupMenuItem::separator())
                    .item(PopupMenuItem::new("Remove from FIR").disabled(!can_remove).on_click(move |_, window, cx| {
                        remove.update(cx, |this, cx| this.remove_document(id, window, cx)).ok();
                    }))
                }
            };
            let dropdown_menu = menu.clone();
            h_flex()
                .id(("document-row", index))
                .test_support()
                .group("document-row")
                .flex_row_reverse()
                .h(px(40.))
                .mx_2()
                .px_2()
                .gap_2()
                .items_center()
                .rounded(px(7.))
                .cursor_pointer()
                .when(active, |this| this.bg(palette.accent))
                .hover(move |style| style.bg(palette.accent))
                .on_click(cx.listener(move |this, _, window, cx| this.select_document(id, window, cx)))
                .children(multiple.then(|| {
                    div()
                        .flex_shrink_0()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Checkbox::new(("include", index))
                                .checked(included)
                                .accessibility_label("Include when printing")
                                .on_click(cx.listener(move |this, _, _, cx| this.toggle_included(id, cx))),
                        )
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_size(px(15.))
                        .child(label(title).rtl().truncate()),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .invisible()
                        .group_hover("document-row", |style| style.visible())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new(("document-actions", index))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Ellipsis)
                                .dropdown_menu(move |menu, _, _| dropdown_menu(menu)),
                        ),
                )
                .context_menu(move |popup, _, _| menu(popup))
                .into_any_element()
        });
        let rows: Vec<AnyElement> = rows.collect();
        let templates = self.models.templates.read(cx);
        let attached: HashSet<TemplateId> = self.documents.iter().map(|document| document.template_id).collect();
        let words: Vec<String> = fold_for_search(&self.add_query).split_whitespace().map(str::to_owned).collect();
        let available: Vec<_> = templates
            .items
            .iter()
            .filter(|item| {
                let haystack = fold_for_search(&item.name);
                words.iter().all(|word| haystack.contains(word.as_str()))
            })
            .map(|item| (item.id, item.name.clone(), attached.contains(&item.id)))
            .collect();
        let no_templates = templates.items.is_empty();
        let add = Popover::new("add-templates")
            .trigger(icon_button("add-templates-trigger", IconName::Plus).tooltip("Add templates (A)"))
            .anchor(gpui_kit::Anchor::TopRight)
            .open(self.add_open)
            .on_open_change({
                let weak = weak.clone();
                move |open, window, cx| {
                    let open = *open;
                    weak.update(cx, |this, cx| {
                        if open {
                            this.open_add_templates(window, cx);
                        } else {
                            this.add_open = false;
                            cx.notify();
                        }
                    })
                    .ok();
                }
            })
            .p_0()
            .content({
                let search = self.add_search.clone();
                move |_, _, cx| {
                    let state = screen.read(cx);
                    let selected = state.add_selected.clone();
                    let selected_count = selected.len();
                    let screen = screen.clone();
                    v_flex()
                        .w(px(320.))
                        .on_action({
                            let screen = screen.clone();
                            cx.listener(move |_, _: &ConfirmSelection, window, cx| {
                                screen.update(cx, |this, cx| this.add_templates(window, cx));
                            })
                        })
                        .child(div().p_2().child(search.clone()))
                        .child(
                            v_flex().id("add-template-list").max_h(px(280.)).overflow_y_scroll().p_1().children(
                                available.iter().map(|(id, name, added)| {
                                    let (id, added) = (*id, *added);
                                    let checked = selected.contains(&id);
                                    let screen = screen.clone();
                                    h_flex()
                                        .id(("add-template", id.get() as usize))
                                        .test_support()
                                        .flex_row_reverse()
                                        .h(px(38.))
                                        .px_2()
                                        .gap_2()
                                        .items_center()
                                        .rounded(px(6.))
                                        .when(!added, |this| {
                                            this.cursor_pointer().hover(move |style| style.bg(palette.accent)).on_click(cx.listener(
                                                move |_, _, _, cx| {
                                                    screen.update(cx, |this, cx| this.toggle_template(id, cx));
                                                    cx.notify();
                                                },
                                            ))
                                        })
                                        .when(added, |this| this.opacity(0.55))
                                        .child(small_icon(if checked { IconName::SquareCheck } else { IconName::Square }, if checked { palette.primary } else { palette.muted_foreground }))
                                        .child(div().flex_1().min_w_0().overflow_hidden().text_size(px(15.)).child(label(name.clone()).rtl().truncate()))
                                        .children(added.then(|| div().text_size(px(12.)).text_color(palette.muted_foreground).child("Added")))
                                }),
                            ),
                        )
                        .child(
                            h_flex()
                                .h(px(44.))
                                .px_3()
                                .gap_2()
                                .items_center()
                                .border_t_1()
                                .border_color(palette.border)
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(12.))
                                        .text_color(palette.muted_foreground)
                                        .child(if no_templates {
                                            "No templates exist yet".to_owned()
                                        } else if selected_count == 0 {
                                            "Select templates to add".to_owned()
                                        } else {
                                            format!("{selected_count} selected")
                                        }),
                                )
                                .child(
                                    Button::new("add-templates-confirm")
                                        .primary()
                                        .xsmall()
                                        .label("Add")
                                        .disabled(selected_count == 0)
                                        .on_click({
                                            let screen = screen.clone();
                                            move |_, window, cx| {
                                                screen.update(cx, |this, cx| this.add_templates(window, cx));
                                            }
                                        }),
                                ),
                        )
                }
            });
        v_flex()
            .child(
                rail_header(&palette, "Documents")
                    .child(div().text_size(px(12.)).text_color(palette.muted_foreground).child(count.to_string()))
                    .child(add),
            )
            .child(v_flex().gap_0p5().children(rows))
            .children((count == 0).then(|| {
                div()
                    .px_4()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(palette.muted_foreground)
                    .child("No documents yet.")
            }))
            .into_any_element()
    }

    fn main_panel(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        if self.documents.is_empty() {
            return empty_state(
                &palette,
                IconName::FileText,
                "No documents yet",
                Some("Add a template to write this FIR's first document.".into()),
                Some(
                    Button::new("empty-add-templates")
                        .primary()
                        .icon(IconName::Plus)
                        .label("Add templates")
                        .on_click(cx.listener(|this, _, window, cx| this.open_add_templates(window, cx)))
                        .into_any_element(),
                ),
            )
            .into_any_element();
        }
        match &self.document_state {
            DocumentState::Loading => div()
                .size_full()
                .bg(palette.canvas)
                .p_6()
                .child(div().h_full().rounded(px(8.)).bg(palette.muted.opacity(0.6)))
                .into_any_element(),
            DocumentState::Failed(message) => {
                empty_state(&palette, IconName::TriangleAlert, "Could not load this document", Some(message.clone().into()), None).into_any_element()
            }
            DocumentState::Idle => div().size_full().child(self.editor.view()).into_any_element(),
        }
    }
}

impl Render for WorkspaceScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        match &self.state {
            WorkspaceState::NotFound => {
                return empty_state(&palette, IconName::Folder, "FIR not found", Some("This FIR doesn't exist or was deleted.".into()), None)
                    .into_any_element();
            }
            WorkspaceState::Failed(message) => {
                return empty_state(
                    &palette,
                    IconName::TriangleAlert,
                    "Could not load this FIR",
                    Some(format!("Your saved documents are safe. Reload to try again. ({message})").into()),
                    Some(
                        Button::new("reload-workspace")
                            .outline()
                            .label("Reload")
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
        let values = self.values.clone();
        let offence = values.as_ref().map(|values| values.fir.fir.offence.clone()).unwrap_or_default();
        let dirty = self.is_dirty(cx);
        let saving = self.saving;
        let has_active = self.active.is_some() && self.document_state == DocumentState::Idle;
        let included = self.included_count();
        let total = self.documents.len();
        let printable = included > 0;
        let names_shown = self.mode == DisplayMode::Labels;
        let weak = cx.entity().downgrade();
        let header = pane_header(&palette)
            .flex_row_reverse()
            .pl_2()
            .pr_4()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_size(px(18.))
                    .font_medium()
                    .child(label(offence).rtl().truncate().w_full()),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("workspace-more")
                            .ghost()
                            .small()
                            .icon(IconName::Ellipsis)
                            .tooltip("More actions")
                            .dropdown_menu({
                                let weak = weak.clone();
                                move |menu, _, _| {
                                    let (toggle, delete) = (weak.clone(), weak.clone());
                                    menu.item(PopupMenuItem::new("Show field names").checked(names_shown).on_click(move |_, _, cx| {
                                        toggle.update(cx, |this, cx| this.toggle_field_names(cx)).ok();
                                    }))
                                    .item(PopupMenuItem::separator())
                                    .item(PopupMenuItem::new("Delete FIR").icon(IconName::Trash).on_click(move |_, window, cx| {
                                        delete.update(cx, |this, cx| this.request_delete(window, cx)).ok();
                                    }))
                                }
                            }),
                    )
                    .children(has_active.then(|| {
                        Button::new("workspace-save")
                            .small()
                            .loading(saving)
                            .disabled(!dirty || saving)
                            .when(dirty, |this| this.primary())
                            .when(!dirty, |this| this.outline())
                            .label(if saving { "Saving…" } else if dirty { "Save" } else { "Saved" })
                            .on_click(cx.listener(|this, _, window, cx| this.save_active(window, cx, None)))
                    }))
                    .child(
                        icon_button("custom-values-button", IconName::Braces)
                            .tooltip("Custom field values")
                            .on_click(cx.listener(|this, _, window, cx| this.edit_custom_values(window, cx))),
                    )
                    .child(
                        icon_button("workspace-print", IconName::Printer)
                            .tooltip("Print preview")
                            .disabled(!printable)
                            .on_click(|_, window, cx| not_available_yet(window, cx, "Print preview")),
                    ),
            );
        let rail = v_flex()
            .id("workspace-rail")
            .w(px(DETAILS_RAIL_WIDTH))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(palette.border)
            .child(
                v_flex()
                    .id("workspace-rail-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(self.documents_rail(&palette, cx))
                    .child(div().h(px(1.)).mx_4().my_1().bg(palette.border))
                    .child(
                        rail_header(&palette, "Details").child(
                            icon_button("edit-details", IconName::Pencil)
                                .tooltip("Edit details (E)")
                                .on_click(cx.listener(|this, _, window, cx| this.edit_details(window, cx))),
                        ),
                    )
                    .children(values.as_ref().map(|values| self.details(&palette, values, cx)))
                    .children(values.as_ref().and_then(|values| self.custom_section(&palette, values, cx))),
            )
            .children((total > 1).then(|| {
                status_bar(&palette)
                    .child(small_icon(IconName::Printer, palette.muted_foreground))
                    .child(if included == total { "All will print".to_owned() } else { format!("{included} of {total} will print") })
            }));
        let body: AnyElement = if self.state == WorkspaceState::Loading {
            div().flex_1().into_any_element()
        } else {
            self.main_panel(&palette, cx)
        };
        h_flex()
            .id("workspace")
            .key_context(WORKSPACE)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save_active(window, cx, None)))
            .on_action(cx.listener(|this, _: &EditFir, window, cx| this.edit_details(window, cx)))
            .on_action(cx.listener(|this, _: &AddTemplates, window, cx| this.open_add_templates(window, cx)))
            .on_action(cx.listener(|this, _: &NextDocument, window, cx| this.step_document(1, window, cx)))
            .on_action(cx.listener(|this, _: &PreviousDocument, window, cx| this.step_document(-1, window, cx)))
            .on_action(cx.listener(|this, _: &ToggleFieldNames, _, cx| this.toggle_field_names(cx)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(header)
                    .child(div().flex_1().min_h_0().child(body)),
            )
            .child(rail)
            .into_any_element()
    }
}
