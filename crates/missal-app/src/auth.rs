use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui_kit::component::v_flex;
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, Global, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Task, Window, div, px,
};
use missal_auth::{Account, AuthError, Config, Session, SessionKeeper, SessionStore};
use missal_text::{TextInput, TextInputEvent};

use crate::state::{Models, toast_warning};
use crate::theme::{Palette, RADIUS_XL};
use crate::ui::input::{Kind, field, field_input, secret_input};

const REFRESH_EVERY: Duration = Duration::from_secs(60 * 60);
pub const MIN_PASSWORD: usize = 10;

const ENDED: &str = "Your session has ended. Sign in again to continue.";
const STALE: &str =
    "Missal has been offline for more than 30 days. Connect to the internet and sign in again.";

#[derive(Clone)]
pub enum AuthState {
    Disabled,
    Restoring,
    SignedOut {
        notice: Option<SharedString>,
    },
    SigningIn,
    MustChangePassword {
        account: Account,
        notice: Option<SharedString>,
        busy: bool,
    },
    SignedIn {
        account: Account,
        online: bool,
    },
}

pub struct AuthModel {
    config: Option<Config>,
    keeper: Option<Arc<SessionKeeper>>,
    store: Option<Arc<SessionStore>>,
    session: Option<Session>,
    state: AuthState,
    refresh_loop: Option<Task<()>>,
}

struct AuthHandle(Entity<AuthModel>);

impl Global for AuthHandle {}

pub fn password_problem(password: &str, confirm: &str) -> Option<&'static str> {
    if password.chars().count() < MIN_PASSWORD {
        Some("Use at least 10 characters.")
    } else if password != confirm {
        Some("The two new passwords do not match.")
    } else {
        None
    }
}

impl AuthModel {
    pub fn install(data_directory: Option<&Path>, cx: &mut App) -> Entity<AuthModel> {
        let config = Config::resolve();
        if config.is_none() {
            eprintln!("missal: sign-in is off because MISSAL_API_URL is not set");
        }
        let store = match (&config, data_directory) {
            (Some(_), Some(directory)) => {
                Some(Arc::new(SessionStore::for_data_directory(directory)))
            }
            _ => None,
        };
        let state = match &config {
            None => AuthState::Disabled,
            Some(_) => AuthState::Restoring,
        };
        let keeper = config
            .as_ref()
            .map(|config| Arc::new(SessionKeeper::new(config, store.clone())));
        let entity = cx.new(|_| AuthModel {
            config,
            keeper,
            store,
            session: None,
            state,
            refresh_loop: None,
        });
        cx.set_global(AuthHandle(entity.clone()));
        entity.update(cx, |model, cx| model.restore(cx));
        entity
    }

    pub fn get(cx: &App) -> Option<Entity<AuthModel>> {
        cx.try_global::<AuthHandle>().map(|handle| handle.0.clone())
    }

    pub fn api_url(&self) -> Option<&str> {
        self.config.as_ref().map(|config| config.base_url.as_str())
    }

    pub fn state(&self) -> &AuthState {
        &self.state
    }

    pub fn keeper(&self) -> Option<Arc<SessionKeeper>> {
        self.keeper.clone()
    }

    pub fn unlocked(&self) -> bool {
        matches!(self.state, AuthState::Disabled | AuthState::SignedIn { .. })
    }

    pub fn account(&self) -> Option<&Account> {
        match &self.state {
            AuthState::SignedIn { account, .. } => Some(account),
            _ => None,
        }
    }

    fn keep(&mut self, session: Option<Session>) {
        if let Some(keeper) = &self.keeper {
            keeper.replace(session.clone());
        }
        self.session = session;
    }

    fn set(&mut self, state: AuthState, cx: &mut Context<Self>) {
        self.state = state;
        cx.notify();
    }

    fn signed_out(&mut self, notice: Option<&str>, cx: &mut Context<Self>) {
        self.session = None;
        self.set(
            AuthState::SignedOut {
                notice: notice.map(|notice| SharedString::from(notice.to_owned())),
            },
            cx,
        );
    }

    fn signed_in(&mut self, session: Session, online: bool, cx: &mut Context<Self>) {
        let state = if session.account.must_change_password {
            AuthState::MustChangePassword {
                account: session.account.clone(),
                notice: None,
                busy: false,
            }
        } else {
            AuthState::SignedIn {
                account: session.account.clone(),
                online,
            }
        };
        self.session = Some(session);
        self.set(state, cx);
        self.schedule_refresh(cx);
    }

