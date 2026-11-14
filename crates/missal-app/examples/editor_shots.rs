use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, Bounds, Entity, HeadlessAppContext, WindowBounds, WindowOptions, point, px,
    size,
};
use missal_app::app::{AppShell, navigate};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::{Preferences, ThemeChoice};
use missal_app::seed::seed_firs;
use missal_app::state::{Models, Route};
use missal_core::document::Inline;
use missal_core::{FirId, TemplateId};
use missal_editor::model::selection::pos;
use missal_editor::model::tree;
use missal_editor::{Editor, Selection};
use missal_page::BlockPath;
use missal_store::Store;

const DEFAULT_OUT: &str =
    "/tmp/claude-0/-home-user-missal/da28de75-7983-5932-bdf9-e2e3d5312f4e/scratchpad/editor-shots";

struct Harness {
    cx: HeadlessAppContext,
    handle: AnyWindowHandle,
    shell: Entity<AppShell>,
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

    fn template_editor(&mut self) -> Entity<Editor> {
        let shell = self.shell.clone();
        self.cx.update(|cx| {
            let page = shell
                .read(cx)
                .templates()
                .unwrap()
                .read(cx)
                .page()
                .cloned()
                .unwrap();
            page.read(cx).editor().editor_entity().unwrap()
        })
    }

    fn workspace_editor(&mut self) -> Entity<Editor> {
        let shell = self.shell.clone();
        self.cx.update(|cx| {
            shell
                .read(cx)
                .workspace()
                .unwrap()
                .read(cx)
                .editor()
                .editor_entity()
                .unwrap()
        })
    }

    fn select(&mut self, editor: &Entity<Editor>, selection: Selection) {
        let editor = editor.clone();
        self.with(move |window, cx| {
            editor.update(cx, |e, cx| {
                e.focus(window, cx);
                e.set_selection(selection, window, cx);
            })
        });
    }
}

fn first_field_block(editor: &Editor) -> Option<BlockPath> {
    tree::all_text_paths(editor.document())
        .into_iter()
        .find(|path| {
            editor
                .core()
                .inlines_at(path)
                .is_some_and(|inlines| inlines.iter().any(|i| matches!(i, Inline::Field(_))))
        })
}

fn run(dark: bool, out: &Path) {
    let directory = tempfile::tempdir().expect("tempdir");
    let database = directory.path().join("missal.db");
    seed_firs(&database, 12).expect("seed");
    let store = Store::open(&database).expect("open");
    let largest = store
        .templates()
        .expect("templates")
        .into_iter()
        .max_by_key(|t| {
            store
                .template(t.id)
                .map(|r| r.document.encode().len())
                .unwrap_or(0)
        })
        .map(|t| t.id)
        .unwrap_or(TemplateId::new(1).unwrap());
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
    let (handle, shell) = cx.update(|cx| {
        let models = init_app(cx, Db::inline(store), None, preferences);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(1280.), px(860.)),
            })),
            ..gpui_kit::component::TitleBar::window_options()
        };
        let (handle, shell) = open_shell(&models, options, cx).expect("window");
        let _ = Models::get(cx);
        (handle, shell)
    });
    let suffix = if dark { "dark" } else { "light" };
    let mut h = Harness {
        cx,
        handle,
        shell,
        out: out.to_owned(),
        suffix,
    };
    h.settle();

    h.go(Route::Template(largest));
    h.shot("01-template-page");

    let editor = h.template_editor();
    if let Some(path) = h.cx.update(|cx| first_field_block(editor.read(cx))) {
        h.select(&editor, Selection::caret(pos(path, 0)));
        h.shot("02-template-fields-labels");
    }

    let start =
        h.cx.update(|cx| tree::first_text(editor.read(cx).document()).unwrap());
    h.select(&editor, Selection::caret(pos(start.clone(), 0)));
    let mixed = "دفعہ 380 PPC اور 411 کے تحت مقدمہ درج ہوا ";
    {
        let editor = editor.clone();
        h.with(move |window, cx| {
            editor.update(cx, |e, cx| e.insert_text_command(mixed, window, cx))
        });
    }
    let a = pos(start.clone(), "دفعہ 3".len());
    let b = pos(start.clone(), "دفعہ 380 PPC اور 4".len());
    h.select(&editor, Selection::new(a, b));
    h.shot("03-selection-rtl-ltr");

    let cell = h.cx.update(|cx| {
        let e = editor.read(cx);
        tree::all_text_paths(e.document())
            .into_iter()
            .find(|p| tree::cell_of(e.document(), p).is_some())
    });
    if let Some(cell) = cell {
        let offset =
            h.cx.update(|cx| editor.read(cx).core().block_len(&cell).unwrap_or(0));
        h.select(&editor, Selection::caret(pos(cell, offset)));
        h.shot("04-caret-in-table-cell");
    }

    h.with(|window, cx| window.close_all_dialogs(cx));
    let editor_dirty = h.cx.update(|cx| editor.read(cx).is_dirty());
    if editor_dirty {
        let editor = editor.clone();
        h.with(move |_, cx| editor.update(cx, |e, cx| e.discard(cx)));
    }

    h.go(Route::Fir(FirId::new(3).expect("id")));
    let editor = h.workspace_editor();
    if let Some(path) = h.cx.update(|cx| first_field_block(editor.read(cx))) {
        h.select(&editor, Selection::caret(pos(path, 0)));
    }
    h.shot("05-workspace-values");
    h.with(|window, cx| window.press("secondary-shift-l", cx));
    h.shot("06-workspace-labels");
    h.with(|window, cx| window.press("secondary-shift-l", cx));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .position(|a| a == "--out")
        .map(|i| PathBuf::from(&args[i + 1]))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    std::fs::create_dir_all(&out).expect("output directory");
    for dark in [false, true] {
        run(dark, &out);
    }
}
