use gpui_kit::component::TitleBar;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};
#[cfg(debug_assertions)]
use missal_text::TextSystem;

use crate::app::{AppShell, ShellHandle};
use crate::db::Db;
use crate::paths::Paths;
use crate::prefs::{DEFAULT_WINDOW, MIN_WINDOW, Preferences};
use crate::shortcuts;
use crate::state::Models;
use crate::theme;
use crate::ui::window_controls::MAC_TRAFFIC_LIGHT_POSITION;

pub fn init_app(cx: &mut App, db: Db, paths: Option<&Paths>, preferences: Preferences) -> Models {
    gpui_kit::init(cx);
    missal_text::init(cx);
    #[cfg(debug_assertions)]
    if let Err(problems) = TextSystem::check_font_mapping(cx) {
        eprintln!("missal: font mapping check failed:\n{problems}");
    }
    shortcuts::bind_keys(cx);
    crate::ui::autocomplete::bind_keys(cx);
    crate::editor::install(cx);
    theme::install(
        theme::resolve_dark(preferences.theme, cx.window_appearance()),
        cx,
    );
    if let Some(paths) = paths {
        cx.set_global(crate::app::AppPaths {
            directory: paths.directory.clone(),
            is_default: paths.is_default,
        });
        crate::auth::AuthModel::install(Some(&paths.directory), cx);
    }
    let models = Models::install(db, paths, preferences, cx);
    crate::library::LibraryModel::install(&models, cx);
    models
}

/// Window options that fit the screen the window opens on.
///
/// Also sets the interface zoom for that screen before the window exists, so
/// the first frame is already drawn at the right size.
pub fn window_options(preferences: &Preferences, cx: &App) -> WindowOptions {
    let saved = preferences.window.map(|window| Bounds {
        origin: point(px(window.x), px(window.y)),
        size: size(px(window.width), px(window.height)),
    });
    let display = saved
        .and_then(|saved| {
            cx.displays()
                .into_iter()
                .find(|display| display.bounds().contains(&saved.center()))
        })
        .or_else(|| cx.primary_display());
    let visible = display.as_ref().map(|display| display.visible_bounds());
    let zoom = crate::zoom::resolve(
        preferences.interface_size,
        visible.map(|visible| visible.size),
    );
    crate::zoom::set(zoom);
    let room = visible.map_or(size(px(f32::MAX), px(f32::MAX)), |visible| visible.size);
    let min = size(
        px(MIN_WINDOW.0 * zoom).min(room.width),
        px(MIN_WINDOW.1 * zoom).min(room.height),
    );
    let bounds = match (preferences.window, saved) {
        (Some(window), Some(saved)) => {
            let bounds = Bounds {
                origin: saved.origin,
                size: size(
                    saved.size.width.min(room.width),
                    saved.size.height.min(room.height),
                ),
            };
            if window.maximized {
                WindowBounds::Maximized(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            }
        }
        _ => {
            let wanted = size(px(DEFAULT_WINDOW.0 * zoom), px(DEFAULT_WINDOW.1 * zoom));
            let fitted = size(
                wanted.width.min(room.width - px(WINDOW_MARGIN)),
                wanted.height.min(room.height - px(WINDOW_MARGIN)),
            );
            let bounds = match visible {
                Some(visible) => Bounds::centered_at(visible.center(), fitted),
                None => Bounds::centered(None, fitted, cx),
            };
            // A screen with no room around the default window gets all of it.
            if fitted != wanted {
                WindowBounds::Maximized(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            }
        }
    };
    WindowOptions {
        window_bounds: Some(bounds),
        app_id: Some(crate::platform::APP_ID.to_owned()),
        window_min_size: Some(min),
        ..chrome_options()
    }
}

/// Room left around a new window so it never touches the screen's edges.
const WINDOW_MARGIN: f32 = 48.0;

pub fn chrome_options() -> WindowOptions {
    let (x, y) = MAC_TRAFFIC_LIGHT_POSITION;
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(crate::platform::PRODUCT_NAME.into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(x), px(y))),
        }),
        app_owns_titlebar_drag: true,
        window_decorations: Some(WindowDecorations::Client),
        ..TitleBar::window_options()
    }
}

pub fn open_shell(
    models: &Models,
    options: WindowOptions,
    cx: &mut App,
) -> gpui_kit::Result<(AnyWindowHandle, Entity<AppShell>)> {
    let theme_choice = models.prefs.read(cx).read().theme;
    let models = models.clone();
    gpui_kit::open_window(options, cx, move |window, cx| {
        theme::apply(theme_choice, window, cx);
        let shell = cx.new(|cx| AppShell::new(models, window, cx));
        cx.set_global(ShellHandle(shell.downgrade()));
        shell
    })
}
