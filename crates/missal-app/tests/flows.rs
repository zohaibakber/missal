use std::path::PathBuf;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Entity, TestAppContext, Window};
use missal_app::app::{AppShell, navigate};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::Preferences;
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_core::FirId;
use missal_store::Store;

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
    fixture.settle(cx);
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

    fn rows(&self, cx: &mut TestAppContext) -> usize {
        cx.update(|cx| self.models.firs.read(cx).rows().len())
    }

    fn db_firs(&self) -> Vec<missal_core::FirSummary> {
        Store::open(&self.database).unwrap().firs().unwrap()
    }
}

fn focus_input(fixture: &Fixture, cx: &mut TestAppContext, input: &Entity<missal_text::TextInput>) {
    fixture.with(cx, |window, cx| {
        input.update(cx, |input, cx| input.focus(window, cx))
    });
}

#[gpui_kit::test]
fn create_an_fir_and_see_it_in_the_list(cx: &mut TestAppContext) {
    let fixture = boot(cx, 3);
    assert_eq!(fixture.rows(cx), 3);
    fixture.go(cx, Route::NewFir);
    let form = cx.update(|cx| {
        fixture
            .shell
            .read(cx)
            .new_fir()
            .unwrap()
            .read(cx)
            .form()
            .clone()
    });
    let (fir_no, offence, dates, accused) = cx.update(|cx| {
        let form = form.read(cx);
        (
            form.fir_no_input().clone(),
            form.offence_input().clone(),
            form.date_inputs().clone(),
            form.list_inputs(missal_app::screens::fir_form::ListKind::Accused)[0].clone(),
        )
    });
    focus_input(&fixture, cx, &fir_no);
    fixture.type_text(cx, "777/2026");
    focus_input(&fixture, cx, &offence);
    fixture.type_text(cx, "چوری");
    focus_input(&fixture, cx, &dates[0]);
    fixture.type_text(cx, "5.3.2026");
    focus_input(&fixture, cx, &dates[1]);
    fixture.type_text(cx, "04-03-2026");
    focus_input(&fixture, cx, &accused);
    fixture.type_text(cx, "علی");
    assert!(cx.update(|cx| form.read(cx).is_dirty()));
    fixture.click(cx, "create-fir");
    assert_eq!(fixture.rows(cx), 4);
    let created = fixture.db_firs();
    assert_eq!(created.len(), 4);
    assert_eq!(created[0].fir_no, "777/2026");
    assert_eq!(created[0].date, "05-03-2026");
    assert!(matches!(fixture.route(cx), Route::Fir(_)));
    fixture.go(cx, Route::Firs);
    let numbers = cx.update(|cx| {
        fixture
            .shell
            .read(cx)
            .fir_list()
            .unwrap()
            .read(cx)
            .visible_numbers(cx)
    });
    assert!(numbers.contains(&"777/2026".to_owned()));
}

#[gpui_kit::test]
fn multi_select_delete_is_atomic_and_updates_the_list(cx: &mut TestAppContext) {
    let fixture = boot(cx, 10);
    let list = cx.update(|cx| fixture.shell.read(cx).fir_list().unwrap());
    fixture.with(cx, |_, cx| {
        for index in 0..3 {
            list.update(cx, |list, cx| list.select_row(index, cx));
        }
    });
    assert_eq!(cx.update(|cx| list.read(cx).selected_count()), 3);
    fixture.click(cx, "delete-selected");
    fixture.click(cx, "confirm");
    assert_eq!(fixture.rows(cx), 7);
    assert_eq!(fixture.db_firs().len(), 7);
    assert_eq!(cx.update(|cx| list.read(cx).selected_count()), 0);
}

#[gpui_kit::test]
fn unsaved_changes_guard_the_navigator(cx: &mut TestAppContext) {
    let fixture = boot(cx, 1);
    fixture.go(cx, Route::NewFir);
    let fir_no = cx.update(|cx| {
        fixture
            .shell
            .read(cx)
            .new_fir()
            .unwrap()
            .read(cx)
            .form()
            .read(cx)
            .fir_no_input()
            .clone()
    });
    focus_input(&fixture, cx, &fir_no);
    fixture.type_text(cx, "1");
    fixture.go(cx, Route::Settings);
    assert_eq!(fixture.route(cx), Route::NewFir);
    assert!(fixture.with(cx, |window, cx| window.has_active_dialog(cx)));
    fixture.click(cx, "keep");
    assert_eq!(fixture.route(cx), Route::NewFir);
    fixture.go(cx, Route::Settings);
    fixture.click(cx, "discard");
    assert_eq!(fixture.route(cx), Route::Settings);
}

