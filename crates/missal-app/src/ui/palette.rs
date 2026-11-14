use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Entity, EventEmitter, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Window, div, px,
};
use missal_core::{FirId, TemplateId};
use missal_text::{TextInput, TextInputEvent, label};

use crate::app::{navigate, set_theme, show_shortcuts, toggle_sidebar};
use crate::prefs::ThemeChoice;
use crate::state::rows::{fold_for_search, search_words};
use crate::state::{Models, Route};
use crate::theme::Palette;
use crate::ui::input::{Kind, text_input};
use crate::ui::status::status_icon;
use crate::ui::{key_hint, small_icon};

pub struct Dismissed;

#[derive(Clone)]
enum Command {
    Go(Route),
    Theme(ThemeChoice),
    ToggleSidebar,
    Shortcuts,
}

#[derive(Clone)]
enum Lead {
    Icon(IconName),
    Status(missal_core::FirStatus),
}

#[derive(Clone)]
struct Entry {
    group: &'static str,
    title: SharedString,
    subtitle: Option<SharedString>,
    lead: Lead,
    keys: Option<&'static str>,
    command: Command,
    checked: bool,
}

pub struct CommandPalette {
    models: Models,
    input: Entity<TextInput>,
    results: Vec<Entry>,
    selected: usize,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<Dismissed> for CommandPalette {}

const LIMIT: usize = 20;

impl CommandPalette {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> CommandPalette {
        let input = text_input(
            window,
            cx,
            Kind::Auto,
            "Search FIRs, templates and commands…",
            "",
        );
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Change => this.refresh(cx),
                TextInputEvent::Submit => this.run(this.selected, window, cx),
                _ => {}
            },
        );
        let mut palette = CommandPalette {
            models,
            input,
            results: Vec::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
            _subscriptions: vec![subscription],
        };
        palette.refresh(cx);
        palette
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }

    pub fn query(&self, cx: &App) -> String {
        self.input.read(cx).text().to_owned()
    }

    pub fn result_titles(&self) -> Vec<String> {
        self.results
            .iter()
            .map(|entry| entry.title.to_string())
            .collect()
    }

    fn commands(&self, cx: &App) -> Vec<Entry> {
        let current_theme = self.models.prefs.read(cx).read().theme;
        let go = |title: &'static str, route: Route, keys: &'static str, icon: IconName| Entry {
            group: "Go to",
            title: title.into(),
            subtitle: None,
            lead: Lead::Icon(icon),
            keys: Some(keys),
            command: Command::Go(route),
            checked: false,
        };
        let create = |title: &'static str, route: Route, keys: &'static str| Entry {
            group: "Create",
            title: title.into(),
            subtitle: None,
            lead: Lead::Icon(IconName::Plus),
            keys: Some(keys),
            command: Command::Go(route),
            checked: false,
        };
        let theme = |choice: ThemeChoice, icon: IconName| Entry {
            group: "Preferences",
            title: format!("{} theme", choice.title()).into(),
            subtitle: None,
            lead: Lead::Icon(icon),
            keys: None,
            command: Command::Theme(choice),
            checked: current_theme == choice,
        };
        vec![
            go("FIRs", Route::Firs, "G then H", IconName::Folder),
            go(
                "Templates",
                Route::Templates,
                "G then T",
                IconName::FileText,
            ),
            go("Settings", Route::Settings, "G then S", IconName::Settings),
            create("New FIR", Route::NewFir, "N"),
            create("New template", Route::NewTemplate, "Shift+N"),
            theme(ThemeChoice::Light, IconName::Sun),
            theme(ThemeChoice::Dark, IconName::Moon),
            theme(ThemeChoice::System, IconName::Monitor),
            Entry {
                group: "Preferences",
                title: "Toggle sidebar".into(),
                subtitle: None,
                lead: Lead::Icon(IconName::PanelLeft),
                keys: Some("Mod+B"),
                command: Command::ToggleSidebar,
                checked: false,
            },
            Entry {
                group: "Preferences",
                title: "Keyboard shortcuts".into(),
                subtitle: None,
                lead: Lead::Icon(IconName::Keyboard),
                keys: Some("?"),
                command: Command::Shortcuts,
                checked: false,
            },
        ]
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).text().to_owned();
        let words = search_words(&query);
        let mut results: Vec<Entry> = self
            .commands(cx)
            .into_iter()
            .filter(|entry| {
                let haystack = fold_for_search(&format!("{} {}", entry.group, entry.title));
                words.iter().all(|word| haystack.contains(word.as_str()))
            })
            .collect();
        let firs = self.models.firs.read(cx);
        results.extend(
            firs.rows
                .iter()
                .filter(|row| words.is_empty() || row.matches(&words))
                .take(LIMIT)
                .map(|row| Entry {
                    group: "FIRs",
                    title: format!("FIR {}", row.no).into(),
                    subtitle: Some(row.offence.clone()),
                    lead: Lead::Status(row.status),
                    keys: None,
                    command: Command::Go(Route::Fir(row.id)),
                    checked: false,
                }),
        );
        let templates = self.models.templates.read(cx);
        results.extend(
            templates
                .items
                .iter()
                .filter(|item| {
                    let haystack = fold_for_search(&format!("template {}", item.name));
                    words.iter().all(|word| haystack.contains(word.as_str()))
                })
                .take(LIMIT)
                .map(|item| Entry {
                    group: "Templates",
                    title: item.name.clone().into(),
                    subtitle: None,
                    lead: Lead::Icon(IconName::FileText),
                    keys: None,
                    command: Command::Go(Route::Template(item.id)),
                    checked: false,
                }),
        );
        self.results = results;
        self.selected = 0;
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        let count = self.results.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(count) as usize;
        self.scroll.scroll_to_item(self.selected);
        cx.notify();
    }

    fn run(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.results.get(index).cloned() else {
            return;
        };
        cx.emit(Dismissed);
        match entry.command {
            Command::Go(route) => navigate(route, window, cx),
            Command::Theme(choice) => set_theme(choice, window, cx),
            Command::ToggleSidebar => toggle_sidebar(window, cx),
            Command::Shortcuts => show_shortcuts(window, cx),
        }
    }
}

