use gpui_kit::AnimationExt;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Render, SharedString, Styled, Window, div, px,
};

use crate::app::{import_word_file, navigate, set_theme, show_about, show_shortcuts};
use crate::prefs::ThemeChoice;
use crate::state::{Models, Route};
use crate::theme::{HEADER_HEIGHT, MENU_ITEM, NAV_MENU_WIDTH, Palette, motion_fast};
use crate::ui::menu::{menu_item, menu_panel, menu_separator};

pub struct Dismissed;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavEntry {
    Firs,
    Templates,
    Settings,
    NewFir,
    ImportWord,
    Shortcuts,
    Theme,
    About,
}

impl NavEntry {
    pub const ALL: [NavEntry; 8] = [
        NavEntry::Firs,
        NavEntry::Templates,
        NavEntry::Settings,
        NavEntry::NewFir,
        NavEntry::ImportWord,
        NavEntry::Shortcuts,
        NavEntry::Theme,
        NavEntry::About,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            NavEntry::Firs => "FIRs",
            NavEntry::Templates => "Templates",
            NavEntry::Settings => "Settings",
            NavEntry::NewFir => "New FIR",
            NavEntry::ImportWord => "Import Word file",
            NavEntry::Shortcuts => "Keyboard shortcuts",
            NavEntry::Theme => "Theme",
            NavEntry::About => "About Missal",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            NavEntry::Firs => "nav-firs",
            NavEntry::Templates => "nav-templates",
            NavEntry::Settings => "nav-settings",
            NavEntry::NewFir => "nav-new-fir",
            NavEntry::ImportWord => "nav-import-word",
            NavEntry::Shortcuts => "nav-shortcuts",
            NavEntry::Theme => "nav-theme",
            NavEntry::About => "nav-about",
        }
    }

    const fn icon(self) -> IconName {
        match self {
            NavEntry::Firs => IconName::Folder,
            NavEntry::Templates => IconName::FileText,
            NavEntry::Settings => IconName::Settings,
            NavEntry::NewFir => IconName::Plus,
            NavEntry::ImportWord => IconName::FileUp,
            NavEntry::Shortcuts => IconName::Keyboard,
            NavEntry::Theme => IconName::SunMoon,
            NavEntry::About => IconName::Info,
        }
    }

    const fn keys(self) -> Option<&'static str> {
        match self {
            NavEntry::Firs => Some("G then H"),
            NavEntry::Templates => Some("G then T"),
            NavEntry::Settings => Some("G then S"),
            NavEntry::NewFir => Some("N"),
            NavEntry::ImportWord => Some("Mod+O"),
            NavEntry::Shortcuts => Some("?"),
            NavEntry::Theme | NavEntry::About => None,
        }
    }

    const fn separated_before(self) -> bool {
        matches!(self, NavEntry::NewFir | NavEntry::Shortcuts)
    }

    pub fn destination(route: Route) -> NavEntry {
        match route {
            Route::Firs | Route::Fir(_) | Route::EditFir(_) | Route::NewFir | Route::Print(_) => {
                NavEntry::Firs
            }
            Route::Templates | Route::Template(_) | Route::NewTemplate => NavEntry::Templates,
            Route::Settings => NavEntry::Settings,
        }
    }
}

pub struct NavMenu {
    focus: FocusHandle,
    highlighted: usize,
    theme_open: bool,
    theme_highlighted: usize,
}

impl EventEmitter<Dismissed> for NavMenu {}

