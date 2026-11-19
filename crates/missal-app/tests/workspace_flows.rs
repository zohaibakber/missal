use std::path::PathBuf;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Entity, Focusable as _, TestAppContext, Window,
};
use missal_app::app::{AppShell, navigate};
use missal_app::autosave::IDLE;
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::Preferences;
use missal_app::screens::fir_form::{FirForm, ListKind};
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_core::FirId;
use missal_store::{DraftKey, Store};

struct Fixture {
    handle: AnyWindowHandle,
    shell: Entity<AppShell>,
    models: Models,
    database: PathBuf,
    _directory: tempfile::TempDir,
}

fn boot(cx: &mut TestAppContext, count: usize) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("missal.db");
    seed_firs(&database, count).unwrap();
    let store = Store::open(&database).unwrap();
    let (handle, shell, models) = cx.update(|cx| {
        let models = init_app(cx, Db::inline(store), None, Preferences::default());
        let options = missal_app::launch::window_options(&Preferences::default(), cx);
        let (handle, shell) = open_shell(&models, options, cx).unwrap();
        (handle, shell, models)
    });
    let fixture = Fixture {
        handle,
        shell,
        models,
        database,
        _directory: directory,
    };
    fixture.with(cx, |window, _| window.activate_window());
    fixture
}

impl Fixture {
    fn settle(&self, cx: &mut TestAppContext) {
        for _ in 0..4 {
            cx.run_until_parked();
            cx.update_window(self.handle, |_, window, _| window.refresh())
                .ok();
        }
        cx.run_until_parked();
    }

    fn with<R>(&self, cx: &mut TestAppContext, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        let result = cx
            .update_window(self.handle, |_, window, cx| f(window, cx))
            .unwrap();
        self.settle(cx);
        result
    }

    fn go(&self, cx: &mut TestAppContext, route: Route) {
        self.with(cx, |window, cx| navigate(route, window, cx));
    }

    fn route(&self, cx: &mut TestAppContext) -> Route {
        cx.update(|cx| self.models.nav.read(cx).current())
    }

    fn press(&self, cx: &mut TestAppContext, keys: &str) {
        self.with(cx, |window, cx| window.press(keys, cx));
    }

    fn type_text(&self, cx: &mut TestAppContext, text: &str) {
        self.with(cx, |window, cx| window.input(text, cx));
    }

    fn click(&self, cx: &mut TestAppContext, id: &'static str) {
        self.with(cx, |window, cx| window.click(id, cx));
    }

    fn form(&self, cx: &mut TestAppContext) -> Entity<FirForm> {
        cx.update(|cx| {
            self.shell
                .read(cx)
                .new_fir()
                .unwrap()
                .read(cx)
                .form()
                .clone()
        })
    }

    fn focus(&self, cx: &mut TestAppContext, input: &Entity<missal_text::TextInput>) {
        self.with(cx, |window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx))
        });
    }
}

