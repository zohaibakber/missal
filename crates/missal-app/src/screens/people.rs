use crate::ui::ScrollArea as _;
use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::avatar::Avatar;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, ScrollHandle, SharedString, Styled,
    Subscription, WeakEntity, Window, div, px,
};
use missal_auth::{
    Account, AuthError, LicenceStatus, ManagedAccount, NewUser, Role, SessionKeeper, UserChanges,
};
use missal_text::{TextInput, TextInputEvent, label};

use crate::auth::{AuthModel, password_problem, password_too_short};
use crate::shortcuts::BlurFocus;
use crate::state::{Load, toast_success};
use crate::theme::{PAGE_GUTTER, Palette};
use crate::ui::admin::{
    ACTIONS_WIDTH, Confirm, FilterOption, PANE_PAD, PANE_WIDTH, admins_only, cell, cell_text,
    confirm_card, data_row, fact, fact_text, filter_menu, head_cell, is_admin, page_toolbar, pane,
    pane_footer, pane_header, pane_section, pane_title,
};
use crate::ui::dialogs::Tone;
use crate::ui::input::{Kind, field, field_input, search_field, secret_input};
use crate::ui::library::day_month_year;
use crate::ui::popup_menu::{MenuContent, MenuEntry, menu};
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::{
    BadgeVariant, Intent, SegmentItem, alert, badge, button, empty_state, field_description,
    field_error, field_group, field_label, form_field, icon_button, primary_sm, segmented,
    skeleton_bar, table, table_body, table_footer, table_header,
};

const ROLE_WIDTH: f32 = 112.;
const STATUS_WIDTH: f32 = 184.;
const LICENCE_WIDTH: f32 = 168.;
const SEEN_WIDTH: f32 = 132.;
const SEARCH_WIDTH: f32 = 260.;
const FIELD_WIDTH: f32 = PANE_WIDTH - PANE_PAD * 2.;
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
        .min_w_0()
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

fn licence_cell_text(user: &Account) -> String {
    if user.role.is_admin() {
        "—".to_owned()
    } else if !user.email_verified {
        "Not started".to_owned()
    } else {
        format!("Until {}", date_text(user.licence.until_ms))
    }
}

fn sessions_text(user: &ManagedAccount) -> String {
    let seen = user
        .last_seen_at
        .map_or_default(|at| format!(" · last active {}", date_text(at)));
    match user.sessions {
        0 => "Not signed in".to_owned(),
        1 => format!("On 1 computer{seen}"),
        count => format!("On {count} computers{seen}"),
    }
}

