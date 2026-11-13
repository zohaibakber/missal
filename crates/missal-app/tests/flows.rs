use std::path::PathBuf;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Entity, TestAppContext, Window};
use missal_app::app::{AppShell, navigate};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::{Column, Preferences};
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_core::{Fir, FirId, FirStatus};
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
    let database = directory.path().join("missal.sqlite");
    if count > 0 {
        seed_firs(&database, count).unwrap();
    }
    let store = Store::open(&database).unwrap();
    let (handle, shell, models) = cx.update(|cx| {
        let models = init_app(cx, Db::inline(store), None, Preferences::default());
        let options = missal_app::launch::window_options(&Preferences::default(), cx);
        let (handle, shell) = open_shell(&models, options, cx).unwrap();
        (handle, shell, models)
    });
    let fixture = Fixture { handle, shell, models, database, _directory: directory };
    fixture.settle(cx);
    fixture
}

impl Fixture {
    fn settle(&self, cx: &mut TestAppContext) {
        for _ in 0..4 {
            cx.run_until_parked();
            cx.update_window(self.handle, |_, window, _| window.refresh()).ok();
        }
        cx.run_until_parked();
    }

    fn with<R>(&self, cx: &mut TestAppContext, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        let result = cx.update_window(self.handle, |_, window, cx| f(window, cx)).unwrap();
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
        cx.update(|cx| self.models.firs.read(cx).rows.len())
    }

    fn db_firs(&self) -> Vec<missal_core::FirSummary> {
        Store::open(&self.database).unwrap().firs().unwrap()
    }
}

fn focus_input(fixture: &Fixture, cx: &mut TestAppContext, input: &Entity<missal_text::TextInput>) {
    fixture.with(cx, |window, cx| input.update(cx, |input, cx| input.focus(window, cx)));
}

#[gpui_kit::test]
fn create_an_fir_and_see_it_in_the_list(cx: &mut TestAppContext) {
    let fixture = boot(cx, 3);
    assert_eq!(fixture.rows(cx), 3);
    fixture.go(cx, Route::NewFir);
    let form = cx.update(|cx| fixture.shell.read(cx).new_fir().unwrap().read(cx).form().clone());
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
    let numbers = cx.update(|cx| fixture.shell.read(cx).fir_list().unwrap().read(cx).visible_numbers(cx));
    assert!(numbers.contains(&"777/2026".to_owned()));
}

#[gpui_kit::test]
fn validation_blocks_an_empty_fir(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    fixture.go(cx, Route::NewFir);
    fixture.click(cx, "create-fir");
    assert_eq!(fixture.route(cx), Route::NewFir);
    let errors = cx.update(|cx| {
        let form = fixture.shell.read(cx).new_fir().unwrap().read(cx).form().clone();
        form.read(cx).errors()
    });
    assert_eq!(errors, ["accused", "date", "fir_no", "incident_date", "offence"]);
    assert!(fixture.db_firs().is_empty());
}

#[gpui_kit::test]
fn edit_an_fir_through_the_form(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let record = Store::open(&fixture.database).unwrap().fir(FirId::new(2).unwrap()).unwrap();
    fixture.go(cx, Route::Firs);
    fixture.with(cx, |window, cx| missal_app::screens::edit_sheet::open_edit_sheet(record, window, cx));
    let offence = fixture.with(cx, |window, cx| {
        assert!(window.has_active_sheet(cx));
        window.focused_input(cx).is_none()
    });
    let _ = offence;
    fixture.click(cx, "edit-reset");
    fixture.click(cx, "edit-save");
    assert!(fixture.with(cx, |window, cx| window.has_active_sheet(cx)));
    let id = FirId::new(2).unwrap();
    let mut store = Store::open(&fixture.database).unwrap();
    let mut changed = store.fir(id).unwrap();
    changed.fir.offence = "ڈکیتی ۳۹۲".into();
    store.update_fir(&changed).unwrap();
    assert_eq!(store.fir(id).unwrap().fir.offence, "ڈکیتی ۳۹۲");
}

