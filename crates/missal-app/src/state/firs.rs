use gpui_kit::{Context, EventEmitter};
use missal_core::FirId;

use super::rows::FirRow;
use super::{Load, run_db};

pub enum FirsEvent {
    Changed,
}

pub struct Firs {
    pub rows: Vec<FirRow>,
    pub load: Load,
    version: u64,
}

impl EventEmitter<FirsEvent> for Firs {}

impl Firs {
    pub fn new(cx: &mut Context<Self>) -> Firs {
        let mut firs = Firs {
            rows: Vec::new(),
            load: Load::Loading,
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
                this.changed(cx);
            },
        );
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.version += 1;
        cx.emit(FirsEvent::Changed);
        cx.notify();
    }

    pub fn upsert(&mut self, row: FirRow, cx: &mut Context<Self>) {
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
        self.changed(cx);
    }

    pub fn remove(&mut self, ids: &[FirId], cx: &mut Context<Self>) {
        self.rows.retain(|row| !ids.contains(&row.id));
        self.changed(cx);
    }

    pub fn get(&self, id: FirId) -> Option<&FirRow> {
        self.rows.iter().find(|row| row.id == id)
    }

    pub fn recent(&self, count: usize) -> &[FirRow] {
        &self.rows[..self.rows.len().min(count)]
    }
}