impl Focusable for NavMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl NavMenu {
    pub fn new(cx: &mut Context<Self>) -> NavMenu {
        let route = Models::get(cx).nav.read(cx).current();
        let entry = NavEntry::destination(route);
        let highlighted = NavEntry::ALL
            .iter()
            .position(|candidate| *candidate == entry)
            .unwrap_or(0);
        NavMenu {
            focus: cx.focus_handle(),
            highlighted,
            theme_open: false,
            theme_highlighted: 0,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn highlighted(&self) -> NavEntry {
        NavEntry::ALL[self.highlighted]
    }

    pub fn theme_open(&self) -> bool {
        self.theme_open
    }

    fn move_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.theme_open {
            let count = ThemeChoice::ALL.len() as isize;
            self.theme_highlighted =
                (self.theme_highlighted as isize + delta).rem_euclid(count) as usize;
        } else {
            let count = NavEntry::ALL.len() as isize;
            self.highlighted = (self.highlighted as isize + delta).rem_euclid(count) as usize;
        }
        cx.notify();
    }

    fn open_theme(&mut self, cx: &mut Context<Self>) {
        let current = Models::get(cx).prefs.read(cx).read().theme;
        self.highlighted = NavEntry::ALL
            .iter()
            .position(|e| *e == NavEntry::Theme)
            .unwrap_or(0);
        self.theme_open = true;
        self.theme_highlighted = ThemeChoice::ALL
            .iter()
            .position(|choice| *choice == current)
            .unwrap_or(0);
        cx.notify();
    }

    pub fn run(&mut self, entry: NavEntry, window: &mut Window, cx: &mut Context<Self>) {
        if entry == NavEntry::Theme {
            self.open_theme(cx);
            return;
        }
        cx.emit(Dismissed);
        match entry {
            NavEntry::Firs => navigate(Route::Firs, window, cx),
            NavEntry::Templates => navigate(Route::Templates, window, cx),
            NavEntry::Settings => navigate(Route::Settings, window, cx),
            NavEntry::NewFir => navigate(Route::NewFir, window, cx),
            NavEntry::ImportWord => import_word_file(window, cx),
            NavEntry::Shortcuts => show_shortcuts(window, cx),
            NavEntry::About => show_about(window, cx),
            NavEntry::Theme => {}
        }
    }

    pub fn choose_theme(
        &mut self,
        choice: ThemeChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.emit(Dismissed);
        set_theme(choice, window, cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        match key {
            "down" => self.move_highlight(1, cx),
            "up" => self.move_highlight(-1, cx),
            "right" if !self.theme_open && self.highlighted() == NavEntry::Theme => {
                self.open_theme(cx)
            }
            "left" if self.theme_open => {
                self.theme_open = false;
                cx.notify();
            }
            "enter" | "space" => {
                if self.theme_open {
                    let choice = ThemeChoice::ALL[self.theme_highlighted];
                    self.choose_theme(choice, window, cx);
                } else {
                    let entry = self.highlighted();
                    self.run(entry, window, cx);
                }
            }
            "escape" => {
                if self.theme_open {
                    self.theme_open = false;
                    cx.notify();
                } else {
                    cx.emit(Dismissed);
                }
            }
            "f10" | "tab" => cx.emit(Dismissed),
            other => {
                let Some(letter) = other.chars().next().filter(|_| other.chars().count() == 1)
                else {
                    return;
                };
                let letter = letter.to_ascii_lowercase();
                let count = NavEntry::ALL.len();
                let Some(index) = (1..=count)
                    .map(|step| (self.highlighted + step) % count)
                    .find(|index| {
                        NavEntry::ALL[*index]
                            .label()
                            .chars()
                            .next()
                            .is_some_and(|first| first.to_ascii_lowercase() == letter)
                    })
                else {
                    return;
                };
                self.theme_open = false;
                self.highlighted = index;
                cx.notify();
            }
        }
        cx.stop_propagation();
    }

    fn theme_top() -> f32 {
        let mut top = 4.0;
        for entry in NavEntry::ALL {
            if entry.separated_before() {
                top += 9.0;
            }
            if entry == NavEntry::Theme {
                break;
            }
            top += MENU_ITEM;
        }
        top
    }
}

impl Render for NavMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let theme = Models::get(cx).prefs.read(cx).read().theme;
        let highlighted = self.highlighted;
        let mut panel = menu_panel(&palette)
            .id("nav-menu-panel")
            .w(px(NAV_MENU_WIDTH))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        for (index, entry) in NavEntry::ALL.into_iter().enumerate() {
            if entry.separated_before() {
                panel = panel.child(menu_separator(&palette));
            }
            let mut item = menu_item(entry.id(), entry.label())
                .icon(entry.icon())
                .highlighted(index == highlighted)
                .on_hover(cx.listener_for_hover(index))
                .on_click(cx.listener(move |this, _, window, cx| this.run(entry, window, cx)));
            item = match entry.keys() {
                Some(keys) => item.keys(keys),
                None if entry == NavEntry::Theme => item.trailing_text(theme.title()),
                None => item,
            };
            panel = panel.child(item);
        }
        let submenu = self.theme_open.then(|| {
            let theme_highlighted = self.theme_highlighted;
            menu_panel(&palette)
                .id("nav-theme-menu")
                .absolute()
                .left(px(NAV_MENU_WIDTH - 4.))
                .top(px(Self::theme_top() - 4.))
                .w(px(160.))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(
                    ThemeChoice::ALL
                        .into_iter()
                        .enumerate()
                        .map(|(index, choice)| {
                            menu_item(
                                SharedString::from(format!("nav-theme-{}", choice.as_str())),
                                choice.title(),
                            )
                            .check(choice == theme)
                            .highlighted(index == theme_highlighted)
                            .on_click(cx.listener(
                                move |this, _, window, cx| this.choose_theme(choice, window, cx),
                            ))
                        }),
                )
        });
        div()
            .id("nav-menu")
            .key_context("NavMenu")
            .track_focus(&self.focus)
            .absolute()
            .inset_0()
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(Dismissed)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(8.))
                    .top(px(HEADER_HEIGHT - 4.))
                    .child(panel)
                    .children(submenu)
                    .with_animation("nav-menu-enter", motion_fast(), |this, delta| {
                        this.opacity(delta).mt(px(-4. * (1. - delta)))
                    }),
            )
    }
}

trait HoverListener {
    fn listener_for_hover(&self, index: usize) -> impl Fn(&mut Window, &mut App) + 'static;
}

impl HoverListener for Context<'_, NavMenu> {
    fn listener_for_hover(&self, index: usize) -> impl Fn(&mut Window, &mut App) + 'static {
        let weak = self.weak_entity();
        move |_, cx| {
            weak.update(cx, |menu, cx| {
                if menu.highlighted != index {
                    menu.highlighted = index;
                    if NavEntry::ALL[index] != NavEntry::Theme {
                        menu.theme_open = false;
                    }
                    cx.notify();
                }
            })
            .ok();
        }
    }
}
