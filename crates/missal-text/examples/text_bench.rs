use std::{borrow::Cow, hint::black_box, sync::Arc, time::Instant};

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb, size,
};
use missal_text::{
    AttributedText, LineHeight, ParagraphStyle, TextEngine, TextStyle, TextSystem, fonts, label,
};

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

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
        })
        .map(|pages| pages * 4)
        .unwrap_or(0)
}

fn fill(engine: &mut TextEngine, text: &str, count: usize) -> f64 {
    let paragraph = ParagraphStyle::rtl().with_line_height(LineHeight::Relative(2.0));
    let before = rss_kib();
    for i in 0..count {
        let unique = format!("{i} {text}");
        black_box(engine.layout(
            &AttributedText::new(unique, TextStyle::document(20.0)),
            &paragraph,
            Some(520.0),
        ));
    }
    rss_kib().saturating_sub(before) as f64 / 1024.0
}

fn memory_costs() {
    let texts = texts();
    let cases = [
        ("300-byte paragraph", &texts[1].1, 4096),
        ("5 KB paragraph", &texts[2].1, 1024),
    ];
    for (name, text, count) in cases {
        let mut engine = TextEngine::new();
        let rss = fill(&mut engine, text, count);
        let stats = engine.cache_stats();
        println!(
            "default cache, {count} distinct {name}s: {} entries, estimate {:.1} MiB, RSS +{rss:.1} MiB",
            stats.entries,
            stats.bytes as f64 / 1048576.0
        );
    }
    let paragraph = ParagraphStyle::rtl().with_line_height(LineHeight::Relative(2.0));
    let mut engine = TextEngine::new();
    let before = rss_kib();
    let resized = AttributedText::new(texts[2].1.as_str(), TextStyle::document(20.0));
    for width in 300..600 {
        black_box(engine.layout(&resized, &paragraph, Some(width as f32)));
    }
    println!(
        "default cache, one 5 KB paragraph at 300 widths (window resize): {} entries, estimate {:.1} MiB, RSS +{:.1} MiB",
        engine.cache_stats().entries,
        engine.cache_stats().bytes as f64 / 1048576.0,
        rss_kib().saturating_sub(before) as f64 / 1024.0
    );
    for (name, text, count) in cases {
        let mut engine = TextEngine::new();
        engine.set_cache_budget(usize::MAX);
        let rss = fill(&mut engine, text, count);
        let stats = engine.cache_stats();
        println!(
            "unbounded cache, {count} distinct {name}s: {:.1} KiB each, estimate {:.1} KiB each",
            rss * 1024.0 / count as f64,
            stats.bytes as f64 / 1024.0 / count as f64
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

struct Io {
    read_bytes: u64,
    major_faults: u64,
}

fn io() -> Io {
    let read_bytes = std::fs::read_to_string("/proc/self/io")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("read_bytes: ").map(str::to_owned))
        })
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    let major_faults = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            s.rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().nth(9).map(str::to_owned))
        })
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    Io {
        read_bytes,
        major_faults,
    }
}

fn step<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let before = io();
    let start = Instant::now();
    let out = f();
    let elapsed = start.elapsed().as_secs_f64() * 1e3;
    let after = io();
    println!(
        "  {name:<44} {elapsed:>8.2} ms {:>8} KiB read {:>6} major faults",
        (after.read_bytes - before.read_bytes) / 1024,
        after.major_faults - before.major_faults
    );
    out
}

fn headless() -> gpui_kit::HeadlessAppContext {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        "sans-serif",
    ));
    gpui_kit::HeadlessAppContext::with_platform(
        text_system,
        Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    )
}

fn startup_costs() {
    println!("startup (run first so bundled font pages start cold):");
    let faces = fonts::bundled_faces();
    let mut cx = headless();
    cx.update(|cx| {
        step("gpui_kit::init", || gpui_kit::init(cx));
        step("GPUI add_fonts (all bundled faces)", || {
            cx.text_system()
                .add_fonts(faces.iter().map(|f| Cow::Borrowed(f.data)).collect())
                .ok()
        });
        step("GPUI resolve_font (all bundled faces)", || {
            for face in faces {
                black_box(cx.text_system().resolve_font(&gpui_kit::Font {
                    family: face.family.into(),
                    features: Default::default(),
                    fallbacks: None,
                    weight: gpui_kit::FontWeight(face.weight as f32),
                    style: if face.italic {
                        gpui_kit::FontStyle::Italic
                    } else {
                        gpui_kit::FontStyle::Normal
                    },
                }));
            }
        });
    });
    let mut cx = headless();
    cx.update(|cx| {
        gpui_kit::init(cx);
        step("missal_text::init", || missal_text::init(cx));
        step("first Urdu layout (shaper warm-up)", || {
            black_box(TextSystem::layout(
                cx,
                &AttributedText::new(SHORT, TextStyle::document(14.0)),
                &ParagraphStyle::rtl(),
                None,
            ))
        });
        step("first Latin layout", || {
            black_box(TextSystem::layout(
                cx,
                &AttributedText::new("FIR 412/2026", TextStyle::interface(14.0)),
                &ParagraphStyle::ltr(),
                None,
            ))
        });
        step("second TextEngine::new (as missal-page does)", || {
            black_box(TextEngine::new())
        });
        step("first Urdu font_id (GPUI registration)", || {
            black_box(TextSystem::global(cx).font_id(missal_text::FaceId(0)))
        });
        step("TextSystem::check_font_mapping", || {
            black_box(TextSystem::check_font_mapping(cx)).ok()
        });
    });
}

fn main() {
    startup_costs();
    layout_costs();
    memory_costs();
    frame_costs();
}
