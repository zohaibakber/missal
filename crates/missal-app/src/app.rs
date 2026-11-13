use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AnyView, App, AppContext, Context, Entity, FocusHandle, Focusable, Global, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Subscription, WeakEntity, Window, div, px,
};

use crate::screens::fir_form::NewFirScreen;
use crate::screens::fir_list::FirListScreen;
use crate::screens::settings::SettingsScreen;
use crate::screens::templates::TemplatesScreen;
use crate::screens::workspace::WorkspaceScreen;
use crate::shortcuts::*;
use crate::state::nav::NavEvent;
use crate::state::{Models, Route, toast_info};
use crate::theme::{self, Palette};
use crate::ui::cheatsheet;
use crate::ui::palette::CommandPalette;
use crate::ui::sidebar::SidebarView;
use crate::ui::titlebar::TitleBarView;

pub struct ShellHandle(pub WeakEntity<AppShell>);

impl Global for ShellHandle {}

enum Screen {
    Firs(Entity<FirListScreen>),
    NewFir(Entity<NewFirScreen>),
    Workspace(Entity<WorkspaceScreen>),
    Settings(Entity<SettingsScreen>),
    Templates(Entity<TemplatesScreen>),
}

impl Screen {
    fn view(&self) -> AnyView {
        match self {
            Screen::Firs(view) => view.clone().into(),
            Screen::NewFir(view) => view.clone().into(),
            Screen::Workspace(view) => view.clone().into(),
            Screen::Settings(view) => view.clone().into(),
            Screen::Templates(view) => view.clone().into(),
        }
    }

    fn guard(&self) -> crate::state::nav::Guard {
        match self {
            Screen::Firs(_) => Rc::new(|_| false),
            Screen::NewFir(view) => {
                let view = view.clone();
                Rc::new(move |cx| view.read(cx).is_dirty(cx))
            }
            Screen::Workspace(view) => {
                let view = view.clone();
                Rc::new(move |cx| view.read(cx).is_dirty(cx))
            }
            Screen::Settings(view) => {
                let view = view.clone();
                Rc::new(move |cx| view.read(cx).is_dirty(cx))
            }
            Screen::Templates(view) => {
                let view = view.clone();
                Rc::new(move |cx| view.read(cx).is_dirty(cx))
            }
        }
    }

    fn focus(&self, window: &mut Window, cx: &mut App) {
        match self {
            Screen::Firs(view) => view.update(cx, |screen, cx| screen.focus(window, cx)),
            Screen::NewFir(view) => view.update(cx, |screen, cx| screen.focus(window, cx)),
            Screen::Workspace(view) => view.update(cx, |screen, cx| screen.focus(window, cx)),
            Screen::Settings(view) => view.update(cx, |screen, cx| screen.focus(window, cx)),
            Screen::Templates(view) => view.update(cx, |screen, cx| screen.focus(window, cx)),
        }
    }
}

pub struct AppShell {
    models: Models,
    focus: FocusHandle,
    titlebar: Entity<TitleBarView>,
    sidebar: Entity<SidebarView>,
    screen: Screen,
    route: Route,
    palette: Option<Entity<CommandPalette>>,
    _subscriptions: Vec<Subscription>,
}

impl AppShell {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> AppShell {
        let focus = cx.focus_handle();
        let titlebar = cx.new(|cx| TitleBarView::new(models.clone(), cx));
        let sidebar = cx.new(|cx| SidebarView::new(models.clone(), cx));
        let route = models.nav.read(cx).current();
        let screen = Self::build_screen(&models, route, window, cx);
        let mut subscriptions = vec![cx.subscribe_in(
            &models.nav,
            window,
            |this, _, _: &NavEvent, window, cx| this.route_changed(window, cx),
        )];
        subscriptions.push(cx.observe_window_bounds(window, |this, window, cx| this.remember_bounds(window, cx)));
        subscriptions.push(cx.observe_window_appearance(window, |this, window, cx| {
            let choice = this.models.prefs.read(cx).read().theme;
            theme::apply(choice, window, cx);
        }));
        let mut shell = AppShell {
            models,
            focus,
            titlebar,
            sidebar,
            screen,
            route,
            palette: None,
            _subscriptions: subscriptions,
        };
        shell.activate(window, cx);
        shell
    }

