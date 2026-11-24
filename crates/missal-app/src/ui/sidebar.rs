use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::app::{navigate, show_shortcuts};
use crate::state::{Models, Route};
use crate::theme::{
    HEADER_HEIGHT, ICON, Palette, RADIUS, SIDEBAR_COMPACT_WIDTH, SIDEBAR_ITEM, SIDEBAR_WIDTH,
};
use crate::ui::admin::is_admin;
use crate::ui::nav_menu::NavEntry;
use crate::ui::status::status_dot;
use crate::ui::titlebar::{
    back_button, drag_region, forward_button, search_button, sidebar_toggle,
};
use crate::ui::user_menu::{UserMenuSlot, user_menu};
use crate::ui::window_controls::{self, ControlsStyle, MAC_TRAFFIC_LIGHT_INSET};
use crate::ui::{UrRole, icon_button, sized_icon, tabular, tooltip, ur_inline};

const RECENT_LIMIT: usize = 6;

#[derive(IntoElement)]
pub struct Sidebar {
    route: Route,
    compact: bool,
}

pub fn sidebar(route: Route, compact: bool) -> Sidebar {
    Sidebar { route, compact }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarMode {
    Hidden,
    Compact,
    Full,
}

impl SidebarMode {
    pub fn of(hidden: bool, route: Route) -> SidebarMode {
        let compact = matches!(route, Route::Fir(_) | Route::Print(_));
        match (hidden, compact) {
            (true, _) => SidebarMode::Hidden,
            (false, true) => SidebarMode::Compact,
            (false, false) => SidebarMode::Full,
        }
    }
}

fn compact_item(
    id: SharedString,
    icon: IconName,
    label: &'static str,
    keys: Option<&'static str>,
    active: bool,
    palette: &Palette,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    item_base(id, active, palette)
        .w(px(SIDEBAR_ITEM))
        .px_0()
        .justify_center()
        .tooltip(tooltip(label, keys))
        .tooltip_show_delay(crate::ui::tooltip_delay())
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(sized_icon(
            icon,
            ICON,
            if active {
                palette.ink
            } else {
                palette.sidebar_ink
            },
        ))
        .test_support()
        .into_any_element()
}

fn traffic_lights(window: &Window, cx: &App) -> bool {
    window_controls::style(window, cx) == ControlsStyle::MacTrafficLights
}

pub fn rail_width(window: &Window, cx: &App) -> Pixels {
    if traffic_lights(window, cx) {
        px(MAC_TRAFFIC_LIGHT_INSET)
    } else {
        px(SIDEBAR_COMPACT_WIDTH)
    }
}

fn render_compact(
    route: Route,
    palette: &Palette,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let width = rail_width(window, cx);
    let lights = traffic_lights(window, cx);
    let top = lights.then(|| {
        let row = h_flex()
            .id("sidebar-drag")
            .flex_none()
            .w_full()
            .h(px(HEADER_HEIGHT));
        drag_region(row, "sidebar-drag-state", window, cx)
    });
    let section = NavEntry::destination(route);
    let admin = is_admin(cx);
    let nav = |entry: NavEntry, icon: IconName, target: Route| {
        compact_item(
            SharedString::from(format!("sidebar-{}", entry.id())),
            icon,
            entry.label(),
            None,
            section == entry,
            palette,
            move |window, cx| navigate(target, window, cx),
        )
    };
    v_flex()
        .id("sidebar")
        .flex_none()
        .w(width)
        .h_full()
        .items_center()
        .when(!lights, |this| this.pt(px(6.)))
        .pb(px(8.))
        .gap(px(2.))
        .bg(palette.sidebar)
        .border_r_1()
        .border_color(palette.line)
        .children(top)
        .child(nav(NavEntry::Firs, IconName::Folders, Route::Firs))
        .child(nav(
            NavEntry::Templates,
            IconName::LayoutTemplate,
            Route::Templates,
        ))
        .child(nav(NavEntry::Settings, IconName::Settings, Route::Settings))
        .when(admin, |this| {
            this.child(
                div()
                    .flex_none()
                    .w(px(20.))
                    .h(px(1.))
                    .my(px(6.))
                    .bg(palette.line),
            )
            .child(nav(NavEntry::People, IconName::Users, Route::People))
            .child(nav(
                NavEntry::Publishing,
                IconName::CloudUpload,
                Route::Publishing,
            ))
        })
        .child(div().flex_1())
        .child(compact_item(
            "sidebar-shortcuts".into(),
            IconName::Keyboard,
            "Shortcuts",
            Some("?"),
            false,
            palette,
            show_shortcuts,
        ))
        .child(
            div()
                .flex_none()
                .mt(px(6.))
                .child(user_menu(UserMenuSlot::Rail, cx)),
        )
        .into_any_element()
}

fn item_base(
    id: impl Into<ElementId>,
    active: bool,
    palette: &Palette,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let hover = palette.sidebar_hover;
    h_flex()
        .id(id)
        .flex_none()
        .w_full()
        .h(px(SIDEBAR_ITEM))
        .px(px(8.))
        .gap(px(8.))
        .items_center()
        .rounded(px(RADIUS))
        .cursor_pointer()
        .text_size(px(14.))
        .line_height(px(20.))
        .when(active, |this| {
            this.bg(palette.sidebar_active)
                .text_color(palette.ink)
                .font_weight(FontWeight::MEDIUM)
        })
        .when(!active, |this| {
            this.text_color(palette.sidebar_ink)
                .hover(move |style| style.bg(hover))
                .active(move |style| style.bg(hover))
        })
}

fn nav_item(
    entry: NavEntry,
    icon: IconName,
    count: Option<usize>,
    active: bool,
    route: Route,
    palette: &Palette,
) -> AnyElement {
    let ink = palette.ink;
    item_base(
        SharedString::from(format!("sidebar-{}", entry.id())),
        active,
        palette,
    )
    .group("sidebar-item")
    .on_click(move |_, window, cx| navigate(route, window, cx))
    .child(sized_icon(
        icon,
        ICON,
        if active {
            palette.ink
        } else {
            palette.sidebar_ink
        },
    ))
    .child(
        div()
            .flex_1()
            .min_w_0()
            .group_hover("sidebar-item", move |style| style.text_color(ink))
            .child(entry.label()),
    )
    .children(count.map(|count| {
        div()
            .flex_none()
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::NORMAL)
            .font_features(tabular())
            .text_color(palette.ink_faint)
            .child(count.to_string())
    }))
    .test_support()
    .into_any_element()
}

