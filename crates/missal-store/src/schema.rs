use rusqlite::{Connection, Transaction};

use crate::{Result, failed, open_error};

const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    open_marker TEXT NOT NULL DEFAULT '«',
    close_marker TEXT NOT NULL DEFAULT '»',
    bundle_version INTEGER NOT NULL DEFAULT 0
);
INSERT INTO settings (id) VALUES (1);

CREATE TABLE placeholders (
    id INTEGER PRIMARY KEY,
    label TEXT NOT NULL UNIQUE,
    shared INTEGER NOT NULL,
    value TEXT NOT NULL DEFAULT ''
);

CREATE TABLE firs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fir_no TEXT NOT NULL,
    date TEXT NOT NULL,
    offence TEXT NOT NULL,
    incident_date TEXT NOT NULL,
    arrest_date TEXT NOT NULL,
    status INTEGER NOT NULL,
    accused TEXT NOT NULL,
    complainant TEXT NOT NULL,
    witness TEXT NOT NULL,
    zimni TEXT NOT NULL,
    investigation_officer TEXT NOT NULL,
    writer TEXT NOT NULL,
    recovery TEXT NOT NULL,
    imei_1 TEXT NOT NULL,
    imei_2 TEXT NOT NULL,
    sim_1 TEXT NOT NULL,
    sim_2 TEXT NOT NULL,
    body TEXT NOT NULL
);

CREATE TABLE fir_values (
    fir_id INTEGER NOT NULL REFERENCES firs (id) ON DELETE CASCADE,
    placeholder_id INTEGER NOT NULL REFERENCES placeholders (id) ON DELETE CASCADE,
    value TEXT NOT NULL,
    PRIMARY KEY (fir_id, placeholder_id)
) WITHOUT ROWID;
CREATE INDEX fir_values_placeholder ON fir_values (placeholder_id);

CREATE TABLE templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    revision INTEGER NOT NULL,
    field_count INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    preview TEXT NOT NULL
);
CREATE INDEX templates_recent ON templates (updated_at DESC, id DESC);

CREATE TABLE template_bodies (
    template_id INTEGER PRIMARY KEY REFERENCES templates (id) ON DELETE CASCADE,
    plain_text TEXT NOT NULL,
    body BLOB NOT NULL
);

CREATE TABLE documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fir_id INTEGER NOT NULL REFERENCES firs (id) ON DELETE CASCADE,
    template_id INTEGER NOT NULL REFERENCES templates (id) ON DELETE RESTRICT,
    source_revision INTEGER NOT NULL,
    title TEXT NOT NULL,
    revision INTEGER NOT NULL,
    position INTEGER NOT NULL,
    field_count INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    preview TEXT NOT NULL
);
CREATE UNIQUE INDEX documents_fir_template ON documents (fir_id, template_id);
CREATE INDEX documents_template ON documents (template_id);

CREATE TABLE document_bodies (
    document_id INTEGER PRIMARY KEY REFERENCES documents (id) ON DELETE CASCADE,
    body BLOB NOT NULL
);
"#];

pub(crate) fn configure(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -16000;
             PRAGMA mmap_size = 67108864;",
        )
        .and_then(|()| connection.busy_timeout(std::time::Duration::from_secs(5)))
        .map_err(failed("database.open"))
}

pub(crate) fn migrate(connection: &mut Connection) -> Result<()> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(failed("database.migrate"))?;
    let version = usize::try_from(version).unwrap_or(usize::MAX);
    if version > MIGRATIONS.len() {
        return Err(open_error(
            "This database was written by a newer version of Missal. Update Missal to open it.",
        ));
    }
    if version == 0 && has_tables(connection)? {
        return Err(open_error("This file is not a Missal database."));
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        let transaction = connection
            .transaction()
            .map_err(failed("database.migrate"))?;
        apply(&transaction, sql, index + 1).map_err(|_| {
            open_error(&format!(
                "The database could not be upgraded (step {}).",
                index + 1
            ))
        })?;
        transaction.commit().map_err(failed("database.migrate"))?;
    }
    Ok(())
}

fn apply(transaction: &Transaction<'_>, sql: &str, version: usize) -> rusqlite::Result<()> {
    transaction.execute_batch(sql)?;
    transaction.pragma_update(None, "user_version", version as i64)
}

fn has_tables(connection: &Connection) -> Result<bool> {
    connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table')",
            [],
            |row| row.get(0),
        )
        .map_err(failed("database.migrate"))
}
