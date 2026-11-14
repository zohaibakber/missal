use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::component::v_flex;
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
use crate::state::{Models, Route};
use crate::theme::{self, Palette};
use crate::ui::cheatsheet;
use crate::ui::components::ComponentsScreen;
use crate::ui::nav_menu::{self, NavMenu};
use crate::ui::palette::CommandPalette;
use crate::ui::print_preview::{PrintEvent, PrintPreview, PrintRequest, PrintSource};
use crate::ui::titlebar::{Header, HeaderContent, HeaderSource};
use crate::ui::toast::{ToastHost, ToastHostHandle};

pub struct ShellHandle(pub WeakEntity<AppShell>);

impl Global for ShellHandle {}

impl ShellHandle {
    pub fn get(cx: &App) -> Option<Entity<AppShell>> {
        cx.try_global::<ShellHandle>()
            .and_then(|handle| handle.0.upgrade())
    }
}

pub struct AppPaths {
    pub directory: PathBuf,
    pub is_default: bool,
}

impl Global for AppPaths {}

enum Screen {
    Firs(Entity<FirListScreen>),
    NewFir(Entity<NewFirScreen>),
    Workspace(Entity<WorkspaceScreen>),
    Settings(Entity<SettingsScreen>),
    Templates(Entity<TemplatesScreen>),
    Components(Entity<ComponentsScreen>),
    Print(Entity<PrintPreview>),
}

fn header_of<V: HeaderSource + Render>(
    view: &Entity<V>,
    window: &mut Window,
    cx: &mut App,
) -> HeaderContent {
    view.update(cx, |view, cx| view.header(window, cx))
}

impl Screen {
    fn view(&self) -> AnyView {
        match self {
            Screen::Firs(view) => view.clone().into(),
            Screen::NewFir(view) => view.clone().into(),
            Screen::Workspace(view) => view.clone().into(),
            Screen::Settings(view) => view.clone().into(),
            Screen::Templates(view) => view.clone().into(),
            Screen::Components(view) => view.clone().into(),
            Screen::Print(view) => view.clone().into(),
        }
    }

    fn header(&self, window: &mut Window, cx: &mut App) -> HeaderContent {
        match self {
            Screen::Firs(view) => header_of(view, window, cx),
            Screen::NewFir(view) => header_of(view, window, cx),
            Screen::Workspace(view) => header_of(view, window, cx),
            Screen::Settings(view) => header_of(view, window, cx),
            Screen::Templates(view) => header_of(view, window, cx),
            Screen::Components(view) => header_of(view, window, cx),
            Screen::Print(view) => header_of(view, window, cx),
        }
    }

    fn observe(&self, cx: &mut Context<AppShell>) -> Subscription {
        match self {
            Screen::Firs(view) => cx.observe(view, |_, _, cx| cx.notify()),
            Screen::NewFir(view) => cx.observe(view, |_, _, cx| cx.notify()),
            Screen::Workspace(view) => cx.observe(view, |_, _, cx| cx.notify()),
            Screen::Settings(view) => cx.observe(view, |_, _, cx| cx.notify()),
            Screen::Templates(view) => cx.observe(view, |_, _, cx| cx.notify()),
            Screen::Components(view) => cx.observe(view, |_, _, cx| cx.notify()),
            Screen::Print(view) => cx.observe(view, |_, _, cx| cx.notify()),
        }
    }

    fn guard(&self) -> crate::state::nav::Guard {
        match self {
            Screen::Firs(_) | Screen::Components(_) | Screen::Print(_) => Rc::new(|_| false),
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
            Screen::Components(view) => view.update(cx, |screen, cx| screen.focus(window, cx)),
            Screen::Print(view) => view.update(cx, |screen, cx| screen.focus(window, cx)),
        }
    }
}

pub struct AppShell {
    models: Models,
    focus: FocusHandle,
    screen: Screen,
    route: Route,
    palette: Option<Entity<CommandPalette>>,
    nav_menu: Option<Entity<NavMenu>>,
    parked: Option<(Route, Screen)>,
    print_subscription: Option<Subscription>,
    toasts: Entity<ToastHost>,
    pending_import: bool,
    screen_subscription: Subscription,
    _subscriptions: Vec<Subscription>,
}

