use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt;
use gpui_kit::{AnyWindowHandle, App, AppContext, Window};

use super::installer::{self, Ready};
use crate::state::{Models, toast_error, toast_warning};

pub fn enabled(data_dir_overridden: bool) -> bool {
    !cfg!(debug_assertions) && !data_dir_overridden && std::env::var_os("MISSAL_NO_UPDATE_CHECK").is_none()
}

pub fn start(window: AnyWindowHandle, cx: &mut App) {
    let fetch = cx.background_spawn(async { installer::fetch() });
    cx.spawn(async move |cx| match fetch.await {
        Ok(Some(ready)) => {
            let ready = Rc::new(ready);
            cx.update(|cx| {
                window
                    .update(cx, |_, window, cx| announce(ready, window, cx))
                    .ok();
            });
        }
        Ok(None) => {}
        Err(error) => eprintln!("missal: update check failed: {error}"),
    })
    .detach();
}

fn announce(ready: Rc<Ready>, window: &mut Window, cx: &mut App) {
    let version = ready.version().to_owned();
    let notification = Notification::info(format!("Version {version} is downloaded. Restart to apply it."))
        .title("Update ready")
        .action(move |_, _, _| {
            let ready = ready.clone();
            Button::new("update-restart")
                .primary()
                .label("Restart now")
                .on_click(move |_, window, cx| restart(ready.clone(), window, cx))
        });
    window.push_notification(notification, cx);
}

fn restart(ready: Rc<Ready>, window: &mut Window, cx: &mut App) {
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
