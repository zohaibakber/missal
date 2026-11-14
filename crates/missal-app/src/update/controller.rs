use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{AnyWindowHandle, App, AppContext, Context, Entity, EventEmitter, Global, Window};

use super::installer::{self, Ready};
use crate::state::{Models, ToastKind, show_toast, toast_error, toast_warning};
use crate::ui::toast::ToastSpec;

pub fn enabled(data_dir_overridden: bool) -> bool {
    !cfg!(debug_assertions)
        && !data_dir_overridden
        && std::env::var_os("MISSAL_NO_UPDATE_CHECK").is_none()
}

#[derive(Clone)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate { at: Instant },
    Ready(Rc<Ready>),
    Failed { at: Instant, message: String },
}

pub struct UpdateStatus {
    state: UpdateState,
}

impl UpdateStatus {
    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    fn set(&mut self, state: UpdateState, cx: &mut Context<Self>) {
        self.state = state;
        cx.notify();
    }
}

impl EventEmitter<()> for UpdateStatus {}

struct StatusHandle(Entity<UpdateStatus>);

impl Global for StatusHandle {}

pub fn status(cx: &mut App) -> Entity<UpdateStatus> {
    if let Some(handle) = cx.try_global::<StatusHandle>() {
        return handle.0.clone();
    }
    let entity = cx.new(|_| UpdateStatus {
        state: UpdateState::Idle,
    });
    cx.set_global(StatusHandle(entity.clone()));
    entity
}

pub fn start(window: AnyWindowHandle, cx: &mut App) {
    run(window, false, cx);
}

pub fn check_now(window: &mut Window, cx: &mut App) {
    if matches!(status(cx).read(cx).state, UpdateState::Checking) {
        return;
    }
    let handle = window.window_handle();
    run(handle, true, cx);
}

fn run(window: AnyWindowHandle, manual: bool, cx: &mut App) {
    let status = status(cx);
    status.update(cx, |status, cx| status.set(UpdateState::Checking, cx));
    let fetch = cx.background_spawn(async { installer::fetch() });
    cx.spawn(async move |cx| {
        let outcome = fetch.await;
        cx.update(|cx| {
            window
                .update(cx, |_, window, cx| match outcome {
                    Ok(Some(ready)) => {
                        let ready = Rc::new(ready);
                        status.update(cx, |status, cx| {
                            status.set(UpdateState::Ready(ready.clone()), cx)
                        });
                        announce(ready, window, cx);
                    }
                    Ok(None) => {
                        status.update(cx, |status, cx| {
                            status.set(UpdateState::UpToDate { at: Instant::now() }, cx)
                        });
                        if manual {
                            show_toast(
                                window,
                                cx,
                                ToastSpec::new(ToastKind::Success, "Missal is up to date."),
                            );
                        }
                    }
                    Err(error) => {
                        eprintln!("missal: update check failed: {error}");
                        status.update(cx, |status, cx| {
                            status.set(
                                UpdateState::Failed {
                                    at: Instant::now(),
                                    message: error.clone(),
                                },
                                cx,
                            )
                        });
                        if manual {
                            toast_error(window, cx, "Could not check for updates.");
                        }
                    }
                })
                .ok();
        });
    })
    .detach();
}

pub fn announce(ready: Rc<Ready>, window: &mut Window, cx: &mut App) {
    let version = ready.version().to_owned();
    let spec = ToastSpec::new(
        ToastKind::Info,
        format!("Version {version} is downloaded. Restart to apply it."),
    )
    .title("Update ready")
    .action("Restart now", move |window, cx| {
        restart(ready.clone(), window, cx)
    });
    show_toast(window, cx, spec);
}

pub fn restart(ready: Rc<Ready>, window: &mut Window, cx: &mut App) {
    let nav = Models::get(cx).nav;
    if nav.read(cx).has_unsaved_changes(cx) {
        toast_warning(
            window,
            cx,
            "You have unsaved changes. Save or close your work, then restart to update.",
        );
        announce(ready, window, cx);
        return;
    }
    let prefs = Models::get(cx).prefs;
    prefs.update(cx, |prefs, _| prefs.flush());
    match ready.apply_on_exit() {
        Ok(()) => cx.quit(),
        Err(error) => toast_error(window, cx, format!("Could not apply the update ({error})")),
    }
}
