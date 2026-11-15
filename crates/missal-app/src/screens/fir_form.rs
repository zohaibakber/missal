use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::calendar::{Calendar, CalendarEvent, CalendarState, Date};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Selectable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, RenderOnce,
    ScrollHandle, StatefulInteractiveElement, Styled, Subscription, Window, anchored, deferred,
    div, px,
};
use missal_core::{
    CalendarDate, Fir, FirId, FirRecord, FirStatus, fold_digits, format_date, parse_date,
};
use missal_text::{Direction, LineHeight, TextInput, TextInputEvent};

use crate::app::navigate_replace;
use crate::shortcuts::*;
use crate::state::rows::FirRow;
use crate::state::{Models, Route, run_db_in, toast_error, toast_success};
use crate::theme::{INPUT_URDU, Palette};
use crate::ui::dialogs::confirm_discard;
use crate::ui::input::{Kind, field, field_input};
use crate::ui::menu::menu_panel;
use crate::ui::status::status_dot;
use crate::ui::{UrRole, button, icon_button, sized_icon, ur_label};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListKind {
    Accused,
    Witness,
    Zimni,
}

impl ListKind {
    const ALL: [ListKind; 3] = [ListKind::Accused, ListKind::Witness, ListKind::Zimni];

    fn index(self) -> usize {
        self as usize
    }

    fn title(self) -> &'static str {
        match self {
            ListKind::Accused => "ملزمان",
            ListKind::Witness => "گواہان",
            ListKind::Zimni => "ضمنی",
        }
    }

    fn add_label(self) -> &'static str {
        match self {
            ListKind::Accused => "Add accused",
            ListKind::Witness => "Add witness",
            ListKind::Zimni => "Add zimni",
        }
    }

    fn field(self) -> &'static str {
        match self {
            ListKind::Accused => "accused",
            ListKind::Witness => "witness",
            ListKind::Zimni => "zimni",
        }
    }

    fn section(self) -> usize {
        2 + self.index()
    }
}

pub enum FirFormEvent {
    Saved {
        record: Box<FirRecord>,
        created: bool,
    },
    DirtyChanged(bool),
}

const DATE_KEYS: [&str; 3] = ["date", "incident_date", "arrest_date"];

pub struct FirForm {
    models: Models,
    id: Option<FirId>,
    initial: Fir,
    fir_no: Entity<TextInput>,
    complainant: Entity<TextInput>,
    offence: Entity<TextInput>,
    dates: [Entity<TextInput>; 3],
    calendars: [Entity<CalendarState>; 3],
    calendar_open: Option<usize>,
    officer: Entity<TextInput>,
    writer: Entity<TextInput>,
    recovery: Entity<TextInput>,
    body: Entity<TextInput>,
    imei: [Entity<TextInput>; 2],
    sim: [Entity<TextInput>; 2],
    status: FirStatus,
    status_open: bool,
    lists: [Vec<Entity<TextInput>>; 3],
    errors: HashMap<&'static str, &'static str>,
    validated: bool,
    dirty: bool,
    saving: bool,
    scroll: ScrollHandle,
    subscriptions: Vec<Subscription>,
}

impl EventEmitter<FirFormEvent> for FirForm {}

fn body_input(window: &mut Window, cx: &mut App, text: &str) -> Entity<TextInput> {
    let faint = Palette::get(cx).ink_faint;
    cx.new(|cx| {
        TextInput::new(window, cx)
            .bare()
            .multiline(6, 14)
            .direction(Direction::Rtl)
            .align(missal_text::Align::Right)
            .text_size(16.)
            .placeholder_color(faint)
            .line_height(LineHeight::Px(30.))
            .default_text(text.to_owned())
    })
}

