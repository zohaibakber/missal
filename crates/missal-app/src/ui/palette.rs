use gpui_kit::AnimationExt;
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FontWeight, HighlightStyle, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, StyledText, Subscription, Window, div, px,
};
use missal_core::FirStatus;
use missal_text::{TextInput, TextInputEvent};

use crate::app::{import_word_file, navigate, set_theme, show_shortcuts};
use crate::prefs::ThemeChoice;
use crate::state::rows::{fold_for_search, search_words};
use crate::state::{Models, Route};
use crate::theme::{ICON, PALETTE_TOP, PALETTE_WIDTH, Palette, RADIUS, RADIUS_XL, motion_base};
use crate::ui::input::{Kind, field_input};
use crate::ui::status::status_dot;
use crate::ui::{KeyTone, UrRole, keycap, keycaps_toned, sized_icon, ur_inline};

pub struct Dismissed;

pub const RECENT_LIMIT: usize = 6;
const FIR_LIMIT: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Recent,
    Actions,
    Firs,
    Templates,
    Commands,
    Theme,
}

impl Group {
    pub const fn title(self) -> &'static str {
        match self {
            Group::Recent => "Recent FIRs",
            Group::Actions => "Actions",
            Group::Firs => "FIRs",
            Group::Templates => "Templates",
            Group::Commands => "Commands",
            Group::Theme => "Theme",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Command {
    Go(Route),
    ImportWord,
    Shortcuts,
    ThemeStep,
    Theme(ThemeChoice),
}

#[derive(Clone)]
enum Kind2 {
    Fir {
        no: SharedString,
        offence: SharedString,
        status: FirStatus,
    },
    Template(SharedString),
    Command {
        title: SharedString,
        icon: IconName,
        keys: Option<&'static str>,
        checked: bool,
    },
}

#[derive(Clone)]
struct Entry {
    group: Group,
    kind: Kind2,
    command: Command,
}

impl Entry {
    fn title(&self) -> String {
        match &self.kind {
            Kind2::Fir { no, .. } => format!("FIR {no}"),
            Kind2::Template(name) => name.to_string(),
            Kind2::Command { title, .. } => title.to_string(),
        }
    }
}

pub struct CommandPalette {
    models: Models,
    input: Entity<TextInput>,
    results: Vec<Entry>,
    selected: usize,
    theme_step: bool,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<Dismissed> for CommandPalette {}

fn command(
    title: &'static str,
    icon: IconName,
    keys: Option<&'static str>,
    command: Command,
) -> Entry {
    Entry {
        group: Group::Actions,
        kind: Kind2::Command {
            title: title.into(),
            icon,
            keys,
            checked: false,
        },
        command,
    }
}

fn actions() -> Vec<Entry> {
    vec![
        command(
            "New FIR",
            IconName::Plus,
            Some("N"),
            Command::Go(Route::NewFir),
        ),
        command(
            "New template",
            IconName::Plus,
            Some("Shift+N"),
            Command::Go(Route::NewTemplate),
        ),
        command(
            "Import Word file",
            IconName::FileUp,
            Some("Mod+O"),
            Command::ImportWord,
        ),
        command(
            "Keyboard shortcuts",
            IconName::Keyboard,
            Some("?"),
            Command::Shortcuts,
        ),
        command("Theme…", IconName::SunMoon, None, Command::ThemeStep),
        command(
            "Go to FIRs",
            IconName::Folder,
            Some("G then H"),
            Command::Go(Route::Firs),
        ),
        command(
            "Go to Templates",
            IconName::FileText,
            Some("G then T"),
            Command::Go(Route::Templates),
        ),
        command(
            "Go to Settings",
            IconName::Settings,
            Some("G then S"),
            Command::Go(Route::Settings),
        ),
    ]
}

impl CommandPalette {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> CommandPalette {
        let input = field_input(
            window,
            cx,
            Kind::Search,
            "Search FIRs, templates, commands",
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
            theme_step: false,
            scroll: ScrollHandle::new(),
            _subscriptions: vec![subscription],
        };
        palette.refresh(cx);
        palette
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }

    pub fn input(&self) -> &Entity<TextInput> {
        &self.input
    }

    pub fn query(&self, cx: &App) -> String {
        self.input.read(cx).text().to_owned()
    }

    pub fn result_titles(&self) -> Vec<String> {
        self.results.iter().map(Entry::title).collect()
    }

    pub fn result_groups(&self) -> Vec<Group> {
        self.results.iter().map(|entry| entry.group).collect()
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).text().to_owned();
        let words = search_words(&query);
        let mut results = Vec::new();
        if self.theme_step {
            let current = self.models.prefs.read(cx).read().theme;
            results.extend(ThemeChoice::ALL.into_iter().map(|choice| Entry {
                group: Group::Theme,
                kind: Kind2::Command {
                    title: choice.title().into(),
                    icon: match choice {
                        ThemeChoice::System => IconName::Monitor,
                        ThemeChoice::Light => IconName::Sun,
                        ThemeChoice::Dark => IconName::Moon,
                    },
                    keys: None,
                    checked: choice == current,
                },
                command: Command::Theme(choice),
            }));
        } else if words.is_empty() {
            let recent = self.models.prefs.read(cx).read().recent_firs.clone();
            let firs = self.models.firs.read(cx);
            results.extend(
                recent
                    .iter()
                    .filter_map(|entry| firs.get(entry.id))
                    .take(RECENT_LIMIT)
                    .map(|row| Entry {
                        group: Group::Recent,
                        kind: Kind2::Fir {
                            no: row.no.clone(),
                            offence: row.offence.clone(),
                            status: row.status,
                        },
                        command: Command::Go(Route::Fir(row.id)),
                    }),
            );
            results.extend(actions());
        } else {
            let firs = self.models.firs.read(cx);
            results.extend(
                firs.rows
                    .iter()
                    .filter(|row| row.matches(&words))
                    .take(FIR_LIMIT)
                    .map(|row| Entry {
                        group: Group::Firs,
                        kind: Kind2::Fir {
                            no: row.no.clone(),
                            offence: row.offence.clone(),
                            status: row.status,
                        },
                        command: Command::Go(Route::Fir(row.id)),
                    }),
            );
            let templates = self.models.templates.read(cx);
            results.extend(
                templates
                    .items
                    .iter()
                    .filter(|item| {
                        let haystack = fold_for_search(&item.name);
                        words.iter().all(|word| haystack.contains(word.as_str()))
                    })
                    .take(FIR_LIMIT)
                    .map(|item| Entry {
                        group: Group::Templates,
                        kind: Kind2::Template(item.name.clone().into()),
                        command: Command::Go(Route::Template(item.id)),
                    }),
            );
            results.extend(
                actions()
                    .into_iter()
                    .filter(|entry| {
                        let haystack = fold_for_search(&entry.title());
                        words.iter().all(|word| haystack.contains(word.as_str()))
                    })
                    .map(|entry| Entry {
                        group: Group::Commands,
                        ..entry
                    }),
            );
        }
        self.results = results;
        self.selected = if self.theme_step {
            self.results
                .iter()
                .position(|entry| matches!(entry.kind, Kind2::Command { checked: true, .. }))
                .unwrap_or(0)
        } else {
            0
        };
        self.scroll.scroll_to_item(self.selected);
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

    fn escape(&mut self, cx: &mut Context<Self>) {
        if self.theme_step {
            self.theme_step = false;
            self.refresh(cx);
        } else if !self.input.read(cx).text().is_empty() {
            self.input.update(cx, |input, cx| input.set_text("", cx));
        } else {
            cx.emit(Dismissed);
        }
    }

    pub fn run(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.results.get(index).cloned() else {
            return;
        };
        if entry.command == Command::ThemeStep {
            self.theme_step = true;
            self.input.update(cx, |input, cx| input.set_text("", cx));
            self.refresh(cx);
            return;
        }
        cx.emit(Dismissed);
        match entry.command {
            Command::Go(route) => navigate(route, window, cx),
            Command::ImportWord => import_word_file(window, cx),
            Command::Shortcuts => show_shortcuts(window, cx),
            Command::Theme(choice) => set_theme(choice, window, cx),
            Command::ThemeStep => {}
        }
    }

    fn render_row(
        &self,
        palette: &Palette,
        index: usize,
        entry: &Entry,
        query: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = index == self.selected;
        let lead: AnyElement;
        let body: AnyElement;
        let mut trail: Option<AnyElement> = None;
        match &entry.kind {
            Kind2::Fir {
                no,
                offence,
                status,
            } => {
                lead = status_dot(palette, *status).into_any_element();
                body = h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(92.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_size(px(14.))
                            .text_color(palette.ink)
                            .child(highlight_number(palette, no, query)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .mt(px(-2.))
                            .text_color(palette.ink)
                            .child(ur_inline(offence.clone(), UrRole::Body).truncate()),
                    )
                    .into_any_element();
            }
            Kind2::Template(name) => {
                lead = sized_icon(IconName::FileText, ICON, palette.ink_muted).into_any_element();
                body = div()
                    .flex_1()
                    .min_w_0()
                    .mt(px(-2.))
                    .text_color(palette.ink)
                    .child(ur_inline(name.clone(), UrRole::Body).truncate())
                    .into_any_element();
            }
            Kind2::Command {
                title,
                icon,
                keys,
                checked,
            } => {
                lead = sized_icon(*icon, ICON, palette.ink_muted).into_any_element();
                body = div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(14.))
                    .text_color(palette.ink)
                    .child(title.clone())
                    .into_any_element();
                if *checked {
                    trail = Some(sized_icon(IconName::Check, ICON, palette.ink).into_any_element());
                } else if let Some(keys) = keys
                    && !selected
                {
                    trail = Some(keycaps_toned(palette, keys, KeyTone::Bare).into_any_element());
                }
            }
        }
        if selected && trail.is_none() {
            trail = Some(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(palette.ink_muted)
                    .child(if entry.command == Command::ThemeStep {
                        "Choose"
                    } else {
                        "Open"
                    })
                    .child(keycap(palette, "↵"))
                    .into_any_element(),
            );
        }
        h_flex()
            .id(("palette-item", index))
            .flex_none()
            .h(px(40.))
            .mx(px(6.))
            .px(px(10.))
            .gap(px(10.))
            .items_center()
            .rounded(px(RADIUS))
            .cursor_pointer()
            .when(selected, |this| this.bg(palette.hover))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.selected != index {
                    this.selected = index;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.run(index, window, cx)))
            .child(lead)
            .child(body)
            .children(trail)
            .test_support()
            .into_any_element()
    }
}

fn highlight_number(palette: &Palette, no: &SharedString, query: &str) -> AnyElement {
    let needle = fold_for_search(query.trim());
    let haystack = fold_for_search(no);
    let range = (!needle.is_empty() && haystack.len() == no.len())
        .then(|| {
            haystack
                .find(needle.as_str())
                .map(|start| start..start + needle.len())
        })
        .flatten();
    match range {
        Some(range) => StyledText::new(no.clone())
            .with_highlights([(
                range,
                HighlightStyle {
                    color: Some(palette.accent),
                    font_weight: Some(FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            )])
            .into_any_element(),
        None => div().child(no.clone()).into_any_element(),
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let query = self.input.read(cx).text().to_owned();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut last_group = None;
        let results = self.results.clone();
        for (index, entry) in results.iter().enumerate() {
            if last_group != Some(entry.group) {
                last_group = Some(entry.group);
                rows.push(
                    div()
                        .id(("palette-group", index))
                        .flex_none()
                        .pt(px(10.))
                        .pb(px(4.))
                        .px(px(16.))
                        .text_size(px(12.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.ink_muted)
                        .child(entry.group.title())
                        .into_any_element(),
                );
            }
            rows.push(self.render_row(&palette, index, entry, &query, cx));
        }
        let empty = self.results.is_empty();
        let count = if query.trim().is_empty() || self.theme_step {
            None
        } else {
            let firs = self
                .results
                .iter()
                .filter(|entry| entry.group == Group::Firs)
                .count();
            Some(if firs == self.results.len() {
                format!("{firs} {}", if firs == 1 { "FIR" } else { "FIRs" })
            } else {
                let total = self.results.len();
                format!("{total} {}", if total == 1 { "result" } else { "results" })
            })
        };
        let hint = |keys: Vec<&'static str>, text: &'static str| {
            h_flex()
                .items_center()
                .gap(px(5.))
                .children(keys.into_iter().map(|key| keycap(&palette, key)))
                .child(text)
        };
        div()
            .id("command-palette")
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .font_family(missal_text::UI_FAMILY)
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "down" => this.move_selection(1, cx),
                    "up" => this.move_selection(-1, cx),
                    "enter" => this.run(this.selected, window, cx),
                    "escape" => this.escape(cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(Dismissed)),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(palette.scrim)
                    .with_animation("palette-scrim", motion_base(), |this, delta| {
                        this.opacity(delta)
                    }),
            )
            .child(
                v_flex()
                    .id("palette-panel")
                    .occlude()
                    .mt(px(PALETTE_TOP))
                    .w(px(PALETTE_WIDTH))
                    .max_w_full()
                    .rounded(px(RADIUS_XL))
                    .bg(palette.raised)
                    .shadow(palette.shadow_lg())
                    .text_color(palette.ink)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .h(px(52.))
                            .pl(px(16.))
                            .pr(px(14.))
                            .gap(px(10.))
                            .items_center()
                            .border_b_1()
                            .border_color(palette.line)
                            .child(sized_icon(IconName::Search, ICON, palette.ink))
                            .child(div().flex_1().min_w_0().child(self.input.clone()))
                            .child(keycap(&palette, "Esc")),
                    )
                    .child(
                        v_flex()
                            .id("palette-list")
                            .max_h(px(520.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .children(rows)
                            .when(empty, |this| {
                                this.child(
                                    div()
                                        .py_6()
                                        .text_center()
                                        .text_size(px(13.))
                                        .text_color(palette.ink_muted)
                                        .child("Nothing matches. Try a FIR number, an offence or a name."),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .h(px(36.))
                            .mt(px(6.))
                            .px(px(16.))
                            .gap(px(14.))
                            .items_center()
                            .border_t_1()
                            .border_color(palette.line)
                            .text_size(px(12.))
                            .text_color(palette.ink_muted)
                            .child(hint(vec!["↑", "↓"], "Move"))
                            .child(hint(vec!["↵"], "Open"))
                            .child(hint(vec!["Esc"], "Close"))
                            .child(div().flex_1())
                            .children(count),
                    )
                    .with_animation("palette-enter", motion_base(), |this, delta| {
                        this.opacity(delta)
                    }),
            )
    }
}
