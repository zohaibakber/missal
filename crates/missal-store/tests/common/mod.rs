#![allow(dead_code)]

use std::path::{Path, PathBuf};

use missal_core::{Fir, FirStatus};
use rusqlite::{Connection, params};
use tempfile::TempDir;

pub const MIGRATIONS: [(&str, &str, i64, &str); 3] = [
    (
        "20261027233901_curly_gambit",
        "aea67cdce384afe64186ddd90086488df8504a8c57721a3b4dee6f0c8cca926f",
        1_793_144_341_000,
        include_str!("../../migrations/20261027233901_curly_gambit.sql"),
    ),
    (
        "20261028010119_chemical_nocturne",
        "680c1c24e13532ffacb3a388f9a154366c4279a0ddcec0e68b64166f812e570b",
        1_793_149_279_000,
        include_str!("../../migrations/20261028010119_chemical_nocturne.sql"),
    ),
    (
        "20261109041241_fir_case_fields",
        "1ffc7578f7704aa196d80b536911e22f0020e3f82503ae073a93b67c6ebc15b9",
        1_794_197_561_000,
        include_str!("../../migrations/20261109041241_fir_case_fields.sql"),
    ),
];

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

pub fn drizzle_database(path: &Path, applied: usize, legacy_table: bool) -> Connection {
    let connection = Connection::open(path).unwrap();
    if legacy_table {
        connection
            .execute_batch("CREATE TABLE \"__drizzle_migrations\" (id SERIAL PRIMARY KEY, hash text NOT NULL, created_at numeric)")
            .unwrap();
    } else {
        connection
            .execute_batch("CREATE TABLE IF NOT EXISTS \"__drizzle_migrations\" (id INTEGER PRIMARY KEY, hash text NOT NULL, created_at numeric, name text, applied_at TEXT)")
            .unwrap();
    }
    for (name, hash, millis, sql) in &MIGRATIONS[..applied] {
        for statement in sql.split("--> statement-breakpoint") {
            connection.execute_batch(statement).unwrap();
        }
        if legacy_table {
            connection
                .execute(
                    "INSERT INTO __drizzle_migrations (hash, created_at) VALUES (?1, ?2)",
                    params![hash, millis],
                )
                .unwrap();
        } else {
            connection
                .execute(
                    "INSERT INTO __drizzle_migrations (hash, created_at, name, applied_at) VALUES (?1, ?2, ?3, '2026-11-01T00:00:00.000Z')",
                    params![hash, millis, name],
                )
                .unwrap();
        }
    }
    connection
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

pub fn count(connection: &Connection, sql: &str) -> i64 {
    connection.query_row(sql, [], |row| row.get(0)).unwrap()
}