impl FirForm {
    pub fn new(
        models: Models,
        record: Option<FirRecord>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FirForm {
        let (id, initial) = match record {
            Some(record) => (Some(record.id), record.fir),
            None => (None, Fir::empty_draft()),
        };
        let urdu = |window: &mut Window, cx: &mut Context<Self>, text: &str| {
            field_input(window, cx, Kind::Urdu, "", text)
        };
        let numeric = |window: &mut Window, cx: &mut Context<Self>, text: &str| {
            field_input(window, cx, Kind::LatinNumeric, "", text)
        };
        let date = |window: &mut Window, cx: &mut Context<Self>, text: &str| {
            field_input(window, cx, Kind::Date, "DD-MM-YYYY", text)
        };
        let fir_no = field_input(
            window,
            cx,
            Kind::LatinNumeric,
            "e.g. 412/2026",
            &initial.fir_no,
        );
        let complainant = urdu(window, cx, &initial.complainant);
        let offence = urdu(window, cx, &initial.offence);
        let dates = [
            date(window, cx, &initial.date),
            date(window, cx, &initial.incident_date),
            date(window, cx, &initial.arrest_date),
        ];
        let calendars = [0, 1, 2].map(|_| cx.new(|cx| CalendarState::new(window, cx)));
        let officer = urdu(window, cx, &initial.investigation_officer);
        let writer = urdu(window, cx, &initial.writer);
        let recovery = urdu(window, cx, &initial.recovery);
        let body = body_input(window, cx, &initial.body);
        let imei = [
            numeric(window, cx, &initial.imei_1),
            numeric(window, cx, &initial.imei_2),
        ];
        let sim = [
            numeric(window, cx, &initial.sim_1),
            numeric(window, cx, &initial.sim_2),
        ];
        let lists = ListKind::ALL.map(|kind| {
            let items = match kind {
                ListKind::Accused => &initial.accused,
                ListKind::Witness => &initial.witness,
                ListKind::Zimni => &initial.zimni,
            };
            items
                .iter()
                .map(|text| urdu(window, cx, text))
                .collect::<Vec<_>>()
        });
        let mut form = FirForm {
            models,
            id,
            status: initial.status,
            initial,
            fir_no,
            complainant,
            offence,
            dates,
            calendars,
            calendar_open: None,
            officer,
            writer,
            recovery,
            body,
            imei,
            sim,
            status_open: false,
            lists,
            errors: HashMap::new(),
            validated: false,
            dirty: false,
            saving: false,
            scroll: ScrollHandle::new(),
            subscriptions: Vec::new(),
        };
        form.wire(window, cx);
        form
    }

    fn singles(&self) -> Vec<Entity<TextInput>> {
        let mut inputs = vec![
            self.fir_no.clone(),
            self.complainant.clone(),
            self.offence.clone(),
            self.officer.clone(),
            self.writer.clone(),
            self.recovery.clone(),
            self.body.clone(),
        ];
        inputs.extend(self.dates.iter().cloned());
        inputs.extend(self.imei.iter().cloned());
        inputs.extend(self.sim.iter().cloned());
        inputs
    }

    fn watch(&mut self, input: &Entity<TextInput>, cx: &mut Context<Self>) {
        let subscription = cx.subscribe(input, |this, _, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Change {
                this.refresh_dirty(cx);
            }
        });
        self.subscriptions.push(subscription);
    }

    fn watch_entry(
        &mut self,
        kind: ListKind,
        input: &Entity<TextInput>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.watch(input, cx);
        let subscription = cx.subscribe_in(
            input,
            window,
            move |this, input, event: &TextInputEvent, window, cx| {
                if *event == TextInputEvent::Submit
                    && this.lists[kind.index()].last() == Some(input)
                {
                    this.add_entry(kind, window, cx);
                }
            },
        );
        self.subscriptions.push(subscription);
    }

    fn fold_on_blur(&mut self, input: &Entity<TextInput>, date: bool, cx: &mut Context<Self>) {
        let subscription = cx.subscribe(input, move |_, input, event: &TextInputEvent, cx| {
            if *event == TextInputEvent::Blur {
                let text = input.read(cx).text().to_owned();
                let normal = if date {
                    format_date(&text)
                } else {
                    fold_digits(&text)
                };
                if normal != text {
                    input.update(cx, |input, cx| input.set_text(normal, cx));
                }
            }
        });
        self.subscriptions.push(subscription);
    }

    fn wire(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in self.singles() {
            self.watch(&input, cx);
        }
        for kind in ListKind::ALL {
            for input in self.lists[kind.index()].clone() {
                self.watch_entry(kind, &input, window, cx);
            }
        }
        for input in self.dates.clone() {
            self.fold_on_blur(&input, true, cx);
        }
        let numbers: Vec<_> = [self.fir_no.clone()]
            .into_iter()
            .chain(self.imei.iter().cloned())
            .chain(self.sim.iter().cloned())
            .collect();
        for input in numbers {
            self.fold_on_blur(&input, false, cx);
        }
        for (index, calendar) in self.calendars.clone().into_iter().enumerate() {
            let subscription = cx.subscribe_in(
                &calendar,
                window,
                move |this, _, event: &CalendarEvent, window, cx| {
                    let CalendarEvent::Selected(Date::Single(Some(date))) = event else {
                        return;
                    };
                    if let Some(date) =
                        CalendarDate::new(date.year(), date.month() as u8, date.day() as u8)
                    {
                        let text = date.to_string();
                        this.dates[index].update(cx, |input, cx| input.set_text(text, cx));
                    }
                    this.close_calendar(window, cx);
                },
            );
            self.subscriptions.push(subscription);
        }
    }

    pub fn values(&self, cx: &App) -> Fir {
        let text = |input: &Entity<TextInput>| input.read(cx).text().to_owned();
        let list = |kind: ListKind| {
            self.lists[kind.index()]
                .iter()
                .map(&text)
                .collect::<Vec<_>>()
        };
        Fir {
            fir_no: text(&self.fir_no),
            complainant: text(&self.complainant),
            date: text(&self.dates[0]),
            offence: text(&self.offence),
            accused: list(ListKind::Accused),
            witness: list(ListKind::Witness),
            zimni: list(ListKind::Zimni),
            incident_date: text(&self.dates[1]),
            arrest_date: text(&self.dates[2]),
            investigation_officer: text(&self.officer),
            writer: text(&self.writer),
            recovery: text(&self.recovery),
            body: text(&self.body),
            imei_1: text(&self.imei[0]),
            imei_2: text(&self.imei[1]),
            sim_1: text(&self.sim[0]),
            sim_2: text(&self.sim[1]),
            status: self.status,
        }
    }

