use std::time::{Duration, Instant};

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EntityId, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use missal_core::{
    CustomFieldDraft, FIR_FIELDS, FieldMarkers, FieldReference, GlobalPlaceholder,
    GlobalPlaceholderDraft, Placeholder, PlaceholderId,
};
use missal_text::{Align, Direction, LineHeight, TextInput, TextInputEvent, label};

use crate::app::{AppPaths, check_for_updates, set_theme};
use crate::prefs::ThemeChoice;
use crate::shortcuts::*;
use crate::state::{Load, Models, run_db_in, toast_error, toast_warning};
use crate::theme::{Palette, RADIUS, RADIUS_SM};
use crate::ui::dialogs::confirm_delete;
use crate::ui::input::{Kind, field, field_input, search_field};
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::{
    SaveState, SegmentItem, UrRole, button, ghost_sm, icon_button, isolate_urdu, primary_sm,
    save_state, segmented, ur_label,
};
use crate::update::controller::{self, UpdateState, UpdateStatus};

const COLUMN_WIDTH: f32 = 680.0;
const NAV_WIDTH: f32 = 220.0;
const FILTER_THRESHOLD: usize = 12;
const FLASH: Duration = Duration::from_secs(2);

const SECTIONS: [&str; 6] = [
    "Global values",
    "Field markers",
    "Custom fields",
    "Built-in fields",
    "Appearance",
    "About",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Section {
    Globals,
    Markers,
    Customs,
}

struct GlobalRow {
    id: Option<PlaceholderId>,
    name: Entity<TextInput>,
    value: Entity<TextInput>,
    initial: (String, String),
    editing_name: bool,
}

struct CustomRow {
    id: Option<PlaceholderId>,
    name: Entity<TextInput>,
    initial: String,
    editing: bool,
}

pub struct SettingsScreen {
    models: Models,
    focus: FocusHandle,
    scroll: ScrollHandle,
    marker_open: Entity<TextInput>,
    marker_close: Entity<TextInput>,
    marker_initial: (String, String),
    globals: Vec<GlobalRow>,
    customs: Vec<CustomRow>,
    custom_filter: Entity<TextInput>,
    built_in_open: bool,
    synced: bool,
    saving: [bool; 3],
    flash: Option<(Section, u64)>,
    flash_generation: u64,
    update_status: Entity<UpdateStatus>,
    subscriptions: Vec<Subscription>,
    row_subscriptions: Vec<Subscription>,
}

fn marker_input(window: &mut Window, cx: &mut App, text: &str) -> Entity<TextInput> {
    cx.new(|cx| {
        TextInput::new(window, cx)
            .bare()
            .direction(Direction::Ltr)
            .align(Align::Center)
            .text_size(16.)
            .line_height(LineHeight::Px(24.))
            .default_text(text.to_owned())
    })
}

fn texts(row: &GlobalRow, cx: &App) -> (String, String) {
    (
        row.name.read(cx).text().trim().to_owned(),
        row.value.read(cx).text().to_owned(),
    )
}

fn blank_new(row: &GlobalRow, cx: &App) -> bool {
    row.id.is_none() && {
        let (name, value) = texts(row, cx);
        name.is_empty() && value.is_empty()
    }
}

fn ago(at: Instant) -> String {
    let seconds = at.elapsed().as_secs();
    match seconds {
        0..=44 => "just now".to_owned(),
        45..=3599 => format!("{} min ago", seconds.div_ceil(60).max(1)),
        _ => format!("{} h ago", seconds / 3600),
    }
}

impl SettingsScreen {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> SettingsScreen {
        let marker_open = marker_input(window, cx, "«");
        let marker_close = marker_input(window, cx, "»");
        let custom_filter = field_input(window, cx, Kind::SearchRtl, "تلاش", "");
        let update_status = controller::status(cx);
        let subscriptions = vec![
            cx.observe(&models.catalog, |this, _, cx| {
                if !this.is_dirty(cx) && !this.in_sync(cx) {
                    this.synced = false;
                }
                cx.notify();
            }),
            cx.observe(&update_status, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &marker_open,
                window,
                |this, input, event: &TextInputEvent, window, cx| {
                    this.marker_event(input, event, window, cx)
                },
            ),
            cx.subscribe_in(
                &marker_close,
                window,
                |this, input, event: &TextInputEvent, window, cx| {
                    this.marker_event(input, event, window, cx)
                },
            ),
            cx.subscribe(&custom_filter, |_, _, event: &TextInputEvent, cx| {
                if *event == TextInputEvent::Change {
                    cx.notify();
                }
            }),
        ];
        SettingsScreen {
            models,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            marker_open,
            marker_close,
            marker_initial: (String::new(), String::new()),
            globals: Vec::new(),
            customs: Vec::new(),
            custom_filter,
            built_in_open: false,
            synced: false,
            saving: [false; 3],
            flash: None,
            flash_generation: 0,
            update_status,
            subscriptions,
            row_subscriptions: Vec::new(),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    fn marker_event(
        &mut self,
        input: &Entity<TextInput>,
        event: &TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            TextInputEvent::Change => {
                let text = input.read(cx).text().to_owned();
                let trimmed = text.trim().to_owned();
                if trimmed != text {
                    input.update(cx, |input, cx| input.set_text(trimmed, cx));
                }
                cx.notify();
            }
            TextInputEvent::Submit | TextInputEvent::Blur => {
                if self.markers_dirty(cx) && self.marker_error(cx).is_none() {
                    self.save_markers(window, cx);
                }
            }
            TextInputEvent::Focus => {}
        }
    }

    fn marker_error(&self, cx: &App) -> Option<&'static str> {
        let open = self.marker_open.read(cx).text();
        let close = self.marker_close.read(cx).text();
        if FieldMarkers::new(open, close).is_err() {
            Some("Use 1–3 symbols, no letters or numbers.")
        } else if open == close {
            Some("The two symbols must be different.")
        } else {
            None
        }
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let catalog = self.models.catalog.read(cx);
        if !catalog.load.is_ready() {
            return;
        }
        self.synced = true;
        let markers = catalog.settings.field_markers.clone();
        let globals: Vec<GlobalPlaceholder> = catalog.globals.clone();
        let customs: Vec<Placeholder> = catalog.custom_fields().cloned().collect();
        self.marker_open
            .update(cx, |input, cx| input.set_text(markers.open(), cx));
        self.marker_close
            .update(cx, |input, cx| input.set_text(markers.close(), cx));
        self.marker_initial = (markers.open().to_owned(), markers.close().to_owned());
        self.rebuild_rows(globals, customs, window, cx);
    }

    fn rebuild_rows(
        &mut self,
        globals: Vec<GlobalPlaceholder>,
        customs: Vec<Placeholder>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.row_subscriptions.clear();
        self.globals = globals
            .into_iter()
            .map(|global| {
                self.global_row(Some(global.id), &global.label, &global.value, window, cx)
            })
            .collect();
        self.customs = customs
            .into_iter()
            .map(|field| self.custom_row(Some(field.id), &field.label, window, cx))
            .collect();
    }

    fn in_sync(&self, cx: &App) -> bool {
        let catalog = self.models.catalog.read(cx);
        let globals_match = {
            let rows: Vec<&GlobalRow> = self
                .globals
                .iter()
                .filter(|row| !blank_new(row, cx))
                .collect();
            rows.len() == catalog.globals.len()
                && catalog.globals.iter().all(|global| {
                    rows.iter().any(|row| {
                        row.id == Some(global.id)
                            && texts(row, cx) == (global.label.clone(), global.value.clone())
                    })
                })
        };
        let customs: Vec<&Placeholder> = catalog.custom_fields().collect();
        let customs_match = self.customs.len() == customs.len()
            && customs.iter().all(|field| {
                self.customs.iter().any(|row| {
                    row.id == Some(field.id) && row.name.read(cx).text().trim() == field.label
                })
            });
        globals_match && customs_match
    }

    fn global_row(
        &mut self,
        id: Option<PlaceholderId>,
        name: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> GlobalRow {
        let name_input = field_input(window, cx, Kind::Urdu, "", name);
        let value_input = field_input(window, cx, Kind::UrduAuto, "Empty", value);
        self.watch_global(&name_input, true, window, cx);
        self.watch_global(&value_input, false, window, cx);
        GlobalRow {
            id,
            name: name_input,
            value: value_input,
            initial: (name.to_owned(), value.to_owned()),
            editing_name: id.is_none(),
        }
    }

    fn watch_global(
        &mut self,
        input: &Entity<TextInput>,
        is_name: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription = cx.subscribe_in(
            input,
            window,
            move |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Change => cx.notify(),
                TextInputEvent::Submit | TextInputEvent::Blur => {
                    if is_name {
                        this.finish_name_editing(window, cx);
                    }
                    this.autosave_globals(window, cx);
                }
                TextInputEvent::Focus => {}
            },
        );
        self.row_subscriptions.push(subscription);
    }

    fn finish_name_editing(&mut self, window: &Window, cx: &mut Context<Self>) {
        for row in &mut self.globals {
            if row.id.is_some() && !row.name.focus_handle(cx).is_focused(window) {
                row.editing_name = false;
            }
        }
        cx.notify();
    }

    fn custom_row(
        &mut self,
        id: Option<PlaceholderId>,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> CustomRow {
        let input = field_input(window, cx, Kind::Urdu, "Field name", name);
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Change => cx.notify(),
                TextInputEvent::Submit | TextInputEvent::Blur => {
                    this.finish_custom_editing(window, cx)
                }
                TextInputEvent::Focus => {}
            },
        );
        self.row_subscriptions.push(subscription);
        CustomRow {
            id,
            name: input,
            initial: name.to_owned(),
            editing: id.is_none(),
        }
    }

    fn finish_custom_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = |row: &CustomRow, window: &Window, cx: &App| {
            row.name.focus_handle(cx).is_focused(window)
        };
        let mut index = 0;
        while index < self.customs.len() {
            let row = &self.customs[index];
            if focused(row, window, cx) {
                index += 1;
                continue;
            }
            let name = row.name.read(cx).text().trim().to_owned();
            if row.id.is_none() && name.is_empty() {
                self.customs.remove(index);
                continue;
            }
            if row.id.is_some() && (name.is_empty() || name == row.initial) {
                let initial = row.initial.clone();
                let input = row.name.clone();
                input.update(cx, |input, cx| input.set_text(initial, cx));
                self.customs[index].editing = false;
            }
            index += 1;
        }
        cx.notify();
        if self.customs_dirty(cx) && self.custom_errors(cx).iter().all(Option::is_none) {
            self.save_customs(window, cx);
        }
    }

    pub fn markers_dirty(&self, cx: &App) -> bool {
        self.synced
            && (
                self.marker_open.read(cx).text().to_owned(),
                self.marker_close.read(cx).text().to_owned(),
            ) != self.marker_initial
    }

    pub fn globals_dirty(&self, cx: &App) -> bool {
        self.globals
            .iter()
            .any(|row| !blank_new(row, cx) && (row.id.is_none() || texts(row, cx) != row.initial))
    }

    pub fn customs_dirty(&self, cx: &App) -> bool {
        self.customs.iter().any(|row| {
            let name = row.name.read(cx).text().trim().to_owned();
            if row.id.is_none() {
                !name.is_empty()
            } else {
                !name.is_empty() && name != row.initial
            }
        })
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.markers_dirty(cx) || self.globals_dirty(cx) || self.customs_dirty(cx)
    }

    pub fn global_inputs(&self) -> Vec<(Entity<TextInput>, Entity<TextInput>)> {
        self.globals
            .iter()
            .map(|row| (row.name.clone(), row.value.clone()))
            .collect()
    }

    pub fn marker_inputs(&self) -> (Entity<TextInput>, Entity<TextInput>) {
        (self.marker_open.clone(), self.marker_close.clone())
    }

    pub fn built_in_open(&self) -> bool {
        self.built_in_open
    }

    pub fn add_global(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = self.global_row(None, "", "", window, cx);
        row.name.update(cx, |input, cx| input.focus(window, cx));
        self.globals.push(row);
        cx.notify();
    }

    pub fn add_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = self.custom_row(None, "", window, cx);
        row.name.update(cx, |input, cx| input.focus(window, cx));
        self.customs.insert(0, row);
        self.scroll_to(2);
        cx.notify();
    }

    fn scroll_to(&self, section: usize) {
        let Some(bounds) = self.scroll.bounds_for_item(section) else {
            self.scroll.scroll_to_item(section);
            return;
        };
        let top = self.scroll.bounds().origin.y + px(20.);
        let maximum = self.scroll.max_offset().y;
        let target = (top - bounds.origin.y).clamp(-maximum, px(0.));
        self.scroll
            .set_offset(gpui_kit::point(self.scroll.offset().x, target));
    }

    fn rename_global(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.globals.get_mut(index) else {
            return;
        };
        row.editing_name = true;
        let input = row.name.clone();
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            let length = input.text().len();
            input.select(0..length, cx);
        });
        cx.notify();
    }

    fn rename_custom(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.customs.get_mut(index) else {
            return;
        };
        row.editing = true;
        let input = row.name.clone();
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            let length = input.text().len();
            input.select(0..length, cx);
        });
        cx.notify();
    }

    fn rename_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let index = self.globals.iter().position(|row| {
            row.value.focus_handle(cx).is_focused(window)
                || row.name.focus_handle(cx).is_focused(window)
        });
        if let Some(index) = index {
            self.rename_global(index, window, cx);
        }
    }

    fn global_errors(&self, cx: &App) -> Vec<Option<&'static str>> {
        let names: Vec<String> = self
            .globals
            .iter()
            .map(|row| texts(row, cx).0.to_lowercase())
            .collect();
        self.globals
            .iter()
            .enumerate()
            .map(|(index, row)| {
                if blank_new(row, cx) {
                    return None;
                }
                let (name, _) = texts(row, cx);
                if name.is_empty() {
                    Some("Enter a name.")
                } else if names
                    .iter()
                    .enumerate()
                    .any(|(other, candidate)| other != index && *candidate == names[index])
                {
                    Some("This name is already used.")
                } else {
                    None
                }
            })
            .collect()
    }

    fn custom_errors(&self, cx: &App) -> Vec<Option<&'static str>> {
        let catalog = self.models.catalog.read(cx);
        let taken: Vec<String> = self
            .customs
            .iter()
            .map(|row| row.name.read(cx).text().trim().to_lowercase())
            .collect();
        let builtin: Vec<String> = FIR_FIELDS
            .iter()
            .map(|field| field.label.to_lowercase())
            .chain(
                catalog
                    .placeholders
                    .iter()
                    .filter(|field| !matches!(field.source, missal_core::PlaceholderSource::Custom))
                    .map(|field| field.label.to_lowercase()),
            )
            .collect();
        self.customs
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let name = taken[index].as_str();
                if name.is_empty() {
                    return None;
                }
                if row.id.is_some() && name == row.initial.to_lowercase() {
                    return None;
                }
                if builtin.iter().any(|label| label == name)
                    || taken
                        .iter()
                        .enumerate()
                        .any(|(other, candidate)| other != index && candidate == name)
                {
                    Some("This name is already used.")
                } else {
                    None
                }
            })
            .collect()
    }

    fn flash_saved(&mut self, section: Section, cx: &mut Context<Self>) {
        self.flash_generation += 1;
        let generation = self.flash_generation;
        self.flash = Some((section, generation));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH).await;
            this.update(cx, |this, cx| {
                if this.flash.is_some_and(|(_, current)| current == generation) {
                    this.flash = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn save_markers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.marker_open.read(cx).text().to_owned();
        let close = self.marker_close.read(cx).text().to_owned();
        if self.marker_error(cx).is_some() {
            cx.notify();
            return;
        }
        let Ok(markers) = FieldMarkers::new(&open, &close) else {
            return;
        };
        self.saving[0] = true;
        run_db_in(
            window,
            cx,
            move |store| store.save_field_markers(&markers),
            |this, result, window, cx| {
                this.saving[0] = false;
                match result {
                    Ok(settings) => {
                        this.marker_initial = (
                            settings.field_markers.open().to_owned(),
                            settings.field_markers.close().to_owned(),
                        );
                        this.models
                            .catalog
                            .update(cx, |catalog, cx| catalog.set_settings(settings, cx));
                        this.flash_saved(Section::Markers, cx);
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    fn autosave_globals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving[1] || !self.globals_dirty(cx) {
            return;
        }
        if self.global_errors(cx).iter().any(Option::is_some) {
            return;
        }
        self.save_globals(window, cx);
    }

    pub fn save_globals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.global_errors(cx).iter().any(Option::is_some) {
            toast_warning(window, cx, "Fix the highlighted global values first");
            return;
        }
        let mut drafts = Vec::new();
        let mut captured: Vec<(EntityId, (String, String))> = Vec::new();
        for row in &self.globals {
            if blank_new(row, cx) {
                continue;
            }
            let (name, value) = texts(row, cx);
            match row.id {
                Some(id) if (name.clone(), value.clone()) != row.initial => {
                    drafts.push(GlobalPlaceholderDraft::Existing {
                        id,
                        label: name.clone(),
                        value: value.clone(),
                    });
                    captured.push((row.name.entity_id(), (name, value)));
                }
                None => {
                    drafts.push(GlobalPlaceholderDraft::New {
                        label: name.clone(),
                        value: value.clone(),
                    });
                    captured.push((row.name.entity_id(), (name, value)));
                }
                Some(_) => {}
            }
        }
        if drafts.is_empty() {
            return;
        }
        self.saving[1] = true;
        cx.notify();
        run_db_in(
            window,
            cx,
            move |store| {
                store.save_global_placeholders(&drafts)?;
                Ok((
                    store.settings()?,
                    store.placeholders()?,
                    store.global_placeholders()?,
                ))
            },
            move |this, result, window, cx| {
                this.saving[1] = false;
                match result {
                    Ok((settings, placeholders, globals)) => {
                        for (entity, saved) in &captured {
                            if let Some(row) = this
                                .globals
                                .iter_mut()
                                .find(|row| row.name.entity_id() == *entity)
                            {
                                if row.id.is_none() {
                                    row.id = globals
                                        .iter()
                                        .find(|global| global.label == saved.0)
                                        .map(|global| global.id);
                                }
                                row.initial = saved.clone();
                            }
                        }
                        this.models.catalog.update(cx, |catalog, cx| {
                            catalog.replace_all(settings, placeholders, globals, cx)
                        });
                        this.flash_saved(Section::Globals, cx);
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    pub fn save_customs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving[2] {
            return;
        }
        let mut drafts = Vec::new();
        let mut captured: Vec<(EntityId, String)> = Vec::new();
        for row in &self.customs {
            let name = row.name.read(cx).text().trim().to_owned();
            if name.is_empty() {
                continue;
            }
            match row.id {
                Some(id) if name != row.initial => {
                    drafts.push(CustomFieldDraft::Rename {
                        id,
                        label: name.clone(),
                    });
                    captured.push((row.name.entity_id(), name));
                }
                None => {
                    drafts.push(CustomFieldDraft::New {
                        label: name.clone(),
                    });
                    captured.push((row.name.entity_id(), name));
                }
                Some(_) => {}
            }
        }
        if drafts.is_empty() {
            return;
        }
        self.saving[2] = true;
        cx.notify();
        run_db_in(
            window,
            cx,
            move |store| {
                store.save_custom_fields(&drafts)?;
                Ok((
                    store.settings()?,
                    store.placeholders()?,
                    store.global_placeholders()?,
                ))
            },
            move |this, result, window, cx| {
                this.saving[2] = false;
                match result {
                    Ok((settings, placeholders, globals)) => {
                        for (entity, saved) in &captured {
                            if let Some(row) = this
                                .customs
                                .iter_mut()
                                .find(|row| row.name.entity_id() == *entity)
                            {
                                if row.id.is_none() {
                                    row.id = placeholders
                                        .iter()
                                        .find(|field| {
                                            field.label == *saved
                                                && matches!(
                                                    field.source,
                                                    missal_core::PlaceholderSource::Custom
                                                )
                                        })
                                        .map(|field| field.id);
                                }
                                row.initial = saved.clone();
                                row.editing = false;
                            }
                        }
                        this.models.catalog.update(cx, |catalog, cx| {
                            catalog.replace_all(settings, placeholders, globals, cx)
                        });
                        this.flash_saved(Section::Customs, cx);
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    fn save_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.markers_dirty(cx) {
            self.save_markers(window, cx);
        }
        if self.globals_dirty(cx) {
            self.save_globals(window, cx);
        }
        if self.customs_dirty(cx) {
            self.save_customs(window, cx);
        }
    }

    fn remove_global_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.globals.get(index) else {
            return;
        };
        match row.id {
            None => {
                self.globals.remove(index);
                cx.notify();
            }
            Some(id) => {
                let name = row.initial.0.clone();
                self.request_delete_field(id, name, window, cx);
            }
        }
    }

    fn request_delete_field(
        &mut self,
        id: PlaceholderId,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        run_db_in(
            window,
            cx,
            move |store| {
                let mut used = 0usize;
                for summary in store.templates()? {
                    let record = store.template(summary.id)?;
                    if record
                        .document
                        .projections()
                        .field_references
                        .contains(&FieldReference::Catalog(id))
                    {
                        used += 1;
                    }
                }
                Ok(used)
            },
            move |this, result, window, cx| {
                let used = match result {
                    Ok(used) => used,
                    Err(error) => {
                        toast_error(window, cx, error.message());
                        return;
                    }
                };
                let usage = match used {
                    0 => "No template uses it.".to_owned(),
                    1 => "Used in 1 template.".to_owned(),
                    count => format!("Used in {count} templates."),
                };
                let weak = cx.entity().downgrade();
                confirm_delete(
                    window,
                    cx,
                    format!("Delete «{}»?", isolate_urdu(&name)),
                    format!(
                        "{usage} Its values in every FIR are deleted too. This can't be undone."
                    ),
                    "Delete field",
                    move |window, cx| {
                        weak.update(cx, |this, cx| this.delete_field(id, window, cx))
                            .ok();
                    },
                );
                let _ = this;
            },
        );
    }

    fn delete_field(&mut self, id: PlaceholderId, window: &mut Window, cx: &mut Context<Self>) {
        run_db_in(
            window,
            cx,
            move |store| {
                store.remove_placeholder(id)?;
                Ok((
                    store.settings()?,
                    store.placeholders()?,
                    store.global_placeholders()?,
                ))
            },
            move |this, result, window, cx| {
                match result {
                    Ok((settings, placeholders, globals)) => {
                        this.globals.retain(|row| row.id != Some(id));
                        this.customs.retain(|row| row.id != Some(id));
                        this.models.catalog.update(cx, |catalog, cx| {
                            catalog.replace_all(settings, placeholders, globals, cx)
                        });
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    fn active_section(&self) -> usize {
        let top = self.scroll.bounds().origin.y;
        let offset = self.scroll.offset().y;
        if offset.abs() > px(1.) && offset <= -self.scroll.max_offset().y + px(1.) {
            return SECTIONS.len() - 1;
        }
        (0..SECTIONS.len())
            .rev()
            .find(|index| {
                self.scroll
                    .bounds_for_item(*index)
                    .is_some_and(|bounds| bounds.origin.y + offset <= top + px(48.))
            })
            .unwrap_or(0)
    }

    fn section(
        &self,
        palette: &Palette,
        index: usize,
        count: Option<usize>,
        flash: bool,
        description: &'static str,
        body: impl IntoElement,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let title = SECTIONS[index];
        let last = index == SECTIONS.len() - 1;
        v_flex()
            .id(("settings-section", index))
            .w(px(COLUMN_WIDTH))
            .ml(px(24.))
            .flex_shrink_0()
            .pb(px(28.))
            .when(!last, |this| {
                this.mb(px(28.)).border_b_1().border_color(palette.line)
            })
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .line_height(px(24.))
                            .font_semibold()
                            .child(title),
                    )
                    .children(count.map(|count| {
                        div()
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(palette.ink_muted)
                            .child(count.to_string())
                    }))
                    .child(div().flex_1())
                    .children(flash.then(|| save_state(palette, SaveState::Saved))),
            )
            .child(
                div()
                    .mt(px(2.))
                    .mb(px(14.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(palette.ink_muted)
                    .child(description),
            )
            .child(body)
    }

    fn error_line(&self, palette: &Palette, text: &'static str) -> gpui_kit::Div {
        div()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(palette.danger_text)
            .child(text)
    }

    fn globals_body(&mut self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let load = self.models.catalog.read(cx).load.clone();
        match load {
            Load::Loading => v_flex()
                .gap(px(8.))
                .children((0..4usize).map(|index| {
                    div()
                        .id(("global-skeleton", index))
                        .h(px(36.))
                        .rounded(px(RADIUS))
                        .bg(palette.hover)
                }))
                .into_any_element(),
            Load::Failed(message) => v_flex()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(13.))
                        .child("Could not load global values"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.ink_faint)
                        .child(message.clone()),
                )
                .child(
                    h_flex().child(
                        button("retry-settings", "Try again")
                            .outline()
                            .sm()
                            .on_click({
                                let catalog = self.models.catalog.clone();
                                move |_, _, cx| catalog.update(cx, |catalog, cx| catalog.reload(cx))
                            }),
                    ),
                )
                .into_any_element(),
            Load::Ready => {
                let errors = self.global_errors(cx);
                let palette = *palette;
                v_flex()
                    .children(self.globals.iter().enumerate().map(|(index, row)| {
                        let error = errors.get(index).copied().flatten();
                        let name_cell: AnyElement = if row.editing_name {
                            field(("global-name", index), &row.name, Kind::Urdu)
                                .width(px(150.))
                                .error(error.is_some())
                                .into_any_element()
                        } else {
                            div()
                                .id(("global-name-label", index))
                                .test_support()
                                .w(px(150.))
                                .h(px(36.))
                                .flex_none()
                                .px(px(10.))
                                .rounded(px(RADIUS))
                                .flex()
                                .items_center()
                                .cursor_text()
                                .hover(move |style| style.bg(palette.hover))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.rename_global(index, window, cx)
                                }))
                                .child(
                                    div().flex_1().min_w_0().mt(px(-2.)).child(
                                        ur_label(row.initial.0.clone(), UrRole::Body)
                                            .text_color(palette.ink)
                                            .truncate(),
                                    ),
                                )
                                .into_any_element()
                        };
                        v_flex()
                            .id(("global-row", index))
                            .py(px(4.))
                            .child(
                                h_flex()
                                    .flex_row_reverse()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(name_cell)
                                    .child(
                                        div().flex_1().min_w_0().child(
                                            field(
                                                ("global-value", index),
                                                &row.value,
                                                Kind::UrduAuto,
                                            )
                                            .error(error.is_some() && row.editing_name),
                                        ),
                                    )
                                    .child(
                                        icon_button(
                                            ("global-delete", index),
                                            IconName::Trash,
                                            "Delete",
                                        )
                                        .dense()
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.remove_global_row(index, window, cx)
                                            }),
                                        ),
                                    ),
                            )
                            .children(error.map(|error| {
                                h_flex()
                                    .flex_row_reverse()
                                    .pt(px(2.))
                                    .child(div().w(px(150.)).flex_none())
                                    .child(self.error_line(&palette, error))
                            }))
                    }))
                    .child(
                        h_flex().flex_row_reverse().pt(px(8.)).child(
                            ghost_sm("add-global", "Add global value")
                                .icon(IconName::Plus)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.add_global(window, cx)),
                                ),
                        ),
                    )
                    .into_any_element()
            }
        }
    }

    fn markers_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let open = self.marker_open.read(cx).text().to_owned();
        let close = self.marker_close.read(cx).text().to_owned();
        let preview = FieldMarkers::new(&open, &close).unwrap_or_default();
        let error = self.marker_error(cx);
        v_flex()
            .gap(px(6.))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(10.))
                    .child(field("marker-open", &self.marker_open, Kind::Urdu).width(px(56.)))
                    .child(
                        div()
                            .mt(px(-2.))
                            .text_color(palette.ink_muted)
                            .child(ur_label("نام", UrRole::Body)),
                    )
                    .child(field("marker-close", &self.marker_close, Kind::Urdu).width(px(56.)))
                    .child(div().w(px(1.)).h(px(24.)).mx(px(8.)).bg(palette.line))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(palette.ink_muted)
                            .child("Example"),
                    )
                    .child(
                        div()
                            .px(px(4.))
                            .rounded(px(RADIUS_SM))
                            .bg(palette.field_wash)
                            .text_color(palette.accent)
                            .child(
                                ur_label(preview.token("جرم"), UrRole::Body)
                                    .truncate()
                                    .mt(px(-2.)),
                            ),
                    ),
            )
            .children(error.map(|error| self.error_line(palette, error)))
            .into_any_element()
    }

    fn customs_body(&mut self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let errors = self.custom_errors(cx);
        let filter = crate::state::rows::fold_for_search(self.custom_filter.read(cx).text())
            .trim()
            .to_owned();
        let show_filter = self.customs.len() > FILTER_THRESHOLD;
        let visible: Vec<usize> = (0..self.customs.len())
            .filter(|index| {
                let row = &self.customs[*index];
                filter.is_empty()
                    || row.editing
                    || crate::state::rows::fold_for_search(&row.initial).contains(&filter)
            })
            .collect();
        v_flex()
            .children(show_filter.then(|| {
                div().pb(px(8.)).child(
                    search_field("custom-filter-field", &self.custom_filter, Kind::SearchRtl)
                        .width(px(300.)),
                )
            }))
            .child(v_flex().children(visible.iter().map(|&index| {
                let row = &self.customs[index];
                let error = errors.get(index).copied().flatten();
                let content: AnyElement = if row.editing {
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            field(("custom-name", index), &row.name, Kind::Urdu)
                                .error(error.is_some()),
                        )
                        .into_any_element()
                } else {
                    div()
                        .flex_1()
                        .min_w_0()
                        .mt(px(-2.))
                        .child(
                            ur_label(row.initial.clone(), UrRole::Body)
                                .text_color(palette.ink)
                                .truncate(),
                        )
                        .into_any_element()
                };
                let id = row.id;
                let name = row.initial.clone();
                let weak = cx.entity().downgrade();
                v_flex()
                    .id(("custom-row", index))
                    .test_support()
                    .border_b_1()
                    .border_color(palette.line)
                    .child(
                        h_flex()
                            .flex_row_reverse()
                            .h(px(40.))
                            .items_center()
                            .gap(px(10.))
                            .px(px(8.))
                            .child(content)
                            .child(
                                Button::new(("custom-more", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Ellipsis)
                                    .tooltip("More actions")
                                    .dropdown_menu(move |menu, _, _| {
                                        let rename = weak.clone();
                                        let delete = weak.clone();
                                        let name = name.clone();
                                        menu.item(
                                            PopupMenuItem::new("Rename")
                                                .icon(IconName::Pencil)
                                                .disabled(id.is_none())
                                                .on_click(move |_, window, cx| {
                                                    rename
                                                        .update(cx, |this, cx| {
                                                            this.rename_custom(index, window, cx)
                                                        })
                                                        .ok();
                                                }),
                                        )
                                        .item(PopupMenuItem::separator())
                                        .item(
                                            PopupMenuItem::new("Delete…")
                                                .icon(IconName::Trash)
                                                .disabled(id.is_none())
                                                .on_click(move |_, window, cx| {
                                                    let name = name.clone();
                                                    delete
                                                        .update(cx, |this, cx| {
                                                            if let Some(id) = id {
                                                                this.request_delete_field(
                                                                    id, name, window, cx,
                                                                )
                                                            }
                                                        })
                                                        .ok();
                                                }),
                                        )
                                    }),
                            ),
                    )
                    .children(error.map(|error| {
                        h_flex()
                            .flex_row_reverse()
                            .px(px(8.))
                            .pb(px(6.))
                            .child(self.error_line(&palette, error))
                    }))
            })))
            .child(
                h_flex().flex_row_reverse().pt(px(8.)).child(
                    ghost_sm("add-custom", "Add custom field")
                        .icon(IconName::Plus)
                        .on_click(cx.listener(|this, _, window, cx| this.add_custom(window, cx))),
                ),
            )
            .into_any_element()
    }

    fn built_in_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let markers = self.models.catalog.read(cx).settings.field_markers.clone();
        let count = FIR_FIELDS.len();
        let toggle = ghost_sm(
            "built-in-toggle",
            if self.built_in_open {
                "Hide built-in fields".to_owned()
            } else {
                format!("Show {count} built-in fields")
            },
        )
        .icon(if self.built_in_open {
            IconName::ChevronUp
        } else {
            IconName::ChevronDown
        })
        .on_click(cx.listener(|this, _, _, cx| {
            this.built_in_open = !this.built_in_open;
            cx.notify();
        }));
        let grid = self.built_in_open.then(|| {
            div()
                .mt(px(8.))
                .flex()
                .flex_row_reverse()
                .flex_wrap()
                .children(FIR_FIELDS.iter().enumerate().map(|(index, field)| {
                    let group = format!("built-in-{index}");
                    h_flex()
                        .id(("built-in", index))
                        .group(group.clone())
                        .w(px(COLUMN_WIDTH / 2.))
                        .h(px(32.))
                        .flex_row_reverse()
                        .items_center()
                        .gap(px(10.))
                        .px(px(8.))
                        .child(
                            div().mt(px(-2.)).child(
                                ur_label(field.label, UrRole::Label)
                                    .text_color(palette.ink)
                                    .truncate(),
                            ),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .opacity(0.)
                                .group_hover(group, |style| style.opacity(1.))
                                .text_color(palette.ink_faint)
                                .mt(px(-2.))
                                .child(
                                    ur_label(markers.token(field.label), UrRole::Label).truncate(),
                                ),
                        )
                }))
        });
        v_flex()
            .child(h_flex().child(toggle))
            .children(grid)
            .into_any_element()
    }

    fn appearance_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.models.prefs.read(cx).read().theme;
        let items = ThemeChoice::ALL
            .into_iter()
            .map(|choice| {
                let id = match choice {
                    ThemeChoice::System => "theme-system",
                    ThemeChoice::Light => "theme-light",
                    ThemeChoice::Dark => "theme-dark",
                };
                SegmentItem::new(id, choice.title(), move |_, window, cx| {
                    set_theme(choice, window, cx)
                })
            })
            .collect();
        let selected = ThemeChoice::ALL
            .iter()
            .position(|choice| *choice == theme)
            .unwrap_or(0);
        h_flex()
            .child(segmented(palette, items, selected))
            .into_any_element()
    }

    fn about_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let (folder, updates_enabled) = match cx.try_global::<AppPaths>() {
            Some(paths) => (
                Some(paths.directory.clone()),
                controller::enabled(!paths.is_default),
            ),
            None => (None, false),
        };
        let state = self.update_status.read(cx).state().clone();
        let checking = matches!(state, UpdateState::Checking);
        let status: AnyElement = if !updates_enabled {
            div()
                .child("Automatic updates are off in this build.")
                .into_any_element()
        } else {
            match &state {
                UpdateState::Idle => div().child("Not checked yet.").into_any_element(),
                UpdateState::Checking => div().child("Checking…").into_any_element(),
                UpdateState::UpToDate { at } => div()
                    .child(format!("Up to date. Checked {}.", ago(*at)))
                    .into_any_element(),
                UpdateState::Ready(ready) => div()
                    .child(format!("Version {} is ready to install.", ready.version()))
                    .into_any_element(),
                UpdateState::Failed { at, .. } => div()
                    .text_color(palette.danger_text)
                    .child(format!("Could not check for updates ({}).", ago(*at)))
                    .into_any_element(),
            }
        };
        let row = |name: &'static str, value: AnyElement| {
            h_flex()
                .min_h(px(28.))
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .w(px(100.))
                        .flex_none()
                        .text_size(px(13.))
                        .text_color(palette.ink_muted)
                        .child(name),
                )
                .child(div().flex_1().min_w_0().text_size(px(13.)).child(value))
        };
        let path_text = folder
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Unavailable".to_owned());
        v_flex()
            .gap(px(4.))
            .child(row(
                "Version",
                div().child(env!("CARGO_PKG_VERSION")).into_any_element(),
            ))
            .child(row(
                "Data folder",
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(palette.ink_muted)
                            .child(label(path_text).ltr().truncate()),
                    )
                    .children(folder.clone().map(|path| {
                        ghost_sm("open-data-folder", "Open folder").on_click(move |_, _, cx| {
                            cx.reveal_path(&path);
                        })
                    }))
                    .into_any_element(),
            ))
            .child(row(
                "Updates",
                h_flex()
                    .gap(px(12.))
                    .items_center()
                    .child(div().text_color(palette.ink_muted).child(status))
                    .into_any_element(),
            ))
            .child(
                h_flex()
                    .mt(px(8.))
                    .gap(px(8.))
                    .child(
                        button("check-updates", "Check for updates")
                            .outline()
                            .sm()
                            .icon(IconName::RefreshCw)
                            .disabled(!updates_enabled || checking)
                            .on_click(|_, window, cx| check_for_updates(window, cx)),
                    )
                    .children(match &state {
                        UpdateState::Ready(ready) if updates_enabled => {
                            let ready = ready.clone();
                            Some(primary_sm("restart-update", "Restart now").on_click(
                                move |_, window, cx| controller::restart(ready.clone(), window, cx),
                            ))
                        }
                        _ => None,
                    }),
            )
            .into_any_element()
    }

    fn nav(&self, palette: &Palette, cx: &mut Context<Self>) -> gpui_kit::Div {
        let palette = *palette;
        let active = self.active_section();
        v_flex()
            .w(px(NAV_WIDTH))
            .flex_none()
            .h_full()
            .px(px(12.))
            .pt(px(20.))
            .gap(px(2.))
            .children(SECTIONS.iter().enumerate().map(|(index, title)| {
                let on = index == active;
                div()
                    .id(("settings-nav", index))
                    .test_support()
                    .h(px(32.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS))
                    .text_size(px(13.))
                    .font_medium()
                    .cursor_pointer()
                    .text_color(if on { palette.ink } else { palette.ink_muted })
                    .when(on, |this| this.bg(palette.hover))
                    .when(!on, |this| {
                        this.hover(move |style| style.bg(palette.hover).text_color(palette.ink))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.scroll_to(index);
                        cx.notify();
                    }))
                    .child(*title)
            }))
    }
}

