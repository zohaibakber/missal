use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    Anchor, AnyElement, App, Bounds, Context, Entity, FocusHandle, Focusable, FontFeatures,
    FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Point, Render, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled,
    Subscription, UniformListScrollHandle, Window, canvas, div, point, px, uniform_list,
};
use missal_core::{FirId, FirStatus};
use missal_text::input::{Enter, MoveDown};
use missal_text::{Align, LineHeight, TextInput, TextInputEvent, label};

use crate::app::navigate;
use crate::prefs::{Column, Sort};
use crate::shortcuts::*;
use crate::state::firs::FirsEvent;
use crate::state::rows::ListIndex;
use crate::state::{Load, Models, PrintTarget, Route, run_db_in, toast_error, toast_success};
use crate::theme::{FILTER_ITEM, ICON_XS, PAGE_GUTTER, Palette, RADIUS, TABLE_HEAD, TABLE_ROW};
use crate::ui::dialogs::{Choice, Tone, open_choice};
use crate::ui::input::{Hint, Kind, field_input, search_field};
use crate::ui::popup_menu::{Dismissed, Placement, PopupEntry, PopupList, PopupRow};
use crate::ui::status::status_dot;
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::{
    ButtonKind, UrRole, button, checkbox, danger_outline, empty_state, icon_button, isolate_urdu,
    sized_icon, skeleton_bar, ur_label,
};

const SELECT_WIDTH: f32 = 36.0;
const ACTIONS_WIDTH: f32 = 36.0;
const TOOLBAR_HEIGHT: f32 = 52.0;
const SEARCH_WIDTH: f32 = 300.0;
const SKELETON_DELAY: Duration = Duration::from_millis(150);

enum Width {
    Fixed(f32),
    Flex(f32),
}

fn column_width(column: Column) -> Width {
    match column {
        Column::FirNo => Width::Fixed(108.0),
        Column::Offence => Width::Flex(1.1),
        Column::Accused => Width::Flex(1.0),
        Column::Status => Width::Fixed(132.0),
        Column::Date | Column::IncidentDate | Column::ArrestDate => Width::Fixed(112.0),
    }
}

fn sized<E: Styled>(element: E, column: Column) -> E {
    match column_width(column) {
        Width::Fixed(width) => element.w(px(width)).flex_none(),
        Width::Flex(grow) => {
            let mut element = element.min_w_0();
            let style = element.style();
            style.flex_grow = Some(grow);
            style.flex_shrink = Some(1.);
            style.flex_basis = Some(px(0.).into());
            element
        }
    }
}

fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

const STATUS_ITEMS: [FirStatus; 4] = [
    FirStatus::Open,
    FirStatus::UnderInvestigation,
    FirStatus::ChallanSubmitted,
    FirStatus::Closed,
];

#[derive(Clone)]
struct RowView {
    id: FirId,
    no: SharedString,
    offence: SharedString,
    accused: SharedString,
    accused_more: u32,
    status: FirStatus,
    date: SharedString,
    incident: SharedString,
    arrest: SharedString,
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
    menu: Option<Entity<PopupList>>,
    columns_bounds: Rc<Cell<Bounds<Pixels>>>,
    focused_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    skeleton: bool,
    _subscriptions: Vec<Subscription>,
}