pub fn fir_route(id: FirId) -> Route {
    Route::Fir(id)
}

pub fn template_route(id: TemplateId) -> Route {
    Route::Template(id)
}

impl Render for CommandPalette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let selected = self.selected;
        let mut rows = Vec::new();
        let mut last_group = "";
        for (index, entry) in self.results.iter().enumerate() {
            if entry.group != last_group {
                last_group = entry.group;
                rows.push(
                    div()
                        .id(("palette-group", index))
                        .px_3()
                        .pt_2()
                        .pb_1()
                        .text_size(px(12.))
                        .font_medium()
                        .text_color(palette.muted_foreground)
                        .child(entry.group)
                        .into_any_element(),
                );
            }
            let lead = match &entry.lead {
                Lead::Icon(name) => small_icon(*name, palette.muted_foreground).into_any_element(),
                Lead::Status(status) => {
                    status_icon(*status, palette.status(*status), px(14.)).into_any_element()
                }
            };
            rows.push(
                h_flex()
                    .id(("palette-item", index))
                    .test_support()
                    .h(px(38.))
                    .mx_1()
                    .px_2()
                    .gap_2()
                    .items_center()
                    .rounded(px(7.))
                    .cursor_pointer()
                    .when(index == selected, |this| this.bg(palette.accent))
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if this.selected != index {
                            this.selected = index;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| this.run(index, window, cx)))
                    .child(lead)
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(14.))
                            .child(label(entry.title.clone()).ltr()),
                    )
                    .children(entry.subtitle.clone().map(|subtitle| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_size(px(14.))
                            .text_color(palette.muted_foreground)
                            .child(label(subtitle).rtl().truncate())
                    }))
                    .when(entry.subtitle.is_none(), |this| this.child(div().flex_1()))
                    .when(entry.checked, |this| {
                        this.child(small_icon(IconName::Check, palette.foreground))
                    })
                    .children(entry.keys.map(|keys| key_hint(&palette, keys)))
                    .into_any_element(),
            );
        }
        let empty = self.results.is_empty();
        div()
            .id("command-palette")
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .bg(palette.background.opacity(0.0))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "down" => this.move_selection(1, cx),
                    "up" => this.move_selection(-1, cx),
                    "enter" => this.run(this.selected, window, cx),
                    "escape" => cx.emit(Dismissed),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(Dismissed)),
            )
            .child(
                v_flex()
                    .id("palette-panel")
                    .occlude()
                    .mt(px(72.))
                    .w(px(576.))
                    .max_w_full()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(palette.input)
                    .bg(palette.popover)
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .h(px(48.))
                            .px_3()
                            .gap_2()
                            .items_center()
                            .border_b_1()
                            .border_color(palette.border)
                            .child(small_icon(IconName::Search, palette.muted_foreground))
                            .child(div().flex_1().child(self.input.clone())),
                    )
                    .child(
                        v_flex()
                            .id("palette-list")
                            .max_h(px(380.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .py_1()
                            .children(rows)
                            .when(empty, |this| {
                                this.child(
                                    div()
                                        .py_6()
                                        .text_center()
                                        .text_size(px(13.))
                                        .text_color(palette.muted_foreground)
                                        .child("No results found."),
                                )
                            }),
                    ),
            )
    }
}