#[gpui_kit::test]
fn the_unified_form_folds_digits_adds_entries_and_guards_cancel(cx: &mut TestAppContext) {
    let fixture = boot(cx, 2);
    fixture.go(cx, Route::Firs);
    fixture.go(cx, Route::NewFir);
    let form = fixture.form(cx);
    let (fir_no, offence, dates, accused) = cx.update(|cx| {
        let form = form.read(cx);
        (
            form.fir_no_input().clone(),
            form.offence_input().clone(),
            form.date_inputs().clone(),
            form.list_inputs(ListKind::Accused)[0].clone(),
        )
    });
    fixture.focus(cx, &fir_no);
    fixture.type_text(cx, "۴۱۲/۲۰۲۶");
    fixture.focus(cx, &dates[0]);
    fixture.type_text(cx, "۱۴/۱۱/۲۰۲۶");
    fixture.focus(cx, &dates[1]);
    fixture.type_text(cx, "13.11.2026");
    fixture.focus(cx, &offence);
    assert_eq!(
        cx.update(|cx| fir_no.read(cx).text().to_owned()),
        "412/2026"
    );
    assert_eq!(
        cx.update(|cx| dates[0].read(cx).text().to_owned()),
        "14-11-2026"
    );
    assert_eq!(
        cx.update(|cx| dates[1].read(cx).text().to_owned()),
        "13-11-2026"
    );
    fixture.type_text(cx, "ڈکیتی");
    fixture.focus(cx, &accused);
    fixture.type_text(cx, "زاہد");
    fixture.press(cx, "enter");
    let entries = cx.update(|cx| form.read(cx).list_inputs(ListKind::Accused).to_vec());
    assert_eq!(entries.len(), 2);
    assert!(fixture.with(cx, |window, cx| {
        entries[1].focus_handle(cx).is_focused(window)
    }));
    fixture.press(cx, "secondary-s");
    assert_eq!(
        cx.update(|cx| form.read(cx).errors()),
        ["accused"],
        "an empty second accused blocks saving"
    );
    fixture.type_text(cx, "طارق");
    fixture.click(cx, "cancel-fir");
    assert_eq!(fixture.route(cx), Route::NewFir);
    fixture.click(cx, "keep");
    assert_eq!(fixture.route(cx), Route::NewFir);
    fixture.press(cx, "secondary-s");
    let id = match fixture.route(cx) {
        Route::Fir(id) => id,
        other => panic!("expected the new FIR's workspace, got {other:?}"),
    };
    let saved = Store::open(&fixture.database).unwrap().fir(id).unwrap().fir;
    assert_eq!(saved.fir_no, "412/2026");
    assert_eq!(saved.date, "14-11-2026");
    assert_eq!(saved.incident_date, "13-11-2026");
    assert_eq!(saved.accused, ["زاہد", "طارق"]);

    fixture.press(cx, "e");
    assert_eq!(fixture.route(cx), Route::EditFir(id));
    let form = fixture.form(cx);
    assert!(cx.update(|cx| form.read(cx).is_edit()));
    let offence = cx.update(|cx| form.read(cx).offence_input().clone());
    fixture.focus(cx, &offence);
    fixture.type_text(cx, " ۳۹۲");
    assert!(cx.update(|cx| form.read(cx).is_dirty()));
    fixture.press(cx, "escape");
    fixture.click(cx, "discard");
    assert_eq!(fixture.route(cx), Route::Fir(id));
    let unchanged = Store::open(&fixture.database).unwrap().fir(id).unwrap().fir;
    assert_eq!(unchanged.offence, "ڈکیتی");
}

fn plain_text(editor: &Entity<missal_editor::Editor>, cx: &mut TestAppContext) -> String {
    cx.update(|cx| editor.read(cx).document().projections().plain_text)
}