impl AppShell {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> AppShell {
        let focus = cx.focus_handle();
        let route = models.nav.read(cx).current();
        let screen = Self::build_screen(&models, route, window, cx);
        let screen_subscription = screen.observe(cx);
        let toasts = cx.new(|_| ToastHost::new());
        cx.set_global(ToastHostHandle(toasts.downgrade()));
        let subscriptions = vec![
            cx.subscribe_in(&models.nav, window, |this, _, _: &NavEvent, window, cx| {
                this.route_changed(window, cx)
            }),
            cx.observe(&models.nav, |_, _, cx| cx.notify()),
            cx.observe(&models.firs, |_, _, cx| cx.notify()),
            cx.observe(&models.templates, |_, _, cx| cx.notify()),
            cx.observe_window_bounds(window, |this, window, cx| this.remember_bounds(window, cx)),
            cx.observe_window_appearance(window, |this, window, cx| {
                let choice = this.models.prefs.read(cx).read().theme;
                theme::apply(choice, window, cx);
            }),
            cx.observe_window_activation(window, |_, window, cx| {
                if window.is_window_active() {
                    gpui_kit::base::apply_system_reduce_motion(cx);
                }
                cx.notify();
            }),
        ];
        let mut shell = AppShell {
            models,
            focus,
            screen,
            route,
            palette: None,
            nav_menu: None,
            parked: None,
            print_subscription: None,
            toasts,
            pending_import: false,
            screen_subscription,
            _subscriptions: subscriptions,
        };
        shell.wire_print(window, cx);
        shell.record_recent(route, cx);
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
        self.models.prefs.update(cx, |prefs, cx| {
            prefs.update(cx, |p| p.window = Some(remembered))
        });
        cx.notify();
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

    pub fn components(&self) -> Option<Entity<ComponentsScreen>> {
        match &self.screen {
            Screen::Components(view) => Some(view.clone()),
            _ => None,
        }
    }

    pub fn toasts(&self) -> &Entity<ToastHost> {
        &self.toasts
    }

    pub fn print_preview(&self) -> Option<Entity<PrintPreview>> {
        match &self.screen {
            Screen::Print(view) => Some(view.clone()),
            _ => None,
        }
    }

    pub fn show_print_preview(
        &mut self,
        request: PrintRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.screen, Screen::Print(_)) {
            return;
        }
        self.palette = None;
        self.nav_menu = None;
        let target = match (&request.values, self.route) {
            (Some(values), _) => Some(crate::state::PrintTarget::Fir(values.fir.id)),
            (None, Route::Template(id)) => Some(crate::state::PrintTarget::Template(id)),
            _ => None,
        };
        let live = match &self.screen {
            Screen::Workspace(view) => view.read(cx).active(),
            _ => None,
        };
        let preview = cx.new(|cx| {
            PrintPreview::from_source(
                PrintSource::Request {
                    request: Box::new(request),
                    target,
                    live,
                },
                window,
                cx,
            )
        });
        let previous = std::mem::replace(&mut self.screen, Screen::Print(preview));
        self.parked = Some((self.route, previous));
        self.screen_subscription = self.screen.observe(cx);
        self.wire_print(window, cx);
        match target {
            Some(target) => self.models.nav.update(cx, |nav, cx| {
                nav.commit(crate::state::nav::Request::Push(Route::Print(target)), cx)
            }),
            None => self.activate(window, cx),
        }
        cx.notify();
    }