fn seen_text(user: &ManagedAccount) -> String {
    match user.last_seen_at {
        Some(at) => date_text(at),
        None => "Never".to_owned(),
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

enum Outcome {
    Updated(ManagedAccount),
    Removed,
    Unchanged,
}

type Job = Box<dyn FnOnce(&SessionKeeper, &str) -> Result<Outcome, AuthError> + Send>;

enum Pane {
    Closed,
    Person(String),
    New(Entity<NewPerson>),
}

pub struct PeopleScreen {
    focus: FocusHandle,
    table_scroll: ScrollHandle,
    pane_scroll: ScrollHandle,
    users: Vec<ManagedAccount>,
    load: Load,
    busy: HashSet<String>,
    problem: Option<SharedString>,
    filter: Filter,
    pane: Pane,
    confirm: Option<Confirm>,
    password: Option<Entity<SetPassword>>,
    search: Entity<TextInput>,
    query: String,
    _subscriptions: Vec<Subscription>,
}

impl PeopleScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = field_input(window, cx, Kind::Search, "Search name or email…", "");
        let mut subscriptions =
            vec![
                cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                    if *event == TextInputEvent::Change {
                        this.query = input.read(cx).text().trim().to_lowercase();
                        cx.notify();
                    }
                }),
            ];
        if let Some(auth) = AuthModel::get(cx) {
            subscriptions.push(cx.observe(&auth, |_, _, cx| cx.notify()));
        }
        let mut screen = PeopleScreen {
            focus: cx.focus_handle(),
            table_scroll: ScrollHandle::new(),
            pane_scroll: ScrollHandle::new(),
            users: Vec::new(),
            load: Load::Loading,
            busy: HashSet::new(),
            problem: None,
            filter: Filter::All,
            pane: Pane::Closed,
            confirm: None,
            password: None,
            search,
            query: String::new(),
            _subscriptions: subscriptions,
        };
        screen.reload(cx);
        screen
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn is_dirty(&self, _: &App) -> bool {
        false
    }

    pub fn open_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.pane, Pane::New(_)) {
            return;
        }
        let handle = cx.entity().downgrade();
        let form = cx.new(|cx| NewPerson::new(handle, window, cx));
        let input = form.read(cx).name.clone();
        window.defer(cx, move |window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
        });
        self.confirm = None;
        self.password = None;
        self.pane = Pane::New(form);
        cx.notify();
    }

    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        if self
            .confirm
            .as_ref()
            .is_some_and(|confirm| *confirm.key != *id)
        {
            self.confirm = None;
        }
        if self
            .password
            .as_ref()
            .is_some_and(|form| form.read(cx).id != id)
        {
            self.password = None;
        }
        self.pane = Pane::Person(id);
        cx.notify();
    }

    fn close_pane(&mut self, cx: &mut Context<Self>) {
        self.pane = Pane::Closed;
        self.confirm = None;
        self.password = None;
        cx.notify();
    }

    fn ask(&mut self, confirm: Confirm, cx: &mut Context<Self>) {
        let key = confirm.key.to_string();
        self.select(key, cx);
        self.confirm = Some(confirm);
        cx.notify();
    }

    fn open_password(
        &mut self,
        id: String,
        email: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select(id.clone(), cx);
        let handle = cx.entity().downgrade();
        let form = cx.new(|cx| SetPassword::new(handle, id, email, window, cx));
        let input = form.read(cx).new.clone();
        window.defer(cx, move |window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
        });
        self.confirm = None;
        self.password = Some(form);
        cx.notify();
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
        let Some(keeper) = keeper(cx).filter(|_| is_admin(cx)) else {
            return;
        };
        self.load = Load::Loading;
        cx.notify();
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

    fn chosen(&self) -> Option<&ManagedAccount> {
        match &self.pane {
            Pane::Person(id) => self.users.iter().find(|user| &user.account.id == id),
            _ => None,
        }
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
                        if matches!(&this.pane, Pane::Person(open) if *open == id) {
                            this.close_pane(cx);
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

    fn renew_entries(
        mut content: MenuContent,
        handle: &WeakEntity<Self>,
        account: &Account,
        prefix: &str,
    ) -> MenuContent {
        for (months, term) in TERMS {
            let handle = handle.clone();
            let id = account.id.clone();
            let email = account.email.clone();
            content = content.item(
                MenuEntry::new(
                    format!("{prefix}-{months}"),
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
                )
                .icon(IconName::CalendarPlus),
            );
        }
        content
    }

    fn renew_menu(&self, user: &Account, busy: bool, cx: &mut Context<Self>) -> AnyElement {
        let handle = cx.entity().downgrade();
        let account = user.clone();
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
            .anchor(gpui_kit::Anchor::TopLeft)
            .width(200.)
            .content(move |content, _, _| {
                Self::renew_entries(content, &handle, &account, "people-renew")
            })
            .into_any_element()
    }

    fn action_entries(
        mut content: MenuContent,
        handle: &WeakEntity<Self>,
        user: &ManagedAccount,
        own: bool,
        with_renew: bool,
    ) -> MenuContent {
        let account = user.account.clone();
        let signed_in = user.sessions > 0;
        let id = account.id.clone();
        let email = account.email.clone();
        let admin = account.role.is_admin();
        let verified = account.email_verified;
        let disabled = account.disabled;
        if !verified {
            let handle = handle.clone();
            let id = id.clone();
            let email = email.clone();
            content = content
                .item(
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
                )
                .separator();
        }
        if !admin {
            if with_renew && verified {
                content =
                    Self::renew_entries(content, handle, &account, "people-row-renew").separator();
            }
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
                                move |keeper, id| {
                                    keeper.extend_trial(id, days).map(Outcome::Updated)
                                },
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
                MenuEntry::new("people-end-licence", "End licence now…", move |_, cx| {
                    let act = handle.clone();
                    let target = id.clone();
                    let address = email.clone();
                    let confirm = Confirm::new(
                        id.clone(),
                        "end-confirm",
                        format!("End the licence for {email}?"),
                        "Their trial or paid time ends today. They can still sign in, but the template library closes until you renew it.",
                        "End licence",
                        Tone::Danger,
                        move |window, cx| {
                            Self::act(
                                &act,
                                &target,
                                format!("Licence ended for {address}"),
                                |keeper, id| keeper.end_licence(id).map(Outcome::Updated),
                                window,
                                cx,
                            );
                        },
                    );
                    handle.update(cx, |this, cx| this.ask(confirm, cx)).ok();
                })
                .icon(IconName::CalendarX)
                .disabled(!active)
            });
            content = content.separator();
        }
        content = content.item({
            let handle = handle.clone();
            let id = id.clone();
            let email = email.clone();
            MenuEntry::new(
                "people-set-password",
                "Set a new password…",
                move |window, cx| {
                    let id = id.clone();
                    let email = email.clone();
                    handle
                        .update(cx, |this, cx| this.open_password(id, email, window, cx))
                        .ok();
                },
            )
            .icon(IconName::KeyRound)
            .disabled(own)
        });
        content = content.item({
            let handle = handle.clone();
            let id = id.clone();
            let email = email.clone();
            MenuEntry::new(
                "people-reset-email",
                "Email a reset code",
                move |window, cx| {
                    Self::act(
                        &handle,
                        &id,
                        format!("Reset code sent to {email}"),
                        |keeper, id| keeper.send_reset(id).map(|()| Outcome::Unchanged),
                        window,
                        cx,
                    );
                },
            )
            .icon(IconName::Mail)
            .disabled(!verified || disabled)
        });
        content = content.item({
            let handle = handle.clone();
            let id = id.clone();
            let email = email.clone();
            MenuEntry::new(
                "people-sign-out",
                "Sign out everywhere",
                move |window, cx| {
                    Self::act(
                        &handle,
                        &id,
                        format!("Signed {email} out everywhere"),
                        |keeper, id| keeper.sign_out_user(id).map(Outcome::Updated),
                        window,
                        cx,
                    );
                },
            )
            .icon(IconName::LogOut)
            .disabled(own || !signed_in)
        });
        content = content.separator();
        content = content.item({
            let handle = handle.clone();
            let id = id.clone();
            let entry = MenuEntry::new(
                "people-toggle",
                if disabled {
                    "Enable account"
                } else {
                    "Disable account"
                },
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
            .icon(if disabled {
                IconName::UserCheck
            } else {
                IconName::UserX
            })
            .disabled(own);
            if disabled { entry } else { entry.danger() }
        });
        let handle = handle.clone();
        content.item(
            MenuEntry::new("people-delete", "Delete account…", move |_, cx| {
                let act = handle.clone();
                let target = id.clone();
                let address = email.clone();
                let confirm = Confirm::new(
                    id.clone(),
                    "delete-confirm",
                    format!("Delete {email}?"),
                    "The account, its licence and its sign-ins are removed for good. FIRs on their computers stay there. Disable the account instead if they might come back.",
                    "Delete account",
                    Tone::Danger,
                    move |window, cx| {
                        Self::act(
                            &act,
                            &target,
                            format!("Deleted {address}"),
                            |keeper, id| keeper.delete_user(id).map(|()| Outcome::Removed),
                            window,
                            cx,
                        );
                    },
                );
                handle.update(cx, |this, cx| this.ask(confirm, cx)).ok();
            })
            .icon(IconName::Trash)
            .danger()
            .disabled(own),
        )
    }

    fn more_menu(
        &self,
        user: &ManagedAccount,
        busy: bool,
        own: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let handle = cx.entity().downgrade();
        let user = user.clone();
        menu("people-more")
            .trigger(icon_button("people-more", IconName::Ellipsis, "More actions").disabled(busy))
            .width(248.)
            .content(move |content, _, _| Self::action_entries(content, &handle, &user, own, false))
            .into_any_element()
    }

    fn head(&self, wide: bool) -> AnyElement {
        table_header()
            .id("people-table-head")
            .child(head_cell("Name").flex_1().min_w_0())
            .child(head_cell("Role").w(px(ROLE_WIDTH)).flex_none())
            .child(head_cell("Status").w(px(STATUS_WIDTH)).flex_none())
            .when(wide, |this| {
                this.child(head_cell("Licence").w(px(LICENCE_WIDTH)).flex_none())
                    .child(head_cell("Last active").w(px(SEEN_WIDTH)).flex_none())
            })
            .child(div().w(px(ACTIONS_WIDTH)).flex_none())
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        index: usize,
        user: &ManagedAccount,
        last: bool,
        wide: bool,
        own: Option<&str>,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let account = &user.account;
        let id = account.id.clone();
        let selected = matches!(&self.pane, Pane::Person(open) if *open == account.id);
        let named = !account.name.trim().is_empty();
        let mine = own == Some(account.id.as_str());
        let handle = cx.entity().downgrade();
        let menu_user = user.clone();
        let ink = if account.disabled {
            palette.ink_faint
        } else {
            palette.ink
        };
        data_row(("people-row", index), "people-row", selected, last, palette)
            .aria_label(SharedString::from(account.display_name().to_owned()))
            .on_click(cx.listener(move |this, _, _, cx| this.select(id.clone(), cx)))
            .child(
                cell()
                    .flex_1()
                    .gap(px(10.))
                    .child(
                        Avatar::new()
                            .name(SharedString::from(account.display_name().to_owned()))
                            .small(),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap(px(6.))
                                    .items_center()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .text_size(px(14.))
                                            .line_height(px(20.))
                                            .text_color(ink)
                                            .child(
                                                label(account.display_name().to_owned())
                                                    .ltr()
                                                    .truncate(),
                                            ),
                                    )
                                    .when(mine, |row| {
                                        row.child(
                                            badge(BadgeVariant::Outline).px(px(5.)).child("You"),
                                        )
                                    }),
                            )
                            .when(named, |this| {
                                this.child(caption(palette, account.email.clone()))
                            }),
                    ),
            )
            .child(
                cell()
                    .w(px(ROLE_WIDTH))
                    .flex_none()
                    .child(cell_text(account.role.label().to_owned(), palette)),
            )
            .child(
                cell()
                    .w(px(STATUS_WIDTH))
                    .flex_none()
                    .child(standing_badge(account)),
            )
            .when(wide, |this| {
                this.child(
                    cell()
                        .w(px(LICENCE_WIDTH))
                        .flex_none()
                        .child(cell_text(licence_cell_text(account), palette)),
                )
                .child(
                    cell()
                        .w(px(SEEN_WIDTH))
                        .flex_none()
                        .child(cell_text(seen_text(user), palette)),
                )
            })
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
                        div().child(
                            menu(format!("people-row-menu-{index}"))
                                .trigger(
                                    icon_button(
                                        ("people-row-more", index),
                                        IconName::Ellipsis,
                                        "Actions",
                                    )
                                    .dense(),
                                )
                                .width(248.)
                                .content(move |content, _, _| {
                                    Self::action_entries(content, &handle, &menu_user, mine, true)
                                }),
                        ),
                    ),
            )
            .test_support()
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
            .child(segmented("person-role", items, selected).manual())
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
        let confirm = self
            .confirm
            .as_ref()
            .filter(|confirm| *confirm.key == *account.id)
            .map(|confirm| {
                let handle = cx.entity().downgrade();
                confirm_card(confirm, move |_, cx| {
                    handle
                        .update(cx, |this, cx| {
                            this.confirm = None;
                            cx.notify();
                        })
                        .ok();
                })
            });
        let password = self
            .password
            .as_ref()
            .filter(|form| form.read(cx).id == account.id)
            .cloned();
        let close = cx.entity().downgrade();
        let title = h_flex()
            .gap(px(8.))
            .items_center()
            .child(pane_title(account.display_name().to_owned()))
            .when(own, |row| {
                row.child(badge(BadgeVariant::Outline).px(px(6.)).child("You"))
            });
        let subtitle = h_flex()
            .gap(px(8.))
            .items_center()
            .child(caption(palette, account.email.clone()))
            .into_any_element();
        pane()
            .id("people-details")
            .child(pane_header(
                title,
                Some(subtitle),
                "people-close",
                move |_, cx| {
                    close.update(cx, |this, cx| this.close_pane(cx)).ok();
                },
            ))
            .child(
                v_flex()
                    .id("people-details-body")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .px(px(PANE_PAD))
                            .pb(px(16.))
                            .gap(px(12.))
                            .child(
                                h_flex()
                                    .gap(px(6.))
                                    .items_center()
                                    .child(standing_badge(account))
                                    .child(div().flex_1())
                                    .children(actions),
                            )
                            .children(confirm)
                            .children(password.map(|form| form.into_any_element())),
                    )
                    .child(
                        pane_section("Account").child(
                            v_flex()
                                .child(fact("Licence", fact_text(licence_text(account)), palette))
                                .child(fact("Email", fact_text(email_state), palette))
                                .child(fact("Signed in", fact_text(sessions_text(user)), palette))
                                .child(fact(
                                    "Joined",
                                    fact_text(date_text(account.created_at)),
                                    palette,
                                )),
                        ),
                    )
                    .child(
                        pane_section("Role")
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
                    .scroll_area_y(&self.pane_scroll),
            )
            .into_any_element()
    }

    fn skeleton(&self, wide: bool) -> AnyElement {
        table()
            .child(self.head(wide))
            .child(
                table_body().child(v_flex().children((0..8usize).map(|index| {
                    h_flex()
                        .id(("people-skeleton", index))
                        .h(px(crate::ui::admin::ROW_HEIGHT))
                        .px(px(12.))
                        .gap(px(12.))
                        .items_center()
                        .child(skeleton_bar(24., 24.).rounded_full())
                        .child(skeleton_bar(180., 12.))
                }))),
            )
            .into_any_element()
    }

    fn body(&self, wide: bool, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        match &self.load {
            Load::Loading => self.skeleton(wide),
            Load::Failed(message) => empty_state(
                IconName::TriangleAlert,
                "Could not load people",
                Some(SharedString::from(message.clone())),
                Some(
                    button("people-retry", "Try again")
                        .outline()
                        .sm()
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx)))
                        .into_any_element(),
                ),
            )
            .into_any_element(),
            Load::Ready => {
                let rows = self.visible();
                if rows.is_empty() {
                    return empty_state(
                        IconName::Users,
                        "Nobody here",
                        Some("No accounts match this filter or search.".into()),
                        None,
                    )
                    .into_any_element();
                }
                let own = own_id(cx);
                let count = rows.len();
                let items: Vec<AnyElement> = rows
                    .iter()
                    .enumerate()
                    .map(|(index, row)| {
                        self.row(
                            index,
                            row,
                            index + 1 == count,
                            wide,
                            own.as_deref(),
                            palette,
                            cx,
                        )
                    })
                    .collect();
                let total = self.users.len();
                let summary = if count == total {
                    format!("{total} {}", if total == 1 { "person" } else { "people" })
                } else {
                    format!("Showing {count} of {total} people")
                };
                table()
                    .child(self.head(wide))
                    .child(
                        table_body().child(
                            v_flex()
                                .id("people-rows")
                                .size_full()
                                .children(items)
                                .scroll_area_y(&self.table_scroll),
                        ),
                    )
                    .child(table_footer().child(summary))
                    .into_any_element()
            }
        }
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected_filter = FILTERS
            .iter()
            .position(|(filter, _)| *filter == self.filter)
            .unwrap_or(0);
        let items = FILTERS
            .iter()
            .map(|(filter, title)| {
                let filter = *filter;
                let count = self
                    .users
                    .iter()
                    .filter(|user| filter.admits(&user.account))
                    .count();
                let handle = cx.entity().downgrade();
                FilterOption {
                    title,
                    count,
                    action: std::rc::Rc::new(move |_, cx| {
                        handle
                            .update(cx, |this, cx| {
                                this.filter = filter;
                                cx.notify();
                            })
                            .ok();
                    }),
                }
            })
            .collect();
        page_toolbar()
            .child(
                div().flex_1().min_w_0().max_w(px(SEARCH_WIDTH)).child(
                    search_field("people-search", &self.search, Kind::Search)
                        .width(px(SEARCH_WIDTH))
                        .height(32.),
                ),
            )
            .child(div().flex_1())
            .child(filter_menu("people-filter", selected_filter, items))
            .into_any_element()
    }
}