    pub fn id(&self) -> Option<FirId> {
        self.id
    }

    pub fn is_edit(&self) -> bool {
        self.id.is_some()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn is_saving(&self) -> bool {
        self.saving
    }

    pub fn status(&self) -> FirStatus {
        self.status
    }

    pub fn set_status(&mut self, status: FirStatus, cx: &mut Context<Self>) {
        self.status = status;
        self.status_open = false;
        self.refresh_dirty(cx);
        cx.notify();
    }

    pub fn fir_no_input(&self) -> &Entity<TextInput> {
        &self.fir_no
    }

    pub fn complainant_input(&self) -> &Entity<TextInput> {
        &self.complainant
    }

    pub fn date_inputs(&self) -> &[Entity<TextInput>; 3] {
        &self.dates
    }

    pub fn list_inputs(&self, kind: ListKind) -> &[Entity<TextInput>] {
        &self.lists[kind.index()]
    }

    pub fn offence_input(&self) -> &Entity<TextInput> {
        &self.offence
    }

    pub fn errors(&self) -> Vec<&'static str> {
        let mut fields: Vec<_> = self.errors.keys().copied().collect();
        fields.sort_unstable();
        fields
    }

    fn refresh_dirty(&mut self, cx: &mut Context<Self>) {
        let values = self.values(cx);
        let dirty = values != self.initial;
        if dirty != self.dirty {
            self.dirty = dirty;
            cx.emit(FirFormEvent::DirtyChanged(dirty));
        }
        if self.validated {
            self.errors = Self::validate(&values);
        }
        cx.notify();
    }

    pub fn focus_first(&self, window: &mut Window, cx: &mut App) {
        self.fir_no.update(cx, |input, cx| input.focus(window, cx));
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        let initial = self.initial.clone();
        self.apply(&initial, cx);
        self.errors.clear();
        self.validated = false;
        self.refresh_dirty(cx);
        cx.notify();
    }

    fn apply(&mut self, fir: &Fir, cx: &mut Context<Self>) {
        let set = |input: &Entity<TextInput>, text: &str, cx: &mut Context<Self>| {
            if input.read(cx).text() != text {
                input.update(cx, |input, cx| input.set_text(text, cx))
            }
        };
        set(&self.fir_no, &fir.fir_no, cx);
        set(&self.complainant, &fir.complainant, cx);
        set(&self.offence, &fir.offence, cx);
        set(&self.dates[0], &fir.date, cx);
        set(&self.dates[1], &fir.incident_date, cx);
        set(&self.dates[2], &fir.arrest_date, cx);
        set(&self.officer, &fir.investigation_officer, cx);
        set(&self.writer, &fir.writer, cx);
        set(&self.recovery, &fir.recovery, cx);
        set(&self.body, &fir.body, cx);
        set(&self.imei[0], &fir.imei_1, cx);
        set(&self.imei[1], &fir.imei_2, cx);
        set(&self.sim[0], &fir.sim_1, cx);
        set(&self.sim[1], &fir.sim_2, cx);
        self.status = fir.status;
        for kind in ListKind::ALL {
            let wanted = match kind {
                ListKind::Accused => &fir.accused,
                ListKind::Witness => &fir.witness,
                ListKind::Zimni => &fir.zimni,
            };
            self.lists[kind.index()].truncate(wanted.len());
            for (position, text) in wanted.iter().enumerate() {
                if let Some(input) = self.lists[kind.index()].get(position) {
                    set(input, text, cx);
                }
            }
        }
    }

    pub fn add_entry(&mut self, kind: ListKind, window: &mut Window, cx: &mut Context<Self>) {
        let input = field_input(window, cx, Kind::Urdu, "", "");
        self.watch_entry(kind, &input, window, cx);
        input.update(cx, |input, cx| input.focus(window, cx));
        self.lists[kind.index()].push(input);
        self.refresh_dirty(cx);
        cx.notify();
    }

    pub fn remove_entry(&mut self, kind: ListKind, position: usize, cx: &mut Context<Self>) {
        let list = &mut self.lists[kind.index()];
        if position < list.len() && !(kind == ListKind::Accused && list.len() == 1) {
            list.remove(position);
            self.refresh_dirty(cx);
            cx.notify();
        }
    }

    fn validate(fir: &Fir) -> HashMap<&'static str, &'static str> {
        let mut errors = HashMap::new();
        let blank = |text: &str| text.trim().is_empty();
        let bad_date = |text: &str| !blank(text) && parse_date(text).is_none();
        if blank(&fir.fir_no) {
            errors.insert("fir_no", "Enter the FIR number, for example 412/2026.");
        }
        if blank(&fir.offence) {
            errors.insert("offence", "Enter the offence.");
        }
        if blank(&fir.date) {
            errors.insert("date", "Enter the FIR date.");
        } else if bad_date(&fir.date) {
            errors.insert("date", "Use the format DD-MM-YYYY.");
        }
        if blank(&fir.incident_date) {
            errors.insert("incident_date", "Enter the incident date.");
        } else if bad_date(&fir.incident_date) {
            errors.insert("incident_date", "Use the format DD-MM-YYYY.");
        }
        if bad_date(&fir.arrest_date) {
            errors.insert("arrest_date", "Use the format DD-MM-YYYY.");
        }
        for kind in ListKind::ALL {
            let items = match kind {
                ListKind::Accused => &fir.accused,
                ListKind::Witness => &fir.witness,
                ListKind::Zimni => &fir.zimni,
            };
            if kind == ListKind::Accused && items.len() == 1 && blank(&items[0]) {
                errors.insert(kind.field(), "Enter at least one accused.");
            } else if items.iter().any(|item| blank(item)) {
                errors.insert(kind.field(), "Fill in or remove the empty entries.");
            }
        }
        errors
    }

