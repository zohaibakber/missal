use crate::ui::ScrollArea as _;
use std::time::{Duration, Instant};

use crate::ui::popup_menu::{MenuEntry, menu};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::avatar::Avatar;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EntityId, FocusHandle, Focusable, FontWeight,
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
use crate::theme::{Palette, RADIUS, RADIUS_LG, RADIUS_SM};
use crate::ui::dialogs::confirm_delete;
use crate::ui::input::{Kind, field, field_input, search_field};
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::{
    BadgeVariant, Intent, SaveState, UrRole, alert, alert_description, alert_title, badge, button,
    card_description, empty_state_body, field_error, frame, frame_panel, icon_button, isolate_urdu,
    kbd_group, primary_sm, save_state, separator_vertical, sidebar_group, sidebar_group_label,
    sidebar_menu_button, sized_icon, skeleton, small_icon, switch, ur_label,
};
use crate::update::controller::{self, UpdateState, UpdateStatus};

const NAME_COLUMN: f32 = 180.;
const COLUMN_WIDTH: f32 = 720.0;
const NAV_WIDTH: f32 = 224.0;
const FILTER_THRESHOLD: usize = 12;
const FLASH: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Globals,
    Customs,
    BuiltIn,
    Markers,
    Appearance,
    About,
    Account,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Globals => "Global values",
            Page::Customs => "Custom fields",
            Page::BuiltIn => "Built-in fields",
            Page::Markers => "Field markers",
            Page::Appearance => "Appearance",
            Page::About => "About",
            Page::Account => "Account",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Page::Globals => "Filled into every FIR's documents, such as the police station name.",
            Page::Customs => {
                "Extra fields you fill in for each FIR. Renaming one keeps its templates linked."
            }
            Page::BuiltIn => {
                "Filled from each FIR's details. Type a field's name between markers to use it in a template."
            }
            Page::Markers => "The symbols around a field name when you type it into a document.",
            Page::Appearance => "Choose how Missal looks on this computer.",
            Page::About => "Missal keeps everything on this computer.",
            Page::Account => "Your Missal account and licence.",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Page::Globals => IconName::Globe,
            Page::Customs => IconName::TextCursorInput,
            Page::BuiltIn => IconName::Braces,
            Page::Markers => IconName::Quote,
            Page::Appearance => IconName::SunMoon,
            Page::About => IconName::Info,
            Page::Account => IconName::CircleUser,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Page::Globals => "settings-nav-globals",
            Page::Customs => "settings-nav-customs",
            Page::BuiltIn => "settings-nav-built-in",
            Page::Markers => "settings-nav-markers",
            Page::Appearance => "settings-nav-appearance",
            Page::About => "settings-nav-about",
            Page::Account => "settings-nav-account",
        }
    }
}

