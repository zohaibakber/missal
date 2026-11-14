use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::{Context, EventEmitter, Task};

use crate::prefs::Preferences;

pub enum PrefsEvent {
    Changed,
}

pub struct PrefsModel {
    prefs: Preferences,
    path: Option<PathBuf>,
    save_task: Option<Task<()>>,
}

impl EventEmitter<PrefsEvent> for PrefsModel {}

impl PrefsModel {
    pub fn new(prefs: Preferences, path: Option<PathBuf>) -> PrefsModel {
        PrefsModel {
            prefs,
            path,
            save_task: None,
        }
    }

    pub fn read(&self) -> &Preferences {
        &self.prefs
    }

    pub fn update(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Preferences)) {
        let before = self.prefs.clone();
        change(&mut self.prefs);
        if self.prefs == before {
            return;
        }
        cx.emit(PrefsEvent::Changed);
        cx.notify();
        if self.path.is_some() {
            self.save_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(350))
                    .await;
                this.update(cx, |this, _| this.flush()).ok();
            }));
        }
    }

    pub fn flush(&mut self) {
        self.save_task = None;
        if let Some(path) = &self.path
            && let Err(error) = self.prefs.save(path)
        {
            eprintln!("missal: preferences were not saved: {error}");
        }
    }
}