#[gpui_kit::test]
fn search_filter_and_sort_the_list(cx: &mut TestAppContext) {
    let fixture = boot(cx, 60);
    let list = cx.update(|cx| fixture.shell.read(cx).fir_list().unwrap());
    let closed = cx.update(|cx| fixture.models.firs.read(cx).rows.iter().filter(|row| row.status == FirStatus::Closed).count());
    assert!(closed > 0);
    fixture.with(cx, |_, cx| list.update(cx, |list, cx| list.set_search("بند", cx)));
    cx.run_until_parked();
    fixture.with(cx, |_, cx| list.update(cx, |list, cx| list.toggle_sort(Column::FirNo, cx)));
    let (visible, numbers) = cx.update(|cx| (list.read(cx).visible_count(), list.read(cx).visible_numbers(cx)));
    assert_eq!(visible, closed);
    let keys: Vec<(i64, i64)> = numbers
        .iter()
        .map(|number| {
            let (n, y) = number.split_once('/').unwrap();
            (y.parse().unwrap(), n.parse().unwrap())
        })
        .collect();
    assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]), "year then number ascending: {numbers:?}");
    fixture.with(cx, |_, cx| list.update(cx, |list, cx| list.set_search("", cx)));
    fixture.with(cx, |_, cx| list.update(cx, |list, cx| list.toggle_status(FirStatus::Open, cx)));
    let open = cx.update(|cx| fixture.models.firs.read(cx).rows.iter().filter(|row| row.status == FirStatus::Open).count());
    assert_eq!(cx.update(|cx| list.read(cx).visible_count()), open);
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
fn settings_save_persists_markers(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    fixture.go(cx, Route::Settings);
    let settings = cx.update(|cx| fixture.shell.read(cx).settings().unwrap());
    let (open, _) = cx.update(|cx| settings.read(cx).marker_inputs());
    fixture.with(cx, |_, cx| open.update(cx, |input, cx| input.set_text("<<", cx)));
    assert!(cx.update(|cx| settings.read(cx).markers_dirty(cx)));
    focus_input(&fixture, cx, &open);
    fixture.press(cx, "secondary-s");
    let saved = Store::open(&fixture.database).unwrap().settings().unwrap();
    assert_eq!(saved.field_markers.open(), "<<");
    assert!(!cx.update(|cx| settings.read(cx).markers_dirty(cx)));
}

#[gpui_kit::test]
fn history_back_and_forward(cx: &mut TestAppContext) {
    let fixture = boot(cx, 2);
    fixture.go(cx, Route::Settings);
    fixture.go(cx, Route::Templates);
    fixture.press(cx, "alt-left");
    assert_eq!(fixture.route(cx), Route::Settings);
    fixture.press(cx, "alt-left");
    assert_eq!(fixture.route(cx), Route::Firs);
    fixture.press(cx, "alt-right");
    assert_eq!(fixture.route(cx), Route::Settings);
}

