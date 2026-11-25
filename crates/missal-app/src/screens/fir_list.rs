use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    Anchor, AnyElement, App, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Point,
    Render, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, Window, canvas, div, point, px, uniform_list,
};
use missal_core::{FirId, FirStatus};
use missal_text::input::{Enter, MoveDown};
use missal_text::{Align, TextInput, TextInputEvent, label};

use crate::app::navigate;
use crate::prefs::{Column, Sort};
use crate::shortcuts::*;
use crate::state::firs::FirsEvent;

use crate::state::{Load, Models, PrintTarget, Route, run_db_in, toast_error, toast_success};
use crate::theme::{ICON_XS, PAGE_GUTTER, Palette, TABLE_ROW};
use crate::ui::dialogs::{Choice, Tone, open_choice};
use crate::ui::input::{Hint, Kind, field_input, search_field};
use crate::ui::popup_menu::{Dismissed, MenuContent, MenuEntry, Placement, PopupList};
use crate::ui::status::status_badge;
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::{
    BadgeVariant, ButtonKind, UrRole, badge, button, checkbox, checkbox_mixed, danger_outline,
    empty_state, icon_button, isolate_urdu, kbd_group, sized_icon, skeleton_bar, table, table_body,
    table_footer, table_head, table_header, tabular, ur_label,
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

const STATUS_ITEMS: [FirStatus; 4] = [
    FirStatus::Open,
    FirStatus::UnderInvestigation,
    FirStatus::ChallanSubmitted,
    FirStatus::Closed,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuKind {
    Columns,
    Status,
    Row,
}

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
    menu_kind: Option<MenuKind>,
    stale: bool,
    selected: HashSet<FirId>,
    focused: Option<usize>,
    scroll: UniformListScrollHandle,
    menu: Option<Entity<PopupList>>,
    columns_bounds: Rc<Cell<Bounds<Pixels>>>,
    filter_bounds: Rc<Cell<Bounds<Pixels>>>,
    focused_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    skeleton: bool,
    _subscriptions: Vec<Subscription>,
}

impl FirListScreen {
    pub fn new(models: Models, window: &mut Window, cx: &mut Context<Self>) -> FirListScreen {
        let prefs = models.prefs.read(cx).read().table.clone();
        let search = field_input(window, cx, Kind::Search, "FIR no., offence or name…", "");
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
        let statuses: Vec<FirStatus> = prefs.status.into_iter().collect();
        FirListScreen {
            focus: cx.focus_handle(),
            search,
            query: String::new(),
            sort: prefs.sort,
            statuses,
            hidden: prefs.hidden,
            visible: Vec::new(),
            menu_kind: None,
            stale: true,
            selected: HashSet::new(),
            focused: None,
            scroll: UniformListScrollHandle::new(),
            menu: None,
            columns_bounds: Rc::new(Cell::new(Bounds::default())),
            filter_bounds: Rc::new(Cell::new(Bounds::default())),
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

    pub fn status_filter(&self) -> Option<FirStatus> {
        self.statuses.first().copied()
    }

    pub fn status_count(&mut self, status: Option<FirStatus>, cx: &App) -> usize {
        self.ensure_derived(cx);
        match status {
            Some(status) => self
                .models
                .firs
                .read(cx)
                .index
                .borrow()
                .status_count(status),
            None => self.models.firs.read(cx).index.borrow().matched_count(),
        }
    }

    pub fn visible_numbers(&self, cx: &App) -> Vec<String> {
        let rows = self.models.firs.read(cx).rows();
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
        let present: HashSet<FirId> = firs.rows().iter().map(|row| row.id).collect();
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
        let mut index = firs.index.borrow_mut();
        let shown = index.update(
            firs.rows(),
            firs.version(),
            &self.query,
            &self.statuses,
            self.sort,
        );
        self.visible.clear();
        self.visible.extend_from_slice(shown);
        drop(index);
        if self
            .focused
            .is_some_and(|index| index >= self.visible.len())
        {
            self.focused = None;
        }
    }

    fn save_table_prefs(&self, cx: &mut Context<Self>) {
        let (sort, status, hidden) = (self.sort, self.status_filter(), self.hidden.clone());
        self.models.prefs.update(cx, |prefs, cx| {
            prefs.update(cx, |preferences| {
                preferences.table.sort = sort;
                preferences.table.status = status;
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

    fn row_id(&self, index: usize, cx: &App) -> Option<FirId> {
        let position = *self.visible.get(index)?;
        self.models
            .firs
            .read(cx)
            .rows()
            .get(position as usize)
            .map(|row| row.id)
    }

    fn row_view(&self, index: usize, cx: &App) -> Option<RowView> {
        let position = *self.visible.get(index)?;
        let row = self.models.firs.read(cx).rows().get(position as usize)?;
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
        let rows = self.models.firs.read(cx).rows();
        !self.visible.is_empty()
            && self
                .visible
                .iter()
                .all(|&index| self.selected.contains(&rows[index as usize].id))
    }

    fn any_visible_selected(&self, cx: &App) -> bool {
        if self.selected.is_empty() {
            return false;
        }
        let rows = self.models.firs.read(cx).rows();
        self.visible
            .iter()
            .any(|&index| self.selected.contains(&rows[index as usize].id))
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
        let rows = self.models.firs.read(cx).rows();
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
            "Every document written for this FIR is deleted too. This can’t be undone."
        } else {
            "Every document written for these FIRs is deleted too. This can’t be undone."
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

    fn open_menu(
        &mut self,
        kind: MenuKind,
        menu: Entity<PopupList>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe_in(&menu, window, |this, _, _: &Dismissed, window, cx| {
            if this.menu.take().is_some() {
                this.menu_kind = None;
                this.focus(window, cx);
                cx.notify();
            }
        })
        .detach();
        self.menu = Some(menu);
        self.menu_kind = Some(kind);
        cx.notify();
    }

    pub fn open_columns_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let rows = MenuContent::new().items(
            Column::ALL
                .iter()
                .filter(|column| **column != Column::FirNo)
                .map(|column| {
                    let column = *column;
                    let weak = weak.clone();
                    MenuEntry::urdu(
                        SharedString::from(format!("column-{}", column.key())),
                        column.title(),
                        move |_, cx| {
                            weak.update(cx, |this, cx| this.toggle_column(column, cx))
                                .ok();
                        },
                    )
                    .check(!self.hidden.contains(&column))
                    .keep_open()
                }),
        );
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
        self.open_menu(MenuKind::Columns, menu, window, cx);
    }

    pub fn open_status_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.entity().downgrade();
        let filter = self.status_filter();
        let palette = *Palette::get(cx);
        let counts: Vec<usize> = {
            let index = self.models.firs.read(cx).index.borrow();
            STATUS_ITEMS
                .iter()
                .map(|status| index.status_count(*status))
                .collect()
        };
        let clear = weak.clone();
        let rows = MenuContent::new()
            .label("Status")
            .items(STATUS_ITEMS.into_iter().zip(counts).map(|(status, count)| {
                let weak = weak.clone();
                MenuEntry::urdu(
                    SharedString::from(format!("status-{}", status.as_str())),
                    status.label(),
                    move |_, cx| {
                        weak.update(cx, |this, cx| this.toggle_status(status, cx))
                            .ok();
                    },
                )
                .check(filter == Some(status))
                .dot(palette.status(status))
                .count(count)
            }))
            .separator()
            .item(
                MenuEntry::new("status-all", "Show all FIRs", move |_, cx| {
                    clear
                        .update(cx, |this, cx| this.clear_status_filter(cx))
                        .ok();
                })
                .disabled(filter.is_none()),
            );
        let bounds = self.filter_bounds.get();
        let position = point(bounds.right(), bounds.bottom() + px(4.));
        let menu = PopupList::open(
            "status-menu",
            rows,
            Placement {
                position,
                anchor: Anchor::TopRight,
                width: 220.,
            },
            window,
            cx,
        );
        self.open_menu(MenuKind::Status, menu, window, cx);
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
        let delete = weak;
        let rows = MenuContent::new()
            .item(
                MenuEntry::new("row-open", "Open", move |window, cx| {
                    navigate(Route::Fir(id), window, cx)
                })
                .icon(IconName::FolderOpen)
                .keys("Enter"),
            )
            .item(
                MenuEntry::new("row-edit", "Edit details", move |window, cx| {
                    edit.update(cx, |this, cx| this.edit_fir(id, window, cx))
                        .ok();
                })
                .icon(IconName::Pencil)
                .keys("E"),
            )
            .item(
                MenuEntry::new("row-print", "Print…", move |window, cx| {
                    navigate(Route::Print(PrintTarget::Fir(id)), window, cx)
                })
                .icon(IconName::Printer)
                .keys("Mod+P"),
            )
            .separator()
            .item(
                MenuEntry::new("row-delete", "Delete…", move |window, cx| {
                    delete
                        .update(cx, |this, cx| this.request_delete(vec![id], window, cx))
                        .ok();
                })
                .icon(IconName::Trash)
                .keys("Delete")
                .danger(),
            );
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
        self.open_menu(MenuKind::Row, menu, window, cx);
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
        let columns_bounds = self.columns_bounds.clone();
        let menu_open = self.menu_kind == Some(MenuKind::Columns);
        let filter_bounds = self.filter_bounds.clone();
        let filter_open = self.menu_kind == Some(MenuKind::Status);
        let action = palette.action;
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
                            .height(36.)
                            .hint(Hint::Slash),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(
                        canvas(
                            move |bounds, _, _| filter_bounds.set(bounds),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        icon_button(
                            "status-filter",
                            IconName::ListFilter,
                            if filter.is_some() {
                                "Filtered by status"
                            } else {
                                "Filter by status"
                            },
                        )
                        .toggled(filter_open || filter.is_some())
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_status_menu(window, cx)),
                        ),
                    )
                    .when(filter.is_some(), |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(5.))
                                .right(px(5.))
                                .size(px(6.))
                                .rounded_full()
                                .bg(action),
                        )
                    }),
            )
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
        let some_selected = !all_selected && self.any_visible_selected(cx);
        table_header()
            .id("fir-table-head")
            .flex_row_reverse()
            .child(
                div()
                    .id("select-all-cell")
                    .w(px(SELECT_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_all(cx)))
                    .child(
                        if some_selected {
                            checkbox_mixed("select-all")
                        } else {
                            checkbox("select-all", all_selected)
                        }
                        .accessibility_label("Select all visible FIRs")
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_all(cx)
                        })),
                    ),
            )
            .children(
                Column::ALL
                    .iter()
                    .filter(|column| !self.hidden.contains(column))
                    .map(|column| {
                        let column = *column;
                        let sorted = self.sort.column == column;
                        let icon = match (sorted, self.sort.descending) {
                            (false, _) => IconName::ChevronsUpDown,
                            (true, true) => IconName::ChevronDown,
                            (true, false) => IconName::ChevronUp,
                        };
                        let ink = palette.ink;
                        let group = SharedString::from(format!("sort-{}", column.key()));
                        sized(
                            table_head()
                                .id(("sort", column as usize))
                                .test_support()
                                .group(group.clone())
                                .flex_row_reverse()
                                .gap(px(4.))
                                .cursor_pointer()
                                .hover(move |style| style.text_color(ink))
                                .when(sorted, |this| this.text_color(ink))
                                .role(gpui_kit::accesskit::Role::ColumnHeader)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.toggle_sort(column, cx)),
                                )
                                .child(
                                    div().mt(px(-2.)).min_w_0().child(
                                        ur_label(column.title(), UrRole::InputLabel).truncate(),
                                    ),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .when(!sorted, |this| {
                                            this.invisible()
                                                .group_hover(group, |style| style.visible())
                                        })
                                        .child(sized_icon(
                                            icon,
                                            ICON_XS,
                                            if sorted {
                                                palette.ink_muted
                                            } else {
                                                palette.ink_faint
                                            },
                                        )),
                                ),
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
        let date_cell = |text: SharedString, column: Column| {
            sized(
                div()
                    .h_full()
                    .px(px(10.))
                    .pt(px(3.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .text_size(px(14.))
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
                        .px(px(10.))
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
                        .px(px(10.))
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
                        .px(px(10.))
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
                                badge(BadgeVariant::Outline)
                                    .text_color(palette.ink_muted)
                                    .child(label(format!("+{}", row.accused_more)).ltr()),
                            )
                        }),
                    *column,
                )
                .into_any_element(),
                Column::Status => sized(
                    h_flex()
                        .h_full()
                        .px(px(10.))
                        .flex_row_reverse()
                        .items_center()
                        .child(status_badge(row.status)),
                    *column,
                )
                .into_any_element(),
                Column::Date => date_cell(row.date.clone(), Column::Date),
                Column::IncidentDate => date_cell(row.incident.clone(), Column::IncidentDate),
                Column::ArrestDate => date_cell(row.arrest.clone(), Column::ArrestDate),
            })
            .collect();
        let last = index + 1 == self.visible.len();
        let show_check = any_selected || selected || focused;
        let focused_bounds = self.focused_bounds.clone();
        let hover = palette.hover.opacity(0.5);
        let element = h_flex()
            .id(("fir-row", index))
            .test_support()
            .group("fir-row")
            .relative()
            .w_full()
            .flex_row_reverse()
            .h(px(TABLE_ROW))
            .items_center()
            .when(!last, |this| this.border_b_1().border_color(palette.line))
            .cursor_pointer()
            .role(gpui_kit::accesskit::Role::Row)
            .aria_label(SharedString::from(format!("FIR {}", row.no)))
            .map(|this| {
                if selected {
                    this.bg(palette.hover)
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
                                checkbox(("row-select", index), selected)
                                    .tab_stop(false)
                                    .accessibility_label(SharedString::from(format!(
                                        "Select FIR {}",
                                        row.no
                                    ))),
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
                                .hover_through()
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
                        .bg(palette.ring),
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

    fn footer(&self, total: usize, palette: &Palette) -> AnyElement {
        let palette = *palette;
        let shown = self.visible.len();
        let selected = self.selected.len();
        let summary = if shown == total {
            format!("{total} {}", if total == 1 { "FIR" } else { "FIRs" })
        } else {
            format!("Showing {shown} of {total} FIRs")
        };
        let hint = |keys: &'static str, text: &'static str| {
            h_flex()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .child(kbd_group(keys))
                .child(div().text_color(palette.ink_faint).child(text))
        };
        let hints = if selected > 0 {
            h_flex()
                .items_center()
                .gap(px(16.))
                .child(hint("Space", "Toggle"))
                .child(hint("Delete", "Delete"))
                .child(hint("Escape", "Clear"))
        } else {
            h_flex()
                .items_center()
                .gap(px(16.))
                .child(hint("J", "Next"))
                .child(hint("K", "Previous"))
                .child(hint("Enter", "Open"))
                .child(hint("Space", "Select"))
        };
        table_footer()
            .id("fir-table-foot")
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.))
                    .child(summary)
                    .when(selected > 0, |this| {
                        this.child(div().text_color(palette.ink_faint).child("·"))
                            .child(
                                div()
                                    .text_color(palette.ink)
                                    .child(format!("{selected} selected")),
                            )
                    }),
            )
            .child(hints)
            .into_any_element()
    }

    fn skeleton_rows(&self) -> AnyElement {
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
                                        .child(skeleton_bar(*width, 12.)),
                                    *column,
                                )
                            }),
                    )
                    .child(div().w(px(ACTIONS_WIDTH)).flex_none())
            }))
            .into_any_element()
    }

    fn empty_body(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.query.trim().is_empty() {
            let status = self.status_filter();
            return empty_state(
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
        let total = self.models.firs.read(cx).rows().len();
        match load {
            Load::Loading => {
                if !self.skeleton {
                    return div().size_full().into_any_element();
                }
                div()
                    .size_full()
                    .px(px(PAGE_GUTTER))
                    .pb(px(PAGE_GUTTER))
                    .child(
                        table()
                            .child(self.header_row(&palette, cx))
                            .child(table_body().child(self.skeleton_rows())),
                    )
                    .into_any_element()
            }
            Load::Failed(message) => empty_state(
                IconName::TriangleAlert,
                "Couldn’t read your FIRs",
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
                    return self.empty_body(cx);
                }
                let count = self.visible.len();
                div()
                    .size_full()
                    .px(px(PAGE_GUTTER))
                    .pb(px(PAGE_GUTTER))
                    .child(
                        table()
                            .child(self.header_row(&palette, cx))
                            .child(
                                table_body().child(
                                    uniform_list(
                                        "fir-rows",
                                        count,
                                        cx.processor(
                                            move |this, range: std::ops::Range<usize>, _, cx| {
                                                let palette = *Palette::get(cx);
                                                let any_selected = !this.selected.is_empty();
                                                range
                                                    .filter_map(|index| {
                                                        this.render_row(
                                                            index,
                                                            &palette,
                                                            any_selected,
                                                            cx,
                                                        )
                                                    })
                                                    .collect::<Vec<_>>()
                                            },
                                        ),
                                    )
                                    .track_scroll(&self.scroll)
                                    .size_full(),
                                ),
                            )
                            .child(self.footer(total, &palette)),
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
        let has_rows = !self.models.firs.read(cx).rows().is_empty();
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
        let content = HeaderContent::new("FIRs");
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
                        .text_size(px(14.))
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