const NAV_GROUPS: [(&str, &[Page]); 2] = [
    (
        "Documents",
        &[Page::Globals, Page::Customs, Page::BuiltIn, Page::Markers],
    ),
    ("General", &[Page::Account, Page::Appearance, Page::About]),
];

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
    page: Page,
    synced: bool,
    saving: [bool; 3],
    flash: Option<(Page, u64)>,
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
            page: Page::Globals,
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
        let value_input = field_input(window, cx, Kind::UrduAuto, "", value);
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

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page != page {
            self.page = page;
            self.scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        }
        cx.notify();
    }

    pub fn add_global(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = self.global_row(None, "", "", window, cx);
        row.name.update(cx, |input, cx| input.focus(window, cx));
        self.globals.push(row);
        self.show(Page::Globals, cx);
    }

    pub fn add_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = self.custom_row(None, "", window, cx);
        row.name.update(cx, |input, cx| input.focus(window, cx));
        self.customs.insert(0, row);
        self.show(Page::Customs, cx);
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
                    .placeholders()
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

    fn flash_saved(&mut self, section: Page, cx: &mut Context<Self>) {
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
                        this.flash_saved(Page::Markers, cx);
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
                        this.flash_saved(Page::Globals, cx);
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
                        this.flash_saved(Page::Customs, cx);
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

    fn reset_markers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let markers = FieldMarkers::default();
        self.marker_open
            .update(cx, |input, cx| input.set_text(markers.open(), cx));
        self.marker_close
            .update(cx, |input, cx| input.set_text(markers.close(), cx));
        if self.markers_dirty(cx) {
            self.save_markers(window, cx);
        }
    }

    fn page_header(&self, palette: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page;
        let saved = self.flash.is_some_and(|(section, _)| section == page);
        let action: Option<AnyElement> = match page {
            Page::Globals if self.models.catalog.read(cx).load.is_ready() => Some(
                button("add-global", "Add value")
                    .outline()
                    .sm()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|this, _, window, cx| this.add_global(window, cx)))
                    .into_any_element(),
            ),
            Page::Customs => Some(
                button("add-custom", "Add field")
                    .outline()
                    .sm()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|this, _, window, cx| this.add_custom(window, cx)))
                    .into_any_element(),
            ),
            _ => None,
        };
        h_flex()
            .items_start()
            .gap(px(16.))
            .pb(px(24.))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .text_size(px(20.))
                            .line_height(px(28.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.ink)
                            .child(page.title())
                            .children(saved.then(|| save_state(SaveState::Saved))),
                    )
                    .child(card_description().child(page.description())),
            )
            .children(action.map(|action| div().flex_none().pt(px(2.)).child(action)))
    }

    fn globals_body(&mut self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let load = self.models.catalog.read(cx).load.clone();
        match load {
            Load::Loading => frame()
                .child(
                    frame_panel().p(px(12.)).child(
                        v_flex()
                            .gap(px(8.))
                            .children((0..4usize).map(|_| skeleton().h(px(36.)).rounded(px(RADIUS)))),
                    ),
                )
                .into_any_element(),
            Load::Failed(message) => alert(Intent::Danger)
                .child(alert_title().child("Could not load global values"))
                .child(alert_description().child(message.clone()))
                .child(
                    h_flex().mt(px(8.)).child(
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
            Load::Ready if self.globals.is_empty() => frame()
                .child(frame_panel().child(empty_state_body(
                    IconName::Globe,
                    "No global values yet",
                    Some("Add the police station, district and officer names once, and every document fills them in.".into()),
                    Some(
                        button("add-global-empty", "Add value")
                            .outline()
                            .sm()
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| this.add_global(window, cx)))
                            .into_any_element(),
                    ),
                ).py(px(40.))))
                .into_any_element(),
            Load::Ready => {
                let errors = self.global_errors(cx);
                let palette = *palette;
                let weak = cx.entity().downgrade();
                let rows = self.globals.iter().enumerate().map(|(index, row)| {
                    let error = errors.get(index).copied().flatten();
                    let name_cell: AnyElement = if row.editing_name {
                        field(("global-name", index), &row.name, Kind::Urdu)
                            .width(px(NAME_COLUMN))
                            .error(error.is_some())
                            .into_any_element()
                    } else {
                        div()
                            .id(("global-name-label", index))
                            .test_support()
                            .max_w(px(NAME_COLUMN))
                            .h(px(32.))
                            .px(px(8.))
                            .mr(px(-8.))
                            .rounded(px(RADIUS))
                            .flex()
                            .items_center()
                            .cursor_text()
                            .hover(move |style| style.bg(palette.hover))
                            .active(move |s| s.bg(palette.pressed))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.rename_global(index, window, cx)
                            }))
                            .child(
                                div().min_w_0().mt(px(-2.)).child(
                                    ur_label(row.initial.0.clone(), UrRole::InputLabel)
                                        .text_color(palette.ink)
                                        .truncate(),
                                ),
                            )
                            .into_any_element()
                    };
                    let saved = row.id.is_some();
                    let rename = weak.clone();
                    let remove = weak.clone();
                    let more = menu(gpui_kit::SharedString::from(format!("global-more-{index}")))
                        .trigger(
                            icon_button(("global-more", index), IconName::Ellipsis, "More actions")
                                .dense(),
                        )
                        .content(move |menu, _, _| {
                            let rename = rename.clone();
                            let remove = remove.clone();
                            menu.item(
                                MenuEntry::new("global-rename", "Rename", move |window, cx| {
                                    rename
                                        .update(cx, |this, cx| this.rename_global(index, window, cx))
                                        .ok();
                                })
                                .icon(IconName::Pencil)
                                .disabled(!saved),
                            )
                            .separator()
                            .item(
                                MenuEntry::new(
                                    "global-delete",
                                    if saved { "Delete…" } else { "Remove" },
                                    move |window, cx| {
                                        remove
                                            .update(cx, |this, cx| {
                                                this.remove_global_row(index, window, cx)
                                            })
                                            .ok();
                                    },
                                )
                                .icon(IconName::Trash)
                                .danger(),
                            )
                        });
                    v_flex()
                        .id(("global-row", index))
                        .px(px(12.))
                        .py(px(8.))
                        .when(index > 0, |this| this.border_t_1().border_color(palette.line))
                        .child(
                            h_flex()
                                .flex_row_reverse()
                                .items_center()
                                .gap(px(12.))
                                .child(
                                    h_flex()
                                        .w(px(NAME_COLUMN))
                                        .flex_none()
                                        .flex_row_reverse()
                                        .child(name_cell),
                                )
                                .child(
                                    div().flex_1().min_w_0().child(
                                        field(("global-value", index), &row.value, Kind::UrduAuto)
                                            .error(error.is_some() && row.editing_name),
                                    ),
                                )
                                .child(more),
                        )
                        .children(error.map(|error| {
                            h_flex()
                                .flex_row_reverse()
                                .pt(px(4.))
                                .child(field_error(error))
                        }))
                });
                frame()
                    .child(
                        h_flex()
                            .h(px(32.))
                            .px(px(12.))
                            .gap(px(12.))
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.ink_muted)
                            .child(div().w(px(24.)).flex_none())
                            .child(div().flex_1().child("Value"))
                            .child(
                                h_flex()
                                    .w(px(NAME_COLUMN))
                                    .flex_none()
                                    .justify_end()
                                    .child("Name"),
                            ),
                    )
                    .child(frame_panel().children(rows))
                    .into_any_element()
            }
        }
    }

    fn customs_body(&mut self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        if self.customs.is_empty() {
            return frame()
                .child(
                    frame_panel().child(
                        empty_state_body(
                            IconName::TextCursorInput,
                            "No custom fields",
                            Some(
                                "Add a field for anything a FIR needs beyond the built-in ones."
                                    .into(),
                            ),
                            None,
                        )
                        .py(px(40.)),
                    ),
                )
                .into_any_element();
        }
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
        let markers = self.models.catalog.read(cx).settings.field_markers.clone();
        let weak = cx.entity().downgrade();
        let rows = visible.iter().enumerate().map(|(position, &index)| {
            let row = &self.customs[index];
            let error = errors.get(index).copied().flatten();
            let content: AnyElement = if row.editing {
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        field(("custom-name", index), &row.name, Kind::Urdu).error(error.is_some()),
                    )
                    .into_any_element()
            } else {
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .flex_row_reverse()
                    .gap(px(12.))
                    .child(
                        div().min_w_0().mt(px(-2.)).child(
                            ur_label(row.initial.clone(), UrRole::Body)
                                .text_color(palette.ink)
                                .truncate(),
                        ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex_none()
                            .mt(px(-2.))
                            .text_color(palette.ink_faint)
                            .child(ur_label(markers.token(&row.initial), UrRole::Label).truncate()),
                    )
                    .into_any_element()
            };
            let id = row.id;
            let name = row.initial.clone();
            let weak = weak.clone();
            v_flex()
                .id(("custom-row", index))
                .test_support()
                .when(position > 0, |this| {
                    this.border_t_1().border_color(palette.line)
                })
                .child(
                    h_flex()
                        .flex_row_reverse()
                        .min_h(px(48.))
                        .items_center()
                        .gap(px(12.))
                        .px(px(12.))
                        .py(px(6.))
                        .child(content)
                        .child(
                            menu(gpui_kit::SharedString::from(format!("custom-more-{index}")))
                                .trigger(
                                    icon_button(
                                        ("custom-more", index),
                                        IconName::Ellipsis,
                                        "More actions",
                                    )
                                    .dense(),
                                )
                                .content(move |menu, _, _| {
                                    let rename = weak.clone();
                                    let delete = weak.clone();
                                    let name = name.clone();
                                    menu.item(
                                        MenuEntry::new(
                                            "custom-rename",
                                            "Rename",
                                            move |window, cx| {
                                                rename
                                                    .update(cx, |this, cx| {
                                                        this.rename_custom(index, window, cx)
                                                    })
                                                    .ok();
                                            },
                                        )
                                        .icon(IconName::Pencil)
                                        .disabled(id.is_none()),
                                    )
                                    .separator()
                                    .item(
                                        MenuEntry::new(
                                            "custom-delete",
                                            "Delete…",
                                            move |window, cx| {
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
                                            },
                                        )
                                        .icon(IconName::Trash)
                                        .danger()
                                        .disabled(id.is_none()),
                                    )
                                }),
                        ),
                )
                .children(error.map(|error| {
                    h_flex()
                        .flex_row_reverse()
                        .px(px(12.))
                        .pb(px(8.))
                        .child(field_error(error))
                }))
        });
        let empty_match = visible.is_empty();
        frame()
            .children(show_filter.then(|| {
                div().p(px(4.)).pb(px(8.)).child(
                    search_field("custom-filter-field", &self.custom_filter, Kind::SearchRtl)
                        .height(32.),
                )
            }))
            .child(frame_panel().children(rows).when(empty_match, |this| {
                this.child(
                    div()
                        .p(px(24.))
                        .text_size(px(14.))
                        .text_color(palette.ink_muted)
                        .flex()
                        .justify_center()
                        .child("No field matches your search."),
                )
            }))
            .into_any_element()
    }

    fn built_in_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let markers = self.models.catalog.read(cx).settings.field_markers.clone();
        let cell = |index: usize, field: &missal_core::field::FirFieldLabel| {
            h_flex()
                .id(("built-in", index))
                .flex_1()
                .min_w_0()
                .h(px(40.))
                .px(px(12.))
                .flex_row_reverse()
                .items_center()
                .gap(px(12.))
                .child(
                    div().min_w_0().mt(px(-2.)).child(
                        ur_label(field.label, UrRole::Label)
                            .text_color(palette.ink)
                            .truncate(),
                    ),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .flex_none()
                        .text_color(palette.ink_faint)
                        .mt(px(-2.))
                        .child(ur_label(markers.token(field.label), UrRole::Label).truncate()),
                )
        };
        let rows = FIR_FIELDS.chunks(2).enumerate().map(|(row, pair)| {
            h_flex()
                .when(row > 0, |this| this.border_t_1().border_color(palette.line))
                .flex_row_reverse()
                .child(cell(row * 2, &pair[0]))
                .child(separator_vertical().h(px(40.)))
                .children(
                    pair.get(1)
                        .map(|field| cell(row * 2 + 1, field).into_any_element())
                        .or_else(|| Some(div().flex_1().into_any_element())),
                )
        });
        frame()
            .child(frame_panel().children(rows))
            .into_any_element()
    }

    fn markers_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let open = self.marker_open.read(cx).text().to_owned();
        let close = self.marker_close.read(cx).text().to_owned();
        let preview = FieldMarkers::new(&open, &close).unwrap_or_default();
        let error = self.marker_error(cx);
        let is_default = {
            let default = FieldMarkers::default();
            open == default.open() && close == default.close()
        };
        let chip = |text: String| {
            div()
                .px(px(6.))
                .rounded(px(RADIUS_SM))
                .bg(palette.field_wash)
                .text_color(palette.accent)
                .child(ur_label(text, UrRole::Body).mt(px(-2.)))
        };
        let token_row = h_flex()
            .flex_row_reverse()
            .flex_wrap()
            .justify_center()
            .items_center()
            .gap(px(8.))
            .text_color(palette.ink)
            .child(div().mt(px(-2.)).child(ur_label("مقدمہ", UrRole::Body)))
            .child(chip(preview.token("جرم")))
            .child(div().mt(px(-2.)).child(ur_label("تھانہ", UrRole::Body)))
            .child(chip(preview.token("تھانہ نام")))
            .child(
                div()
                    .mt(px(-2.))
                    .child(ur_label("میں درج ہوا۔", UrRole::Body)),
            );
        let symbol_row = |id: &'static str,
                          title: &'static str,
                          description: &'static str,
                          input: &Entity<TextInput>| {
            setting_row(&palette, title, Some(description.into()))
                .child(field(id, input, Kind::Urdu).width(px(64.)))
        };
        frame()
            .child(
                h_flex()
                    .h(px(32.))
                    .pl(px(12.))
                    .pr(px(4.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.ink_muted)
                    .child(div().flex_1().child("Preview"))
                    .when(!is_default, |this| {
                        this.child(
                            button("marker-reset", "Reset")
                                .ghost()
                                .xs()
                                .icon(IconName::RotateCcw)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.reset_markers(window, cx)
                                })),
                        )
                    }),
            )
            .child(
                frame_panel()
                    .child(
                        div()
                            .px(px(16.))
                            .py(px(24.))
                            .border_b_1()
                            .border_color(palette.line)
                            .child(token_row),
                    )
                    .child(symbol_row(
                        "marker-open",
                        "Opening symbol",
                        "Typed before a field name.",
                        &self.marker_open,
                    ))
                    .child(
                        symbol_row(
                            "marker-close",
                            "Closing symbol",
                            "Typed after a field name.",
                            &self.marker_close,
                        )
                        .top_border(),
                    )
                    .children(
                        error.map(|error| div().px(px(16.)).pb(px(12.)).child(field_error(error))),
                    ),
            )
            .into_any_element()
    }

    fn appearance_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let prefs = self.models.prefs.read(cx).read();
        let theme = prefs.theme;
        let sidebar_shown = !prefs.sidebar_hidden;
        let cards = ThemeChoice::ALL.into_iter().map(|choice| {
            let selected = choice == theme;
            let id = match choice {
                ThemeChoice::System => "theme-system",
                ThemeChoice::Light => "theme-light",
                ThemeChoice::Dark => "theme-dark",
            };
            v_flex()
                .id(id)
                .test_support()
                .flex_1()
                .min_w_0()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(RADIUS_LG))
                .border_1()
                .border_color(if selected {
                    palette.ink.opacity(0.48)
                } else {
                    palette.line
                })
                .when(selected, |this| this.bg(palette.hover))
                .cursor_pointer()
                .hover(move |style| style.bg(palette.hover))
                .on_click(move |_, window, cx| set_theme(choice, window, cx))
                .child(
                    div()
                        .h(px(88.))
                        .rounded(px(RADIUS))
                        .overflow_hidden()
                        .border_1()
                        .border_color(palette.line_strong)
                        .child(theme_preview(choice)),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .items_center()
                        .child(radio_dot(&palette, selected))
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .text_color(palette.ink)
                                .when(selected, |this| this.font_weight(FontWeight::MEDIUM))
                                .child(choice.title()),
                        ),
                )
        });
        v_flex()
            .gap(px(24.))
            .child(
                frame().child(frame_label(&palette, "Theme")).child(
                    frame_panel()
                        .p(px(16.))
                        .child(h_flex().gap(px(12.)).children(cards))
                        .child(
                            div()
                                .pt(px(14.))
                                .text_size(px(13.))
                                .line_height(px(18.))
                                .text_color(palette.ink_muted)
                                .child(
                                    "System follows your operating system's light or dark setting.",
                                ),
                        ),
                ),
            )
            .child(
                frame().child(frame_label(&palette, "Layout")).child(
                    frame_panel().child(
                        setting_row(
                            &palette,
                            "Show sidebar",
                            Some("Keep the navigation sidebar open beside your work.".into()),
                        )
                        .child(kbd_group("Mod+\\").chord())
                        .child(
                            switch("sidebar-switch", sidebar_shown)
                                .accessibility_label("Show sidebar")
                                .on_click(|_, _, cx| crate::app::toggle_sidebar(cx)),
                        ),
                    ),
                ),
            )
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
        let (status, intent): (String, Option<Intent>) = if !updates_enabled {
            ("Automatic updates are off in this build.".to_owned(), None)
        } else {
            match &state {
                UpdateState::Idle => ("Not checked yet.".to_owned(), None),
                UpdateState::Checking => ("Checking for updates…".to_owned(), None),
                UpdateState::UpToDate { at } => (
                    format!("You're up to date. Checked {}.", ago(*at)),
                    Some(Intent::Success),
                ),
                UpdateState::Ready(ready) => (
                    format!("Version {} is ready to install.", ready.version()),
                    Some(Intent::Info),
                ),
                UpdateState::Failed { at, .. } => (
                    format!("Could not check for updates ({}).", ago(*at)),
                    Some(Intent::Danger),
                ),
            }
        };
        let update_action: AnyElement = match &state {
            UpdateState::Ready(ready) if updates_enabled => {
                let ready = ready.clone();
                primary_sm("restart-update", "Restart now")
                    .on_click(move |_, window, cx| controller::restart(ready.clone(), window, cx))
                    .into_any_element()
            }
            _ => button("check-updates", "Check now")
                .outline()
                .sm()
                .icon(IconName::RefreshCw)
                .disabled(!updates_enabled)
                .busy(checking)
                .on_click(|_, window, cx| check_for_updates(window, cx))
                .into_any_element(),
        };
        let path_text = folder
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Unavailable".to_owned());
        let identity = h_flex()
            .gap(px(12.))
            .items_center()
            .px(px(16.))
            .py(px(16.))
            .child(
                div()
                    .size(px(40.))
                    .flex_none()
                    .rounded(px(RADIUS_LG))
                    .bg(palette.action)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(sized_icon(IconName::FileText, 20., palette.on_action)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(15.))
                            .line_height(px(22.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.ink)
                            .child("Missal"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .text_color(palette.ink_muted)
                            .child("FIRs and their documents, in Urdu."),
                    ),
            )
            .child(
                badge(BadgeVariant::Outline)
                    .px(px(6.))
                    .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
            );
        let status_line = h_flex()
            .gap(px(6.))
            .items_center()
            .children(intent.map(|intent| {
                let color = match intent {
                    Intent::Success => palette.success,
                    Intent::Danger => palette.danger,
                    _ => palette.accent,
                };
                div().size(px(6.)).rounded_full().bg(color).flex_none()
            }))
            .child(status);
        v_flex()
            .gap(px(24.))
            .child(
                frame().child(
                    frame_panel().child(identity).child(
                        setting_row(&palette, "Updates", None)
                            .top_border()
                            .child(update_action)
                            .map(|row| row.description_element(&palette, status_line)),
                    ),
                ),
            )
            .child(
                frame()
                    .child(frame_label(&palette, "On this computer"))
                    .child(
                        frame_panel()
                            .child(
                                setting_row(&palette, "Data folder", None)
                                    .map(|row| {
                                        row.description_element(
                                            &palette,
                                            label(path_text).ltr().truncate(),
                                        )
                                    })
                                    .children(folder.clone().map(|path| {
                                        button("open-data-folder", "Open")
                                            .outline()
                                            .sm()
                                            .icon(IconName::FolderOpen)
                                            .on_click(move |_, _, cx| cx.reveal_path(&path))
                                    })),
                            )
                            .child(
                                setting_row(
                                    &palette,
                                    "Keyboard shortcuts",
                                    Some("Every shortcut in Missal, in one place.".into()),
                                )
                                .top_border()
                                .child(
                                    button("show-shortcuts", "Show")
                                        .outline()
                                        .sm()
                                        .icon(IconName::Keyboard)
                                        .on_click(|_, window, cx| {
                                            crate::app::show_shortcuts(window, cx)
                                        }),
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn account_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let signed_in = crate::auth::AuthModel::get(cx).and_then(|auth| {
            let auth = auth.read(cx);
            let purchase = auth.purchase_url().is_some();
            match auth.state() {
                crate::auth::AuthState::SignedIn { account, online } => {
                    Some((account.clone(), *online, purchase))
                }
                _ => None,
            }
        });
        let Some((account, online, purchase)) = signed_in else {
            return frame()
                .child(
                    frame_panel().child(
                        empty_state_body(
                            IconName::CircleUser,
                            "Not signed in",
                            Some("This copy of Missal runs without an account.".into()),
                            None,
                        )
                        .py(px(40.)),
                    ),
                )
                .into_any_element();
        };
        let admin = account.role.is_admin();
        let licensed = admin || account.licence.status == missal_auth::LicenceStatus::Licensed;
        let more = menu("account-more")
            .trigger(icon_button(
                "account-more",
                IconName::Ellipsis,
                "Account actions",
            ))
            .content(|menu, _, _| {
                menu.item(
                    MenuEntry::new("change-password", "Change password…", |window, cx| {
                        crate::ui::account::open_change_password(window, cx)
                    })
                    .icon(IconName::KeyRound),
                )
                .separator()
                .item(
                    MenuEntry::new("sign-out", "Sign out", |window, cx| {
                        crate::auth::sign_out(window, cx)
                    })
                    .icon(IconName::LogOut),
                )
            });
        let profile = h_flex()
            .gap(px(12.))
            .items_center()
            .px(px(16.))
            .py(px(16.))
            .child(Avatar::new().name(account.display_name().to_owned()))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(
                                div()
                                    .min_w_0()
                                    .text_size(px(15.))
                                    .line_height(px(22.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.ink)
                                    .child(
                                        label(account.display_name().to_owned()).ltr().truncate(),
                                    ),
                            )
                            .child(
                                badge(BadgeVariant::Outline)
                                    .px(px(6.))
                                    .child(account.role.label()),
                            )
                            .when(!online, |this| {
                                this.child(
                                    badge(BadgeVariant::Intent(Intent::Neutral))
                                        .px(px(6.))
                                        .child("Offline"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .text_color(palette.ink_muted)
                            .child(label(account.email.clone()).ltr().truncate()),
                    ),
            )
            .child(more);
        let licence_note = if admin {
            "Administrators never need a licence.".to_owned()
        } else {
            crate::screens::people::licence_text(&account)
        };
        let licence = setting_row(&palette, "Licence", Some(licence_note.into()))
            .top_border()
            .child(crate::screens::people::standing_badge(&account))
            .when(purchase && !licensed, |this| {
                this.child(
                    primary_sm("buy-licence", "Buy licence")
                        .on_click(|_, _, cx| crate::auth::open_purchase(cx)),
                )
            });
        let link_row = |id: &'static str,
                        icon: IconName,
                        title: &'static str,
                        description: &'static str,
                        open: fn(&mut Window, &mut App)| {
            h_flex()
                .id(id)
                .test_support()
                .gap(px(12.))
                .items_center()
                .px(px(16.))
                .py(px(12.))
                .cursor_pointer()
                .hover(move |style| style.bg(palette.hover))
                .active(move |style| style.bg(palette.pressed))
                .on_click(move |_, window, cx| open(window, cx))
                .child(
                    div()
                        .size(px(32.))
                        .flex_none()
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(palette.line)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(small_icon(icon, palette.ink_muted)),
                )
                .child(row_text(&palette, title, Some(description.into())))
                .child(small_icon(IconName::ChevronRight, palette.ink_faint))
        };
        v_flex()
            .gap(px(24.))
            .child(frame().child(frame_panel().child(profile).child(licence)))
            .when(admin, |this| {
                this.child(
                    frame()
                        .child(frame_label(&palette, "Administration"))
                        .child(
                            frame_panel()
                                .overflow_hidden()
                                .child(link_row(
                                    "manage-people",
                                    IconName::Users,
                                    "People",
                                    "Add people, renew licences and reset passwords.",
                                    |window, cx| {
                                        crate::app::navigate(
                                            crate::state::Route::People,
                                            window,
                                            cx,
                                        )
                                    },
                                ))
                                .child(
                                    link_row(
                                        "manage-publishing",
                                        IconName::Upload,
                                        "Publishing",
                                        "Share templates through the library.",
                                        |window, cx| {
                                            crate::app::navigate(
                                                crate::state::Route::Publishing,
                                                window,
                                                cx,
                                            )
                                        },
                                    )
                                    .border_t_1()
                                    .border_color(palette.line),
                                ),
                        ),
                )
            })
            .into_any_element()
    }

    fn nav(&self, palette: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *palette;
        let has_account = crate::auth::AuthModel::get(cx).is_some();
        let globals = self.globals.iter().filter(|row| row.id.is_some()).count();
        let customs = self.customs.iter().filter(|row| row.id.is_some()).count();
        let ready = self.models.catalog.read(cx).load.is_ready();
        v_flex()
            .w(px(NAV_WIDTH))
            .flex_none()
            .h_full()
            .px(px(8.))
            .py(px(12.))
            .gap(px(16.))
            .children(NAV_GROUPS.iter().filter_map(|(group, pages)| {
                let pages: Vec<Page> = pages
                    .iter()
                    .copied()
                    .filter(|page| *page != Page::Account || has_account)
                    .collect();
                if pages.is_empty() {
                    return None;
                }
                Some(
                    sidebar_group()
                        .child(sidebar_group_label().child(*group))
                        .children(pages.into_iter().map(|page| {
                            let active = page == self.page;
                            let count = match page {
                                Page::Globals if ready => Some(globals),
                                Page::Customs if ready => Some(customs),
                                Page::BuiltIn => Some(FIR_FIELDS.len()),
                                _ => None,
                            };
                            sidebar_menu_button(page.id(), active)
                                .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx)))
                                .child(small_icon(
                                    page.icon(),
                                    if active {
                                        palette.ink
                                    } else {
                                        palette.ink_muted
                                    },
                                ))
                                .child(div().flex_1().min_w_0().truncate().child(page.title()))
                                .children(count.map(|count| {
                                    div()
                                        .text_size(px(12.))
                                        .text_color(palette.ink_faint)
                                        .font_features(crate::ui::tabular())
                                        .child(count.to_string())
                                }))
                        })),
                )
            }))
    }
}

struct SettingRow {
    top: bool,
    title: &'static str,
    description: Option<AnyElement>,
    controls: Vec<AnyElement>,
    palette: Palette,
}

fn setting_row(
    palette: &Palette,
    title: &'static str,
    description: Option<gpui_kit::SharedString>,
) -> SettingRow {
    SettingRow {
        top: false,
        title,
        description: description.map(|text| div().child(text).into_any_element()),
        controls: Vec::new(),
        palette: *palette,
    }
}

impl SettingRow {
    fn description_element(mut self, _: &Palette, element: impl IntoElement) -> SettingRow {
        self.description = Some(element.into_any_element());
        self
    }

    fn map(self, f: impl FnOnce(SettingRow) -> SettingRow) -> SettingRow {
        f(self)
    }
}

impl ParentElement for SettingRow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.controls.extend(elements)
    }
}

impl SettingRow {
    fn top_border(mut self) -> SettingRow {
        self.top = true;
        self
    }

    fn into_div(self) -> gpui_kit::Div {
        h_flex()
            .gap(px(16.))
            .items_center()
            .px(px(16.))
            .py(px(12.))
            .min_h(px(60.))
            .when(self.top, |this| {
                this.border_t_1().border_color(self.palette.line)
            })
            .child(row_text_with(&self.palette, self.title, self.description))
            .children(self.controls)
    }
}

impl IntoElement for SettingRow {
    type Element = gpui_kit::Div;

    fn into_element(self) -> Self::Element {
        self.into_div()
    }
}

fn row_text(
    palette: &Palette,
    title: &'static str,
    description: Option<gpui_kit::SharedString>,
) -> gpui_kit::Div {
    row_text_with(
        palette,
        title,
        description.map(|text| div().child(text).into_any_element()),
    )
}

fn row_text_with(
    palette: &Palette,
    title: &'static str,
    description: Option<AnyElement>,
) -> gpui_kit::Div {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(14.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.ink)
                .child(title),
        )
        .children(description.map(|description| {
            div()
                .min_w_0()
                .text_size(px(13.))
                .line_height(px(18.))
                .text_color(palette.ink_muted)
                .child(description)
        }))
}

