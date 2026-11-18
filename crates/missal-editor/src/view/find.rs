use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, AppContext, ClickEvent, Context, Entity, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Styled, Subscription, Window, div, px,
};
use missal_page::Position;
use missal_text::{TextInput, TextInputEvent};

use super::Editor;
use super::keys;
use super::theme::EditorTheme;
use super::toolbar::tool;
use crate::model::find::{self, Match, Query};
use crate::model::selection::document_start;
use crate::model::{Change, Selection};

pub const FIND_CONTEXT: &str = "FindBar";
pub const QUERY_CONTEXT: &str = "FindQuery > TextInput";

pub(crate) struct FindState {
    pub open: bool,
    pub replace: bool,
    pub match_case: bool,
    pub query: String,
    pub input: Entity<TextInput>,
    pub with: Entity<TextInput>,
    pub matches: Vec<Match>,
    stamp: Option<(u64, Query)>,
    origin: Option<Position>,
    replaced: Option<(usize, u64)>,
}

fn find_input(placeholder: &'static str, window: &mut Window, cx: &mut App) -> Entity<TextInput> {
    let theme = EditorTheme::get(cx);
    cx.new(|cx| {
        TextInput::new(window, cx)
            .bare()
            .placeholder(placeholder)
            .placeholder_color(theme.ink_faint)
            .placeholder_direction(missal_text::Direction::Ltr)
            .direction(missal_text::Direction::AutoLtr)
            .text_size(15.0)
    })
}

impl FindState {
    pub(crate) fn new(window: &mut Window, cx: &mut App) -> FindState {
        FindState {
            open: false,
            replace: false,
            match_case: false,
            query: String::new(),
            input: find_input("Find", window, cx),
            with: find_input("Replace with", window, cx),
            matches: Vec::new(),
            stamp: None,
            origin: None,
            replaced: None,
        }
    }

    pub(crate) fn subscribe(
        &self,
        window: &mut Window,
        cx: &mut Context<Editor>,
    ) -> Vec<Subscription> {
        vec![
            cx.subscribe_in(
                &self.input,
                window,
                |this, input, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Change => {
                        let text = input.read(cx).text().to_owned();
                        this.query_changed(text, window, cx);
                    }
                    TextInputEvent::Submit => this.find_next(window, cx),
                    _ => {}
                },
            ),
            cx.subscribe_in(
                &self.with,
                window,
                |this, _, event: &TextInputEvent, window, cx| {
                    if event == &TextInputEvent::Submit {
                        this.replace_current(window, cx);
                    }
                },
            ),
        ]
    }

    fn query(&self) -> Query {
        Query::new(self.query.clone(), self.match_case)
    }
}

impl Editor {
    pub fn is_find_open(&self) -> bool {
        self.find.open
    }

    pub fn is_replace_open(&self) -> bool {
        self.find.open && self.find.replace && !self.read_only
    }

    pub fn find_query(&self) -> &str {
        &self.find.query
    }

    pub fn find_match_case(&self) -> bool {
        self.find.match_case
    }

    pub fn find_matches(&self) -> &[Match] {
        if self.find.open {
            &self.find.matches
        } else {
            &[]
        }
    }

    pub fn current_match(&self) -> Option<usize> {
        if !self.find.open {
            return None;
        }
        let (start, end) = self.core.selection().ordered();
        find::exact(&self.find.matches, &start, &end)
    }

    pub(crate) fn refresh_find(&mut self) {
        if !self.find.open {
            return;
        }
        let state = self.core.state();
        if self.find.replaced.is_some_and(|(_, at)| at != state) {
            self.find.replaced = None;
        }
        let query = self.find.query();
        let matches = match self.find.stamp.take() {
            Some((s, q)) if s == state && q == query => {
                self.find.stamp = Some((s, q));
                return;
            }
            Some((s, q)) if s == state && find::narrows(&q, &query) => {
                let mut paths: Vec<_> = self.find.matches.iter().map(|m| m.path.clone()).collect();
                paths.dedup();
                find::find_in_paths(self.core.document(), &query, &paths)
            }
            _ => self.core.find(&query),
        };
        self.find.matches = matches;
        self.find.stamp = Some((state, query));
    }

