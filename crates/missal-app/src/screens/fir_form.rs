use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::calendar::{Calendar, CalendarEvent, CalendarState, Date};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, div, px,
};
use missal_core::{CalendarDate, Fir, FirId, FirRecord, FirStatus, format_date, parse_date};
use missal_text::{TextInput, TextInputEvent, label};

use crate::app::navigate_replace;
use crate::shortcuts::*;
use crate::state::rows::FirRow;
use crate::state::{Models, Route, run_db_in, toast_error, toast_success, toast_warning};
use crate::theme::Palette;
use crate::ui::input::{Kind, text_area, text_input};
use crate::ui::status::status_icon;
use crate::ui::{pane_header, save_status, small_icon};

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

    fn placeholder(self) -> &'static str {
        match self {
            ListKind::Accused => "نام، ولدیت اور سکونت",
            ListKind::Witness => "نام اور تفصیل",
            ListKind::Zimni => "ضمنی کی تفصیل",
        }
    }

    fn field(self) -> &'static str {
        match self {
            ListKind::Accused => "accused",
            ListKind::Witness => "witness",
            ListKind::Zimni => "zimni",
        }
    }
}

pub enum FirFormEvent {
    Saved {
        record: Box<FirRecord>,
        created: bool,
    },
    DirtyChanged(bool),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DateField {
    Date = 0,
    Incident = 1,
    Arrest = 2,
}

pub struct FirForm {
    models: Models,
    id: Option<FirId>,
    initial: Fir,
    compact: bool,
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
    dirty: bool,
    saving: bool,
    subscriptions: Vec<Subscription>,
}

impl EventEmitter<FirFormEvent> for FirForm {}

impl FirForm {
    pub fn new(
        models: Models,
        record: Option<FirRecord>,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FirForm {
        let (id, initial) = match record {
            Some(record) => (Some(record.id), record.fir),
            None => (None, Fir::empty_draft()),
        };
        let urdu = |window: &mut Window, cx: &mut Context<Self>, placeholder: &str, text: &str| {
            text_input(window, cx, Kind::Urdu, placeholder, text)
        };
        let numeric =
            |window: &mut Window, cx: &mut Context<Self>, placeholder: &str, text: &str| {
                text_input(window, cx, Kind::Numeric, placeholder, text)
            };
        let fir_no = urdu(window, cx, "412/2026", &initial.fir_no);
        let complainant = urdu(window, cx, "", &initial.complainant);
        let offence = urdu(window, cx, "چوری ۳۷۹ ت پ", &initial.offence);
        let dates = [
            numeric(window, cx, "DD-MM-YYYY", &initial.date),
            numeric(window, cx, "DD-MM-YYYY", &initial.incident_date),
            numeric(window, cx, "DD-MM-YYYY", &initial.arrest_date),
        ];
        let calendars = [0, 1, 2].map(|_| cx.new(|cx| CalendarState::new(window, cx)));
        let officer = urdu(window, cx, "", &initial.investigation_officer);
        let writer = urdu(window, cx, "", &initial.writer);
        let recovery = text_area(window, cx, "", &initial.recovery, 2);
        let body = text_area(window, cx, "", &initial.body, 4);
        let imei = [
            numeric(window, cx, "", &initial.imei_1),
            numeric(window, cx, "", &initial.imei_2),
        ];
        let sim = [
            numeric(window, cx, "", &initial.sim_1),
            numeric(window, cx, "", &initial.sim_2),
        ];
        let lists = ListKind::ALL.map(|kind| {
            let items = match kind {
                ListKind::Accused => &initial.accused,
                ListKind::Witness => &initial.witness,
                ListKind::Zimni => &initial.zimni,
            };
            items
                .iter()
                .map(|text| text_input(window, cx, Kind::Urdu, kind.placeholder(), text))
                .collect::<Vec<_>>()
        });
        let mut form = FirForm {
            models,
            id,
            status: initial.status,
            initial,
            compact,
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
            dirty: false,
            saving: false,
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

    fn wire(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in self.singles() {
            self.watch(&input, cx);
        }
        let all_lists: Vec<_> = self.lists.iter().flatten().cloned().collect();
        for input in all_lists {
            self.watch(&input, cx);
        }
        for (index, input) in self.dates.clone().into_iter().enumerate() {
            let subscription =
                cx.subscribe(&input, move |this, input, event: &TextInputEvent, cx| {
                    if *event == TextInputEvent::Blur {
                        let text = input.read(cx).text().to_owned();
                        let normal = format_date(&text);
                        if normal != text {
                            input.update(cx, |input, cx| input.set_text(normal, cx));
                        }
                        let _ = (&this.calendar_open, index);
                    }
                });
            self.subscriptions.push(subscription);
        }
        for (index, calendar) in self.calendars.clone().into_iter().enumerate() {
            let subscription = cx.subscribe_in(
                &calendar,
                window,
                move |this, _, event: &CalendarEvent, _, cx| {
                    let CalendarEvent::Selected(Date::Single(Some(date))) = event else {
                        return;
                    };
                    if let Some(date) = CalendarDate::new(
                        i64::from(date.year()),
                        date.month() as u8,
                        date.day() as u8,
                    ) {
                        let text = date.to_string();
                        this.dates[index].update(cx, |input, cx| input.set_text(text, cx));
                    }
                    this.calendar_open = None;
                    cx.notify();
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
        let dirty = self.values(cx) != self.initial;
        if dirty != self.dirty {
            self.dirty = dirty;
            cx.emit(FirFormEvent::DirtyChanged(dirty));
            cx.notify();
        }
        if !self.errors.is_empty() {
            self.errors.clear();
            cx.notify();
        }
    }

    pub fn focus_first(&self, window: &mut Window, cx: &mut App) {
        self.fir_no.update(cx, |input, cx| input.focus(window, cx));
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        let initial = self.initial.clone();
        self.apply(&initial, cx);
        self.errors.clear();
        self.refresh_dirty(cx);
        cx.notify();
    }

    fn apply(&mut self, fir: &Fir, cx: &mut Context<Self>) {
        let set = |input: &Entity<TextInput>, text: &str, cx: &mut Context<Self>| {
            input.update(cx, |input, cx| input.set_text(text, cx))
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
        let input = text_input(window, cx, Kind::Urdu, kind.placeholder(), "");
        self.watch(&input, cx);
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

    fn validate(&self, fir: &Fir) -> HashMap<&'static str, &'static str> {
        let mut errors = HashMap::new();
        let blank = |text: &str| missal_core::text::js_trim(text).is_empty();
        if blank(&fir.fir_no) {
            errors.insert("fir_no", "Required");
        }
        if blank(&fir.date) {
            errors.insert("date", "Required");
        }
        if blank(&fir.offence) {
            errors.insert("offence", "Required");
        }
        if blank(&fir.incident_date) {
            errors.insert("incident_date", "Required");
        }
        for kind in ListKind::ALL {
            let items = match kind {
                ListKind::Accused => &fir.accused,
                ListKind::Witness => &fir.witness,
                ListKind::Zimni => &fir.zimni,
            };
            if items.iter().any(|item| blank(item)) {
                errors.insert(kind.field(), "Fill in or remove empty entries");
            }
        }
        errors
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let mut fir = self.values(cx);
        fir.date = format_date(&fir.date);
        fir.incident_date = format_date(&fir.incident_date);
        fir.arrest_date = format_date(&fir.arrest_date);
        let errors = self.validate(&fir);
        if !errors.is_empty() {
            self.errors = errors;
            toast_warning(window, cx, "Please fill in the required fields");
            cx.notify();
            return;
        }
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
        let text = self.dates[index].read(cx).text().to_owned();
        let date = parse_date(&text).and_then(|date| {
            NaiveDate::from_ymd_opt(
                date.year() as i32,
                u32::from(date.month()),
                u32::from(date.day()),
            )
        });
        self.calendars[index].update(cx, |state, cx| {
            state.set_date(Date::Single(date), window, cx)
        });
        self.calendar_open = Some(index);
        cx.notify();
    }

    fn label_row(
        &self,
        palette: &Palette,
        title: &'static str,
        required: bool,
    ) -> impl IntoElement {
        h_flex()
            .flex_row_reverse()
            .justify_start()
            .gap_1()
            .text_size(px(16.))
            .text_color(palette.foreground)
            .child(label(title).rtl())
            .children(required.then(|| div().text_color(palette.destructive).child("*")))
    }

    fn field(
        &self,
        palette: &Palette,
        title: &'static str,
        required: bool,
        key: &'static str,
        control: impl IntoElement,
    ) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(self.label_row(palette, title, required))
            .child(control)
            .children(self.errors.get(key).map(|message| {
                div()
                    .text_size(px(12.))
                    .text_color(palette.destructive)
                    .text_right()
                    .child(*message)
            }))
    }

    fn section(
        &self,
        palette: &Palette,
        title: &'static str,
        body: impl IntoElement,
    ) -> impl IntoElement {
        let heading = div()
            .flex_shrink_0()
            .text_size(px(15.))
            .font_semibold()
            .child(label(title).rtl());
        if self.compact {
            v_flex()
                .gap_3()
                .py_5()
                .border_b_1()
                .border_color(palette.border)
                .child(heading)
                .child(v_flex().gap_4().child(body))
                .into_any_element()
        } else {
            h_flex()
                .flex_row_reverse()
                .items_start()
                .gap_8()
                .py_6()
                .border_b_1()
                .border_color(palette.border)
                .child(div().w(px(150.)).child(heading))
                .child(v_flex().flex_1().min_w_0().gap_4().child(body))
                .into_any_element()
        }
    }

    fn pair(&self, first: impl IntoElement, second: impl IntoElement) -> impl IntoElement {
        if self.compact {
            v_flex()
                .gap_4()
                .child(first)
                .child(second)
                .into_any_element()
        } else {
            h_flex()
                .flex_row_reverse()
                .gap_4()
                .items_start()
                .child(first)
                .child(second)
                .into_any_element()
        }
    }

    fn status_select(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let status = self.status;
        let weak = cx.entity().downgrade();
        let open_weak = weak.clone();
        let trigger = Button::new("status-select").outline().w_full().child(
            h_flex()
                .w_full()
                .flex_row_reverse()
                .items_center()
                .gap_2()
                .child(status_icon(status, palette.status(status), px(14.)))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(15.))
                        .child(label(status.label()).rtl()),
                )
                .child(small_icon(
                    IconName::ChevronsUpDown,
                    palette.muted_foreground,
                )),
        );
        Popover::new("status-select-popover")
            .trigger(trigger)
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
            .trigger_style(gpui_kit::StyleRefinement::default().w_full())
            .p_1()
            .content(move |_, _, cx| {
                let weak = weak.clone();
                v_flex()
                    .w(px(220.))
                    .gap_0p5()
                    .children(FirStatus::ALL.iter().map(|status| {
                        let status = *status;
                        let weak = weak.clone();
                        h_flex()
                            .id(("status-choice", status as usize))
                            .test_support()
                            .flex_row_reverse()
                            .h(px(36.))
                            .px_2()
                            .gap_2()
                            .items_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(move |style| style.bg(palette.accent))
                            .on_click(cx.listener(move |_, _, window, cx| {
                                weak.update(cx, |this, cx| this.set_status(status, cx)).ok();
                                let _ = window;
                            }))
                            .child(status_icon(status, palette.status(status), px(14.)))
                            .child(div().text_size(px(15.)).child(label(status.label()).rtl()))
                    }))
            })
            .into_any_element()
    }

    fn date_control(
        &self,
        palette: &Palette,
        which: DateField,
        key: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let _ = (palette, key);
        let index = which as usize;
        let weak = cx.entity().downgrade();
        let open_weak = weak.clone();
        let state = self.calendars[index].clone();
        let trigger = Button::new(("calendar-trigger", index))
            .ghost()
            .small()
            .icon(IconName::Calendar)
            .tooltip("Pick a date");
        h_flex()
            .id(("date-control", index))
            .w_full()
            .gap_1()
            .items_center()
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "down" && !event.keystroke.modifiers.modified() {
                    this.open_calendar(index, window, cx);
                }
            }))
            .child(div().flex_1().min_w_0().child(self.dates[index].clone()))
            .child(
                Popover::new(("calendar-popover", index))
                    .trigger(trigger)
                    .open(self.calendar_open == Some(index))
                    .on_open_change(move |open, window, cx| {
                        let open = *open;
                        open_weak
                            .update(cx, |this, cx| {
                                if open {
                                    this.open_calendar(index, window, cx);
                                } else if this.calendar_open == Some(index) {
                                    this.calendar_open = None;
                                    cx.notify();
                                }
                            })
                            .ok();
                    })
                    .content(move |_, _, _| Calendar::new(&state)),
            )
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
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .flex_row_reverse()
                    .items_center()
                    .justify_between()
                    .child(self.label_row(&palette, kind.title(), kind == ListKind::Accused))
                    .child(
                        Button::new(("add-entry", kind.index()))
                            .outline()
                            .xsmall()
                            .icon(IconName::Plus)
                            .label("Add")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.add_entry(kind, window, cx)
                            })),
                    ),
            )
            .children(items.iter().enumerate().map(|(position, input)| {
                let removable = !(kind == ListKind::Accused && count == 1);
                let weak = weak.clone();
                h_flex()
                    .id(("entry", kind.index() * 1000 + position))
                    .flex_row_reverse()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(22.))
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .text_color(palette.muted_foreground)
                            .text_center()
                            .child((position + 1).to_string()),
                    )
                    .child(div().flex_1().min_w_0().child(input.clone()))
                    .child(
                        div()
                            .w(px(28.))
                            .flex_shrink_0()
                            .children(removable.then(|| {
                                Button::new(("remove-entry", kind.index() * 1000 + position))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::X)
                                    .tooltip("Remove")
                                    .on_click(move |_, _, cx| {
                                        weak.update(cx, |this, cx| {
                                            this.remove_entry(kind, position, cx)
                                        })
                                        .ok();
                                    })
                            })),
                    )
            }))
            .children(self.errors.get(kind.field()).map(|message| {
                div()
                    .text_size(px(12.))
                    .text_color(palette.destructive)
                    .text_right()
                    .child(*message)
            }))
            .into_any_element()
    }
}