    fn first_error(&self, cx: &App) -> Option<(usize, Entity<TextInput>)> {
        let order: [(&str, usize, Option<Entity<TextInput>>); 8] = [
            ("fir_no", 0, Some(self.fir_no.clone())),
            ("offence", 0, Some(self.offence.clone())),
            ("date", 1, Some(self.dates[0].clone())),
            ("incident_date", 1, Some(self.dates[1].clone())),
            ("arrest_date", 1, Some(self.dates[2].clone())),
            ("accused", 2, None),
            ("witness", 3, None),
            ("zimni", 4, None),
        ];
        for (key, section, input) in order {
            if !self.errors.contains_key(key) {
                continue;
            }
            let input = input.or_else(|| {
                let kind = ListKind::ALL.into_iter().find(|kind| kind.field() == key)?;
                let list = &self.lists[kind.index()];
                list.iter()
                    .find(|input| is_blank(input, cx))
                    .or(list.first())
                    .cloned()
            })?;
            return Some((section, input));
        }
        None
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let mut fir = self.values(cx);
        fir.fir_no = fold_digits(fir.fir_no.trim());
        fir.date = format_date(&fir.date);
        fir.incident_date = format_date(&fir.incident_date);
        fir.arrest_date = format_date(&fir.arrest_date);
        for value in [
            &mut fir.imei_1,
            &mut fir.imei_2,
            &mut fir.sim_1,
            &mut fir.sim_2,
        ] {
            *value = fold_digits(value);
        }
        self.validated = true;
        let errors = Self::validate(&fir);
        if !errors.is_empty() {
            self.errors = errors;
            if let Some((section, input)) = self.first_error(cx) {
                self.scroll.scroll_to_item(section);
                input.update(cx, |input, cx| input.focus(window, cx));
            }
            cx.notify();
            return;
        }
        self.errors.clear();
        self.saving = true;
        cx.notify();
        let id = self.id;
        let job_fir = fir.clone();
        run_db_in(
            window,
            cx,
            move |store| match id {
                Some(id) => store.update_fir(&FirRecord { id, fir: job_fir }),
                None => store.create_fir(&job_fir),
            },
            move |this, result, window, cx| {
                this.saving = false;
                match result {
                    Ok(record) => {
                        let created = this.id.is_none();
                        this.id = Some(record.id);
                        this.initial = record.fir.clone();
                        this.apply(&record.fir.clone(), cx);
                        this.dirty = false;
                        this.validated = false;
                        this.errors.clear();
                        this.models
                            .firs
                            .update(cx, |firs, cx| firs.upsert(FirRow::from_record(&record), cx));
                        cx.emit(FirFormEvent::DirtyChanged(false));
                        cx.emit(FirFormEvent::Saved {
                            record: Box::new(record),
                            created,
                        });
                    }
                    Err(error) => toast_error(window, cx, error.message()),
                }
                cx.notify();
            },
        );
    }

    fn open_calendar(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let date = parse_date(self.dates[index].read(cx).text()).and_then(|date| {
            NaiveDate::from_ymd_opt(date.year(), u32::from(date.month()), u32::from(date.day()))
        });
        self.calendars[index].update(cx, |state, cx| {
            state.set_date(Date::Single(date), window, cx)
        });
        self.calendar_open = Some(index);
        cx.notify();
    }