    pub(crate) fn find_after_change(&mut self, edit: &missal_page::Edit) {
        if !self.find.open {
            return;
        }
        let (before, after) = self.core.transition();
        let Some((state, query)) = self.find.stamp.take() else {
            return;
        };
        if state != before || after != self.core.state() {
            return;
        }
        if find::retarget(&mut self.find.matches, self.core.document(), &query, edit) {
            self.find.stamp = Some((after, query));
        }
    }

    pub fn open_find(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        let was_open = self.find.open;
        self.find.open = true;
        self.find.replace = replace || (was_open && self.find.replace);
        self.field_menu_open = false;
        let selection = self.core.selection().clone();
        let (start, end) = selection.ordered();
        self.find.origin = Some(start.clone());
        let seed = (!selection.is_collapsed() && start.path == end.path)
            .then(|| self.core.selected_plain_text())
            .filter(|text| !text.trim().is_empty() && text.len() <= 256 && !text.contains('\n'));
        let input = self.find.input.clone();
        if let Some(seed) = seed {
            input.update(cx, |input, cx| input.set_text(seed.clone(), cx));
            self.query_changed(seed, window, cx);
        } else {
            self.find.stamp = None;
        }
        input.update(cx, |input, cx| {
            let len = input.text().len();
            input.select(0..len, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.find.open {
            return;
        }
        self.find.open = false;
        self.find.matches.clear();
        self.find.stamp = None;
        self.find.replaced = None;
        window.focus(&self.focus, cx);
        self.restart_blink(cx);
        cx.notify();
    }

    pub fn set_find_query(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.find.input.clone();
        input.update(cx, |input, cx| input.set_text(text, cx));
        self.query_changed(text.to_owned(), window, cx);
    }

    pub fn set_replacement(&mut self, text: &str, cx: &mut Context<Self>) {
        self.find
            .with
            .update(cx, |input, cx| input.set_text(text, cx));
    }

    pub fn replacement(&self, cx: &App) -> String {
        self.find.with.read(cx).text().to_owned()
    }

    pub fn set_match_case(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.match_case == on {
            return;
        }
        self.find.match_case = on;
        self.search_from_origin(window, cx);
    }

    fn query_changed(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if !self.find.open || text == self.find.query {
            return;
        }
        self.find.query = text;
        self.find.replaced = None;
        self.search_from_origin(window, cx);
    }

    fn search_from_origin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_find();
        let origin = self
            .find
            .origin
            .clone()
            .unwrap_or_else(|| self.core.selection().ordered().0);
        match find::at_or_after(&self.find.matches, &origin) {
            Some(index) => self.show_match(index, false, window, cx),
            None => {
                if !self.core.selection().is_collapsed() && !self.find.query.is_empty() {
                    let caret = Selection::caret(origin);
                    let change = self.core.set_selection(caret);
                    self.apply(change, window, cx);
                    self.autoscroll = false;
                }
                cx.notify();
            }
        }
    }

    fn show_match(
        &mut self,
        index: usize,
        set_origin: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(found) = self.find.matches.get(index).cloned() else {
            return;
        };
        if set_origin {
            self.find.origin = Some(found.start());
        }
        self.goal_x = None;
        let selection = Selection::new(found.start(), found.end());
        let change = self.core.set_selection(selection);
        self.apply(change, window, cx);
        self.reveal_match(&found);
        cx.notify();
    }

    fn reveal_match(&mut self, found: &Match) {
        self.autoscroll = false;
        let Some(bounds) = self.viewport else { return };
        let (Some(a), Some(b)) = (
            self.caret_canvas_rect(&found.start()),
            self.caret_canvas_rect(&found.end()),
        ) else {
            return;
        };
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let top = a.y.min(b.y);
        let bottom = a.bottom().max(b.bottom());
        let margin = ((bottom - top).max(24.0) * 2.0).min(height / 4.0);
        let covered: f32 = if self.is_replace_open() { 140.0 } else { 100.0 };
        if top < self.scroll.y + covered.min(height / 3.0)
            || bottom > self.scroll.y + height - margin
        {
            self.scroll.y = top - height / 3.0;
        }
        let left = a.x.min(b.x);
        let right = a.x.max(b.x);
        if left < self.scroll.x + 16.0 || right > self.scroll.x + width - 16.0 {
            self.scroll.x = (left + right) / 2.0 - width / 2.0;
        }
        self.clamp_scroll();
    }

    pub fn find_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step(true, window, cx);
    }

    pub fn find_previous(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step(false, window, cx);
    }

    fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.find.open {
            self.open_find(false, window, cx);
            return;
        }
        self.refresh_find();
        let count = self.find.matches.len();
        if count == 0 {
            return;
        }
        let index = match self.current_match() {
            Some(current) if forward => (current + 1) % count,
            Some(current) => (current + count - 1) % count,
            None => {
                let (start, _) = self.core.selection().ordered();
                let found = if forward {
                    find::at_or_after(&self.find.matches, &start)
                } else {
                    find::before(&self.find.matches, &start)
                };
                let Some(found) = found else { return };
                found
            }
        };
        self.show_match(index, true, window, cx);
    }