fn group_label(text: &'static str, palette: &Palette) -> AnyElement {
    h_flex()
        .flex_none()
        .h(px(28.))
        .px(px(8.))
        .items_center()
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.ink_faint)
        .child(text)
        .into_any_element()
}

fn control_row(window: &mut Window, cx: &mut App) -> AnyElement {
    let leading = if traffic_lights(window, cx) {
        MAC_TRAFFIC_LIGHT_INSET - 8.
    } else {
        0.
    };
    let (can_back, can_forward) = {
        let models = Models::get(cx);
        let nav = models.nav.read(cx);
        (nav.can_go_back(), nav.can_go_forward())
    };
    let row = h_flex()
        .id("sidebar-drag")
        .flex_none()
        .h(px(HEADER_HEIGHT))
        .pl(px(leading))
        .mb(px(4.))
        .items_center()
        .gap(px(2.))
        .child(sidebar_toggle())
        .child(search_button())
        .child(div().flex_1())
        .child(back_button(can_back))
        .child(forward_button(can_forward));
    drag_region(row, "sidebar-drag-state", window, cx).into_any_element()
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        if self.compact {
            return render_compact(self.route, &palette, window, cx);
        }
        let controls = control_row(window, cx);
        let models = Models::get(cx);
        let section = NavEntry::destination(self.route);
        let prefs = models.prefs.read(cx).read();
        let firs = models.firs.read(cx);
        let recent: Vec<AnyElement> = prefs
            .recent_firs
            .iter()
            .filter_map(|entry| firs.get(entry.id))
            .take(RECENT_LIMIT)
            .enumerate()
            .map(|(index, row)| {
                let id = row.id;
                let active = matches!(self.route, Route::Fir(current) | Route::EditFir(current) if current == id);
                let ink = palette.ink;
                item_base(("sidebar-recent", index), active, &palette)
                    .group("sidebar-recent")
                    .font_weight(FontWeight::NORMAL)
                    .on_click(move |_, window, cx| navigate(Route::Fir(id), window, cx))
                    .child(div().w(px(16.)).flex().justify_center().child(status_dot(row.status)))
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(13.))
                            .font_features(tabular())
                            .child(row.no.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .mt(px(-2.))
                            .text_color(if active { palette.ink_muted } else { palette.ink_faint })
                            .group_hover("sidebar-recent", move |style| style.text_color(ink))
                            .child(ur_inline(row.offence.clone(), UrRole::Label).truncate()),
                    )
                    .test_support()
                    .into_any_element()
            })
            .collect();
        let has_recent = !recent.is_empty();
        let admin = is_admin(cx);
        v_flex()
            .id("sidebar")
            .flex_none()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .px(px(8.))
            .pb(px(8.))
            .gap(px(2.))
            .bg(palette.sidebar)
            .border_r_1()
            .border_color(palette.line)
            .font_family(missal_text::UI_FAMILY)
            .child(controls)
            .child(nav_item(
                NavEntry::Firs,
                IconName::Folders,
                None,
                section == NavEntry::Firs,
                Route::Firs,
                &palette,
            ))
            .child(nav_item(
                NavEntry::Templates,
                IconName::LayoutTemplate,
                None,
                section == NavEntry::Templates,
                Route::Templates,
                &palette,
            ))
            .child(nav_item(
                NavEntry::Settings,
                IconName::Settings,
                None,
                section == NavEntry::Settings,
                Route::Settings,
                &palette,
            ))
            .when(admin, |this| {
                this.child(div().h(px(14.)))
                    .child(group_label("Admin", &palette))
                    .child(nav_item(
                        NavEntry::People,
                        IconName::Users,
                        None,
                        section == NavEntry::People,
                        Route::People,
                        &palette,
                    ))
                    .child(nav_item(
                        NavEntry::Publishing,
                        IconName::CloudUpload,
                        None,
                        section == NavEntry::Publishing,
                        Route::Publishing,
                        &palette,
                    ))
            })
            .when(has_recent, |this| {
                this.child(div().h(px(14.)))
                    .child(group_label("Recent", &palette))
                    .children(recent)
            })
            .child(div().flex_1().min_h(px(16.)))
            .child(
                h_flex()
                    .flex_none()
                    .mt(px(6.))
                    .gap(px(2.))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(user_menu(UserMenuSlot::Sidebar, cx)),
                    )
                    .child(
                        icon_button(
                            "sidebar-shortcuts",
                            IconName::Keyboard,
                            "Keyboard shortcuts",
                        )
                        .keys("?")
                        .color(palette.sidebar_ink)
                        .on_click(|_, window, cx| show_shortcuts(window, cx)),
                    ),
            )
            .into_any_element()
    }
}