    fn wire_print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.print_subscription = match &self.screen {
            Screen::Print(view) => Some(cx.subscribe_in(
                view,
                window,
                |this, _, event: &PrintEvent, window, cx| match event {
                    PrintEvent::Close => this.leave_print(window, cx),
                    PrintEvent::FillIn(fir) => {
                        let back = this
                            .parked
                            .as_ref()
                            .is_some_and(|(route, _)| *route == Route::Fir(*fir));
                        if back {
                            this.leave_print(window, cx);
                        } else {
                            navigate(Route::Fir(*fir), window, cx);
                        }
                    }
                },
            )),
            _ => None,
        };
    }

    fn print_guard(&self) -> Option<crate::state::nav::Guard> {
        match (&self.screen, &self.parked) {
            (Screen::Print(_), Some((_, parked))) => Some(parked.guard()),
            _ => None,
        }
    }

    fn leave_print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.screen, Screen::Print(_)) {
            return;
        }
        let parked_route = self.parked.as_ref().map(|(route, _)| *route);
        if matches!(self.route, Route::Print(_)) {
            let previous = {
                let nav = self.models.nav.read(cx);
                let (entries, index) = nav.history();
                index.checked_sub(1).map(|index| entries[index])
            };
            if previous.is_some() && previous == parked_route {
                self.models.nav.update(cx, |nav, cx| {
                    nav.commit(crate::state::nav::Request::Back, cx)
                });
            } else if previous.is_some() {
                self.models.nav.update(cx, |nav, cx| nav.back(window, cx));
            } else {
                navigate(Route::Firs, window, cx);
            }
            return;
        }
        if let Some((_, screen)) = self.parked.take() {
            self.screen = screen;
            self.screen_subscription = self.screen.observe(cx);
            self.print_subscription = None;
            self.activate(window, cx);
            cx.notify();
        }
    }

    pub fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.screen, Screen::Print(_)) {
            self.leave_print(window, cx);
        } else {
            self.models.nav.update(cx, |nav, cx| nav.back(window, cx));
        }
    }

    fn print_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.screen {
            Screen::Print(view) => view.update(cx, |preview, cx| preview.print(window, cx)),
            Screen::Firs(view) => view.update(cx, |list, cx| list.print_focused(window, cx)),
            Screen::Workspace(view) => {
                view.update(cx, |screen, cx| screen.open_print_preview(window, cx))
            }
            Screen::Templates(view) => {
                if let Some(page) = view.read(cx).page().cloned() {
                    page.update(cx, |page, cx| page.open_print_preview(window, cx));
                }
            }
            _ => {}
        }
    }

    fn save_pdf_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Screen::Print(preview) = &self.screen {
            preview.update(cx, |preview, cx| preview.export_pdf(window, cx));
        }
    }

    pub fn import_word(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.nav_menu = None;
        self.palette = None;
        if let Screen::Templates(view) = &self.screen
            && matches!(self.route, Route::Template(_) | Route::NewTemplate)
            && let Some(page) = view.read(cx).page().cloned()
        {
            page.update(cx, |page, cx| page.request_import(window, cx));
            return;
        }
        self.pending_import = true;
        navigate(Route::NewTemplate, window, cx);
    }

    fn run_pending_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.pending_import) || self.route != Route::NewTemplate {
            return;
        }
        if let Screen::Templates(view) = &self.screen
            && let Some(page) = view.read(cx).page().cloned()
        {
            window.defer(cx, move |window, cx| {
                page.update(cx, |page, cx| page.request_import(window, cx));
            });
        }
    }

    pub fn palette_entity(&self) -> Option<Entity<CommandPalette>> {
        self.palette.clone()
    }

    pub fn nav_menu(&self) -> Option<Entity<NavMenu>> {
        self.nav_menu.clone()
    }

    pub fn current_route(&self) -> Route {
        self.route
    }

    fn build_screen(
        models: &Models,
        route: Route,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Screen {
        match route {
            Route::Firs => {
                Screen::Firs(cx.new(|cx| FirListScreen::new(models.clone(), window, cx)))
            }
            Route::NewFir => {
                Screen::NewFir(cx.new(|cx| NewFirScreen::new(models.clone(), window, cx)))
            }
            Route::Fir(id) => {
                Screen::Workspace(cx.new(|cx| WorkspaceScreen::new(models.clone(), id, window, cx)))
            }
            Route::EditFir(id) => {
                Screen::NewFir(cx.new(|cx| NewFirScreen::edit(models.clone(), id, window, cx)))
            }
            Route::Print(target) => Screen::Print(
                cx.new(|cx| PrintPreview::from_source(PrintSource::Target(target), window, cx)),
            ),
            Route::Settings => {
                Screen::Settings(cx.new(|cx| SettingsScreen::new(models.clone(), window, cx)))
            }
            Route::Templates | Route::NewTemplate | Route::Template(_) => Screen::Templates(
                cx.new(|cx| TemplatesScreen::new(models.clone(), route, window, cx)),
            ),
            Route::Components => Screen::Components(cx.new(|cx| ComponentsScreen::new(window, cx))),
        }
    }

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let guard = self.print_guard().unwrap_or_else(|| self.screen.guard());
        self.models
            .nav
            .update(cx, |nav, _| nav.set_guard(Some(guard)));
        self.screen.focus(window, cx);
    }

    fn record_recent(&self, route: Route, cx: &mut Context<Self>) {
        if let Route::Fir(id) = route {
            let now = chrono::Utc::now().timestamp();
            self.models.prefs.update(cx, |prefs, cx| {
                prefs.update(cx, |p| p.record_recent(id, now))
            });
        }
    }

    fn route_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let route = self.models.nav.read(cx).current();
        if route == self.route {
            return;
        }
        self.palette = None;
        self.nav_menu = None;
        let entering_print = matches!(
            (&self.screen, route),
            (Screen::Print(view), Route::Print(target)) if view.read(cx).target() == Some(target)
        );
        let restoring = self
            .parked
            .as_ref()
            .is_some_and(|(parked, _)| *parked == route);
        if restoring && let Some((_, screen)) = self.parked.take() {
            self.screen = screen;
            self.screen_subscription = self.screen.observe(cx);
            self.print_subscription = None;
            self.route = route;
            self.record_recent(route, cx);
            self.activate(window, cx);
            cx.notify();
            return;
        }
        if entering_print {
            self.route = route;
            self.activate(window, cx);
            cx.notify();
            return;
        }
        self.parked = None;
        let stays_in_templates = matches!(
            (&self.screen, route),
            (
                Screen::Templates(_),
                Route::Templates | Route::NewTemplate | Route::Template(_)
            )
        );
        if stays_in_templates {
            if let Screen::Templates(view) = &self.screen {
                view.update(cx, |screen, cx| screen.route_changed(route, window, cx));
            }
        } else {
            self.screen = Self::build_screen(&self.models, route, window, cx);
            self.screen_subscription = self.screen.observe(cx);
            self.wire_print(window, cx);
        }
        self.route = route;
        self.record_recent(route, cx);
        self.activate(window, cx);
        self.run_pending_import(window, cx);
        cx.notify();
    }

    pub fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.nav_menu = None;
        if self.palette.take().is_some() {
            self.screen.focus(window, cx);
        } else {
            let palette = cx.new(|cx| CommandPalette::new(self.models.clone(), window, cx));
            cx.subscribe_in(
                &palette,
                window,
                |this, _, _: &crate::ui::palette::Dismissed, window, cx| {
                    this.palette = None;
                    this.screen.focus(window, cx);
                    cx.notify();
                },
            )
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

    pub fn toggle_nav_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.nav_menu.take().is_some() {
            self.screen.focus(window, cx);
            cx.notify();
            return;
        }
        self.palette = None;
        let menu = cx.new(NavMenu::new);
        cx.subscribe_in(
            &menu,
            window,
            |this, _, _: &nav_menu::Dismissed, window, cx| {
                if this.nav_menu.take().is_some() {
                    this.screen.focus(window, cx);
                }
                cx.notify();
            },
        )
        .detach();
        menu.update(cx, |menu, cx| menu.focus(window, cx));
        self.nav_menu = Some(menu);
        cx.notify();
    }
}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn with_shell(
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut AppShell, &mut Window, &mut Context<AppShell>) + 'static,
) {
    if let Some(handle) = cx
        .try_global::<ShellHandle>()
        .map(|handle| handle.0.clone())
    {
        handle.update(cx, |shell, cx| f(shell, window, cx)).ok();
    }
}

