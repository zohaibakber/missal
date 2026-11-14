use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui_kit::assets::IconName;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    Anchor, AnyElement, App, AppContext, Bounds, Context, Entity, EventEmitter, FocusHandle,
    Focusable, FontFeatures, FontWeight, Global, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Render, ScrollHandle, SharedString, Size, StatefulInteractiveElement,
    Styled, Subscription, TestSupportExt as _, Window, canvas, div, point, px,
};
use missal_core::{
    DisplayMode, Document, FieldCatalog, FirDocumentId, FirId, FirValues, TemplateId,
};
use missal_page::paint::{PagePaint, paint_page};
use missal_page::pdf::{PdfOptions, write_pages};
use missal_page::{Packet, Page};
use missal_text::{
    Align, Attr, AttributedText, Direction, FontFamily, LineHeight, TextInput, TextInputEvent,
    TextStyle, to_rgba,
};

use crate::packet::{LaidOut, PacketJob, distinct_empty};
use crate::printing::{self, Handoff};
use crate::shortcuts::*;
use crate::state::{
    Models, PrintTarget, Toast, ToastKind, ToastLog, db, show_toast, toast_success,
};
use crate::theme::{ICON_SM, ICON_XS, PANEL_PADDING, Palette, RADIUS, RADIUS_PAPER};
use crate::ui::popup_menu::{Dismissed, Placement, PopupEntry, PopupList, PopupRow};
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::toast::ToastSpec;
use crate::ui::{
    UrRole, button, checkbox, empty_state, icon_button, isolate_urdu, primary_sm, section_label,
    sized_icon, skeleton_bar, ur_inline, ur_label, vertical_rule,
};
use missal_core::digit_value;

const PANEL_WIDTH: f32 = 280.0;
const FOOTER_HEIGHT: f32 = 44.0;
const PAD_TOP: f32 = 20.0;
const FIT_PAGE_MARGIN: f32 = 80.0;
const FIT_WIDTH_MARGIN: f32 = 24.0;
const GAP: f32 = 24.0;
const FIT_MIN: f32 = 0.1;
const ZOOM_STEPS: [u16; 6] = [50, 75, 100, 125, 150, 200];
const MENU_STEPS: [u16; 5] = [50, 75, 100, 125, 150];
const LAYING_OUT_DELAY: Duration = Duration::from_millis(150);

#[derive(Clone)]
pub enum PreviewDocument {
    Content(Box<Document>),
    Stored(FirDocumentId),
}

#[derive(Clone)]
pub struct PrintRequest {
    pub title: String,
    pub note: Option<String>,
    pub export_name: String,
    pub catalog: FieldCatalog,
    pub values: Option<FirValues>,
    pub mode: DisplayMode,
    pub documents: Vec<PreviewDocument>,
}

pub type HandOff = Rc<dyn Fn(&Path, &mut App) -> Handoff>;

#[derive(Clone)]
pub struct PrintHandler(pub HandOff);

impl Global for PrintHandler {}

impl PrintHandler {
    pub fn system() -> PrintHandler {
        PrintHandler(Rc::new(|path, cx| {
            printing::hand_off(path, &printing::current_route(), cx)
        }))
    }
}

#[derive(Default)]
pub struct PrintChoices(HashMap<FirId, HashSet<FirDocumentId>>);

impl Global for PrintChoices {}

pub fn excluded_documents(fir: FirId, cx: &App) -> HashSet<FirDocumentId> {
    cx.try_global::<PrintChoices>()
        .and_then(|choices| choices.0.get(&fir).cloned())
        .unwrap_or_default()
}

fn set_excluded(fir: FirId, excluded: HashSet<FirDocumentId>, cx: &mut App) {
    cx.default_global::<PrintChoices>().0.insert(fir, excluded);
}

pub struct PendingFill(pub FirId);

impl Global for PendingFill {}

pub fn take_fill_request(fir: FirId, cx: &mut App) -> bool {
    let pending = cx
        .try_global::<PendingFill>()
        .is_some_and(|pending| pending.0 == fir);
    if pending {
        cx.remove_global::<PendingFill>();
    }
    pending
}

pub enum PrintSource {
    Request {
        request: Box<PrintRequest>,
        target: Option<PrintTarget>,
        live: Option<FirDocumentId>,
    },
    Target(PrintTarget),
}

pub enum PrintEvent {
    Close,
    FillIn(FirId),
}