fn frame_label(palette: &Palette, text: &'static str) -> gpui_kit::Div {
    h_flex()
        .h(px(32.))
        .px(px(12.))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.ink_muted)
        .child(text)
}

fn radio_dot(palette: &Palette, selected: bool) -> gpui_kit::Div {
    div()
        .size(px(16.))
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(if selected {
            palette.ink
        } else {
            palette.line_strong
        })
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |this| {
            this.child(div().size(px(8.)).rounded_full().bg(palette.ink))
        })
}

fn bar(color: gpui_kit::Hsla, width: f32) -> gpui_kit::Div {
    div()
        .h(px(4.))
        .w(gpui_kit::relative(width))
        .rounded(px(2.))
        .bg(color)
}

fn preview_sidebar(palette: &Palette) -> gpui_kit::Div {
    v_flex()
        .w(px(44.))
        .flex_none()
        .h_full()
        .bg(palette.sidebar)
        .p(px(7.))
        .gap(px(5.))
        .child(bar(palette.ink_muted, 0.8))
        .child(bar(palette.line_strong, 0.6))
        .child(bar(palette.line_strong, 0.7))
}

fn preview_content(palette: &Palette) -> gpui_kit::Div {
    v_flex()
        .flex_1()
        .h_full()
        .bg(palette.surface)
        .p(px(8.))
        .gap(px(6.))
        .child(bar(palette.ink.opacity(0.72), 0.45))
        .child(
            div()
                .h(px(16.))
                .w_full()
                .rounded(px(3.))
                .border_1()
                .border_color(palette.line_strong),
        )
        .child(bar(palette.line_strong, 0.9))
        .child(bar(palette.line_strong, 0.7))
        .child(bar(palette.line_strong, 0.8))
}

