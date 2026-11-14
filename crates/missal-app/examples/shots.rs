use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, Bounds, HeadlessAppContext, Size, WindowBounds, WindowOptions, point, px, size,
};
use missal_app::app::{AppShell, ShellHandle, navigate, show_shortcuts, toggle_nav_menu};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::{Preferences, ThemeChoice};
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_app::state::{ToastKind, show_toast};
use missal_app::ui::dialogs::confirm_delete;
use missal_app::ui::toast::ToastSpec;
use missal_app::ui::window_controls::{ControlsStyle, ForceControls};
use missal_core::{FirId, TemplateId};
use missal_store::Store;

const DEFAULT_OUT: &str =
    "/tmp/claude-0/-home-user-missal/da28de75-7983-5932-bdf9-e2e3d5312f4e/scratchpad/wp0";

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
    let database = directory.path().join("missal.db");
    seed_firs(&database, 90).expect("seed");
    let mut store = Store::open(&database).expect("open");
    let customs: Vec<_> = store
        .placeholders()
        .expect("placeholders")
        .into_iter()
        .filter(|p| p.source == missal_core::PlaceholderSource::Custom)
        .collect();
    let fir = FirId::new(3).expect("id");
    store
        .set_placeholder_values(
            fir,
            &[
                (customs[0].id, "نیشنل بینک آف پاکستان".to_owned()),
                (customs[3].id, "123456".to_owned()),
            ],
        )
        .expect("values");
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
    let (handle, models) = cx.update(|cx| {
        let models = init_app(cx, Db::inline(store), None, preferences);
        cx.set_reduce_motion(true);
        cx.set_global(ForceControls(ControlsStyle::Windows));
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
    let _ = models;
    let mut harness = Harness {
        cx,
        handle,
        out: out.to_owned(),
        suffix,
    };
    harness.settle();
    (harness, directory)
}

fn shell(cx: &gpui_kit::App) -> gpui_kit::Entity<AppShell> {
    ShellHandle::get(cx).expect("shell")
}

fn run_wp0(h: &mut Harness) {
    h.go(Route::Components);
    h.shot("00-components");
    h.go(Route::Firs);
    h.shot("01-fir-list");
    h.with_window(toggle_nav_menu);
    h.shot("25-nav-menu");
    h.press("up");
    h.press("up");
    h.press("right");
    h.shot("25b-nav-menu-theme");
    h.press("escape");
    h.press("escape");
    h.with_window(show_shortcuts);
    h.shot("16-shortcuts");
    h.reset();
    for id in [11, 7, 3] {
        h.go(Route::Fir(FirId::new(id).expect("id")));
    }
    h.shot("28-workspace-header");
    h.go(Route::Firs);
    h.press("secondary-k");
    h.shot("17b-palette-recents");
    h.with_window(|window, cx| window.input("11/20", cx));
    h.shot("17-command-palette");
    h.press("escape");
    h.press("escape");
    h.with_window(|window, cx| {
        confirm_delete(
            window,
            cx,
            "Delete FIR 9/2026?",
            "Its 3 documents are deleted too. This can't be undone.",
            "Delete FIR",
            |_, _| {},
        )
    });
    h.shot("26-dialog");
    h.reset();
    h.with_window(|window, cx| {
        show_toast(
            window,
            cx,
            ToastSpec::new(
                ToastKind::Success,
                format!(
                    "Added {}",
                    missal_app::ui::isolate_urdu("ضمنی حصول ریمانڈ جسمانی")
                ),
            )
            .action("Undo", |_, _| {}),
        );
        show_toast(
            window,
            cx,
            ToastSpec::new(ToastKind::Error, "Couldn't save. The disk is full.")
                .action("Retry", |_, _| {}),
        );
    });
    h.shot("27-toasts");
    h.with_window(|_, cx| {
        let toasts = shell(cx).read(cx).toasts().clone();
        toasts.update(cx, |toasts, cx| toasts.clear(cx));
    });
    let template = h.models_template_id();
    h.go(Route::Template(template));
    h.shot("29-template-header");
    h.go(Route::Firs);
}

fn run(dark: bool, out: &Path, only_wp0: bool) {
    let (mut h, _directory) = build(dark, out, 1280.0, 800.0);
    run_wp0(&mut h);
    if only_wp0 {
        return;
    }

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

    h.go(Route::EditFir(FirId::new(3).expect("id")));
    h.shot("09-edit-fir");
    h.reset();

    h.go(Route::Fir(FirId::new(3).expect("id")));
    h.shot("10-workspace");
    h.click("add-templates-trigger");
    h.with_window(|window, cx| window.input("ریمانڈ", cx));
    h.shot("11-add-document");
    h.press("escape");
    h.with_window(|window, cx| {
        if let Some(workspace) = shell(cx).read(cx).workspace() {
            workspace.update(cx, |workspace, cx| workspace.show_fields(window, cx));
        }
    });
    h.shot("12-fields-panel");
    h.reset();

    h.go(Route::Templates);
    h.shot("13-templates-empty");
    let template = h.models_template_id();
    h.go(Route::Template(template));
    h.shot("14-template-page");

    h.go(Route::Settings);
    h.shot("15-settings");

    h.with_window(show_shortcuts);
    h.shot("16b-shortcuts-legacy-path");
    h.reset();

    h.go(Route::Firs);
    h.press("secondary-k");
    h.shot("18-command-palette-empty");
    h.press("escape");
    h.shot("18-fir-list-after");

    h.go(Route::Fir(FirId::new(3).expect("id")));
    h.press("secondary-p");
    h.wait_for_preview();
    h.shot("19-print-preview-fir");
    h.press("pagedown");
    h.press("pagedown");
    h.shot("20-print-preview-page-3");
    h.press("secondary-=");
    h.press("secondary-=");
    h.shot("21-print-preview-zoomed");
    h.press("secondary-0");
    h.click("preview-zoom");
    h.shot("22-print-preview-zoom-menu");
    h.press("escape");
    h.reset();
    h.press("escape");

    let template = h.models_template_id();
    h.go(Route::Template(template));
    h.press("secondary-p");
    h.wait_for_preview();
    h.shot("23-print-preview-template");
    h.press("escape");

    h.go(Route::Fir(FirId::new(3).expect("id")));
    h.open_failing_preview();
    h.shot("24-print-preview-error");
    h.press("escape");
    h.go(Route::Firs);
}

impl Harness {
    fn wait_for_preview(&mut self) {
        for _ in 0..40 {
            self.settle();
            let ready = self.cx.update(|cx| {
                let shell = missal_app::app::ShellHandle::get(cx);
                shell
                    .and_then(|shell| shell.read(cx).print_preview())
                    .is_some_and(|preview| preview.read(cx).is_ready())
            });
            if ready {
                break;
            }
        }
    }

    fn open_failing_preview(&mut self) {
        self.with_window(|window, cx| {
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
    }

    fn models_template_id(&mut self) -> TemplateId {
        self.cx.update(|cx| {
            let models = Models::get(cx);
            models
                .templates
                .read(cx)
                .items
                .first()
                .map(|item| item.id)
                .expect("a template")
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
    let only = args
        .iter()
        .position(|argument| argument == "--theme")
        .map(|index| args[index + 1].clone());
    let only_wp0 = args.iter().any(|argument| argument == "--wp0");
    for dark in [false, true] {
        if only.as_deref().is_some_and(|only| (only == "dark") != dark) {
            continue;
        }
        run(dark, &out, only_wp0);
    }
    let _ = Size::<gpui_kit::Pixels>::default;
}
