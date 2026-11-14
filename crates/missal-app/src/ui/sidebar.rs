use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::{StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;

use gpui_kit::{
    AnyElement, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Render,
    Stateful, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use missal_text::label;

use crate::app::{navigate, show_shortcuts};
use crate::state::{Models, Route};
use crate::theme::{Palette, SIDEBAR_ICON_WIDTH, SIDEBAR_WIDTH};
use crate::ui::status::status_icon;
use crate::ui::{small_icon, tooltip_with_keys};

pub struct SidebarView {
    models: Models,
    _subscriptions: Vec<Subscription>,
}

impl SidebarView {
    pub fn new(models: Models, cx: &mut Context<Self>) -> SidebarView {
        let subscriptions = vec![
            cx.observe(&models.firs, |_, _, cx| cx.notify()),
            cx.observe(&models.templates, |_, _, cx| cx.notify()),
            cx.observe(&models.nav, |_, _, cx| cx.notify()),
            cx.observe(&models.prefs, |_, _, cx| cx.notify()),
        ];
        SidebarView {
            models,
            _subscriptions: subscriptions,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn nav_item(
    palette: &Palette,
    id: impl Into<ElementId>,
    name: IconName,
    text: &'static str,
    active: bool,
    collapsed: bool,
    badge: Option<usize>,
    hint: Option<&'static str>,
) -> Stateful<gpui_kit::Div> {
    let accent = palette.sidebar_accent;
    let foreground = if active {
        palette.foreground
    } else {
        palette.sidebar_foreground
    };
    let base = h_flex()
        .id(id)
        .h(px(32.))
        .items_center()
        .gap_2()
        .rounded(px(7.))
        .cursor_pointer()
        .text_size(px(14.))
        .text_color(foreground)
        .when(active, |this| this.bg(accent).font_medium())
        .hover(move |style| style.bg(accent));
    if collapsed {
        return base
            .w(px(32.))
            .justify_center()
            .tooltip(tooltip_with_keys(text, hint))
            .child(small_icon(name, foreground));
    }
    base.w_full()
        .px_2()
        .child(small_icon(name, foreground))
        .child(div().flex_1().min_w_0().truncate().child(text))
        .children(badge.filter(|count| *count > 0).map(|count| {
            div()
                .text_size(px(12.))
                .text_color(palette.muted_foreground)
                .child(count.to_string())
        }))
}

impl Render for SidebarView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let collapsed = !self.models.prefs.read(cx).read().sidebar_open;
        let route = self.models.nav.read(cx).current();
        let firs = self.models.firs.read(cx);
        let fir_count = firs.rows.len();
        let recent: Vec<_> = firs
            .recent(5)
            .iter()
            .map(|row| (row.id, row.no.clone(), row.offence.clone(), row.status))
            .collect();
        let loading = !firs.load.is_ready();
        let template_count = self.models.templates.read(cx).items.len();
        let width = if collapsed {
            SIDEBAR_ICON_WIDTH
        } else {
            SIDEBAR_WIDTH
        };
        let fir_active = matches!(route, Route::Firs | Route::Fir(_));
        let templates_active = matches!(
            route,
            Route::Templates | Route::NewTemplate | Route::Template(_)
        );
        let current_fir = match route {
            Route::Fir(id) => Some(id),
            _ => None,
        };
        let new_fir: AnyElement = if collapsed {
            Button::new("sidebar-new-fir")
                .outline()
                .icon(IconName::Plus)
                .small()
                .tooltip("New FIR")
                .on_click(|_, window, cx| navigate(Route::NewFir, window, cx))
                .into_any_element()
        } else {
            Button::new("sidebar-new-fir")
                .outline()
                .w_full()
                .on_click(|_, window, cx| navigate(Route::NewFir, window, cx))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .child(small_icon(IconName::Plus, palette.foreground))
                        .child(div().flex_1().text_left().child("New FIR"))
                        .child(crate::ui::key_cap(&palette, "N")),
                )
                .into_any_element()
        };
        v_flex()
            .id("sidebar")
            .w(px(width))
            .flex_shrink_0()
            .h_full()
            .bg(palette.sidebar)
            .text_color(palette.sidebar_foreground)
            .px_2()
            .pb_2()
            .gap_1()
            .when(collapsed, |this| this.items_center())
            .child(
                div()
                    .py_2()
                    .when(collapsed, |this| this.flex().justify_center())
                    .child(new_fir),
            )
            .child(
                v_flex()
                    .gap_0p5()
                    .when(collapsed, |this| this.items_center())
                    .child(
                        nav_item(
                            &palette,
                            "sidebar-firs",
                            IconName::Folder,
                            "FIRs",
                            fir_active,
                            collapsed,
                            Some(fir_count),
                            None,
                        )
                        .on_click(|_, window, cx| navigate(Route::Firs, window, cx))
                        .test_support(),
                    )
                    .child(
                        nav_item(
                            &palette,
                            "sidebar-templates",
                            IconName::FileText,
                            "Templates",
                            templates_active,
                            collapsed,
                            Some(template_count),
                            None,
                        )
                        .on_click(|_, window, cx| navigate(Route::Templates, window, cx))
                        .test_support(),
                    ),
            )
            .when(!collapsed && (fir_count > 0 || loading), |this| {
                this.child(
                    div()
                        .px_2()
                        .pt_3()
                        .pb_1()
                        .text_size(px(12.))
                        .font_medium()
                        .text_color(palette.muted_foreground)
                        .child("Recent"),
                )
                .child(v_flex().gap_0p5().children(
                    if loading && recent.is_empty() {
                        (0..3usize)
                            .map(|index| {
                                div()
                                    .id(("recent-skeleton", index))
                                    .h(px(32.))
                                    .mx_2()
                                    .my(px(2.))
                                    .rounded(px(5.))
                                    .bg(palette.muted)
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    } else {
                        recent
                            .into_iter()
                            .map(|(id, no, offence, status)| {
                                let active = current_fir == Some(id);
                                let accent = palette.sidebar_accent;
                                h_flex()
                                    .id(("recent", id.get() as usize))
                                    .test_support()
                                    .h(px(36.))
                                    .px_2()
                                    .gap_2()
                                    .items_center()
                                    .rounded(px(7.))
                                    .cursor_pointer()
                                    .when(active, |this| this.bg(accent))
                                    .hover(move |style| style.bg(accent))
                                    .on_click(move |_, window, cx| {
                                        navigate(Route::Fir(id), window, cx)
                                    })
                                    .child(status_icon(status, palette.status(status), px(14.)))
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_size(px(13.))
                                            .text_color(palette.foreground)
                                            .child(label(no).ltr()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .text_size(px(13.))
                                            .text_color(palette.muted_foreground)
                                            .child(label(offence).rtl().truncate()),
                                    )
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                ))
            })
            .child(div().flex_1())
            .child(
                v_flex()
                    .gap_0p5()
                    .when(collapsed, |this| this.items_center())
                    .child(
                        nav_item(
                            &palette,
                            "sidebar-shortcuts",
                            IconName::Keyboard,
                            "Shortcuts",
                            false,
                            collapsed,
                            None,
                            Some("?"),
                        )
                        .on_click(|_, window, cx| show_shortcuts(window, cx))
                        .test_support(),
                    )
                    .child(
                        nav_item(
                            &palette,
                            "sidebar-settings",
                            IconName::Settings,
                            "Settings",
                            route == Route::Settings,
                            collapsed,
                            None,
                            None,
                        )
                        .on_click(|_, window, cx| navigate(Route::Settings, window, cx))
                        .test_support(),
                    ),
            )
    }
}