impl Render for FirForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let status_control = self.status_select(&palette, cx);
        let date_specs = [
            ("تاریخ ایف آئی آر", true, "date", DateField::Date),
            ("تاریخ وقوعہ", true, "incident_date", DateField::Incident),
            ("تاریخ گرفتاری", false, "arrest_date", DateField::Arrest),
        ];
        let date_controls: Vec<AnyElement> = date_specs
            .iter()
            .map(|(_, _, key, which)| self.date_control(&palette, *which, key, cx))
            .collect();
        let people: Vec<AnyElement> = ListKind::ALL
            .iter()
            .map(|kind| self.list_section(&palette, *kind, cx))
            .collect();
        let save = cx.listener(|this, _: &Save, window, cx| this.submit(window, cx));
        let intro = self.pair(
            self.field(
                &palette,
                "ایف آئی آر نمبر",
                true,
                "fir_no",
                self.fir_no.clone(),
            ),
            self.field(
                &palette,
                "مدعی مقدمہ",
                false,
                "complainant",
                self.complainant.clone(),
            ),
        );
        let second = self.pair(
            self.field(&palette, "حالت", false, "status", status_control),
            self.field(&palette, "جرم", true, "offence", self.offence.clone()),
        );
        let date_fields: Vec<AnyElement> = date_specs
            .iter()
            .zip(date_controls)
            .map(|((title, required, key, _), control)| {
                self.field(&palette, title, *required, key, control)
                    .into_any_element()
            })
            .collect();
        let details = [
            self.pair(
                self.field(
                    &palette,
                    "تفتیشی افسر",
                    false,
                    "investigation_officer",
                    self.officer.clone(),
                ),
                self.field(
                    &palette,
                    "تحریر کنندہ",
                    false,
                    "writer",
                    self.writer.clone(),
                ),
            )
            .into_any_element(),
            self.field(
                &palette,
                "برآمدگی",
                false,
                "recovery",
                self.recovery.clone(),
            )
            .into_any_element(),
            self.field(&palette, "متن", false, "body", self.body.clone())
                .into_any_element(),
            self.pair(
                self.field(
                    &palette,
                    "ایمی نمبر 1",
                    false,
                    "imei_1",
                    self.imei[0].clone(),
                ),
                self.field(
                    &palette,
                    "ایمی نمبر 2",
                    false,
                    "imei_2",
                    self.imei[1].clone(),
                ),
            )
            .into_any_element(),
            self.pair(
                self.field(&palette, "سم نمبر 1", false, "sim_1", self.sim[0].clone()),
                self.field(&palette, "سم نمبر 2", false, "sim_2", self.sim[1].clone()),
            )
            .into_any_element(),
        ];
        v_flex()
            .id("fir-form")
            .key_context(FIR_FORM)
            .w_full()
            .on_action(save)
            .child(self.section(
                &palette,
                "مقدمہ",
                v_flex().gap_4().child(intro).child(second),
            ))
            .child(self.section(
                &palette,
                "تاریخیں",
                if self.compact {
                    v_flex().gap_4().children(date_fields).into_any_element()
                } else {
                    h_flex()
                        .flex_row_reverse()
                        .gap_4()
                        .items_start()
                        .children(date_fields)
                        .into_any_element()
                },
            ))
            .child(self.section(&palette, "دیگر تفصیلات", v_flex().gap_4().children(details)))
            .child(self.section(
                &palette,
                "ملزمان / گواہان / ضمنی",
                v_flex().gap_6().children(people),
            ))
    }
}

