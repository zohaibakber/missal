use std::collections::HashSet;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;

use gpui_kit::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Render, ScrollStrategy, SharedString,
    StatefulInteractiveElement, Styled, Subscription, UniformListScrollHandle, WeakEntity, Window,
    div, px, uniform_list,
};
use missal_core::{FirId, FirStatus};
use missal_text::{TextInput, TextInputEvent, label};

use crate::app::navigate;
use crate::prefs::{Column, Sort};
use crate::screens::edit_sheet::open_edit_sheet;
use crate::shortcuts::*;
use crate::state::firs::FirsEvent;
use crate::state::rows::{FirRow, ListIndex};
use crate::state::{Load, Models, Route, run_db_in, toast_error, toast_success};
use crate::theme::{Palette, ROW_HEIGHT};
use crate::ui::dialogs::confirm_delete;
use crate::ui::input::{Kind, text_input};
use crate::ui::status::status_dot;
use crate::ui::{empty_state, key_cap, kit_icon_button, pane_header, small_icon, status_bar};

const SELECT_WIDTH: f32 = 44.0;
const ACTIONS_WIDTH: f32 = 48.0;

fn column_width(column: Column) -> Option<f32> {
    match column {
        Column::FirNo => Some(120.0),
        Column::Offence => None,
        Column::Status => Some(172.0),
        Column::Date | Column::IncidentDate | Column::ArrestDate => Some(124.0),
    }
}

