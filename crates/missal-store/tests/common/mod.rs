use std::path::PathBuf;

use missal_core::{Fir, FirStatus};
use tempfile::TempDir;

pub struct Scratch {
    pub dir: TempDir,
}

impl Scratch {
    pub fn new() -> Scratch {
        Scratch {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

pub fn fir(number: &str) -> Fir {
    Fir {
        fir_no: number.into(),
        date: "12-09-2026".into(),
        offence: "چوری ۳۷۹ ت پ".into(),
        incident_date: "11-09-2026".into(),
        accused: vec!["علی ولد احمد".into()],
        status: FirStatus::Open,
        ..Fir::default()
    }
}