#[gpui_kit::test]
fn workspace_edit_save_and_discard_flow(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fixture.go(cx, Route::Fir(FirId::new(3).unwrap()));
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    assert_eq!(cx.update(|cx| workspace.read(cx).documents().len()), 3);
    let editor = cx.update(|cx| workspace.read(cx).editor().clone());
    assert!(!cx.update(|cx| editor.read(cx).is_dirty()));
    let id = cx.update(|cx| workspace.read(cx).active().unwrap());
    let before = Store::open(&fixture.database)
        .unwrap()
        .fir_document(id)
        .unwrap()
        .revision;
    fixture.with(cx, |window, cx| {
        missal_app::editor::focus(&editor, window, cx)
    });
    fixture.type_text(cx, "zzsaved");
    assert!(cx.update(|cx| editor.read(cx).is_dirty()));
    fixture.go(cx, Route::Settings);
    assert_eq!(fixture.route(cx), Route::Fir(FirId::new(3).unwrap()));
    fixture.click(cx, "keep");
    fixture.press(cx, "secondary-s");
    assert!(!cx.update(|cx| editor.read(cx).is_dirty()));
    let after = Store::open(&fixture.database)
        .unwrap()
        .fir_document(id)
        .unwrap();
    assert_eq!(after.revision.get(), before.get() + 1);
    assert!(after.document.projections().plain_text.contains("zzsaved"));
    fixture.with(cx, |window, cx| {
        missal_app::editor::focus(&editor, window, cx)
    });
    fixture.type_text(cx, "zzdiscarded");
    assert!(cx.update(|cx| editor.read(cx).is_dirty()));
    fixture.go(cx, Route::Settings);
    fixture.click(cx, "discard");
    assert_eq!(fixture.route(cx), Route::Settings);
    let stored = Store::open(&fixture.database)
        .unwrap()
        .fir_document(id)
        .unwrap();
    assert!(
        !stored
            .document
            .projections()
            .plain_text
            .contains("zzdiscarded")
    );
}

#[gpui_kit::test]
fn custom_field_values_are_saved_per_fir(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let fir = FirId::new(3).unwrap();
    fixture.go(cx, Route::Fir(fir));
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    let panel = cx.update(|cx| workspace.read(cx).panel().clone());
    fixture.click(cx, "details-tab");
    fixture.with(cx, |_, cx| {
        panel.update(cx, |panel, cx| panel.set_show_all(true, cx))
    });
    let custom = Store::open(&fixture.database)
        .unwrap()
        .placeholders()
        .unwrap()
        .into_iter()
        .find(|p| p.source == missal_core::PlaceholderSource::Custom)
        .unwrap()
        .id;
    let input = cx.update(|cx| panel.read(cx).custom_input(custom).unwrap());
    focus_input(&fixture, cx, &input);
    fixture.type_text(cx, "قدر");
    fixture.press(cx, "enter");
    let stored = Store::open(&fixture.database)
        .unwrap()
        .placeholder_values(fir)
        .unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].value, "قدر");
    let values = cx.update(|cx| workspace.read(cx).values().unwrap().clone());
    assert_eq!(values.overrides[0].value, "قدر");
    fixture.with(cx, |window, cx| {
        input.update(cx, |input, cx| {
            input.set_text("", cx);
            input.focus(window, cx);
        })
    });
    fixture.press(cx, "enter");
    assert!(
        Store::open(&fixture.database)
            .unwrap()
            .placeholder_values(fir)
            .unwrap()
            .is_empty()
    );
    fixture.go(cx, Route::Fir(FirId::new(2).unwrap()));
    assert!(
        Store::open(&fixture.database)
            .unwrap()
            .placeholder_values(FirId::new(2).unwrap())
            .unwrap()
            .is_empty()
    );
}
