use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, Global, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Task, Window, div, px,
};
use missal_auth::{
    Account, AuthError, CodePurpose, Config, LicenceStatus, Session, SessionKeeper, SessionStore,
};
use missal_text::{TextInput, TextInputEvent};

use crate::state::{Models, toast_warning};
use crate::theme::{Palette, RADIUS_XL};
use crate::ui::input::{Kind, field, field_input, secret_input};
use crate::ui::{ButtonKind, button};

const REFRESH_EVERY: Duration = Duration::from_secs(60 * 60);
pub const MIN_PASSWORD: usize = 10;
const CODE_LENGTH: usize = 6;

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
            eprintln!("missal: sign-in is off because MISSAL_API_URL is not set");
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
        self.attempt(
            move |keeper| keeper.sign_up(&email, &name, &password),
            move |this, flow, outcome, cx| match outcome {
                Ok(()) => this.signed_out(Flow::VerifyEmail { email: address }, None, cx),
                Err(error) => this.signed_out(flow, Notice::problem(error), cx),
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
        let width = px(340.);
        let input = |label: &'static str, id: &'static str, input: &Entity<TextInput>| {
            Self::labelled(palette, label, field(id, input, Kind::Latin).width(width))
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
        let (title, message, fields, action, links): (
            &str,
            String,
            Vec<AnyElement>,
            &str,
            Vec<AnyElement>,
        ) = match flow {
            Flow::SignIn => (
                "Sign in to Missal",
                "Use the email and password of your Missal account.".to_owned(),
                vec![
                    input("Email", "sign-in-email", &self.email),
                    input("Password", "sign-in-password", &self.password),
                ],
                "Sign in",
                vec![
                    Self::link(
                        "auth-sign-up",
                        "Create an account",
                        busy,
                        cx,
                        Self::go(Flow::SignUp),
                    ),
                    Self::link(
                        "auth-forgot",
                        "Forgot password?",
                        busy,
                        cx,
                        Self::go(Flow::ForgotPassword),
                    ),
                ],
            ),
            Flow::SignUp => {
                let mut fields = vec![
                    input("Name", "sign-up-name", &self.name),
                    input("Email", "sign-up-email", &self.email),
                ];
                fields.extend(new_password("Password"));
                (
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
                    .icon(gpui_kit::assets::IconName::ExternalLink)
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
            title: expired_title(account),
            message,
            notice,
            busy,
            fields: vec![
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_muted)
                    .child(format!("Signed in as {}", account.email))
                    .into_any_element(),
            ],
            action: "I've bought a licence, check again",
            links,
        }
    }
}

impl Render for AuthGate {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let state = self.auth.read(cx).state().clone();
        let screen = match &state {
            AuthState::Restoring => Screen {
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
        let notice = self.problem.clone().map(Notice::Problem).or(screen.notice);
        let busy = screen.busy;
        let action = screen.action;
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
                            .child(screen.title),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .text_color(palette.ink_muted)
                            .child(screen.message),
                    )
                    .children(screen.fields)
                    .children(notice.map(|notice| {
                        let (color, text) = match notice {
                            Notice::Problem(text) => (palette.danger_text, text),
                            Notice::Info(text) => (palette.ink, text),
                        };
                        div()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .text_color(color)
                            .child(text)
                    }))
                    .when(!action.is_empty(), |this| {
                        this.child(
                            div().mt(px(6.)).child(
                                button("auth-submit", button_label)
                                    .primary()
                                    .disabled(busy)
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.submit(window, cx)),
                                    ),
                            ),
                        )
                    })
                    .when(!screen.links.is_empty(), |this| {
                        this.child(
                            h_flex()
                                .flex_wrap()
                                .gap(px(4.))
                                .ml(px(-10.))
                                .children(screen.links),
                        )
                    }),
            )
            .into_any_element()
    }
}
