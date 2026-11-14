use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, Bounds, Entity, HeadlessAppContext, WindowBounds, WindowOptions, point, px,
    size,
};
use missal_app::app::{AppShell, ShellHandle, navigate};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::{Preferences, ThemeChoice};
use missal_app::screens::fir_form::{FirForm, ListKind};
use missal_app::seed::seed_firs;
use missal_app::state::Route;
use missal_app::ui::window_controls::{ControlsStyle, ForceControls};
use missal_core::FirId;
use missal_store::Store;
use missal_text::TextInput;

const DEFAULT_OUT: &str =
    "/tmp/claude-0/-home-user-missal/da28de75-7983-5932-bdf9-e2e3d5312f4e/scratchpad/wp2";

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
        println!("{}", path.display());
    }

    fn with<R>(&mut self, f: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App) -> R) -> R {
        let result = self
            .cx
            .update_window(self.handle, |_, window, cx| f(window, cx))
            .expect("window");
        self.settle();
        result
    }

    fn go(&mut self, route: Route) {
        self.with(|window, cx| {
            window.close_all_dialogs(cx);
            navigate(route, window, cx)
        });
    }

    fn press(&mut self, keys: &str) {
        self.with(|window, cx| window.press(keys, cx));
    }

    fn click(&mut self, id: &'static str) {
        self.with(|window, cx| window.click(id, cx));
    }

    fn shell(&mut self) -> Entity<AppShell> {
        self.cx.update(|cx| ShellHandle::get(cx).expect("shell"))
    }

    fn form(&mut self) -> Entity<FirForm> {
        let shell = self.shell();
        self.cx.update(|cx| {
            shell
                .read(cx)
                .new_fir()
                .expect("form screen")
                .read(cx)
                .form()
                .clone()
        })
    }

    fn type_into(&mut self, input: Entity<TextInput>, text: &str) {
        let text = text.to_owned();
        self.with(move |window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
            window.input(&text, cx);
        });
    }
}

fn build(dark: bool, out: &Path, width: f32, height: f32) -> (Harness, tempfile::TempDir) {
    let directory = tempfile::tempdir().expect("tempdir");
    let database = directory.path().join("missal.db");
    seed_firs(&database, 90).expect("seed");
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
                size: size(px(width), px(height)),
            })),
            ..gpui_kit::component::TitleBar::window_options()
        };
        open_shell(&models, options, cx).expect("window").0
    });
    let mut harness = Harness {
        cx,
        handle,
        out: out.to_owned(),
        suffix: if dark { "dark" } else { "light" },
    };
    harness.with(|window, _| window.activate_window());
    (harness, directory)
}

fn run(dark: bool, out: &Path) {
    let (mut h, _directory) = build(dark, out, 1280.0, 800.0);

    h.go(Route::NewFir);
    let form = h.form();
    let inputs = h.cx.update(|cx| {
        let f = form.read(cx);
        (
            f.fir_no_input().clone(),
            f.offence_input().clone(),
            f.complainant_input().clone(),
            f.date_inputs().clone(),
        )
    });
    h.type_into(inputs.0, "412/2026");
    h.type_into(inputs.1, "ڈکیتی ۳۹۲ ت پ");
    h.type_into(inputs.2, "محمد اسلم ولد اکرم سکنہ چک ۴۵");
    h.type_into(inputs.3[0].clone(), "14-11-2026");
    h.type_into(inputs.3[1].clone(), "13-11-2026");
    let accused =
        h.cx.update(|cx| form.read(cx).list_inputs(ListKind::Accused)[0].clone());
    h.type_into(accused, "زاہد حسین ولد حسین بخش سکنہ ساہیوال");
    h.with({
        let form = form.clone();
        move |window, cx| form.update(cx, |form, cx| form.add_entry(ListKind::Accused, window, cx))
    });
    h.with({
        let form = form.clone();
        move |window, cx| form.update(cx, |form, cx| form.add_entry(ListKind::Witness, window, cx))
    });
    let witness =
        h.cx.update(|cx| form.read(cx).list_inputs(ListKind::Witness)[0].clone());
    h.type_into(witness, "کامران اکبر ولد اکبر علی سکنہ سیالکوٹ");
    let second =
        h.cx.update(|cx| form.read(cx).list_inputs(ListKind::Accused)[1].clone());
    h.type_into(second, "طارق جاوید ولد جاوید اقبال سکنہ فیص");
    h.shot("07-new-fir");

    h.go(Route::Firs);
    h.click("discard");
    h.go(Route::NewFir);
    h.click("create-fir");
    h.shot("08-new-fir-validation");
    h.with(|window, cx| window.close_all_dialogs(cx));

    h.go(Route::EditFir(FirId::new(3).expect("id")));
    h.shot("09-edit-fir");

    h.go(Route::Fir(FirId::new(3).expect("id")));
    h.shot("10-workspace");
    h.click("add-templates-trigger");
    h.with(|window, cx| window.input("ریمانڈ", cx));
    h.press("down");
    h.press("space");
    h.shot("11-add-document");
    h.press("escape");

    let workspace =
        h.cx.update(|cx| ShellHandle::get(cx).unwrap().read(cx).workspace().unwrap());
    h.with({
        let workspace = workspace.clone();
        move |window, cx| workspace.update(cx, |ws, cx| ws.show_fields(window, cx))
    });
    h.with(|window, cx| window.input("تھانہ صدر", cx));
    h.shot("12-fields-panel");
    h.press("escape");
    h.click("details-tab");
    h.shot("12b-case-details");
    h.with({
        let workspace = workspace.clone();
        move |_, cx| {
            let panel = workspace.read(cx).panel().clone();
            panel.update(cx, |panel, cx| panel.set_show_all(true, cx))
        }
    });
    h.shot("12c-custom-values");

    h.go(Route::Fir(FirId::new(3).expect("id")));
    h.press("secondary-shift-l");
    h.shot("10b-workspace-names");
    h.press("secondary-shift-l");

    let (mut narrow, _dir) = build(dark, out, 1024.0, 700.0);
    narrow.go(Route::Fir(FirId::new(3).expect("id")));
    narrow.shot("10c-workspace-1024");
    narrow.go(Route::NewFir);
    narrow.shot("07b-new-fir-1024");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .position(|a| a == "--out")
        .map(|i| PathBuf::from(&args[i + 1]))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    std::fs::create_dir_all(&out).expect("output directory");
    let only = args
        .iter()
        .position(|a| a == "--theme")
        .map(|i| args[i + 1].clone());
    for dark in [false, true] {
        if only.as_deref().is_some_and(|only| (only == "dark") != dark) {
            continue;
        }
        run(dark, &out);
    }
}
