use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Entity, TestAppContext, Window};
use missal_app::app::{AppShell, navigate};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::Preferences;
use missal_app::printing::Handoff;
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route, ToastKind};
use missal_app::ui::print_preview::{PreviewDocument, PrintHandler, PrintPreview};
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
    let printed = Rc::new(std::cell::RefCell::new(Vec::new()));
    let (handle, shell, models) = cx.update(|cx| {
        let log = printed.clone();
        cx.set_global(PrintHandler(Rc::new(move |path, _| {
            log.borrow_mut().push(path.to_path_buf());
            Handoff::PrintDialog
        })));
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

    fn press(&self, cx: &mut TestAppContext, keys: &str) {
        self.with(cx, |window, cx| window.press(keys, cx));
    }

    fn click(&self, cx: &mut TestAppContext, id: &'static str) {
        self.with(cx, |window, cx| window.click(id, cx));
    }

    fn preview(&self, cx: &mut TestAppContext) -> Option<Entity<PrintPreview>> {
        cx.update(|cx| self.shell.read(cx).print_preview())
    }

    fn open_preview(&self, cx: &mut TestAppContext) -> Entity<PrintPreview> {
        self.press(cx, "secondary-p");
        let preview = self.preview(cx).expect("the print preview opens");
        for _ in 0..20 {
            if cx.update(|cx| preview.read(cx).is_ready()) {
                break;
            }
            self.settle(cx);
        }
        assert!(
            cx.update(|cx| preview.read(cx).is_ready()),
            "the packet is laid out"
        );
        preview
    }

    fn toasts(
        &self,
        cx: &mut TestAppContext,
    ) -> Vec<(
        ToastKind,
        Option<gpui_kit::SharedString>,
        gpui_kit::SharedString,
    )> {
        cx.update(|cx| self.shell.read(cx).toasts().read(cx).shown())
    }
}

fn fir_workspace(
    fixture: &Fixture,
    cx: &mut TestAppContext,
) -> Entity<missal_app::screens::workspace::WorkspaceScreen> {
    fixture.go(cx, Route::Fir(FirId::new(3).unwrap()));
    cx.update(|cx| fixture.shell.read(cx).workspace().unwrap())
}

fn count_pdf_pages(bytes: &[u8]) -> usize {
    let needle = b"/Type /Page";
    bytes
        .windows(needle.len() + 1)
        .filter(|window| &window[..needle.len()] == needle && window[needle.len()] != b's')
        .count()
}

fn template_docx(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../templates")
        .join(name)
}

const BAIL_DOCX: &str = "آمد روبکار عبوری ضمانت.docx";

#[gpui_kit::test]
fn print_preview_uses_the_live_editor_content_of_the_active_document(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let workspace = fir_workspace(&fixture, cx);
    let editor = cx.update(|cx| workspace.read(cx).editor().clone());
    let active = cx.update(|cx| workspace.read(cx).active().unwrap());
    fixture.with(cx, |window, cx| {
        missal_app::editor::focus(&editor, window, cx)
    });
    fixture.with(cx, |window, cx| window.input("zzlive", cx));
    let preview = fixture.open_preview(cx);
    cx.update(|cx| {
        let preview = preview.read(cx);
        let documents = &preview.request().documents;
        assert_eq!(documents.len(), 3);
        let ids: Vec<_> = cx.update_global_free(&workspace);
        let live: Vec<_> = documents
            .iter()
            .filter_map(|document| match document {
                PreviewDocument::Content(document) => {
                    Some(document.projections().plain_text.contains("zzlive"))
                }
                PreviewDocument::Stored(_) => None,
            })
            .collect();
        assert_eq!(live, [true]);
        assert_eq!(ids, vec![active]);
    });
}

trait ActiveIds {
    fn update_global_free(
        &self,
        workspace: &Entity<missal_app::screens::workspace::WorkspaceScreen>,
    ) -> Vec<missal_core::FirDocumentId>;
}

impl ActiveIds for App {
    fn update_global_free(
        &self,
        workspace: &Entity<missal_app::screens::workspace::WorkspaceScreen>,
    ) -> Vec<missal_core::FirDocumentId> {
        workspace.read(self).active().into_iter().collect()
    }
}

#[gpui_kit::test]
fn export_pdf_writes_a_valid_pdf_with_every_page(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fir_workspace(&fixture, cx);
    let preview = fixture.open_preview(cx);
    let pages = cx.update(|cx| preview.read(cx).page_count());
    let target = fixture._directory.path().join("FIR export.pdf");
    fixture.click(cx, "preview-export");
    assert!(cx.did_prompt_for_new_path());
    let chosen = target.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    fixture.settle(cx);
    let bytes = std::fs::read(&target).expect("the PDF is written");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.trim_ascii_end().ends_with(b"%%EOF"));
    assert_eq!(count_pdf_pages(&bytes), pages);
    assert!(
        fixture
            .toasts(cx)
            .iter()
            .any(|toast| toast.0 == ToastKind::Success && toast.2.contains("FIR export.pdf"))
    );
    assert!(fixture.preview(cx).is_some());
    assert_eq!(cx.update(|cx| preview.read(cx).busy()), None);
}

#[gpui_kit::test]
fn word_import_into_an_existing_template_asks_before_replacing(cx: &mut TestAppContext) {
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
    fixture.go(cx, Route::Template(template));
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
    let before = cx.update(|cx| {
        page.read(cx)
            .editor()
            .read(cx)
            .snapshot()
            .0
            .projections()
            .plain_text
    });
    let name_before = cx.update(|cx| page.read(cx).title_input().read(cx).text().to_owned());
    let docx = template_docx(BAIL_DOCX);

    fixture.press(cx, "secondary-o");
    let chosen = docx.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![chosen]));
    fixture.settle(cx);
    assert!(fixture.with(cx, |window, cx| window.has_active_dialog(cx)));
    fixture.click(cx, "cancel");
    assert_eq!(
        cx.update(|cx| page
            .read(cx)
            .editor()
            .read(cx)
            .snapshot()
            .0
            .projections()
            .plain_text),
        before
    );

    fixture.press(cx, "secondary-o");
    let chosen = docx.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![chosen]));
    fixture.settle(cx);
    fixture.click(cx, "import-replace");
    let (after, dirty, name) = cx.update(|cx| {
        let page = page.read(cx);
        (
            page.editor().read(cx).snapshot().0.projections().plain_text,
            page.is_dirty(cx),
            page.title_input().read(cx).text().to_owned(),
        )
    });
    assert_ne!(after, before);
    assert!(dirty);
    assert_eq!(name, name_before);
    let stored = Store::open(&fixture.database)
        .unwrap()
        .template(template)
        .unwrap();
    assert_eq!(stored.document.projections().plain_text, before);
}

#[gpui_kit::test]
fn word_import_reports_a_corrupt_file(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    fixture.go(cx, Route::NewTemplate);
    let broken = fixture._directory.path().join("broken.docx");
    std::fs::write(&broken, b"not a zip").unwrap();
    fixture.press(cx, "secondary-o");
    cx.simulate_path_prompt_response(move |_| Some(vec![broken]));
    fixture.settle(cx);
    let toasts = fixture.toasts(cx);
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0].0, ToastKind::Error);
    assert_eq!(
        toasts[0].1.as_deref(),
        Some("Could not import Word document")
    );
    assert!(!toasts[0].2.is_empty());
}