    fn remember_bounds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (bounds, maximized) = match window.window_bounds() {
            gpui_kit::WindowBounds::Windowed(bounds) => (bounds, false),
            gpui_kit::WindowBounds::Maximized(bounds) => (bounds, true),
            gpui_kit::WindowBounds::Fullscreen(bounds) => (bounds, false),
        };
        let remembered = crate::prefs::WindowPrefs {
            x: bounds.origin.x.into(),
            y: bounds.origin.y.into(),
            width: bounds.size.width.into(),
            height: bounds.size.height.into(),
            maximized,
        };
        self.models.prefs.update(cx, |prefs, cx| prefs.update(cx, |p| p.window = Some(remembered)));
    }

    pub fn fir_list(&self) -> Option<Entity<FirListScreen>> {
        match &self.screen {
            Screen::Firs(view) => Some(view.clone()),
            _ => None,
        }
    }

    pub fn new_fir(&self) -> Option<Entity<NewFirScreen>> {
        match &self.screen {
            Screen::NewFir(view) => Some(view.clone()),
            _ => None,
        }
    }

    pub fn workspace(&self) -> Option<Entity<WorkspaceScreen>> {
        match &self.screen {
            Screen::Workspace(view) => Some(view.clone()),
            _ => None,
        }
    }

    pub fn settings(&self) -> Option<Entity<SettingsScreen>> {
        match &self.screen {
            Screen::Settings(view) => Some(view.clone()),
            _ => None,
        }
    }

    pub fn templates(&self) -> Option<Entity<TemplatesScreen>> {
        match &self.screen {
            Screen::Templates(view) => Some(view.clone()),
            _ => None,
        }
    }

    pub fn palette_entity(&self) -> Option<Entity<CommandPalette>> {
        self.palette.clone()
    }

    pub fn current_route(&self) -> Route {
        self.route
    }

    fn build_screen(models: &Models, route: Route, window: &mut Window, cx: &mut Context<Self>) -> Screen {
        match route {
            Route::Firs => Screen::Firs(cx.new(|cx| FirListScreen::new(models.clone(), window, cx))),
            Route::NewFir => Screen::NewFir(cx.new(|cx| NewFirScreen::new(models.clone(), window, cx))),
            Route::Fir(id) => Screen::Workspace(cx.new(|cx| WorkspaceScreen::new(models.clone(), id, window, cx))),
            Route::Settings => Screen::Settings(cx.new(|cx| SettingsScreen::new(models.clone(), window, cx))),
            Route::Templates | Route::NewTemplate | Route::Template(_) => {
                Screen::Templates(cx.new(|cx| TemplatesScreen::new(models.clone(), route, window, cx)))
            }
        }
    }

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let guard = self.screen.guard();
        self.models.nav.update(cx, |nav, _| nav.set_guard(Some(guard)));
        self.screen.focus(window, cx);
    }

    fn route_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let route = self.models.nav.read(cx).current();
        if route == self.route {
            return;
        }
        self.palette = None;
        let stays_in_templates = matches!(
            (&self.screen, route),
            (Screen::Templates(_), Route::Templates | Route::NewTemplate | Route::Template(_))
        );
        if stays_in_templates {
            if let Screen::Templates(view) = &self.screen {
                view.update(cx, |screen, cx| screen.route_changed(route, window, cx));
            }
        } else {
            self.screen = Self::build_screen(&self.models, route, window, cx);
        }
        self.route = route;
        self.activate(window, cx);
        cx.notify();
    }

    pub fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            self.screen.focus(window, cx);
        } else {
            let palette = cx.new(|cx| CommandPalette::new(self.models.clone(), window, cx));
            cx.subscribe_in(&palette, window, |this, _, _: &crate::ui::palette::Dismissed, window, cx| {
                this.palette = None;
                this.screen.focus(window, cx);
                cx.notify();
            })
            .detach();
            palette.update(cx, |palette, cx| palette.focus(window, cx));
            self.palette = Some(palette);
        }
        cx.notify();
    }

    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            self.screen.focus(window, cx);
            cx.notify();
        }
    }

    pub fn has_palette(&self) -> bool {
        self.palette.is_some()
    }
}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub fn navigate(route: Route, window: &mut Window, cx: &mut App) {
    let nav = Models::get(cx).nav;
    window.defer(cx, move |window, cx| nav.update(cx, |nav, cx| nav.go(route, window, cx)));
}

