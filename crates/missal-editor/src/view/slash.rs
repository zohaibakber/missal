use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::{
    AnchoredPositionMode, AnyElement, App, Context, FontWeight, IntoElement, ParentElement,
    ScrollHandle, SharedString, Styled, Window, div, px,
};
use missal_core::document::{Direction, HeadingLevel, ListType};
use missal_core::text::fold_text;
use missal_core::{FIR_FIELDS, FieldCatalog, FieldReference, PlaceholderSource};
use missal_page::BlockPath;
use missal_text::listbox::{self, ListNav, ListboxTheme, listbox, listbox_row, listbox_section};
use missal_text::{Align, LineHeight, label};

use super::Editor;
use super::theme::EditorTheme;
use crate::model::{BlockCommand, TextKind};

pub const SLASH_CONTEXT: &str = "SlashMenu";
const ROW_HEIGHT: f32 = 34.0;
const WIDTH: f32 = 300.0;
const MAX_ROWS: usize = 9;
const PAGE: usize = 5;
const MAX_QUERY_CHARS: usize = 32;
const DEFAULT_TABLE: (u32, u32) = (3, 3);

pub fn bind_keys(cx: &mut App) {
    listbox::bind_keys(cx, SLASH_CONTEXT);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SlashGroup {
    Blocks,
    Insert,
    Fields,
}

impl SlashGroup {
    pub const fn title(self) -> &'static str {
        match self {
            SlashGroup::Blocks => "Blocks",
            SlashGroup::Insert => "Insert",
            SlashGroup::Fields => "Fields",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlashItem {
    pub title: SharedString,
    pub group: SlashGroup,
    pub icon: IconName,
    pub command: BlockCommand,
    pub detail: Option<SharedString>,
    pub urdu: bool,
}

struct Spec {
    title: &'static str,
    group: SlashGroup,
    icon: IconName,
    command: fn() -> BlockCommand,
    keywords: &'static [&'static str],
}

const SPECS: &[Spec] = &[
    Spec {
        title: "Text",
        group: SlashGroup::Blocks,
        icon: IconName::Pilcrow,
        command: || BlockCommand::Text(TextKind::Paragraph),
        keywords: &["paragraph", "plain", "body", "متن", "پیراگراف"],
    },
    Spec {
        title: "Heading 1",
        group: SlashGroup::Blocks,
        icon: IconName::Heading1,
        command: || BlockCommand::Text(TextKind::Heading(HeadingLevel::H1)),
        keywords: &["h1", "title", "heading", "عنوان", "سرخی"],
    },
    Spec {
        title: "Heading 2",
        group: SlashGroup::Blocks,
        icon: IconName::Heading2,
        command: || BlockCommand::Text(TextKind::Heading(HeadingLevel::H2)),
        keywords: &["h2", "subtitle", "heading", "عنوان", "سرخی"],
    },
    Spec {
        title: "Heading 3",
        group: SlashGroup::Blocks,
        icon: IconName::Heading3,
        command: || BlockCommand::Text(TextKind::Heading(HeadingLevel::H3)),
        keywords: &["h3", "heading", "عنوان", "سرخی"],
    },
    Spec {
        title: "Quote",
        group: SlashGroup::Blocks,
        icon: IconName::TextQuote,
        command: || BlockCommand::Text(TextKind::Quote),
        keywords: &["blockquote", "citation", "اقتباس"],
    },
    Spec {
        title: "Bulleted list",
        group: SlashGroup::Blocks,
        icon: IconName::List,
        command: || BlockCommand::List(ListType::Bullet),
        keywords: &["ul", "bullet", "unordered", "list", "فہرست"],
    },
    Spec {
        title: "Numbered list",
        group: SlashGroup::Blocks,
        icon: IconName::ListOrdered,
        command: || BlockCommand::List(ListType::Number),
        keywords: &["ol", "ordered", "number", "list", "فہرست", "نمبر"],
    },
    Spec {
        title: "Table",
        group: SlashGroup::Insert,
        icon: IconName::Table,
        command: || BlockCommand::table(DEFAULT_TABLE.0, DEFAULT_TABLE.1),
        keywords: &["grid", "rows", "columns", "جدول", "ٹیبل", "خانے"],
    },
    Spec {
        title: "Divider",
        group: SlashGroup::Insert,
        icon: IconName::Minus,
        command: || BlockCommand::Divider,
        keywords: &["hr", "rule", "line", "separator", "لکیر"],
    },
    Spec {
        title: "Page break",
        group: SlashGroup::Insert,
        icon: IconName::SeparatorHorizontal,
        command: || BlockCommand::PageBreak,
        keywords: &["new page", "break", "صفحہ"],
    },
];

fn table_size(query: &str) -> Option<(&str, u32, u32)> {
    let trimmed = query.trim_end();
    let split = trimmed
        .rfind(|c: char| !(c.is_ascii_digit() || matches!(c, 'x' | '×' | '*') || c == ' '))
        .map_or(0, |index| {
            index + trimmed[index..].chars().next().map_or(1, char::len_utf8)
        });
    let (rest, size) = trimmed.split_at(split);
    let (rows, columns) = size.split_once(['x', '×', '*'])?;
    let rows: u32 = rows.trim().parse().ok()?;
    let columns: u32 = columns.trim().parse().ok()?;
    (rows > 0 && columns > 0).then_some((rest.trim(), rows, columns))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    TitleStart,
    WordStart,
    Keyword,
    Inside,
}

fn rank(title: &str, keywords: &[&str], needle: &str) -> Option<Rank> {
    if needle.is_empty() {
        return Some(Rank::TitleStart);
    }
    let title = fold_text(title);
    if title.starts_with(needle) {
        return Some(Rank::TitleStart);
    }
    if title.split(' ').any(|word| word.starts_with(needle)) {
        return Some(Rank::WordStart);
    }
    if keywords
        .iter()
        .any(|keyword| fold_text(keyword).starts_with(needle))
    {
        return Some(Rank::Keyword);
    }
    (needle.chars().count() >= 2 && title.contains(needle)).then_some(Rank::Inside)
}

fn is_urdu(text: &str) -> bool {
    text.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

pub fn field_choices(catalog: &FieldCatalog) -> Vec<(String, FieldReference, bool)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for field in FIR_FIELDS.iter() {
        let reference = FieldReference::Fir(field.field);
        if seen.insert(reference.clone()) {
            out.push((field.label.to_owned(), reference, is_urdu(field.label)));
        }
    }
    for source in [PlaceholderSource::Shared, PlaceholderSource::Custom] {
        for placeholder in catalog.placeholders() {
            if placeholder.source == source {
                out.push((
                    placeholder.label.clone(),
                    FieldReference::Catalog(placeholder.id),
                    is_urdu(&placeholder.label),
                ));
            }
        }
    }
    out
}

pub fn slash_items(
    query: &str,
    fields: &[(String, FieldReference, bool)],
    in_table: bool,
) -> Vec<SlashItem> {
    let folded = fold_text(query);
    let needle = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    let size = table_size(&needle);
    let mut ranked: Vec<(Rank, usize, SlashItem)> = Vec::new();
    for (order, spec) in SPECS.iter().enumerate() {
        if in_table && matches!((spec.command)(), BlockCommand::PageBreak) {
            continue;
        }
        let mut command = (spec.command)();
        let mut detail = None;
        let mut target = needle.as_str();
        if matches!(command, BlockCommand::Table { .. }) {
            let (rest, rows, columns) = size.unwrap_or((target, DEFAULT_TABLE.0, DEFAULT_TABLE.1));
            target = rest;
            command = BlockCommand::table(rows, columns);
            if let BlockCommand::Table { rows, columns } = command {
                detail = Some(format!("{rows} × {columns}").into());
            }
        } else if size.is_some() {
            continue;
        }
        if let Some(rank) = rank(spec.title, spec.keywords, target) {
            ranked.push((
                rank,
                order,
                SlashItem {
                    title: spec.title.into(),
                    group: spec.group,
                    icon: spec.icon,
                    command,
                    detail,
                    urdu: false,
                },
            ));
        }
    }
    if size.is_none() {
        for (order, (name, reference, urdu)) in fields.iter().enumerate() {
            if let Some(rank) = rank(name, &[], &needle) {
                ranked.push((
                    rank,
                    SPECS.len() + order,
                    SlashItem {
                        title: name.clone().into(),
                        group: SlashGroup::Fields,
                        icon: IconName::Braces,
                        command: BlockCommand::Field(reference.clone()),
                        detail: None,
                        urdu: *urdu,
                    },
                ));
            }
        }
    }
    if needle.is_empty() {
        ranked.sort_by_key(|(_, order, item)| (item.group, *order));
    } else {
        ranked.sort_by_key(|(rank, order, _)| (*rank, *order));
    }
    ranked.into_iter().map(|(_, _, item)| item).collect()
}

pub(crate) struct SlashMenu {
    path: BlockPath,
    start: usize,
    query: String,
    items: Vec<SlashItem>,
    nav: ListNav,
    reveal: bool,
}

pub(crate) struct SlashState {
    menu: Option<SlashMenu>,
    scroll: ScrollHandle,
}

impl Default for SlashState {
    fn default() -> SlashState {
        SlashState {
            menu: None,
            scroll: ScrollHandle::new(),
        }
    }
}

fn preceded_by_space(text: &str, offset: usize) -> bool {
    text[..offset]
        .chars()
        .next_back()
        .is_none_or(|c| c.is_whitespace() || c == missal_page::OBJECT)
}

impl Editor {
    pub fn slash_is_open(&self) -> bool {
        self.slash.menu.is_some()
    }

    pub fn slash_titles(&self) -> Vec<String> {
        self.slash
            .menu
            .iter()
            .flat_map(|menu| menu.items.iter().map(|item| item.title.to_string()))
            .collect()
    }

    pub fn slash_active(&self) -> Option<usize> {
        self.slash.menu.as_ref().and_then(|menu| menu.nav.active())
    }

    fn slash_fields(&self) -> Vec<(String, FieldReference, bool)> {
        field_choices(&self.fields.catalog)
    }

    pub(crate) fn typed_slash(&mut self, typed: &str, cx: &mut Context<Self>) {
        if typed != "/" || self.read_only || self.slash.menu.is_some() {
            return;
        }
        let selection = self.core.selection();
        if !selection.is_collapsed() {
            return;
        }
        let head = selection.head.clone();
        let Some(text) = self.core.block_text(&head.path) else {
            return;
        };
        let Some(start) = head.offset.checked_sub(1) else {
            return;
        };
        if text.get(start..head.offset) != Some("/") || !preceded_by_space(&text, start) {
            return;
        }
        let items = slash_items("", &self.slash_fields(), self.core.in_table());
        self.slash.menu = Some(SlashMenu {
            path: head.path,
            start,
            query: String::new(),
            nav: ListNav::with_active(items.len(), Some(0)),
            items,
            reveal: true,
        });
        cx.notify();
    }

    pub(crate) fn sync_slash(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.slash.menu.as_ref() else {
            return;
        };
        let selection = self.core.selection();
        let head = &selection.head;
        let query =
            (selection.is_collapsed() && head.path == menu.path && head.offset > menu.start)
                .then(|| self.core.block_text(&menu.path))
                .flatten()
                .and_then(|text| {
                    (text.get(menu.start..menu.start + 1) == Some("/"))
                        .then(|| text.get(menu.start + 1..head.offset).map(str::to_owned))
                        .flatten()
                })
                .filter(|query| {
                    query.chars().count() <= MAX_QUERY_CHARS
                        && !query.contains(missal_page::OBJECT)
                        && !query.starts_with(' ')
                        && !query.contains("  ")
                });
        let Some(query) = query else {
            self.close_slash(cx);
            return;
        };
        if query == menu.query {
            return;
        }
        let items = slash_items(&query, &self.slash_fields(), self.core.in_table());
        if items.is_empty() && query.ends_with(' ') {
            self.close_slash(cx);
            return;
        }
        if let Some(menu) = self.slash.menu.as_mut() {
            menu.nav = ListNav::with_active(items.len(), Some(0));
            menu.items = items;
            menu.query = query;
            menu.reveal = true;
        }
        cx.notify();
    }

    pub(crate) fn close_slash(&mut self, cx: &mut Context<Self>) {
        if self.slash.menu.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn slash_navigate(
        &mut self,
        f: impl FnOnce(&mut ListNav) -> bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(menu) = self.slash.menu.as_mut() else {
            return false;
        };
        if f(&mut menu.nav) {
            menu.reveal = true;
            cx.notify();
        }
        true
    }

    pub(crate) fn slash_step(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        self.slash_navigate(|nav| nav.step(delta), cx)
    }

    pub(crate) fn slash_page(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        self.slash_navigate(|nav| nav.page(delta, PAGE), cx)
    }

    fn slash_hover(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(menu) = self.slash.menu.as_mut()
            && menu.nav.set_active(Some(index))
        {
            menu.reveal = false;
            cx.notify();
        }
    }

    pub(crate) fn slash_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.slash_active() else {
            return self.slash.menu.is_some();
        };
        self.choose_slash(index, window, cx);
        true
    }

    pub fn choose_slash(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.slash.menu.take() else {
            return;
        };
        let Some(item) = menu.items.get(index) else {
            return;
        };
        let end = menu.start + 1 + menu.query.len();
        let command = item.command.clone();
        let path = menu.path.clone();
        self.edit(window, cx, |core| {
            core.apply_block_command(&path, menu.start..end, command)
        });
        cx.notify();
    }

    fn slash_direction(&self, path: &BlockPath) -> Direction {
        crate::model::tree::props(self.core.document(), path)
            .and_then(|props| props.direction)
            .or(self.core.document().root.direction)
            .unwrap_or(Direction::Rtl)
    }

    pub(crate) fn render_slash(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.slash.menu.as_ref()?;
        let anchor =
            self.position_screen_bounds(&crate::model::pos(menu.path.clone(), menu.start))?;
        let viewport = self.viewport?;
        if anchor.bottom() < viewport.top() || anchor.top() > viewport.bottom() {
            return None;
        }
        let theme = EditorTheme::get(cx);
        let rtl = self.slash_direction(&menu.path) == Direction::Rtl;
        let grouped = menu.query.is_empty();
        let row = |item: &SlashItem| {
            let title: AnyElement = if item.urdu {
                div()
                    .text_size(px(16.))
                    .child(
                        label(item.title.clone())
                            .rtl()
                            .align(Align::Left)
                            .leading(LineHeight::Px(30.))
                            .truncate(),
                    )
                    .into_any_element()
            } else {
                div()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .truncate()
                    .child(item.title.clone())
                    .into_any_element()
            };
            let mut row = listbox_row(title).lead(
                Icon::new(item.icon)
                    .size(px(16.))
                    .text_color(theme.ink_muted),
            );
            if let Some(detail) = item.detail.clone() {
                row = row.trail(detail);
            }
            row
        };
        let mut list = listbox("slash-menu", slash_theme(&theme))
            .active(menu.nav.active())
            .row_height(ROW_HEIGHT)
            .width(px(WIDTH))
            .max_rows(MAX_ROWS)
            .track_scroll(&self.slash.scroll, menu.reveal)
            .empty(
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .text_size(px(13.))
                    .text_color(theme.ink_muted)
                    .child("No matching blocks or fields"),
            )
            .footer(slash_hints(&theme));
        if grouped {
            for group in [SlashGroup::Blocks, SlashGroup::Insert, SlashGroup::Fields] {
                let rows = menu
                    .items
                    .iter()
                    .filter(|item| item.group == group)
                    .map(row)
                    .collect();
                list = list.section(listbox_section(Some(group.title().into()), rows));
            }
        } else {
            list = list.rows(menu.items.iter().map(row).collect());
        }
        let hover = cx.entity().downgrade();
        let pick = hover.clone();
        let list = list
            .on_hover(move |index, _, cx| {
                hover
                    .update(cx, |editor, cx| editor.slash_hover(index, cx))
                    .ok();
            })
            .on_pick(move |index, window, cx| {
                pick.update(cx, |editor, cx| editor.choose_slash(index, window, cx))
                    .ok();
            });
        Some(listbox::popover(anchor, AnchoredPositionMode::Window, rtl, list).into_any_element())
    }
}

pub(crate) fn slash_theme(theme: &EditorTheme) -> ListboxTheme {
    ListboxTheme {
        panel: theme.raised,
        ink: theme.ink,
        ink_muted: theme.ink_muted,
        ink_faint: theme.ink_faint,
        active: theme.hover,
        line: theme.line,
        shadow: theme.bar_shadows().to_vec(),
        radius: 10.0,
        row_radius: 6.0,
    }
}

fn keycap(theme: &EditorTheme, text: &'static str) -> AnyElement {
    div()
        .h(px(18.))
        .min_w(px(18.))
        .px(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .bg(theme.hover)
        .text_color(theme.ink_muted)
        .text_size(px(11.))
        .line_height(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .child(text)
        .into_any_element()
}

fn slash_hints(theme: &EditorTheme) -> AnyElement {
    let hint = |keys: &[&'static str], text: &'static str| {
        h_flex()
            .items_center()
            .gap(px(4.))
            .children(keys.iter().map(|key| keycap(theme, key)))
            .child(text)
    };
    h_flex()
        .h(px(32.))
        .px(px(12.))
        .gap(px(12.))
        .items_center()
        .text_size(px(12.))
        .text_color(theme.ink_faint)
        .child(hint(&["↑", "↓"], "Navigate"))
        .child(hint(&["↵"], "Insert"))
        .child(div().flex_1())
        .child(hint(&["Esc"], "Close"))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::blocks::MAX_TABLE_SIDE;
    use missal_core::{FirField, FirProperty};

    fn titles(items: &[SlashItem]) -> Vec<&str> {
        items.iter().map(|item| item.title.as_ref()).collect()
    }

    fn fields() -> Vec<(String, FieldReference, bool)> {
        field_choices(&FieldCatalog::default())
    }

    #[test]
    fn an_empty_query_lists_blocks_then_insert_then_fields() {
        let items = slash_items("", &fields(), false);
        assert_eq!(items[0].title.as_ref(), "Text");
        let groups: Vec<SlashGroup> = items.iter().map(|item| item.group).collect();
        assert!(groups.is_sorted());
        assert!(groups.contains(&SlashGroup::Fields));
    }

    #[test]
    fn titles_rank_before_keywords() {
        let items = slash_items("h", &fields(), false);
        assert_eq!(
            &titles(&items)[..3],
            ["Heading 1", "Heading 2", "Heading 3"]
        );
        assert_eq!(titles(&slash_items("h1", &fields(), false))[0], "Heading 1");
        assert_eq!(titles(&slash_items("hr", &fields(), false)), ["Divider"]);
        assert_eq!(
            titles(&slash_items("ol", &fields(), false)),
            ["Numbered list"]
        );
    }

    #[test]
    fn urdu_queries_find_blocks_and_fields_with_letter_variants() {
        assert_eq!(titles(&slash_items("جدول", &fields(), false)), ["Table"]);
        let found = slash_items("مقدمه", &fields(), false);
        assert_eq!(found[0].group, SlashGroup::Fields);
        assert_eq!(
            found[0].command,
            BlockCommand::Field(FieldReference::Fir(FirField {
                property: FirProperty::FirNo,
                index: None
            }))
        );
    }

    #[test]
    fn duplicate_field_aliases_are_listed_once() {
        let body = FieldReference::Fir(FirField {
            property: FirProperty::Body,
            index: None,
        });
        let count = fields()
            .iter()
            .filter(|(_, reference, _)| *reference == body)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn table_sizes_are_read_from_the_query() {
        let items = slash_items("table 4x2", &fields(), false);
        assert_eq!(titles(&items), ["Table"]);
        assert_eq!(items[0].command, BlockCommand::table(4, 2));
        assert_eq!(items[0].detail.as_deref(), Some("4 × 2"));
        assert_eq!(
            slash_items("5×6", &fields(), false)[0].command,
            BlockCommand::table(5, 6)
        );
        assert_eq!(
            slash_items("table 99x99", &fields(), false)[0].command,
            BlockCommand::table(MAX_TABLE_SIDE, MAX_TABLE_SIDE)
        );
        assert_eq!(
            slash_items("table", &fields(), false)[0].detail.as_deref(),
            Some("3 × 3")
        );
        assert!(slash_items("quote 2x2", &fields(), false).is_empty());
    }

    #[test]
    fn page_breaks_are_hidden_inside_tables() {
        assert!(titles(&slash_items("page", &fields(), false)).contains(&"Page break"));
        assert!(!titles(&slash_items("page", &fields(), true)).contains(&"Page break"));
    }

    #[test]
    fn nonsense_matches_nothing() {
        assert!(slash_items("zzqx", &fields(), false).is_empty());
    }
}
