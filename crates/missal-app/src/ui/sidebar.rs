use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    Anchor, AnyElement, App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::app::{import_word_file, navigate, set_theme, show_shortcuts};
use crate::prefs::ThemeChoice;
use crate::state::{Models, Route};
use crate::theme::{
    ICON, Palette, RADIUS, SIDEBAR_COMPACT_WIDTH, SIDEBAR_ITEM, SIDEBAR_WIDTH, TOOLTIP_DELAY,
};
use crate::ui::nav_menu::NavEntry;
use crate::ui::popup_menu::{MenuEntry, menu};
use crate::ui::status::status_dot;
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

pub fn compact_for(route: Route) -> bool {
    matches!(
        route,
        Route::Fir(_)
            | Route::Print(_)
            | Route::Templates
            | Route::Template(_)
            | Route::NewTemplate
    )
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
        .tooltip_show_delay(TOOLTIP_DELAY)
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

fn render_compact(route: Route, palette: &Palette) -> AnyElement {
    let section = NavEntry::destination(route);
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
        .w(px(SIDEBAR_COMPACT_WIDTH))
        .h_full()
        .items_center()
        .pb(px(8.))
        .gap(px(2.))
        .bg(palette.canvas)
        .child(nav(NavEntry::Firs, IconName::Folders, Route::Firs))
        .child(nav(
            NavEntry::Templates,
            IconName::LayoutTemplate,
            Route::Templates,
        ))
        .child(nav(NavEntry::Settings, IconName::Settings, Route::Settings))
        .child(div().flex_1())
        .child(compact_item(
            "sidebar-import".into(),
            IconName::FileUp,
            "Import Word file",
            Some("Mod+O"),
            false,
            palette,
            import_word_file,
        ))
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
                .mt(px(6.))
                .pt(px(10.))
                .border_t_1()
                .border_color(palette.line)
                .child(brand_mark(palette)),
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

fn theme_icon(choice: ThemeChoice) -> IconName {
    match choice {
        ThemeChoice::System => IconName::Monitor,
        ThemeChoice::Light => IconName::Sun,
        ThemeChoice::Dark => IconName::Moon,
    }
}

fn theme_menu(current: ThemeChoice) -> AnyElement {
    menu("sidebar-theme")
        .anchor(Anchor::BottomRight)
        .width(176.)
        .trigger(
            icon_button(
                "sidebar-theme",
                theme_icon(current),
                format!("Theme: {}", current.title()),
            )
            .dense(),
        )
        .content(move |menu, _, _| {
            menu.label("Theme")
                .items(ThemeChoice::ALL.into_iter().map(|choice| {
                    MenuEntry::new(
                        SharedString::from(format!("sidebar-theme-{}", choice.as_str())),
                        choice.title(),
                        move |window, cx| set_theme(choice, window, cx),
                    )
                    .icon(theme_icon(choice))
                    .check(choice == current)
                }))
        })
        .into_any_element()
}

fn footer_tools(theme: ThemeChoice) -> AnyElement {
    h_flex()
        .flex_none()
        .gap(px(2.))
        .items_center()
        .child(
            icon_button("sidebar-import", IconName::FileUp, "Import Word file")
                .keys("Mod+O")
                .dense()
                .on_click(|_, window, cx| import_word_file(window, cx)),
        )
        .child(
            icon_button(
                "sidebar-shortcuts",
                IconName::Keyboard,
                "Keyboard shortcuts",
            )
            .keys("?")
            .dense()
            .on_click(|_, window, cx| show_shortcuts(window, cx)),
        )
        .child(theme_menu(theme))
        .into_any_element()
}

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        if self.compact {
            return render_compact(self.route, &palette);
        }
        let models = Models::get(cx);
        let section = NavEntry::destination(self.route);
        let prefs = models.prefs.read(cx).read();
        let theme = prefs.theme;
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
        v_flex()
            .id("sidebar")
            .flex_none()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .px(px(8.))
            .pb(px(8.))
            .gap(px(2.))
            .bg(palette.canvas)
            .font_family(missal_text::UI_FAMILY)
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
                    .pt(px(8.))
                    .px(px(4.))
                    .border_t_1()
                    .border_color(palette.line)
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(brand_mark(&palette))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.ink)
                                    .child("Missal"),
                            ),
                    )
                    .child(footer_tools(theme)),
            )
            .into_any_element()
    }
}

pub fn brand_mark(palette: &Palette) -> AnyElement {
    div()
        .flex_none()
        .size(px(22.))
        .rounded(px(6.))
        .bg(palette.action)
        .flex()
        .items_center()
        .justify_center()
        .child(sized_icon(IconName::Feather, 13., palette.on_action))
        .into_any_element()
}
