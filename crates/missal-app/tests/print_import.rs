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
use missal_app::state::{Models, Route, Toast, ToastKind, ToastLog};
use missal_app::ui::print_preview::{PreviewDocument, PrintHandler, PrintPreview, Zoom};
use missal_core::FirId;
use missal_store::Store;

struct Fixture {
    handle: AnyWindowHandle,
    shell: Entity<AppShell>,
    models: Models,
    database: PathBuf,
    toasts: ToastLog,
    printed: Rc<std::cell::RefCell<Vec<PathBuf>>>,
    _directory: tempfile::TempDir,
}

fn boot(cx: &mut TestAppContext, count: usize) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("missal.sqlite");
    if count > 0 {
        seed_firs(&database, count).unwrap();
    }
    let store = Store::open(&database).unwrap();
    let toasts = ToastLog::default();
    let printed = Rc::new(std::cell::RefCell::new(Vec::new()));
    let (handle, shell, models) = cx.update(|cx| {
        cx.set_global(toasts.clone());
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
        toasts,
        printed,
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

    fn toasts(&self) -> Vec<Toast> {
        self.toasts.0.borrow().clone()
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
fn print_preview_lays_out_the_included_documents_and_navigates(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let workspace = fir_workspace(&fixture, cx);
    assert!(fixture.preview(cx).is_none());
    let preview = fixture.open_preview(cx);
    let (documents, pages, description) = cx.update(|cx| {
        let preview = preview.read(cx);
        (
            preview.document_count(),
            preview.page_count(),
            preview.description(),
        )
    });
    assert_eq!(documents, 3);
    assert!(pages >= 3, "{pages} pages for three documents");
    assert!(description.starts_with("FIR "), "{description}");
    assert!(
        description.ends_with(&format!("3 documents · {pages} pages")),
        "{description}"
    );
    assert_eq!(cx.update(|cx| preview.read(cx).current_page()), 1);

    fixture.press(cx, "pagedown");
    assert_eq!(cx.update(|cx| preview.read(cx).current_page()), 2);
    fixture.click(cx, "preview-next");
    assert_eq!(cx.update(|cx| preview.read(cx).current_page()), 3);
    fixture.click(cx, "preview-previous");
    assert_eq!(cx.update(|cx| preview.read(cx).current_page()), 2);
    fixture.press(cx, "pageup");
    fixture.press(cx, "pageup");
    assert_eq!(cx.update(|cx| preview.read(cx).current_page()), 1);
    fixture.with(cx, |_, cx| {
        preview.update(cx, |preview, cx| preview.go_to_page(pages + 10, cx))
    });
    assert_eq!(cx.update(|cx| preview.read(cx).current_page()), pages);

    fixture.press(cx, "escape");
    assert!(fixture.preview(cx).is_none());
    let _ = workspace;
}

#[gpui_kit::test]
fn print_preview_uses_the_live_editor_content_of_the_active_document(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let workspace = fir_workspace(&fixture, cx);
    let editor = cx.update(|cx| workspace.read(cx).editor().clone());
    let active = cx.update(|cx| workspace.read(cx).active().unwrap());
    fixture.with(cx, |window, cx| editor.focus(window, cx));
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
fn print_preview_respects_the_include_when_printing_choice(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let workspace = fir_workspace(&fixture, cx);
    let fir = cx.update(|cx| workspace.read(cx).id());
    let ids: Vec<_> = cx.update(|cx| {
        workspace
            .read(cx)
            .documents()
            .iter()
            .map(|d| d.id)
            .collect()
    });
    let preview = fixture.open_preview(cx);
    assert_eq!(cx.update(|cx| preview.read(cx).document_count()), 3);
    fixture.with(cx, |_, cx| {
        preview.update(cx, |preview, cx| preview.toggle_included(1, cx))
    });
    assert_eq!(cx.update(|cx| preview.read(cx).document_count()), 2);
    let excluded = cx.update(|cx| missal_app::ui::print_preview::excluded_documents(fir, cx));
    assert_eq!(excluded.into_iter().collect::<Vec<_>>(), [ids[1]]);
    fixture.press(cx, "escape");
    let preview = fixture.open_preview(cx);
    assert_eq!(cx.update(|cx| preview.read(cx).document_count()), 2);
}

#[gpui_kit::test]
fn print_preview_zoom_steps_fit_and_preserves_the_scroll_position(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fir_workspace(&fixture, cx);
    let preview = fixture.open_preview(cx);
    assert_eq!(cx.update(|cx| preview.read(cx).zoom()), Zoom::Fit);
    let fit = cx.update(|cx| preview.read(cx).zoom_percent());
    assert!((25..=100).contains(&fit));
    fixture.with(cx, |_, cx| {
        preview.update(cx, |preview, cx| preview.go_to_page(3, cx))
    });
    let before = cx.update(|cx| preview.read(cx).scroll_fraction());
    fixture.press(cx, "secondary-=");
    let zoom = cx.update(|cx| preview.read(cx).zoom());
    assert!(
        matches!(zoom, Zoom::Percent(percent) if percent > fit),
        "{zoom:?} from {fit}"
    );
    let after = cx.update(|cx| preview.read(cx).scroll_fraction());
    assert!((before - after).abs() < 0.03, "{before} vs {after}");
    fixture.press(cx, "secondary--");
    fixture.press(cx, "secondary--");
    let Zoom::Percent(smaller) = cx.update(|cx| preview.read(cx).zoom()) else {
        panic!("a fixed zoom")
    };
    assert!(smaller < fit.max(100));
    fixture.press(cx, "secondary-0");
    assert_eq!(cx.update(|cx| preview.read(cx).zoom()), Zoom::Fit);
    fixture.click(cx, "preview-zoom-in");
    assert!(matches!(
        cx.update(|cx| preview.read(cx).zoom()),
        Zoom::Percent(_)
    ));
    fixture.click(cx, "preview-zoom-out");
    for _ in 0..8 {
        fixture.click(cx, "preview-zoom-out");
    }
    assert_eq!(cx.update(|cx| preview.read(cx).zoom()), Zoom::Percent(50));
    for _ in 0..8 {
        fixture.click(cx, "preview-zoom-in");
    }
    assert_eq!(cx.update(|cx| preview.read(cx).zoom()), Zoom::Percent(200));
}

#[gpui_kit::test]
fn printing_from_the_workspace_offers_every_document_on_the_print_screen(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    let workspace = fir_workspace(&fixture, cx);
    let count = cx.update(|cx| workspace.read(cx).documents().len());
    fixture.click(cx, "header-print");
    let preview = fixture.preview(cx).expect("the Print screen opens");
    assert_eq!(
        cx.update(|cx| preview.read(cx).request().documents.len()),
        count
    );
    assert!(fixture.toasts().is_empty());
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
            .toasts()
            .iter()
            .any(|toast| toast.kind == ToastKind::Success
                && toast.message.contains("FIR export.pdf"))
    );
    assert!(fixture.preview(cx).is_some());
    assert_eq!(cx.update(|cx| preview.read(cx).busy()), None);
}

#[gpui_kit::test]
fn cancelling_the_save_dialog_writes_nothing(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fir_workspace(&fixture, cx);
    let preview = fixture.open_preview(cx);
    fixture.click(cx, "preview-export");
    cx.simulate_new_path_selection(|_| None);
    fixture.settle(cx);
    assert!(fixture.toasts().is_empty());
    assert_eq!(cx.update(|cx| preview.read(cx).busy()), None);
}

#[gpui_kit::test]
fn print_hands_a_spooled_pdf_to_the_system_and_closes_the_preview(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
    fir_workspace(&fixture, cx);
    let preview = fixture.open_preview(cx);
    let pages = cx.update(|cx| preview.read(cx).page_count());
    fixture.press(cx, "secondary-p");
    let printed = fixture.printed.borrow().clone();
    assert_eq!(printed.len(), 1);
    let bytes = std::fs::read(&printed[0]).expect("the spooled PDF");
    assert!(bytes.starts_with(b"%PDF-"));
    assert_eq!(count_pdf_pages(&bytes), pages);
    assert!(fixture.preview(cx).is_none());
    assert!(
        fixture
            .toasts()
            .iter()
            .any(|toast| toast.kind == ToastKind::Success)
    );
    std::fs::remove_file(&printed[0]).ok();
}

#[gpui_kit::test]
fn a_template_previews_with_placeholder_names(cx: &mut TestAppContext) {
    let fixture = boot(cx, 4);
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
    let preview = fixture.open_preview(cx);
    cx.update(|cx| {
        let preview = preview.read(cx);
        assert_eq!(preview.document_count(), 1);
        assert!(preview.page_count() >= 1);
        assert!(preview.request().values.is_none());
        assert!(
            preview
                .description()
                .contains("Placeholders show their names")
        );
    });
}

#[gpui_kit::test]
fn word_import_fills_a_new_template_through_the_file_picker(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    let existing = Store::open(&fixture.database)
        .unwrap()
        .templates()
        .unwrap()
        .len();
    fixture.go(cx, Route::NewTemplate);
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
    assert!(!cx.update(|cx| page.read(cx).is_dirty(cx)));
    fixture.press(cx, "secondary-o");
    assert!(cx.did_prompt_for_paths());
    let docx = template_docx(BAIL_DOCX);
    assert!(docx.exists());
    let chosen = docx.clone();
    cx.simulate_path_prompt_response(move |options| {
        assert!(options.files && !options.directories && !options.multiple);
        Some(vec![chosen])
    });
    fixture.settle(cx);
    assert!(!cx.update(|cx| page.read(cx).is_importing()));
    assert!(!fixture.with(cx, |window, cx| window.has_active_dialog(cx)));
    let (text, dirty, name) = cx.update(|cx| {
        let page = page.read(cx);
        (
            page.editor().snapshot(cx).document.projections().plain_text,
            page.is_dirty(cx),
            page.title_input().read(cx).text().to_owned(),
        )
    });
    assert!(!text.trim().is_empty());
    assert!(dirty);
    assert!(!name.trim().is_empty());
    assert!(cx.update(|cx| page.read(cx).is_reviewing()));
    assert!(fixture.toasts().is_empty());

    fixture.click(cx, "template-save");
    let saved = Store::open(&fixture.database).unwrap().templates().unwrap();
    assert_eq!(saved.len(), existing + 1);
    let created = saved
        .iter()
        .find(|template| template.name == name)
        .expect("the imported template is saved");
    assert!(created.preview_text.len() > 3);
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
            .snapshot(cx)
            .document
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
            .snapshot(cx)
            .document
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
            page.editor().snapshot(cx).document.projections().plain_text,
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
fn word_import_rejects_files_that_are_not_docx(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    fixture.go(cx, Route::NewTemplate);
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
    fixture.press(cx, "secondary-o");
    cx.simulate_path_prompt_response(|_| Some(vec![PathBuf::from("/tmp/notes.txt")]));
    fixture.settle(cx);
    assert!(!cx.update(|cx| page.read(cx).is_dirty(cx)));
    let toasts = fixture.toasts();
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0].kind, ToastKind::Error);
    assert_eq!(
        toasts[0].title.as_deref(),
        Some("Could not import Word document")
    );
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
    let toasts = fixture.toasts();
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0].kind, ToastKind::Error);
    assert_eq!(
        toasts[0].title.as_deref(),
        Some("Could not import Word document")
    );
    assert!(!toasts[0].message.is_empty());
}

#[gpui_kit::test]
fn the_upload_button_opens_the_same_picker(cx: &mut TestAppContext) {
    let fixture = boot(cx, 0);
    fixture.go(cx, Route::NewTemplate);
    fixture.click(cx, "template-import");
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| None);
    fixture.settle(cx);
    assert!(fixture.toasts().is_empty());
}