impl Render for SettingsScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let load = self.models.catalog.read(cx).load.clone();
        if load.is_ready() && !self.synced {
            self.sync(window, cx);
        }
        let _ = &self.subscriptions;
        let flash = self.flash.map(|(section, _)| section);
        let custom_count = self.customs.len();
        let globals_body = self.globals_body(&palette, cx);
        let markers_body = self.markers_body(&palette, cx);
        let customs_body = self.customs_body(&palette, cx);
        let built_in_body = self.built_in_body(&palette, cx);
        let appearance_body = self.appearance_body(&palette, cx);
        let about_body = self.about_body(&palette, cx);
        let nav = self.nav(&palette, cx);
        let sections = [
            self.section(
                &palette,
                0,
                None,
                flash == Some(Section::Globals),
                "Filled into every FIR's documents, such as the police station name.",
                globals_body,
            ),
            self.section(
                &palette,
                1,
                None,
                flash == Some(Section::Markers),
                "The symbols around a field name when you type it into a document.",
                markers_body,
            ),
            self.section(
                &palette,
                2,
                Some(custom_count),
                flash == Some(Section::Customs),
                "Extra fields you fill per FIR. Renaming keeps templates linked.",
                customs_body,
            ),
            self.section(
                &palette,
                3,
                None,
                false,
                "Filled from each FIR's details. Hover a name to see how it is typed.",
                built_in_body,
            ),
            self.section(
                &palette,
                4,
                None,
                false,
                "System follows your operating system.",
                appearance_body,
            ),
            self.section(
                &palette,
                5,
                None,
                false,
                "Missal keeps everything on this computer.",
                about_body,
            ),
        ];
        h_flex()
            .id("settings")
            .key_context("Settings")
            .track_focus(&self.focus)
            .size_full()
            .bg(palette.surface)
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save_all(window, cx)))
            .on_action(
                cx.listener(|this, _: &RenameTemplate, window, cx| this.rename_focused(window, cx)),
            )
            .child(nav)
            .child(
                v_flex()
                    .id("settings-body")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .pt(px(20.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
                    .children(sections)
                    .child(div().h(px(420.)).flex_none()),
            )
    }
}

impl HeaderSource for SettingsScreen {
    fn header(&mut self, _: &mut Window, _: &mut Context<Self>) -> HeaderContent {
        HeaderContent::new("Settings")
    }
}
