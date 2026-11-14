use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable as _, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::{
    App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, Global, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Render, ScrollHandle, Size,
    StatefulInteractiveElement, Styled, Subscription, TestSupportExt as _, WeakEntity, Window,
    canvas, div, point, px,
};
use missal_core::{DisplayMode, Document, FieldCatalog, FirDocumentId, FirValues};
use missal_page::paint::{PagePaint, paint_page};
use missal_page::pdf::{PdfOptions, write_packet};
use missal_page::{Packet, Page};
use missal_text::{TextInput, TextInputEvent, label};

use crate::packet::{LaidOut, PacketJob};
use crate::printing::{self, Handoff};
use crate::shortcuts::*;
use crate::state::{db, toast_error, toast_info, toast_success};
use crate::theme::Palette;
use crate::ui::input::{Kind, text_input};
use crate::ui::{kit_icon_button, small_icon};

const PAD_X: f32 = 48.0;
const PAD_Y: f32 = 36.0;
const GAP: f32 = 24.0;
const FIT_MIN: f32 = 0.25;
const ZOOM_STEPS: [u16; 6] = [50, 75, 100, 125, 150, 200];

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

pub struct PreviewClosed;

impl EventEmitter<PreviewClosed> for PrintPreview {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    Fit,
    Percent(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Busy {
    Exporting,
    Printing,
}

enum State {
    Loading,
    Ready(Ready),
    Failed(String),
}

struct Ready {
    packet: Arc<Packet>,
    pages: Vec<Page>,
    documents: usize,
}

pub struct PrintPreview {
    request: Rc<PrintRequest>,
    focus: FocusHandle,
    state: State,
    attempt: u64,
    zoom: Zoom,
    scroll: ScrollHandle,
    viewport: Size<Pixels>,
    current: usize,
    page_input: Entity<TextInput>,
    page_editing: bool,
    busy: Option<Busy>,
    _subscriptions: Vec<Subscription>,
}

impl PrintPreview {
    pub fn new(request: PrintRequest, window: &mut Window, cx: &mut Context<Self>) -> PrintPreview {
        let page_input = text_input(window, cx, Kind::LatinNumeric, "", "1");
        let subscriptions = vec![cx.subscribe_in(
            &page_input,
            window,
            |this, input, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Change => {
                    let text = input.read(cx).text().to_owned();
                    let digits: String =
                        text.chars().filter(char::is_ascii_digit).take(5).collect();
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
                TextInputEvent::Focus => this.page_editing = true,
                TextInputEvent::Blur => {
                    this.page_editing = false;
                    this.sync_page_input(cx);
                }
            },
        )];
        let mut preview = PrintPreview {
            request: Rc::new(request),
            focus: cx.focus_handle(),
            state: State::Loading,
            attempt: 0,
            zoom: Zoom::Fit,
            scroll: ScrollHandle::new(),
            viewport: window.viewport_size(),
            current: 1,
            page_input,
            page_editing: false,
            busy: None,
            _subscriptions: subscriptions,
        };
        preview.viewport.width *= 0.8;
        preview.start(window, cx);
        preview
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
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

    pub fn page_count(&self) -> usize {
        match &self.state {
            State::Ready(ready) => ready.pages.len(),
            _ => 0,
        }
    }

    pub fn document_count(&self) -> usize {
        match &self.state {
            State::Ready(ready) => ready.documents,
            _ => 0,
        }
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

    pub fn description(&self) -> String {
        let mut parts = vec![self.request.title.clone()];
        if let Some(note) = &self.request.note {
            parts.push(note.clone());
        }
        match &self.state {
            State::Ready(ready) => {
                let documents = ready.documents;
                parts.push(format!(
                    "{documents} {}",
                    if documents == 1 {
                        "document"
                    } else {
                        "documents"
                    }
                ));
                let pages = ready.pages.len();
                parts.push(format!(
                    "{pages} {}",
                    if pages == 1 { "page" } else { "pages" }
                ));
            }
            _ => {
                let documents = self.request.documents.len();
                parts.push(format!(
                    "{documents} {}",
                    if documents == 1 {
                        "document"
                    } else {
                        "documents"
                    }
                ));
            }
        }
        parts.join(" · ")
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.attempt += 1;
        let attempt = self.attempt;
        self.state = State::Loading;
        self.current = 1;
        cx.notify();
        let request = self.request.clone();
        let stored: Vec<FirDocumentId> = request
            .documents
            .iter()
            .filter_map(|document| match document {
                PreviewDocument::Stored(id) => Some(*id),
                PreviewDocument::Content(_) => None,
            })
            .collect();
        let fetch = (!stored.is_empty())
            .then(|| db(cx).run(move |store| store.fir_documents_by_id(&stored)));
        cx.spawn_in(window, async move |this, cx| {
            let fetched = match fetch {
                Some(reply) => match reply.await {
                    Ok(records) => Ok(records),
                    Err(error) => Err(error.message()),
                },
                None => Ok(Vec::new()),
            };
            let outcome: Result<(LaidOut, usize), String> = match fetched {
                Err(message) => Err(message),
                Ok(records) => {
                    let mut stored = records.into_iter();
                    let documents: Vec<Document> = request
                        .documents
                        .iter()
                        .filter_map(|document| match document {
                            PreviewDocument::Content(document) => Some((**document).clone()),
                            PreviewDocument::Stored(_) => {
                                stored.next().map(|record| record.document)
                            }
                        })
                        .collect();
                    let count = documents.len();
                    let job = PacketJob {
                        catalog: request.catalog.clone(),
                        values: request.values.clone(),
                        mode: request.mode,
                        documents,
                    };
                    let laid = cx
                        .background_executor()
                        .spawn(async move { job.lay_out() })
                        .await;
                    Ok((laid, count))
                }
            };
            this.update_in(cx, |this, _, cx| {
                if this.attempt != attempt {
                    return;
                }
                this.state = match outcome {
                    Ok((laid, documents)) => State::Ready(Ready {
                        packet: Arc::new(laid.packet),
                        pages: laid.pages,
                        documents,
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
            self.start(window, cx);
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

    fn fit_scale(&self) -> f32 {
        let available = f32::from(self.viewport.width) - PAD_X * 2.0;
        (available / self.widest()).clamp(FIT_MIN, 1.0)
    }

    fn scale(&self) -> f32 {
        match self.zoom {
            Zoom::Fit => self.fit_scale(),
            Zoom::Percent(percent) => f32::from(percent) / 100.0,
        }
    }

    fn layout(&self, scale: f32) -> (Vec<f32>, f32) {
        let State::Ready(ready) = &self.state else {
            return (Vec::new(), 0.0);
        };
        let mut tops = Vec::with_capacity(ready.pages.len());
        let mut cursor = PAD_Y;
        for page in &ready.pages {
            tops.push(cursor);
            cursor += page.size.height * scale + GAP;
        }
        (tops, cursor - GAP + PAD_Y)
    }

    fn scroll_top(&self) -> f32 {
        -f32::from(self.scroll.offset().y)
    }

    fn page_at_probe(&self) -> usize {
        let State::Ready(ready) = &self.state else {
            return 1;
        };
        let scale = self.scale();
        let (tops, _) = self.layout(scale);
        let probe = self.scroll_top() + f32::from(self.viewport.height) / 3.0;
        ready
            .pages
            .iter()
            .zip(&tops)
            .position(|(page, top)| top + page.size.height * scale >= probe)
            .map_or(ready.pages.len().max(1), |index| index + 1)
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
        self.set_scroll_top(tops[page - 1] - GAP);
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
        cx.emit(PreviewClosed);
    }

    pub fn export_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let State::Ready(ready) = &self.state else {
            return;
        };
        if self.busy.is_some() {
            return;
        }
        let packet = ready.packet.clone();
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
                        toast_error(
                            window,
                            cx,
                            format!("Could not open the save dialog ({error})"),
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
                        let (bytes, _) = write_packet(&packet, &options);
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
                        toast_success(window, cx, format!("PDF saved as {shown}"));
                    }
                    Err(error) => {
                        toast_error(window, cx, format!("Could not save the PDF ({error})"))
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let State::Ready(ready) = &self.state else {
            return;
        };
        if self.busy.is_some() {
            return;
        }
        let packet = ready.packet.clone();
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
                        let (bytes, _) = write_packet(&packet, &options);
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
                                toast_info(window, cx, handoff.message());
                            }
                        }
                        this.close(cx);
                    }
                    Err(error) => toast_error(
                        window,
                        cx,
                        format!("Unable to prepare the print job ({error})"),
                    ),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn header(&self, palette: &Palette, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let ready = self.is_ready();
        let busy = self.busy;
        let description = self.description();
        let title_parts: Vec<&str> = description.splitn(2, " · ").collect();
        let (lead, rest) = (
            title_parts[0].to_owned(),
            title_parts.get(1).map(|rest| format!(" · {rest}")),
        );
        h_flex()
            .flex_shrink_0()
            .h(px(48.))
            .items_center()
            .gap_3()
            .pl_4()
            .pr_2()
            .border_b_1()
            .border_color(palette.line)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_baseline()
                    .gap_2()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(15.))
                            .font_semibold()
                            .child("Print preview"),
                    )
                    .child(
                        h_flex()
                            .min_w_0()
                            .overflow_hidden()
                            .text_size(px(13.))
                            .text_color(palette.ink_muted)
                            .child(div().min_w_0().child(label(lead).truncate()))
                            .children(rest.map(|rest| div().flex_shrink_0().child(rest))),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Button::new("preview-cancel")
                            .ghost()
                            .small()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
                    )
                    .child(
                        Button::new("preview-export")
                            .outline()
                            .small()
                            .icon(IconName::FileDown)
                            .label("Export PDF")
                            .loading(busy == Some(Busy::Exporting))
                            .disabled(!ready || busy.is_some())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.export_pdf(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("preview-print")
                            .primary()
                            .small()
                            .icon(IconName::Printer)
                            .label("Print")
                            .loading(busy == Some(Busy::Printing))
                            .disabled(!ready || busy.is_some())
                            .on_click(cx.listener(|this, _, window, cx| this.print(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn pages_area(&mut self, palette: &Palette, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let palette = *palette;
        match &self.state {
            State::Loading => h_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .text_size(px(13.))
                .text_color(palette.ink_muted)
                .child(Spinner::new())
                .child("Laying out pages…")
                .into_any_element(),
            State::Failed(message) => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .child(small_icon(IconName::TriangleAlert, palette.ink_muted))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_medium()
                        .child("Couldn't prepare the preview"),
                )
                .child(
                    div()
                        .max_w(px(420.))
                        .text_size(px(13.))
                        .text_color(palette.ink_muted)
                        .child(label(message.clone()).align(missal_text::Align::Center)),
                )
                .child(
                    Button::new("preview-retry")
                        .outline()
                        .small()
                        .label("Try again")
                        .on_click(cx.listener(|this, _, window, cx| this.retry(window, cx))),
                )
                .into_any_element(),
            State::Ready(ready) => {
                let scale = self.scale();
                let frames = ready.pages.iter().enumerate().map(|(index, page)| {
                    let width = page.size.width * scale;
                    let height = page.size.height * scale;
                    let page = page.clone();
                    div()
                        .id(("preview-page", index))
                        .test_support()
                        .flex_shrink_0()
                        .w(px(width))
                        .h(px(height))
                        .bg(gpui_kit::white())
                        .border_1()
                        .border_color(palette.line)
                        .shadow_md()
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
                });
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
                            .px(px(PAD_X))
                            .py(px(PAD_Y))
                            .children(frames),
                    )
                    .vertical_scrollbar(&self.scroll)
                    .horizontal_scrollbar(&self.scroll)
                    .into_any_element()
            }
        }
    }

    fn footer(&self, palette: &Palette, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let ready = self.is_ready();
        let count = self.page_count();
        let current = self.current;
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let zoom = self.zoom;
        let percent = self.zoom_percent();
        let zoom_label = match zoom {
            Zoom::Fit => format!("Fit · {percent}%"),
            Zoom::Percent(percent) => format!("{percent}%"),
        };
        h_flex()
            .flex_shrink_0()
            .h(px(40.))
            .items_center()
            .gap_1()
            .px_2()
            .border_t_1()
            .border_color(palette.line)
            .text_size(px(12.))
            .text_color(palette.ink_muted)
            .child(
                kit_icon_button("preview-previous", IconName::ChevronLeft)
                    .tooltip("Previous page")
                    .disabled(!ready || current <= 1)
                    .on_click(cx.listener(|this, _, _, cx| this.step_page(-1, cx))),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .px_1()
                    .child("Page")
                    .child(
                        div()
                            .id("preview-page-input")
                            .w(px(52.))
                            .child(self.page_input.clone()),
                    )
                    .child(format!(
                        "of {}",
                        if count == 0 {
                            "—".to_owned()
                        } else {
                            count.to_string()
                        }
                    )),
            )
            .child(
                kit_icon_button("preview-next", IconName::ChevronRight)
                    .tooltip("Next page")
                    .disabled(!ready || current >= count)
                    .on_click(cx.listener(|this, _, _, cx| this.step_page(1, cx))),
            )
            .child(div().flex_1())
            .child(
                kit_icon_button("preview-zoom-out", IconName::Minus)
                    .tooltip("Zoom out")
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, _, cx| this.step_zoom(-1, cx))),
            )
            .child(
                Button::new("preview-zoom")
                    .outline()
                    .small()
                    .w(px(112.))
                    .label(zoom_label)
                    .disabled(!ready)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        let options = std::iter::once((Zoom::Fit, "Fit width".to_owned())).chain(
                            ZOOM_STEPS
                                .iter()
                                .map(|step| (Zoom::Percent(*step), format!("{step}%"))),
                        );
                        for (option, text) in options {
                            let weak = weak.clone();
                            menu = menu.item(
                                PopupMenuItem::new(text).checked(option == zoom).on_click(
                                    move |_, _, cx| {
                                        weak.update(cx, |this, cx| this.set_zoom(option, cx)).ok();
                                    },
                                ),
                            );
                        }
                        menu
                    }),
            )
            .child(
                kit_icon_button("preview-zoom-in", IconName::Plus)
                    .tooltip("Zoom in")
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, _, cx| this.step_zoom(1, cx))),
            )
            .into_any_element()
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let header = self.header(&palette, cx);
        let footer = self.footer(&palette, cx);
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
        div()
            .id("print-preview")
            .key_context(PRINT_PREVIEW_CONTEXT)
            .track_focus(&self.focus)
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .bg(gpui_kit::black().opacity(0.5))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close(cx)),
            )
            .on_action(cx.listener(|this, _: &PreviewNextPage, _, cx| this.step_page(1, cx)))
            .on_action(cx.listener(|this, _: &PreviewPreviousPage, _, cx| this.step_page(-1, cx)))
            .on_action(cx.listener(|this, _: &PreviewZoomIn, _, cx| this.step_zoom(1, cx)))
            .on_action(cx.listener(|this, _: &PreviewZoomOut, _, cx| this.step_zoom(-1, cx)))
            .on_action(cx.listener(|this, _: &PreviewZoomFit, _, cx| this.set_zoom(Zoom::Fit, cx)))
            .on_action(cx.listener(|this, _: &PreviewClose, _, cx| this.close(cx)))
            .child(
                v_flex()
                    .id("print-preview-panel")
                    .occlude()
                    .size_full()
                    .max_w(px(1152.))
                    .overflow_hidden()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(palette.line_strong)
                    .bg(palette.surface)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(header)
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
    }
}