impl FirListScreen {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> FirListScreen {
        let prefs = models.prefs.read(cx).read().table.clone();
        let search = field_input(window, cx, Kind::Search, "FIR no., offence or name", "");
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
        if !models.firs.read(cx).load.is_ready() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SKELETON_DELAY).await;
                this.update(cx, |this, cx| {
                    this.skeleton = true;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        let mut statuses = prefs.statuses;
        statuses.truncate(1);
        FirListScreen {
            focus: cx.focus_handle(),
            search,
            query: String::new(),
            sort: prefs.sort,
            statuses,
            hidden: prefs.hidden,
            visible: Vec::new(),
            index: ListIndex::default(),
            stale: true,
            selected: HashSet::new(),
            focused: None,
            scroll: UniformListScrollHandle::new(),
            menu: None,
            columns_bounds: Rc::new(Cell::new(Bounds::default())),
            focused_bounds: Rc::new(Cell::new(None)),
            skeleton: false,
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

    pub fn focused_row(&self) -> Option<usize> {
        self.focused
    }

    pub fn status_filter(&self) -> Option<FirStatus> {
        self.statuses.first().copied()
    }

    pub fn hidden_columns(&self) -> &[Column] {
        &self.hidden
    }

    pub fn menu_labels(&self, cx: &App) -> Option<Vec<String>> {
        self.menu.as_ref().map(|menu| menu.read(cx).labels())
    }

    pub fn status_count(&mut self, status: Option<FirStatus>, cx: &App) -> usize {
        self.ensure_derived(cx);
        match status {
            Some(status) => self.index.status_count(status),
            None => self.index.matched_count(),
        }
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
        if self.selected.is_empty() {
            return;
        }
        let firs = self.models.firs.read(cx);
        let present: HashSet<FirId> = firs.rows.iter().map(|row| row.id).collect();
        self.selected.retain(|id| present.contains(id));
    }

    fn ensure_derived(&mut self, cx: &App) {
        if !self.stale {
            return;
        }
        let firs = self.models.firs.read(cx);
        if !firs.load.is_ready() {
            return;
        }
        self.stale = false;
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

    pub fn set_status_filter(&mut self, status: Option<FirStatus>, cx: &mut Context<Self>) {
        let next: Vec<FirStatus> = status.into_iter().collect();
        if next == self.statuses {
            return;
        }
        self.statuses = next;
        self.focused = None;
        self.stale = true;
        self.save_table_prefs(cx);
        cx.notify();
    }

    pub fn toggle_status(&mut self, status: FirStatus, cx: &mut Context<Self>) {
        let next = (self.status_filter() != Some(status)).then_some(status);
        self.set_status_filter(next, cx);
    }

    pub fn clear_status_filter(&mut self, cx: &mut Context<Self>) {
        self.set_status_filter(None, cx);
    }

    pub fn toggle_column(&mut self, column: Column, cx: &mut Context<Self>) {
        if column == Column::FirNo {
            return;
        }
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

    fn row_id(&self, index: usize, cx: &App) -> Option<FirId> {
        let position = *self.visible.get(index)?;
        self.models
            .firs
            .read(cx)
            .rows
            .get(position as usize)
            .map(|row| row.id)
    }

    fn row_view(&self, index: usize, cx: &App) -> Option<RowView> {
        let position = *self.visible.get(index)?;
        let row = self.models.firs.read(cx).rows.get(position as usize)?;
        Some(RowView {
            id: row.id,
            no: row.no.clone(),
            offence: row.offence.clone(),
            accused: row.accused.clone(),
            accused_more: row.accused_more,
            status: row.status,
            date: row.date.clone(),
            incident: row.incident.clone(),
            arrest: row.arrest.clone(),
        })
    }

    pub fn toggle_selected(&mut self, id: FirId, cx: &mut Context<Self>) {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
        cx.notify();
    }

    pub fn select_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(id) = self.row_id(index, cx) {
            self.toggle_selected(id, cx);
        }
    }

    fn all_visible_selected(&self, cx: &App) -> bool {
        let rows = &self.models.firs.read(cx).rows;
        !self.visible.is_empty()
            && self
                .visible
                .iter()
                .all(|&index| self.selected.contains(&rows[index as usize].id))
    }

    pub fn toggle_all(&mut self, cx: &mut Context<Self>) {
        if self.all_visible_selected(cx) {
            self.selected.clear();
        } else {
            self.select_all(cx);
        }
        cx.notify();
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let rows = &self.models.firs.read(cx).rows;
        self.selected
            .extend(self.visible.iter().map(|&index| rows[index as usize].id));
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selected.clear();
        cx.notify();
    }

    pub fn move_focus(&mut self, delta: isize, cx: &mut Context<Self>) {
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
        if let Some(id) = self.row_id(index, cx) {
            self.focused = Some(index);
            navigate(Route::Fir(id), window, cx);
        }
    }

    pub fn edit_fir(&mut self, id: FirId, window: &mut Window, cx: &mut Context<Self>) {
        navigate(Route::EditFir(id), window, cx);
    }

    fn target(&self, cx: &App) -> Option<FirId> {
        self.focused
            .and_then(|index| self.row_id(index, cx))
            .or_else(|| {
                (self.selected.len() == 1)
                    .then(|| self.selected.iter().next().copied())
                    .flatten()
            })
    }

    fn edit_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.target(cx) {
            self.edit_fir(id, window, cx);
        }
    }

    pub fn print_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.target(cx) {
            navigate(Route::Print(PrintTarget::Fir(id)), window, cx);
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
            "Delete FIR".to_owned()
        } else {
            format!("Delete {} FIRs", ids.len())
        };
        open_choice(
            window,
            cx,
            title,
            description,
            vec![
                Choice::new("cancel", "Cancel", Tone::Neutral, |_, _| {}),
                Choice::new("confirm", label, Tone::Danger, move |window, cx| {
                    screen
                        .update(cx, |this, cx| this.delete(ids.clone(), window, cx))
                        .ok();
                }),
            ],
        );
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

    fn delete_focused_or_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<FirId> = if self.selected.is_empty() {
            self.focused
                .and_then(|index| self.row_id(index, cx))
                .into_iter()
                .collect()
        } else {
            self.selected.iter().copied().collect()
        };
        self.request_delete(ids, window, cx);
    }

    fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| {
            input.focus(window, cx);
            let length = input.text().len();
            input.select(0..length, cx);
        });
    }

