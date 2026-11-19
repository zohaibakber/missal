use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnimationExt, AnyElement, App, AppContext, Context, ElementId, Entity, FontWeight, Global,
    Hsla, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Task, Window, div, linear_color_stop,
    linear_gradient, px, relative,
};
use missal_auth::{
    Account, AuthError, CodePurpose, Config, LicenceStatus, Session, SessionKeeper, SessionStore,
};
use missal_text::{TextInput, TextInputEvent};

use crate::state::{Models, toast_warning};
use crate::theme::{
    BUTTON_XS, Palette, SHELL_INSET, hex, hex_alpha, inset, motion_entrance, outline, shadow,
};
use crate::ui::input::{Kind, field, field_input, secret_input};
use crate::ui::{ButtonKind, UrRole, button, sized_icon, ur_label};

const REFRESH_EVERY: Duration = Duration::from_secs(60 * 60);
pub const MIN_PASSWORD: usize = 10;
const CODE_LENGTH: usize = 6;

const BEZEL: f32 = 6.0;
const SHELL_RADIUS: f32 = 28.0;
const CORE_RADIUS: f32 = SHELL_RADIUS - BEZEL;
const FORM_WIDTH: f32 = 360.0;
const SPLIT_FROM: f32 = 980.0;
const RISE: f32 = 14.0;
const PAGE_WIDTH: f32 = 340.0;
const PAGE_HEIGHT: f32 = 380.0;
const PAGE_OFFSET: f32 = 16.0;
const FADE_BLEED: f32 = 96.0;

const ENDED: &str = "Your session has ended. Sign in again to continue.";
const STALE: &str =
    "Missal has been offline for more than 30 days. Connect to the internet and sign in again.";
const NOT_YET: &str =
    "There is no licence on this account yet. If you have just paid, try again in a little while.";

#[derive(Clone, PartialEq, Eq)]
pub enum Flow {
    SignIn,
    SignUp,
    VerifyEmail { email: String },
    ForgotPassword,
    ResetPassword { email: String },
}

#[derive(Clone)]
pub enum Notice {
    Problem(SharedString),
    Info(SharedString),
}

impl Notice {
    fn problem(text: impl ToString) -> Option<Notice> {
        Some(Notice::Problem(text.to_string().into()))
    }

    fn info(text: impl ToString) -> Option<Notice> {
        Some(Notice::Info(text.to_string().into()))
    }
}

#[derive(Clone)]
pub enum AuthState {
    Disabled,
    Restoring,
    SignedOut {
        flow: Flow,
        notice: Option<Notice>,
        busy: bool,
    },
    SignedIn {
        account: Account,
        online: bool,
    },
    LicenceExpired {
        account: Account,
        notice: Option<Notice>,
        busy: bool,
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
        Some("The two passwords do not match.")
    } else {
        None
    }
}

fn code_digits(text: &str) -> Option<String> {
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    let clean = text
        .chars()
        .all(|c| c.is_ascii_digit() || c.is_whitespace());
    (clean && digits.len() == CODE_LENGTH).then_some(digits)
}

impl AuthModel {
    pub fn install(data_directory: Option<&Path>, cx: &mut App) -> Entity<AuthModel> {
        let config = Config::resolve();
        if config.is_none() {
            eprintln!("missal: sign-in is off because API_URL is not set");
        }
        Self::install_with(config, data_directory, cx)
    }