    fn close_calendar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.calendar_open.take() {
            self.dates[index].update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    fn label_row(palette: &Palette, title: &'static str, required: bool) -> AnyElement {
        h_flex()
            .flex_row_reverse()
            .items_center()
            .gap(px(4.))
            .child(
                div()
                    .text_color(palette.ink)
                    .child(ur_label(title, UrRole::InputLabel)),
            )
            .children(required.then(|| {
                div()
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(palette.danger)
                    .child("*")
            }))
            .into_any_element()
    }

    fn error_line(palette: &Palette, message: &'static str) -> AnyElement {
        div()
            .w_full()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(palette.danger_text)
            .child(
                missal_text::label(message)
                    .align(missal_text::Align::Right)
                    .leading(LineHeight::Px(16.)),
            )
            .into_any_element()
    }

    fn field_cell(
        &self,
        palette: &Palette,
        title: &'static str,
        required: bool,
        key: &'static str,
        control: AnyElement,
    ) -> AnyElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(4.))
            .child(Self::label_row(palette, title, required))
            .child(control)
            .children(
                self.errors
                    .get(key)
                    .map(|message| Self::error_line(palette, message)),
            )
            .into_any_element()
    }

    fn input_cell(
        &self,
        palette: &Palette,
        title: &'static str,
        required: bool,
        key: &'static str,
        input: &Entity<TextInput>,
        kind: Kind,
    ) -> AnyElement {
        let control = field(
            gpui_kit::SharedString::from(format!("form-{key}")),
            input,
            kind,
        )
        .height(INPUT_URDU)
        .error(self.errors.contains_key(key))
        .into_any_element();
        self.field_cell(palette, title, required, key, control)
    }

    fn row(cells: Vec<AnyElement>) -> AnyElement {
        h_flex()
            .w_full()
            .flex_row_reverse()
            .items_start()
            .gap(px(20.))
            .children(cells)
            .into_any_element()
    }

    fn section(
        palette: &Palette,
        index: usize,
        title: &'static str,
        body: AnyElement,
    ) -> AnyElement {
        v_flex()
            .id(("form-section", index))
            .w_full()
            .pt(px(20.))
            .child(
                h_flex()
                    .flex_row_reverse()
                    .items_center()
                    .gap(px(12.))
                    .mb(px(12.))
                    .child(
                        div()
                            .flex_none()
                            .text_color(palette.ink)
                            .child(ur_label(title, UrRole::Title)),
                    )
                    .child(div().flex_1().h(px(1.)).bg(palette.line)),
            )
            .child(body)
            .into_any_element()
    }

    fn status_select(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let weak = cx.entity().downgrade();
        let open_weak = weak.clone();
        Popover::new("status-select-popover")
            .appearance(false)
            .trigger(StatusTrigger {
                status: self.status,
                open: self.status_open,
            })
            .trigger_style(gpui_kit::StyleRefinement::default().w_full())
            .anchor(gpui_kit::Anchor::TopRight)
            .open(self.status_open)
            .on_open_change(move |open, _, cx| {
                let open = *open;
                open_weak
                    .update(cx, |this, cx| {
                        this.status_open = open;
                        cx.notify();
                    })
                    .ok();
            })
            .content(move |_, _, cx| {
                let weak = weak.clone();
                let current = weak
                    .upgrade()
                    .map(|form| form.read(cx).status)
                    .unwrap_or(FirStatus::Open);
                menu_panel(&palette)
                    .w(px(240.))
                    .children(FirStatus::ALL.iter().map(|status| {
                        let status = *status;
                        let weak = weak.clone();
                        let hover = palette.hover;
                        h_flex()
                            .id(("status-choice", status as usize))
                            .test_support()
                            .flex_row_reverse()
                            .h(px(36.))
                            .px(px(10.))
                            .gap(px(8.))
                            .items_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .role(gpui_kit::accesskit::Role::MenuItemRadio)
                            .when(status == current, |this| this.bg(palette.pressed))
                            .hover(move |style| style.bg(hover))
                            .on_click(move |_, _, cx| {
                                weak.update(cx, |this, cx| this.set_status(status, cx)).ok();
                            })
                            .child(status_dot(&palette, status))
                            .child(
                                div()
                                    .mt(px(-2.))
                                    .text_color(palette.ink)
                                    .child(ur_label(status.label(), UrRole::Body)),
                            )
                    }))
            })
            .into_any_element()
    }

    fn date_control(&self, palette: &Palette, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let weak = cx.entity().downgrade();
        let state = self.calendars[index].clone();
        let open = self.calendar_open == Some(index);
        let key = DATE_KEYS[index];
        let opener = weak.clone();
        let input = field(
            gpui_kit::SharedString::from(format!("form-{key}")),
            &self.dates[index],
            Kind::Date,
        )
        .height(INPUT_URDU)
        .error(self.errors.contains_key(key))
        .on_calendar(move |window, cx| {
            opener
                .update(cx, |this, cx| {
                    if this.calendar_open == Some(index) {
                        this.close_calendar(window, cx);
                    } else {
                        this.open_calendar(index, window, cx);
                    }
                })
                .ok();
        });
        let closer = weak.clone();
        div()
            .id(("date-control", index))
            .relative()
            .w_full()
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "down" && event.keystroke.modifiers.alt {
                    this.open_calendar(index, window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(input)
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(INPUT_URDU + 4.))
                        .left_0()
                        .child(deferred(
                            anchored().snap_to_window_with_margin(px(8.)).child(
                                div()
                                    .id(("calendar-panel", index))
                                    .occlude()
                                    .p(px(8.))
                                    .rounded(px(8.))
                                    .bg(palette.raised)
                                    .shadow(palette.shadow_md())
                                    .on_mouse_down_out(move |_, window, cx| {
                                        closer
                                            .update(cx, |this, cx| {
                                                this.calendar_open = None;
                                                cx.notify();
                                            })
                                            .ok();
                                        let _ = window;
                                    })
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, window, cx| {
                                            if event.keystroke.key == "escape" {
                                                this.close_calendar(window, cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    ))
                                    .child(Calendar::new(&state)),
                            ),
                        )),
                )
            })
            .into_any_element()
    }

    fn list_section(
        &self,
        palette: &Palette,
        kind: ListKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = *palette;
        let items = &self.lists[kind.index()];
        let count = items.len();
        let weak = cx.entity().downgrade();
        let error = self.errors.get(kind.field()).copied();
        let ink = palette.ink;
        v_flex()
            .gap(px(8.))
            .children(items.iter().enumerate().map(|(position, input)| {
                let removable = !(kind == ListKind::Accused && count == 1);
                let weak = weak.clone();
                let blank = is_blank(input, cx);
                h_flex()
                    .id(("entry", kind.index() * 1000 + position))
                    .flex_row_reverse()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .w(px(18.))
                            .flex_none()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(palette.ink_faint)
                            .text_center()
                            .child((position + 1).to_string()),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            field(
                                ("entry-field", kind.index() * 1000 + position),
                                input,
                                Kind::Urdu,
                            )
                            .error(error.is_some() && blank),
                        ),
                    )
                    .child(
                        div()
                            .w(px(28.))
                            .flex_none()
                            .flex()
                            .justify_center()
                            .children(removable.then(|| {
                                icon_button(
                                    ("remove-entry", kind.index() * 1000 + position),
                                    IconName::X,
                                    "Remove",
                                )
                                .dense()
                                .on_click(move |_, _, cx| {
                                    weak.update(cx, |this, cx| {
                                        this.remove_entry(kind, position, cx)
                                    })
                                    .ok();
                                })
                            })),
                    )
            }))
            .children(error.map(|message| {
                div()
                    .pr(px(26.))
                    .pl(px(36.))
                    .child(Self::error_line(&palette, message))
            }))
            .child(
                h_flex().flex_row_reverse().child(
                    h_flex()
                        .id(("add-entry", kind.index()))
                        .h(px(28.))
                        .px(px(8.))
                        .gap(px(6.))
                        .items_center()
                        .rounded(px(6.))
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.ink_muted)
                        .cursor_pointer()
                        .hover(move |s| s.bg(palette.hover).text_color(ink))
                        .role(gpui_kit::accesskit::Role::Button)
                        .on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.add_entry(kind, window, cx)
                            }),
                        )
                        .child(sized_icon(IconName::Plus, 14., palette.ink_muted))
                        .child(kind.add_label())
                        .test_support(),
                ),
            )
            .into_any_element()
    }

    fn body_control(&self, palette: &Palette, window: &Window, cx: &App) -> AnyElement {
        let focused = self.body.focus_handle(cx).is_focused(window);
        let mut shadows = palette.shadow_sm();
        if focused {
            shadows.extend(palette.input_focus_ring());
        }
        let target = self.body.clone();
        div()
            .id("form-field-body")
            .w_full()
            .px(px(10.))
            .py(px(3.))
            .rounded(px(6.))
            .border_1()
            .border_color(if focused {
                palette.accent
            } else {
                palette.line_strong
            })
            .bg(palette.surface)
            .shadow(shadows)
            .cursor_text()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                target.update(cx, |input, cx| input.focus(window, cx));
            })
            .child(self.body.clone())
            .into_any_element()
    }
}

