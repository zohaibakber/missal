use std::time::{Duration, Instant};

use gpui_kit::{App, Context, Entity, Task, WeakEntity, Window};
use missal_core::{Document, Revision};
use missal_editor::Editor;
use missal_store::DraftKey;

use crate::state::{ToastKind, db, show_toast};
use crate::ui::toast::ToastSpec;

pub const IDLE: Duration = Duration::from_millis(1200);
pub const MAX_WAIT: Duration = Duration::from_secs(8);

#[derive(Default)]
pub struct Autosave {
    target: Option<(DraftKey, Revision)>,
    first_edit: Option<Instant>,
    last_edit: Option<Instant>,
    stored: bool,
    task: Option<Task<()>>,
}

pub trait Autosaving: Sized + 'static {
    fn autosave(&mut self) -> &mut Autosave;
    fn autosave_ready(&self, cx: &App) -> bool;
    fn autosave_editor(&self) -> &Entity<Editor>;
}

impl Autosave {
    pub fn target(&self) -> Option<(DraftKey, Revision)> {
        self.target
    }

    pub fn is_pending(&self) -> bool {
        self.first_edit.is_some()
    }

    pub fn track(&mut self, key: DraftKey, revision: Revision, stored: bool) {
        if self.target.is_some_and(|(current, _)| current != key) {
            self.cancel();
        }
        self.target = Some((key, revision));
        self.stored = stored;
    }

    pub fn saved(&mut self, revision: Revision) {
        if let Some((_, current)) = self.target.as_mut() {
            *current = revision;
        }
        self.stored = false;
    }

    pub fn cancel(&mut self) {
        self.first_edit = None;
        self.last_edit = None;
        self.task = None;
    }

    pub fn untrack(&mut self) {
        self.cancel();
        self.target = None;
        self.stored = false;
    }

    pub fn discard(&mut self, cx: &App) {
        self.cancel();
        if let Some((key, _)) = self.target
            && self.stored
        {
            self.stored = false;
            db(cx).run(move |store| store.discard_draft(key));
        }
    }

    fn due(&self) -> Option<Instant> {
        let first = self.first_edit?;
        let last = self.last_edit.unwrap_or(first);
        Some((last + IDLE).min(first + MAX_WAIT))
    }

    fn write(&mut self, document: Option<Document>, cx: &App) {
        self.cancel();
        let Some((key, revision)) = self.target else {
            return;
        };
        match document {
            Some(document) => {
                self.stored = true;
                db(cx).run(move |store| store.put_draft(key, revision, &document));
            }
            None if self.stored => {
                self.stored = false;
                db(cx).run(move |store| store.discard_draft(key));
            }
            None => {}
        }
    }
}

pub fn edited<V: Autosaving>(view: &mut V, cx: &mut Context<V>) {
    let now = cx.background_executor().now();
    let autosave = view.autosave();
    if autosave.target.is_none() {
        return;
    }
    autosave.last_edit = Some(now);
    if autosave.first_edit.is_none() {
        autosave.first_edit = Some(now);
    }
    if autosave.task.is_some() {
        return;
    }
    autosave.task = Some(cx.spawn(async move |this: WeakEntity<V>, cx| {
        loop {
            let Ok(Some(due)) = this.update(cx, |view, _| view.autosave().due()) else {
                return;
            };
            let wait = due.saturating_duration_since(cx.background_executor().now());
            if !wait.is_zero() {
                cx.background_executor().timer(wait).await;
                continue;
            }
            this.update(cx, |view, cx| flush(view, cx)).ok();
            return;
        }
    }));
}

pub fn flush<V: Autosaving>(view: &mut V, cx: &mut Context<V>) {
    if !view.autosave().is_pending() {
        return;
    }
    if !view.autosave_ready(cx) {
        view.autosave().task = None;
        return;
    }
    let editor = view.autosave_editor().read(cx);
    let document = editor.is_dirty().then(|| editor.document().clone());
    view.autosave().write(document, cx);
}

pub fn resume<V: Autosaving>(view: &mut V, cx: &mut Context<V>) {
    if view.autosave().is_pending() && view.autosave().task.is_none() {
        edited(view, cx);
    }
}

pub fn announce_recovered(
    window: &mut Window,
    cx: &mut App,
    discard: impl Fn(&mut Window, &mut App) + 'static,
) {
    let spec = ToastSpec::new(
        ToastKind::Info,
        "Unsaved changes from your last session were restored. Save to keep them.",
    )
    .title("Changes recovered")
    .action("Discard", discard);
    show_toast(window, cx, spec);
}
