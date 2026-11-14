use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, HeadlessAppContext, WindowBounds, WindowOptions,
    point, px, size,
};
use missal_core::document::TextFormat;
use missal_core::{Document, FieldCatalog, FieldMarkers};
use missal_editor::model::selection::{document_end, document_start, pos};
use missal_editor::model::tree;
use missal_editor::{Editor, EditorFields, Selection};

fn largest() -> (String, Document) {
    missal_store::TemplateBundle::embedded()
        .templates()
        .unwrap()
        .into_iter()
        .max_by_key(|(_, d)| d.encode().len())
        .unwrap()
}

fn draw(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .unwrap();
}

fn report(name: &str, mut samples: Vec<(Duration, Duration)>) {
    samples.sort_by_key(|s| s.0);
    let n = samples.len();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    let edit: Vec<f64> = {
        let mut v: Vec<f64> = samples.iter().map(|s| ms(s.1)).collect();
        v.sort_by(f64::total_cmp);
        v
    };
    println!(
        "{name:<34} n={n:<3} total p50={:>6.2}ms p95={:>6.2}ms max={:>6.2}ms | edit+relayout p50={:>6.2}ms",
        ms(samples[n / 2].0),
        ms(samples[(n * 95 / 100).min(n - 1)].0),
        ms(samples[n - 1].0),
        edit[n / 2],
    );
}

fn main() {
    let (name, document) = largest();
    println!(
        "largest template: {name} ({} top-level blocks, {} KB encoded)",
        document.blocks.len(),
        document.encode().len() / 1024
    );
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new("sans-serif"));
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    let (handle, editor): (AnyWindowHandle, Entity<Editor>) = cx.update(|cx| {
        gpui_kit::init(cx);
        missal_text::init(cx);
        missal_editor::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(1280.), px(860.)),
            })),
            ..WindowOptions::default()
        };
        let mut editor = None;
        let handle = cx
            .open_window(options, |window, cx| {
                let e = cx.new(|cx| Editor::new(window, cx));
                editor = Some(e.clone());
                e
            })
            .unwrap();
        (handle.into(), editor.unwrap())
    });
    draw(&mut cx, handle);
    let fields = EditorFields::labels(
        std::rc::Rc::new(FieldCatalog::default()),
        FieldMarkers::default(),
    );
    let started = Instant::now();
    cx.update(|cx| {
        editor.update(cx, |e, cx| {
            e.load("perf".into(), document.clone(), fields.clone(), false, cx)
        })
    });
    let loaded = started.elapsed();
    draw(&mut cx, handle);
    let first_paint = started.elapsed();
    let pages = cx.update(|cx| editor.read(cx).packet().page_count());
    println!(
        "open: load+layout {:.1}ms, to first paint {:.1}ms ({pages} pages)",
        loaded.as_secs_f64() * 1e3,
        first_paint.as_secs_f64() * 1e3
    );

    let paths = cx.update(|cx| tree::all_text_paths(editor.read(cx).document()));
    let outside: Vec<_> = cx.update(|cx| {
        paths
            .iter()
            .filter(|p| tree::cell_of(editor.read(cx).document(), p).is_none())
            .cloned()
            .collect()
    });
    let cell = cx.update(|cx| {
        paths
            .iter()
            .find(|p| tree::cell_of(editor.read(cx).document(), p).is_some())
            .cloned()
    });
    let place = |cx: &mut HeadlessAppContext, path: missal_page::BlockPath, end: bool| {
        cx.update_window(handle, |_, window, cx| {
            editor.update(cx, |e, cx| {
                let offset = if end {
                    e.core().block_len(&path).unwrap_or(0)
                } else {
                    0
                };
                e.set_selection(Selection::caret(pos(path.clone(), offset)), window, cx);
            })
        })
        .unwrap();
        draw(cx, handle);
    };
    let keystrokes = |cx: &mut HeadlessAppContext, label: &str| {
        let mut samples = Vec::new();
        for i in 0..40 {
            let text = if i % 5 == 4 { " " } else { "ک" };
            let t = Instant::now();
            cx.update_window(handle, |_, window, cx| {
                editor.update(cx, |e, cx| e.insert_text_command(text, window, cx))
            })
            .unwrap();
            let edit = t.elapsed();
            draw(cx, handle);
            samples.push((t.elapsed(), edit));
        }
        report(label, samples);
    };
    place(&mut cx, outside[0].clone(), true);
    keystrokes(&mut cx, "keystroke at start");
    place(&mut cx, outside[outside.len() / 2].clone(), true);
    keystrokes(&mut cx, "keystroke in the middle");
    place(&mut cx, outside[outside.len() - 1].clone(), true);
    keystrokes(&mut cx, "keystroke at end");
    if let Some(cell) = cell {
        place(&mut cx, cell, true);
        keystrokes(&mut cx, "keystroke inside a table cell");
    }
    let mut samples = Vec::new();
    for i in 0..20 {
        place(&mut cx, outside[outside.len() / 3 + i].clone(), false);
        cx.update_window(handle, |_, window, cx| {
            editor.update(cx, |e, cx| {
                let head = e.selection().head.clone();
                let mid = e.core().block_len(&head.path).unwrap_or(0) / 2;
                let mid = e
                    .core()
                    .block_text(&head.path)
                    .map(|t| {
                        let mut m = mid;
                        while !t.is_char_boundary(m) {
                            m -= 1;
                        }
                        m
                    })
                    .unwrap_or(0);
                e.set_selection(Selection::caret(pos(head.path, mid)), window, cx);
            })
        })
        .unwrap();
        let t = Instant::now();
        cx.update_window(handle, |_, window, cx| {
            editor.update(cx, |e, cx| e.run(window, cx, |core| core.split()))
        })
        .unwrap();
        let edit = t.elapsed();
        draw(&mut cx, handle);
        samples.push((t.elapsed(), edit));
    }
    report("enter splitting a paragraph", samples);
    let paste: String = (0..60)
        .map(|i| format!("سطر {i}: مدعی مقدمہ نے تھانہ صدر میں درخواست دی کہ\n"))
        .collect::<String>();
    let paste = &paste[..paste
        .char_indices()
        .find(|(i, _)| *i >= 5 * 1024)
        .map(|(i, _)| i)
        .unwrap_or(0)];
    let mut samples = Vec::new();
    for i in 0..5 {
        place(&mut cx, outside[outside.len() / 2 + i].clone(), true);
        let t = Instant::now();
        cx.update_window(handle, |_, window, cx| {
            editor.update(cx, |e, cx| e.run(window, cx, |core| core.paste_text(paste)))
        })
        .unwrap();
        let edit = t.elapsed();
        draw(&mut cx, handle);
        samples.push((t.elapsed(), edit));
    }
    report(&format!("paste {} bytes of text", paste.len()), samples);
    let mut samples = Vec::new();
    for _ in 0..3 {
        let t = Instant::now();
        cx.update_window(handle, |_, window, cx| {
            editor.update(cx, |e, cx| {
                let doc = e.document();
                let selection =
                    Selection::new(document_start(doc).unwrap(), document_end(doc).unwrap());
                e.set_selection(selection, window, cx);
                e.toggle_format(TextFormat::BOLD, window, cx);
            })
        })
        .unwrap();
        let edit = t.elapsed();
        draw(&mut cx, handle);
        samples.push((t.elapsed(), edit));
    }
    report("select all then bold", samples);
}