fn is_blank(input: &Entity<TextInput>, cx: &App) -> bool {
    input.read(cx).text().trim().is_empty()
}

#[derive(IntoElement)]
struct StatusTrigger {
    status: FirStatus,
    open: bool,
}

impl Selectable for StatusTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for StatusTrigger {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let mut shadows = palette.shadow_sm();
        if self.open {
            shadows.extend(palette.input_focus_ring());
        }
        h_flex()
            .id("status-select")
            .w_full()
            .h(px(INPUT_URDU))
            .px(px(10.))
            .gap(px(8.))
            .flex_row_reverse()
            .items_center()
            .rounded(px(6.))
            .border_1()
            .border_color(if self.open {
                palette.accent
            } else {
                palette.line_strong
            })
            .bg(palette.surface)
            .shadow(shadows)
            .cursor_pointer()
            .role(gpui_kit::accesskit::Role::ComboBox)
            .aria_label("Status")
            .child(status_dot(&palette, self.status))
            .child(
                div()
                    .mt(px(-2.))
                    .text_color(palette.ink)
                    .child(ur_label(self.status.label(), UrRole::Body)),
            )
            .child(div().flex_1())
            .child(sized_icon(IconName::ChevronsUpDown, 14., palette.ink_faint))
            .test_support()
    }
}

