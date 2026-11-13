use gpui_kit::component::TitleBar;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, Size, WindowBounds, WindowOptions, point, px, size,
};
use missal_text::TextSystem;

use crate::app::{AppShell, ShellHandle};
use crate::db::Db;
use crate::paths::Paths;
use crate::prefs::{DEFAULT_WINDOW, MIN_WINDOW, Preferences};
use crate::shortcuts;
use crate::state::Models;
use crate::theme;

pub fn init_app(cx: &mut App, db: Db, paths: Option<&Paths>, preferences: Preferences) -> Models {
    gpui_kit::init(cx);
    missal_text::init(cx);
    if let Err(problems) = TextSystem::check_font_mapping(cx) {
        eprintln!("missal: font mapping check failed:\n{problems}");
    }
    shortcuts::bind_keys(cx);
    theme::install(theme::resolve_dark(preferences.theme, cx.window_appearance()), cx);
    Models::install(db, paths, preferences, cx)
}

pub fn window_options(preferences: &Preferences, cx: &App) -> WindowOptions {
    let bounds = match preferences.window {
        Some(window) => {
            let bounds = Bounds {
                origin: point(px(window.x), px(window.y)),
                size: size(px(window.width), px(window.height)),
            };
            if window.maximized { WindowBounds::Maximized(bounds) } else { WindowBounds::Windowed(bounds) }
        }
        None => WindowBounds::Windowed(Bounds::centered(None, size(px(DEFAULT_WINDOW.0), px(DEFAULT_WINDOW.1)), cx)),
    };
    WindowOptions {
        window_bounds: Some(bounds),
        window_min_size: Some(Size {
            width: px(MIN_WINDOW.0),
            height: px(MIN_WINDOW.1),
        }),
        ..TitleBar::window_options()
    }
}

pub fn open_shell(
    models: &Models,
    options: WindowOptions,
    cx: &mut App,
) -> gpui_kit::Result<(AnyWindowHandle, Entity<AppShell>)> {
    let preferences = models.prefs.read(cx).read().clone();
    let models = models.clone();
    gpui_kit::open_window(options, cx, move |window, cx| {
        theme::apply(preferences.theme, window, cx);
        let shell = cx.new(|cx| AppShell::new(models, window, cx));
        cx.set_global(ShellHandle(shell.downgrade()));
        shell
    })
}
