use std::{ops::Range, sync::Arc};

use gpui_kit::{
    prelude::FluentBuilder,
    AppContext, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, canvas, div, hsla, point, px, rgb, size,
};
use missal_text::{
    Align, Attr, AttributedText, InlineObject, LineHeight, ParagraphStyle, Rgba,
    TextLayout, TextPaint, TextStyle, TextSystem, label, paint_text,
};

const P1: &str = "مدعی مقدمہ نے تھانہ صدر میں درخواست دی کہ مورخہ بارہ مارچ کی رات نامعلوم ملزمان اس کے گھر میں داخل ہوئے اور قیمتی سامان چوری کر کے فرار ہو گئے۔ تفتیشی افسر نے موقع پر پہنچ کر گواہان کے بیانات قلمبند کیے اور برآمدگی کی کارروائی شروع کی۔";
const P2: &str = "مقدمہ نمبر 412/2026 بجرم 379 PPC تھانہ صدر";
const P3: &str = "ایف آئی آر نمبر 412/2026 مورخہ 15-03-2026 کو درج ہوئی، ملزم کا ایمی نمبر 356789012345678 اور سم نمبر 0300-1234567 ہے (Section 380 PPC) جبکہ تحریر کنندہ نے ضمنی میں تاریخ گرفتاری درج کی۔";
const P4: [&str; 6] = ["تفتیشی افسر ", "تحر", "یر", " کنندہ ", "ملزمان", " اور گواہان"];
const P5: &str = "یہ سطر درمیان میں ہے";
const P6: &str = "Section 380 PPC کے تحت مقدمہ درج کیا گیا اور ملزم کو گرفتار کر لیا گیا";
const P8: &str = "The complainant (مدعی) filed FIR 412/2026 at تھانہ صدر on 15-03-2026 under Section 380 PPC.";
const P9: &str = "ملزم کا نام: Muhammad Aslam ولد Akram، عمر 35 سال";
const P10: &str = "Draft – not yet registered with تھانہ صدر";

const WIDTH: f32 = 520.0;
const RED: Rgba = Rgba::rgb(0xcc0000);
const INK: Rgba = Rgba::rgb(0x111111);

fn range_of(hay: &str, needle: &str) -> Range<usize> {
    let start = hay.find(needle).expect("needle");
    start..start + needle.len()
}

struct Sample {
    text: AttributedText,
    paragraph: ParagraphStyle,
    html: String,
    chip: Option<AttributedText>,
}

fn doc(text: &str) -> AttributedText {
    AttributedText::new(text, TextStyle::document(24.0).with_color(INK))
}

