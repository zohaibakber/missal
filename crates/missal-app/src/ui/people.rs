use crate::ui::ScrollArea as _;
use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use gpui_kit::assets::IconName;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, WeakEntity, Window, div, px,
};
use missal_auth::{
    Account, AuthError, LicenceStatus, ManagedAccount, NewUser, Role, SessionKeeper, UserChanges,
};
use missal_text::{TextInput, TextInputEvent, label};

use crate::auth::{AuthModel, password_problem};
use crate::state::{Load, toast_success};
use crate::theme::Palette;
use crate::ui::dialogs::{
    Choice, Tone, confirm_delete, dialog_description, dialog_footer, dialog_header, dialog_surface,
    dialog_title, open_choice, optical_top,
};
use crate::ui::input::{Kind, field, field_input, search_field, secret_input};
use crate::ui::library::day_month_year;
use crate::ui::popup_menu::{MenuEntry, menu};
use crate::ui::{
    BadgeVariant, Intent, SegmentItem, alert, badge, button, empty_state_body, field_description,
    field_error, field_group, field_label, form_field, icon_button, primary_sm, segmented,
    sidebar_menu_button, skeleton_bar,
};

const WIDTH: f32 = 960.;
const MARGIN: f32 = 48.;
const LIST_WIDTH: f32 = 340.;
const BODY_HEIGHT: f32 = 500.;
const MIN_BODY_HEIGHT: f32 = 320.;
const DIALOG_CHROME: f32 = 260.;
const FACT_LABEL: f32 = 112.;
const ROW_HEIGHT: f32 = 58.;
const FORM_WIDTH: f32 = 460.;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

const TERMS: [(u8, &str); 5] = [
    (1, "1 month"),
    (3, "3 months"),
    (6, "6 months"),
    (12, "1 year"),
    (24, "2 years"),
];
const START_TERMS: [(u8, &str); 4] = [
    (0, "7-day trial"),
    (1, "1 month"),
    (6, "6 months"),
    (12, "1 year"),
];
const TRIAL_DAYS: [u8; 3] = [7, 14, 30];

fn keeper(cx: &App) -> Option<Arc<SessionKeeper>> {
    AuthModel::get(cx)?.read(cx).keeper()
}

fn own_id(cx: &App) -> Option<String> {
    AuthModel::get(cx).and_then(|auth| auth.read(cx).account().map(|account| account.id.clone()))
}

fn caption(palette: &Palette, text: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(palette.ink_muted)
        .child(label(text.into()).ltr().truncate())
        .into_any_element()
}

