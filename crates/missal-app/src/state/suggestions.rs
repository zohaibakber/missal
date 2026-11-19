use std::sync::Arc;

use gpui_kit::{Context, Entity};
use missal_core::{Suggestion, SuggestionIndex, SuggestionKey};

use super::firs::{Firs, FirsEvent};
use super::run_db;

#[derive(Default)]
pub struct Suggestions {
    index: Arc<SuggestionIndex>,
    loading: bool,
    stale: bool,
}

impl Suggestions {
    pub fn new(firs: &Entity<Firs>, cx: &mut Context<Self>) -> Suggestions {
        cx.subscribe(firs, |this, _, _: &FirsEvent, cx| this.reload(cx))
            .detach();
        Suggestions::default()
    }

    pub fn index(&self) -> &Arc<SuggestionIndex> {
        &self.index
    }

    pub fn suggest(&self, key: SuggestionKey, query: &str, limit: usize) -> Vec<Suggestion> {
        self.index.suggest(key, query, limit)
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            self.stale = true;
            return;
        }
        self.loading = true;
        run_db(
            cx,
            |store| Ok(SuggestionIndex::build(store.suggestion_entries()?)),
            |this, result, cx| {
                this.loading = false;
                if let Ok(index) = result {
                    this.index = Arc::new(index);
                    cx.notify();
                }
                if std::mem::take(&mut this.stale) {
                    this.reload(cx);
                }
            },
        );
    }
}