fn nastaleeq_paragraph(align: Align) -> ParagraphStyle {
    ParagraphStyle::rtl()
        .with_align(align)
        .with_line_height(LineHeight::Relative(2.4))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

fn samples(with_chip: bool) -> Vec<Sample> {
    let p4: String = P4.concat();
    let mut starts = vec![0usize];
    for s in P4 {
        starts.push(starts.last().copied().unwrap_or(0) + s.len());
    }
    let mut p4_text = doc(&p4)
        .with(starts[2]..starts[3], Attr::Weight(700))
        .with(starts[2]..starts[3], Attr::Color(RED))
        .with(starts[4]..starts[5], Attr::Weight(700))
        .with(starts[4]..starts[5], Attr::Color(RED))
        .with(range_of(&p4, "گواہان"), Attr::Underline(true));
    let chip_label = AttributedText::new("نام ملزم", TextStyle::document(18.0).with_color(INK));
    if with_chip {
        p4_text = p4_text.with_object(InlineObject {
            id: 1,
            index: starts[5],
            width: 74.0,
            height: 34.0,
            baseline: Some(26.0),
        });
    }
    let p4_html = format!(
        "{}{}<b>{}</b>{}<b>{}</b>{}",
        P4[0],
        P4[1],
        P4[2],
        P4[3],
        P4[4],
        P4[5].replace("گواہان", "<u>گواہان</u>")
    );
    let p6 = doc(P6)
        .with(range_of(P6, "Section 380 PPC"), Attr::Background(Rgba::rgb(0xfff0a0)))
        .with(range_of(P6, "گرفتار"), Attr::Strikethrough(true));
    let p6_html = P6
        .replace(
            "Section 380 PPC",
            "<span style=\"background:#fff0a0\">Section 380 PPC</span>",
        )
        .replace("گرفتار", "<s>گرفتار</s>");
    let ui = |text: &str, size: f32| {
        AttributedText::new(text, TextStyle::interface(size).with_color(INK))
    };
    let p8 = ui(P8, 16.0)
        .with(range_of(P8, "FIR 412/2026"), Attr::Weight(600))
        .with(range_of(P8, "تھانہ صدر"), Attr::Color(RED));
    let p8_html = esc(P8)
        .replace("FIR 412/2026", "<span style=\"font-weight:600\">FIR 412/2026</span>")
        .replace("تھانہ صدر", "<span style=\"color:#c00\">تھانہ صدر</span>");
    let p9 = ui(P9, 14.0).with(range_of(P9, "Muhammad Aslam"), Attr::Weight(700));
    let p9_html = P9.replace("Muhammad Aslam", "<b>Muhammad Aslam</b>");
    let p10 = ui(P10, 15.0).with(0..P10.len(), Attr::Italic(true)).with(
        range_of(P10, "تھانہ صدر"),
        Attr::Weight(700),
    );
    let p10_html = format!(
        "<i>{}</i>",
        P10.replace("تھانہ صدر", "<b>تھانہ صدر</b>")
    );
    let normal = |dir: missal_text::Direction| ParagraphStyle {
        direction: dir,
        ..ParagraphStyle::default()
    };
    vec![
        Sample { text: doc(P1), paragraph: nastaleeq_paragraph(Align::Start), html: P1.into(), chip: None },
        Sample { text: doc(P2), paragraph: nastaleeq_paragraph(Align::Start), html: P2.into(), chip: None },
        Sample { text: doc(P3), paragraph: nastaleeq_paragraph(Align::Start), html: P3.into(), chip: None },
        Sample { text: p4_text, paragraph: nastaleeq_paragraph(Align::Start), html: p4_html, chip: with_chip.then_some(chip_label) },
        Sample { text: doc(P5), paragraph: nastaleeq_paragraph(Align::Center), html: P5.into(), chip: None },
        Sample { text: p6, paragraph: nastaleeq_paragraph(Align::Start), html: p6_html, chip: None },
        Sample { text: doc(P1), paragraph: nastaleeq_paragraph(Align::Justify), html: P1.into(), chip: None },
        Sample { text: p8, paragraph: normal(missal_text::Direction::Ltr), html: p8_html, chip: None },
        Sample { text: p9, paragraph: normal(missal_text::Direction::Rtl), html: p9_html, chip: None },
        Sample { text: p10, paragraph: normal(missal_text::Direction::Ltr), html: p10_html, chip: None },
    ]
}

fn reference_html() -> String {
    let fonts = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let fonts = fonts.canonicalize().expect("fonts dir");
    let face = |family: &str, file: &str, weight: u16, style: &str| {
        format!(
            "@font-face{{font-family:{family};src:url(\"file://{}/{file}\");font-weight:{weight};font-style:{style}}}\n",
            fonts.display()
        )
    };
    let mut css = String::new();
    css += &face("XJNN", "JameelNooriNastaleeq.ttf", 400, "normal");
    for (file, weight, style) in [
        ("Inter-Regular.ttf", 400, "normal"),
        ("Inter-Medium.ttf", 500, "normal"),
        ("Inter-SemiBold.ttf", 600, "normal"),
        ("Inter-Bold.ttf", 700, "normal"),
        ("Inter-Italic.ttf", 400, "italic"),
        ("Inter-BoldItalic.ttf", 700, "italic"),
    ] {
        css += &face("XInter", file, weight, style);
    }
    let mut body = String::new();
    for (i, s) in samples(false).iter().enumerate() {
        let align = match s.paragraph.align {
            Align::Center => "center",
            Align::Justify => "justify",
            _ => "start",
        };
        let (dir, cls) = match (i, s.paragraph.direction) {
            (0..=6, _) => ("rtl", "doc"),
            (_, missal_text::Direction::Ltr) => ("ltr", "ui"),
            _ => ("rtl", "ui"),
        };
        let size = match i {
            7 => 16,
            8 => 14,
            9 => 15,
            _ => 24,
        };
        body += &format!(
            "<p dir=\"{dir}\" class=\"{cls}\" style=\"text-align:{align};font-size:{size}px\">{}</p>\n",
            s.html
        );
    }
    format!(
        "<!doctype html><meta charset=\"utf-8\"><style>{css}
body{{margin:0;background:#fff;color:#111}}
.page{{width:{WIDTH}px;padding:16px 20px}}
p{{margin:0 0 8px;outline:1px solid #ccc}}
.doc{{font-family:XJNN;line-height:2.4}}
.ui{{font-family:XInter,XJNN;line-height:normal}}
b{{color:#c00}}
.ui b{{color:inherit}}
</style><div class=\"page\">{body}</div><script>document.fonts.ready.then(()=>document.title='ready')</script>"
    )
}

struct Para {
    layout: Arc<TextLayout>,
    chip: Option<Arc<TextLayout>>,
}

struct ComparePage {
    paras: Vec<Para>,
}

impl ComparePage {
    fn new(cx: &mut gpui_kit::App, with_chip: bool) -> Self {
        let paras = samples(with_chip)
            .into_iter()
            .map(|s| Para {
                layout: TextSystem::layout(cx, &s.text, &s.paragraph, Some(WIDTH)),
                chip: s
                    .chip
                    .map(|c| TextSystem::layout(cx, &c, &ParagraphStyle::rtl(), None)),
            })
            .collect();
        Self { paras }
    }
}

fn paragraph_view(para: &Para) -> impl IntoElement {
    let layout = para.layout.clone();
    let chip = para.chip.clone();
    div()
        .w(px(WIDTH))
        .h(px(layout.height()))
        .mb(px(8.))
        .border_1()
        .border_color(rgb(0xcccccc))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| {
                    let origin = bounds.origin;
                    for object in layout.objects() {
                        let b = Bounds::new(
                            point(origin.x + px(object.rect.x), origin.y + px(object.rect.y)),
                            size(px(object.rect.width), px(object.rect.height)),
                        );
                        window.paint_quad(gpui_kit::quad(
                            b,
                            px(8.),
                            hsla(0.12, 0.9, 0.85, 1.0),
                            px(1.),
                            hsla(0.12, 0.8, 0.4, 1.0),
                            Default::default(),
                        ));
                        if let Some(chip) = &chip {
                            let o = point(
                                b.origin.x + (b.size.width - px(chip.width())) / 2.0,
                                b.origin.y + (b.size.height - px(chip.height())) / 2.0,
                            );
                            paint_text(chip, o, &TextPaint::default(), window, cx);
                        }
                    }
                    paint_text(&layout, origin, &TextPaint::default(), window, cx);
                },
            )
            .size_full(),
        )
}

impl Render for ComparePage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("compare")
            .size_full()
            .bg(rgb(0xffffff))
            .px(px(20.))
            .py(px(16.))
            .children(self.paras.iter().map(paragraph_view))
    }
}

struct LabelPage;

impl Render for LabelPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111111))
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(label("ملزم کا نام: Muhammad Aslam").text_size(px(14.)))
            .child(label("Section 380 PPC کے تحت").text_size(px(14.)).font_weight(gpui_kit::FontWeight::BOLD))
            .child(
                div()
                    .w(px(160.))
                    .border_1()
                    .border_color(rgb(0xcccccc))
                    .child(label(P3).text_size(px(14.)).truncate()),
            )
            .child(
                label(P1)
                    .document_font()
                    .text_size(px(20.))
                    .w(px(400.))
                    .line_clamp(2)
                    .text_ellipsis(),
            )
    }
}


