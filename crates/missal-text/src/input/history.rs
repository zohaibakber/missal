use std::time::{Duration, Instant};

use crate::layout::TextPosition;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Snapshot {
    pub(crate) text: String,
    pub(crate) anchor: TextPosition,
    pub(crate) head: TextPosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditKind {
    Typing,
    Delete,
    Compose,
    Other,
}

const COALESCE_WINDOW: Duration = Duration::from_millis(1200);
const LIMIT: usize = 500;

#[derive(Default)]
pub(crate) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    open: Option<(EditKind, Instant)>,
}

impl History {
    pub(crate) fn record(&mut self, before: Snapshot, kind: EditKind) {
        let now = Instant::now();
        let coalesce = match self.open {
            Some((open, at)) => {
                open == kind
                    && kind != EditKind::Other
                    && (kind == EditKind::Compose || now.duration_since(at) < COALESCE_WINDOW)
            }
            None => false,
        };
        if !coalesce {
            self.undo.push(before);
            if self.undo.len() > LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.open = (kind != EditKind::Other).then_some((kind, now));
    }

    pub(crate) fn break_coalescing(&mut self) {
        self.open = None;
    }

    pub(crate) fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let previous = self.undo.pop()?;
        self.redo.push(current);
        self.open = None;
        Some(previous)
    }

    pub(crate) fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        self.open = None;
        Some(next)
    }

    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open = None;
    }
}