impl Focusable for PeopleScreen {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PeopleScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let root = v_flex()
            .id("people")
            .track_focus(&self.focus)
            .size_full()
            .bg(palette.surface);
        if !is_admin(cx) {
            return root.child(admins_only()).into_any_element();
        }
        let side: Option<AnyElement> = match &self.pane {
            Pane::Closed => None,
            Pane::New(form) => Some(form.clone().into_any_element()),
            Pane::Person(_) => self
                .chosen()
                .cloned()
                .map(|user| self.details(&user, &palette, cx)),
        };
        let wide = side.is_none();
        root.on_action(cx.listener(|this, _: &BlurFocus, _, cx| {
            if matches!(this.pane, Pane::Closed) {
                cx.propagate();
            } else {
                this.close_pane(cx);
            }
        }))
        .child(
            h_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.toolbar(cx))
                        .children(self.problem.clone().map(|problem| {
                            div()
                                .px(px(PAGE_GUTTER))
                                .pb(px(12.))
                                .child(alert(Intent::Danger).child(problem))
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .px(px(PAGE_GUTTER))
                                .pb(px(PAGE_GUTTER))
                                .child(self.body(wide, &palette, cx)),
                        ),
                )
                .children(side),
        )
        .into_any_element()
    }
}

impl HeaderSource for PeopleScreen {
    fn header(&mut self, _: &mut Window, cx: &mut Context<Self>) -> HeaderContent {
        let content = HeaderContent::new("People");
        if !is_admin(cx) {
            return content;
        }
        let loading = matches!(self.load, Load::Loading);
        let content = if self.load.is_ready() {
            let total = self.users.len();
            content.meta(format!(
                "{total} {}",
                if total == 1 { "account" } else { "accounts" }
            ))
        } else {
            content
        };
        content
            .action(
                icon_button("people-refresh", IconName::RefreshCw, "Refresh")
                    .disabled(loading)
                    .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
            )
            .action(
                primary_sm("people-add", "Add person")
                    .icon(IconName::UserPlus)
                    .on_click(cx.listener(|this, _, window, cx| this.open_new(window, cx))),
            )
    }
}