pub fn navigate_replace(route: Route, cx: &mut App) {
    let nav = Models::get(cx).nav;
    nav.update(cx, |nav, cx| nav.replace_unguarded(route, cx));
}

pub fn toggle_sidebar(_: &mut Window, cx: &mut App) {
    let prefs = Models::get(cx).prefs;
    prefs.update(cx, |prefs, cx| prefs.update(cx, |p| p.sidebar_open = !p.sidebar_open));
}

pub fn open_palette(window: &mut Window, cx: &mut App) {
    if let Some(handle) = cx.try_global::<ShellHandle>().map(|handle| handle.0.clone()) {
        handle
            .update(cx, |shell, cx| {
                if !shell.has_palette() {
                    shell.toggle_palette(window, cx);
                }
            })
            .ok();
    }
}

pub fn show_shortcuts(window: &mut Window, cx: &mut App) {
    cheatsheet::open(window, cx);
}

pub fn set_theme(choice: crate::prefs::ThemeChoice, window: &mut Window, cx: &mut App) {
    let prefs = Models::get(cx).prefs;
    prefs.update(cx, |prefs, cx| prefs.update(cx, |p| p.theme = choice));
    theme::apply(choice, window, cx);
}

impl Render for AppShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let nav = self.models.nav.clone();
        let sidebar_open = self.models.prefs.read(cx).read().sidebar_open;
        let _ = sidebar_open;
        v_flex()
            .id("app-shell")
            .key_context(SHELL)
            .track_focus(&self.focus)
            .size_full()
            .bg(palette.sidebar)
            .text_color(palette.foreground)
            .font_family(missal_text::UI_FAMILY)
            .text_size(px(14.))
            .on_action(cx.listener(|this, _: &ToggleCommandPalette, window, cx| this.toggle_palette(window, cx)))
            .on_action(cx.listener(|_, _: &ShowShortcuts, window, cx| show_shortcuts(window, cx)))
            .on_action(cx.listener(|_, _: &ToggleSidebar, window, cx| toggle_sidebar(window, cx)))
            .on_action(cx.listener(|_, _: &GoFirs, window, cx| navigate(Route::Firs, window, cx)))
            .on_action(cx.listener(|_, _: &GoTemplates, window, cx| navigate(Route::Templates, window, cx)))
            .on_action(cx.listener(|_, _: &GoSettings, window, cx| navigate(Route::Settings, window, cx)))
            .on_action(cx.listener(|_, _: &NewFir, window, cx| navigate(Route::NewFir, window, cx)))
            .on_action(cx.listener(|_, _: &NewTemplate, window, cx| navigate(Route::NewTemplate, window, cx)))
            .on_action({
                let nav = nav.clone();
                cx.listener(move |_, _: &HistoryBack, window, cx| nav.update(cx, |nav, cx| nav.back(window, cx)))
            })
            .on_action({
                let nav = nav.clone();
                cx.listener(move |_, _: &HistoryForward, window, cx| nav.update(cx, |nav, cx| nav.forward(window, cx)))
            })
            .on_action(cx.listener(|this, _: &BlurFocus, window, cx| {
                if this.palette.is_some() {
                    this.close_palette(window, cx);
                } else {
                    window.focus(&this.focus, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &Print, window, cx| {
                toast_info(window, cx, "Print preview is not available yet");
            }))
            .on_action(cx.listener(|_, _: &ImportWord, window, cx| {
                toast_info(window, cx, "Word import is not available yet");
            }))
            .child(self.titlebar.clone())
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .id("content")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .pr_2()
                            .pb_2()
                            .child(
                                div()
                                    .size_full()
                                    .overflow_hidden()
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(palette.border)
                                    .bg(palette.background)
                                    .child(self.screen.view()),
                            ),
                    ),
            )
            .children(self.palette.clone().map(|palette| palette.into_any_element()))
    }
}
