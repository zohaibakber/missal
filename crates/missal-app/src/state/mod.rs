pub mod catalog;
pub mod firs;
pub mod nav;
pub mod prefs_model;
pub mod rows;
pub mod templates;

use gpui_kit::{App, AppContext, Context, Entity, Global, Window};
use missal_store::Store;

use crate::db::{Db, StoreResult};
use crate::paths::Paths;
use crate::prefs::Preferences;

pub use catalog::Catalog;
pub use firs::Firs;
pub use nav::{Navigator, PrintTarget, Route};
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
    pub fn install(
        db: Db,
        paths: Option<&Paths>,
        preferences: Preferences,
        cx: &mut App,
    ) -> Models {
        cx.set_global(db);
        let models = Models {
            firs: cx.new(Firs::new),
            templates: cx.new(Templates::new),
            catalog: cx.new(Catalog::new),
            nav: cx.new(|_| Navigator::new(Route::Firs)),
            prefs: cx.new(|_| {
                PrefsModel::new(preferences, paths.map(|paths| paths.preferences.clone()))
            }),
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
        this.update_in(cx, |this, window, cx| done(this, result, window, cx))
            .ok();
    })
    .detach();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

fn push_toast(
    window: &mut Window,
    cx: &mut App,
    kind: ToastKind,
    title: Option<gpui_kit::SharedString>,
    message: gpui_kit::SharedString,
) {
    let mut spec = crate::ui::toast::ToastSpec::new(kind, message);
    if let Some(title) = title {
        spec = spec.title(title);
    }
    show_toast(window, cx, spec);
}

pub fn show_toast(_: &mut Window, cx: &mut App, spec: crate::ui::toast::ToastSpec) {
    if let Some(host) = crate::ui::toast::ToastHost::get(cx) {
        host.update(cx, |host, cx| {
            host.push(spec, cx);
        });
    }
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

pub fn toast_success(
    window: &mut Window,
    cx: &mut App,
    message: impl Into<gpui_kit::SharedString>,
) {
    push_toast(window, cx, ToastKind::Success, None, message.into());
}

pub fn toast_error(window: &mut Window, cx: &mut App, message: impl Into<gpui_kit::SharedString>) {
    push_toast(window, cx, ToastKind::Error, None, message.into());
}

pub fn toast_warning(
    window: &mut Window,
    cx: &mut App,
    message: impl Into<gpui_kit::SharedString>,
) {
    push_toast(window, cx, ToastKind::Warning, None, message.into());
}

pub fn toast_info(window: &mut Window, cx: &mut App, message: impl Into<gpui_kit::SharedString>) {
    push_toast(window, cx, ToastKind::Info, None, message.into());
}
