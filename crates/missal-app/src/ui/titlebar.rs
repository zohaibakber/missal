use gpui_kit::assets::IconName;
use gpui_kit::component::{InteractiveElementExt as _, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, RenderOnce, SharedString, Styled, Window, WindowControlArea, div, px,
};

use gpui_kit::component::Sizable as _;
use gpui_kit::component::avatar::Avatar;

use crate::app::{navigate, navigate_back, navigate_forward, open_palette, toggle_sidebar};
use crate::auth::AuthModel;
use crate::state::Route;
use crate::theme::{HEADER_HEIGHT, Palette};
use crate::ui::popup_menu::{MenuEntry, menu};
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
    sidebar_open: bool,
    navigation: bool,
}

impl Header {
    pub fn new(
        content: HeaderContent,
        can_back: bool,
        can_forward: bool,
        sidebar_open: bool,
    ) -> Self {
        Header {
            content,
            can_back,
            can_forward,
            sidebar_open,
            navigation: true,
        }
    }

    pub fn locked(content: HeaderContent) -> Self {
        Header {
            content,
            can_back: false,
            can_forward: false,
            sidebar_open: false,
            navigation: false,
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
            .text_size(px(14.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
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
            .when(self.navigation, |this| {
                this.child(
                    h_flex()
                        .flex_none()
                        .items_center()
                        .gap(px(2.))
                        .when(self.sidebar_open, |this| {
                            this.w(px(crate::theme::SIDEBAR_WIDTH) - inset - px(8.))
                        })
                        .child(
                            icon_button("sidebar-toggle", IconName::PanelLeft, "Sidebar")
                                .keys("Mod+\\")
                                .on_click(|_, _, cx| toggle_sidebar(cx)),
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
                            icon_button("header-search", IconName::Search, "Search")
                                .keys("Mod+K")
                                .on_click(|_, window, cx| open_palette(window, cx)),
                        ),
                )
            })
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
                    .text_size(px(14.))
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
            .when(self.navigation, |this| this.children(account_menu(cx)))
            .when(controls.is_none(), |this| this.pr(px(8.)));
        h_flex()
            .id("header")
            .flex_none()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .items_center()
            .bg(palette.canvas)
            .font_family(missal_text::UI_FAMILY)
            .child(content)
            .when_some(controls, |this, controls| {
                this.child(div().flex_none().ml(px(8.)).child(controls))
            })
    }
}

fn account_menu(cx: &App) -> Option<AnyElement> {
    let account = AuthModel::get(cx)?.read(cx).account()?.clone();
    let name = SharedString::from(account.display_name().to_owned());
    let email = SharedString::from(account.email.clone());
    let admin = account.role.is_admin();
    let trial = crate::auth::trial_days_left(&account).map(crate::auth::trial_label);
    let purchase = AuthModel::get(cx)?.read(cx).purchase_url().is_some();
    Some(
        div()
            .mr(px(4.))
            .child(
                menu("account-menu")
                    .trigger(
                        icon_button("account-menu", IconName::User, name.clone())
                            .content(Avatar::new().name(name.clone()).small()),
                    )
                    .content(move |menu, _, _| {
                        menu.header(name.clone(), Some(email.clone()))
                            .when_some(trial.clone(), |menu, trial| {
                                let menu = menu.separator().header(
                                    trial,
                                    (!purchase).then(|| {
                                        SharedString::from("Contact us to buy a licence.")
                                    }),
                                );
                                if purchase {
                                    menu.item(
                                        MenuEntry::new(
                                            "account-buy",
                                            "Buy a licence…",
                                            |_, cx| crate::auth::open_purchase(cx),
                                        )
                                        .icon(IconName::ExternalLink),
                                    )
                                } else {
                                    menu
                                }
                            })
                            .separator()
                            .item(
                                MenuEntry::new(
                                    "account-password",
                                    "Change password…",
                                    |window, cx| {
                                        crate::ui::account::open_change_password(window, cx)
                                    },
                                )
                                .icon(IconName::KeyRound),
                            )
                            .when(admin, |menu| {
                                menu.item(
                                    MenuEntry::new("account-people", "People…", |window, cx| {
                                        crate::ui::account::open_users(window, cx)
                                    })
                                    .icon(IconName::Users),
                                )
                            })
                            .item(
                                MenuEntry::new("account-settings", "Settings", |window, cx| {
                                    navigate(Route::Settings, window, cx)
                                })
                                .icon(IconName::Settings),
                            )
                            .separator()
                            .item(
                                MenuEntry::new("account-sign-out", "Sign out", |window, cx| {
                                    crate::auth::sign_out(window, cx)
                                })
                                .icon(IconName::LogOut),
                            )
                    }),
            )
            .into_any_element(),
    )
}
