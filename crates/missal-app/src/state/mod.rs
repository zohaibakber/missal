pub mod catalog;
pub mod firs;
pub mod nav;
pub mod prefs_model;
pub mod rows;
pub mod templates;

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use gpui_kit::{App, AppContext, Context, Entity, Global, Task, Window};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt;
use missal_core::RepositoryError;
use missal_store::Store;

use crate::db::{Db, StoreResult};
use crate::paths::Paths;
use crate::prefs::Preferences;

pub use catalog::Catalog;
pub use firs::Firs;
pub use nav::{Navigator, Route};
pub use prefs_model::PrefsModel;
pub use templates::Templates;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load {
    Loading,
    Ready,
    Failed(String),
}

impl Load {
    pub fn is_ready(&self) -> bool {
        matches!(self, Load::Ready)
    }
}

#[derive(Clone)]
pub struct Models {
    pub firs: Entity<Firs>,
    pub templates: Entity<Templates>,
    pub catalog: Entity<Catalog>,
    pub nav: Entity<Navigator>,
    pub prefs: Entity<PrefsModel>,
}

impl Global for Models {}

impl Models {
    pub fn install(db: Db, paths: Option<&Paths>, preferences: Preferences, cx: &mut App) -> Models {
        cx.set_global(db);
        let models = Models {
            firs: cx.new(Firs::new),
            templates: cx.new(Templates::new),
            catalog: cx.new(Catalog::new),
            nav: cx.new(|_| Navigator::new(Route::Firs)),
            prefs: cx.new(|_| PrefsModel::new(preferences, paths.map(|paths| paths.preferences.clone()))),
        };
        cx.set_global(models.clone());
        models
    }

    pub fn get(cx: &App) -> Models {
        cx.global::<Models>().clone()
    }
}

pub fn db(cx: &App) -> Db {
    cx.global::<Db>().clone()
}

pub fn run_db<V, T, J, D>(cx: &mut Context<V>, job: J, done: D)
where
    V: 'static,
    T: Send + 'static,
    J: FnOnce(&mut Store) -> StoreResult<T> + Send + 'static,
    D: FnOnce(&mut V, StoreResult<T>, &mut Context<V>) + 'static,
{
    let reply = db(cx).run(job);
    cx.spawn(async move |this, cx| {
        let result = reply.await;
        this.update(cx, |this, cx| done(this, result, cx)).ok();
    })
    .detach();
}

pub fn run_db_in<V, T, J, D>(window: &Window, cx: &mut Context<V>, job: J, done: D)
where
    V: 'static,
    T: Send + 'static,
    J: FnOnce(&mut Store) -> StoreResult<T> + Send + 'static,
    D: FnOnce(&mut V, StoreResult<T>, &mut Window, &mut Context<V>) + 'static,
{
    let reply = db(cx).run(job);
    cx.spawn_in(window, async move |this, cx| {
        let result = reply.await;
        this.update_in(cx, |this, window, cx| done(this, result, window, cx)).ok();
    })
    .detach();
}

pub fn spawn_db<T, J>(cx: &mut App, job: J) -> Task<StoreResult<T>>
where
    T: Send + 'static,
    J: FnOnce(&mut Store) -> StoreResult<T> + Send + 'static,
{
    let reply = db(cx).run(job);
    cx.spawn(async move |_| reply.await)
}

pub fn await_reply<T: Send + 'static>(
    reply: impl Future<Output = StoreResult<T>> + 'static,
) -> impl Future<Output = StoreResult<T>> {
    reply
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub kind: ToastKind,
    pub title: Option<String>,
    pub message: String,
}

#[derive(Clone, Default)]
pub struct ToastLog(pub Rc<RefCell<Vec<Toast>>>);

impl Global for ToastLog {}

fn push_toast(
    window: &mut Window,
    cx: &mut App,
    kind: ToastKind,
    title: Option<gpui_kit::SharedString>,
    message: gpui_kit::SharedString,
) {
    if let Some(log) = cx.try_global::<ToastLog>() {
        log.0.borrow_mut().push(Toast {
            kind,
            title: title.as_ref().map(|title| title.to_string()),
            message: message.to_string(),
        });
    }
    let notification = match kind {
        ToastKind::Info => Notification::info(message),
        ToastKind::Success => Notification::success(message),
        ToastKind::Warning => Notification::warning(message),
        ToastKind::Error => Notification::error(message),
    };
    let notification = match title {
        Some(title) => notification.title(title),
        None => notification,
    };
    window.push_notification(notification, cx);
}

pub fn toast_detailed(
    window: &mut Window,
    cx: &mut App,
    kind: ToastKind,
    title: impl Into<gpui_kit::SharedString>,
    message: impl Into<gpui_kit::SharedString>,
) {
    push_toast(window, cx, kind, Some(title.into()), message.into());
}

pub fn toast_success(window: &mut Window, cx: &mut App, message: impl Into<gpui_kit::SharedString>) {
    push_toast(window, cx, ToastKind::Success, None, message.into());
}

pub fn toast_error(window: &mut Window, cx: &mut App, message: impl Into<gpui_kit::SharedString>) {
    push_toast(window, cx, ToastKind::Error, None, message.into());
}

pub fn toast_warning(window: &mut Window, cx: &mut App, message: impl Into<gpui_kit::SharedString>) {
    push_toast(window, cx, ToastKind::Warning, None, message.into());
}

pub fn toast_info(window: &mut Window, cx: &mut App, message: impl Into<gpui_kit::SharedString>) {
    push_toast(window, cx, ToastKind::Info, None, message.into());
}

pub fn not_available_yet(window: &mut Window, cx: &mut App, what: &str) {
    toast_info(window, cx, format!("{what} is not available yet"));
}

pub fn error_text(error: &RepositoryError) -> String {
    error.message()
}
