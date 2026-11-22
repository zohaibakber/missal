use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, Bounds, HeadlessAppContext, Size, WindowBounds, WindowOptions, point, px, size,
};
use missal_app::app::{
    AppShell, ShellHandle, navigate, show_shortcuts, toggle_nav_menu, toggle_sidebar,
};
use missal_app::auth::{AuthModel, AuthState, Flow};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::library::LibraryModel;
use missal_app::prefs::{Preferences, ThemeChoice};
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_app::state::{ToastKind, show_toast};
use missal_app::ui::dialogs::confirm_delete;
use missal_app::ui::toast::ToastSpec;
use missal_app::ui::window_controls::{ControlsStyle, ForceControls};
use missal_core::{FirId, TemplateId};
use missal_store::Store;

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

fn build(
    dark: bool,
    out: &Path,
    width: f32,
    height: f32,
    auth: Option<missal_auth::Config>,
) -> (Harness, tempfile::TempDir) {
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
        if let Some(config) = auth {
            AuthModel::install_with(Some(config), None, cx);
            LibraryModel::install(&models, cx);
        }
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
    h.click("account-menu");
    h.shot("26-user-menu");
    h.press("right");
    h.shot("26b-user-menu-theme-switched");
    h.press("left");
    h.press("escape");
    h.with_window(|_, cx| toggle_sidebar(cx));
    h.shot("26c-sidebar-hidden");
    h.with_window(|_, cx| toggle_sidebar(cx));
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
        show_toast(
            window,
            cx,
            ToastSpec::new(ToastKind::Info, "Update ready")
                .sticky("Later")
                .action("Restart now", |_, _| {}),
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
    let (mut h, _directory) = build(dark, out, 1280.0, 800.0, None);
    run_wp0(&mut h);
    if only_wp0 {
        return;
    }

    h.click("select-all");
    h.shot("03-selection");
    h.click("select-all");
    h.with_window(|window, cx| {
        window.click(("row-select-cell", 0usize), cx);
        window.click(("row-select-cell", 2usize), cx);
    });
    h.press("j");
    h.shot("03b-partial-selection");
    h.press("escape");
    h.press("escape");
    h.click("status-filter");
    h.shot("04-status-menu");
    h.click("status-Open");
    h.shot("04-status-filter");
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
    let prefix = h.with_window(|window, cx| {
        let top = Models::get(cx).suggestions.read(cx).suggest(
            missal_core::SuggestionKey::Fir(missal_core::FirProperty::Offence),
            "",
            1,
        );
        let form = shell(cx)
            .read(cx)
            .new_fir()
            .expect("form")
            .read(cx)
            .form()
            .clone();
        let offence = form.read(cx).offence_input().clone();
        offence.update(cx, |input, cx| input.focus(window, cx));
        top.first()
            .map(|s| s.value.chars().take(1).collect::<String>())
            .unwrap_or_default()
    });
    h.with_window(|window, cx| window.input(&prefix, cx));
    h.press("down");
    h.shot("07b-autocomplete");
    h.press("escape");
    h.press("secondary-a");
    h.press("backspace");
    h.click("create-fir");
    h.shot("08-new-fir-validation");
    h.reset();

    h.go(Route::EditFir(FirId::new(3).expect("id")));
    h.shot("09-edit-fir");
    h.reset();

    h.go(Route::Fir(FirId::new(3).expect("id")));
    h.shot("10-workspace");
    h.with_window(|window, cx| {
        let workspace = shell(cx).read(cx).workspace().expect("workspace");
        let editor = workspace.read(cx).editor().clone();
        let handle = editor.read(cx).focus_handle_ref().clone();
        window.focus(&handle, cx);
    });
    h.press("ctrl-end");
    h.press("enter");
    h.with_window(|window, cx| window.input("/", cx));
    h.shot("10b-slash-menu");
    h.with_window(|window, cx| window.input("tab", cx));
    h.shot("10c-slash-filter");
    h.press("escape");
    h.with_window(|_, cx| {
        let workspace = shell(cx).read(cx).workspace().expect("workspace");
        let editor = workspace.read(cx).editor().clone();
        editor.update(cx, |editor, cx| editor.discard(cx));
    });
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
    h.click("template-name");
    h.shot("14b-template-rename");
    h.press("escape");

    h.go(Route::Settings);
    h.shot("15-settings");

    h.with_window(show_shortcuts);
    h.shot("16b-shortcuts-second-open");
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
                catalog,
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
        .unwrap_or_else(|| std::env::temp_dir().join("missal-shots"));
    std::fs::create_dir_all(&out).expect("output directory");
    let only = args
        .iter()
        .position(|argument| argument == "--theme")
        .map(|index| args[index + 1].clone());
    let only_wp0 = args.iter().any(|argument| argument == "--wp0");
    let only_gate = args.iter().any(|argument| argument == "--gate");
    let skip_gate = args.iter().any(|argument| argument == "--no-gate");
    let api = fake_api::start();
    for dark in [false, true] {
        if only.as_deref().is_some_and(|only| (only == "dark") != dark) {
            continue;
        }
        if !only_gate {
            run(dark, &out, only_wp0);
        }
        if !only_wp0 && !skip_gate {
            run_gate(dark, &out, &api);
        }
    }
    let _ = Size::<gpui_kit::Pixels>::default;
}

impl Harness {
    fn fill(&mut self, id: &'static str, text: &str) {
        self.click(id);
        self.with_window(|window, cx| window.input(text, cx));
    }

    fn replace(&mut self, id: &'static str, text: &str) {
        self.click(id);
        self.press("secondary-a");
        self.with_window(|window, cx| window.input(text, cx));
    }

    fn wait_auth(&mut self, done: impl Fn(&AuthState) -> bool) {
        for _ in 0..200 {
            self.settle();
            let ready = self
                .cx
                .update(|cx| AuthModel::get(cx).is_some_and(|auth| done(auth.read(cx).state())));
            if ready {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("the sign-in gate never reached the expected state");
    }

    fn wait_flow(&mut self, expected: fn(&Flow) -> bool) {
        self.wait_auth(|state| {
            matches!(state, AuthState::SignedOut { flow, busy: false, .. } if expected(flow))
        });
    }

    fn idle(&mut self) {
        for _ in 0..20 {
            self.settle();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

fn noticed(state: &AuthState) -> bool {
    matches!(
        state,
        AuthState::SignedOut {
            notice: Some(_),
            busy: false,
            ..
        } | AuthState::LicenceExpired {
            notice: Some(_),
            busy: false,
            ..
        }
    )
}

fn run_gate(dark: bool, out: &Path, api: &str) {
    let config = missal_auth::Config {
        base_url: api.to_owned(),
        purchase_url: Some("https://example.com/buy".to_owned()),
    };
    let (mut h, _directory) = build(dark, out, 1280.0, 800.0, Some(config));
    h.wait_flow(|flow| *flow == Flow::SignIn);
    h.shot("40-gate-sign-in");

    h.click("auth-sign-up");
    h.wait_flow(|flow| *flow == Flow::SignUp);
    h.fill("sign-up-name", "Amina Khan");
    h.fill("sign-up-email", "new@example.com");
    h.fill("auth-new-password", "short");
    h.fill("auth-confirm", "short");
    h.click("auth-submit");
    h.shot("41-gate-sign-up-problem");
    h.replace("auth-new-password", "correct horse battery");
    h.replace("auth-confirm", "correct horse battery");
    h.shot("41b-gate-sign-up");
    h.click("auth-submit");
    h.wait_flow(|flow| matches!(flow, Flow::VerifyEmail { .. }));
    h.shot("42-gate-verify");
    h.fill("verify-code", "000000");
    h.click("auth-submit");
    h.wait_auth(noticed);
    h.shot("43-gate-verify-wrong-code");
    h.click("auth-resend");
    h.wait_auth(noticed);
    h.shot("43b-gate-verify-resent");

    h.click("auth-other-email");
    h.wait_flow(|flow| *flow == Flow::SignUp);
    h.click("auth-have-account");
    h.wait_flow(|flow| *flow == Flow::SignIn);
    h.fill("sign-in-password", "correct horse battery");
    h.click("auth-submit");
    h.wait_flow(|flow| matches!(flow, Flow::VerifyEmail { .. }));
    h.shot("44-gate-sign-in-unverified");

    h.click("auth-other-email");
    h.wait_flow(|flow| *flow == Flow::SignUp);
    h.click("auth-have-account");
    h.wait_flow(|flow| *flow == Flow::SignIn);
    h.click("auth-forgot");
    h.wait_flow(|flow| *flow == Flow::ForgotPassword);
    h.shot("45-gate-forgot");
    h.click("auth-submit");
    h.wait_flow(|flow| matches!(flow, Flow::ResetPassword { .. }));
    h.fill("reset-code", "123456");
    h.replace("auth-new-password", "short");
    h.replace("auth-confirm", "shorter");
    h.click("auth-submit");
    h.shot("46-gate-reset");

    h.click("auth-back");
    h.wait_flow(|flow| *flow == Flow::SignIn);
    h.replace("sign-in-email", "expired@example.com");
    h.press("tab");
    h.with_window(|window, cx| window.input("correct horse battery", cx));
    h.click("auth-submit");
    h.wait_auth(|state| matches!(state, AuthState::LicenceExpired { busy: false, .. }));
    h.shot("47-gate-licence-expired");
    h.click("auth-submit");
    h.wait_auth(noticed);
    h.shot("47b-gate-licence-check-again");

    h.click("licence-sign-out");
    h.wait_flow(|flow| *flow == Flow::SignIn);
    h.replace("sign-in-email", "trial@example.com");
    h.fill("sign-in-password", "correct horse battery");
    h.click("auth-submit");
    h.wait_auth(|state| matches!(state, AuthState::SignedIn { .. }));
    h.go(Route::Firs);
    h.click("account-menu");
    h.shot("48-account-menu-trial");
    h.press("escape");

    h.with_window(|_, cx| {
        AuthModel::get(cx)
            .expect("auth")
            .update(cx, |auth, cx| auth.sign_out(cx));
    });
    h.wait_flow(|flow| *flow == Flow::SignIn);
    h.replace("sign-in-email", "admin@example.com");
    h.fill("sign-in-password", "correct horse battery");
    h.click("auth-submit");
    h.wait_auth(|state| matches!(state, AuthState::SignedIn { .. }));
    h.with_window(missal_app::ui::account::open_users);
    h.idle();
    h.shot("49-people");
    h.with_window(|window, cx| window.click(("user-more", 2usize), cx));
    h.shot("49a-people-row-menu");
    h.click("user-licence-2");
    h.idle();
    h.shot("49b-people-added-year");
    h.press("escape");
    publishing_shots(&mut h);
    h.with_window(missal_app::ui::account::open_change_password);
    h.idle();
    h.fill("password-current", "correct horse battery");
    h.fill("password-new", "short");
    h.fill("password-confirm", "short");
    h.click("password-save");
    h.idle();
    h.shot("49c-change-password");
}

fn publishing_shots(h: &mut Harness) {
    h.with_window(|_, cx| {
        let mut items: Vec<_> = Models::get(cx)
            .templates
            .read(cx)
            .items
            .iter()
            .filter(|item| item.name != fake_api::TWIN)
            .cloned()
            .collect();
        items.sort_by(|left, right| left.name.cmp(&right.name));
        let stale = missal_core::Revision::new(999_999).expect("revision");
        let links = [
            (items[0].id, "r-current", 3, items[0].revision),
            (items[1].id, "r-changed", 2, stale),
            (items[2].id, "r-behind", 1, items[2].revision),
        ];
        drop(missal_app::state::db(cx).run(move |store| {
            for (template, remote, version, revision) in links {
                store.link_template(template, remote, version, revision)?;
            }
            Ok(())
        }));
        LibraryModel::get(cx)
            .expect("library")
            .update(cx, |library, cx| library.refresh(cx));
    });
    h.idle();
    h.with_window(missal_app::ui::publishing::open_publishing);
    h.idle();
    h.shot("50-publishing");
    let behind = h.with_window(|_, cx| {
        let library = LibraryModel::get(cx).expect("library");
        let library = library.read(cx);
        library
            .rows(cx)
            .iter()
            .position(|row| {
                row.remote
                    .as_ref()
                    .is_some_and(|remote| remote.id == "r-behind")
            })
            .expect("behind row")
    });
    h.with_window(|window, cx| window.click(("publishing-row", behind), cx));
    h.idle();
    h.shot("50b-publishing-history");
    h.with_window(|window, cx| window.click(("publishing-restore", 2usize), cx));
    h.idle();
    h.shot("50c-publishing-restore");
    h.click("restore-confirm");
    h.idle();
    h.shot("50d-publishing-restored");
    h.with_window(|window, cx| window.click(("publishing-filter", 1usize), cx));
    h.idle();
    h.shot("50e-publishing-needs-action");
    h.click("publishing-all");
    h.idle();
    h.shot("50f-publishing-all");
    h.click("publish-confirm");
    h.idle();
    h.idle();
    h.shot("50g-publishing-all-done");
    h.reset();
}

mod fake_api {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};

    const DAY: i64 = 24 * 60 * 60 * 1000;
    const FOREVER: i64 = 253_402_300_799_000;
    pub const TWIN: &str = "اخراج بذریعہ CDR";

    static PUBLISHED: Mutex<Vec<Value>> = Mutex::new(Vec::new());

    fn managed(id: &str, name: &str, version: i64, fields: i64, size: i64) -> Value {
        json!({
            "id": id,
            "name": name,
            "version": version,
            "fieldCount": fields,
            "preview": "",
            "size": size,
            "publisherName": "Bilal Ahmed",
            "publisherEmail": "lapsed@example.com",
            "versionCount": version,
            "createdAt": now() - 90 * DAY,
            "updatedAt": now() - 2 * DAY,
        })
    }

    fn managed_list() -> Value {
        let mut rows = vec![
            managed("r-current", "موجودہ", 3, 12, 48_200),
            managed("r-changed", "تبدیل شدہ", 2, 8, 31_400),
            managed("r-behind", "پرانا", 4, 15, 1_620_000),
            managed("r-twin", TWIN, 1, 6, 22_000),
            managed("r-remote", "صرف لائبریری میں", 2, 9, 40_960),
        ];
        rows.extend(PUBLISHED.lock().expect("published").iter().cloned());
        Value::Array(rows)
    }

    fn versions(path: &str) -> Value {
        let latest = match path.split('/').nth(4).unwrap_or_default() {
            "r-current" => 3,
            "r-changed" | "r-remote" => 2,
            "r-behind" => 4,
            _ => 1,
        };
        let row = |version: i64, restored: Option<i64>, age: i64| {
            json!({
                "version": version,
                "name": "پرانا",
                "fieldCount": 10 + version,
                "size": 1_000_000 + version * 155_000,
                "publisherName": if version % 2 == 0 { "Bilal Ahmed" } else { "Admin" },
                "publisherEmail": if version % 2 == 0 { "lapsed@example.com" } else { "admin@example.com" },
                "publishedAt": now() - age * DAY,
                "restoredFrom": restored,
            })
        };
        Value::Array(
            [
                row(4, None, 2),
                row(3, Some(1), 12),
                row(2, None, 40),
                row(1, None, 90),
            ]
            .into_iter()
            .filter(|entry| entry["version"].as_i64().unwrap_or(0) <= latest)
            .collect(),
        )
    }

    fn publish(body: &Value) -> Value {
        let mut published = PUBLISHED.lock().expect("published");
        let id = format!("r-new-{}", published.len());
        published.push(managed(
            &id,
            body["name"].as_str().unwrap_or_default(),
            1,
            body["fieldCount"].as_i64().unwrap_or(0),
            body["content"]
                .as_str()
                .map_or(0, |text| text.len() as i64 * 3 / 4),
        ));
        json!({ "id": id, "version": 1 })
    }

    fn now() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }

    fn licence(email: &str, paid: i64) -> Value {
        match email {
            "admin@example.com" => json!({ "status": "licensed", "until": FOREVER }),
            "expired@example.com" => json!({ "status": "trialEnded", "until": now() - 2 * DAY }),
            "lapsed@example.com" if paid > now() => json!({ "status": "licensed", "until": paid }),
            "lapsed@example.com" => json!({ "status": "expired", "until": now() - 40 * DAY }),
            _ => json!({ "status": "trial", "until": now() + 4 * DAY + 3_600_000 }),
        }
    }

    fn account(email: &str, paid: &Mutex<i64>) -> Value {
        let paid = *paid.lock().expect("paid");
        let (name, role, created) = match email {
            "admin@example.com" => ("Admin", "admin", now() - 400 * DAY),
            "lapsed@example.com" => ("Bilal Ahmed", "publisher", now() - 420 * DAY),
            "trial@example.com" => ("Sara Malik", "member", now() - 3 * DAY),
            _ => ("", "member", now() - 3 * DAY),
        };
        json!({
            "id": email.split('@').next().unwrap_or("user"),
            "email": email,
            "name": name,
            "role": role,
            "disabled": false,
            "createdAt": created,
            "licence": licence(email, paid),
        })
    }

    fn session(email: &str, paid: &Mutex<i64>) -> Value {
        json!({
            "accessToken": "access",
            "refreshToken": email,
            "expiresIn": 900,
            "account": account(email, paid),
        })
    }

    fn problem(tag: &str, error: &str) -> Value {
        json!({ "_tag": tag, "error": error })
    }

    fn answer(method: &str, path: &str, body: &Value, paid: &Mutex<i64>) -> (u16, Value) {
        let email = body["email"].as_str().unwrap_or_default();
        match (method, path) {
            ("POST", "/v1/auth/sign-up" | "/v1/auth/codes") => (202, json!({})),
            ("POST", "/v1/auth/sign-in") if email.starts_with("new") => {
                (403, problem("EmailNotVerified", "confirm your email first"))
            }
            ("POST", "/v1/auth/sign-in") => {
                PUBLISHED.lock().expect("published").clear();
                (200, session(email, paid))
            }
            ("POST", "/v1/auth/verify-email" | "/v1/auth/reset-password")
                if body["code"] == "123456" =>
            {
                (200, session(email, paid))
            }
            ("POST", "/v1/auth/verify-email" | "/v1/auth/reset-password") => (
                400,
                problem("InvalidCode", "that code is wrong or has expired"),
            ),
            ("POST", "/v1/auth/refresh") => (
                200,
                session(body["refreshToken"].as_str().unwrap_or_default(), paid),
            ),
            ("POST", "/v1/auth/sign-out") => (204, Value::Null),
            ("GET", "/v1/templates") => (200, json!({ "publisher": true, "templates": [] })),
            ("GET", "/v1/admin/templates") => (200, managed_list()),
            ("GET", path) if path.ends_with("/versions") => (200, versions(path)),
            ("POST", path) if path.ends_with("/restore") => {
                (200, json!({ "id": "r-behind", "version": 5 }))
            }
            ("POST", "/v1/templates") => (201, publish(body)),
            ("PUT", path) if path.starts_with("/v1/templates/") => (
                200,
                json!({ "id": path.trim_start_matches("/v1/templates/"), "version": 3 }),
            ),
            ("GET", "/v1/users") => (
                200,
                json!([
                    account("admin@example.com", paid),
                    account("trial@example.com", paid),
                    account("lapsed@example.com", paid),
                ]),
            ),
            ("POST", "/v1/users/lapsed/licence") if body["years"] == 1 => {
                *paid.lock().expect("paid") = now() + 365 * DAY;
                (200, account("lapsed@example.com", paid))
            }
            _ => (404, problem("NotFound", "not found")),
        }
    }

    fn serve(stream: TcpStream, paid: &Mutex<i64>) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_owned();
        let path = parts.next().unwrap_or_default().to_owned();
        let mut length = 0;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            if header.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;
        let body = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let (status, reply) = answer(&method, &path, &body, paid);
        let text = if reply.is_null() {
            String::new()
        } else {
            reply.to_string()
        };
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
    }

    pub fn start() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("address");
        let paid = Arc::new(Mutex::new(0));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if let Err(error) = serve(stream, &paid) {
                    eprintln!("fake api: {error}");
                }
            }
        });
        format!("http://{address}")
    }
}