impl EventEmitter<PrintEvent> for PrintPreview {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    Fit,
    FitWidth,
    Percent(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Busy {
    Exporting,
    Printing,
}

#[derive(Clone)]
struct DocEntry {
    id: Option<FirDocumentId>,
    title: SharedString,
    source: PreviewDocument,
    included: bool,
}

struct Plan {
    fir: Option<FirId>,
    title: String,
    note: Option<String>,
    export_name: String,
    catalog: FieldCatalog,
    values: Option<FirValues>,
    mode: DisplayMode,
    docs: Vec<DocEntry>,
}

struct Ready {
    pages: Vec<Page>,
    ranges: Vec<Range<usize>>,
    empty: Vec<Vec<String>>,
    _packet: Arc<Packet>,
}

enum State {
    Loading,
    Ready(Ready),
    Failed(String),
}

pub struct PrintPreview {
    target: Option<PrintTarget>,
    source: Option<PrintSource>,
    plan: Option<Plan>,
    request: PrintRequest,
    focus: FocusHandle,
    state: State,
    attempt: u64,
    zoom: Zoom,
    scroll: ScrollHandle,
    viewport: Size<Pixels>,
    current: usize,
    selected_doc: usize,
    page_input: Entity<TextInput>,
    page_editing: bool,
    busy: Option<Busy>,
    show_skeleton: bool,
    menu: Option<Entity<PopupList>>,
    zoom_bounds: Rc<std::cell::Cell<Bounds<Pixels>>>,
    _subscriptions: Vec<Subscription>,
}

impl PrintPreview {
    pub fn new(request: PrintRequest, window: &mut Window, cx: &mut Context<Self>) -> PrintPreview {
        Self::from_source(
            PrintSource::Request {
                request: Box::new(request),
                target: None,
                live: None,
            },
            window,
            cx,
        )
    }

    pub fn from_source(
        source: PrintSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PrintPreview {
        let page_input = cx.new(|cx| {
            TextInput::new(window, cx)
                .bare()
                .direction(Direction::Ltr)
                .align(Align::Center)
                .text_size(13.)
                .line_height(LineHeight::Px(20.))
                .default_text("1".to_owned())
        });
        let subscriptions = vec![cx.subscribe_in(
            &page_input,
            window,
            |this, input, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Change => {
                    let text = input.read(cx).text().to_owned();
                    let digits: String = text
                        .chars()
                        .filter_map(digit_value)
                        .filter_map(|digit| char::from_digit(digit, 10))
                        .take(5)
                        .collect();
                    if digits != text {
                        input.update(cx, |input, cx| input.set_text(digits, cx));
                    }
                }
                TextInputEvent::Submit => {
                    let target = input.read(cx).text().parse::<usize>().ok();
                    if let Some(target) =
                        target.filter(|target| (1..=this.page_count()).contains(target))
                    {
                        this.go_to_page(target, cx);
                    }
                    this.page_editing = false;
                    this.sync_page_input(cx);
                    window.focus(&this.focus, cx);
                }
                TextInputEvent::Focus => {
                    this.page_editing = true;
                    cx.notify();
                }
                TextInputEvent::Blur => {
                    this.page_editing = false;
                    this.sync_page_input(cx);
                    cx.notify();
                }
            },
        )];
        let target = match &source {
            PrintSource::Target(target) => Some(*target),
            PrintSource::Request {
                request, target, ..
            } => target.or_else(|| {
                request
                    .values
                    .as_ref()
                    .map(|values| PrintTarget::Fir(values.fir.id))
            }),
        };
        let mut viewport = window.viewport_size();
        viewport.width -= px(PANEL_WIDTH);
        viewport.height -= px(crate::theme::HEADER_HEIGHT + FOOTER_HEIGHT);
        let mut preview = PrintPreview {
            target,
            source: Some(source),
            plan: None,
            request: PrintRequest {
                title: String::new(),
                note: None,
                export_name: String::new(),
                catalog: FieldCatalog::default(),
                values: None,
                mode: DisplayMode::Labels,
                documents: Vec::new(),
            },
            focus: cx.focus_handle(),
            state: State::Loading,
            attempt: 0,
            zoom: Zoom::Fit,
            scroll: ScrollHandle::new(),
            viewport,
            current: 1,
            selected_doc: 0,
            page_input,
            page_editing: false,
            busy: None,
            show_skeleton: false,
            menu: None,
            zoom_bounds: Rc::new(std::cell::Cell::new(Bounds::default())),
            _subscriptions: subscriptions,
        };
        preview.start(window, cx);
        preview
    }

    pub fn target(&self) -> Option<PrintTarget> {
        self.target
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn is_dirty(&self, _: &App) -> bool {
        false
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    pub fn failure(&self) -> Option<&str> {
        match &self.state {
            State::Failed(message) => Some(message),
            _ => None,
        }
    }

    fn visible_pages(&self) -> Vec<usize> {
        let (State::Ready(ready), Some(plan)) = (&self.state, &self.plan) else {
            return Vec::new();
        };
        plan.docs
            .iter()
            .zip(&ready.ranges)
            .filter(|(doc, _)| doc.included)
            .flat_map(|(_, range)| range.clone())
            .collect()
    }

    pub fn page_count(&self) -> usize {
        match (&self.state, &self.plan) {
            (State::Ready(ready), Some(plan)) => plan
                .docs
                .iter()
                .zip(&ready.ranges)
                .filter(|(doc, _)| doc.included)
                .map(|(_, range)| range.len())
                .sum(),
            _ => 0,
        }
    }

    pub fn document_count(&self) -> usize {
        self.plan.as_ref().map_or(0, |plan| {
            plan.docs.iter().filter(|doc| doc.included).count()
        })
    }

    pub fn document_titles(&self) -> Vec<String> {
        self.plan.as_ref().map_or_else(Vec::new, |plan| {
            plan.docs.iter().map(|doc| doc.title.to_string()).collect()
        })
    }

    pub fn document_pages(&self) -> Vec<usize> {
        match &self.state {
            State::Ready(ready) => ready.ranges.iter().map(|range| range.len()).collect(),
            _ => Vec::new(),
        }
    }

    pub fn empty_fields(&self) -> Vec<String> {
        let (State::Ready(ready), Some(plan)) = (&self.state, &self.plan) else {
            return Vec::new();
        };
        distinct_empty(
            plan.docs
                .iter()
                .zip(&ready.empty)
                .filter(|(doc, _)| doc.included)
                .map(|(_, empty)| empty),
        )
    }

    pub fn current_page(&self) -> usize {
        self.current
    }

    pub fn zoom(&self) -> Zoom {
        self.zoom
    }

    pub fn zoom_percent(&self) -> u16 {
        (self.scale() * 100.0).round() as u16
    }

    pub fn request(&self) -> &PrintRequest {
        &self.request
    }

    pub fn scroll_fraction(&self) -> f32 {
        let (_, total) = self.layout(self.scale());
        if total > 0.0 {
            self.scroll_top() / total
        } else {
            0.0
        }
    }

    pub fn busy(&self) -> Option<Busy> {
        self.busy
    }

    fn plural(count: usize, one: &str, many: &str) -> String {
        format!("{count} {}", if count == 1 { one } else { many })
    }

    pub fn summary(&self) -> String {
        let documents = Self::plural(self.document_count(), "document", "documents");
        if self.is_ready() {
            format!(
                "{documents} · {}",
                Self::plural(self.page_count(), "page", "pages")
            )
        } else {
            documents
        }
    }

    pub fn description(&self) -> String {
        let mut parts = vec![self.request.title.clone()];
        if let Some(note) = &self.request.note {
            parts.push(note.clone());
        }
        parts.push(self.summary());
        parts.join(" · ")
    }

    fn rebuild_request(&mut self) {
        let Some(plan) = &self.plan else {
            return;
        };
        self.request = PrintRequest {
            title: plan.title.clone(),
            note: plan.note.clone(),
            export_name: plan.export_name.clone(),
            catalog: plan.catalog.clone(),
            values: plan.values.clone(),
            mode: plan.mode,
            documents: plan
                .docs
                .iter()
                .filter(|doc| doc.included)
                .map(|doc| doc.source.clone())
                .collect(),
        };
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.attempt += 1;
        let attempt = self.attempt;
        self.state = State::Loading;
        self.show_skeleton = false;
        self.current = 1;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LAYING_OUT_DELAY).await;
            this.update(cx, |this, cx| {
                if this.attempt == attempt {
                    this.show_skeleton = true;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        let catalog = (*Models::get(cx).catalog.read(cx).field_catalog()).clone();
        let source = self.source.take();
        let fetch: PlanFetch = match (self.plan.take(), source) {
            (Some(plan), _) => PlanFetch::Ready(Box::new(plan)),
            (_, Some(source)) => fetch_plan(source, catalog, cx),
            (None, None) => PlanFetch::Ready(Box::new(Plan {
                fir: None,
                title: String::new(),
                note: None,
                export_name: String::new(),
                catalog,
                values: None,
                mode: DisplayMode::Labels,
                docs: Vec::new(),
            })),
        };
        cx.spawn_in(window, async move |this, cx| {
            let plan = match fetch {
                PlanFetch::Ready(plan) => Ok(*plan),
                PlanFetch::Pending(reply) => reply.await,
            };
            let mut plan = match plan {
                Ok(plan) => plan,
                Err(message) => {
                    this.update(cx, |this, cx| {
                        if this.attempt == attempt {
                            this.state = State::Failed(message);
                            cx.notify();
                        }
                    })
                    .ok();
                    return;
                }
            };
            let stored: Vec<FirDocumentId> = plan
                .docs
                .iter()
                .filter_map(|doc| match doc.source {
                    PreviewDocument::Stored(id) => Some(id),
                    PreviewDocument::Content(_) => None,
                })
                .collect();
            let fetched = if stored.is_empty() {
                Ok(Vec::new())
            } else {
                let reply = this
                    .update(cx, |_, cx| {
                        db(cx).run(move |store| store.fir_documents_by_id(&stored))
                    })
                    .ok();
                match reply {
                    Some(reply) => reply.await.map_err(|error| error.message()),
                    None => return,
                }
            };
            let outcome: Result<LaidOut, String> = match fetched {
                Err(message) => Err(message),
                Ok(records) => {
                    let mut by_id: HashMap<FirDocumentId, Document> = records
                        .into_iter()
                        .map(|record| (record.id, record.document))
                        .collect();
                    let mut documents = Vec::with_capacity(plan.docs.len());
                    let mut missing = false;
                    for doc in &plan.docs {
                        match &doc.source {
                            PreviewDocument::Content(document) => {
                                documents.push((**document).clone())
                            }
                            PreviewDocument::Stored(id) => match by_id.remove(id) {
                                Some(document) => documents.push(document),
                                None => missing = true,
                            },
                        }
                    }
                    if missing {
                        Err(
                            "A document to print could not be found. It may have been removed."
                                .to_owned(),
                        )
                    } else {
                        let job = PacketJob {
                            catalog: plan.catalog.clone(),
                            values: plan.values.clone(),
                            mode: plan.mode,
                            documents,
                        };
                        Ok(cx
                            .background_executor()
                            .spawn(async move { job.lay_out() })
                            .await)
                    }
                }
            };
            this.update_in(cx, |this, _, cx| {
                if this.attempt != attempt {
                    return;
                }
                if plan.docs.iter().all(|doc| !doc.included) {
                    for doc in &mut plan.docs {
                        doc.included = true;
                    }
                }
                this.plan = Some(plan);
                this.rebuild_request();
                this.state = match outcome {
                    Ok(laid) => State::Ready(Ready {
                        pages: laid.pages,
                        ranges: laid.ranges,
                        empty: laid.empty,
                        _packet: Arc::new(laid.packet),
                    }),
                    Err(message) => State::Failed(message),
                };
                this.current = 1;
                this.sync_page_input(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.state, State::Failed(_)) {
            if self.plan.is_none()
                && self.source.is_none()
                && let Some(target) = self.target
            {
                self.source = Some(PrintSource::Target(target));
            }
            self.start(window, cx);
        }
    }

    pub fn set_included(&mut self, index: usize, included: bool, cx: &mut Context<Self>) {
        let Some(plan) = self.plan.as_mut() else {
            return;
        };
        let Some(doc) = plan.docs.get_mut(index) else {
            return;
        };
        if doc.included == included {
            return;
        }
        doc.included = included;
        self.selected_doc = index;
        if let Some(fir) = plan.fir {
            let excluded = plan
                .docs
                .iter()
                .filter(|doc| !doc.included)
                .filter_map(|doc| doc.id)
                .collect();
            set_excluded(fir, excluded, cx);
        }
        self.rebuild_request();
        let count = self.page_count();
        self.current = self.current.clamp(1, count.max(1));
        self.set_scroll_top(0.);
        self.current = 1;
        self.sync_page_input(cx);
        cx.notify();
    }

    pub fn toggle_included(&mut self, index: usize, cx: &mut Context<Self>) {
        let included = self
            .plan
            .as_ref()
            .and_then(|plan| plan.docs.get(index))
            .is_some_and(|doc| doc.included);
        self.set_included(index, !included, cx);
    }

    pub fn show_document(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_doc = index;
        let (State::Ready(ready), Some(plan)) = (&self.state, &self.plan) else {
            return;
        };
        if !plan.docs.get(index).is_some_and(|doc| doc.included) {
            cx.notify();
            return;
        }
        let first = ready.ranges[index].start;
        if let Some(position) = self.visible_pages().iter().position(|page| *page == first) {
            self.go_to_page(position + 1, cx);
        }
    }

    pub fn move_document(&mut self, delta: isize, cx: &mut Context<Self>) {
        let index = self.selected_doc;
        let Some(plan) = self.plan.as_mut() else {
            return;
        };
        let target = index as isize + delta;
        if target < 0 || target as usize >= plan.docs.len() || plan.fir.is_none() {
            return;
        }
        let target = target as usize;
        plan.docs.swap(index, target);
        if let State::Ready(ready) = &mut self.state {
            ready.ranges.swap(index, target);
            ready.empty.swap(index, target);
        }
        self.selected_doc = target;
        let fir = plan.fir;
        let order: Vec<FirDocumentId> = plan.docs.iter().filter_map(|doc| doc.id).collect();
        self.rebuild_request();
        if let State::Ready(_) = self.state {
            self.reorder_pages(cx);
        }
        if let Some(fir) = fir {
            let reply = db(cx).run(move |store| store.reorder_fir_documents(fir, &order));
            cx.spawn(async move |_, _| {
                reply.await.ok();
            })
            .detach();
        }
        cx.notify();
    }

    fn reorder_pages(&mut self, cx: &mut Context<Self>) {
        let State::Ready(ready) = &mut self.state else {
            return;
        };
        let mut pages = Vec::with_capacity(ready.pages.len());
        let mut ranges = Vec::with_capacity(ready.ranges.len());
        for range in &ready.ranges {
            let start = pages.len();
            pages.extend(ready.pages[range.clone()].iter().cloned());
            ranges.push(start..pages.len());
        }
        ready.pages = pages;
        ready.ranges = ranges;
        self.sync_page_input(cx);
    }

    pub fn fill_in(&mut self, cx: &mut Context<Self>) {
        if let Some(fir) = self.plan.as_ref().and_then(|plan| plan.fir) {
            cx.set_global(PendingFill(fir));
            cx.emit(PrintEvent::FillIn(fir));
        }
    }

    fn widest(&self) -> f32 {
        match &self.state {
            State::Ready(ready) => ready
                .pages
                .iter()
                .map(|page| page.size.width)
                .fold(1.0, f32::max),
            _ => 1.0,
        }
    }

    fn tallest(&self) -> f32 {
        match &self.state {
            State::Ready(ready) => ready
                .pages
                .iter()
                .map(|page| page.size.height)
                .fold(1.0, f32::max),
            _ => 1.0,
        }
    }

    fn fit_width_scale(&self) -> f32 {
        let available = f32::from(self.viewport.width) - FIT_WIDTH_MARGIN * 2.0;
        (available / self.widest()).clamp(FIT_MIN, 4.0)
    }

    fn fit_page_scale(&self) -> f32 {
        let height = f32::from(self.viewport.height) - FIT_PAGE_MARGIN;
        (height / self.tallest())
            .min(self.fit_width_scale())
            .clamp(FIT_MIN, 4.0)
    }

    fn scale(&self) -> f32 {
        match self.zoom {
            Zoom::Fit => self.fit_page_scale(),
            Zoom::FitWidth => self.fit_width_scale(),
            Zoom::Percent(percent) => f32::from(percent) / 100.0,
        }
    }

    fn layout(&self, scale: f32) -> (Vec<f32>, f32) {
        let State::Ready(ready) = &self.state else {
            return (Vec::new(), 0.0);
        };
        let visible = self.visible_pages();
        let mut tops = Vec::with_capacity(visible.len());
        let mut cursor = PAD_TOP;
        for index in &visible {
            tops.push(cursor);
            cursor += ready.pages[*index].size.height * scale + GAP;
        }
        (tops, cursor - GAP + PAD_TOP)
    }

    fn scroll_top(&self) -> f32 {
        -f32::from(self.scroll.offset().y)
    }

    fn page_at_probe(&self) -> usize {
        let State::Ready(ready) = &self.state else {
            return 1;
        };
        let visible = self.visible_pages();
        let scale = self.scale();
        let (tops, _) = self.layout(scale);
        let probe = self.scroll_top() + f32::from(self.viewport.height) / 3.0;
        visible
            .iter()
            .zip(&tops)
            .position(|(page, top)| top + ready.pages[*page].size.height * scale >= probe)
            .map_or(visible.len().max(1), |index| index + 1)
    }

    fn observe(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        let mut changed = false;
        if self.viewport != bounds.size {
            self.viewport = bounds.size;
            changed = true;
        }
        let current = self.page_at_probe();
        if current != self.current {
            self.current = current;
            self.sync_page_input(cx);
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }

    fn sync_page_input(&mut self, cx: &mut Context<Self>) {
        if self.page_editing {
            return;
        }
        let shown = self.current.to_string();
        self.page_input.update(cx, |input, cx| {
            if input.text() != shown {
                input.set_text(shown, cx);
            }
        });
    }

    fn set_scroll_top(&mut self, top: f32) {
        let offset = self.scroll.offset();
        self.scroll.set_offset(point(offset.x, px(-top.max(0.0))));
    }

    pub fn go_to_page(&mut self, page: usize, cx: &mut Context<Self>) {
        let count = self.page_count();
        if count == 0 {
            return;
        }
        let page = page.clamp(1, count);
        let (tops, _) = self.layout(self.scale());
        self.set_scroll_top(tops[page - 1] - PAD_TOP);
        self.current = page;
        self.sync_page_input(cx);
        cx.notify();
    }

    pub fn step_page(&mut self, delta: isize, cx: &mut Context<Self>) {
        let target = (self.current as isize + delta).max(1) as usize;
        self.go_to_page(target, cx);
    }

    pub fn set_zoom(&mut self, zoom: Zoom, cx: &mut Context<Self>) {
        let (_, before) = self.layout(self.scale());
        let fraction = if before > 0.0 {
            self.scroll_top() / before
        } else {
            0.0
        };
        self.zoom = zoom;
        let (_, after) = self.layout(self.scale());
        self.set_scroll_top(fraction * after);
        cx.notify();
    }

    pub fn step_zoom(&mut self, direction: i32, cx: &mut Context<Self>) {
        let percent = self.zoom_percent();
        let next = if direction > 0 {
            ZOOM_STEPS.iter().copied().find(|step| *step > percent)
        } else {
            ZOOM_STEPS
                .iter()
                .rev()
                .copied()
                .find(|step| *step < percent)
        };
        if let Some(next) = next {
            self.set_zoom(Zoom::Percent(next), cx);
        }
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(PrintEvent::Close);
    }

    fn selected_pages(&self) -> Option<Vec<Page>> {
        let State::Ready(ready) = &self.state else {
            return None;
        };
        let pages: Vec<Page> = self
            .visible_pages()
            .into_iter()
            .map(|index| ready.pages[index].clone())
            .collect();
        (!pages.is_empty()).then_some(pages)
    }

    pub fn export_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        let Some(pages) = self.selected_pages() else {
            return;
        };
        let title = self.request.export_name.clone();
        let name = printing::file_name_for(&title);
        let directory = export_directory();
        let chosen = cx.prompt_for_new_path(&directory, Some(&name));
        self.busy = Some(Busy::Exporting);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let path = match chosen.await {
                Ok(Ok(Some(path))) => Some(path),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    this.update_in(cx, |this, window, cx| {
                        this.busy = None;
                        logged_toast(
                            window,
                            cx,
                            ToastSpec::new(
                                ToastKind::Error,
                                format!("Couldn't open the save dialog ({error})"),
                            ),
                        );
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let Some(path) = path else {
                this.update(cx, |this, cx| {
                    this.busy = None;
                    cx.notify();
                })
                .ok();
                return;
            };
            let path = with_pdf_extension(path);
            let written = cx
                .background_executor()
                .spawn({
                    let path = path.clone();
                    let title = title.clone();
                    async move {
                        let options = PdfOptions {
                            title: Some(title),
                            compress: true,
                        };
                        let (bytes, _) = write_pages(&pages, &options);
                        std::fs::write(&path, bytes)
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.busy = None;
                match written {
                    Ok(()) => {
                        let shown = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let reveal = path.clone();
                        logged_toast(
                            window,
                            cx,
                            ToastSpec::new(ToastKind::Success, format!("Saved {shown}"))
                                .action("Show in folder", move |_, cx| cx.reveal_path(&reveal)),
                        );
                    }
                    Err(error) => {
                        let retry = cx.entity().downgrade();
                        logged_toast(
                            window,
                            cx,
                            ToastSpec::new(
                                ToastKind::Error,
                                format!("Couldn't save the PDF ({error})"),
                            )
                            .action("Retry", move |window, cx| {
                                retry
                                    .update(cx, |this, cx| this.export_pdf(window, cx))
                                    .ok();
                            }),
                        )
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        let Some(pages) = self.selected_pages() else {
            return;
        };
        let title = self.request.export_name.clone();
        self.busy = Some(Busy::Printing);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let spooled = cx
                .background_executor()
                .spawn({
                    let title = title.clone();
                    async move {
                        let options = PdfOptions {
                            title: Some(title.clone()),
                            compress: true,
                        };
                        let (bytes, _) = write_pages(&pages, &options);
                        printing::spool(
                            &printing::spool_directory(),
                            &title,
                            &bytes,
                            SystemTime::now(),
                        )
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.busy = None;
                match spooled {
                    Ok(path) => {
                        let handler = cx
                            .try_global::<PrintHandler>()
                            .cloned()
                            .unwrap_or_else(PrintHandler::system);
                        let handoff = (handler.0)(&path, cx);
                        match handoff {
                            Handoff::PrintDialog => toast_success(window, cx, handoff.message()),
                            Handoff::DefaultViewer => {
                                crate::state::toast_info(window, cx, handoff.message());
                            }
                        }
                        this.close(cx);
                    }
                    Err(error) => {
                        let retry = cx.entity().downgrade();
                        logged_toast(
                            window,
                            cx,
                            ToastSpec::new(
                                ToastKind::Error,
                                format!("Couldn't prepare the print job ({error})"),
                            )
                            .action("Retry", move |window, cx| {
                                retry.update(cx, |this, cx| this.print(window, cx)).ok();
                            }),
                        );
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_zoom_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let current = self.zoom;
        let mut rows = Vec::new();
        let options = [
            (Zoom::Fit, "Fit page".to_owned()),
            (Zoom::FitWidth, "Fit width".to_owned()),
        ]
        .into_iter()
        .chain(
            MENU_STEPS
                .iter()
                .map(|step| (Zoom::Percent(*step), format!("{step}%"))),
        );
        for (option, text) in options {
            let weak = weak.clone();
            rows.push(PopupRow::Entry(
                PopupEntry::new(
                    SharedString::from(format!("zoom-{}", text.to_lowercase().replace(' ', "-"))),
                    text,
                    move |_, cx| {
                        weak.update(cx, |this, cx| this.set_zoom(option, cx)).ok();
                    },
                )
                .check(option == current),
            ));
        }
        let bounds = self.zoom_bounds.get();
        let menu = PopupList::open(
            "zoom-menu",
            rows,
            Placement {
                position: point(bounds.right(), bounds.top() - px(4.)),
                anchor: Anchor::BottomRight,
                width: 160.,
            },
            window,
            cx,
        );
        cx.subscribe_in(&menu, window, |this, _, _: &Dismissed, window, cx| {
            if this.menu.take().is_some() {
                window.focus(&this.focus, cx);
                cx.notify();
            }
        })
        .detach();
        self.menu = Some(menu);
        cx.notify();
    }

    fn warning(&self, palette: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let empty = self.empty_fields();
        let first = empty.first()?;
        let count = empty.len();
        let lead = format!(
            "{} empty.",
            if count == 1 {
                "1 field is".to_owned()
            } else {
                format!("{count} fields are")
            }
        );
        let middle = " They will print as their names, for example ";
        let name = isolate_urdu(first);
        let link = "Fill them in";
        let text = format!("{lead}{middle}{name}. {link}");
        let base = TextStyle::interface(13.).with_color(to_rgba(palette.notice_text));
        let name_start = lead.len() + middle.len();
        let link_start = text.len() - link.len();
        let rich = AttributedText::new(text.clone(), base)
            .with(0..lead.len(), Attr::Weight(600))
            .with(
                name_start..name_start + name.len(),
                Attr::Family(FontFamily::Document),
            )
            .with(name_start..name_start + name.len(), Attr::Size(14.))
            .with(link_start..text.len(), Attr::Underline(true))
            .with(link_start..text.len(), Attr::Weight(500));
        let fill = self.plan.as_ref().and_then(|plan| plan.fir).is_some();
        Some(
            h_flex()
                .id("print-warning")
                .test_support()
                .m(px(12.))
                .px(px(12.))
                .py(px(10.))
                .gap(px(10.))
                .items_start()
                .rounded(px(crate::theme::RADIUS_LG))
                .bg(palette.notice_wash)
                .when(fill, |this| {
                    this.cursor_pointer()
                        .role(gpui_kit::accesskit::Role::Link)
                        .aria_label(SharedString::from(format!("{lead} {link}")))
                        .on_click(cx.listener(|this, _, _, cx| this.fill_in(cx)))
                })
                .child(div().pt(px(2.)).child(sized_icon(
                    IconName::TriangleAlert,
                    ICON_SM,
                    palette.notice_text,
                )))
                .child(
                    div().flex_1().min_w_0().child(
                        missal_text::Label::rich(rich)
                            .ltr()
                            .align(Align::Left)
                            .leading(LineHeight::Px(18.)),
                    ),
                )
                .into_any_element(),
        )
    }

    fn panel(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let current_doc = self.current_document();
        let ranges: Vec<usize> = self.document_pages();
        let docs: Vec<DocEntry> = self
            .plan
            .as_ref()
            .map(|plan| plan.docs.clone())
            .unwrap_or_default();
        let hover = palette.hover;
        let warning = self.warning(&palette, cx);
        let has_warning = warning.is_some();
        let rows = docs.into_iter().enumerate().map(|(index, doc)| {
            let pages = ranges.get(index).copied();
            let on = current_doc == Some(index);
            h_flex()
                .id(("print-doc", index))
                .test_support()
                .flex_none()
                .h(px(44.))
                .mx(px(8.))
                .px(px(10.))
                .gap(px(10.))
                .flex_row_reverse()
                .items_center()
                .rounded(px(RADIUS))
                .cursor_pointer()
                .when(on, |this| this.bg(hover))
                .when(!on, |this| this.hover(move |style| style.bg(hover)))
                .on_click(cx.listener(move |this, _, _, cx| this.show_document(index, cx)))
                .child(
                    checkbox(&palette, ("print-include", index), doc.included)
                        .cursor_pointer()
                        .aria_label(SharedString::from(format!("Print {}", doc.title)))
                        .test_support()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_included(index, cx)
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(if doc.included {
                            palette.ink
                        } else {
                            palette.ink_faint
                        })
                        .child(ur_label(doc.title.clone(), UrRole::Body).truncate()),
                )
                .children(pages.map(|pages| {
                    div()
                        .flex_none()
                        .text_size(px(12.))
                        .text_color(palette.ink_faint)
                        .font_features(tabular())
                        .child(Self::plural(pages, "page", "pages"))
                }))
        });
        v_flex()
            .id("print-panel")
            .w(px(PANEL_WIDTH))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(palette.line)
            .children(warning)
            .child(
                div()
                    .h(px(32.))
                    .flex_none()
                    .px(px(PANEL_PADDING))
                    .flex()
                    .items_center()
                    .when(!has_warning, |this| this.mt(px(8.)))
                    .child(section_label(&palette, "Documents to print")),
            )
            .child(
                v_flex()
                    .id("print-documents")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(
                div()
                    .flex_none()
                    .px(px(PANEL_PADDING))
                    .py(px(12.))
                    .border_t_1()
                    .border_color(palette.line)
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_faint)
                    .child("You choose the printer in the next step."),
            )
            .into_any_element()
    }

    fn current_document(&self) -> Option<usize> {
        let (State::Ready(ready), Some(_)) = (&self.state, &self.plan) else {
            return None;
        };
        let visible = self.visible_pages();
        let page = *visible.get(self.current.checked_sub(1)?)?;
        ready.ranges.iter().position(|range| range.contains(&page))
    }

    fn pages_area(&mut self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        match &self.state {
            State::Loading => {
                let scale = (f32::from(self.viewport.height) - FIT_PAGE_MARGIN) / 1123.0;
                v_flex()
                    .size_full()
                    .items_center()
                    .pt(px(PAD_TOP))
                    .when(self.show_skeleton, |this| {
                        this.child(
                            v_flex()
                                .id("print-skeleton")
                                .w(px(794. * scale))
                                .h(px(1123. * scale))
                                .rounded(px(RADIUS_PAPER))
                                .bg(palette.surface)
                                .shadow(palette.paper_shadow())
                                .p(px(48. * scale))
                                .gap(px(14.))
                                .items_end()
                                .child(skeleton_bar(&palette, 160., 12.))
                                .child(skeleton_bar(&palette, 260., 12.))
                                .child(skeleton_bar(&palette, 220., 12.))
                                .child(skeleton_bar(&palette, 300., 12.)),
                        )
                    })
                    .into_any_element()
            }
            State::Failed(message) => empty_state(
                &palette,
                IconName::TriangleAlert,
                "Couldn't prepare the preview",
                Some(message.clone().into()),
                Some(
                    button("preview-retry", "Try again")
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| this.retry(window, cx)))
                        .into_any_element(),
                ),
            )
            .into_any_element(),
            State::Ready(_) if self.document_count() == 0 => empty_state(
                &palette,
                IconName::Printer,
                "Nothing to print",
                Some("Tick a document on the left to print it.".into()),
                None,
            )
            .into_any_element(),
            State::Ready(ready) => {
                let scale = self.scale();
                let frames: Vec<AnyElement> = self
                    .visible_pages()
                    .into_iter()
                    .enumerate()
                    .map(|(position, index)| {
                        let page = ready.pages[index].clone();
                        let width = page.size.width * scale;
                        let height = page.size.height * scale;
                        div()
                            .id(("preview-page", position))
                            .test_support()
                            .flex_none()
                            .w(px(width))
                            .h(px(height))
                            .rounded(px(RADIUS_PAPER))
                            .bg(gpui_kit::white())
                            .shadow(palette.paper_shadow())
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds, _, window, cx| {
                                        if window.content_mask().bounds.intersects(&bounds) {
                                            paint_page(
                                                &page,
                                                bounds.origin,
                                                &PagePaint::print(scale),
                                                window,
                                                cx,
                                            );
                                        }
                                    },
                                )
                                .size_full(),
                            )
                            .into_any_element()
                    })
                    .collect();
                div()
                    .id("preview-scroll")
                    .size_full()
                    .relative()
                    .flex()
                    .items_start()
                    .track_scroll(&self.scroll)
                    .overflow_scroll()
                    .child(
                        v_flex()
                            .flex_none()
                            .min_w_full()
                            .items_center()
                            .gap(px(GAP))
                            .px(px(FIT_WIDTH_MARGIN))
                            .py(px(PAD_TOP))
                            .children(frames),
                    )
                    .vertical_scrollbar(&self.scroll)
                    .horizontal_scrollbar(&self.scroll)
                    .into_any_element()
            }
        }
    }

    fn footer(&self, palette: &Palette, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let ready = self.is_ready() && self.document_count() > 0;
        let count = self.page_count();
        let current = self.current;
        let zoom_label = match self.zoom {
            Zoom::Fit => "Fit page".to_owned(),
            Zoom::FitWidth => "Fit width".to_owned(),
            Zoom::Percent(percent) => format!("{percent}%"),
        };
        let page_focused = self.page_input.focus_handle(cx).is_focused(window);
        let zoom_bounds = self.zoom_bounds.clone();
        let menu_open = self.menu.is_some();
        let hover = palette.hover;
        let laying_out = matches!(self.state, State::Loading) && self.show_skeleton;
        let page_input = self.page_input.clone();
        h_flex()
            .id("print-footer")
            .flex_none()
            .h(px(FOOTER_HEIGHT))
            .items_center()
            .gap(px(8.))
            .px(px(PANEL_PADDING))
            .border_t_1()
            .border_color(palette.line)
            .bg(palette.surface)
            .text_size(px(13.))
            .text_color(palette.ink_muted)
            .font_features(tabular())
            .child(
                icon_button("preview-previous", IconName::ChevronLeft, "Previous page")
                    .keys("PageUp")
                    .dense()
                    .disabled(!ready || current <= 1)
                    .on_click(cx.listener(|this, _, _, cx| this.step_page(-1, cx))),
            )
            .child("Page")
            .child(
                div()
                    .id("preview-page-input")
                    .w(px(44.))
                    .h(px(28.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(4.))
                    .rounded(px(RADIUS))
                    .border_1()
                    .border_color(if page_focused {
                        palette.accent
                    } else {
                        palette.line_strong
                    })
                    .bg(palette.surface)
                    .text_color(palette.ink)
                    .when(page_focused, |this| this.shadow(palette.input_focus_ring()))
                    .cursor_text()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        page_input.update(cx, |input, cx| input.focus(window, cx));
                    })
                    .child(div().w_full().child(self.page_input.clone())),
            )
            .child(format!(
                "of {}",
                if count == 0 {
                    "—".to_owned()
                } else {
                    count.to_string()
                }
            ))
            .child(
                icon_button("preview-next", IconName::ChevronRight, "Next page")
                    .keys("PageDown")
                    .dense()
                    .disabled(!ready || current >= count)
                    .on_click(cx.listener(|this, _, _, cx| this.step_page(1, cx))),
            )
            .when(laying_out, |this| {
                this.child(
                    div()
                        .ml(px(8.))
                        .text_size(px(12.))
                        .text_color(palette.ink_muted)
                        .child("Laying out pages…"),
                )
            })
            .child(div().flex_1())
            .child(
                icon_button("preview-zoom-out", IconName::Minus, "Zoom out")
                    .keys("Mod+-")
                    .dense()
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, _, cx| this.step_zoom(-1, cx))),
            )
            .child(
                h_flex()
                    .id("preview-zoom")
                    .test_support()
                    .relative()
                    .flex_none()
                    .h(px(28.))
                    .pl(px(10.))
                    .pr(px(8.))
                    .gap(px(6.))
                    .items_center()
                    .rounded(px(RADIUS))
                    .border_1()
                    .border_color(palette.line)
                    .text_color(palette.ink)
                    .cursor_pointer()
                    .when(menu_open, |this| this.bg(hover))
                    .when(!ready, |this| this.opacity(0.4))
                    .hover(move |style| style.bg(hover))
                    .role(gpui_kit::accesskit::Role::ComboBox)
                    .aria_label("Zoom")
                    .when(ready, |this| {
                        this.on_click(
                            cx.listener(|this, _, window, cx| this.open_zoom_menu(window, cx)),
                        )
                    })
                    .child(
                        canvas(move |bounds, _, _| zoom_bounds.set(bounds), |_, _, _, _| {})
                            .absolute()
                            .size_full(),
                    )
                    .child(zoom_label)
                    .child(sized_icon(
                        IconName::ChevronDown,
                        ICON_XS,
                        palette.ink_faint,
                    )),
            )
            .child(
                icon_button("preview-zoom-in", IconName::Plus, "Zoom in")
                    .keys("Mod+=")
                    .dense()
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, _, cx| this.step_zoom(1, cx))),
            )
            .into_any_element()
    }
}

fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

fn logged_toast(window: &mut Window, cx: &mut App, spec: ToastSpec) {
    if let Some(log) = cx.try_global::<ToastLog>() {
        log.0.borrow_mut().push(Toast {
            kind: spec.kind,
            title: spec.title.as_ref().map(|title| title.to_string()),
            message: spec.message.to_string(),
        });
    }
    show_toast(window, cx, spec);
}

type PendingPlan = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Plan, String>>>>;

enum PlanFetch {
    Ready(Box<Plan>),
    Pending(PendingPlan),
}

fn fetch_plan(source: PrintSource, catalog: FieldCatalog, cx: &App) -> PlanFetch {
    match source {
        PrintSource::Request {
            request,
            target,
            live,
        } => {
            let fir = request.values.as_ref().map(|values| values.fir.id);
            let Some(fir) = fir else {
                let docs = request
                    .documents
                    .into_iter()
                    .map(|source| DocEntry {
                        id: match source {
                            PreviewDocument::Stored(id) => Some(id),
                            PreviewDocument::Content(_) => None,
                        },
                        title: request.title.clone().into(),
                        source,
                        included: true,
                    })
                    .collect();
                let _ = target;
                return PlanFetch::Ready(Box::new(Plan {
                    fir: None,
                    title: request.title,
                    note: request.note,
                    export_name: request.export_name,
                    catalog: request.catalog,
                    values: request.values,
                    mode: request.mode,
                    docs,
                }));
            };
            let reply = db(cx).run(move |store| store.fir_documents(fir));
            let included: HashSet<FirDocumentId> = request
                .documents
                .iter()
                .filter_map(|document| match document {
                    PreviewDocument::Stored(id) => Some(*id),
                    PreviewDocument::Content(_) => None,
                })
                .chain(live)
                .collect();
            let content = request
                .documents
                .iter()
                .find_map(|document| match document {
                    PreviewDocument::Content(document) => Some(document.clone()),
                    PreviewDocument::Stored(_) => None,
                });
            PlanFetch::Pending(Box::pin(async move {
                let summaries = reply.await.map_err(|error| error.message())?;
                let mut docs: Vec<DocEntry> = summaries
                    .into_iter()
                    .map(|summary| DocEntry {
                        id: Some(summary.id),
                        title: summary.title.into(),
                        source: match (&content, live) {
                            (Some(document), Some(live)) if live == summary.id => {
                                PreviewDocument::Content(document.clone())
                            }
                            _ => PreviewDocument::Stored(summary.id),
                        },
                        included: included.contains(&summary.id),
                    })
                    .collect();
                if docs.is_empty() {
                    docs = request
                        .documents
                        .into_iter()
                        .map(|source| DocEntry {
                            id: match source {
                                PreviewDocument::Stored(id) => Some(id),
                                PreviewDocument::Content(_) => None,
                            },
                            title: request.title.clone().into(),
                            source,
                            included: true,
                        })
                        .collect();
                }
                Ok(Plan {
                    fir: Some(fir),
                    title: request.title,
                    note: request.note,
                    export_name: request.export_name,
                    catalog: request.catalog,
                    values: request.values,
                    mode: request.mode,
                    docs,
                })
            }))
        }
        PrintSource::Target(PrintTarget::Fir(fir)) => {
            let excluded = excluded_documents(fir, cx);
            let reply = db(cx).run(move |store| store.fir_workspace(fir));
            PlanFetch::Pending(Box::pin(async move {
                let workspace = reply.await.map_err(|error| error.message())?;
                let number = workspace.values.fir.fir.fir_no.clone();
                let docs = workspace
                    .documents
                    .into_iter()
                    .map(|summary| DocEntry {
                        id: Some(summary.id),
                        title: summary.title.into(),
                        source: PreviewDocument::Stored(summary.id),
                        included: !excluded.contains(&summary.id),
                    })
                    .collect();
                let title = format!("FIR {number}");
                Ok(Plan {
                    fir: Some(fir),
                    export_name: title.clone(),
                    title,
                    note: None,
                    catalog,
                    values: Some(workspace.values),
                    mode: DisplayMode::Values,
                    docs,
                })
            }))
        }
        PrintSource::Target(PrintTarget::Template(template)) => {
            let reply = db(cx).run(move |store| store.template(template));
            PlanFetch::Pending(Box::pin(async move {
                let record = reply.await.map_err(|error| error.message())?;
                Ok(template_plan(
                    template,
                    record.name,
                    record.document,
                    catalog,
                ))
            }))
        }
    }
}

fn template_plan(_: TemplateId, name: String, document: Document, catalog: FieldCatalog) -> Plan {
    Plan {
        fir: None,
        title: name.clone(),
        note: Some("Placeholders show their names".to_owned()),
        export_name: name.clone(),
        catalog,
        values: None,
        mode: DisplayMode::Labels,
        docs: vec![DocEntry {
            id: None,
            title: name.into(),
            source: PreviewDocument::Content(Box::new(document)),
            included: true,
        }],
    }
}

fn with_pdf_extension(mut path: PathBuf) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension("pdf");
    }
    path
}

fn export_directory() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    match home {
        Some(home) => {
            let documents = home.join("Documents");
            if documents.is_dir() { documents } else { home }
        }
        None => std::env::temp_dir(),
    }
}

impl Focusable for PrintPreview {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PrintPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let panel = self.panel(&palette, cx);
        let footer = self.footer(&palette, window, cx);
        let area = self.pages_area(&palette, cx);
        let weak = cx.entity().downgrade();
        let probe = canvas(
            |_, _, _| (),
            move |bounds, _, _, cx| {
                weak.update(cx, |this, cx| this.observe(bounds, cx)).ok();
            },
        )
        .absolute()
        .size_full();
        h_flex()
            .id("print-preview")
            .key_context(PRINT_PREVIEW)
            .track_focus(&self.focus)
            .size_full()
            .bg(palette.surface)
            .on_action(cx.listener(|this, _: &PreviewNextPage, _, cx| this.step_page(1, cx)))
            .on_action(cx.listener(|this, _: &PreviewPreviousPage, _, cx| this.step_page(-1, cx)))
            .on_action(cx.listener(|this, _: &PreviewZoomIn, _, cx| this.step_zoom(1, cx)))
            .on_action(cx.listener(|this, _: &PreviewZoomOut, _, cx| this.step_zoom(-1, cx)))
            .on_action(cx.listener(|this, _: &PreviewZoomFit, _, cx| this.set_zoom(Zoom::Fit, cx)))
            .on_action(cx.listener(|this, _: &PreviewClose, _, cx| this.close(cx)))
            .on_action(cx.listener(|this, _: &Print, window, cx| this.print(window, cx)))
            .on_action(cx.listener(|this, _: &SavePdf, window, cx| this.export_pdf(window, cx)))
            .on_action(cx.listener(|this, _: &MoveDocumentUp, _, cx| this.move_document(-1, cx)))
            .on_action(cx.listener(|this, _: &MoveDocumentDown, _, cx| this.move_document(1, cx)))
            .child(panel)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        div()
                            .id("preview-viewport")
                            .flex_1()
                            .min_h_0()
                            .relative()
                            .bg(palette.sunken)
                            .child(probe)
                            .child(area),
                    )
                    .child(footer),
            )
            .children(self.menu.clone().map(|menu| menu.into_any_element()))
    }
}

impl HeaderSource for PrintPreview {
    fn header(&mut self, _: &mut Window, cx: &mut Context<Self>) -> HeaderContent {
        let palette = *Palette::get(cx);
        let ready = self.is_ready() && self.document_count() > 0;
        let busy = self.busy;
        let fir = matches!(self.target, Some(PrintTarget::Fir(_)))
            || self.plan.as_ref().is_some_and(|plan| plan.fir.is_some());
        let title = &self.request.title;
        let content = if self.plan.is_none() {
            HeaderContent::new("Print")
        } else if fir {
            HeaderContent::new(format!("Print {title}"))
        } else {
            HeaderContent::element(
                h_flex().items_center().gap(px(8.)).child("Preview").child(
                    div()
                        .mt(px(-2.))
                        .font_weight(FontWeight::NORMAL)
                        .child(ur_inline(title.clone(), UrRole::Title).truncate()),
                ),
            )
        };
        let meta = match &self.request.note {
            Some(note) if self.plan.is_some() => format!("{note} · {}", self.summary()),
            _ if self.plan.is_some() => self.summary(),
            _ => String::new(),
        };
        content
            .meta(meta)
            .action(
                icon_button("preview-export", IconName::FileDown, "Save as PDF")
                    .keys("Mod+Shift+S")
                    .disabled(!ready || busy.is_some())
                    .on_click(cx.listener(|this, _, window, cx| this.export_pdf(window, cx))),
            )
            .action(
                primary_sm("preview-print", "Print")
                    .icon(IconName::Printer)
                    .keys("Mod+P")
                    .disabled(!ready || busy.is_some())
                    .on_click(cx.listener(|this, _, window, cx| this.print(window, cx))),
            )
            .action(div().mx(px(4.)).child(vertical_rule(&palette)))
    }
}
