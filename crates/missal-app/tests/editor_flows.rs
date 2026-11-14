use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Entity, TestAppContext, Window};
use missal_app::app::{AppShell, navigate};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::Preferences;
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_core::{DisplayMode, FirId};
use missal_store::Store;

struct Fixture {
    handle: AnyWindowHandle,
    shell: Entity<AppShell>,
    models: Models,
    database: std::path::PathBuf,
    _directory: tempfile::TempDir,
}

fn boot(cx: &mut TestAppContext, count: usize) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("missal.sqlite");
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
}

#[gpui_kit::test]
fn bundled_template_type_and_save_bumps_the_revision(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    let template = cx.update(|cx| {
        fixture
            .models
            .templates
            .read(cx)
            .items
            .first()
            .map(|item| item.id)
            .unwrap()
    });
    let before = Store::open(&fixture.database)
        .unwrap()
        .template(template)
        .unwrap();
    fixture.with(cx, |window, cx| {
        navigate(Route::Template(template), window, cx)
    });
    let page = cx.update(|cx| {
        fixture
            .shell
            .read(cx)
            .templates()
            .unwrap()
            .read(cx)
            .page()
            .cloned()
            .unwrap()
    });
    let editor = cx.update(|cx| page.read(cx).editor().clone());
    let loaded = cx.update(|cx| editor.snapshot(cx).document);
    assert_eq!(
        loaded.blocks.len(),
        before.document.blocks.len(),
        "the real template is in the editor"
    );
    assert!(!cx.update(|cx| editor.is_dirty(cx)));
    fixture.with(cx, |window, cx| editor.focus(window, cx));
    fixture.with(cx, |window, cx| window.input("نیا متن", cx));
    assert!(cx.update(|cx| editor.is_dirty(cx)));
    fixture.with(cx, |window, cx| window.press("secondary-s", cx));
    assert!(!cx.update(|cx| editor.is_dirty(cx)));
    let after = Store::open(&fixture.database)
        .unwrap()
        .template(template)
        .unwrap();
    assert_eq!(after.revision.get(), before.revision.get() + 1);
    assert!(after.document.projections().plain_text.contains("نیا متن"));
    assert_eq!(
        after.document.field_references(),
        before.document.field_references(),
        "fields survive the edit"
    );
}

#[gpui_kit::test]
fn switching_documents_offers_save_first_and_saves(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fixture.with(cx, |window, cx| {
        navigate(Route::Fir(FirId::new(3).unwrap()), window, cx)
    });
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    let editor = cx.update(|cx| workspace.read(cx).editor().clone());
    let ids: Vec<_> = cx.update(|cx| {
        workspace
            .read(cx)
            .documents()
            .iter()
            .map(|d| d.id)
            .collect()
    });
    let first = ids[0];
    let before = Store::open(&fixture.database)
        .unwrap()
        .fir_document(first)
        .unwrap()
        .revision;
    fixture.with(cx, |window, cx| editor.focus(window, cx));
    fixture.with(cx, |window, cx| window.input("پہلی تبدیلی", cx));
    fixture.with(cx, |window, cx| {
        workspace.update(cx, |ws, cx| ws.select_document(ids[1], window, cx))
    });
    assert!(
        fixture.with(cx, |window, cx| window.has_active_dialog(cx)),
        "save-first prompt"
    );
    fixture.with(cx, |window, cx| window.click("save", cx));
    assert_eq!(cx.update(|cx| workspace.read(cx).active()), Some(ids[1]));
    let saved = Store::open(&fixture.database)
        .unwrap()
        .fir_document(first)
        .unwrap();
    assert_eq!(saved.revision.get(), before.get() + 1);
    assert!(
        saved
            .document
            .projections()
            .plain_text
            .contains("پہلی تبدیلی")
    );
    assert!(!cx.update(|cx| editor.is_dirty(cx)));
    fixture.with(cx, |window, cx| editor.focus(window, cx));
    fixture.with(cx, |window, cx| window.input("zzdiscarded", cx));
    fixture.with(cx, |window, cx| {
        workspace.update(cx, |ws, cx| ws.select_document(ids[2], window, cx))
    });
    fixture.with(cx, |window, cx| window.click("discard", cx));
    assert_eq!(cx.update(|cx| workspace.read(cx).active()), Some(ids[2]));
    let untouched = Store::open(&fixture.database)
        .unwrap()
        .fir_document(ids[1])
        .unwrap();
    assert!(
        !untouched
            .document
            .projections()
            .plain_text
            .contains("zzdiscarded")
    );
}

#[gpui_kit::test]
fn field_names_toggle_keeps_edits_and_history(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fixture.with(cx, |window, cx| {
        navigate(Route::Fir(FirId::new(3).unwrap()), window, cx)
    });
    let workspace = cx.update(|cx| fixture.shell.read(cx).workspace().unwrap());
    let editor = cx.update(|cx| workspace.read(cx).editor().clone());
    fixture.with(cx, |window, cx| editor.focus(window, cx));
    fixture.with(cx, |window, cx| window.input("zztoggle", cx));
    let before = cx.update(|cx| editor.snapshot(cx).document.encode());
    fixture.with(cx, |window, cx| window.press("secondary-shift-l", cx));
    assert_eq!(
        cx.update(|cx| workspace.read(cx).mode()),
        DisplayMode::Labels
    );
    assert_eq!(
        cx.update(|cx| editor.snapshot(cx).document.encode()),
        before,
        "presentation does not edit the document"
    );
    assert!(cx.update(|cx| editor.is_dirty(cx)));
    fixture.with(cx, |window, cx| window.press("secondary-z", cx));
    assert!(
        !cx.update(|cx| editor
            .snapshot(cx)
            .document
            .projections()
            .plain_text
            .contains("zztoggle")),
        "undo history survives the toggle"
    );
    fixture.with(cx, |window, cx| window.press("secondary-shift-l", cx));
    assert_eq!(
        cx.update(|cx| workspace.read(cx).mode()),
        DisplayMode::Values
    );
}