const NAMES: [&str; 8] = ["محمد اسلم", "Muhammad Aslam", "غلام رسول ولد اللہ دتہ", "شازیہ بی بی", "Rana Tariq", "عبدالستار خان", "نامعلوم ملزمان", "Zahid Hussain عرف زاہدی"];
const OFFENCES: [&str; 5] = ["379 PPC چوری", "380 PPC گھر میں چوری", "302 PPC قتل", "Section 489-F چیک", "تفتیش جاری ہے"];

pub(crate) fn row_cells(i: usize) -> [String; 4] {
    [
        format!("{}/2026", 400 + i),
        NAMES[i % NAMES.len()].to_string(),
        OFFENCES[i % OFFENCES.len()].to_string(),
        format!("{:02}-03-2026", 1 + i % 28),
    ]
}

struct WidgetsPage {
    inputs: Vec<(&'static str, Entity<missal_text::TextInput>)>,
    focused: Entity<missal_text::TextInput>,
}

impl WidgetsPage {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        use missal_text::TextInput;
        let mk = |cx: &mut Context<Self>, window: &mut Window, f: &dyn Fn(TextInput) -> TextInput| {
            cx.new(|cx| f(TextInput::new(window, cx)))
        };
        let focused = mk(cx, window, &|i| i.document_font().text_size(20.0).default_text("ملزم محمد اسلم ولد اکرم"));
        let selected = mk(cx, window, &|i| i.document_font().text_size(20.0).default_text("مقدمہ نمبر 412/2026 بجرم 379 PPC"));
        let composing = mk(cx, window, &|i| i.document_font().text_size(20.0).default_text("تھانہ "));
        let placeholder = mk(cx, window, &|i| i.document_font().text_size(20.0).placeholder("مدعی کا نام لکھیں"));
        let ui_placeholder = mk(cx, window, &|i| i.placeholder("Search FIRs / ایف آئی آر تلاش کریں"));
        let disabled = mk(cx, window, &|i| i.document_font().text_size(20.0).default_text("تھانہ صدر، ضلع لاہور").disabled(true));
        let read_only = mk(cx, window, &|i| i.document_font().text_size(20.0).default_text("یہ خانہ صرف پڑھنے کے لیے ہے").read_only(true));
        let long = mk(cx, window, &|i| i.document_font().text_size(20.0).default_text(P3));
        let area = mk(cx, window, &|i| i.document_font().text_size(20.0).multiline(3, 6).default_text(P1));
        selected.update(cx, |i, cx| {
            let t = i.text().to_string();
            let s = t.find("412").unwrap_or(0);
            i.select(s - "نمبر ".len()..s + "412/2026".len(), cx)
        });
        long.update(cx, |i, cx| i.select(0..0, cx));
        composing.update(cx, |i, cx| {
            use gpui_kit::EntityInputHandler;
            i.replace_and_mark_text_in_range(None, "صدر", None, window, cx)
        });
        Self {
            inputs: vec![
                ("Focused, caret at end", focused.clone()),
                ("Selection across RTL/LTR", selected),
                ("IME composition (marked)", composing),
                ("Placeholder (document font)", placeholder),
                ("Placeholder (UI font)", ui_placeholder),
                ("Disabled", disabled),
                ("Read-only", read_only),
                ("Long single line, scrolled to start", long),
                ("Textarea (3-6 rows)", area),
            ],
            focused,
        }
    }
}

