use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{StyledExt, TitleBar, h_flex};
use gpui_kit::prelude::*;

use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use missal_text::label;

use crate::app::{navigate, open_palette, toggle_sidebar};
use crate::state::{Models, Route};
use crate::theme::{Palette, TITLE_BAR_HEIGHT};
use crate::ui::{key_hint, small_icon, tooltip_with_keys};

pub struct TitleBarView {
    models: Models,
    _subscriptions: Vec<Subscription>,
}

struct Crumb {
    text: SharedString,
    route: Option<Route>,
}

impl TitleBarView {
    pub fn new(models: Models, cx: &mut Context<Self>) -> TitleBarView {
        let subscriptions = vec![
            cx.observe(&models.nav, |_, _, cx| cx.notify()),
            cx.observe(&models.firs, |_, _, cx| cx.notify()),
            cx.observe(&models.templates, |_, _, cx| cx.notify()),
        ];
        TitleBarView {
            models,
            _subscriptions: subscriptions,
        }
    }

    fn crumbs(&self, route: Route, cx: &gpui_kit::App) -> Vec<Crumb> {
        let section = |text: &'static str, route: Route| Crumb {
            text: text.into(),
            route: Some(route),
        };
        let leaf = |text: SharedString| Crumb { text, route: None };
        match route {
            Route::Firs => vec![leaf("FIRs".into())],
            Route::NewFir => vec![section("FIRs", Route::Firs), leaf("New FIR".into())],
            Route::Fir(id) => {
                let no = self
                    .models
                    .firs
                    .read(cx)
                    .get(id)
                    .map_or_else(|| SharedString::from("FIR"), |row| row.no.clone());
                vec![section("FIRs", Route::Firs), leaf(no)]
            }
            Route::Settings => vec![leaf("Settings".into())],
            Route::Templates => vec![leaf("Templates".into())],
            Route::NewTemplate => vec![
                section("Templates", Route::Templates),
                leaf("New template".into()),
            ],
            Route::Template(id) => {
                let name = self.models.templates.read(cx).get(id).map_or_else(
                    || SharedString::from("Template"),
                    |item| item.name.clone().into(),
                );
                vec![section("Templates", Route::Templates), leaf(name)]
            }
        }
    }
}

impl Render for TitleBarView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let nav = self.models.nav.read(cx);
        let route = nav.current();
        let (can_back, can_forward) = (nav.can_go_back(), nav.can_go_forward());
        let crumbs = self.crumbs(route, cx);
        let count = crumbs.len();
        let nav_entity = self.models.nav.clone();
        let forward_entity = self.models.nav.clone();
        TitleBar::new()
            .h(px(TITLE_BAR_HEIGHT))
            .bg(palette.sidebar)
            .border_color(palette.sidebar_border)
            .pl_2()
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("toggle-sidebar")
                            .ghost()
                            .small()
                            .icon(IconName::PanelLeft)
                            .tooltip("Toggle sidebar")
                            .on_click(|_, window, cx| toggle_sidebar(window, cx)),
                    )
                    .child(
                        Button::new("history-back")
                            .ghost()
                            .small()
                            .icon(IconName::ArrowLeft)
                            .disabled(!can_back)
                            .tooltip("Back")
                            .on_click(move |_, window, cx| {
                                nav_entity.update(cx, |nav, cx| nav.back(window, cx));
                            }),
                    )
                    .child(
                        Button::new("history-forward")
                            .ghost()
                            .small()
                            .icon(IconName::ArrowRight)
                            .disabled(!can_forward)
                            .tooltip("Forward")
                            .on_click(move |_, window, cx| {
                                forward_entity.update(cx, |nav, cx| nav.forward(window, cx));
                            }),
                    )
                    .child(
                        h_flex()
                            .id("breadcrumbs")
                            .ml_2()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .text_size(px(13.))
                            .children(crumbs.into_iter().enumerate().flat_map(|(index, crumb)| {
                                let last = index + 1 == count;
                                let mut items: Vec<AnyElement> = Vec::new();
                                if index > 0 {
                                    items.push(
                                        small_icon(
                                            IconName::ChevronRight,
                                            palette.muted_foreground,
                                        )
                                        .into_any_element(),
                                    );
                                }
                                let text = div()
                                    .id(("crumb", index))
                                    .max_w(px(260.))
                                    .py_1()
                                    .overflow_hidden()
                                    .text_color(if last {
                                        palette.foreground
                                    } else {
                                        palette.muted_foreground
                                    })
                                    .when(last, |this| this.font_medium())
                                    .child(label(crumb.text).truncate());
                                items.push(match crumb.route {
                                    Some(target) if !last => text
                                        .cursor_pointer()
                                        .hover(move |style| style.text_color(palette.foreground))
                                        .on_click(move |_, window, cx| navigate(target, window, cx))
                                        .into_any_element(),
                                    _ => text.into_any_element(),
                                });
                                items
                            })),
                    ),
            )
            .child(
                div().w(px(448.)).flex_shrink(1.).min_w(px(180.)).child(
                    h_flex()
                        .id("palette-trigger")
                        .test_support()
                        .h(px(30.))
                        .w_full()
                        .px_3()
                        .gap_2()
                        .items_center()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(palette.input)
                        .bg(palette.background)
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(palette.muted_foreground)
                        .hover(move |style| style.bg(palette.accent))
                        .tooltip(tooltip_with_keys("Command menu", Some("Mod+K")))
                        .on_click(|_, window, cx| open_palette(window, cx))
                        .child(small_icon(IconName::Search, palette.muted_foreground))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child("Search FIRs, templates and commands"),
                        )
                        .child(key_hint(&palette, "Mod+K")),
                ),
            )
            .child(div().flex_1().min_w_0())
    }
}