pub fn navigate(route: Route, window: &mut Window, cx: &mut App) {
    let nav = Models::get(cx).nav;
    window.defer(cx, move |window, cx| {
        nav.update(cx, |nav, cx| nav.go(route, window, cx))
    });
}

pub fn navigate_replace(route: Route, cx: &mut App) {
    let nav = Models::get(cx).nav;
    nav.update(cx, |nav, cx| nav.replace_unguarded(route, cx));
}

pub fn navigate_back(window: &mut Window, cx: &mut App) {
    window.defer(cx, move |window, cx| {
        with_shell(window, cx, |shell, window, cx| shell.back(window, cx));
    });
}

pub fn navigate_forward(window: &mut Window, cx: &mut App) {
    let nav = Models::get(cx).nav;
    window.defer(cx, move |window, cx| {
        nav.update(cx, |nav, cx| nav.forward(window, cx))
    });
}

pub fn open_palette(window: &mut Window, cx: &mut App) {
    with_shell(window, cx, |shell, window, cx| {
        if !shell.has_palette() {
            shell.toggle_palette(window, cx);
        }
    });
}

pub fn toggle_nav_menu(window: &mut Window, cx: &mut App) {
    with_shell(window, cx, |shell, window, cx| {
        shell.toggle_nav_menu(window, cx)
    });
}

