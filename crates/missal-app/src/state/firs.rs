use std::cell::RefCell;

use gpui_kit::{Context, EventEmitter};
use missal_core::FirId;

use super::rows::{FirRow, ListIndex};
use super::{Load, run_db};

pub enum FirsEvent {
    Reloaded,
    Changed(Vec<FirId>),
}

impl FirsEvent {
    pub fn touches(&self, id: FirId) -> bool {
        match self {
            FirsEvent::Reloaded => true,
            FirsEvent::Changed(ids) => ids.contains(&id),
        }
    }
}

pub struct Firs {
    pub rows: Vec<FirRow>,
    pub load: Load,
    pub index: RefCell<ListIndex>,
    version: u64,
}

impl EventEmitter<FirsEvent> for Firs {}

impl Firs {
    pub fn new(cx: &mut Context<Self>) -> Firs {
        let mut firs = Firs {
            rows: Vec::new(),
            load: Load::Loading,
            index: RefCell::new(ListIndex::default()),
            version: 0,
        };
        firs.reload(cx);
        firs
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        run_db(
            cx,
            |store| {
                Ok(store
                    .firs()?
                    .iter()
                    .map(FirRow::from_summary)
                    .collect::<Vec<_>>())
            },
            |this, result, cx| {
                match result {
                    Ok(rows) => {
                        this.rows = rows;
                        this.load = Load::Ready;
                    }
                    Err(error) => this.load = Load::Failed(error.message()),
                }
                this.changed(FirsEvent::Reloaded, cx);
            },
        );
    }

    fn changed(&mut self, event: FirsEvent, cx: &mut Context<Self>) {
        self.version += 1;
        cx.emit(event);
        cx.notify();
    }

    pub fn upsert(&mut self, row: FirRow, cx: &mut Context<Self>) {
        let id = row.id;
        match self.rows.iter().position(|existing| existing.id == row.id) {
            Some(position) => self.rows[position] = row,
            None => {
                let position = self
                    .rows
                    .iter()
                    .position(|existing| existing.id < row.id)
                    .unwrap_or(self.rows.len());
                self.rows.insert(position, row);
            }
        }
        self.load = Load::Ready;
        self.changed(FirsEvent::Changed(vec![id]), cx);
    }

    pub fn remove(&mut self, ids: &[FirId], cx: &mut Context<Self>) {
        self.rows.retain(|row| !ids.contains(&row.id));
        self.changed(FirsEvent::Changed(ids.to_vec()), cx);
    }

    pub fn get(&self, id: FirId) -> Option<&FirRow> {
        self.rows.iter().find(|row| row.id == id)
    }
}
