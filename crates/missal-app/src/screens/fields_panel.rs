use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use missal_core::{
    DisplayMode, FieldPresentation, FieldReference, FirField, FirId, FirValues, PlaceholderId,
    PlaceholderSource, format_date,
};
use missal_text::{Align, LineHeight, TextInput, TextInputEvent, label};

use crate::app::{navigate, navigate_back};
use crate::state::rows::fold_for_search;
use crate::state::{Models, Route, run_db_in, toast_error};
use crate::theme::{DETAILS_RAIL_WIDTH, Palette, TAB_HEIGHT};
use crate::ui::input::{Kind, field, field_input, search_field};
use crate::ui::status::status_label;
use crate::ui::{UrRole, button, count_badge, section_label, sized_icon, ur_label};

pub const AUTO_HIDE_BELOW: f32 = 1180.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldsTab {
    Fields,
    Details,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Global(PlaceholderId),
    Custom(PlaceholderId),
    Fir(FirField),
}

impl Scope {
    fn tag(&self) -> &'static str {
        match self {
            Scope::Global(_) => "Global",
            Scope::Custom(_) => "This FIR",
            Scope::Fir(_) => "FIR details",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldItem {
    pub reference: FieldReference,
    pub name: String,
    pub value: String,
    pub scope: Scope,
}

impl FieldItem {
    pub fn is_empty(&self) -> bool {
        self.value.trim().is_empty()
    }
}

pub fn field_items(
    references: &[FieldReference],
    catalog: &missal_core::FieldCatalog,
    values: &FirValues,
) -> Vec<FieldItem> {
    let presentation = FieldPresentation::for_fir(catalog, values, DisplayMode::Values);
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for reference in references {
        if !seen.insert(reference.clone()) {
            continue;
        }
        let scope = match reference {
            FieldReference::Fir(field) => Scope::Fir(*field),
            FieldReference::Catalog(id) => match catalog.placeholder(*id) {
                Some(placeholder) => match &placeholder.source {
                    PlaceholderSource::Shared => Scope::Global(*id),
                    PlaceholderSource::Custom => Scope::Custom(*id),
                },
                None => continue,
            },
            FieldReference::Unresolved(_) => continue,
        };
        let Some(name) = catalog.label(reference) else {
            continue;
        };
        let shown = presentation.display_text(reference);
        let value = if shown.unresolved {
            String::new()
        } else {
            shown.text
        };
        items.push(FieldItem {
            reference: reference.clone(),
            name,
            value,
            scope,
        });
    }
    items
}

pub fn empty_references(
    references: &[FieldReference],
    catalog: &missal_core::FieldCatalog,
    values: &FirValues,
) -> Vec<FieldReference> {
    field_items(references, catalog, values)
        .into_iter()
        .filter(FieldItem::is_empty)
        .map(|item| item.reference)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingFields(pub FirId);

impl gpui_kit::Global for PendingFields {}

pub fn open_fields_panel(fir: FirId, window: &mut Window, cx: &mut App) {
    cx.set_global(PendingFields(fir));
    let models = Models::get(cx);
    let (back, current) = {
        let nav = models.nav.read(cx);
        let (history, index) = nav.history();
        (
            index
                .checked_sub(1)
                .and_then(|previous| history.get(previous))
                .copied(),
            nav.current(),
        )
    };
    if current == Route::Fir(fir) {
        if let Some(shell) = crate::app::ShellHandle::get(cx)
            && let Some(workspace) = shell.read(cx).workspace()
        {
            workspace.update(cx, |workspace, cx| {
                workspace.take_pending_fields(window, cx)
            });
        }
    } else if back == Some(Route::Fir(fir)) {
        navigate_back(window, cx);
    } else {
        navigate(Route::Fir(fir), window, cx);
    }
}

pub fn take_pending_fields(fir: FirId, cx: &mut App) -> bool {
    if cx.try_global::<PendingFields>() == Some(&PendingFields(fir)) {
        cx.remove_global::<PendingFields>();
        return true;
    }
    false
}

pub enum FieldsPanelEvent {
    ValuesChanged(Rc<FirValues>),
    Reveal(FieldReference, bool),
    EditDetails,
}

pub struct FieldsPanel {
    models: Models,
    fir_id: FirId,
    values: Option<Rc<FirValues>>,
    references: Vec<FieldReference>,
    has_document: bool,
    tab: FieldsTab,
    inputs: HashMap<FieldReference, Entity<TextInput>>,
    filter: Entity<TextInput>,
    query: String,
    show_all: bool,
    focus: FocusHandle,
    subscriptions: HashMap<FieldReference, [Subscription; 1]>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<FieldsPanelEvent> for FieldsPanel {}

impl FieldsPanel {
    pub fn new(
        models: Models,
        fir_id: FirId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FieldsPanel {
        let filter = field_input(window, cx, Kind::SearchRtl, "تلاش", "");
        let subscriptions =
            vec![
                cx.subscribe(&filter, |this, input, event: &TextInputEvent, cx| {
                    if *event == TextInputEvent::Change {
                        this.query = input.read(cx).text().to_owned();
                        cx.notify();
                    }
                }),
            ];
        FieldsPanel {
            models,
            fir_id,
            values: None,
            references: Vec::new(),
            has_document: false,
            tab: FieldsTab::Fields,
            inputs: HashMap::new(),
            filter,
            query: String::new(),
            show_all: false,
            focus: cx.focus_handle(),
            subscriptions: HashMap::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn tab(&self) -> FieldsTab {
        self.tab
    }

    pub fn set_tab(&mut self, tab: FieldsTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    pub fn set_show_all(&mut self, show_all: bool, cx: &mut Context<Self>) {
        self.show_all = show_all;
        cx.notify();
    }

    pub fn set_values(&mut self, values: Option<Rc<FirValues>>, cx: &mut Context<Self>) {
        self.values = values;
        cx.notify();
    }

    pub fn set_references(
        &mut self,
        references: Vec<FieldReference>,
        has_document: bool,
        cx: &mut Context<Self>,
    ) {
        if self.references != references || self.has_document != has_document {
            self.references = references;
            self.has_document = has_document;
            cx.notify();
        }
    }

    pub fn references(&self) -> &[FieldReference] {
        &self.references
    }

    pub fn items(&self, cx: &App) -> Vec<FieldItem> {
        let Some(values) = self.values.as_ref() else {
            return Vec::new();
        };
        let catalog = self.models.catalog.read(cx).field_catalog();
        field_items(&self.references, &catalog, values)
    }

    pub fn empty_count(&self, cx: &App) -> usize {
        self.items(cx).iter().filter(|item| item.is_empty()).count()
    }

    pub fn input_for(&self, reference: &FieldReference) -> Option<Entity<TextInput>> {
        self.inputs.get(reference).cloned()
    }

    pub fn custom_input(&self, id: PlaceholderId) -> Option<Entity<TextInput>> {
        self.input_for(&FieldReference::Catalog(id))
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    pub fn focus_first_empty(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.tab = FieldsTab::Fields;
        let first = self
            .items(cx)
            .into_iter()
            .find(|item| item.is_empty() && !matches!(item.scope, Scope::Fir(_)));
        cx.notify();
        let Some(item) = first else {
            return false;
        };
        let input = self.ensure_input(&item.reference, "", window, cx);
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.emit(FieldsPanelEvent::Reveal(item.reference, false));
        true
    }

    fn ensure_input(
        &mut self,
        reference: &FieldReference,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        if let Some(input) = self.inputs.get(reference) {
            return input.clone();
        }
        let input = field_input(window, cx, Kind::UrduAuto, "Empty", text);
        let key = reference.clone();
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, input, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Focus => cx.notify(),
                TextInputEvent::Blur => {
                    let text = input.read(cx).text().to_owned();
                    this.save(key.clone(), text, window, cx);
                    cx.notify();
                }
                TextInputEvent::Submit => {
                    let text = input.read(cx).text().to_owned();
                    this.save(key.clone(), text, window, cx);
                }
                TextInputEvent::Change => cx.notify(),
            },
        );
        self.inputs.insert(reference.clone(), input.clone());
        self.subscriptions.insert(reference.clone(), [subscription]);
        input
    }

    fn current_value(&self, reference: &FieldReference, cx: &App) -> Option<(Scope, String)> {
        let values = self.values.as_ref()?;
        let catalog = self.models.catalog.read(cx).field_catalog();
        field_items(std::slice::from_ref(reference), &catalog, values)
            .into_iter()
            .next()
            .map(|item| (item.scope, item.value))
    }

    pub fn save(
        &mut self,
        reference: FieldReference,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((scope, current)) = self.current_value(&reference, cx) else {
            return;
        };
        let text = text.trim().to_owned();
        if text == current {
            return;
        }
        let fir_id = self.fir_id;
        match scope {
            Scope::Global(id) => {
                run_db_in(
                    window,
                    cx,
                    move |store| {
                        store.set_shared_value(id, &text)?;
                        store.settings()
                    },
                    |this, result, window, cx| match result {
                        Ok(settings) => {
                            let shared = settings.shared_values.clone();
                            this.models
                                .catalog
                                .update(cx, |catalog, cx| catalog.set_settings(settings, cx));
                            if let Some(values) = this.values.as_ref() {
                                let mut updated = (**values).clone();
                                updated.shared_values = shared;
                                let updated = Rc::new(updated);
                                this.values = Some(updated.clone());
                                cx.emit(FieldsPanelEvent::ValuesChanged(updated));
                            }
                            cx.notify();
                        }
                        Err(error) => toast_error(window, cx, error.message()),
                    },
                );
            }
            Scope::Custom(id) => {
                run_db_in(
                    window,
                    cx,
                    move |store| store.set_placeholder_values(fir_id, &[(id, text)]),
                    |this, result, window, cx| match result {
                        Ok(overrides) => {
                            if let Some(values) = this.values.as_ref() {
                                let mut updated = (**values).clone();
                                updated.overrides = overrides;
                                let updated = Rc::new(updated);
                                this.values = Some(updated.clone());
                                cx.emit(FieldsPanelEvent::ValuesChanged(updated));
                            }
                            cx.notify();
                        }
                        Err(error) => toast_error(window, cx, error.message()),
                    },
                );
            }
            Scope::Fir(_) => {}
        }
    }

    fn tabs(&self, palette: &Palette, empty: usize, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let tab = |id: &'static str, text: &'static str, which: FieldsTab, badge: Option<usize>| {
            let on = self.tab == which;
            let ink = palette.ink;
            h_flex()
                .id(id)
                .h(px(TAB_HEIGHT))
                .gap(px(6.))
                .items_center()
                .border_b_2()
                .border_color(if on {
                    palette.ink
                } else {
                    gpui_kit::transparent_black()
                })
                .text_size(px(13.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { palette.ink } else { palette.ink_muted })
                .cursor_pointer()
                .role(gpui_kit::accesskit::Role::Tab)
                .when(on, |this| this.aria_selected(true))
                .when(!on, |this| this.hover(move |s| s.text_color(ink)))
                .on_click(cx.listener(move |this, _, _, cx| this.set_tab(which, cx)))
                .child(div().pt(px(2.)).child(text))
                .children(
                    badge
                        .filter(|count| *count > 0)
                        .map(|count| div().pt(px(2.)).child(count_badge(&palette, count))),
                )
                .test_support()
        };
        h_flex()
            .flex_none()
            .h(px(TAB_HEIGHT))
            .px(px(16.))
            .gap(px(16.))
            .items_end()
            .border_b_1()
            .border_color(palette.line)
            .child(tab("fields-tab", "Fields", FieldsTab::Fields, Some(empty)))
            .child(tab("details-tab", "Case details", FieldsTab::Details, None))
            .into_any_element()
    }

    fn section(palette: &Palette, title: &'static str, count: Option<usize>) -> AnyElement {
        h_flex()
            .px(px(16.))
            .pt(px(12.))
            .pb(px(4.))
            .gap(px(8.))
            .items_center()
            .child(section_label(palette, title))
            .children(count.map(|count| {
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_faint)
                    .child(count.to_string())
            }))
            .into_any_element()
    }

    fn value_cell(palette: &Palette, value: &str) -> AnyElement {
        if is_latin(value) {
            div()
                .flex_1()
                .min_w_0()
                .pt(px(3.))
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(palette.ink)
                .child(
                    label(value.to_owned())
                        .ltr()
                        .align(Align::Right)
                        .leading(LineHeight::Px(20.))
                        .truncate(),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .text_color(palette.ink)
                .child(ur_label(value.to_owned(), UrRole::Body).truncate())
                .into_any_element()
        }
    }

    fn kv_row(
        palette: &Palette,
        id: impl Into<gpui_kit::ElementId>,
        name: &str,
        value: AnyElement,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        h_flex()
            .id(id)
            .flex_row_reverse()
            .items_start()
            .gap(px(12.))
            .px(px(16.))
            .py(px(2.))
            .child(
                div()
                    .w(px(92.))
                    .flex_none()
                    .pt(px(2.))
                    .text_color(palette.ink_muted)
                    .child(ur_label(name.to_owned(), UrRole::Label).truncate()),
            )
            .child(value)
    }

    fn fields_tab(
        &mut self,
        palette: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let palette = *palette;
        let items = self.items(cx);
        let mut out: Vec<AnyElement> = Vec::new();
        if !self.has_document {
            out.push(
                div()
                    .px(px(16.))
                    .py(px(12.))
                    .text_size(px(13.))
                    .text_color(palette.ink_muted)
                    .child("Open a document to see its fields.")
                    .into_any_element(),
            );
            return out;
        }
        let (empty, filled): (Vec<FieldItem>, Vec<FieldItem>) =
            items.into_iter().partition(FieldItem::is_empty);
        if empty.is_empty() {
            out.push(
                v_flex()
                    .px(px(16.))
                    .pt(px(20.))
                    .pb(px(8.))
                    .gap(px(10.))
                    .items_center()
                    .child(
                        div()
                            .size(px(36.))
                            .rounded(px(8.))
                            .bg(palette.hover)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(sized_icon(IconName::CircleCheck, 16., palette.st_closed)),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(palette.ink_muted)
                            .child("Every field in this document is filled."),
                    )
                    .into_any_element(),
            );
        } else {
            out.push(Self::section(&palette, "Empty in this document", None));
            for (index, item) in empty.iter().enumerate() {
                out.push(self.empty_item(&palette, index, item, window, cx));
            }
        }
        if !filled.is_empty() {
            out.push(
                div()
                    .pt(px(4.))
                    .child(Self::section(&palette, "Filled", Some(filled.len())))
                    .into_any_element(),
            );
            for (index, item) in filled.iter().enumerate() {
                let reference = item.reference.clone();
                out.push(
                    Self::kv_row(
                        &palette,
                        ("filled-field", index),
                        &item.name,
                        Self::value_cell(&palette, &item.value),
                    )
                    .cursor_pointer()
                    .hover(move |s| s.bg(palette.hover))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(FieldsPanelEvent::Reveal(reference.clone(), true))
                    }))
                    .test_support()
                    .into_any_element(),
                );
            }
        }
        out
    }

    fn empty_item(
        &mut self,
        palette: &Palette,
        index: usize,
        item: &FieldItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = *palette;
        let reference = item.reference.clone();
        let reveal = reference.clone();
        let top = h_flex()
            .id(("empty-field", index))
            .flex_row_reverse()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(FieldsPanelEvent::Reveal(reveal.clone(), true))
            }))
            .child(
                div()
                    .min_w_0()
                    .text_color(palette.ink)
                    .child(ur_label(item.name.clone(), UrRole::Body).truncate()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(11.))
                    .line_height(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.ink_faint)
                    .child(item.scope.tag()),
            )
            .test_support();
        let body: AnyElement = match &item.scope {
            Scope::Fir(_) => h_flex()
                .flex_row_reverse()
                .child(
                    button(("edit-in-details", index), "Edit in details")
                        .ghost()
                        .sm()
                        .icon(IconName::Pencil)
                        .on_click(
                            cx.listener(|_, _, _, cx| cx.emit(FieldsPanelEvent::EditDetails)),
                        ),
                )
                .into_any_element(),
            _ => {
                let input = self.ensure_input(&reference, "", window, cx);
                field(("empty-field-input", index), &input, Kind::UrduAuto).into_any_element()
            }
        };
        let help = matches!(item.scope, Scope::Global(_))
            && self
                .inputs
                .get(&reference)
                .is_some_and(|input| input.focus_handle(cx).is_focused(window));
        v_flex()
            .px(px(16.))
            .py(px(6.))
            .gap(px(6.))
            .child(top)
            .child(body)
            .when(help, |this| {
                this.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(palette.ink_faint)
                        .child("Used by every FIR. Press Enter to save."),
                )
            })
            .into_any_element()
    }

    fn details_tab(
        &mut self,
        palette: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let palette = *palette;
        let mut out: Vec<AnyElement> = Vec::new();
        let Some(values) = self.values.clone() else {
            return out;
        };
        out.push(
            h_flex()
                .px(px(10.))
                .pt(px(10.))
                .pb(px(4.))
                .child(
                    button("details-edit", "Edit details")
                        .ghost()
                        .sm()
                        .icon(IconName::Pencil)
                        .tooltip("Edit details", Some("E"))
                        .on_click(
                            cx.listener(|_, _, _, cx| cx.emit(FieldsPanelEvent::EditDetails)),
                        ),
                )
                .into_any_element(),
        );
        let fir = &values.fir.fir;
        let mut row = 0usize;
        let mut push = |out: &mut Vec<AnyElement>, name: &str, value: AnyElement| {
            out.push(Self::kv_row(&palette, ("detail", row), name, value).into_any_element());
            row += 1;
        };
        let text = |value: &str| -> Option<AnyElement> {
            (!value.trim().is_empty()).then(|| Self::value_cell(&palette, value))
        };
        let dash = || {
            div()
                .flex_1()
                .pt(px(3.))
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(palette.ink_faint)
                .child(label("—").align(Align::Right).leading(LineHeight::Px(20.)))
                .into_any_element()
        };
        push(
            &mut out,
            "ایف آئی آر نمبر",
            text(&fir.fir_no).unwrap_or_else(dash),
        );
        push(
            &mut out,
            "حالت",
            h_flex()
                .flex_1()
                .flex_row_reverse()
                .child(status_label(&palette, fir.status).flex_row_reverse())
                .into_any_element(),
        );
        push(&mut out, "جرم", text(&fir.offence).unwrap_or_else(dash));
        push(
            &mut out,
            "مدعی مقدمہ",
            text(&fir.complainant).unwrap_or_else(dash),
        );
        for (name, value) in [
            ("تاریخ ایف آئی آر", &fir.date),
            ("تاریخ وقوعہ", &fir.incident_date),
            ("تاریخ گرفتاری", &fir.arrest_date),
        ] {
            push(
                &mut out,
                name,
                text(&format_date(value)).unwrap_or_else(dash),
            );
        }
        for (name, items) in [
            ("ملزمان", &fir.accused),
            ("گواہان", &fir.witness),
            ("ضمنی", &fir.zimni),
        ] {
            let list = if items.iter().all(|item| item.trim().is_empty()) {
                dash()
            } else {
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .children(items.iter().enumerate().map(|(index, item)| {
                        h_flex()
                            .flex_row_reverse()
                            .items_start()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(12.))
                                    .pt(px(6.))
                                    .text_size(px(12.))
                                    .line_height(px(16.))
                                    .text_color(palette.ink_faint)
                                    .child((index + 1).to_string()),
                            )
                            .child(Self::value_cell(&palette, item))
                    }))
                    .into_any_element()
            };
            push(&mut out, name, list);
        }
        for (name, value) in [
            ("تفتیشی افسر", &fir.investigation_officer),
            ("تحریر کنندہ", &fir.writer),
            ("برآمدگی", &fir.recovery),
            ("متن", &fir.body),
            ("ایمی نمبر 1", &fir.imei_1),
            ("سم نمبر 1", &fir.sim_1),
            ("ایمی نمبر 2", &fir.imei_2),
            ("سم نمبر 2", &fir.sim_2),
        ] {
            push(&mut out, name, text(value).unwrap_or_else(dash));
        }
        let customs: Vec<(PlaceholderId, String)> = self
            .models
            .catalog
            .read(cx)
            .custom_fields()
            .map(|field| (field.id, field.label.clone()))
            .collect();
        if customs.is_empty() {
            return out;
        }
        let value_of = |id: PlaceholderId| {
            values
                .overrides
                .iter()
                .rev()
                .find(|value| value.placeholder_id == id)
                .map(|value| value.value.clone())
                .unwrap_or_default()
        };
        let total = customs.len();
        let with_value = customs
            .iter()
            .filter(|(id, _)| !value_of(*id).trim().is_empty())
            .count();
        out.push(
            div()
                .pt(px(12.))
                .child(Self::section(&palette, "Custom values", Some(with_value)))
                .into_any_element(),
        );
        if self.show_all || total > 12 {
            out.push(
                div()
                    .px(px(16.))
                    .py(px(4.))
                    .child(search_field("custom-filter", &self.filter, Kind::SearchRtl))
                    .into_any_element(),
            );
        }
        let focused = self
            .inputs
            .iter()
            .find(|(_, input)| input.focus_handle(cx).is_focused(window))
            .map(|(reference, _)| reference.clone());
        let words: Vec<String> = fold_for_search(&self.query)
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let shown: Vec<(PlaceholderId, String, String)> = customs
            .into_iter()
            .map(|(id, name)| {
                let value = value_of(id);
                (id, name, value)
            })
            .filter(|(id, _, value)| {
                self.show_all
                    || !value.trim().is_empty()
                    || self
                        .inputs
                        .get(&FieldReference::Catalog(*id))
                        .is_some_and(|input| {
                            focused.as_ref() == Some(&FieldReference::Catalog(*id))
                                || !input.read(cx).text().is_empty()
                        })
            })
            .filter(|(_, name, _)| {
                let haystack = fold_for_search(name);
                words.iter().all(|word| haystack.contains(word.as_str()))
            })
            .collect();
        for (index, (id, name, value)) in shown.iter().enumerate() {
            let reference = FieldReference::Catalog(*id);
            let input = self.ensure_input(&reference, value, window, cx);
            if focused.as_ref() != Some(&reference) && input.read(cx).text() != value {
                input.update(cx, |input, cx| input.set_text(value.clone(), cx));
            }
            out.push(
                v_flex()
                    .id(("custom-value", index))
                    .px(px(16.))
                    .py(px(6.))
                    .gap(px(6.))
                    .child(
                        div()
                            .text_color(palette.ink)
                            .child(ur_label(name.clone(), UrRole::Body).truncate()),
                    )
                    .child(
                        field(("custom-value-input", index), &input, Kind::UrduAuto)
                            .clearable(focused.as_ref() == Some(&reference)),
                    )
                    .into_any_element(),
            );
        }
        if shown.is_empty() && !self.query.is_empty() {
            out.push(
                div()
                    .px(px(16.))
                    .py(px(8.))
                    .text_size(px(13.))
                    .text_color(palette.ink_muted)
                    .child("No custom fields match.")
                    .into_any_element(),
            );
        }
        let show_all = self.show_all;
        out.push(
            h_flex()
                .px(px(12.))
                .py(px(8.))
                .child(
                    button(
                        "custom-show-all",
                        if show_all {
                            SharedString::from("Show filled only")
                        } else {
                            SharedString::from(format!("Show all {total}"))
                        },
                    )
                    .ghost()
                    .sm()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_all = !this.show_all;
                        cx.notify();
                    })),
                )
                .into_any_element(),
        );
        out
    }
}

pub fn is_latin(value: &str) -> bool {
    !value.chars().any(|c| {
        matches!(c, '\u{0590}'..='\u{08FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
    })
}

impl Render for FieldsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let empty = if self.has_document {
            self.empty_count(cx)
        } else {
            0
        };
        let body = match self.tab {
            FieldsTab::Fields => self.fields_tab(&palette, window, cx),
            FieldsTab::Details => self.details_tab(&palette, window, cx),
        };
        v_flex()
            .id("fields-panel")
            .track_focus(&self.focus)
            .w(px(DETAILS_RAIL_WIDTH))
            .flex_none()
            .h_full()
            .border_l_1()
            .border_color(palette.line)
            .bg(palette.surface)
            .child(self.tabs(&palette, empty, cx))
            .child(
                v_flex()
                    .id("fields-panel-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb(px(16.))
                    .children(body),
            )
    }
}
