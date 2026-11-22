use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::avatar::Avatar;
use gpui_kit::prelude::*;
use gpui_kit::{
    Anchor, AnyElement, App, ClickEvent, FontWeight, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use missal_text::label;
use std::rc::Rc;

use crate::app::{navigate, set_theme};
use crate::auth::{AuthModel, open_purchase, sign_out, trial_days_left, trial_label};
use crate::prefs::ThemeChoice;
use crate::state::{Models, Route};
use crate::theme::Palette;
use crate::ui::account::{open_change_password, open_users};
use crate::ui::popup_menu::{MenuContent, MenuEntry, MenuSwitchOption, MenuTrigger, menu};
use crate::ui::publishing::open_publishing;
use crate::ui::{ClickHandler, IconButton, icon_button, sidebar_menu_button};

const SIGNED_OUT: &str = "Not signed in";
const USER_ROW: f32 = 36.;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UserMenuSlot {
    Sidebar,
    Rail,
    Header,
}

#[derive(Clone)]
struct Profile {
    name: SharedString,
    email: SharedString,
    admin: bool,
    trial: Option<SharedString>,
    purchase: bool,
}

fn profile(cx: &App) -> Option<Profile> {
    let auth = AuthModel::get(cx)?;
    let auth = auth.read(cx);
    let account = auth.account()?;
    Some(Profile {
        name: account.display_name().to_owned().into(),
        email: account.email.clone().into(),
        admin: account.role.is_admin(),
        trial: trial_days_left(account).map(|days| trial_label(days).into()),
        purchase: auth.purchase_url().is_some(),
    })
}

fn theme_icon(choice: ThemeChoice) -> IconName {
    match choice {
        ThemeChoice::System => IconName::Monitor,
        ThemeChoice::Light => IconName::Sun,
        ThemeChoice::Dark => IconName::Moon,
    }
}

fn theme_options() -> Vec<MenuSwitchOption> {
    ThemeChoice::ALL
        .into_iter()
        .map(|choice| {
            MenuSwitchOption::new(
                format!("account-theme-{}", choice.as_str()),
                theme_icon(choice),
                choice.title(),
                move |window, cx| set_theme(choice, window, cx),
            )
        })
        .collect()
}

fn licence_rows(menu: MenuContent, trial: SharedString, purchase: bool) -> MenuContent {
    let detail = (!purchase).then(|| SharedString::from("Contact us to buy a licence."));
    menu.separator()
        .header(trial, detail)
        .when(purchase, |menu| {
            menu.item(
                MenuEntry::new("account-buy", "Buy a licence…", |_, cx| open_purchase(cx))
                    .icon(IconName::ExternalLink),
            )
        })
}

fn rows(menu: MenuContent, profile: Option<&Profile>, theme: ThemeChoice) -> MenuContent {
    let selected = ThemeChoice::ALL
        .iter()
        .position(|choice| *choice == theme)
        .unwrap_or(0);
    let menu = match profile {
        Some(profile) => menu
            .header(profile.name.clone(), Some(profile.email.clone()))
            .when_some(profile.trial.clone(), |menu, trial| {
                licence_rows(menu, trial, profile.purchase)
            }),
        None => menu.header(SIGNED_OUT, None),
    };
    menu.separator()
        .switch("Theme", theme_options(), selected)
        .separator()
        .when(profile.is_some(), |menu| {
            menu.item(
                MenuEntry::new("account-password", "Change password…", open_change_password)
                    .icon(IconName::KeyRound),
            )
        })
        .when(profile.is_some_and(|profile| profile.admin), |menu| {
            menu.item(MenuEntry::new("account-people", "People…", open_users).icon(IconName::Users))
                .item(
                    MenuEntry::new("account-publishing", "Publishing…", open_publishing)
                        .icon(IconName::CloudUpload),
                )
        })
        .item(
            MenuEntry::new("account-settings", "Settings", |window, cx| {
                navigate(Route::Settings, window, cx)
            })
            .icon(IconName::Settings),
        )
        .when(profile.is_some(), |menu| {
            menu.separator().item(
                MenuEntry::new("account-sign-out", "Sign out", sign_out).icon(IconName::LogOut),
            )
        })
}

#[derive(IntoElement)]
struct UserButton {
    name: SharedString,
    avatar: Avatar,
    open: bool,
    on_click: Option<ClickHandler>,
}

impl MenuTrigger for UserButton {
    fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    fn on_activate(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for UserButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        sidebar_menu_button("account-menu", self.open)
            .h(px(USER_ROW))
            .aria_label(self.name.clone())
            .when_some(self.on_click, |this, handler| {
                this.on_click(move |event, window, cx| handler(event, window, cx))
            })
            .child(self.avatar)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(palette.ink)
                    .child(label(self.name.to_string()).ltr().truncate()),
            )
    }
}

fn avatar(profile: Option<&Profile>) -> Avatar {
    match profile {
        Some(profile) => Avatar::new().name(profile.name.clone()).small(),
        None => Avatar::new().small(),
    }
}

fn avatar_button(name: SharedString, profile: Option<&Profile>) -> IconButton {
    icon_button("account-menu", IconName::User, name).content(avatar(profile))
}

fn attach<T: MenuTrigger>(trigger: T, anchor: Anchor, profile: Option<Profile>) -> AnyElement {
    menu("account-menu")
        .anchor(anchor)
        .trigger(trigger)
        .content(move |menu, _, cx| {
            let theme = Models::get(cx).prefs.read(cx).read().theme;
            rows(menu, profile.as_ref(), theme)
        })
        .into_any_element()
}

pub fn user_menu(slot: UserMenuSlot, cx: &App) -> AnyElement {
    let profile = profile(cx);
    let name = profile
        .as_ref()
        .map_or_else(|| SIGNED_OUT.into(), |profile| profile.name.clone());
    match slot {
        UserMenuSlot::Sidebar => attach(
            UserButton {
                name,
                avatar: avatar(profile.as_ref()),
                open: false,
                on_click: None,
            },
            Anchor::BottomLeft,
            profile,
        ),
        UserMenuSlot::Rail => attach(
            avatar_button(name, profile.as_ref()),
            Anchor::BottomLeft,
            profile,
        ),
        UserMenuSlot::Header => attach(
            avatar_button(name, profile.as_ref()),
            Anchor::TopRight,
            profile,
        ),
    }
}