    fn leave_search(&mut self, first: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.visible.is_empty() {
            return;
        }
        self.focus(window, cx);
        if first || self.focused.is_none() {
            self.focused = None;
            self.move_focus(1, cx);
        }
        cx.stop_propagation();
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.selected.is_empty() {
            self.clear_selection(cx);
            return true;
        }
        if !self.query.is_empty() {
            self.set_search("", cx);
            return true;
        }
        let _ = window;
        false
    }

    fn open_menu(&mut self, menu: Entity<PopupList>, window: &mut Window, cx: &mut Context<Self>) {
        cx.subscribe_in(&menu, window, |this, _, _: &Dismissed, window, cx| {
            if this.menu.take().is_some() {
                this.focus(window, cx);
                cx.notify();
            }
        })
        .detach();
        self.menu = Some(menu);
        cx.notify();
    }

    pub fn open_columns_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let rows = Column::ALL
            .iter()
            .filter(|column| **column != Column::FirNo)
            .map(|column| {
                let column = *column;
                let weak = weak.clone();
                PopupRow::Entry(
                    PopupEntry::urdu(
                        SharedString::from(format!("column-{}", column.key())),
                        column.title(),
                        move |_, cx| {
                            weak.update(cx, |this, cx| this.toggle_column(column, cx))
                                .ok();
                        },
                    )
                    .check(!self.hidden.contains(&column))
                    .keep_open(),
                )
            })
            .collect();
        let bounds = self.columns_bounds.get();
        let position = point(bounds.right(), bounds.bottom() + px(4.));
        let menu = PopupList::open(
            "columns-menu",
            rows,
            Placement {
                position,
                anchor: Anchor::TopRight,
                width: 200.,
            },
            window,
            cx,
        );
        self.open_menu(menu, window, cx);
    }

    pub fn open_row_menu(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.row_id(index, cx) else {
            return;
        };
        self.focused = Some(index);
        let weak = cx.entity().downgrade();
        let edit = weak.clone();
        let delete = weak.clone();
        let rows = vec![
            PopupRow::Entry(
                PopupEntry::new("row-open", "Open", move |window, cx| {
                    navigate(Route::Fir(id), window, cx)
                })
                .icon(IconName::FolderOpen)
                .keys("Enter"),
            ),
            PopupRow::Entry(
                PopupEntry::new("row-edit", "Edit details", move |window, cx| {
                    edit.update(cx, |this, cx| this.edit_fir(id, window, cx))
                        .ok();
                })
                .icon(IconName::Pencil)
                .keys("E"),
            ),
            PopupRow::Entry(
                PopupEntry::new("row-print", "Print…", move |window, cx| {
                    navigate(Route::Print(PrintTarget::Fir(id)), window, cx)
                })
                .icon(IconName::Printer)
                .keys("Mod+P"),
            ),
            PopupRow::Separator,
            PopupRow::Entry(
                PopupEntry::new("row-delete", "Delete", move |window, cx| {
                    delete
                        .update(cx, |this, cx| this.request_delete(vec![id], window, cx))
                        .ok();
                })
                .icon(IconName::Trash)
                .keys("Delete")
                .danger(),
            ),
        ];
        let menu = PopupList::open(
            "row-menu",
            rows,
            Placement {
                position,
                anchor: Anchor::TopLeft,
                width: 220.,
            },
            window,
            cx,
        );
        self.open_menu(menu, window, cx);
    }

    fn open_focused_row_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let index = match self.focused {
            Some(index) => index,
            None if !self.visible.is_empty() => {
                self.move_focus(1, cx);
                0
            }
            None => return,
        };
        let position = match self.focused_bounds.get() {
            Some(bounds) => point(bounds.left() + px(8.), bounds.bottom() - px(6.)),
            None => window.mouse_position(),
        };
        self.open_row_menu(index, position, window, cx);
    }

    fn toolbar(&mut self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let filter = self.status_filter();
        let all_count = self.index.matched_count();
        let counts: Vec<usize> = STATUS_ITEMS
            .iter()
            .map(|status| self.index.status_count(*status))
            .collect();
        let item = |id: SharedString, on: bool, count: usize, content: Option<FirStatus>| {
            let hover = palette.hover;
            h_flex()
                .id(id)
                .test_support()
                .flex_none()
                .h(px(FILTER_ITEM))
                .px(px(10.))
                .gap(px(6.))
                .items_center()
                .rounded(px(RADIUS))
                .cursor_pointer()
                .text_size(px(13.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { palette.ink } else { palette.ink_muted })
                .when(on, |this| this.bg(palette.pressed))
                .when(!on, |this| this.hover(move |style| style.bg(hover)))
                .when(count == 0 && !on, |this| this.opacity(0.5))
                .role(gpui_kit::accesskit::Role::RadioButton)
                .when(on, |this| {
                    this.aria_toggled(gpui_kit::accesskit::Toggled::True)
                })
                .map(|this| match content {
                    None => this.child("All"),
                    Some(status) => this.child(status_dot(&palette, status)).child(
                        div().font_weight(FontWeight::NORMAL).mt(px(-3.)).child(
                            label(status.label())
                                .rtl()
                                .leading(LineHeight::Px(26.))
                                .text_size(px(15.)),
                        ),
                    ),
                })
                .child(
                    div()
                        .font_weight(FontWeight::NORMAL)
                        .font_features(tabular())
                        .text_color(if on {
                            palette.ink_muted
                        } else {
                            palette.ink_faint
                        })
                        .child(count.to_string()),
                )
        };
        let mut row = h_flex()
            .id("status-filter")
            .gap(px(2.))
            .items_center()
            .child(
                item("status-all".into(), filter.is_none(), all_count, None)
                    .on_click(cx.listener(|this, _, _, cx| this.set_status_filter(None, cx))),
            );
        for (position, status) in STATUS_ITEMS.into_iter().enumerate() {
            row = row.child(
                item(
                    SharedString::from(format!("status-{}", status.as_str())),
                    filter == Some(status),
                    counts[position],
                    Some(status),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_status(status, cx))),
            );
        }
        let columns_bounds = self.columns_bounds.clone();
        let menu_open = self
            .menu
            .as_ref()
            .is_some_and(|menu| menu.read(cx).labels().len() == Column::ALL.len() - 1);
        h_flex()
            .id("fir-toolbar")
            .flex_none()
            .h(px(TOOLBAR_HEIGHT))
            .px(px(PAGE_GUTTER))
            .gap(px(12.))
            .items_center()
            .child(
                div()
                    .id("fir-search")
                    .flex_none()
                    .capture_action(cx.listener(|this, _: &BlurFocus, window, cx| {
                        if !this.query.is_empty() {
                            this.set_search("", cx);
                        } else if !this.selected.is_empty() {
                            this.clear_selection(cx);
                        } else {
                            this.focus(window, cx);
                        }
                        cx.stop_propagation();
                    }))
                    .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                        this.leave_search(false, window, cx)
                    }))
                    .capture_action(cx.listener(|this, _: &Enter, window, cx| {
                        this.leave_search(true, window, cx)
                    }))
                    .child(
                        search_field("fir-search-field", &self.search, Kind::Search)
                            .width(px(SEARCH_WIDTH))
                            .hint(Hint::Slash),
                    ),
            )
            .child(row)
            .child(div().flex_1())
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(
                        canvas(
                            move |bounds, _, _| columns_bounds.set(bounds),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        icon_button("columns", IconName::Columns3, "Columns")
                            .toggled(menu_open)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_columns_menu(window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }

    fn header_row(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let palette = *palette;
        let all_selected = self.all_visible_selected(cx);
        h_flex()
            .id("fir-table-head")
            .flex_row_reverse()
            .flex_none()
            .h(px(TABLE_HEAD))
            .items_center()
            .border_b_1()
            .border_color(palette.line)
            .text_color(palette.ink_muted)
            .child(
                div()
                    .w(px(SELECT_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        checkbox(&palette, "select-all", all_selected)
                            .cursor_pointer()
                            .aria_label("Select all visible FIRs")
                            .test_support()
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
                        let icon = if self.sort.descending {
                            IconName::ArrowDown
                        } else {
                            IconName::ArrowUp
                        };
                        let ink = palette.ink;
                        sized(
                            h_flex()
                                .id(("sort", column as usize))
                                .test_support()
                                .flex_row_reverse()
                                .h_full()
                                .px(px(8.))
                                .gap(px(4.))
                                .items_center()
                                .cursor_pointer()
                                .hover(move |style| style.text_color(ink))
                                .role(gpui_kit::accesskit::Role::ColumnHeader)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.toggle_sort(column, cx)),
                                )
                                .child(
                                    div()
                                        .mt(px(-2.))
                                        .min_w_0()
                                        .child(ur_label(column.title(), UrRole::Label).truncate()),
                                )
                                .children(sorted.then(|| sized_icon(icon, ICON_XS, palette.ink))),
                            column,
                        )
                    }),
            )
            .child(div().w(px(ACTIONS_WIDTH)).flex_none())
            .into_any_element()
    }

    fn render_row(
        &self,
        index: usize,
        palette: &Palette,
        any_selected: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let row = self.row_view(index, cx)?;
        let id = row.id;
        let selected = self.selected.contains(&id);
        let focused = self.focused == Some(index);
        let palette = *palette;
        let hover = palette.hover;
        let date_cell = |text: SharedString, column: Column| {
            sized(
                div()
                    .h_full()
                    .px(px(8.))
                    .pt(px(3.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .text_size(px(13.))
                    .text_color(palette.ink_muted)
                    .font_features(tabular())
                    .whitespace_nowrap()
                    .child(if text.is_empty() {
                        SharedString::from("—")
                    } else {
                        text
                    }),
                column,
            )
            .into_any_element()
        };
        let cells: Vec<AnyElement> = Column::ALL
            .iter()
            .filter(|column| !self.hidden.contains(column))
            .map(|column| match column {
                Column::FirNo => sized(
                    div()
                        .h_full()
                        .px(px(8.))
                        .pt(px(1.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.ink)
                        .child(label(row.no.clone()).ltr().align(Align::Right).truncate()),
                    *column,
                )
                .into_any_element(),
                Column::Offence => sized(
                    div()
                        .h_full()
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .text_color(palette.ink)
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .child(ur_label(row.offence.clone(), UrRole::Body).truncate()),
                        ),
                    *column,
                )
                .into_any_element(),
                Column::Accused => sized(
                    h_flex()
                        .h_full()
                        .px(px(8.))
                        .flex_row_reverse()
                        .items_center()
                        .gap(px(6.))
                        .text_color(palette.ink_muted)
                        .child(
                            div()
                                .min_w_0()
                                .flex_shrink(1.)
                                .child(ur_label(row.accused.clone(), UrRole::Body).truncate()),
                        )
                        .when(row.accused_more > 0, |this| {
                            this.child(
                                div()
                                    .flex_none()
                                    .pt(px(3.))
                                    .text_size(px(13.))
                                    .text_color(palette.ink_faint)
                                    .font_features(tabular())
                                    .child(format!("+{}", row.accused_more)),
                            )
                        }),
                    *column,
                )
                .into_any_element(),
                Column::Status => sized(
                    h_flex()
                        .h_full()
                        .px(px(8.))
                        .flex_row_reverse()
                        .items_center()
                        .gap(px(8.))
                        .text_color(palette.ink)
                        .child(status_dot(&palette, row.status))
                        .child(ur_label(row.status.label(), UrRole::Body)),
                    *column,
                )
                .into_any_element(),
                Column::Date => date_cell(row.date.clone(), Column::Date),
                Column::IncidentDate => date_cell(row.incident.clone(), Column::IncidentDate),
                Column::ArrestDate => date_cell(row.arrest.clone(), Column::ArrestDate),
            })
            .collect();
        let show_check = any_selected || selected || focused;
        let focused_bounds = self.focused_bounds.clone();
        let element = h_flex()
            .id(("fir-row", index))
            .test_support()
            .group("fir-row")
            .relative()
            .w_full()
            .flex_row_reverse()
            .h(px(TABLE_ROW))
            .items_center()
            .border_b_1()
            .border_color(palette.line)
            .cursor_pointer()
            .role(gpui_kit::accesskit::Role::Row)
            .aria_label(SharedString::from(format!("FIR {}", row.no)))
            .map(|this| {
                if selected {
                    this.bg(palette.accent_wash)
                } else if focused {
                    this.bg(hover)
                } else {
                    this.hover(move |style| style.bg(hover))
                }
            })
            .on_click(cx.listener(move |this, _, window, cx| this.open_row(index, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.open_row_menu(index, event.position, window, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .id(("row-select-cell", index))
                    .test_support()
                    .w(px(SELECT_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_selected(id, cx)
                    }))
                    .child(
                        div()
                            .when(!show_check, |this| this.invisible())
                            .group_hover("fir-row", |style| style.visible())
                            .child(
                                checkbox(&palette, ("row-select", index), selected)
                                    .cursor_pointer()
                                    .aria_label(SharedString::from(format!(
                                        "Select FIR {}",
                                        row.no
                                    )))
                                    .test_support(),
                            ),
                    ),
            )
            .children(cells)
            .child(
                div()
                    .w(px(ACTIONS_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .invisible()
                            .group_hover("fir-row", |style| style.visible())
                            .child(
                                icon_button(
                                    ("row-actions", index),
                                    IconName::Ellipsis,
                                    "More actions",
                                )
                                .keys("Shift+F10")
                                .dense()
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        let position = window.mouse_position();
                                        this.open_row_menu(index, position, window, cx)
                                    },
                                )),
                            ),
                    ),
            )
            .when(focused, |this| {
                this.child(
                    div()
                        .absolute()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        .w(px(2.))
                        .bg(palette.accent),
                )
                .child(
                    canvas(
                        move |bounds, _, _| focused_bounds.set(Some(bounds)),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
            });
        Some(element.into_any_element())
    }

    fn skeleton_rows(&self, palette: &Palette) -> AnyElement {
        let palette = *palette;
        let widths = [
            (Column::FirNo, 56.),
            (Column::Offence, 150.),
            (Column::Accused, 190.),
            (Column::Status, 72.),
            (Column::Date, 76.),
            (Column::IncidentDate, 76.),
            (Column::ArrestDate, 76.),
        ];
        v_flex()
            .size_full()
            .children((0..8usize).map(|index| {
                h_flex()
                    .id(("skeleton-row", index))
                    .flex_row_reverse()
                    .flex_none()
                    .h(px(TABLE_ROW))
                    .items_center()
                    .border_b_1()
                    .border_color(palette.line)
                    .child(div().w(px(SELECT_WIDTH)).flex_none())
                    .children(
                        widths
                            .iter()
                            .filter(|(column, _)| !self.hidden.contains(column))
                            .map(|(column, width)| {
                                sized(
                                    h_flex()
                                        .px(px(8.))
                                        .flex_row_reverse()
                                        .child(skeleton_bar(&palette, *width, 12.)),
                                    *column,
                                )
                            }),
                    )
                    .child(div().w(px(ACTIONS_WIDTH)).flex_none())
            }))
            .into_any_element()
    }

    fn empty_body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.query.trim().is_empty() {
            let status = self.status_filter();
            return empty_state(
                palette,
                IconName::ListFilter,
                "No FIRs with this status",
                status.map(|status| {
                    SharedString::from(format!(
                        "No FIR is marked {}.",
                        isolate_urdu(status.label())
                    ))
                }),
                Some(
                    button("empty-show-all", "Show all FIRs")
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| this.clear_status_filter(cx)))
                        .into_any_element(),
                ),
            )
            .into_any_element();
        }
        let query = self.query.trim();
        empty_state(
            palette,
            IconName::Search,
            format!("No FIRs match “{}”", isolate_query(query)),
            None,
            Some(
                button("empty-clear", "Clear search")
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_search("", cx);
                        this.search.update(cx, |input, cx| input.focus(window, cx));
                    }))
                    .into_any_element(),
            ),
        )
        .into_any_element()
    }

    fn body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let palette = *Palette::get(cx);
        let load = self.models.firs.read(cx).load.clone();
        let total = self.models.firs.read(cx).rows.len();
        match load {
            Load::Loading => {
                if !self.skeleton {
                    return div().size_full().into_any_element();
                }
                v_flex()
                    .size_full()
                    .px(px(PAGE_GUTTER))
                    .child(self.header_row(&palette, cx))
                    .child(self.skeleton_rows(&palette))
                    .into_any_element()
            }
            Load::Failed(message) => empty_state(
                &palette,
                IconName::TriangleAlert,
                "Couldn't read your FIRs",
                Some("Missal could not open its database.".into()),
                Some(
                    v_flex()
                        .items_center()
                        .gap(px(12.))
                        .child(button("retry-load", "Try again").outline().on_click({
                            let firs = self.models.firs.clone();
                            move |_, _, cx| firs.update(cx, |firs, cx| firs.reload(cx))
                        }))
                        .child(
                            div()
                                .id("load-error")
                                .max_w(px(480.))
                                .text_size(px(12.))
                                .text_color(palette.ink_faint)
                                .child(label(message).align(Align::Center)),
                        )
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
                    button("empty-new-fir", "New FIR")
                        .primary()
                        .icon(IconName::Plus)
                        .keys("N")
                        .on_click(|_, window, cx| navigate(Route::NewFir, window, cx))
                        .into_any_element(),
                ),
            )
            .into_any_element(),
            Load::Ready => {
                self.ensure_derived(cx);
                if self.visible.is_empty() {
                    return self.empty_body(&palette, cx);
                }
                let count = self.visible.len();
                v_flex()
                    .size_full()
                    .px(px(PAGE_GUTTER))
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
}

fn isolate_query(query: &str) -> String {
    let rtl = query
        .chars()
        .find(|c| c.is_alphabetic())
        .is_some_and(|c| ('\u{0590}'..='\u{08FF}').contains(&c));
    if rtl {
        isolate_urdu(query)
    } else {
        format!("\u{2068}{query}\u{2069}")
    }
}

impl Focusable for FirListScreen {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FirListScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        self.ensure_derived(cx);
        let ready = self.models.firs.read(cx).load.is_ready();
        let has_rows = !self.models.firs.read(cx).rows.is_empty();
        let toolbar = (has_rows || (!ready && self.skeleton)).then(|| self.toolbar(&palette, cx));
        let body = self.body(cx);
        v_flex()
            .id("fir-list")
            .test_support()
            .key_context(FIR_LIST)
            .track_focus(&self.focus)
            .size_full()
            .bg(palette.surface)
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
                this.delete_focused_or_selected(window, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectAllRows, _, cx| this.select_all(cx)))
            .on_action(
                cx.listener(|this, _: &RowMenu, window, cx| this.open_focused_row_menu(window, cx)),
            )
            .on_action(cx.listener(|this, _: &BlurFocus, window, cx| {
                if !this.escape(window, cx) {
                    cx.propagate();
                }
            }))
            .on_action(
                cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)),
            )
            .children(toolbar)
            .child(div().flex_1().min_h_0().child(body))
            .children(self.menu.clone().map(|menu| menu.into_any_element()))
    }
}

