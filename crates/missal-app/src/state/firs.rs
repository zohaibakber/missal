use std::cell::RefCell;

use gpui_kit::{Context, EventEmitter};
use missal_core::FirId;
use missal_core::parallel::map_chunks;

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

const ROWS_PER_WORKER: usize = 4096;

pub struct Firs {
    rows: Vec<FirRow>,
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

    pub fn rows(&self) -> &[FirRow] {
        &self.rows
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        run_db(
            cx,
            |store| {
                Ok(map_chunks(
                    &store.firs()?,
                    ROWS_PER_WORKER,
                    FirRow::from_summary,
                ))
            },
            |this, result, cx| {
                match result {
                    Ok(rows) => {
                        debug_assert!(rows.is_sorted_by(|a, b| a.id > b.id));
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
        match self.position(id) {
            Ok(position) => self.rows[position] = row,
            Err(position) => self.rows.insert(position, row),
        }
        self.load = Load::Ready;
        self.changed(FirsEvent::Changed(vec![id]), cx);
    }

    pub fn remove(&mut self, ids: &[FirId], cx: &mut Context<Self>) {
        self.rows.retain(|row| !ids.contains(&row.id));
        self.changed(FirsEvent::Changed(ids.to_vec()), cx);
    }

    pub fn get(&self, id: FirId) -> Option<&FirRow> {
        self.position(id).ok().map(|position| &self.rows[position])
    }

    fn position(&self, id: FirId) -> Result<usize, usize> {
        self.rows.binary_search_by(|row| row.id.cmp(&id).reverse())
    }
}