    pub fn install_with(
        config: Option<Config>,
        data_directory: Option<&Path>,
        cx: &mut App,
    ) -> Entity<AuthModel> {
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

    pub fn purchase_url(&self) -> Option<&str> {
        self.config.as_ref()?.purchase_url.as_deref()
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

    fn signed_out(&mut self, flow: Flow, notice: Option<Notice>, cx: &mut Context<Self>) {
        self.session = None;
        self.set(
            AuthState::SignedOut {
                flow,
                notice,
                busy: false,
            },
            cx,
        );
    }

    fn signed_in(&mut self, session: Session, online: bool, cx: &mut Context<Self>) {
        let account = session.account.clone();
        let state = if account.licence.active(SystemTime::now()) {
            AuthState::SignedIn { account, online }
        } else {
            AuthState::LicenceExpired {
                account,
                notice: None,
                busy: false,
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

    fn ended(&mut self, cx: &mut Context<Self>) {
        self.forget(cx);
        self.signed_out(Flow::SignIn, Notice::problem(ENDED), cx);
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        if self.keeper.is_none() {
            return;
        }
        let Some(store) = self.store.clone() else {
            self.signed_out(Flow::SignIn, None, cx);
            return;
        };
        let load = cx.background_spawn(async move { store.load() });
        cx.spawn(async move |this, cx| {
            let loaded = load.await;
            this.update(cx, |this, cx| match loaded {
                Ok(Some(session)) => {
                    let now = SystemTime::now();
                    let settled =
                        session.usable_offline(now) || !session.account.licence.active(now);
                    this.keep(Some(session.clone()));
                    if settled {
                        this.signed_in(session, false, cx);
                    }
                    this.refresh(cx);
                }
                Ok(None) => this.signed_out(Flow::SignIn, None, cx),
                Err(error) => {
                    eprintln!("missal: could not read the saved session: {error}");
                    this.signed_out(Flow::SignIn, None, cx);
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
        let Some(session) = self.session.clone() else {
            return;
        };
        match outcome {
            Ok(session) => self.signed_in(session, true, cx),
            Err(error) if error.ends_session() => self.ended(cx),
            Err(error) => {
                eprintln!("missal: session refresh failed: {error}");
                let now = SystemTime::now();
                if session.usable_offline(now) {
                    if let AuthState::SignedIn { account, .. } = &self.state {
                        let account = account.clone();
                        self.set(
                            AuthState::SignedIn {
                                account,
                                online: false,
                            },
                            cx,
                        );
                    }
                } else if !session.account.licence.active(now) {
                    if !matches!(self.state, AuthState::LicenceExpired { .. }) {
                        self.set(
                            AuthState::LicenceExpired {
                                account: session.account,
                                notice: None,
                                busy: false,
                            },
                            cx,
                        );
                    }
                } else if !matches!(self.state, AuthState::SignedOut { .. }) {
                    self.set(
                        AuthState::SignedOut {
                            flow: Flow::SignIn,
                            notice: Notice::problem(STALE),
                            busy: false,
                        },
                        cx,
                    );
                }
            }
        }
    }

    pub fn show(&mut self, flow: Flow, cx: &mut Context<Self>) {
        if let AuthState::SignedOut { busy: false, .. } = self.state {
            self.set(
                AuthState::SignedOut {
                    flow,
                    notice: None,
                    busy: false,
                },
                cx,
            );
        }
    }

    fn attempt<T: Send + 'static>(
        &mut self,
        job: impl FnOnce(&SessionKeeper) -> Result<T, AuthError> + Send + 'static,
        done: impl FnOnce(&mut Self, Flow, Result<T, AuthError>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let (
            Some(keeper),
            AuthState::SignedOut {
                flow, busy: false, ..
            },
        ) = (self.keeper.clone(), &self.state)
        else {
            return;
        };
        let flow = flow.clone();
        self.set(
            AuthState::SignedOut {
                flow: flow.clone(),
                notice: None,
                busy: true,
            },
            cx,
        );
        let task = cx.background_spawn(async move { job(&keeper) });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| done(this, flow, outcome, cx))
                .ok();
        })
        .detach();
    }

    fn opened(&mut self, flow: Flow, outcome: Result<Session, AuthError>, cx: &mut Context<Self>) {
        match outcome {
            Ok(session) => self.signed_in(session, true, cx),
            Err(error) => self.signed_out(flow, Notice::problem(error), cx),
        }
    }

    pub fn sign_in(&mut self, email: String, password: String, cx: &mut Context<Self>) {
        let address = email.clone();
        self.attempt(
            move |keeper| keeper.sign_in(&email, &password),
            move |this, flow, outcome, cx| match outcome {
                Err(AuthError::EmailNotVerified) => {
                    let notice = Notice::info("Confirm your email first. We sent you a new code.");
                    this.signed_out(Flow::VerifyEmail { email: address }, notice, cx);
                }
                outcome => this.opened(flow, outcome, cx),
            },
            cx,
        );
    }

    pub fn sign_up(
        &mut self,
        email: String,
        name: String,
        password: String,
        cx: &mut Context<Self>,
    ) {
        let address = email.clone();
        // The service creates the account at once unless it is set to confirm emails first;
        // then signing in reports EmailNotVerified and the code screen takes over.
        self.attempt(
            move |keeper| {
                keeper.sign_up(&email, &name, &password)?;
                keeper.sign_in(&email, &password)
            },
            move |this, flow, outcome, cx| match outcome {
                Err(AuthError::EmailNotVerified) => {
                    this.signed_out(Flow::VerifyEmail { email: address }, None, cx)
                }
                outcome => this.opened(flow, outcome, cx),
            },
            cx,
        );
    }

    pub fn send_code(&mut self, email: String, purpose: CodePurpose, cx: &mut Context<Self>) {
        let address = email.clone();
        self.attempt(
            move |keeper| keeper.request_code(&email, purpose),
            move |this, flow, outcome, cx| match outcome {
                Ok(()) => {
                    let target = match purpose {
                        CodePurpose::Verify => Flow::VerifyEmail {
                            email: address.clone(),
                        },
                        CodePurpose::Reset => Flow::ResetPassword {
                            email: address.clone(),
                        },
                    };
                    let notice = if target == flow {
                        Notice::info(format!("We sent a new code to {address}."))
                    } else {
                        None
                    };
                    this.signed_out(target, notice, cx);
                }
                Err(error) => this.signed_out(flow, Notice::problem(error), cx),
            },
            cx,
        );
    }

    pub fn verify_email(&mut self, email: String, code: String, cx: &mut Context<Self>) {
        self.attempt(
            move |keeper| keeper.verify_email(&email, &code),
            Self::opened,
            cx,
        );
    }

    pub fn reset_password(
        &mut self,
        email: String,
        code: String,
        password: String,
        cx: &mut Context<Self>,
    ) {
        self.attempt(
            move |keeper| keeper.reset_password(&email, &code, &password),
            Self::opened,
            cx,
        );
    }

    pub fn check_licence(&mut self, cx: &mut Context<Self>) {
        let (
            Some(keeper),
            AuthState::LicenceExpired {
                account,
                busy: false,
                ..
            },
        ) = (self.keeper.clone(), &self.state)
        else {
            return;
        };
        let account = account.clone();
        self.set(
            AuthState::LicenceExpired {
                account,
                notice: None,
                busy: true,
            },
            cx,
        );
        let task = cx.background_spawn(async move { keeper.refresh() });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| match outcome {
                Ok(session) => {
                    this.signed_in(session, true, cx);
                    if let AuthState::LicenceExpired { notice, .. } = &mut this.state {
                        *notice = Notice::info(NOT_YET);
                    }
                }
                Err(error) if error.ends_session() => this.ended(cx),
                Err(error) => {
                    if let AuthState::LicenceExpired { notice, busy, .. } = &mut this.state {
                        *notice = Notice::problem(error);
                        *busy = false;
                        cx.notify();
                    }
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
                        this.ended(cx);
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
        self.ended(cx);
    }

    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        if let Some(keeper) = self.keeper.clone() {
            cx.background_spawn(async move { keeper.sign_out() })
                .detach();
        }
        self.signed_out(Flow::SignIn, None, cx);
    }
}

pub fn trial_days_left(account: &Account) -> Option<i64> {
    (account.licence.status == LicenceStatus::Trial)
        .then(|| account.licence.days_left(SystemTime::now()))
}

pub fn trial_label(days: i64) -> String {
    if days == 1 {
        "Trial: 1 day left".to_owned()
    } else {
        format!("Trial: {days} days left")
    }
}

pub fn open_purchase(cx: &mut App) {
    let url = AuthModel::get(cx).and_then(|auth| auth.read(cx).purchase_url().map(str::to_owned));
    if let Some(url) = url {
        cx.open_url(&url);
    }
}

fn expired_title(account: &Account) -> String {
    let paid = account.licence.status == LicenceStatus::Expired;
    match crate::ui::library::day_month_year(account.licence.until_ms) {
        Some(date) if paid => format!("Your licence ended on {date}"),
        _ => "Your free week has ended".to_owned(),
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
    name: Entity<TextInput>,
    password: Entity<TextInput>,
    new_password: Entity<TextInput>,
    confirm: Entity<TextInput>,
    code: Entity<TextInput>,
    problem: Option<SharedString>,
    shown: Option<Flow>,
    _subscriptions: Vec<Subscription>,
}

struct Screen {
    key: &'static str,
    title: String,
    message: String,
    notice: Option<Notice>,
    busy: bool,
    fields: Vec<AnyElement>,
    action: &'static str,
    links: Vec<AnyElement>,
}

impl AuthGate {
    pub fn new(auth: Entity<AuthModel>, window: &mut Window, cx: &mut Context<Self>) -> AuthGate {
        let email = field_input(window, cx, Kind::Latin, "name@example.com", "");
        let name = field_input(window, cx, Kind::Latin, "Your name", "");
        let password = secret_input(window, cx, "Password");
        let new_password = secret_input(window, cx, "At least 10 characters");
        let confirm = secret_input(window, cx, "Repeat the password");
        let code = field_input(window, cx, Kind::Latin, "6-digit code", "");
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
            submit(&name, window, cx),
            submit(&password, window, cx),
            submit(&new_password, window, cx),
            submit(&confirm, window, cx),
            submit(&code, window, cx),
            cx.observe_in(&auth, window, |this, auth, window, cx| {
                match auth.read(cx).state().clone() {
                    AuthState::SignedIn { .. } | AuthState::LicenceExpired { .. } => {
                        this.shown = None;
                        for input in [
                            &this.password,
                            &this.new_password,
                            &this.confirm,
                            &this.code,
                        ] {
                            input.update(cx, |input, cx| input.set_text("", cx));
                        }
                    }
                    AuthState::SignedOut {
                        flow, busy: false, ..
                    } => {
                        if this.shown.as_ref() != Some(&flow) {
                            this.problem = None;
                            this.code.update(cx, |input, cx| input.set_text("", cx));
                            this.shown = Some(flow);
                        }
                        this.focus(window, cx);
                    }
                    _ => {}
                }
                cx.notify();
            }),
        ];
        AuthGate {
            auth,
            email,
            name,
            password,
            new_password,
            confirm,
            code,
            problem: None,
            shown: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        let AuthState::SignedOut { flow, .. } = self.auth.read(cx).state().clone() else {
            return;
        };
        let empty = |input: &Entity<TextInput>| input.read(cx).text().trim().is_empty();
        let input = match flow {
            Flow::SignIn if empty(&self.email) => &self.email,
            Flow::SignIn => &self.password,
            Flow::SignUp if empty(&self.name) => &self.name,
            Flow::SignUp if empty(&self.email) => &self.email,
            Flow::SignUp => &self.new_password,
            Flow::ForgotPassword => &self.email,
            Flow::VerifyEmail { .. } | Flow::ResetPassword { .. } => &self.code,
        }
        .clone();
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    fn refuse(&mut self, problem: &str, cx: &mut Context<Self>) {
        self.problem = Some(problem.to_owned().into());
        cx.notify();
    }

    fn text(&self, input: &Entity<TextInput>, cx: &App) -> String {
        input.read(cx).text().to_owned()
    }

    fn email(&self, cx: &App) -> Option<String> {
        let email = self.email.read(cx).text().trim().to_owned();
        email.contains('@').then_some(email)
    }

    fn new_password(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let password = self.text(&self.new_password, cx);
        let confirm = self.text(&self.confirm, cx);
        match password_problem(&password, &confirm) {
            Some(problem) => {
                self.refuse(problem, cx);
                None
            }
            None => Some(password),
        }
    }

    fn code(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let code = code_digits(self.code.read(cx).text());
        if code.is_none() {
            self.refuse("Enter the 6-digit code from the email.", cx);
        }
        code
    }

    fn submit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let flow = match self.auth.read(cx).state() {
            AuthState::SignedOut {
                flow, busy: false, ..
            } => flow.clone(),
            AuthState::LicenceExpired { busy: false, .. } => {
                self.auth.update(cx, |auth, cx| auth.check_licence(cx));
                return;
            }
            _ => return,
        };
        match flow {
            Flow::SignIn => {
                let email = self.email.read(cx).text().trim().to_owned();
                let password = self.text(&self.password, cx);
                if email.is_empty() || password.is_empty() {
                    return self.refuse("Enter your email and password.", cx);
                }
                self.password.update(cx, |input, cx| input.set_text("", cx));
                self.auth
                    .update(cx, |auth, cx| auth.sign_in(email, password, cx));
            }
            Flow::SignUp => {
                let name = self.name.read(cx).text().trim().to_owned();
                if name.is_empty() {
                    return self.refuse("Enter your name.", cx);
                }
                let Some(email) = self.email(cx) else {
                    return self.refuse("Enter your email address.", cx);
                };
                let Some(password) = self.new_password(cx) else {
                    return;
                };
                self.auth
                    .update(cx, |auth, cx| auth.sign_up(email, name, password, cx));
            }
            Flow::VerifyEmail { email } => {
                let Some(code) = self.code(cx) else {
                    return;
                };
                self.auth
                    .update(cx, |auth, cx| auth.verify_email(email, code, cx));
            }
            Flow::ForgotPassword => {
                let Some(email) = self.email(cx) else {
                    return self.refuse("Enter your email address.", cx);
                };
                self.auth
                    .update(cx, |auth, cx| auth.send_code(email, CodePurpose::Reset, cx));
            }
            Flow::ResetPassword { email } => {
                let Some(code) = self.code(cx) else {
                    return;
                };
                let Some(password) = self.new_password(cx) else {
                    return;
                };
                self.auth.update(cx, |auth, cx| {
                    auth.reset_password(email, code, password, cx)
                });
            }
        }
    }

    fn labelled(
        palette: &Palette,
        text: &'static str,
        trailing: Option<AnyElement>,
        element: impl IntoElement,
    ) -> AnyElement {
        v_flex()
            .gap(px(6.))
            .child(
                h_flex()
                    .h(px(BUTTON_XS))
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.ink)
                            .child(text),
                    )
                    .children(trailing.map(|trailing| div().mr(px(-7.)).child(trailing))),
            )
            .child(element)
            .into_any_element()
    }

    fn link(
        id: &'static str,
        text: impl Into<SharedString>,
        busy: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        button(id, text)
            .kind(ButtonKind::GhostMuted)
            .sm()
            .disabled(busy)
            .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
            .into_any_element()
    }

    fn go(flow: Flow) -> impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static {
        move |this, _, cx| {
            let flow = flow.clone();
            this.auth.update(cx, |auth, cx| auth.show(flow, cx));
        }
    }

    fn resend(
        email: String,
        purpose: CodePurpose,
    ) -> impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static {
        move |this, _, cx| {
            let email = email.clone();
            this.auth
                .update(cx, |auth, cx| auth.send_code(email, purpose, cx));
        }
    }

    fn signed_out_screen(
        &self,
        flow: &Flow,
        notice: Option<Notice>,
        busy: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> Screen {
        let input = |label: &'static str, id: &'static str, input: &Entity<TextInput>| {
            Self::labelled(palette, label, None, field(id, input, Kind::Latin))
        };
        let new_password = |label: &'static str| {
            [
                input(label, "auth-new-password", &self.new_password),
                input("Repeat the password", "auth-confirm", &self.confirm),
            ]
        };
        let back = |cx: &mut Context<Self>| {
            Self::link(
                "auth-back",
                "Back to sign in",
                busy,
                cx,
                Self::go(Flow::SignIn),
            )
        };
        let (key, title, message, fields, action, links): (
            &'static str,
            &str,
            String,
            Vec<AnyElement>,
            &str,
            Vec<AnyElement>,
        ) = match flow {
            Flow::SignIn => (
                "sign-in",
                "Sign in to Missal",
                "Use the email and password of your Missal account.".to_owned(),
                vec![
                    input("Email", "sign-in-email", &self.email),
                    Self::labelled(
                        palette,
                        "Password",
                        Some(
                            button("auth-forgot", "Forgot password?")
                                .kind(ButtonKind::GhostMuted)
                                .xs()
                                .tab_index(1)
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    Self::go(Flow::ForgotPassword)(this, window, cx)
                                }))
                                .into_any_element(),
                        ),
                        field("sign-in-password", &self.password, Kind::Latin),
                    ),
                ],
                "Sign in",
                vec![Self::link(
                    "auth-sign-up",
                    "Create an account",
                    busy,
                    cx,
                    Self::go(Flow::SignUp),
                )],
            ),
            Flow::SignUp => {
                let mut fields = vec![
                    input("Name", "sign-up-name", &self.name),
                    input("Email", "sign-up-email", &self.email),
                ];
                fields.extend(new_password("Password"));
                (
                    "sign-up",
                    "Create your Missal account",
                    "Your first week is free. We will email you a code to confirm the address."
                        .to_owned(),
                    fields,
                    "Create account",
                    vec![Self::link(
                        "auth-have-account",
                        "I already have an account",
                        busy,
                        cx,
                        Self::go(Flow::SignIn),
                    )],
                )
            }
            Flow::VerifyEmail { email } => (
                "verify",
                "Check your email",
                format!("Enter the 6-digit code we sent to {email}. It expires in 10 minutes."),
                vec![input("Code", "verify-code", &self.code)],
                "Confirm email",
                vec![
                    Self::link(
                        "auth-resend",
                        "Send a new code",
                        busy,
                        cx,
                        Self::resend(email.clone(), CodePurpose::Verify),
                    ),
                    Self::link(
                        "auth-other-email",
                        "Use a different email",
                        busy,
                        cx,
                        Self::go(Flow::SignUp),
                    ),
                ],
            ),
            Flow::ForgotPassword => (
                "forgot",
                "Reset your password",
                "Enter the email of your account and we will send you a code.".to_owned(),
                vec![input("Email", "forgot-email", &self.email)],
                "Send code",
                vec![back(cx)],
            ),
            Flow::ResetPassword { email } => {
                let mut fields = vec![input("Code", "reset-code", &self.code)];
                fields.extend(new_password("New password"));
                (
                    "reset",
                    "Choose a new password",
                    format!(
                        "Enter the code we sent to {email} and a new password. Other computers signed in to this account are signed out."
                    ),
                    fields,
                    "Save password",
                    vec![
                        Self::link(
                            "auth-resend",
                            "Send a new code",
                            busy,
                            cx,
                            Self::resend(email.clone(), CodePurpose::Reset),
                        ),
                        back(cx),
                    ],
                )
            }
        };
        Screen {
            key,
            title: title.to_owned(),
            message,
            notice,
            busy,
            fields,
            action,
            links,
        }
    }

    fn expired_screen(
        &self,
        account: &Account,
        notice: Option<Notice>,
        busy: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> Screen {
        let purchase = self.auth.read(cx).purchase_url().is_some();
        let mut message = "Buy a one-year licence to keep using Missal. Your documents are safe on this computer.".to_owned();
        if !purchase {
            message.push_str(" Contact us to buy a licence.");
        }
        let mut links = Vec::new();
        if purchase {
            links.push(
                button("licence-buy", "Buy a licence")
                    .kind(ButtonKind::Ghost)
                    .sm()
                    .icon(IconName::ExternalLink)
                    .on_click(|_, _, cx| open_purchase(cx))
                    .into_any_element(),
            );
        }
        links.push(Self::link(
            "licence-sign-out",
            "Sign out",
            busy,
            cx,
            |_, window, cx| sign_out(window, cx),
        ));
        Screen {
            key: "licence",
            title: expired_title(account),
            message,
            notice,
            busy,
            fields: vec![account_chip(palette, &account.email)],
            action: "I've bought a licence, check again",
            links,
        }
    }
}

impl Render for AuthGate {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let state = self.auth.read(cx).state().clone();
        let screen = match &state {
            AuthState::Restoring => Screen {
                key: "restoring",
                title: "Missal".to_owned(),
                message: "Checking your session…".to_owned(),
                notice: None,
                busy: true,
                fields: Vec::new(),
                action: "",
                links: Vec::new(),
            },
            AuthState::SignedOut { flow, notice, busy } => {
                self.signed_out_screen(flow, notice.clone(), *busy, &palette, cx)
            }
            AuthState::LicenceExpired {
                account,
                notice,
                busy,
            } => self.expired_screen(account, notice.clone(), *busy, &palette, cx),
            AuthState::Disabled | AuthState::SignedIn { .. } => {
                return div().into_any_element();
            }
        };
        let still = cx.reduce_motion();
        let split = window.viewport_size().width >= px(SPLIT_FROM);
        let notice = self.problem.clone().map(Notice::Problem).or(screen.notice);
        let key = screen.key;
        let busy = screen.busy;
        let action = screen.action;
        let label = if busy && !action.is_empty() {
            "Please wait…"
        } else {
            action
        };
        let step = |index: u32| ElementId::Name(format!("auth-{key}-{index}").into());
        let header = v_flex()
            .items_center()
            .gap(px(8.))
            .text_center()
            .child(
                div()
                    .text_size(px(24.))
                    .line_height(px(30.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.ink)
                    .child(screen.title),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .line_height(px(21.))
                    .text_color(palette.ink_muted)
                    .child(screen.message),
            );
        let mut parts = vec![rise(header, step(0), 0, still)];
        if !screen.fields.is_empty() {
            parts.push(rise(
                v_flex().gap(px(14.)).children(screen.fields),
                step(1),
                1,
                still,
            ));
        }
        if let Some(notice) = notice {
            parts.push(rise(callout(&palette, notice), step(2), 0, still));
        }
        if !action.is_empty() {
            parts.push(rise(
                button("auth-submit", label)
                    .primary()
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                step(3),
                2,
                still,
            ));
        }
        if !screen.links.is_empty() {
            parts.push(rise(
                h_flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(4.))
                    .children(screen.links),
                step(4),
                3,
                still,
            ));
        }
        let form = v_flex().w(px(FORM_WIDTH)).gap(px(24.)).children(parts);
        h_flex()
            .id("sign-in-gate")
            .size_full()
            .p(px(SHELL_INSET))
            .gap(px(SHELL_INSET))
            .bg(palette.canvas)
            .when(split, |this| this.child(case_file(&palette, still)))
            .child(
                div()
                    .id("auth-form-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .min_h_full()
                            .items_center()
                            .justify_center()
                            .p(px(16.))
                            .child(form),
                    ),
            )
            .into_any_element()
    }
}

fn rise(element: impl IntoElement + 'static, id: ElementId, step: u32, still: bool) -> AnyElement {
    let holder = div().w_full().relative().child(element);
    if still {
        return holder.into_any_element();
    }
    holder
        .with_animation(id, motion_entrance(step), |this, delta| {
            this.opacity(delta).top(px(RISE * (1. - delta)))
        })
        .into_any_element()
}

fn callout(palette: &Palette, notice: Notice) -> impl IntoElement + use<> {
    let (icon, fill, edge, tint, ink, text) = match notice {
        Notice::Problem(text) => (
            IconName::CircleAlert,
            palette.danger_wash,
            palette.danger.opacity(0.2),
            palette.danger_text,
            palette.danger_text,
            text,
        ),
        Notice::Info(text) => (
            IconName::Info,
            palette.accent_wash,
            palette.accent.opacity(0.2),
            palette.accent,
            palette.ink,
            text,
        ),
    };
    h_flex()
        .items_start()
        .gap(px(10.))
        .p(px(12.))
        .rounded(px(14.))
        .bg(fill)
        .border_1()
        .border_color(edge)
        .child(
            div()
                .flex_none()
                .mt(px(1.))
                .child(sized_icon(icon, 16., tint)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(ink)
                .child(text),
        )
}

fn account_chip(palette: &Palette, email: &str) -> AnyElement {
    h_flex()
        .gap(px(10.))
        .p(px(6.))
        .pr(px(16.))
        .rounded(px(16.))
        .bg(palette.sunken)
        .shadow(palette.ring_only())
        .child(
            div()
                .flex_none()
                .size(px(32.))
                .rounded_full()
                .bg(palette.raised)
                .shadow(palette.shadow_xs())
                .flex()
                .items_center()
                .justify_center()
                .child(sized_icon(IconName::CircleUser, 16., palette.ink_muted)),
        )
        .child(
            v_flex()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(palette.ink_muted)
                        .child("Signed in as"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .text_color(palette.ink)
                        .truncate()
                        .child(email.to_owned()),
                ),
        )
        .into_any_element()
}

struct Slab {
    fill: Hsla,
    ink: Hsla,
    muted: Hsla,
    wash: Hsla,
}

impl Slab {
    fn new(palette: &Palette) -> Slab {
        Slab {
            fill: hex(if palette.dark { 0x19191b } else { 0x18181b }),
            ink: hex(0xf5f5f5),
            muted: hex_alpha(0xffffff, 0.56),
            wash: hex_alpha(0xffffff, 0.06),
        }
    }
}

fn case_file(palette: &Palette, still: bool) -> impl IntoElement + use<> {
    let slab = Slab::new(palette);
    div()
        .flex_none()
        .w(relative(0.46))
        .h_full()
        .p(px(BEZEL))
        .rounded(px(SHELL_RADIUS))
        .bg(palette.hover)
        .shadow(palette.ring_only())
        .child(
            v_flex()
                .size_full()
                .p(px(40.))
                .gap(px(32.))
                .rounded(px(CORE_RADIUS))
                .bg(slab.fill)
                .shadow(vec![inset(slab.wash, 1., 0.)])
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            div()
                                .size(px(24.))
                                .rounded(px(6.))
                                .bg(slab.ink)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(crate::ui::sized_icon(
                                    gpui_kit::assets::IconName::Feather,
                                    14.,
                                    slab.fill,
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(15.))
                                .line_height(px(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(slab.ink)
                                .child("Missal"),
                        ),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .items_center()
                        .justify_center()
                        .child(form_pages(palette, &slab, still)),
                )
                .child(
                    v_flex()
                        .gap(px(12.))
                        .child(
                            div()
                                .max_w(px(420.))
                                .text_size(px(34.))
                                .line_height(px(40.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(slab.ink)
                                .child("First Information Reports, written in Urdu."),
                        )
                        .child(
                            div()
                                .max_w(px(380.))
                                .text_size(px(14.))
                                .line_height(px(21.))
                                .text_color(slab.muted)
                                .child(
                                    "Templates, fields and print-ready pages, kept together on this computer.",
                                ),
                        ),
                ),
        )
}

fn form_row(palette: &Palette, label: &'static str) -> gpui_kit::Div {
    h_flex()
        .flex_1()
        .items_end()
        .gap(px(10.))
        .child(
            div()
                .flex_1()
                .h(px(1.))
                .mb(px(7.))
                .bg(palette.ink.opacity(0.16)),
        )
        .child(
            div()
                .flex_none()
                .text_color(palette.ink_muted)
                .child(ur_label(label, UrRole::Label)),
        )
}

fn form_page(palette: &Palette) -> gpui_kit::Div {
    let ink = palette.ink;
    let status = palette.st_investigating;
    v_flex()
        .size_full()
        .px(px(28.))
        .pt(px(24.))
        .gap(px(14.))
        .child(
            h_flex().child(
                div()
                    .px(px(10.))
                    .rounded_full()
                    .bg(status.opacity(0.12))
                    .text_color(status)
                    .child(ur_label("زیر تفتیش", UrRole::Label)),
            ),
        )
        .child(
            v_flex()
                .items_center()
                .text_color(ink)
                .child(ur_label("ابتدائی اطلاعی رپورٹ", UrRole::Display))
                .child(
                    div()
                        .text_color(palette.ink_muted)
                        .child(ur_label("زیر دفعہ 154 ضابطہ فوجداری", UrRole::Label)),
                ),
        )
        .child(div().h(px(1.)).bg(ink.opacity(0.1)))
        .child(
            h_flex()
                .gap(px(20.))
                .child(form_row(palette, "تاریخ ایف آئی آر"))
                .child(form_row(palette, "ایف آئی آر نمبر")),
        )
        .children(
            [
                "تھانہ نام",
                "جرم",
                "تاریخ ووقت وقوعہ",
                "تفتیشی افسر",
                "تحریر کنندہ",
            ]
            .map(|label| h_flex().child(form_row(palette, label))),
        )
}

fn form_pages(palette: &Palette, slab: &Slab, still: bool) -> impl IntoElement + use<> {
    let page_shadow = vec![
        outline(palette.ink.opacity(if palette.dark { 0.08 } else { 0.0 })),
        shadow(hex_alpha(0x000000, 0.45), 30., 60., -18.),
        shadow(hex_alpha(0x000000, 0.2), 2., 6., -2.),
    ];
    let page = |index: u32, content: Option<gpui_kit::Div>| {
        let depth = 2 - index;
        let left = PAGE_OFFSET * depth as f32;
        let top = PAGE_OFFSET * index as f32;
        let fill = palette
            .paper
            .blend(slab.fill.opacity([0.62, 0.34, 0.0][index as usize]));
        let sheet = div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(PAGE_WIDTH))
            .h(px(PAGE_HEIGHT))
            .rounded(px(4.))
            .overflow_hidden()
            .bg(fill)
            .shadow(page_shadow.clone())
            .children(content);
        if still {
            return sheet.into_any_element();
        }
        sheet
            .with_animation(
                ElementId::Name(format!("auth-page-{index}").into()),
                motion_entrance(index * 2),
                move |this, delta| this.opacity(delta).top(px(top + RISE * 2. * (1. - delta))),
            )
            .into_any_element()
    };
    let width = PAGE_WIDTH + PAGE_OFFSET * 2.;
    let height = PAGE_HEIGHT + PAGE_OFFSET * 2.;
    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .child(page(0, None))
        .child(page(1, None))
        .child(page(2, Some(form_page(palette))))
        .child(
            div()
                .absolute()
                .left(px(-FADE_BLEED))
                .bottom(px(-FADE_BLEED))
                .w(px(width + FADE_BLEED * 2.))
                .h(px(height * 0.42 + FADE_BLEED))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(slab.fill.opacity(0.), 0.),
                    linear_color_stop(slab.fill, 0.55),
                )),
        )
}