impl HeaderSource for FirListScreen {
    fn header(&mut self, _: &mut Window, cx: &mut Context<Self>) -> HeaderContent {
        self.ensure_derived(cx);
        let total = self.models.firs.read(cx).rows.len();
        let visible = self.visible_count();
        let ready = self.models.firs.read(cx).load.is_ready();
        let meta = if !ready {
            String::new()
        } else if visible == total {
            total.to_string()
        } else {
            format!("{visible} of {total}")
        };
        let content = HeaderContent::new("FIRs").meta(meta);
        let selected = self.selected.len();
        if selected == 0 {
            return content.action(
                crate::ui::primary_sm("header-new-fir", "New FIR")
                    .icon(IconName::Plus)
                    .keys("N")
                    .tooltip("New FIR", Some("N"))
                    .on_click(|_, window, cx| navigate(Route::NewFir, window, cx)),
            );
        }
        let palette = *Palette::get(cx);
        let clear = cx.entity().downgrade();
        let delete = cx.entity().downgrade();
        content.action(
            h_flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .mr(px(4.))
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(palette.ink)
                        .font_features(tabular())
                        .child(format!("{selected} selected")),
                )
                .child(
                    button("clear-selection", "Clear")
                        .kind(ButtonKind::GhostMuted)
                        .sm()
                        .tooltip("Clear selection", Some("Escape"))
                        .on_click(move |_, _, cx| {
                            clear.update(cx, |this, cx| this.clear_selection(cx)).ok();
                        }),
                )
                .child(
                    danger_outline(
                        "delete-selected",
                        if selected == 1 {
                            "Delete 1 FIR".to_owned()
                        } else {
                            format!("Delete {selected} FIRs")
                        },
                    )
                    .icon(IconName::Trash)
                    .tooltip("Delete", Some("Delete"))
                    .on_click(move |_, window, cx| {
                        delete
                            .update(cx, |this, cx| this.delete_selected(window, cx))
                            .ok();
                    }),
                ),
        )
    }
}
