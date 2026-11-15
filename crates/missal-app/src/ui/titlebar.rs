use gpui_kit::assets::IconName;
use gpui_kit::component::{InteractiveElementExt as _, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, RenderOnce, SharedString, Styled, Window, WindowControlArea, div, px,
};

use crate::app::{navigate_back, navigate_forward, open_palette, toggle_nav_menu};
use crate::theme::{HEADER_HEIGHT, Palette};
use crate::ui::{UrRole, icon_button, ur_inline, window_controls};

pub enum HeaderTitle {
    Latin(SharedString),
    Urdu(SharedString),
    Element(AnyElement),
}

pub struct HeaderContent {
    pub title: HeaderTitle,
    pub meta: Option<AnyElement>,
    pub actions: Vec<AnyElement>,
}

impl HeaderContent {
    pub fn new(title: impl Into<SharedString>) -> HeaderContent {
        HeaderContent {
            title: HeaderTitle::Latin(title.into()),
            meta: None,
            actions: Vec::new(),
        }
    }

    pub fn urdu(title: impl Into<SharedString>) -> HeaderContent {
        HeaderContent {
            title: HeaderTitle::Urdu(title.into()),
            meta: None,
            actions: Vec::new(),
        }
    }

    pub fn element(title: impl IntoElement) -> HeaderContent {
        HeaderContent {
            title: HeaderTitle::Element(title.into_any_element()),
            meta: None,
            actions: Vec::new(),
        }
    }

    pub fn meta(mut self, text: impl Into<SharedString>) -> HeaderContent {
        let text: SharedString = text.into();
        self.meta = Some(div().child(text).into_any_element());
        self
    }

    pub fn meta_element(mut self, element: impl IntoElement) -> HeaderContent {
        self.meta = Some(element.into_any_element());
        self
    }

    pub fn action(mut self, element: impl IntoElement) -> HeaderContent {
        self.actions.push(element.into_any_element());
        self
    }

    pub fn actions(mut self, elements: impl IntoIterator<Item = AnyElement>) -> HeaderContent {
        self.actions.extend(elements);
        self
    }
}

pub trait HeaderSource: Sized + 'static {
    fn header(&mut self, window: &mut Window, cx: &mut Context<Self>) -> HeaderContent;
}

#[derive(IntoElement)]
pub struct Header {
    content: HeaderContent,
    can_back: bool,
    can_forward: bool,
    menu_open: bool,
}

impl Header {
    pub fn new(content: HeaderContent, can_back: bool, can_forward: bool, menu_open: bool) -> Self {
        Header {
            content,
            can_back,
            can_forward,
            menu_open,
        }
    }
}

struct DragState {
    should_move: bool,
}

impl Render for DragState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub fn title_element(palette: &Palette, title: HeaderTitle) -> AnyElement {
    match title {
        HeaderTitle::Latin(text) => div()
            .flex_none()
            .text_size(px(15.))
            .line_height(px(20.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(palette.ink)
            .whitespace_nowrap()
            .child(text)
            .into_any_element(),
        HeaderTitle::Urdu(text) => div()
            .min_w_0()
            .mt(px(-2.))
            .text_color(palette.ink)
            .child(ur_inline(text, UrRole::Title).truncate())
            .into_any_element(),
        HeaderTitle::Element(element) => element,
    }
}

impl RenderOnce for Header {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let state = window.use_state(cx, |_, _| DragState { should_move: false });
        let linux = cfg!(target_os = "linux");
        let macos = cfg!(target_os = "macos");
        let client_decorated = matches!(
            window.window_decorations(),
            gpui_kit::Decorations::Client { .. }
        );
        let inset = window_controls::leading_inset(window, cx);
        let controls = window_controls::render(window, cx);
        let HeaderContent {
            title,
            meta,
            actions,
        } = self.content;
        let content = h_flex()
            .id("header-drag")
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .gap(px(2.))
            .pl(inset)
            .window_control_area(WindowControlArea::Drag)
            .when(linux, |this| {
                this.on_double_click(|_, window, _| window.zoom_window())
            })
            .when(macos, |this| {
                this.on_double_click(|_, window, _| window.titlebar_double_click())
            })
            .when(linux && client_decorated, |this| {
                this.on_mouse_down(MouseButton::Right, |event, window, _| {
                    window.show_window_menu(event.position)
                })
            })
            .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                state.should_move = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = false;
                }),
            )
            .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                if state.should_move {
                    state.should_move = false;
                    if !cfg!(target_os = "windows") {
                        window.start_window_move();
                    }
                }
            }))
            .child(
                icon_button("nav-menu", IconName::Menu, "Menu")
                    .keys("F10")
                    .toggled(self.menu_open)
                    .on_click(|_, window, cx| toggle_nav_menu(window, cx)),
            )
            .child(
                icon_button("history-back", IconName::ArrowLeft, "Back")
                    .keys("Alt+Left")
                    .disabled(!self.can_back)
                    .on_click(|_, window, cx| navigate_back(window, cx)),
            )
            .child(
                icon_button("history-forward", IconName::ArrowRight, "Forward")
                    .keys("Alt+Right")
                    .disabled(!self.can_forward)
                    .on_click(|_, window, cx| navigate_forward(window, cx)),
            )
            .child(
                h_flex()
                    .id("header-title")
                    .ml(px(8.))
                    .min_w_0()
                    .flex_shrink(1.)
                    .items_center()
                    .gap(px(10.))
                    .child(title_element(&palette, title)),
            )
            .children(meta.map(|meta| {
                h_flex()
                    .ml(px(8.))
                    .min_w_0()
                    .flex_shrink(1.)
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(palette.ink_muted)
                    .whitespace_nowrap()
                    .child(meta)
            }))
            .child(div().flex_1().min_w(px(16.)).h_full())
            .child(
                h_flex()
                    .id("header-actions")
                    .flex_none()
                    .items_center()
                    .gap(px(4.))
                    .mr(px(4.))
                    .children(actions),
            )
            .child(
                icon_button("header-search", IconName::Search, "Search")
                    .keys("Mod+K")
                    .on_click(|_, window, cx| open_palette(window, cx)),
            )
            .when(controls.is_none(), |this| this.pr(px(8.)));
        h_flex()
            .id("header")
            .flex_none()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .items_center()
            .bg(palette.surface)
            .border_b_1()
            .border_color(palette.line)
            .font_family(missal_text::UI_FAMILY)
            .child(content)
            .when_some(controls, |this, controls| {
                this.child(div().flex_none().ml(px(8.)).child(controls))
            })
    }
}