pub fn import_word_file(window: &mut Window, cx: &mut App) {
    window.defer(cx, |window, cx| {
        with_shell(window, cx, |shell, window, cx| {
            shell.import_word(window, cx)
        });
    });
}

pub fn open_print_preview(request: PrintRequest, window: &mut Window, cx: &mut App) {
    if let Some(handle) = cx
        .try_global::<ShellHandle>()
        .map(|handle| handle.0.clone())
    {
        window.defer(cx, move |window, cx| {
            handle
                .update(cx, |shell, cx| {
                    shell.show_print_preview(request, window, cx)
                })
                .ok();
        });
    }
}

pub fn show_shortcuts(window: &mut Window, cx: &mut App) {
    cheatsheet::open(window, cx);
}

pub fn show_about(window: &mut Window, cx: &mut App) {
    let (folder, can_check) = match cx.try_global::<AppPaths>() {
        Some(paths) => (
            Some(paths.directory.display().to_string()),
            crate::update::controller::enabled(!paths.is_default),
        ),
        None => (None, false),
    };
    crate::ui::dialogs::about(window, cx, folder, can_check);
}

pub fn check_for_updates(window: &mut Window, cx: &mut App) {
    crate::update::controller::check_now(window, cx);
}

pub fn set_theme(choice: crate::prefs::ThemeChoice, window: &mut Window, cx: &mut App) {
    let prefs = Models::get(cx).prefs;
    prefs.update(cx, |prefs, cx| prefs.update(cx, |p| p.theme = choice));
    theme::apply(choice, window, cx);
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let nav = self.models.nav.clone();
        let (can_back, can_forward) = {
            let nav = nav.read(cx);
            (nav.can_go_back(), nav.can_go_forward())
        };
        let content = self.screen.header(window, cx);
        let header = Header::new(content, can_back, can_forward, self.nav_menu.is_some());
        v_flex()
            .id("app-shell")
            .key_context(SHELL)
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(palette.surface)
            .text_color(palette.ink)
            .font_family(missal_text::UI_FAMILY)
            .text_size(px(14.))
            .on_action(cx.listener(|this, _: &ToggleCommandPalette, window, cx| {
                this.toggle_palette(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &ToggleNavMenu, window, cx| this.toggle_nav_menu(window, cx)),
            )
            .on_action(cx.listener(|_, _: &ShowShortcuts, window, cx| show_shortcuts(window, cx)))
            .on_action(cx.listener(|_, _: &GoFirs, window, cx| navigate(Route::Firs, window, cx)))
            .on_action(
                cx.listener(|_, _: &GoTemplates, window, cx| {
                    navigate(Route::Templates, window, cx)
                }),
            )
            .on_action(
                cx.listener(|_, _: &GoSettings, window, cx| navigate(Route::Settings, window, cx)),
            )
            .on_action(cx.listener(|_, _: &NewFir, window, cx| navigate(Route::NewFir, window, cx)))
            .on_action(cx.listener(|_, _: &NewTemplate, window, cx| {
                navigate(Route::NewTemplate, window, cx)
            }))
            .on_action(cx.listener(|this, _: &HistoryBack, window, cx| this.back(window, cx)))
            .on_action({
                let nav = nav.clone();
                cx.listener(move |_, _: &HistoryForward, window, cx| {
                    nav.update(cx, |nav, cx| nav.forward(window, cx))
                })
            })
            .on_action(cx.listener(|this, _: &BlurFocus, window, cx| {
                if this.palette.is_some() {
                    this.close_palette(window, cx);
                } else if this.nav_menu.take().is_some() {
                    this.screen.focus(window, cx);
                    cx.notify();
                } else {
                    window.focus(&this.focus, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Print, window, cx| this.print_action(window, cx)))
            .on_action(
                cx.listener(|this, _: &SavePdf, window, cx| this.save_pdf_action(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ImportWord, window, cx| this.import_word(window, cx)))
            .child(header)
            .child(
                div()
                    .id("content")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .bg(palette.surface)
                    .child(self.screen.view()),
            )
            .children(self.nav_menu.clone().map(|menu| menu.into_any_element()))
            .children(
                self.palette
                    .clone()
                    .map(|palette| palette.into_any_element()),
            )
            .child(self.toasts.clone())
    }
}