fn date_text(millis: i64) -> String {
    day_month_year(millis).unwrap_or_else(|| "Unknown".to_owned())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Standing {
    Admin,
    Disabled,
    Unverified,
    Trial,
    Licensed,
    TrialEnded,
    Expired,
}

fn standing(user: &Account) -> Standing {
    if user.disabled {
        Standing::Disabled
    } else if !user.email_verified {
        Standing::Unverified
    } else if user.role.is_admin() {
        Standing::Admin
    } else {
        match user.licence.status {
            LicenceStatus::Trial => Standing::Trial,
            LicenceStatus::Licensed => Standing::Licensed,
            LicenceStatus::TrialEnded => Standing::TrialEnded,
            LicenceStatus::Expired => Standing::Expired,
        }
    }
}

fn days_left(user: &Account) -> i64 {
    ((user.licence.until_ms - Utc::now().timestamp_millis()) + DAY_MS - 1).max(0) / DAY_MS
}

pub(crate) fn standing_badge(user: &Account) -> AnyElement {
    let (intent, text) = match standing(user) {
        Standing::Admin => (Intent::Success, "Always licensed".to_owned()),
        Standing::Disabled => (Intent::Neutral, "Disabled".to_owned()),
        Standing::Unverified => (Intent::Warning, "Email not confirmed".to_owned()),
        Standing::Trial => {
            let days = days_left(user);
            let unit = if days == 1 { "day" } else { "days" };
            (Intent::Info, format!("Trial · {days} {unit} left"))
        }
        Standing::Licensed => (Intent::Success, "Licensed".to_owned()),
        Standing::TrialEnded => (Intent::Warning, "Trial ended".to_owned()),
        Standing::Expired => (Intent::Danger, "Licence ended".to_owned()),
    };
    badge(BadgeVariant::Intent(intent))
        .px(px(6.))
        .child(text)
        .into_any_element()
}

pub(crate) fn licence_text(user: &Account) -> String {
    let date = date_text(user.licence.until_ms);
    match user.licence.status {
        _ if user.role.is_admin() => "Always licensed".to_owned(),
        _ if !user.email_verified => "The trial starts when the email is confirmed".to_owned(),
        LicenceStatus::Trial => format!("Trial until {date}"),
        LicenceStatus::Licensed => format!("Licensed until {date}"),
        LicenceStatus::TrialEnded => format!("Trial ended {date}"),
        LicenceStatus::Expired => format!("Licence ended {date}"),
    }
}

fn sessions_text(user: &ManagedAccount) -> String {
    let seen = user
        .last_seen_at
        .map(|at| format!(" · last active {}", date_text(at)))
        .unwrap_or_default();
    match user.sessions {
        0 => "Not signed in".to_owned(),
        1 => format!("On 1 computer{seen}"),
        count => format!("On {count} computers{seen}"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Trial,
    Licensed,
    Ended,
    Unconfirmed,
    Disabled,
}

const FILTERS: [(Filter, &str); 6] = [
    (Filter::All, "All"),
    (Filter::Trial, "Trial"),
    (Filter::Licensed, "Licensed"),
    (Filter::Ended, "Ended"),
    (Filter::Unconfirmed, "Unconfirmed"),
    (Filter::Disabled, "Disabled"),
];

impl Filter {
    fn admits(self, user: &Account) -> bool {
        let standing = standing(user);
        match self {
            Filter::All => true,
            Filter::Trial => standing == Standing::Trial,
            Filter::Licensed => matches!(standing, Standing::Licensed | Standing::Admin),
            Filter::Ended => matches!(standing, Standing::TrialEnded | Standing::Expired),
            Filter::Unconfirmed => standing == Standing::Unverified,
            Filter::Disabled => standing == Standing::Disabled,
        }
    }
}

fn body_height(window: &Window) -> f32 {
    (f32::from(window.viewport_size().height) - DIALOG_CHROME).clamp(MIN_BODY_HEIGHT, BODY_HEIGHT)
}

pub fn open_users(window: &mut Window, cx: &mut App) {
    if keeper(cx).is_none() {
        return;
    }
    let panel = cx.new(|cx| UsersPanel::new(window, cx));
    window.open_dialog(cx, move |dialog: Dialog, window, cx| {
        let palette = *Palette::get(cx);
        let room = f32::from(window.viewport_size().width) - MARGIN;
        dialog_surface(dialog, &palette, WIDTH.min(room))
            .margin_top(optical_top(window, body_height(window) + 180.))
            .child(panel.clone())
    });
}

enum Outcome {
    Updated(ManagedAccount),
    Removed,
    Unchanged,
}

type Job = Box<dyn FnOnce(&SessionKeeper, &str) -> Result<Outcome, AuthError> + Send>;

pub struct UsersPanel {
    details_scroll: gpui_kit::ScrollHandle,
    list_scroll: gpui_kit::ScrollHandle,
    users: Vec<ManagedAccount>,
    load: Load,
    busy: HashSet<String>,
    problem: Option<SharedString>,
    filter: Filter,
    selected: Option<String>,
    search: Entity<TextInput>,
    query: String,
    _subscriptions: Vec<Subscription>,
}

impl UsersPanel {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = field_input(window, cx, Kind::Search, "Name or email", "");
        let subscriptions =
            vec![
                cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                    if *event == TextInputEvent::Change {
                        this.query = input.read(cx).text().trim().to_lowercase();
                        cx.notify();
                    }
                }),
            ];
        let mut panel = UsersPanel {
            details_scroll: gpui_kit::ScrollHandle::new(),
            list_scroll: gpui_kit::ScrollHandle::new(),
            users: Vec::new(),
            load: Load::Loading,
            busy: HashSet::new(),
            problem: None,
            filter: Filter::All,
            selected: None,
            search,
            query: String::new(),
            _subscriptions: subscriptions,
        };
        panel.reload(cx);
        panel
    }

    fn failed(error: &AuthError, cx: &mut Context<Self>) -> SharedString {
        if error.ends_session()
            && let Some(auth) = AuthModel::get(cx)
        {
            auth.update(cx, |auth, cx| auth.end_session(cx));
        }
        error.to_string().into()
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(keeper) = keeper(cx) else {
            return;
        };
        let task = cx.background_spawn(async move { keeper.users() });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| {
                match outcome {
                    Ok(users) => {
                        this.users = users;
                        this.load = Load::Ready;
                    }
                    Err(error) => {
                        let message = Self::failed(&error, cx);
                        this.load = Load::Failed(message.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn visible(&self) -> Vec<ManagedAccount> {
        self.users
            .iter()
            .filter(|user| self.filter.admits(&user.account))
            .filter(|user| {
                self.query.is_empty()
                    || user.account.email.contains(&self.query)
                    || user.account.name.to_lowercase().contains(&self.query)
            })
            .cloned()
            .collect()
    }

    fn current(&self, rows: &[ManagedAccount]) -> Option<usize> {
        self.selected
            .as_ref()
            .and_then(|id| rows.iter().position(|row| &row.account.id == id))
            .or((!rows.is_empty()).then_some(0))
    }

    fn upsert(&mut self, account: ManagedAccount) {
        match self
            .users
            .iter_mut()
            .find(|user| user.account.id == account.account.id)
        {
            Some(user) => *user = account,
            None => {
                self.users.push(account);
                self.users
                    .sort_by(|left, right| left.account.email.cmp(&right.account.email));
            }
        }
    }

    fn run(
        &mut self,
        id: String,
        done: Option<SharedString>,
        job: Job,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keeper) = keeper(cx) else {
            return;
        };
        if !self.busy.insert(id.clone()) {
            return;
        }
        self.problem = None;
        cx.notify();
        let task = cx.background_spawn({
            let id = id.clone();
            async move { job(&keeper, &id) }
        });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, window, cx| {
                this.busy.remove(&id);
                match outcome {
                    Ok(Outcome::Updated(account)) => this.upsert(account),
                    Ok(Outcome::Unchanged) => {}
                    Ok(Outcome::Removed) => {
                        this.users.retain(|user| user.account.id != id);
                        if this.selected.as_deref() == Some(id.as_str()) {
                            this.selected = None;
                        }
                    }
                    Err(error) => this.problem = Some(Self::failed(&error, cx)),
                }
                if this.problem.is_none()
                    && let Some(done) = done
                {
                    toast_success(window, cx, done);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn act(
        handle: &WeakEntity<Self>,
        id: &str,
        done: impl Into<SharedString>,
        job: impl FnOnce(&SessionKeeper, &str) -> Result<Outcome, AuthError> + Send + 'static,
        window: &mut Window,
        cx: &mut App,
    ) {
        let id = id.to_owned();
        let done = done.into();
        handle
            .update(cx, |this, cx| {
                this.run(id, Some(done), Box::new(job), window, cx);
            })
            .ok();
    }

    fn change(
        &mut self,
        id: String,
        changes: UserChanges,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run(
            id,
            None,
            Box::new(move |keeper, id| keeper.update_user(id, &changes).map(Outcome::Updated)),
            window,
            cx,
        );
    }

    fn renew_menu(&self, user: &Account, busy: bool, cx: &mut Context<Self>) -> AnyElement {
        let handle = cx.entity().downgrade();
        let id = user.id.clone();
        let email = user.email.clone();
        let title = if standing(user) == Standing::Licensed {
            "Extend licence"
        } else {
            "Renew licence"
        };
        menu("people-renew")
            .trigger(
                primary_sm("people-renew", title)
                    .icon(IconName::CalendarPlus)
                    .disabled(busy),
            )
            .width(200.)
            .content(move |mut content, _, _| {
                for (months, term) in TERMS {
                    let handle = handle.clone();
                    let id = id.clone();
                    let email = email.clone();
                    content = content.item(MenuEntry::new(
                        format!("people-renew-{months}"),
                        format!("Add {term}"),
                        move |window, cx| {
                            Self::act(
                                &handle,
                                &id,
                                format!("Added {term} for {email}"),
                                move |keeper, id| {
                                    keeper.grant_licence(id, months).map(Outcome::Updated)
                                },
                                window,
                                cx,
                            );
                        },
                    ));
                }
                content
            })
            .into_any_element()
    }

    fn more_menu(
        &self,
        user: &ManagedAccount,
        busy: bool,
        own: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let handle = cx.entity().downgrade();
        let account = user.account.clone();
        let signed_in = user.sessions > 0;
        menu("people-more")
            .trigger(icon_button("people-more", IconName::Ellipsis, "More actions").disabled(busy))
            .width(248.)
            .content(move |mut content, _, _| {
                let id = account.id.clone();
                let email = account.email.clone();
                let admin = account.role.is_admin();
                let verified = account.email_verified;
                let disabled = account.disabled;
                if !admin {
                    for days in TRIAL_DAYS {
                        let handle = handle.clone();
                        let id = id.clone();
                        let email = email.clone();
                        content = content.item(
                            MenuEntry::new(
                                format!("people-trial-{days}"),
                                format!("Extend trial by {days} days"),
                                move |window, cx| {
                                    Self::act(
                                        &handle,
                                        &id,
                                        format!("Trial extended by {days} days for {email}"),
                                        move |keeper, id| keeper.extend_trial(id, days).map(Outcome::Updated),
                                        window,
                                        cx,
                                    );
                                },
                            )
                            .icon(IconName::ClockPlus),
                        );
                    }
                    let active = account.licence.status.active();
                    content = content.item({
                        let handle = handle.clone();
                        let id = id.clone();
                        let email = email.clone();
                        MenuEntry::new("people-end-licence", "End licence now", move |window, cx| {
                            let handle = handle.clone();
                            let id = id.clone();
                            let email = email.clone();
                            open_choice(
                                window,
                                cx,
                                format!("End the licence for {email}?"),
                                "Their trial or paid time ends today. They can still sign in, but the template library closes until you renew it.",
                                vec![
                                    Choice::new("end-cancel", "Cancel", Tone::Neutral, |_, _| {}),
                                    Choice::new("end-confirm", "End licence", Tone::Danger, move |window, cx| {
                                        Self::act(
                                            &handle,
                                            &id,
                                            format!("Licence ended for {email}"),
                                            |keeper, id| keeper.end_licence(id).map(Outcome::Updated),
                                            window,
                                            cx,
                                        );
                                    }),
                                ],
                            );
                        })
                        .icon(IconName::CalendarX)
                        .disabled(!active)
                    });
                    content = content.separator();
                }
                if !verified {
                    let handle = handle.clone();
                    let id = id.clone();
                    let email = email.clone();
                    content = content.item(
                        MenuEntry::new("people-verify", "Confirm email", move |window, cx| {
                            Self::act(
                                &handle,
                                &id,
                                format!("Confirmed {email}"),
                                |keeper, id| keeper.verify_user(id).map(Outcome::Updated),
                                window,
                                cx,
                            );
                        })
                        .icon(IconName::MailCheck),
                    );
                }
                content = content.item({
                    let handle = handle.clone();
                    let id = id.clone();
                    let email = email.clone();
                    MenuEntry::new("people-set-password", "Set a new password…", move |window, cx| {
                        open_set_password(handle.clone(), id.clone(), email.clone(), window, cx);
                    })
                    .icon(IconName::KeyRound)
                    .disabled(own)
                });
                content = content.item({
                    let handle = handle.clone();
                    let id = id.clone();
                    let email = email.clone();
                    MenuEntry::new("people-reset-email", "Email a reset code", move |window, cx| {
                        Self::act(
                            &handle,
                            &id,
                            format!("Reset code sent to {email}"),
                            |keeper, id| keeper.send_reset(id).map(|()| Outcome::Unchanged),
                            window,
                            cx,
                        );
                    })
                    .icon(IconName::Mail)
                    .disabled(!verified || disabled)
                });
                content = content.item({
                    let handle = handle.clone();
                    let id = id.clone();
                    let email = email.clone();
                    MenuEntry::new("people-sign-out", "Sign out everywhere", move |window, cx| {
                        Self::act(
                            &handle,
                            &id,
                            format!("Signed {email} out everywhere"),
                            |keeper, id| keeper.sign_out_user(id).map(Outcome::Updated),
                            window,
                            cx,
                        );
                    })
                    .icon(IconName::LogOut)
                    .disabled(own || !signed_in)
                });
                content = content.separator();
                content = content.item({
                    let handle = handle.clone();
                    let id = id.clone();
                    let entry = MenuEntry::new(
                        "people-toggle",
                        if disabled { "Enable account" } else { "Disable account" },
                        move |window, cx| {
                            let id = id.clone();
                            handle
                                .update(cx, |this, cx| {
                                    this.change(
                                        id,
                                        UserChanges {
                                            disabled: Some(!disabled),
                                            ..UserChanges::default()
                                        },
                                        window,
                                        cx,
                                    );
                                })
                                .ok();
                        },
                    )
                    .icon(if disabled { IconName::UserCheck } else { IconName::UserX })
                    .disabled(own);
                    if disabled { entry } else { entry.danger() }
                });
                let handle = handle.clone();
                let id = id.clone();
                let email = email.clone();
                content.item(
                    MenuEntry::new("people-delete", "Delete account…", move |window, cx| {
                        let handle = handle.clone();
                        let id = id.clone();
                        let email = email.clone();
                        confirm_delete(
                            window,
                            cx,
                            format!("Delete {email}?"),
                            "The account, its licence and its sign-ins are removed for good. FIRs on their computers stay there. Disable the account instead if they might come back.",
                            "Delete account",
                            move |window, cx| {
                                Self::act(
                                    &handle,
                                    &id,
                                    format!("Deleted {email}"),
                                    |keeper, id| keeper.delete_user(id).map(|()| Outcome::Removed),
                                    window,
                                    cx,
                                );
                            },
                        );
                    })
                    .icon(IconName::Trash)
                    .danger()
                    .disabled(own),
                )
            })
            .into_any_element()
    }

    fn list_row(
        &self,
        index: usize,
        user: &ManagedAccount,
        active: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let account = &user.account;
        let id = account.id.clone();
        let named = !account.name.trim().is_empty();
        sidebar_menu_button(("people-row", index), active)
            .h(px(ROW_HEIGHT))
            .flex_none()
            .aria_label(account.display_name().to_owned())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = Some(id.clone());
                cx.notify();
            }))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(14.))
                                    .line_height(px(20.))
                                    .text_color(if account.disabled {
                                        palette.ink_faint
                                    } else {
                                        palette.ink
                                    })
                                    .child(
                                        label(account.display_name().to_owned()).ltr().truncate(),
                                    ),
                            )
                            .child(standing_badge(account)),
                    )
                    .child(caption(
                        palette,
                        if named {
                            format!("{} · {}", account.email, account.role.label())
                        } else {
                            account.role.label().to_owned()
                        },
                    )),
            )
            .into_any_element()
    }

    fn fact(name: &'static str, value: impl Into<AnyElement>, palette: &Palette) -> AnyElement {
        h_flex()
            .min_h(px(28.))
            .gap(px(8.))
            .items_center()
            .text_size(px(13.))
            .child(
                div()
                    .w(px(FACT_LABEL))
                    .flex_none()
                    .text_color(palette.ink_muted)
                    .child(name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(palette.ink)
                    .child(value.into()),
            )
            .into_any_element()
    }

    fn text(value: String) -> AnyElement {
        div()
            .font_features(crate::ui::tabular())
            .child(label(value).ltr().truncate())
            .into_any_element()
    }

    fn role_picker(&self, user: &Account, locked: bool, cx: &mut Context<Self>) -> AnyElement {
        let current = user.role;
        let items = Role::ALL
            .iter()
            .enumerate()
            .map(|(position, role)| {
                let role = *role;
                let id = user.id.clone();
                SegmentItem::new(
                    ("people-role", position),
                    role.label(),
                    cx.listener(move |this, _, window, cx| {
                        if locked || role == current {
                            return;
                        }
                        this.change(
                            id.clone(),
                            UserChanges {
                                role: Some(role),
                                ..UserChanges::default()
                            },
                            window,
                            cx,
                        );
                    }),
                )
            })
            .collect();
        let selected = Role::ALL
            .iter()
            .position(|role| *role == current)
            .unwrap_or(0);
        h_flex()
            .child(segmented(items, selected))
            .into_any_element()
    }

    fn details(
        &self,
        user: &ManagedAccount,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let account = &user.account;
        let own = own_id(cx).as_deref() == Some(account.id.as_str());
        let busy = self.busy.contains(&account.id);
        let mut actions: Vec<AnyElement> = Vec::new();
        if !account.email_verified {
            let handle = cx.entity().downgrade();
            let id = account.id.clone();
            let email = account.email.clone();
            actions.push(
                primary_sm("people-confirm", "Confirm email")
                    .icon(IconName::MailCheck)
                    .disabled(busy)
                    .busy(busy)
                    .on_click(move |_, window, cx| {
                        Self::act(
                            &handle,
                            &id,
                            format!("Confirmed {email}"),
                            |keeper, id| keeper.verify_user(id).map(Outcome::Updated),
                            window,
                            cx,
                        );
                    })
                    .into_any_element(),
            );
        } else if !account.role.is_admin() {
            actions.push(self.renew_menu(account, busy, cx));
        }
        actions.push(self.more_menu(user, busy, own, cx));
        let email_state = if account.email_verified {
            "Confirmed"
        } else {
            "Not confirmed. They cannot sign in until it is."
        };
        let role_hint = match account.role {
            Role::Member => "Uses FIRs and library templates.",
            Role::Publisher => "Can also publish templates to the library.",
            Role::Admin => "Can also manage people and publishing. Always licensed.",
        };
        v_flex()
            .id("people-details")
            .flex_1()
            .min_w_0()
            .h_full()
            .pl(px(20.))
            .gap(px(16.))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .gap(px(10.))
                            .items_center()
                            .child(
                                div()
                                    .min_w_0()
                                    .text_size(px(20.))
                                    .line_height(px(28.))
                                    .text_color(palette.ink)
                                    .child(
                                        label(account.display_name().to_owned()).ltr().truncate(),
                                    ),
                            )
                            .child(standing_badge(account))
                            .when(own, |row| {
                                row.child(badge(BadgeVariant::Outline).px(px(6.)).child("You"))
                            }),
                    )
                    .child(caption(palette, account.email.clone())),
            )
            .child(h_flex().gap(px(6.)).items_center().children(actions))
            .child(
                v_flex()
                    .gap(px(2.))
                    .child(Self::fact(
                        "Licence",
                        Self::text(licence_text(account)),
                        palette,
                    ))
                    .child(Self::fact(
                        "Email",
                        Self::text(email_state.to_owned()),
                        palette,
                    ))
                    .child(Self::fact(
                        "Signed in",
                        Self::text(sessions_text(user)),
                        palette,
                    ))
                    .child(Self::fact(
                        "Joined",
                        Self::text(date_text(account.created_at)),
                        palette,
                    )),
            )
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(field_label("Role"))
                    .child(self.role_picker(account, busy || own, cx))
                    .child(caption(
                        palette,
                        if own {
                            "You cannot change your own role."
                        } else {
                            role_hint
                        },
                    )),
            )
            .scroll_area_y(&self.details_scroll)
            .into_any_element()
    }

    fn body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        match &self.load {
            Load::Loading => h_flex()
                .gap(px(20.))
                .size_full()
                .items_start()
                .child(
                    v_flex()
                        .w(px(LIST_WIDTH))
                        .gap(px(8.))
                        .children((0..7).map(|_| skeleton_bar(LIST_WIDTH, 48.))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .gap(px(10.))
                        .children((0..5).map(|_| skeleton_bar(WIDTH - LIST_WIDTH - 100., 28.))),
                )
                .into_any_element(),
            Load::Failed(message) => div()
                .pt(px(80.))
                .child(empty_state_body(
                    IconName::TriangleAlert,
                    "Could not load people",
                    Some(SharedString::from(message.clone())),
                    Some(
                        button("people-retry", "Try again")
                            .outline()
                            .sm()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load = Load::Loading;
                                this.reload(cx);
                            }))
                            .into_any_element(),
                    ),
                ))
                .into_any_element(),
            Load::Ready => {
                let rows = self.visible();
                if rows.is_empty() {
                    return div()
                        .pt(px(100.))
                        .child(empty_state_body(
                            IconName::Users,
                            "Nobody here",
                            Some("No accounts match this filter or search.".into()),
                            None,
                        ))
                        .into_any_element();
                }
                let chosen = self.current(&rows).unwrap_or(0);
                let items: Vec<AnyElement> = rows
                    .iter()
                    .enumerate()
                    .map(|(index, row)| self.list_row(index, row, index == chosen, palette, cx))
                    .collect();
                let details = self.details(&rows[chosen], palette, cx);
                h_flex()
                    .size_full()
                    .items_start()
                    .child(
                        v_flex()
                            .id("people-list")
                            .w(px(LIST_WIDTH))
                            .flex_none()
                            .h_full()
                            .pr(px(10.))
                            .gap(px(2.))
                            .border_r_1()
                            .border_color(palette.line)
                            .children(items)
                            .scroll_area_y(&self.list_scroll),
                    )
                    .child(details)
                    .into_any_element()
            }
        }
    }
}