pub struct FirListScreen {
    models: Models,
    focus: FocusHandle,
    search: Entity<TextInput>,
    query: String,
    sort: Sort,
    statuses: Vec<FirStatus>,
    hidden: Vec<Column>,
    visible: Vec<u32>,
    index: ListIndex,
    stale: bool,
    selected: HashSet<FirId>,
    focused: Option<usize>,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl FirListScreen {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> FirListScreen {
        let prefs = models.prefs.read(cx).read().table.clone();
        let search = text_input(window, cx, Kind::UrduAuto, "Search FIRs", "");
        let subscriptions = vec![
            cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                if *event == TextInputEvent::Change {
                    this.query = input.read(cx).text().to_owned();
                    this.stale = true;
                    this.focused = None;
                    cx.notify();
                }
            }),
            cx.subscribe(&models.firs, |this, _, _: &FirsEvent, cx| {
                this.stale = true;
                this.prune_selection(cx);
                cx.notify();
            }),
        ];
        FirListScreen {
            focus: cx.focus_handle(),
            search,
            query: String::new(),
            sort: prefs.sort,
            statuses: prefs.statuses,
            hidden: prefs.hidden,
            visible: Vec::new(),
            index: ListIndex::default(),
            stale: true,
            selected: HashSet::new(),
            focused: None,
            scroll: UniformListScrollHandle::new(),
            models,
            _subscriptions: subscriptions,
        }
    }

    pub fn is_dirty(&self, _: &App) -> bool {
        false
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn search_input(&self, _: &App) -> Entity<TextInput> {
        self.search.clone()
    }

    pub fn scroll_to_row(&mut self, index: usize, cx: &mut Context<Self>) {
        self.scroll
            .scroll_to_item_strict(index, ScrollStrategy::Top);
        cx.notify();
    }

    pub fn visible_count(&self) -> usize {
        self.visible.len()
    }

    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    pub fn visible_numbers(&self, cx: &App) -> Vec<String> {
        let rows = &self.models.firs.read(cx).rows;
        self.visible
            .iter()
            .map(|&index| rows[index as usize].no.to_string())
            .collect()
    }

    pub fn sort(&self) -> Sort {
        self.sort
    }

    fn prune_selection(&mut self, cx: &App) {
        let firs = self.models.firs.read(cx);
        if self.selected.is_empty() {
            return;
        }
        let present: HashSet<FirId> = firs.rows.iter().map(|row| row.id).collect();
        self.selected.retain(|id| present.contains(id));
    }

    fn ensure_derived(&mut self, cx: &App) {
        if !self.stale {
            return;
        }
        self.stale = false;
        let firs = self.models.firs.read(cx);
        let shown = self.index.update(
            &firs.rows,
            firs.version(),
            &self.query,
            &self.statuses,
            self.sort,
        );
        self.visible.clear();
        self.visible.extend_from_slice(shown);
        if self
            .focused
            .is_some_and(|index| index >= self.visible.len())
        {
            self.focused = None;
        }
    }

    fn filtered(&self) -> bool {
        !self.query.trim().is_empty() || !self.statuses.is_empty()
    }

    fn save_table_prefs(&self, cx: &mut Context<Self>) {
        let (sort, statuses, hidden) = (self.sort, self.statuses.clone(), self.hidden.clone());
        self.models.prefs.update(cx, |prefs, cx| {
            prefs.update(cx, |preferences| {
                preferences.table.sort = sort;
                preferences.table.statuses = statuses;
                preferences.table.hidden = hidden;
            })
        });
    }

    pub fn toggle_sort(&mut self, column: Column, cx: &mut Context<Self>) {
        self.sort = if self.sort.column == column {
            Sort {
                column,
                descending: !self.sort.descending,
            }
        } else {
            Sort {
                column,
                descending: false,
            }
        };
        self.stale = true;
        self.save_table_prefs(cx);
        cx.notify();
    }

    pub fn set_search(&mut self, text: &str, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| input.set_text(text, cx));
    }

    pub fn toggle_status(&mut self, status: FirStatus, cx: &mut Context<Self>) {
        match self
            .statuses
            .iter()
            .position(|existing| *existing == status)
        {
            Some(position) => {
                self.statuses.remove(position);
            }
            None => self.statuses.push(status),
        }
        self.stale = true;
        self.save_table_prefs(cx);
        cx.notify();
    }

    pub fn clear_status_filter(&mut self, cx: &mut Context<Self>) {
        self.statuses.clear();
        self.stale = true;
        self.save_table_prefs(cx);
        cx.notify();
    }

    pub fn toggle_column(&mut self, column: Column, cx: &mut Context<Self>) {
        match self.hidden.iter().position(|existing| *existing == column) {
            Some(position) => {
                self.hidden.remove(position);
            }
            None => self.hidden.push(column),
        }
        self.save_table_prefs(cx);
        cx.notify();
    }

    pub fn clear_filters(&mut self, cx: &mut Context<Self>) {
        self.statuses.clear();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        self.stale = true;
        self.save_table_prefs(cx);
        cx.notify();
    }

    fn row_at(&self, index: usize, cx: &App) -> Option<FirRow> {
        let position = *self.visible.get(index)?;
        self.models
            .firs
            .read(cx)
            .rows
            .get(position as usize)
            .cloned()
    }

    pub fn toggle_selected(&mut self, id: FirId, cx: &mut Context<Self>) {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
        cx.notify();
    }

    pub fn select_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.row_at(index, cx) {
            self.toggle_selected(row.id, cx);
        }
    }

    pub fn toggle_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<FirId> = {
            let rows = &self.models.firs.read(cx).rows;
            self.visible
                .iter()
                .map(|&index| rows[index as usize].id)
                .collect()
        };
        if !ids.is_empty() && ids.iter().all(|id| self.selected.contains(id)) {
            self.selected.clear();
        } else {
            self.selected.extend(ids);
        }
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selected.clear();
        cx.notify();
    }

    fn move_focus(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.visible.is_empty() {
            return;
        }
        let last = self.visible.len() - 1;
        let next = match self.focused {
            None => {
                if delta >= 0 {
                    0
                } else {
                    last
                }
            }
            Some(current) => (current as isize + delta).clamp(0, last as isize) as usize,
        };
        self.focused = Some(next);
        self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
        cx.notify();
    }

    pub fn open_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self.row_at(index, cx) {
            self.focused = Some(index);
            navigate(Route::Fir(row.id), window, cx);
        }
    }

    pub fn edit_fir(&mut self, id: FirId, window: &mut Window, cx: &mut Context<Self>) {
        run_db_in(
            window,
            cx,
            move |store| store.fir(id),
            |_, result, window, cx| match result {
                Ok(record) => open_edit_sheet(record, window, cx),
                Err(error) => toast_error(window, cx, error.message()),
            },
        );
    }

    fn edit_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self
            .focused
            .and_then(|index| self.row_at(index, cx))
            .map(|row| row.id)
            .or_else(|| {
                (self.selected.len() == 1).then(|| {
                    *self
                        .selected
                        .iter()
                        .next()
                        .unwrap_or_else(|| unreachable!())
                })
            });
        if let Some(id) = target {
            self.edit_fir(id, window, cx);
        }
    }

    pub fn request_delete(&mut self, ids: Vec<FirId>, window: &mut Window, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        let title = if ids.len() == 1 {
            let no = self
                .models
                .firs
                .read(cx)
                .get(ids[0])
                .map_or_else(|| SharedString::from(""), |row| row.no.clone());
            format!("Delete FIR {no}?")
        } else {
            format!("Delete {} FIRs?", ids.len())
        };
        let description = if ids.len() == 1 {
            "Every document written for this FIR is deleted too. This can't be undone."
        } else {
            "Every document written for these FIRs is deleted too. This can't be undone."
        };
        let screen = cx.entity().downgrade();
        let label = if ids.len() == 1 {
            "Delete FIR"
        } else {
            "Delete FIRs"
        };
        confirm_delete(window, cx, title, description, label, move |window, cx| {
            screen
                .update(cx, |this, cx| this.delete(ids.clone(), window, cx))
                .ok();
        });
    }

    pub fn delete(&mut self, ids: Vec<FirId>, window: &mut Window, cx: &mut Context<Self>) {
        let job_ids = ids.clone();
        run_db_in(
            window,
            cx,
            move |store| store.remove_firs(&job_ids),
            move |this, result, window, cx| match result {
                Ok(()) => {
                    let count = ids.len();
                    this.models
                        .firs
                        .update(cx, |firs, cx| firs.remove(&ids, cx));
                    this.selected.clear();
                    this.focused = None;
                    let message = if count == 1 {
                        "FIR deleted".to_owned()
                    } else {
                        format!("{count} FIRs deleted")
                    };
                    toast_success(window, cx, message);
                    this.focus(window, cx);
                }
                Err(error) => toast_error(window, cx, error.message()),
            },
        );
    }

    fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<FirId> = self.selected.iter().copied().collect();
        self.request_delete(ids, window, cx);
    }

    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| {
            input.focus(window, cx);
            let length = input.text().len();
            input.select(0..length, cx);
        });
    }

    fn header(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = *Palette::get(cx);
        let screen = cx.entity();
        let has_query = !self.query.is_empty();
        let filtered = self.filtered();
        let status_selected = self.statuses.clone();
        let hidden = self.hidden.clone();
        let focus_is_search = self.search.focus_handle(cx).is_focused(window);
        pane_header(&palette)
            .child(
                h_flex()
                    .id("fir-search")
                    .w(px(264.))
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.as_str();
                        let plain = !event.keystroke.modifiers.modified();
                        if key == "escape" && plain {
                            if this.query.is_empty() {
                                this.focus(window, cx);
                            } else {
                                this.set_search("", cx);
                            }
                            cx.stop_propagation();
                        } else if (key == "down" || key == "enter")
                            && plain
                            && !this.visible.is_empty()
                        {
                            this.focus(window, cx);
                            this.move_focus(1, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(div().flex_1().min_w_0().child(self.search.clone()))
                    .child(if has_query {
                        kit_icon_button("clear-search", IconName::X)
                            .tooltip("Clear search")
                            .on_click(cx.listener(|this, _, _, cx| this.set_search("", cx)))
                            .into_any_element()
                    } else if focus_is_search {
                        div().into_any_element()
                    } else {
                        key_cap(&palette, "/").into_any_element()
                    }),
            )
            .child(div().flex_1())
            .children(filtered.then(|| {
                Button::new("clear-filters")
                    .ghost()
                    .small()
                    .label("Clear")
                    .on_click(cx.listener(|this, _, _, cx| this.clear_filters(cx)))
            }))
            .child(self.status_filter(&palette, status_selected, screen.clone()))
            .child(
                Button::new("columns")
                    .outline()
                    .small()
                    .icon(IconName::Columns3)
                    .label("Columns")
                    .dropdown_menu({
                        let weak = screen.downgrade();
                        move |menu, _, _| {
                            Column::ALL.iter().fold(menu, |menu, column| {
                                let column = *column;
                                let weak = weak.clone();
                                menu.item(
                                    PopupMenuItem::element(move |_, _| {
                                        div().text_size(px(14.)).child(label(column.title()).rtl())
                                    })
                                    .checked(!hidden.contains(&column))
                                    .on_click(
                                        move |_, _, cx| {
                                            weak.update(cx, |this, cx| {
                                                this.toggle_column(column, cx)
                                            })
                                            .ok();
                                        },
                                    ),
                                )
                            })
                        }
                    }),
            )
            .child(
                Button::new("header-new-fir")
                    .primary()
                    .small()
                    .icon(IconName::Plus)
                    .label("New FIR")
                    .on_click(|_, window, cx| navigate(Route::NewFir, window, cx)),
            )
            .into_any_element()
    }

    fn status_filter(
        &self,
        palette: &Palette,
        selected: Vec<FirStatus>,
        screen: Entity<Self>,
    ) -> impl IntoElement {
        let palette = *palette;
        let active = !selected.is_empty();
        let trigger = Button::new("status-filter")
            .small()
            .icon(IconName::ListFilter)
            .when(active, |this| this.secondary())
            .when(!active, |this| this.outline())
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child("Status")
                    .children(selected.iter().map(|status| status_dot(&palette, *status))),
            );
        Popover::new("status-filter-popover")
            .trigger(trigger)
            .anchor(gpui_kit::Anchor::TopRight)
            .p_1()
            .content(move |_, _, cx| {
                let selected = screen.read(cx).statuses.clone();
                let screen = screen.clone();
                v_flex()
                    .w(px(200.))
                    .gap_0p5()
                    .children(FirStatus::ALL.iter().map(|status| {
                        let status = *status;
                        let checked = selected.contains(&status);
                        let screen = screen.clone();
                        h_flex()
                            .id(("status-option", status as usize))
                            .test_support()
                            .h(px(34.))
                            .px_2()
                            .gap_2()
                            .items_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(move |style| style.bg(palette.hover))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                screen.update(cx, |this, cx| this.toggle_status(status, cx));
                                cx.notify();
                            }))
                            .child(div().w(px(16.)).children(
                                checked.then(|| small_icon(IconName::Check, palette.ink)),
                            ))
                            .child(status_dot(&palette, status))
                            .child(div().text_size(px(14.)).child(label(status.label()).rtl()))
                    }))
                    .when(!selected.is_empty(), |this| {
                        let screen = screen.clone();
                        this.child(
                            div()
                                .id("status-clear")
                                .h(px(30.))
                                .px_2()
                                .flex()
                                .items_center()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .text_size(px(13.))
                                .text_color(palette.ink_muted)
                                .hover(move |style| style.bg(palette.hover))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    screen.update(cx, |this, cx| this.clear_status_filter(cx));
                                    cx.notify();
                                }))
                                .child("Clear filter"),
                        )
                    })
            })
    }

    fn header_row(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let all_selected = !self.visible.is_empty() && {
            let rows = &self.models.firs.read(cx).rows;
            self.visible
                .iter()
                .all(|&index| self.selected.contains(&rows[index as usize].id))
        };
        let any_selected = !self.selected.is_empty();
        h_flex()
            .id("fir-table-head")
            .flex_row_reverse()
            .flex_shrink_0()
            .h(px(40.))
            .items_center()
            .border_b_1()
            .border_color(palette.line)
            .text_size(px(12.))
            .text_color(palette.ink_muted)
            .child(
                div()
                    .w(px(SELECT_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .child(
                        Checkbox::new("select-all")
                            .checked(all_selected)
                            .accessibility_label("Select all FIRs")
                            .when(!any_selected, |this| this)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_all(cx))),
                    ),
            )
            .children(
                Column::ALL
                    .iter()
                    .filter(|column| !self.hidden.contains(column))
                    .map(|column| {
                        let column = *column;
                        let sorted = self.sort.column == column;
                        let icon = if !sorted {
                            IconName::ArrowUpDown
                        } else if self.sort.descending {
                            IconName::ArrowDown
                        } else {
                            IconName::ArrowUp
                        };
                        let cell = h_flex()
                            .id(("sort", column as usize))
                            .test_support()
                            .flex_row_reverse()
                            .h_full()
                            .px_2()
                            .gap_1p5()
                            .items_center()
                            .cursor_pointer()
                            .when(sorted, |this| this.text_color(palette.ink))
                            .hover({
                                let foreground = palette.ink;
                                move |style| style.text_color(foreground)
                            })
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.toggle_sort(column, cx)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_medium()
                                    .child(label(column.title()).rtl()),
                            )
                            .child(small_icon(
                                icon,
                                if sorted {
                                    palette.ink
                                } else {
                                    palette.ink_muted.opacity(0.7)
                                },
                            ));
                        match column_width(column) {
                            Some(width) => cell.w(px(width)).flex_shrink_0(),
                            None => cell.flex_1().min_w_0(),
                        }
                    }),
            )
            .child(div().w(px(ACTIONS_WIDTH)).flex_shrink_0())
            .into_any_element()
    }

    fn render_row(
        &self,
        index: usize,
        palette: &Palette,
        any_selected: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let row = self.row_at(index, cx)?;
        let id = row.id;
        let selected = self.selected.contains(&id);
        let focused = self.focused == Some(index);
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let palette = *palette;
        let element = h_flex()
            .id(("fir-row", index))
            .test_support()
            .group("fir-row")
            .w_full()
            .flex_row_reverse()
            .h(px(ROW_HEIGHT))
            .items_center()
            .border_b_1()
            .border_color(palette.line)
            .cursor_pointer()
            .when(selected, |this| this.bg(palette.hover))
            .when(focused, |this| {
                this.bg(palette.hover)
                    .border_l_2()
                    .border_color(palette.accent)
            })
            .hover(move |style| style.bg(palette.hover.opacity(0.7)))
            .on_click(cx.listener(move |this, _, window, cx| this.open_row(index, window, cx)))
            .child(
                div()
                    .w(px(SELECT_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .when(!any_selected && !selected && !focused, |this| {
                                this.invisible()
                            })
                            .group_hover("fir-row", |style| style.visible())
                            .child(
                                Checkbox::new(("row-select", index))
                                    .checked(selected)
                                    .accessibility_label("Select FIR")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_selected(id, cx)
                                    })),
                            ),
                    ),
            )
            .children(
                Column::ALL
                    .iter()
                    .filter(|column| !self.hidden.contains(column))
                    .map(|column| {
                        let cell = div()
                            .h_full()
                            .px_2()
                            .flex()
                            .items_center()
                            .overflow_hidden();
                        let cell = match column_width(*column) {
                            Some(width) => cell.w(px(width)).flex_shrink_0(),
                            None => cell.flex_1().min_w_0(),
                        };
                        match column {
                            Column::FirNo => cell
                                .text_size(px(14.))
                                .font_medium()
                                .child(label(row.no.clone()).rtl().truncate()),
                            Column::Offence => cell
                                .text_size(px(17.))
                                .child(label(row.offence.clone()).rtl().truncate()),
                            Column::Status => cell
                                .gap_2()
                                .flex_row_reverse()
                                .justify_start()
                                .text_size(px(16.))
                                .child(status_dot(&palette, row.status))
                                .child(label(row.status.label()).rtl()),
                            Column::Date => cell
                                .text_size(px(13.))
                                .text_color(palette.ink_muted)
                                .child(label(row.date.clone()).rtl()),
                            Column::IncidentDate => cell
                                .text_size(px(13.))
                                .text_color(palette.ink_muted)
                                .child(label(row.incident.clone()).rtl()),
                            Column::ArrestDate => {
                                cell.text_size(px(13.)).text_color(palette.ink_muted).child(
                                    label(if row.arrest.is_empty() {
                                        SharedString::from("—")
                                    } else {
                                        row.arrest.clone()
                                    })
                                    .rtl(),
                                )
                            }
                        }
                    }),
            )
            .child(
                div()
                    .w(px(ACTIONS_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .invisible()
                            .group_hover("fir-row", |style| style.visible())
                            .child(
                                Button::new(("row-actions", index))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Ellipsis)
                                    .dropdown_menu({
                                        let weak = weak.clone();
                                        move |menu, _, _| row_menu(menu, weak.clone(), id)
                                    }),
                            ),
                    ),
            );
        Some(
            element
                .context_menu({
                    let weak = weak.clone();
                    move |menu, _, _| row_menu(menu, weak.clone(), id)
                })
                .into_any_element(),
        )
    }

    fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = *Palette::get(cx);
        let firs = self.models.firs.read(cx);
        let total = firs.rows.len();
        match &firs.load {
            Load::Loading => v_flex()
                .size_full()
                .children((0..8usize).map(|index| {
                    h_flex()
                        .id(("skeleton-row", index))
                        .h(px(ROW_HEIGHT))
                        .px_4()
                        .gap_4()
                        .items_center()
                        .border_b_1()
                        .border_color(palette.line)
                        .child(crate::ui::skeleton_bar(&palette, 90., 12.))
                        .child(crate::ui::skeleton_bar(&palette, 220., 12.))
                        .child(crate::ui::skeleton_bar(&palette, 120., 12.))
                }))
                .into_any_element(),
            Load::Failed(message) => empty_state(
                &palette,
                IconName::TriangleAlert,
                "Could not load FIRs",
                Some(format!("The database could not be read. Restart Missal. ({message})").into()),
                Some(
                    Button::new("retry-load")
                        .outline()
                        .label("Try again")
                        .on_click({
                            let firs = self.models.firs.clone();
                            move |_, _, cx| firs.update(cx, |firs, cx| firs.reload(cx))
                        })
                        .into_any_element(),
                ),
            )
            .into_any_element(),
            Load::Ready if total == 0 => empty_state(
                &palette,
                IconName::Folder,
                "No FIRs yet",
                Some("Create the first FIR to start writing its documents.".into()),
                Some(
                    Button::new("empty-new-fir")
                        .primary()
                        .icon(IconName::Plus)
                        .label("New FIR")
                        .on_click(|_, window, cx| navigate(Route::NewFir, window, cx))
                        .into_any_element(),
                ),
            )
            .into_any_element(),
            Load::Ready => {
                self.ensure_derived(cx);
                if self.visible.is_empty() {
                    return empty_state(
                        &palette,
                        IconName::Search,
                        "No matching FIRs",
                        Some("Try a different search or clear the filters.".into()),
                        Some(
                            Button::new("empty-clear")
                                .outline()
                                .label("Clear filters")
                                .on_click(cx.listener(|this, _, _, cx| this.clear_filters(cx)))
                                .into_any_element(),
                        ),
                    )
                    .into_any_element();
                }
                let count = self.visible.len();
                let _ = window;
                v_flex()
                    .size_full()
                    .child(self.header_row(&palette, cx))
                    .child(
                        uniform_list(
                            "fir-rows",
                            count,
                            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                let palette = *Palette::get(cx);
                                let any_selected = !this.selected.is_empty();
                                range
                                    .filter_map(|index| {
                                        this.render_row(index, &palette, any_selected, cx)
                                    })
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .track_scroll(&self.scroll)
                        .flex_1()
                        .min_h_0(),
                    )
                    .into_any_element()
            }
        }
    }

    fn footer(&self, palette: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let total = self.models.firs.read(cx).rows.len();
        let selected = self.selected.len();
        if selected > 0 {
            return Some(
                status_bar(palette)
                    .child(div().child(format!("{selected} selected")))
                    .child(
                        Button::new("clear-selection")
                            .ghost()
                            .xsmall()
                            .label("Clear")
                            .on_click(cx.listener(|this, _, _, cx| this.clear_selection(cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("delete-selected")
                            .danger()
                            .xsmall()
                            .icon(IconName::Trash)
                            .label(if selected == 1 {
                                "Delete FIR".to_owned()
                            } else {
                                format!("Delete {selected} FIRs")
                            })
                            .on_click(
                                cx.listener(|this, _, window, cx| this.delete_selected(window, cx)),
                            ),
                    )
                    .into_any_element(),
            );
        }
        if self.filtered() && total > 0 {
            return Some(
                status_bar(palette)
                    .child(format!("{} of {total} FIRs", self.visible.len()))
                    .into_any_element(),
            );
        }
        None
    }
}

fn row_menu(
    menu: gpui_kit::component::menu::PopupMenu,
    weak: WeakEntity<FirListScreen>,
    id: FirId,
) -> gpui_kit::component::menu::PopupMenu {
    let edit = weak.clone();
    let delete = weak;
    menu.item(
        PopupMenuItem::new("Open")
            .icon(IconName::FolderOpen)
            .on_click(move |_, window, cx| navigate(Route::Fir(id), window, cx)),
    )
    .item(
        PopupMenuItem::new("Edit details")
            .icon(IconName::Pencil)
            .on_click(move |_, window, cx| {
                edit.update(cx, |this, cx| this.edit_fir(id, window, cx))
                    .ok();
            }),
    )
    .item(PopupMenuItem::separator())
    .item(
        PopupMenuItem::new("Delete")
            .icon(IconName::Trash)
            .on_click(move |_, window, cx| {
                delete
                    .update(cx, |this, cx| this.request_delete(vec![id], window, cx))
                    .ok();
            }),
    )
}

impl Focusable for FirListScreen {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FirListScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let body = self.body(window, cx);
        let footer = self.footer(&palette, cx);
        v_flex()
            .id("fir-list")
            .test_support()
            .key_context(FIR_LIST)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &NextRow, _, cx| this.move_focus(1, cx)))
            .on_action(cx.listener(|this, _: &PreviousRow, _, cx| this.move_focus(-1, cx)))
            .on_action(cx.listener(|this, _: &ToggleRow, _, cx| {
                if let Some(index) = this.focused {
                    this.select_row(index, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &OpenRow, window, cx| {
                if let Some(index) = this.focused {
                    this.open_row(index, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &EditFir, window, cx| this.edit_focused(window, cx)))
            .on_action(cx.listener(|this, _: &DeleteSelection, window, cx| {
                let ids: Vec<FirId> = if this.selected.is_empty() {
                    this.focused
                        .and_then(|index| this.row_at(index, cx))
                        .map(|row| vec![row.id])
                        .unwrap_or_default()
                } else {
                    this.selected.iter().copied().collect()
                };
                if !ids.is_empty() {
                    this.request_delete(ids, window, cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)),
            )
            .child(self.header(window, cx))
            .child(div().flex_1().min_h_0().child(body))
            .children(footer)
    }
}

impl crate::ui::titlebar::HeaderSource for FirListScreen {
    fn header(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::ui::titlebar::HeaderContent {
        let total = self.models.firs.read(cx).rows.len();
        let visible = self.visible_count();
        let meta = if visible == total {
            total.to_string()
        } else {
            format!("{visible} of {total}")
        };
        crate::ui::titlebar::HeaderContent::new("FIRs")
            .meta(meta)
            .action(
                crate::ui::primary_sm("header-new-fir", "New FIR")
                    .icon(IconName::Plus)
                    .keys("N")
                    .on_click(|_, window, cx| navigate(Route::NewFir, window, cx)),
            )
    }
}
