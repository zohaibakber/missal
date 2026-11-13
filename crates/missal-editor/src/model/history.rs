use std::collections::VecDeque;
use std::time::{Duration, Instant};

use missal_core::document::Block;
use missal_page::BlockPath;

use super::selection::Selection;

pub const DEPTH: usize = 100;
const COALESCE: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Typing,
    Delete,
    Compose,
    Format,
    Structure,
    Field,
    Paste,
}

impl Kind {
    fn coalesces(self) -> bool {
        matches!(self, Kind::Typing | Kind::Delete | Kind::Compose)
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub start: usize,
    pub blocks: Vec<Block>,
    pub len: usize,
    pub before: Selection,
    pub after: Selection,
    pub state_before: u64,
    pub state_after: u64,
    pub kind: Kind,
    pub host: Option<BlockPath>,
    pub at: Instant,
}

#[derive(Default)]
pub struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    open: bool,
}

impl History {
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open = false;
    }

    pub fn break_coalescing(&mut self) {
        self.open = false;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn mergeable(
        &self,
        kind: Kind,
        host: Option<&BlockPath>,
        range: &std::ops::Range<usize>,
        selection: &Selection,
    ) -> bool {
        let Some(last) = self.undo.back() else {
            return false;
        };
        self.open
            && kind.coalesces()
            && last.kind == kind
            && host.is_some()
            && last.host.as_ref() == host
            && last.start == range.start
            && last.len == range.len()
            && range.len() == 1
            && last.blocks.len() == 1
            && &last.after == selection
            && last.at.elapsed() < COALESCE
    }

    pub fn last_mut(&mut self) -> Option<&mut Entry> {
        self.undo.back_mut()
    }

    pub fn last(&self) -> Option<&Entry> {
        self.undo.back()
    }

    pub fn push(&mut self, entry: Entry) {
        self.open = entry.kind.coalesces();
        self.undo.push_back(entry);
        while self.undo.len() > DEPTH {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    pub fn touch(&mut self, selection: Selection, state: u64) {
        if let Some(last) = self.undo.back_mut() {
            last.after = selection;
            last.state_after = state;
            last.at = Instant::now();
        }
        self.redo.clear();
    }

    pub fn pop_undo(&mut self) -> Option<Entry> {
        self.open = false;
        self.undo.pop_back()
    }

    pub fn push_redo(&mut self, entry: Entry) {
        self.redo.push(entry);
    }

    pub fn pop_redo(&mut self) -> Option<Entry> {
        self.open = false;
        self.redo.pop()
    }

    pub fn push_undo_keep_redo(&mut self, entry: Entry) {
        self.undo.push_back(entry);
        while self.undo.len() > DEPTH {
            self.undo.pop_front();
        }
    }
}