impl Render for UsersPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let height = body_height(window);
        let selected_filter = FILTERS
            .iter()
            .position(|(filter, _)| *filter == self.filter)
            .unwrap_or(0);
        let items = FILTERS
            .iter()
            .enumerate()
            .map(|(index, (filter, title))| {
                let filter = *filter;
                let count = self
                    .users
                    .iter()
                    .filter(|user| filter.admits(&user.account))
                    .count();
                SegmentItem::new(
                    ("people-filter", index),
                    format!("{title} {count}"),
                    cx.listener(move |this, _, _, cx| {
                        this.filter = filter;
                        cx.notify();
                    }),
                )
            })
            .collect();
        let handle = cx.entity().downgrade();
        let loading = matches!(self.load, Load::Loading);
        v_flex()
            .gap(px(12.))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(dialog_title("People"))
                    .child(div().flex_1())
                    .child(
                        icon_button("people-refresh", IconName::RefreshCw, "Refresh")
                            .disabled(loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load = Load::Loading;
                                this.reload(cx);
                            })),
                    )
                    .child(
                        primary_sm("people-add", "Add person")
                            .icon(IconName::UserPlus)
                            .on_click(move |_, window, cx| {
                                open_new_person(handle.clone(), window, cx);
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(segmented(items, selected_filter))
                    .child(div().flex_1())
                    .child(
                        search_field("people-search", &self.search, Kind::Search)
                            .width(px(220.))
                            .height(32.),
                    ),
            )
            .children(
                self.problem
                    .clone()
                    .map(|problem| alert(Intent::Danger).child(problem)),
            )
            .child(div().h(px(height)).child(self.body(&palette, cx)))
            .child(
                dialog_footer().mt(px(12.)).child(div().flex_1()).child(
                    button("people-close", "Close")
                        .primary()
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    }
}

fn open_new_person(panel: WeakEntity<UsersPanel>, window: &mut Window, cx: &mut App) {
    let form = cx.new(|cx| NewPerson::new(panel, window, cx));
    let focus = form.clone();
    window.open_dialog(cx, move |dialog: Dialog, window, cx| {
        let palette = *Palette::get(cx);
        dialog_surface(dialog, &palette, FORM_WIDTH)
            .margin_top(optical_top(window, 560.))
            .child(form.clone())
    });
    window.defer(cx, move |window, cx| {
        let input = focus.read(cx).name.clone();
        input.update(cx, |input, cx| input.focus(window, cx));
    });
}

struct NewPerson {
    panel: WeakEntity<UsersPanel>,
    name: Entity<TextInput>,
    email: Entity<TextInput>,
    password: Entity<TextInput>,
    role: Role,
    term: usize,
    problem: Option<SharedString>,
    busy: bool,
    _subscriptions: Vec<Subscription>,
}

impl NewPerson {
    fn new(panel: WeakEntity<UsersPanel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = field_input(window, cx, Kind::Latin, "Full name", "");
        let email = field_input(window, cx, Kind::Latin, "name@example.com", "");
        let password = secret_input(window, cx, "Temporary password");
        let watch = |input: &Entity<TextInput>, window: &mut Window, cx: &mut Context<Self>| {
            cx.subscribe_in(
                input,
                window,
                |this, _, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Submit => this.save(window, cx),
                    TextInputEvent::Change => {
                        this.problem = None;
                        cx.notify();
                    }
                    _ => {}
                },
            )
        };
        let subscriptions = vec![
            watch(&name, window, cx),
            watch(&email, window, cx),
            watch(&password, window, cx),
        ];
        NewPerson {
            panel,
            name,
            email,
            password,
            role: Role::Member,
            term: 0,
            problem: None,
            busy: false,
            _subscriptions: subscriptions,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let name = self.name.read(cx).text().trim().to_owned();
        let email = self.email.read(cx).text().trim().to_lowercase();
        let password = self.password.read(cx).text().to_owned();
        let valid_email = email.split_once('@').is_some_and(|(user, host)| {
            !user.is_empty() && host.contains('.') && !host.ends_with('.')
        }) && !email.contains(char::is_whitespace);
        let problem = if !valid_email {
            Some("Enter a valid email address.")
        } else {
            password_problem(&password, &password)
        };
        if let Some(problem) = problem {
            self.problem = Some(problem.into());
            cx.notify();
            return;
        }
        let Some(keeper) = keeper(cx) else {
            return;
        };
        let user = NewUser {
            email: email.clone(),
            name,
            role: self.role,
            password,
            months: START_TERMS[self.term].0,
        };
        self.busy = true;
        cx.notify();
        let task = cx.background_spawn(async move { keeper.create_user(&user) });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match outcome {
                    Ok(account) => {
                        let id = account.account.id.clone();
                        this.panel
                            .update(cx, |panel, cx| {
                                panel.upsert(account);
                                panel.filter = Filter::All;
                                panel.selected = Some(id);
                                cx.notify();
                            })
                            .ok();
                        window.close_dialog(cx);
                        toast_success(window, cx, format!("Created an account for {email}"));
                    }
                    Err(error) => {
                        this.problem = Some(error.to_string().into());
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }
}

impl Render for NewPerson {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = px(FORM_WIDTH - 48.);
        let roles = Role::ALL
            .iter()
            .enumerate()
            .map(|(index, role)| {
                let role = *role;
                SegmentItem::new(
                    ("new-person-role", index),
                    role.label(),
                    cx.listener(move |this, _, _, cx| {
                        this.role = role;
                        cx.notify();
                    }),
                )
            })
            .collect();
        let role_index = Role::ALL
            .iter()
            .position(|role| *role == self.role)
            .unwrap_or(0);
        let terms = START_TERMS
            .iter()
            .enumerate()
            .map(|(index, (_, term))| {
                SegmentItem::new(
                    ("new-person-term", index),
                    *term,
                    cx.listener(move |this, _, _, cx| {
                        this.term = index;
                        cx.notify();
                    }),
                )
            })
            .collect();
        let licence_hint = if self.role.is_admin() {
            "Administrators are always licensed."
        } else if self.term == 0 {
            "The trial starts today. Renew it from People when they pay."
        } else {
            "Paid time starts today."
        };
        v_flex()
            .child(
                dialog_header()
                    .child(dialog_title("Add person"))
                    .child(dialog_description(
                        "The account is ready to use at once, with its email already confirmed. Give them the password; they can change it from their account menu.",
                    )),
            )
            .child(
                field_group()
                    .mt(px(20.))
                    .child(
                        form_field()
                            .child(field_label("Name"))
                            .child(field("new-person-name", &self.name, Kind::Latin).width(width)),
                    )
                    .child(
                        form_field()
                            .child(field_label("Email"))
                            .child(field("new-person-email", &self.email, Kind::Latin).width(width)),
                    )
                    .child(
                        form_field()
                            .child(field_label("Password"))
                            .child(field("new-person-password", &self.password, Kind::Latin).width(width))
                            .child(field_description("At least 10 characters.")),
                    )
                    .child(
                        form_field()
                            .child(field_label("Role"))
                            .child(h_flex().child(segmented(roles, role_index))),
                    )
                    .child(
                        form_field()
                            .child(field_label("Licence"))
                            .child(h_flex().child(segmented(terms, self.term)))
                            .child(field_description(licence_hint))
                            .children(self.problem.clone().map(field_error)),
                    ),
            )
            .child(
                dialog_footer()
                    .child(div().flex_1())
                    .child(
                        button("new-person-cancel", "Cancel")
                            .outline()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button(
                            "new-person-save",
                            if self.busy { "Creating…" } else { "Create account" },
                        )
                        .primary()
                        .busy(self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

fn open_set_password(
    panel: WeakEntity<UsersPanel>,
    id: String,
    email: String,
    window: &mut Window,
    cx: &mut App,
) {
    let form = cx.new(|cx| SetPassword::new(panel, id, email, window, cx));
    let focus = form.clone();
    window.open_dialog(cx, move |dialog: Dialog, window, cx| {
        let palette = *Palette::get(cx);
        dialog_surface(dialog, &palette, FORM_WIDTH)
            .margin_top(optical_top(window, 320.))
            .child(form.clone())
    });
    window.defer(cx, move |window, cx| {
        let input = focus.read(cx).new.clone();
        input.update(cx, |input, cx| input.focus(window, cx));
    });
}

struct SetPassword {
    panel: WeakEntity<UsersPanel>,
    id: String,
    email: String,
    new: Entity<TextInput>,
    confirm: Entity<TextInput>,
    problem: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl SetPassword {
    fn new(
        panel: WeakEntity<UsersPanel>,
        id: String,
        email: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let new = secret_input(window, cx, "New password");
        let confirm = secret_input(window, cx, "Repeat the new password");
        let watch = |input: &Entity<TextInput>, window: &mut Window, cx: &mut Context<Self>| {
            cx.subscribe_in(
                input,
                window,
                |this, _, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Submit => this.save(window, cx),
                    TextInputEvent::Change => {
                        this.problem = None;
                        cx.notify();
                    }
                    _ => {}
                },
            )
        };
        let subscriptions = vec![watch(&new, window, cx), watch(&confirm, window, cx)];
        SetPassword {
            panel,
            id,
            email,
            new,
            confirm,
            problem: None,
            _subscriptions: subscriptions,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new = self.new.read(cx).text().to_owned();
        let confirm = self.confirm.read(cx).text().to_owned();
        if let Some(problem) = password_problem(&new, &confirm) {
            self.problem = Some(problem.into());
            cx.notify();
            return;
        }
        window.close_dialog(cx);
        UsersPanel::act(
            &self.panel,
            &self.id,
            format!("New password set for {}", self.email),
            move |keeper, id| keeper.set_password(id, &new).map(Outcome::Updated),
            window,
            cx,
        );
    }
}

impl Render for SetPassword {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = px(FORM_WIDTH - 48.);
        v_flex()
            .child(
                dialog_header()
                    .child(dialog_title("Set a new password"))
                    .child(dialog_description(format!(
                        "{} is signed out everywhere and signs in with this password next time.",
                        self.email
                    ))),
            )
            .child(
                field_group()
                    .mt(px(20.))
                    .child(
                        form_field()
                            .child(field_label("New password"))
                            .child(field("set-password-new", &self.new, Kind::Latin).width(width))
                            .child(field_description("At least 10 characters.")),
                    )
                    .child(
                        form_field()
                            .child(field_label("Repeat the new password"))
                            .child(
                                field("set-password-confirm", &self.confirm, Kind::Latin)
                                    .width(width),
                            )
                            .children(self.problem.clone().map(field_error)),
                    ),
            )
            .child(
                dialog_footer()
                    .child(div().flex_1())
                    .child(
                        button("set-password-cancel", "Cancel")
                            .outline()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        button("set-password-save", "Set password")
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}
