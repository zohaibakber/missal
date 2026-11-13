use std::{hint::black_box, sync::Arc, time::Instant};

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb, size,
};
use missal_text::{AttributedText, LineHeight, ParagraphStyle, TextEngine, TextStyle, label};

const SHORT: &str = "ملزم محمد اسلم";
const P1: &str = "مدعی مقدمہ نے تھانہ صدر میں درخواست دی کہ مورخہ بارہ مارچ کی رات نامعلوم ملزمان اس کے گھر میں داخل ہوئے اور قیمتی سامان چوری کر کے فرار ہو گئے۔ تفتیشی افسر نے موقع پر پہنچ کر گواہان کے بیانات قلمبند کیے اور برآمدگی کی کارروائی شروع کی۔";
const P3: &str = "ایف آئی آر نمبر 412/2026 مورخہ 15-03-2026 کو درج ہوئی، ملزم کا ایمی نمبر 356789012345678 اور سم نمبر 0300-1234567 ہے (Section 380 PPC) جبکہ تحریر کنندہ نے ضمنی میں تاریخ گرفتاری درج کی۔";

fn per_iter(n: u32, mut f: impl FnMut(u32)) -> f64 {
    let start = Instant::now();
    for i in 0..n {
        f(i);
    }
    start.elapsed().as_secs_f64() * 1e6 / n as f64
}

fn texts() -> Vec<(&'static str, String)> {
    let mut five = String::new();
    while five.len() < 5000 {
        five.push_str(P1);
        five.push(' ');
        five.push_str(P3);
        five.push(' ');
    }
    let mut three = String::new();
    for word in P1.split(' ') {
        if three.len() + word.len() + 1 > 300 {
            break;
        }
        if !three.is_empty() {
            three.push(' ');
        }
        three.push_str(word);
    }
    vec![
        ("short label", SHORT.to_string()),
        ("300-byte paragraph", three),
        ("5 KB paragraph", five),
    ]
}

fn layout_costs() {
    let paragraph = ParagraphStyle::rtl().with_line_height(LineHeight::Relative(2.0));
    let start = Instant::now();
    let mut engine = TextEngine::new();
    println!(
        "TextEngine::new (font registration): {:.1} ms",
        start.elapsed().as_secs_f64() * 1e3
    );
    let first = Instant::now();
    black_box(engine.layout_uncached(
        &AttributedText::new(SHORT, TextStyle::document(20.0)),
        &paragraph,
        None,
    ));
    println!(
        "first layout ever (font parsing, shaper warm-up): {:.2} ms",
        first.elapsed().as_secs_f64() * 1e3
    );
    println!(
        "{:<20} {:>6} {:>12} {:>12} {:>12} {:>8}",
        "text", "bytes", "cold us", "cached us", "rebreak us", "lines"
    );
    for (name, text) in texts() {
        let attributed = AttributedText::new(text.as_str(), TextStyle::document(20.0));
        let n = if text.len() > 1000 { 200 } else { 2000 };
        let cold = per_iter(n, |_| {
            black_box(engine.layout_uncached(&attributed, &paragraph, Some(520.0)));
        });
        engine.clear_cache();
        let layout = engine.layout(&attributed, &paragraph, Some(520.0));
        let cached = per_iter(n * 10, |_| {
            black_box(engine.layout(&attributed, &paragraph, Some(520.0)));
        });
        let rebreak = per_iter(n, |i| {
            black_box(layout.with_max_width(Some(400.0 + (i % 100) as f32)));
        });
        println!(
            "{name:<20} {:>6} {cold:>12.1} {cached:>12.2} {rebreak:>12.1} {:>8}",
            text.len(),
            layout.lines().len()
        );
    }
}

struct Screen {
    builtin: bool,
}

impl Render for Screen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let builtin = self.builtin;
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .flex()
            .flex_col()
            .children((0..60).map(move |i| {
                let text = format!("ملزم نمبر {i}: محمد اسلم ولد اکرم، تھانہ صدر");
                let row = div().h(px(22.)).w_full();
                if builtin {
                    row.child(div().text_size(px(14.)).child(text))
                } else {
                    row.child(label(text).rtl().text_size(px(14.)))
                }
            }))
    }
}

fn frame_costs() {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new("sans-serif"));
    let mut cx = gpui_kit::HeadlessAppContext::with_platform(
        text_system,
        Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        missal_text::init(cx);
    });
    for builtin in [false, true] {
        let window = cx
            .open_window(size(px(500.), px(1400.)), move |_, cx| {
                cx.new(|_| Screen { builtin })
            })
            .expect("window");
        cx.run_until_parked();
        let any: gpui_kit::AnyWindowHandle = window.into();
        let first = Instant::now();
        cx.update_window(any, |_, window, cx| window.render_frame(cx))
            .ok();
        let first = first.elapsed().as_secs_f64() * 1e3;
        let n = 300;
        let start = Instant::now();
        for _ in 0..n {
            cx.update_window(any, |_, window, cx| window.render_frame(cx))
                .ok();
        }
        let per = start.elapsed().as_secs_f64() * 1e3 / n as f64;
        println!(
            "60 Urdu labels, {}: first frame {first:.2} ms, steady frame (layout+prepaint+paint, CPU) {per:.3} ms",
            if builtin {
                "GPUI built-in text (wrong RTL, reference only)"
            } else {
                "missal-text Label"
            }
        );
    }
}

fn main() {
    layout_costs();
    frame_costs();
}