impl Render for FirForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let status = self.status_select(&palette, cx);
        let first = Self::section(
            &palette,
            0,
            "مقدمہ",
            v_flex()
                .gap(px(14.))
                .child(Self::row(vec![
                    self.input_cell(
                        &palette,
                        "ایف آئی آر نمبر",
                        true,
                        "fir_no",
                        &self.fir_no,
                        Kind::LatinNumeric,
                    ),
                    self.field_cell(&palette, "حالت", false, "status", status),
                ]))
                .child(Self::row(vec![
                    self.input_cell(&palette, "جرم", true, "offence", &self.offence, Kind::Urdu),
                    self.input_cell(
                        &palette,
                        "مدعی مقدمہ",
                        false,
                        "complainant",
                        &self.complainant,
                        Kind::Urdu,
                    ),
                ]))
                .into_any_element(),
        );
        let date_titles = [
            ("تاریخ ایف آئی آر", true),
            ("تاریخ وقوعہ", true),
            ("تاریخ گرفتاری", false),
        ];
        let dates: Vec<AnyElement> = date_titles
            .iter()
            .enumerate()
            .map(|(index, (title, required))| {
                let control = self.date_control(&palette, index, cx);
                self.field_cell(&palette, title, *required, DATE_KEYS[index], control)
            })
            .collect();
        let second = Self::section(&palette, 1, "تاریخیں", Self::row(dates));
        let lists: Vec<AnyElement> = ListKind::ALL
            .iter()
            .map(|kind| {
                let body = self.list_section(&palette, *kind, cx);
                Self::section(&palette, kind.section(), kind.title(), body)
            })
            .collect();
        let investigation = Self::section(
            &palette,
            5,
            "تفتیش",
            v_flex()
                .gap(px(14.))
                .child(Self::row(vec![
                    self.input_cell(
                        &palette,
                        "تفتیشی افسر",
                        false,
                        "investigation_officer",
                        &self.officer,
                        Kind::Urdu,
                    ),
                    self.input_cell(
                        &palette,
                        "تحریر کنندہ",
                        false,
                        "writer",
                        &self.writer,
                        Kind::Urdu,
                    ),
                ]))
                .child(self.input_cell(
                    &palette,
                    "برآمدگی",
                    false,
                    "recovery",
                    &self.recovery,
                    Kind::Urdu,
                ))
                .into_any_element(),
        );
        let mobile = Self::section(
            &palette,
            6,
            "موبائل",
            v_flex()
                .gap(px(14.))
                .child(Self::row(vec![
                    self.input_cell(
                        &palette,
                        "ایمی نمبر 1",
                        false,
                        "imei_1",
                        &self.imei[0],
                        Kind::LatinNumeric,
                    ),
                    self.input_cell(
                        &palette,
                        "سم نمبر 1",
                        false,
                        "sim_1",
                        &self.sim[0],
                        Kind::LatinNumeric,
                    ),
                ]))
                .child(Self::row(vec![
                    self.input_cell(
                        &palette,
                        "ایمی نمبر 2",
                        false,
                        "imei_2",
                        &self.imei[1],
                        Kind::LatinNumeric,
                    ),
                    self.input_cell(
                        &palette,
                        "سم نمبر 2",
                        false,
                        "sim_2",
                        &self.sim[1],
                        Kind::LatinNumeric,
                    ),
                ]))
                .into_any_element(),
        );
        let body = self.body_control(&palette, window, cx);
        let text = Self::section(
            &palette,
            7,
            "متن",
            self.field_cell(&palette, "متن", false, "body", body),
        );
        let mut sections = vec![first, second];
        sections.extend(lists);
        sections.push(investigation);
        sections.push(mobile);
        sections.push(text);
        div()
            .id("fir-form-scroll")
            .key_context(FIR_FORM)
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_action(cx.listener(|this, _: &Save, window, cx| this.submit(window, cx)))
            .child(
                v_flex()
                    .id("fir-form")
                    .w(px(760.))
                    .max_w_full()
                    .mx_auto()
                    .pt(px(8.))
                    .pb(px(40.))
                    .children(sections),
            )
    }
}

pub const FIR_FORM: &str = "FirForm";

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FormState {
    Loading,
    Ready,
    Failed(String),
}

