use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::{Context, Task};

use crate::prefs::{Preferences, WindowPrefs, write_text};

pub struct PrefsModel {
    prefs: Preferences,
    path: Option<PathBuf>,
    save_task: Option<Task<()>>,
    write_task: Option<Task<()>>,
    writing: Arc<Mutex<()>>,
}

impl PrefsModel {
    pub fn new(prefs: Preferences, path: Option<PathBuf>) -> PrefsModel {
        PrefsModel {
            prefs,
            path,
            save_task: None,
            write_task: None,
            writing: Arc::new(Mutex::new(())),
        }
    }

    pub fn read(&self) -> &Preferences {
        &self.prefs
    }

    pub fn update(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Preferences)) {
        change(&mut self.prefs);
        cx.notify();
        self.schedule(cx);
    }

    pub fn set_window(&mut self, window: WindowPrefs, cx: &mut Context<Self>) {
        if self.prefs.window != Some(window) {
            self.prefs.window = Some(window);
            self.schedule(cx);
        }
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        if self.path.is_none() || self.save_task.is_some() {
            return;
        }
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(350))
                .await;
            this.update(cx, |this, cx| this.write_in_background(cx))
                .ok();
        }));
    }

    fn write_in_background(&mut self, cx: &mut Context<Self>) {
        self.save_task = None;
        let Some(path) = self.path.clone() else {
            return;
        };
        let text = match self.prefs.serialize() {
            Ok(text) => text,
            Err(error) => {
                eprintln!("missal: preferences were not saved: {error}");
                return;
            }
        };
        let writing = self.writing.clone();
        let previous = self.write_task.take();
        self.write_task = Some(cx.background_executor().spawn(async move {
            if let Some(previous) = previous {
                previous.await;
            }
            let _guard = writing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Err(error) = write_text(&path, &text) {
                eprintln!("missal: preferences were not saved: {error}");
            }
        }));
    }

    pub fn flush(&mut self) {
        self.save_task = None;
        self.write_task = None;
        if let Some(path) = &self.path {
            let _guard = self
                .writing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Err(error) = self.prefs.save(path) {
                eprintln!("missal: preferences were not saved: {error}");
            }
        }
    }
}