fn theme_preview(choice: ThemeChoice) -> gpui_kit::Div {
    let light = Palette::light();
    let dark = Palette::dark();
    match choice {
        ThemeChoice::Light => h_flex()
            .size_full()
            .child(preview_sidebar(&light))
            .child(preview_content(&light)),
        ThemeChoice::Dark => h_flex()
            .size_full()
            .child(preview_sidebar(&dark))
            .child(preview_content(&dark)),
        ThemeChoice::System => h_flex()
            .size_full()
            .child(preview_sidebar(&light))
            .child(preview_content(&light))
            .child(preview_content(&dark)),
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
        if self.page == Page::Account && crate::auth::AuthModel::get(cx).is_none() {
            self.page = Page::Globals;
        }
        let body = match self.page {
            Page::Globals => self.globals_body(&palette, cx),
            Page::Customs => self.customs_body(&palette, cx),
            Page::BuiltIn => self.built_in_body(&palette, cx),
            Page::Markers => self.markers_body(&palette, cx),
            Page::Appearance => self.appearance_body(&palette, cx),
            Page::About => self.about_body(&palette, cx),
            Page::Account => self.account_body(&palette, cx),
        };
        let header = self.page_header(&palette, cx).into_any_element();
        let nav = self.nav(&palette, cx).into_any_element();
        h_flex()
            .id("settings")
            .key_context("Settings")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .bg(palette.surface)
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save_all(window, cx)))
            .on_action(
                cx.listener(|this, _: &RenameTemplate, window, cx| this.rename_focused(window, cx)),
            )
            .child(nav)
            .child(crate::ui::pane_divider())
            .child(
                v_flex()
                    .id("settings-body")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .child(
                        v_flex()
                            .id(("settings-page", self.page as usize))
                            .w(px(COLUMN_WIDTH))
                            .max_w_full()
                            .flex_shrink_0()
                            .px(px(24.))
                            .pt(px(32.))
                            .pb(px(96.))
                            .child(header)
                            .child(body),
                    )
                    .scroll_area_y(&self.scroll),
            )
    }
}

impl HeaderSource for SettingsScreen {
    fn header(&mut self, _: &mut Window, _: &mut Context<Self>) -> HeaderContent {
        HeaderContent::new("Settings")
    }
}
