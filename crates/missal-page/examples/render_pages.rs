use std::io::Read;
use std::sync::Arc;

use flate2::read::GzDecoder;
use gpui_kit::{
    AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, canvas, div, px, size,
};
use missal_core::Document;
use missal_core::document::Envelope;
use missal_page::paint::{PagePaint, paint_page};
use missal_page::{DocSource, Page, PageEngine, TokenFields};
use serde_json::Value;

const BUNDLE: &[u8] = include_bytes!("../../../templates/bundle.json.gz");

struct PageView {
    page: Page,
    paint: PagePaint,
}

impl Render for PageView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let page = self.page.clone();
        let paint = self.paint;
        div().size_full().bg(gpui_kit::rgb(0xd0d0d0)).child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| paint_page(&page, bounds.origin, &paint, window, cx),
            )
            .size_full(),
        )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .map(|i| args[i + 1].clone())
    };
    let out = opt("--out").unwrap_or_else(|| ".".into());
    let name = opt("--template").unwrap_or_default();
    let pages: Vec<usize> = opt("--pages")
        .map(|p| p.split(',').map(|n| n.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![0]);
    let zoom: f32 = opt("--zoom").map(|z| z.parse().unwrap()).unwrap_or(1.0);
    let editor = args.iter().any(|a| a == "--editor");
    let mut text = String::new();
    GzDecoder::new(BUNDLE).read_to_string(&mut text).unwrap();
    let Value::Array(items) = serde_json::from_str(&text).unwrap() else {
        panic!()
    };
    let item = items
        .iter()
        .find(|i| i["name"].as_str().unwrap().starts_with(name.as_str()))
        .expect("template");
    let document =
        Document::from_envelope(&Envelope::from_value(item["document"].clone()).unwrap()).unwrap();
    let mut engine = PageEngine::new();
    let packet = engine.layout(&[DocSource::new(&document, &TokenFields)]);
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
    for index in pages {
        let Some(page) = packet.page(index).cloned() else {
            continue;
        };
        let paint = if editor {
            PagePaint::editor(zoom)
        } else {
            PagePaint::print(zoom)
        };
        let (w, h) = (page.size.width * zoom, page.size.height * zoom);
        let window = cx
            .open_window(size(px(w), px(h)), move |_, cx| {
                cx.new(|_| PageView { page, paint })
            })
            .expect("window");
        cx.run_until_parked();
        let image = cx.capture_screenshot(window.into()).expect("screenshot");
        let path = format!(
            "{out}/page-{index:02}{}.png",
            if editor { "-editor" } else { "" }
        );
        image.save(&path).expect("save");
        println!("{}x{} -> {path}", image.width(), image.height());
    }
}