struct NewPerson {
    screen: WeakEntity<PeopleScreen>,
    scroll: ScrollHandle,
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
    fn new(screen: WeakEntity<PeopleScreen>, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            screen,
            scroll: ScrollHandle::new(),
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

    fn cancel(&self, cx: &mut App) {
        self.screen
            .update(cx, |screen, cx| screen.close_pane(cx))
            .ok();
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
            let field = if valid_email {
                &self.password
            } else {
                &self.email
            };
            field.update(cx, |input, cx| input.focus(window, cx));
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
                        this.screen
                            .update(cx, |screen, cx| {
                                screen.upsert(account);
                                screen.filter = Filter::All;
                                screen.select(id, cx);
                            })
                            .ok();
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
        let palette = *Palette::get(cx);
        let width = px(FIELD_WIDTH);
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
        let close = cx.entity().downgrade();
        pane()
            .id("new-person")
            .child(pane_header(
                pane_title("Add person"),
                Some(caption(&palette, "Ready to use at once, email already confirmed.")),
                "new-person-close",
                move |_, cx| {
                    close.update(cx, |this, cx| this.cancel(cx)).ok();
                },
            ))
            .child(
                v_flex()
                    .id("new-person-body")
                    .flex_1()
                    .min_h_0()
                    .child(
                        field_group()
                            .px(px(PANE_PAD))
                            .pt(px(4.))
                            .pb(px(20.))
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
                                    .child(
                                        field("new-person-password", &self.password, Kind::Latin)
                                            .width(width),
                                    )
                                    .child(field_description(
                                        "At least 10 characters. Give it to them; they can change it from their account menu.",
                                    )),
                            )
                            .child(
                                form_field()
                                    .child(field_label("Role"))
                                    .child(h_flex().child(segmented("new-person-role", roles, role_index))),
                            )
                            .child(
                                form_field()
                                    .child(field_label("Licence"))
                                    .child(h_flex().child(segmented("new-person-licence", terms, self.term)))
                                    .child(field_description(licence_hint))
                                    .children(self.problem.clone().map(field_error)),
                            ),
                    )
                    .scroll_area_y(&self.scroll),
            )
            .child(
                pane_footer()
                    .child(
                        button("new-person-cancel", "Cancel")
                            .outline()
                            .sm()
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    )
                    .child(
                        button(
                            "new-person-save",
                            if self.busy {
                                "Creating…"
                            } else {
                                "Create account"
                            },
                        )
                        .primary()
                        .sm()
                        .busy(self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

struct SetPassword {
    screen: WeakEntity<PeopleScreen>,
    id: String,
    email: String,
    new: Entity<TextInput>,
    confirm: Entity<TextInput>,
    problem: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl SetPassword {
    fn new(
        screen: WeakEntity<PeopleScreen>,
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
            screen,
            id,
            email,
            new,
            confirm,
            problem: None,
            _subscriptions: subscriptions,
        }
    }

    fn dismiss(&self, cx: &mut App) {
        self.screen
            .update(cx, |screen, cx| {
                screen.password = None;
                cx.notify();
            })
            .ok();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new = self.new.read(cx).text().to_owned();
        let confirm = self.confirm.read(cx).text().to_owned();
        if let Some(problem) = password_problem(&new, &confirm) {
            self.problem = Some(problem.into());
            let field = if password_too_short(&new) {
                &self.new
            } else {
                &self.confirm
            };
            field.update(cx, |input, cx| input.focus(window, cx));
            cx.notify();
            return;
        }
        self.dismiss(cx);
        PeopleScreen::act(
            &self.screen,
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
        let palette = *Palette::get(cx);
        let width = px(FIELD_WIDTH - 30.);
        v_flex()
            .id("set-password")
            .gap(px(14.))
            .p(px(14.))
            .rounded(px(crate::theme::RADIUS_XL))
            .border_1()
            .border_color(palette.line)
            .bg(palette.hover.opacity(0.4))
            .child(
                v_flex()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .child("Set a new password"),
                    )
                    .child(caption(
                        &palette,
                        "They are signed out everywhere and use it next time.",
                    )),
            )
            .child(
                form_field()
                    .child(field_label("New password"))
                    .child(field("set-password-new", &self.new, Kind::Latin).width(width))
                    .child(field_description("At least 10 characters.")),
            )
            .child(
                form_field()
                    .child(field_label("Repeat the new password"))
                    .child(field("set-password-confirm", &self.confirm, Kind::Latin).width(width))
                    .children(self.problem.clone().map(field_error)),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .justify_end()
                    .child(
                        button("set-password-cancel", "Cancel")
                            .outline()
                            .sm()
                            .on_click(cx.listener(|this, _, _, cx| this.dismiss(cx))),
                    )
                    .child(
                        button("set-password-save", "Set password")
                            .primary()
                            .sm()
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}