pub struct NewFirScreen {
    models: Models,
    edit: Option<FirId>,
    state: FormState,
    form: Option<Entity<FirForm>>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl NewFirScreen {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> NewFirScreen {
        let mut screen = NewFirScreen {
            models,
            edit: None,
            state: FormState::Ready,
            form: None,
            focus: cx.focus_handle(),
            _subscriptions: Vec::new(),
        };
        screen.attach(None, window, cx);
        screen
    }

    pub fn edit(
        models: Models,
        id: FirId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> NewFirScreen {
        let mut screen = NewFirScreen {
            models,
            edit: Some(id),
            state: FormState::Loading,
            form: None,
            focus: cx.focus_handle(),
            _subscriptions: Vec::new(),
        };
        screen.load(window, cx);
        screen
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.edit else {
            return;
        };
        run_db_in(
            window,
            cx,
            move |store| store.fir(id),
            |this, result, window, cx| {
                match result {
                    Ok(record) => {
                        this.attach(Some(record), window, cx);
                        this.state = FormState::Ready;
                        this.focus(window, cx);
                    }
                    Err(error) => this.state = FormState::Failed(error.message()),
                }
                cx.notify();
            },
        );
    }

    fn attach(&mut self, record: Option<FirRecord>, window: &mut Window, cx: &mut Context<Self>) {
        let models = self.models.clone();
        let form = cx.new(|cx| FirForm::new(models, record, window, cx));
        self._subscriptions = vec![
            cx.observe(&form, |_, _, cx| cx.notify()),
            cx.subscribe_in(&form, window, |_, _, event: &FirFormEvent, window, cx| {
                if let FirFormEvent::Saved { record, created } = event {
                    toast_success(
                        window,
                        cx,
                        if *created {
                            format!("FIR {} created", record.fir.fir_no)
                        } else {
                            format!("FIR {} saved", record.fir.fir_no)
                        },
                    );
                    navigate_replace(Route::Fir(record.id), cx);
                }
            }),
        ];
        self.form = Some(form);
    }

    pub fn form(&self) -> &Entity<FirForm> {
        self.form.as_ref().expect("the form is loaded")
    }

    pub fn editing(&self) -> Option<FirId> {
        self.edit
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.form
            .as_ref()
            .is_some_and(|form| form.read(cx).is_dirty())
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        match self.form.clone() {
            Some(form) => form.update(cx, |form, cx| form.focus_first(window, cx)),
            None => window.focus(&self.focus, cx),
        }
    }

    pub fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let leave = {
            let nav = self.models.nav.clone();
            let edit = self.edit;
            move |window: &mut Window, cx: &mut App| {
                if nav.read(cx).can_go_back() {
                    nav.update(cx, |nav, cx| nav.back(window, cx));
                } else {
                    navigate_replace(edit.map_or(Route::Firs, Route::Fir), cx);
                }
            }
        };
        if !self.is_dirty(cx) {
            leave(window, cx);
            return;
        }
        window.focus(&self.focus, cx);
        let form = self.form.clone();
        confirm_discard(window, cx, move |window, cx| {
            if let Some(form) = form.as_ref() {
                form.update(cx, |form, cx| form.reset(cx));
            }
            leave(window, cx);
        });
    }

    fn footer(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let Some(form) = self.form.clone() else {
            return div().into_any_element();
        };
        let (dirty, saving, edit) = {
            let form = form.read(cx);
            (form.is_dirty(), form.is_saving(), form.is_edit())
        };
        let hint = match (edit, dirty) {
            (false, _) => "Not saved yet",
            (true, false) => "No changes",
            (true, true) => "Unsaved changes",
        };
        let hint_icon = if edit && !dirty {
            IconName::Check
        } else {
            IconName::CircleAlert
        };
        h_flex()
            .flex_none()
            .h(px(56.))
            .px(px(24.))
            .gap(px(8.))
            .items_center()
            .border_t_1()
            .border_color(palette.line)
            .bg(palette.surface)
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_muted)
                    .child(sized_icon(hint_icon, 14., palette.ink_muted))
                    .child(hint),
            )
            .child(div().flex_1())
            .child(
                button("cancel-fir", "Cancel")
                    .ghost()
                    .tooltip("Cancel", Some("Escape"))
                    .on_click(cx.listener(|this, _, window, cx| this.cancel(window, cx))),
            )
            .child(
                button(
                    if edit { "save-fir" } else { "create-fir" },
                    if saving {
                        "Saving…"
                    } else if edit {
                        "Save changes"
                    } else {
                        "Create FIR"
                    },
                )
                .primary()
                .keys("Mod+S")
                .disabled(saving)
                .on_click(move |_, window, cx| form.update(cx, |form, cx| form.submit(window, cx))),
            )
            .into_any_element()
    }
}

impl Render for NewFirScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let body: AnyElement = match (&self.state, self.form.clone()) {
            (FormState::Ready, Some(form)) => form.into_any_element(),
            (FormState::Failed(message), _) => crate::ui::empty_state(
                &palette,
                IconName::TriangleAlert,
                "Couldn't open this FIR",
                Some(message.clone().into()),
                Some(
                    button("form-retry", "Try again")
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.state = FormState::Loading;
                            this.load(window, cx);
                            cx.notify();
                        }))
                        .into_any_element(),
                ),
            )
            .into_any_element(),
            _ => div().into_any_element(),
        };
        let footer = self.footer(&palette, cx);
        v_flex()
            .id("new-fir")
            .key_context("NewFir")
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &BlurFocus, window, cx| this.cancel(window, cx)))
            .on_action(cx.listener(|this, _: &Save, window, cx| {
                if let Some(form) = this.form.clone() {
                    form.update(cx, |form, cx| form.submit(window, cx));
                }
            }))
            .child(div().flex_1().min_h_0().child(body))
            .child(footer)
    }
}

impl crate::ui::titlebar::HeaderSource for NewFirScreen {
    fn header(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::ui::titlebar::HeaderContent {
        let palette = *Palette::get(cx);
        let title = match (self.edit, self.form.as_ref()) {
            (Some(_), Some(form)) => format!("Edit FIR {}", form.read(cx).initial.fir_no),
            (Some(id), None) => self
                .models
                .firs
                .read(cx)
                .get(id)
                .map(|row| format!("Edit FIR {}", row.no))
                .unwrap_or_else(|| "Edit FIR".to_owned()),
            (None, _) => "New FIR".to_owned(),
        };
        crate::ui::titlebar::HeaderContent::new(title).meta_element(
            h_flex()
                .text_size(px(12.))
                .text_color(palette.ink_faint)
                .child("Fields marked ")
                .child(div().text_color(palette.danger).child("*"))
                .child(" are required"),
        )
    }
}

impl Focusable for NewFirScreen {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