    fn forget(&mut self, cx: &mut Context<Self>) {
        self.keep(None);
        if let Some(store) = self.store.clone() {
            cx.background_spawn(async move {
                if let Err(error) = store.clear() {
                    eprintln!("missal: could not remove the saved session: {error}");
                }
            })
            .detach();
        }
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        if self.keeper.is_none() {
            return;
        }
        let Some(store) = self.store.clone() else {
            self.signed_out(None, cx);
            return;
        };
        let load = cx.background_spawn(async move { store.load() });
        cx.spawn(async move |this, cx| {
            let loaded = load.await;
            this.update(cx, |this, cx| match loaded {
                Ok(Some(session)) if !session.account.must_change_password => {
                    let usable = session.usable_offline(SystemTime::now());
                    this.keep(Some(session.clone()));
                    if usable {
                        this.signed_in(session, false, cx);
                    }
                    this.refresh(cx);
                }
                Ok(Some(_)) => {
                    this.forget(cx);
                    this.signed_out(None, cx);
                }
                Ok(None) => this.signed_out(None, cx),
                Err(error) => {
                    eprintln!("missal: could not read the saved session: {error}");
                    this.signed_out(None, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn schedule_refresh(&mut self, cx: &mut Context<Self>) {
        if self.refresh_loop.is_some() {
            return;
        }
        self.refresh_loop = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REFRESH_EVERY).await;
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    pub fn check(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let (Some(keeper), true) = (self.keeper.clone(), self.session.is_some()) else {
            return;
        };
        let task = cx.background_spawn(async move { keeper.refresh() });
        cx.spawn(async move |this, cx| {
            let refreshed = task.await;
            this.update(cx, |this, cx| this.refreshed(refreshed, cx))
                .ok();
        })
        .detach();
    }

    fn refreshed(&mut self, outcome: Result<Session, AuthError>, cx: &mut Context<Self>) {
        if self.session.is_none() {
            return;
        }
        match outcome {
            Ok(session) => self.signed_in(session, true, cx),
            Err(error) if error.ends_session() => {
                self.forget(cx);
                self.signed_out(Some(ENDED), cx);
            }
            Err(error) => {
                eprintln!("missal: session refresh failed: {error}");
                let usable = self
                    .session
                    .as_ref()
                    .is_some_and(|session| session.usable_offline(SystemTime::now()));
                match (&self.state, usable) {
                    (_, false) => {
                        let session = self.session.take();
                        self.set(
                            AuthState::SignedOut {
                                notice: Some(STALE.into()),
                            },
                            cx,
                        );
                        self.session = session;
                    }
                    (AuthState::SignedIn { account, .. }, true) => {
                        let account = account.clone();
                        self.set(
                            AuthState::SignedIn {
                                account,
                                online: false,
                            },
                            cx,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn sign_in(&mut self, email: String, password: String, cx: &mut Context<Self>) {
        let Some(keeper) = self.keeper.clone() else {
            return;
        };
        if matches!(self.state, AuthState::SigningIn) {
            return;
        }
        self.set(AuthState::SigningIn, cx);
        let task = cx.background_spawn(async move { keeper.sign_in(&email, &password) });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| match outcome {
                Ok(session) => this.signed_in(session, true, cx),
                Err(error) => {
                    let notice = error.to_string();
                    this.signed_out(Some(&notice), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn change_password(
        &mut self,
        current: String,
        new: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), AuthError>> {
        let Some(keeper) = self.keeper.clone() else {
            return Task::ready(Err(AuthError::SessionEnded));
        };
        if let AuthState::MustChangePassword { account, .. } = &self.state {
            let account = account.clone();
            self.set(
                AuthState::MustChangePassword {
                    account,
                    notice: None,
                    busy: true,
                },
                cx,
            );
        }
        let task = cx.background_spawn(async move { keeper.change_password(&current, &new) });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| match outcome {
                Ok(session) => {
                    this.signed_in(session, true, cx);
                    Ok(())
                }
                Err(error) => {
                    if error.ends_session() {
                        this.forget(cx);
                        this.signed_out(Some(ENDED), cx);
                    } else if let AuthState::MustChangePassword { account, .. } = &this.state {
                        let account = account.clone();
                        this.set(
                            AuthState::MustChangePassword {
                                account,
                                notice: Some(error.to_string().into()),
                                busy: false,
                            },
                            cx,
                        );
                    }
                    Err(error)
                }
            })
            .unwrap_or(Err(AuthError::SessionEnded))
        })
    }

    pub fn end_session(&mut self, cx: &mut Context<Self>) {
        if self.session.is_none() {
            return;
        }
        self.forget(cx);
        self.signed_out(Some(ENDED), cx);
    }

    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        if let Some(keeper) = self.keeper.clone() {
            cx.background_spawn(async move { keeper.sign_out() })
                .detach();
        }
        self.session = None;
        self.signed_out(None, cx);
    }
}

pub fn sign_out(window: &mut Window, cx: &mut App) {
    if Models::get(cx).nav.read(cx).has_unsaved_changes(cx) {
        toast_warning(
            window,
            cx,
            "Save or discard your changes before signing out.",
        );
        return;
    }
    if let Some(auth) = AuthModel::get(cx) {
        auth.update(cx, |auth, cx| auth.sign_out(cx));
    }
}

pub struct AuthGate {
    auth: Entity<AuthModel>,
    email: Entity<TextInput>,
    password: Entity<TextInput>,
    new_password: Entity<TextInput>,
    confirm: Entity<TextInput>,
    last_password: Option<String>,
    problem: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl AuthGate {
    pub fn new(auth: Entity<AuthModel>, window: &mut Window, cx: &mut Context<Self>) -> AuthGate {
        let email = field_input(window, cx, Kind::Latin, "name@example.com", "");
        let password = secret_input(window, cx, "Password");
        let new_password = secret_input(window, cx, "New password");
        let confirm = secret_input(window, cx, "Repeat the new password");
        let submit = |input: &Entity<TextInput>, window: &mut Window, cx: &mut Context<Self>| {
            cx.subscribe_in(
                input,
                window,
                |this, _, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Submit => this.submit(window, cx),
                    TextInputEvent::Change => {
                        this.problem = None;
                        cx.notify();
                    }
                    _ => {}
                },
            )
        };
        let subscriptions = vec![
            submit(&email, window, cx),
            submit(&password, window, cx),
            submit(&new_password, window, cx),
            submit(&confirm, window, cx),
            cx.observe_in(&auth, window, |this, auth, window, cx| {
                match auth.read(cx).state() {
                    AuthState::SignedIn { .. } => {
                        this.last_password = None;
                        for input in [&this.password, &this.new_password, &this.confirm] {
                            input.update(cx, |input, cx| input.set_text("", cx));
                        }
                    }
                    AuthState::SignedOut { .. }
                    | AuthState::MustChangePassword { busy: false, .. } => this.focus(window, cx),
                    _ => {}
                }
                cx.notify();
            }),
        ];
        AuthGate {
            auth,
            email,
            password,
            new_password,
            confirm,
            last_password: None,
            problem: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        let state = self.auth.read(cx).state().clone();
        let input = match state {
            AuthState::MustChangePassword { .. } if self.last_password.is_some() => {
                &self.new_password
            }
            AuthState::MustChangePassword { .. } => &self.password,
            _ if self.email.read(cx).text().trim().is_empty() => &self.email,
            _ => &self.password,
        };
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    fn submit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        match self.auth.read(cx).state().clone() {
            AuthState::SignedOut { .. } => {
                let email = self.email.read(cx).text().trim().to_owned();
                let password = self.password.read(cx).text().to_owned();
                if email.is_empty() || password.is_empty() {
                    self.problem = Some("Enter your email and password.".into());
                    cx.notify();
                    return;
                }
                self.last_password = Some(password.clone());
                self.password.update(cx, |input, cx| input.set_text("", cx));
                self.auth
                    .update(cx, |auth, cx| auth.sign_in(email, password, cx));
            }
            AuthState::MustChangePassword { busy: false, .. } => {
                let current = match &self.last_password {
                    Some(password) => password.clone(),
                    None => self.password.read(cx).text().to_owned(),
                };
                let new = self.new_password.read(cx).text().to_owned();
                let confirm = self.confirm.read(cx).text().to_owned();
                if let Some(problem) = password_problem(&new, &confirm) {
                    self.problem = Some(problem.into());
                    cx.notify();
                    return;
                }
                if new == current {
                    self.problem =
                        Some("Choose a password different from the temporary one.".into());
                    cx.notify();
                    return;
                }
                self.auth
                    .update(cx, |auth, cx| auth.change_password(current, new, cx))
                    .detach();
            }
            _ => {}
        }
    }

    fn labelled(palette: &Palette, text: &'static str, element: impl IntoElement) -> AnyElement {
        v_flex()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_muted)
                    .child(text),
            )
            .child(element)
            .into_any_element()
    }
}

impl Render for AuthGate {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let state = self.auth.read(cx).state().clone();
        let width = px(340.);
        let (title, message, notice, busy, fields, action): (
            &str,
            String,
            Option<SharedString>,
            bool,
            Vec<AnyElement>,
            &str,
        ) = match &state {
            AuthState::Restoring => (
                "Missal",
                "Checking your session…".to_owned(),
                None,
                true,
                Vec::new(),
                "",
            ),
            AuthState::SignedOut { .. } | AuthState::SigningIn => {
                let notice = match &state {
                    AuthState::SignedOut { notice } => notice.clone(),
                    _ => None,
                };
                (
                    "Sign in to Missal",
                    "Use the email and password your administrator gave you.".to_owned(),
                    notice,
                    matches!(state, AuthState::SigningIn),
                    vec![
                        Self::labelled(
                            &palette,
                            "Email",
                            field("sign-in-email", &self.email, Kind::Latin).width(width),
                        ),
                        Self::labelled(
                            &palette,
                            "Password",
                            field("sign-in-password", &self.password, Kind::Latin).width(width),
                        ),
                    ],
                    "Sign in",
                )
            }
            AuthState::MustChangePassword {
                account,
                notice,
                busy,
            } => {
                let mut fields = Vec::new();
                if self.last_password.is_none() {
                    fields.push(Self::labelled(
                        &palette,
                        "Current password",
                        field("change-current", &self.password, Kind::Latin).width(width),
                    ));
                }
                fields.push(Self::labelled(
                    &palette,
                    "New password",
                    field("change-new", &self.new_password, Kind::Latin).width(width),
                ));
                fields.push(Self::labelled(
                    &palette,
                    "Repeat the new password",
                    field("change-confirm", &self.confirm, Kind::Latin).width(width),
                ));
                (
                    "Choose a new password",
                    format!(
                        "{} is signed in with a temporary password. Choose your own to continue; at least 10 characters.",
                        account.email
                    ),
                    notice.clone(),
                    *busy,
                    fields,
                    "Save password",
                )
            }
            AuthState::Disabled | AuthState::SignedIn { .. } => {
                return div().into_any_element();
            }
        };
        let problem = self.problem.clone().or(notice);
        let button_label = if busy && !action.is_empty() {
            "Please wait…"
        } else {
            action
        };
        v_flex()
            .id("sign-in-gate")
            .size_full()
            .items_center()
            .justify_center()
            .bg(palette.canvas)
            .child(
                v_flex()
                    .w(px(400.))
                    .p(px(32.))
                    .gap(px(14.))
                    .rounded(px(crate::theme::RADIUS_2XL))
                    .bg(palette.raised)
                    .shadow(palette.shadow_lg())
                    .child(
                        div()
                            .mb(px(6.))
                            .size(px(40.))
                            .rounded(px(RADIUS_XL))
                            .bg(palette.action)
                            .shadow(palette.primary_shadow())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(crate::ui::sized_icon(
                                gpui_kit::assets::IconName::FileText,
                                20.,
                                palette.on_action,
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(20.))
                            .line_height(px(28.))
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .text_color(palette.ink)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .text_color(palette.ink_muted)
                            .child(message),
                    )
                    .children(fields)
                    .children(problem.map(|problem| {
                        div()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .text_color(palette.danger_text)
                            .child(problem)
                    }))
                    .when(!action.is_empty(), |this| {
                        this.child(
                            div().mt(px(6.)).child(
                                crate::ui::button("auth-submit", button_label)
                                    .primary()
                                    .disabled(busy)
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.submit(window, cx)),
                                    ),
                            ),
                        )
                    }),
            )
            .into_any_element()
    }
}
