use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, Bounds, HeadlessAppContext, Size, WindowBounds, WindowOptions, point, px, size,
};
use missal_app::app::{navigate, show_shortcuts};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::{Preferences, ThemeChoice};
use missal_app::screens::edit_sheet::open_edit_sheet;
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_core::{FirId, TemplateId};
use missal_store::Store;

const DEFAULT_OUT: &str = "/tmp/claude-0/-home-user-missal/da28de75-7983-5932-bdf9-e2e3d5312f4e/scratchpad/app-shots";

struct Harness {
    cx: HeadlessAppContext,
    handle: AnyWindowHandle,
    models: Models,
    out: PathBuf,
    suffix: &'static str,
}

impl Harness {
    fn settle(&mut self) {
        for _ in 0..3 {
            self.cx.run_until_parked();
            self.cx.update_window(self.handle, |_, window, _| window.refresh()).ok();
        }
        self.cx.run_until_parked();
    }

    fn shot(&mut self, name: &str) {
        self.settle();
        let image = self.cx.capture_screenshot(self.handle).expect("screenshot");
        let path = self.out.join(format!("{name}-{}.png", self.suffix));
        image.save(&path).expect("save");
        println!("{name}-{}: {}x{}", self.suffix, image.width(), image.height());
    }

    fn with_window<R>(&mut self, f: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App) -> R) -> R {
        let result = self.cx.update_window(self.handle, |_, window, cx| f(window, cx)).expect("window");
        self.settle();
        result
    }

    fn go(&mut self, route: Route) {
        self.with_window(|window, cx| {
            window.close_all_dialogs(cx);
            window.close_sheet(cx);
            navigate(route, window, cx)
        });
    }

    fn click(&mut self, id: &'static str) {
        self.with_window(|window, cx| window.click(id, cx));
    }

    fn press(&mut self, keys: &str) {
        self.with_window(|window, cx| window.press(keys, cx));
    }

    fn reset(&mut self) {
        self.with_window(|window, cx| {
            window.close_all_dialogs(cx);
            window.close_sheet(cx);
        });
    }
}

fn build(dark: bool, out: &Path, width: f32, height: f32) -> (Harness, tempfile::TempDir) {
    let directory = tempfile::tempdir().expect("tempdir");
    let database = directory.path().join("missal.sqlite");
    seed_firs(&database, 90).expect("seed");
    let mut store = Store::open(&database).expect("open");
    let customs: Vec<_> = store.placeholders().expect("placeholders").into_iter().filter(|p| p.source == missal_core::PlaceholderSource::Custom).collect();
    let fir = FirId::new(3).expect("id");
    store
        .set_placeholder_values(fir, &[(customs[0].id, "نیشنل بینک آف پاکستان".to_owned()), (customs[3].id, "123456".to_owned())])
        .expect("values");
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new("sans-serif"));
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    let preferences = Preferences {
        theme: if dark { ThemeChoice::Dark } else { ThemeChoice::Light },
        ..Preferences::default()
    };
    let (handle, models) = cx.update(|cx| {
        let models = init_app(cx, Db::inline(store), None, preferences);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(width), px(height)),
            })),
            ..gpui_kit::component::TitleBar::window_options()
        };
        let (handle, _) = open_shell(&models, options, cx).expect("window");
        (handle, models)
    });
    let suffix = if dark { "dark" } else { "light" };
    let mut harness = Harness { cx, handle, models, out: out.to_owned(), suffix };
    harness.settle();
    (harness, directory)
}

fn run(dark: bool, out: &Path) {
    let (mut h, _directory) = build(dark, out, 1280.0, 800.0);
    h.shot("01-fir-list");

    h.with_window(|window, cx| {
        let nav_models = Models::get(cx);
        nav_models.prefs.update(cx, |prefs, cx| prefs.update(cx, |p| p.sidebar_open = false));
        let _ = window;
    });
    h.shot("02-sidebar-collapsed");
    h.with_window(|_, cx| {
        let models = Models::get(cx);
        models.prefs.update(cx, |prefs, cx| prefs.update(cx, |p| p.sidebar_open = true));
    });

    h.click("select-all");
    h.shot("03-selection");
    h.click("select-all");
    h.click("status-filter");
    h.shot("04-status-filter");
    h.press("escape");
    h.click("columns");
    h.shot("05-columns-menu");
    h.press("escape");

    h.with_window(|window, cx| {
        window.right_click(("fir-row", 1usize), cx);
    });
    h.shot("06-row-context-menu");
    h.press("escape");

    h.go(Route::NewFir);
    h.shot("07-new-fir");
    h.click("create-fir");
    h.shot("08-new-fir-validation");
    h.reset();

    h.with_window(|window, cx| {
        let record = Models::get(cx);
        let _ = record;
        let _ = window;
    });
    let record = {
        let id = FirId::new(3).expect("id");
        h.cx.update(|cx| {
            let store_db = missal_app::state::db(cx);
            let reply = store_db.run(move |store| store.fir(id));
            let waker = std::task::Waker::noop();
            let mut context = std::task::Context::from_waker(waker);
            let mut reply = Box::pin(reply);
            match std::future::Future::poll(reply.as_mut(), &mut context) {
                std::task::Poll::Ready(result) => result.expect("fir"),
                std::task::Poll::Pending => panic!("inline database replies immediately"),
            }
        })
    };
    h.go(Route::Firs);
    h.with_window(|window, cx| open_edit_sheet(record, window, cx));
    h.shot("09-edit-sheet");
    h.reset();

    h.go(Route::Fir(FirId::new(3).expect("id")));
    h.shot("10-workspace");
    h.click("add-templates-trigger");
    h.shot("11-add-templates");
    h.press("escape");
    h.click("edit-custom-values");
    h.shot("12-custom-values");
    h.reset();

    h.go(Route::Templates);
    h.shot("13-templates-empty");
    let template = h.models_template_id();
    h.go(Route::Template(template));
    h.shot("14-template-page");

    h.go(Route::Settings);
    h.shot("15-settings");

    h.with_window(|window, cx| show_shortcuts(window, cx));
    h.shot("16-shortcuts");
    h.reset();

    h.go(Route::Firs);
    h.press("secondary-k");
    h.shot("17-command-palette");
    h.press("escape");
    h.shot("18-fir-list-after");
}

impl Harness {
    fn models_template_id(&mut self) -> TemplateId {
        self.cx.update(|cx| {
            let models = Models::get(cx);
            models.templates.read(cx).items.first().map(|item| item.id).expect("a template")
        })
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .position(|argument| argument == "--out")
        .map(|index| PathBuf::from(&args[index + 1]))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    std::fs::create_dir_all(&out).expect("output directory");
    let only = args.iter().position(|argument| argument == "--theme").map(|index| args[index + 1].clone());
    for dark in [false, true] {
        if only.as_deref().is_some_and(|only| (only == "dark") != dark) {
            continue;
        }
        run(dark, &out);
    }
    let _ = Size::<gpui_kit::Pixels>::default;
}