    pub fn replace_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only || !self.find.open {
            return;
        }
        self.refresh_find();
        let Some(current) = self.current_match() else {
            self.find_next(window, cx);
            return;
        };
        let target = self.find.matches[current].clone();
        let with = self.replacement(cx);
        let change = self.core.replace_matches(&[target], &with);
        self.after_replace(change, 1, window, cx);
        self.refresh_find();
        let caret = self.core.selection().head.clone();
        if let Some(index) = find::at_or_after(&self.find.matches, &caret) {
            self.show_match(index, true, window, cx);
        }
    }

    pub fn replace_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only || !self.find.open {
            return;
        }
        self.refresh_find();
        let matches = std::mem::take(&mut self.find.matches);
        let count = matches.len();
        if count == 0 {
            return;
        }
        let with = self.replacement(cx);
        let change = self.core.replace_matches(&matches, &with);
        self.after_replace(change, count, window, cx);
        self.refresh_find();
    }

    fn after_replace(
        &mut self,
        change: Option<Change>,
        count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if change.is_none() {
            return;
        }
        self.goal_x = None;
        self.apply(change, window, cx);
        self.find.replaced = Some((count, self.core.state()));
        self.find.origin = Some(self.core.selection().head.clone());
        cx.notify();
    }

    pub(crate) fn find_on_load(&mut self) {
        self.find.stamp = None;
        self.find.matches.clear();
        self.find.replaced = None;
        self.find.origin = document_start(self.core.document());
    }

    pub(crate) fn find_on_reset(&mut self, cx: &mut Context<Self>) {
        self.find.open = false;
        self.find.replace = false;
        self.find.query.clear();
        self.find
            .input
            .update(cx, |input, cx| input.set_text("", cx));
        self.find
            .with
            .update(cx, |input, cx| input.set_text("", cx));
        self.find_on_load();
    }

    fn status(&self) -> (String, bool) {
        if let Some((count, _)) = self.find.replaced {
            let noun = if count == 1 { "match" } else { "matches" };
            return (format!("Replaced {count} {noun}"), false);
        }
        if self.find.query.is_empty() {
            return (String::new(), false);
        }
        let total = self.find.matches.len();
        if total == 0 {
            return ("No results".to_owned(), true);
        }
        match self.current_match() {
            Some(index) => (format!("{} of {}", index + 1, total), false),
            None if total == 1 => ("1 match".to_owned(), false),
            None => (format!("{total} matches"), false),
        }
    }

    pub(crate) fn render_find_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.find.open {
            return None;
        }
        self.refresh_find();
        let theme = EditorTheme::get(cx);
        let entity = cx.entity();
        let act = |f: fn(&mut Editor, &mut Window, &mut Context<Editor>)| {
            let entity = entity.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                entity.update(cx, |editor, cx| f(editor, window, cx));
            }
        };
        let can_replace = !self.read_only;
        let replace_open = self.is_replace_open();
        let has_matches = !self.find.matches.is_empty();
        let (status, empty) = self.status();
        let field = |id: &'static str, input: Entity<TextInput>| {
            h_flex()
                .key_context(id)
                .flex_1()
                .min_w_0()
                .h(px(34.))
                .pl(px(10.))
                .pr(px(4.))
                .gap(px(4.))
                .items_center()
                .rounded(px(10.))
                .border_1()
                .border_color(theme.input)
                .bg(theme.surface)
                .child(div().flex_1().min_w_0().child(input))
        };
        let count = div()
            .id("editor-find-count")
            .flex_none()
            .px(px(4.))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::NORMAL)
            .text_color(if empty {
                theme.notice_text
            } else {
                theme.ink_muted
            })
            .font_features(gpui_kit::FontFeatures(std::sync::Arc::new(vec![(
                "tnum".into(),
                1,
            )])))
            .child(status);
        let match_case = self.find.match_case;
        let case = tool("editor-find-case", IconName::CaseSensitive, "Match case")
            .keys("Alt+C")
            .dense()
            .toggled(match_case)
            .on_click(act(|e, w, cx| {
                let on = !e.find.match_case;
                e.set_match_case(on, w, cx);
                e.find.input.update(cx, |input, cx| input.focus(w, cx));
            }));
        let toggle = can_replace.then(|| {
            tool(
                "editor-find-toggle-replace",
                if replace_open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                },
                if replace_open {
                    "Hide replace"
                } else {
                    "Show replace"
                },
            )
            .dense()
            .on_click(act(|e, _, cx| {
                e.find.replace = !e.find.replace;
                cx.notify();
            }))
        });
        let find_row = h_flex()
            .gap(px(2.))
            .items_center()
            .children(toggle)
            .child(
                field("FindQuery", self.find.input.clone())
                    .child(count)
                    .child(case),
            )
            .child(
                tool(
                    "editor-find-previous",
                    IconName::ChevronUp,
                    "Previous match",
                )
                .keys("Shift+Enter")
                .dense()
                .disabled(!has_matches)
                .on_click(act(Editor::find_previous)),
            )
            .child(
                tool("editor-find-next", IconName::ChevronDown, "Next match")
                    .keys("Enter")
                    .dense()
                    .disabled(!has_matches)
                    .on_click(act(Editor::find_next)),
            )
            .child(
                tool("editor-find-close", IconName::X, "Close")
                    .keys("Escape")
                    .dense()
                    .on_click(act(Editor::close_find)),
            );
        let replace_row = replace_open.then(|| {
            h_flex()
                .gap(px(2.))
                .items_center()
                .child(div().flex_none().w(px(24.)))
                .child(field("FindReplacement", self.find.with.clone()))
                .child(
                    tool("editor-replace-one", IconName::Replace, "Replace")
                        .keys("Enter")
                        .dense()
                        .disabled(!has_matches)
                        .on_click(act(Editor::replace_current)),
                )
                .child(
                    tool("editor-replace-all", IconName::ReplaceAll, "Replace all")
                        .keys("Mod+Alt+Enter")
                        .dense()
                        .disabled(!has_matches)
                        .on_click(act(Editor::replace_all)),
                )
                .child(div().flex_none().w(px(24.)))
        });
        Some(
            v_flex()
                .id("editor-find")
                .key_context(FIND_CONTEXT)
                .absolute()
                .top(px(10.))
                .right(px(16.))
                .w(px(400.))
                .p(px(6.))
                .gap(px(6.))
                .rounded(px(14.))
                .bg(theme.raised)
                .shadow(theme.bar_shadows().to_vec())
                .occlude()
                .font_family(missal_text::UI_FAMILY)
                .on_action(
                    cx.listener(|this, _: &keys::CloseFind, window, cx| {
                        this.close_find(window, cx)
                    }),
                )
                .on_action(cx.listener(|this, _: &keys::ReplaceAll, window, cx| {
                    this.replace_all(window, cx)
                }))
                .on_action(cx.listener(|this, _: &keys::ToggleMatchCase, window, cx| {
                    let on = !this.find.match_case;
                    this.set_match_case(on, window, cx)
                }))
                .child(find_row)
                .children(replace_row)
                .into_any_element(),
        )
    }
}
