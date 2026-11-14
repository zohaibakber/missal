use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, Bounds, HeadlessAppContext, WindowBounds, WindowOptions, point, px, size,
};
use missal_app::app::{ShellHandle, navigate};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::{Preferences, ThemeChoice};
use missal_app::seed::seed_firs;
use missal_app::state::{Models, PrintTarget, Route};
use missal_app::ui::window_controls::{ControlsStyle, ForceControls};
use missal_core::{FirId, TemplateId};
use missal_store::Store;

const DEFAULT_OUT: &str =
    "/tmp/claude-0/-home-user-missal/da28de75-7983-5932-bdf9-e2e3d5312f4e/scratchpad/wp1";

struct Harness {
    cx: HeadlessAppContext,
    handle: AnyWindowHandle,
    out: PathBuf,
    suffix: &'static str,
}

impl Harness {
    fn settle(&mut self) {
        for _ in 0..3 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.handle, |_, window, _| window.refresh())
                .ok();
        }
        self.cx.run_until_parked();
    }

    fn shot(&mut self, name: &str) {
        self.settle();
        let image = self.cx.capture_screenshot(self.handle).expect("screenshot");
        let path = self.out.join(format!("{name}-{}.png", self.suffix));
        image.save(&path).expect("save");
        println!("{name}-{}", self.suffix);
    }

    fn with_window<R>(
        &mut self,
        f: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App) -> R,
    ) -> R {
        let result = self
            .cx
            .update_window(self.handle, |_, window, cx| f(window, cx))
            .expect("window");
        self.settle();
        result
    }

    fn go(&mut self, route: Route) {
        self.with_window(|window, cx| navigate(route, window, cx));
    }

    fn click(&mut self, id: impl Into<gpui_kit::ElementId>) {
        let id = id.into();
        self.with_window(move |window, cx| window.click(id, cx));
    }

    fn press(&mut self, keys: &str) {
        self.with_window(|window, cx| window.press(keys, cx));
    }

    fn wait_for_print(&mut self) {
        for _ in 0..60 {
            self.settle();
            let ready = self.cx.update(|cx| {
                ShellHandle::get(cx)
                    .and_then(|shell| shell.read(cx).print_preview())
                    .is_some_and(|preview| {
                        let preview = preview.read(cx);
                        preview.is_ready() || preview.failure().is_some()
                    })
            });
            if ready {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        self.settle();
    }

    fn template_id(&mut self) -> TemplateId {
        self.cx.update(|cx| {
            Models::get(cx)
                .templates
                .read(cx)
                .items
                .first()
                .map(|item| item.id)
                .expect("a template")
        })
    }
}

fn build(dark: bool, out: &Path, count: usize) -> (Harness, tempfile::TempDir) {
    let directory = tempfile::tempdir().expect("tempdir");
    let database = directory.path().join("missal.sqlite");
    seed_firs(&database, count).expect("seed");
    let store = Store::open(&database).expect("open");
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new("sans-serif"));
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    let preferences = Preferences {
        theme: if dark {
            ThemeChoice::Dark
        } else {
            ThemeChoice::Light
        },
        ..Preferences::default()
    };
    let handle = cx.update(|cx| {
        let models = init_app(cx, Db::inline(store), None, preferences);
        cx.set_reduce_motion(true);
        cx.set_global(ForceControls(ControlsStyle::Windows));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(1280.), px(800.)),
            })),
            ..gpui_kit::component::TitleBar::window_options()
        };
        let (handle, _) = open_shell(&models, options, cx).expect("window");
        handle
    });
    let mut harness = Harness {
        cx,
        handle,
        out: out.to_owned(),
        suffix: if dark { "dark" } else { "light" },
    };
    harness.settle();
    (harness, directory)
}

fn run(dark: bool, out: &Path) {
    let (mut h, _directory) = build(dark, out, 90);
    h.go(Route::Firs);
    h.press("down");
    h.press("down");
    h.press("down");
    h.with_window(|window, cx| window.hover(("fir-row", 5usize), cx));
    h.shot("01-fir-list");

    h.click(("row-select-cell", 0usize));
    h.click(("row-select-cell", 1usize));
    h.shot("03-selection");
    h.press("escape");

    h.click("status-Challan Submitted");
    h.shot("04-status-filter");
    h.click("status-all");

    h.click("columns");
    h.shot("05-columns-menu");
    h.press("escape");

    h.with_window(|window, cx| window.right_click(("fir-row", 1usize), cx));
    h.shot("06-row-context-menu");
    h.press("escape");

    h.press("/");
    h.with_window(|window, cx| window.input("ڈکیتی", cx));
    h.click(("row-select-cell", 0usize));
    h.click(("row-select-cell", 1usize));
    h.shot("04b-search-urdu");
    h.press("escape");
    h.press("escape");
    h.press("escape");

    h.press("/");
    h.with_window(|window, cx| window.input("zzzz", cx));
    h.shot("04c-no-match");
    h.press("escape");

    h.go(Route::Print(PrintTarget::Fir(FirId::new(3).expect("id"))));
    h.wait_for_print();
    h.click(("print-include", 2usize));
    h.shot("19-print-fir");
    h.press("pagedown");
    h.press("pagedown");
    h.shot("20-print-page-3");
    h.press("secondary-=");
    h.press("secondary-=");
    h.shot("21-print-zoomed");
    h.press("secondary-0");
    h.click("preview-zoom");
    h.shot("22-print-zoom-menu");
    h.press("escape");
    h.click(("print-include", 2usize));

    let template = h.template_id();
    h.go(Route::Print(PrintTarget::Template(template)));
    h.wait_for_print();
    h.shot("23-print-template");

    h.go(Route::Fir(FirId::new(3).expect("id")));
    h.press("secondary-p");
    h.wait_for_print();
    h.shot("19b-print-from-workspace");
    h.press("escape");
    h.shot("19c-back-to-workspace");

    h.with_window(|window, cx| {
        let catalog = Models::get(cx).catalog.read(cx).field_catalog();
        let request = missal_app::ui::print_preview::PrintRequest {
            title: "FIR 3/2026".to_owned(),
            note: None,
            export_name: "FIR 3-2026".to_owned(),
            catalog: (*catalog).clone(),
            values: None,
            mode: missal_core::DisplayMode::Labels,
            documents: vec![missal_app::ui::print_preview::PreviewDocument::Stored(
                missal_core::FirDocumentId::new(999_999).expect("id"),
            )],
        };
        missal_app::app::open_print_preview(request, window, cx);
    });
    h.wait_for_print();
    h.shot("24-print-error");
    h.press("escape");

    let (mut empty, _empty_directory) = build(dark, out, 0);
    empty.go(Route::Firs);
    empty.shot("02-fir-list-empty");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .position(|argument| argument == "--out")
        .map(|index| PathBuf::from(&args[index + 1]))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    std::fs::create_dir_all(&out).expect("output directory");
    let only = args
        .iter()
        .position(|argument| argument == "--theme")
        .map(|index| args[index + 1].clone());
    for dark in [false, true] {
        if only.as_deref().is_some_and(|only| (only == "dark") != dark) {
            continue;
        }
        run(dark, &out);
    }
}
