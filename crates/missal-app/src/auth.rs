use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use gpui_kit::component::v_flex;
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, Global, SharedString, Task, Window, div, px,
};
use missal_auth::{
    Account, AuthError, Config, Session, SessionKeeper, SessionStore, begin_sign_in,
};

use crate::state::{Models, toast_warning};
use crate::theme::Palette;
use crate::ui::{button, primary_sm};

const REFRESH_EVERY: Duration = Duration::from_secs(60 * 60);

const ENDED: &str = "Your session has ended. Sign in again to continue.";
const STALE: &str =
    "Missal has been offline for more than 30 days. Connect to the internet and sign in again.";

#[derive(Clone)]
pub enum AuthState {
    Disabled,
    Restoring,
    SignedOut { notice: Option<SharedString> },
    SigningIn { cancel: Arc<AtomicBool> },
    SignedIn { account: Account, online: bool },
}

pub struct AuthModel {
    config: Option<Config>,
    store: Option<Arc<SessionStore>>,
    session: Option<Session>,
    keeper: Option<Arc<SessionKeeper>>,
    state: AuthState,
    refresh_loop: Option<Task<()>>,
}

struct AuthHandle(Entity<AuthModel>);

impl Global for AuthHandle {}

impl AuthModel {
    pub fn install(data_directory: Option<&Path>, cx: &mut App) -> Entity<AuthModel> {
        let config = Config::resolve();
        if config.is_none() {
            eprintln!("missal: sign-in is off because WORKOS_CLIENT_ID is not set");
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
            .clone()
            .map(|config| Arc::new(SessionKeeper::new(config, store.clone())));
        let entity = cx.new(|_| AuthModel {
            config,
            store,
            session: None,
            keeper,
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

    pub fn state(&self) -> &AuthState {
        &self.state
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

    pub fn keeper(&self) -> Option<Arc<SessionKeeper>> {
        self.keeper.clone()
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

    fn restore(&mut self, cx: &mut Context<Self>) {
        if self.config.is_none() {
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
                Ok(Some(session)) => {
                    if session.usable_offline(SystemTime::now()) {
                        this.set(
                            AuthState::SignedIn {
                                account: session.account.clone(),
                                online: false,
                            },
                            cx,
                        );
                    }
                    this.keep(Some(session));
                    this.refresh(cx);
                    this.schedule_refresh(cx);
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
            Ok(session) => {
                self.set(
                    AuthState::SignedIn {
                        account: session.account.clone(),
                        online: true,
                    },
                    cx,
                );
                self.session = Some(session);
            }
            Err(AuthError::Rejected(reason)) => {
                eprintln!("missal: session ended: {reason}");
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

    pub fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(config) = self.config.clone() else {
            return;
        };
        if matches!(self.state, AuthState::SigningIn { .. }) {
            return;
        }
        let pending = match begin_sign_in(&config) {
            Ok(pending) => pending,
            Err(error) => {
                self.set(
                    AuthState::SignedOut {
                        notice: Some(format!("Could not start sign-in: {error}.").into()),
                    },
                    cx,
                );
                return;
            }
        };
        cx.open_url(pending.url());
        let cancel = pending.canceller();
        self.set(
            AuthState::SigningIn {
                cancel: cancel.clone(),
            },
            cx,
        );
        let store = self.store.clone();
        let task = cx.background_spawn(async move {
            let session = pending.finish()?;
            let saved = match store.as_deref() {
                Some(store) => store.save(&session),
                None => Ok(()),
            };
            Ok::<_, AuthError>((session, saved))
        });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, window, cx| {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                match outcome {
                    Ok((session, saved)) => {
                        if let Err(error) = saved {
                            eprintln!("missal: could not save the session: {error}");
                            toast_warning(
                                window,
                                cx,
                                "Signed in, but this computer could not remember the session. You will need to sign in again next time.",
                            );
                        }
                        this.set(
                            AuthState::SignedIn {
                                account: session.account.clone(),
                                online: true,
                            },
                            cx,
                        );
                        this.keep(Some(session));
                        this.schedule_refresh(cx);
                    }
                    Err(AuthError::Cancelled) => {}
                    Err(error) => {
                        let notice = match error {
                            AuthError::Offline(_) => {
                                "Could not reach the sign-in service. Check the internet connection and try again.".to_owned()
                            }
                            AuthError::TimedOut => {
                                "Sign-in took too long. Try again.".to_owned()
                            }
                            other => format!("Sign-in failed: {other}."),
                        };
                        this.set(
                            AuthState::SignedOut {
                                notice: Some(notice.into()),
                            },
                            cx,
                        );
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn cancel_sign_in(&mut self, cx: &mut Context<Self>) {
        if let AuthState::SigningIn { cancel } = &self.state {
            cancel.store(true, Ordering::Relaxed);
            let notice = self.session.is_some().then_some(STALE);
            let session = self.session.take();
            self.signed_out(notice, cx);
            self.session = session;
        }
    }

    pub fn end_session(&mut self, cx: &mut Context<Self>) {
        if self.session.is_none() {
            return;
        }
        self.forget(cx);
        self.signed_out(Some(ENDED), cx);
    }

    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        self.forget(cx);
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

pub fn render_gate(auth: &Entity<AuthModel>, cx: &mut App) -> AnyElement {
    let palette = *Palette::get(cx);
    let state = auth.read(cx).state().clone();
    let (message, notice, action): (&str, Option<SharedString>, Option<AnyElement>) = match state {
        AuthState::Restoring => ("Checking your session…", None, None),
        AuthState::SigningIn { .. } => (
            "Finish signing in in your browser.",
            None,
            Some(
                button("cancel-sign-in", "Cancel")
                    .outline()
                    .sm()
                    .on_click({
                        let auth = auth.clone();
                        move |_, _, cx| auth.update(cx, |auth, cx| auth.cancel_sign_in(cx))
                    })
                    .into_any_element(),
            ),
        ),
        AuthState::SignedOut { notice } => (
            "Sign in to open your FIRs and templates.",
            notice,
            Some(
                primary_sm("sign-in", "Sign in")
                    .on_click({
                        let auth = auth.clone();
                        move |_, window, cx| auth.update(cx, |auth, cx| auth.sign_in(window, cx))
                    })
                    .into_any_element(),
            ),
        ),
        AuthState::Disabled | AuthState::SignedIn { .. } => return div().into_any_element(),
    };
    v_flex()
        .id("sign-in-gate")
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .bg(palette.surface)
        .child(
            div()
                .text_size(px(20.))
                .text_color(palette.ink)
                .child(crate::platform::PRODUCT_NAME),
        )
        .child(
            div()
                .text_size(px(14.))
                .text_color(palette.ink_muted)
                .child(message),
        )
        .children(notice.map(|notice| {
            div()
                .max_w(px(420.))
                .text_size(px(13.))
                .text_color(palette.danger_text)
                .child(notice)
        }))
        .children(action.map(|action| div().mt(px(4.)).child(action)))
        .into_any_element()
}