#[gpui_kit::test]
fn unsaved_document_edits_are_drafted_and_recovered_after_a_crash(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let fir = FirId::new(3).unwrap();
    fixture.go(cx, Route::Fir(fir));
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    let editor = cx.update(|cx| workspace.read(cx).editor().clone());
    let id = cx.update(|cx| workspace.read(cx).active().unwrap());
    let key = DraftKey::FirDocument(id);
    let draft = |fixture: &Fixture| Store::open(&fixture.database).unwrap().draft(key).unwrap();
    fixture.with(cx, |window, cx| {
        missal_app::editor::focus(&editor, window, cx)
    });
    fixture.type_text(cx, "zzdraft");
    cx.executor().advance_clock(IDLE / 2);
    fixture.settle(cx);
    assert_eq!(
        draft(&fixture),
        None,
        "typing is not interrupted by a write"
    );
    fixture.type_text(cx, "zz");
    cx.executor().advance_clock(IDLE / 2 + IDLE / 4);
    fixture.settle(cx);
    assert_eq!(
        draft(&fixture),
        None,
        "each keystroke pushes the write back"
    );
    cx.executor().advance_clock(IDLE);
    fixture.settle(cx);
    let written = draft(&fixture).expect("an idle editor writes a draft");
    assert!(
        written
            .document
            .projections()
            .plain_text
            .contains("zzdraftzz")
    );

    fixture.with(cx, |_, cx| {
        fixture
            .models
            .nav
            .update(cx, |nav, cx| nav.replace_unguarded(Route::Firs, cx))
    });
    fixture.go(cx, Route::Fir(fir));
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    assert_eq!(cx.update(|cx| workspace.read(cx).active()), Some(id));
    assert!(cx.update(|cx| editor.read(cx).is_dirty()));
    assert!(plain_text(&editor, cx).contains("zzdraftzz"));
    assert!(cx.update(|cx| workspace.read(cx).is_dirty(cx)));
    let saved = Store::open(&fixture.database)
        .unwrap()
        .fir_document(id)
        .unwrap();
    assert!(!saved.document.projections().plain_text.contains("zzdraft"));

    fixture.press(cx, "secondary-s");
    assert!(!cx.update(|cx| editor.read(cx).is_dirty()));
    assert_eq!(draft(&fixture), None, "saving clears the draft");
    let saved = Store::open(&fixture.database)
        .unwrap()
        .fir_document(id)
        .unwrap();
    assert!(
        saved
            .document
            .projections()
            .plain_text
            .contains("zzdraftzz")
    );
}

#[gpui_kit::test]
fn a_recovered_draft_undoes_to_the_saved_document_and_dont_save_drops_it(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    let template = cx.update(|cx| fixture.models.templates.read(cx).items.first().unwrap().id);
    let key = DraftKey::Template(template);
    let draft = |fixture: &Fixture| Store::open(&fixture.database).unwrap().draft(key).unwrap();
    fixture.go(cx, Route::Template(template));
    let editor = cx.update(|cx| {
        let page = fixture
            .shell
            .read(cx)
            .templates()
            .unwrap()
            .read(cx)
            .page()
            .cloned()
            .unwrap();
        page.read(cx).editor().clone()
    });
    let original = plain_text(&editor, cx);
    fixture.with(cx, |window, cx| {
        missal_app::editor::focus(&editor, window, cx)
    });
    fixture.type_text(cx, "qqdraft");
    cx.executor().advance_clock(IDLE * 2);
    fixture.settle(cx);
    assert!(draft(&fixture).is_some());

    fixture.press(cx, "secondary-z");
    cx.executor().advance_clock(IDLE * 2);
    fixture.settle(cx);
    assert_eq!(
        draft(&fixture),
        None,
        "undoing back to the saved text drops the draft"
    );
    fixture.type_text(cx, "qqdraft");
    cx.executor().advance_clock(IDLE * 2);
    fixture.settle(cx);
    assert!(draft(&fixture).is_some());

    fixture.with(cx, |_, cx| {
        fixture
            .models
            .nav
            .update(cx, |nav, cx| nav.replace_unguarded(Route::Templates, cx))
    });
    fixture.go(cx, Route::Template(template));
    assert!(plain_text(&editor, cx).contains("qqdraft"));
    assert!(cx.update(|cx| editor.read(cx).is_dirty()));
    fixture.with(cx, |window, cx| {
        missal_app::editor::focus(&editor, window, cx)
    });
    fixture.press(cx, "secondary-z");
    assert_eq!(plain_text(&editor, cx), original);
    assert!(!cx.update(|cx| editor.read(cx).is_dirty()));
    fixture.press(cx, "secondary-shift-z");
    assert!(plain_text(&editor, cx).contains("qqdraft"));

    fixture.go(cx, Route::Settings);
    assert_eq!(fixture.route(cx), Route::Template(template));
    fixture.click(cx, "discard");
    assert_eq!(fixture.route(cx), Route::Settings);
    assert_eq!(draft(&fixture), None, "Don't save drops the draft");
    fixture.go(cx, Route::Template(template));
    assert_eq!(plain_text(&editor, cx), original);
    assert!(!cx.update(|cx| editor.read(cx).is_dirty()));
}