pub const FIR_FORM: &str = "FirForm";

pub struct NewFirScreen {
    models: Models,
    form: Entity<FirForm>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl NewFirScreen {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> NewFirScreen {
        let form = cx.new(|cx| FirForm::new(models.clone(), None, false, window, cx));
        let subscriptions = vec![
            cx.observe(&form, |_, _, cx| cx.notify()),
            cx.subscribe_in(&form, window, |_, _, event: &FirFormEvent, window, cx| {
                if let FirFormEvent::Saved { record, .. } = event {
                    toast_success(window, cx, "FIR created");
                    navigate_replace(Route::Fir(record.id), cx);
                }
            }),
        ];
        NewFirScreen {
            models,
            form,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    pub fn form(&self) -> &Entity<FirForm> {
        &self.form
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.form.read(cx).is_dirty()
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        let form = self.form.clone();
        form.update(cx, |form, cx| form.focus_first(window, cx));
    }
}

impl Render for NewFirScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let form = self.form.clone();
        let dirty = form.read(cx).is_dirty();
        let saving = form.read(cx).is_saving();
        let nav = self.models.nav.clone();
        v_flex()
            .id("new-fir")
            .key_context("NewFir")
            .track_focus(&self.focus)
            .size_full()
            .child(
                pane_header(&palette)
                    .child(div().text_size(px(14.)).font_medium().child("New FIR"))
                    .child(save_status(&palette, dirty))
                    .child(div().flex_1())
                    .child(
                        Button::new("cancel-new")
                            .ghost()
                            .small()
                            .label("Cancel")
                            .on_click(move |_, window, cx| {
                                nav.update(cx, |nav, cx| nav.back(window, cx))
                            }),
                    )
                    .child(
                        Button::new("create-fir")
                            .primary()
                            .small()
                            .loading(saving)
                            .label("Create FIR")
                            .tooltip("Create FIR  (Ctrl+S)")
                            .on_click({
                                let form = form.clone();
                                move |_, window, cx| {
                                    form.update(cx, |form, cx| form.submit(window, cx))
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .id("new-fir-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().max_w(px(768.)).mx_auto().px_4().child(form)),
            )
    }
}