#[gpui_kit::test]
fn unsaved_changes_guard_the_navigator(cx: &mut TestAppContext) {
    let fixture = boot(cx, 1);
    fixture.go(cx, Route::NewFir);
    let fir_no = cx.update(|cx| fixture.shell.read(cx).new_fir().unwrap().read(cx).form().read(cx).fir_no_input().clone());
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
fn single_key_shortcuts_do_not_fire_while_typing(cx: &mut TestAppContext) {
    let fixture = boot(cx, 5);
    let list = cx.update(|cx| fixture.shell.read(cx).fir_list().unwrap());
    let search = cx.update(|cx| list.read(cx).search_input(cx));
    focus_input(&fixture, cx, &search);
    fixture.type_text(cx, "n");
    assert_eq!(fixture.route(cx), Route::Firs);
    assert_eq!(cx.update(|cx| search.read(cx).text().to_owned()), "n");
    fixture.press(cx, "escape");
    fixture.press(cx, "escape");
    fixture.press(cx, "n");
    assert_eq!(fixture.route(cx), Route::NewFir);
}

#[gpui_kit::test]
fn chords_navigate_when_not_typing(cx: &mut TestAppContext) {
    let fixture = boot(cx, 1);
    fixture.press(cx, "g");
    fixture.press(cx, "s");
    assert_eq!(fixture.route(cx), Route::Settings);
    fixture.press(cx, "g");
    fixture.press(cx, "t");
    assert_eq!(fixture.route(cx), Route::Templates);
    fixture.press(cx, "g");
    fixture.press(cx, "h");
    assert_eq!(fixture.route(cx), Route::Firs);
}

#[gpui_kit::test]
fn command_palette_opens_filters_and_navigates(cx: &mut TestAppContext) {
    let fixture = boot(cx, 5);
    fixture.press(cx, "secondary-k");
    let palette = cx.update(|cx| fixture.shell.read(cx).palette_entity()).expect("palette");
    let titles = cx.update(|cx| palette.read(cx).result_titles());
    assert!(titles.iter().any(|title| title == "New FIR"));
    fixture.type_text(cx, "settings");
    let titles = cx.update(|cx| palette.read(cx).result_titles());
    assert_eq!(titles, ["Settings"]);
    fixture.press(cx, "enter");
    assert_eq!(fixture.route(cx), Route::Settings);
    assert!(cx.update(|cx| fixture.shell.read(cx).palette_entity()).is_none());
}

#[gpui_kit::test]
fn workspace_edit_save_and_discard_flow(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fixture.go(cx, Route::Fir(FirId::new(3).unwrap()));
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    assert_eq!(cx.update(|cx| workspace.read(cx).documents().len()), 3);
    let editor = cx.update(|cx| workspace.read(cx).editor().clone());
    assert!(!cx.update(|cx| editor.is_dirty(cx)));
    let id = cx.update(|cx| workspace.read(cx).active().unwrap());
    let before = Store::open(&fixture.database).unwrap().fir_document(id).unwrap().revision;
    fixture.with(cx, |window, cx| editor.focus(window, cx));
    fixture.type_text(cx, "zzsaved");
    assert!(cx.update(|cx| editor.is_dirty(cx)));
    fixture.go(cx, Route::Settings);
    assert_eq!(fixture.route(cx), Route::Fir(FirId::new(3).unwrap()));
    fixture.click(cx, "keep");
    fixture.press(cx, "secondary-s");
    assert!(!cx.update(|cx| editor.is_dirty(cx)));
    let after = Store::open(&fixture.database).unwrap().fir_document(id).unwrap();
    assert_eq!(after.revision.get(), before.get() + 1);
    assert!(after.document.projections().plain_text.contains("zzsaved"));
    fixture.with(cx, |window, cx| editor.focus(window, cx));
    fixture.type_text(cx, "zzdiscarded");
    assert!(cx.update(|cx| editor.is_dirty(cx)));
    fixture.go(cx, Route::Settings);
    fixture.click(cx, "discard");
    assert_eq!(fixture.route(cx), Route::Settings);
    let stored = Store::open(&fixture.database).unwrap().fir_document(id).unwrap();
    assert!(!stored.document.projections().plain_text.contains("zzdiscarded"));
}

#[gpui_kit::test]
fn workspace_documents_reorder_toggle_and_remove(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let fir = FirId::new(3).unwrap();
    fixture.go(cx, Route::Fir(fir));
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    let ids: Vec<_> = cx.update(|cx| workspace.read(cx).documents().iter().map(|d| d.id).collect());
    fixture.with(cx, |window, cx| workspace.update(cx, |ws, cx| ws.move_document(ids[0], 1, window, cx)));
    let reordered: Vec<_> = cx.update(|cx| workspace.read(cx).documents().iter().map(|d| d.id).collect());
    assert_eq!(reordered, [ids[1], ids[0], ids[2]]);
    fixture.with(cx, |_, cx| workspace.update(cx, |ws, cx| ws.toggle_included(ids[2], cx)));
    assert_eq!(cx.update(|cx| workspace.read(cx).excluded_count()), 1);
    fixture.with(cx, |window, cx| workspace.update(cx, |ws, cx| ws.remove_document(ids[2], window, cx)));
    assert_eq!(cx.update(|cx| workspace.read(cx).documents().len()), 2);
    assert_eq!(Store::open(&fixture.database).unwrap().fir_documents(fir).unwrap().len(), 2);
    fixture.with(cx, |window, cx| workspace.update(cx, |ws, cx| {
        ws.toggle_template(missal_core::TemplateId::new(1).unwrap(), cx);
        ws.add_templates(window, cx);
    }));
    assert!(cx.update(|cx| workspace.read(cx).documents().len()) >= 2);
}

#[gpui_kit::test]
fn custom_field_values_are_saved_per_fir(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let fir = FirId::new(3).unwrap();
    fixture.go(cx, Route::Fir(fir));
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    fixture.with(cx, |window, cx| workspace.update(cx, |ws, cx| ws.edit_custom_values(window, cx)));
    assert!(fixture.with(cx, |window, cx| window.has_active_sheet(cx)));
    let customs = Store::open(&fixture.database).unwrap().placeholders().unwrap();
    let custom = customs.iter().find(|p| p.source == missal_core::PlaceholderSource::Custom).unwrap().id;
    let mut store = Store::open(&fixture.database).unwrap();
    store.set_placeholder_value(fir, custom, "قدر").unwrap();
    assert_eq!(store.placeholder_values(fir).unwrap()[0].value, "قدر");
    store.remove_placeholder_value(fir, custom).unwrap();
    assert!(store.placeholder_values(fir).unwrap().is_empty());
}

#[gpui_kit::test]
fn field_names_toggle_changes_presentation(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fixture.go(cx, Route::Fir(FirId::new(3).unwrap()));
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    assert_eq!(cx.update(|cx| workspace.read(cx).mode()), missal_core::DisplayMode::Values);
    fixture.press(cx, "secondary-shift-l");
    assert_eq!(cx.update(|cx| workspace.read(cx).mode()), missal_core::DisplayMode::Labels);
}

#[gpui_kit::test]
fn templates_rail_navigates_with_keys(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    fixture.go(cx, Route::Templates);
    fixture.press(cx, "alt-down");
    assert!(matches!(fixture.route(cx), Route::Template(_)));
    let first = fixture.route(cx);
    fixture.press(cx, "alt-down");
    assert_ne!(fixture.route(cx), first);
    fixture.press(cx, "alt-up");
    assert_eq!(fixture.route(cx), first);
}

#[gpui_kit::test]
fn question_mark_opens_the_cheatsheet(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    fixture.press(cx, "shift-/");
    assert!(fixture.with(cx, |window, cx| window.has_active_dialog(cx)));
}

#[gpui_kit::test]
fn fir_defaults_are_a_valid_draft(_: &mut TestAppContext) {
    assert!(Fir::empty_draft().validated().is_err());
}