impl Render for WidgetsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme;
        let theme = cx.theme().clone();
        let table = gpui_kit::uniform_list("table", 5000, |range, _, _| {
            range
                .map(|i| {
                    let cells = row_cells(i);
                    div()
                        .flex()
                        .flex_row_reverse()
                        .h(px(30.))
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xe5e5e5))
                        .when(i % 2 == 1, |d| d.bg(rgb(0xfafafa)))
                        .child(div().w(px(80.)).px(px(6.)).child(label(cells[0].clone()).text_size(px(13.)).truncate()))
                        .child(div().w(px(150.)).px(px(6.)).child(label(cells[1].clone()).text_size(px(13.)).truncate()))
                        .child(div().w(px(130.)).px(px(6.)).child(label(cells[2].clone()).text_size(px(13.)).truncate()))
                        .child(div().flex_1().px(px(6.)).child(label(cells[3].clone()).text_size(px(13.)).truncate()))
                })
                .collect()
        })
        .h(px(270.));
        let sidebar = ["تمام ایف آئی آر", "نئی ایف آئی آر", "Templates / سانچے", "مقامی ترتیبات", "ضمنی رپورٹس", "Archive"];
        let _ = &self.focused;
        div()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .flex()
            .flex_row_reverse()
            .child(
                div()
                    .w(px(190.))
                    .h_full()
                    .bg(theme.sidebar)
                    .border_l_1()
                    .border_color(theme.border)
                    .p(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .children(sidebar.iter().enumerate().map(|(i, s)| {
                        div()
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(6.))
                            .when(i == 1, |d| d.bg(theme.sidebar_accent))
                            .child(label(*s).text_size(px(14.)).when(i == 1, |l| l.font_weight(gpui_kit::FontWeight::SEMIBOLD)))
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(label("ایف آئی آر فارم — Inputs").text_size(px(18.)).font_weight(gpui_kit::FontWeight::SEMIBOLD))
                    .children(self.inputs.iter().map(|(caption, input)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(label(*caption).ltr().text_size(px(11.)).text_color(theme.muted_foreground))
                            .child(input.clone())
                    }))
                    .child(label("جدول — 5000 rows (uniform_list, virtualised)").text_size(px(14.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).mt(px(6.)))
                    .child(div().border_1().border_color(theme.border).rounded(px(6.)).overflow_hidden().child(table)),
            )
    }
}

#[cfg(feature = "headless")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--html") {
        std::fs::write(&args[i + 1], reference_html()).expect("write html");
        return;
    }
    let out_dir = args
        .iter()
        .position(|a| a == "--out")
        .map(|i| args[i + 1].clone())
        .unwrap_or_else(|| ".".into());
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new("sans-serif"));
    let mut cx = gpui_kit::HeadlessAppContext::with_platform(
        text_system,
        Arc::new(gpui_kit::assets::Assets),
        || gpui_kit::platform::current_headless_renderer(),
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        missal_text::init(cx);
        match TextSystem::check_font_mapping(cx) {
            Ok(()) => println!("font mapping: GPUI font ids match the bundled faces"),
            Err(e) => println!("font mapping problems:\n{e}"),
        }
    });
    let shots: Vec<(&str, f32, f32, Box<dyn Fn(&mut gpui_kit::App) -> gpui_kit::AnyView>)> = vec![
        ("compare", 560.0, 1240.0, Box::new(|cx| {
            let page = ComparePage::new(cx, false);
            let e: Entity<ComparePage> = cx.new(|_| page);
            e.into()
        })),
        ("compare_chip", 560.0, 520.0, Box::new(|cx| {
            let page = ComparePage::new(cx, true);
            let e: Entity<ComparePage> = cx.new(|_| page);
            e.into()
        })),
        ("labels", 560.0, 300.0, Box::new(|cx| {
            let e: Entity<LabelPage> = cx.new(|_| LabelPage);
            e.into()
        })),
    ];
    let mut shots = shots;
    shots.truncate(if args.iter().any(|a| a == "--widgets-only") { 0 } else { shots.len() });
    {
        let window = cx
            .open_window(size(px(900.), px(1040.)), |window, cx| {
                let page = cx.new(|cx| WidgetsPage::new(window, cx));
                let focused = page.read(cx).focused.clone();
                focused.update(cx, |i, cx| i.focus(window, cx));
                page
            })
            .expect("window");
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, _| window.refresh()).ok();
        cx.run_until_parked();
        let image = cx.capture_screenshot(window.into()).expect("screenshot");
        let path = format!("{out_dir}/widgets.png");
        image.save(&path).expect("save");
        println!("widgets: {}x{} -> {path}", image.width(), image.height());
    }
    for (name, w, h, build) in shots {
        let window = cx
            .open_window(size(px(w), px(h)), |_, cx| {
                let view = build(cx);
                cx.new(|_| Host { view })
            })
            .expect("window");
        cx.run_until_parked();
        let image = cx.capture_screenshot(window.into()).expect("screenshot");
        let path = format!("{out_dir}/{name}.png");
        image.save(&path).expect("save");
        println!("{name}: {}x{} -> {path}", image.width(), image.height());
    }
}

struct Host {
    view: gpui_kit::AnyView,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.view.clone())
    }
}

#[cfg(not(feature = "headless"))]
fn main() {
    let _ = (reference_html, ComparePage::new, LabelPage, WidgetsPage::new, row_cells);
}
