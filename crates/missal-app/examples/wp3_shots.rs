use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, Bounds, HeadlessAppContext, WindowBounds, WindowOptions, point, px, size,
};
use missal_app::app::{ShellHandle, navigate};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::{Preferences, ThemeChoice};
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_app::ui::window_controls::{ControlsStyle, ForceControls};
use missal_store::Store;

const DEFAULT_OUT: &str =
    "/tmp/claude-0/-home-user-missal/da28de75-7983-5932-bdf9-e2e3d5312f4e/scratchpad/wp3";

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
        println!(
            "{name}-{}: {}x{}",
            self.suffix,
            image.width(),
            image.height()
        );
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
        self.with_window(|window, cx| {
            window.close_all_dialogs(cx);
            navigate(route, window, cx)
        });
    }

    fn click(&mut self, id: &'static str) {
        self.with_window(|window, cx| window.click(id, cx));
    }

    fn press(&mut self, keys: &str) {
        self.with_window(|window, cx| window.press(keys, cx));
    }

    fn type_text(&mut self, text: &str) {
        self.with_window(|window, cx| window.input(text, cx));
    }
}

fn build(dark: bool, out: &Path, strip_customs: bool) -> (Harness, tempfile::TempDir) {
    let directory = tempfile::tempdir().expect("tempdir");
    let database = directory.path().join("missal.sqlite");
    seed_firs(&database, 5).expect("seed");
    let mut store = Store::open(&database).expect("open");
    if strip_customs {
        let customs: Vec<_> = store
            .placeholders()
            .expect("placeholders")
            .into_iter()
            .filter(|p| p.source == missal_core::PlaceholderSource::Custom)
            .collect();
        for custom in customs {
            store.remove_placeholder(custom.id).expect("remove");
        }
    }
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
    let (mut h, _directory) = build(dark, out, false);

    h.go(Route::Templates);
    h.shot("13-templates-empty");

    h.go(Route::Templates);
    let search = h.cx.update(|cx| {
        ShellHandle::get(cx)
            .expect("shell")
            .read(cx)
            .templates()
            .expect("templates")
            .read(cx)
            .search_input()
            .clone()
    });
    h.with_window(|window, cx| search.update(cx, |input, cx| input.focus(window, cx)));
    h.type_text("چالان");
    let target = h.cx.update(|cx| {
        let screen = ShellHandle::get(cx)
            .expect("shell")
            .read(cx)
            .templates()
            .expect("templates");
        let visible = screen.read(cx).visible(cx);
        visible
            .iter()
            .find(|item| item.name.contains("401") && item.name.contains("جوڈیشل"))
            .or(visible.first())
            .map(|item| item.id)
            .expect("a template")
    });
    h.go(Route::Template(target));
    h.shot("14-template-page");

    h.press("f2");
    h.shot("14b-template-rename");
    h.press("escape");

    h.go(Route::Settings);
    h.shot("15-settings");
    h.with_window(|window, cx| window.click(("settings-nav", 3usize), cx));
    h.click("built-in-toggle");
    h.shot("15c-settings-builtin");
    h.with_window(|window, cx| window.click(("settings-nav", 5usize), cx));
    h.shot("15d-settings-about");
}

fn run_review(dark: bool, out: &Path) {
    let (mut h, _directory) = build(dark, out, false);
    h.go(Route::NewTemplate);
    let page = h.cx.update(|cx| {
        ShellHandle::get(cx)
            .expect("shell")
            .read(cx)
            .templates()
            .expect("templates")
            .read(cx)
            .page()
            .cloned()
            .expect("page")
    });
    let docx = std::env::var_os("WP3_REVIEW_DOCX")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../templates/آمد روبکار عبوری ضمانت.docx")
        });
    h.with_window(|window, cx| {
        page.update(cx, |page, cx| page.choose_import(docx, window, cx));
    });
    h.shot("15b-import-review");
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
        run_review(dark, &out);
    }
    let _ = Models::get;
}
