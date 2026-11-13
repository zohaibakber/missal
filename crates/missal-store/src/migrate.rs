use std::collections::{HashMap, HashSet};

use missal_core::field::{fir_field_by_key, json_int, map_references};
use missal_core::{FieldReference, field_lookup_key};
use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};
use serde_json::Value;

use crate::{Result, failed, now, open_error};

pub(crate) struct DrizzleMigration {
    pub name: &'static str,
    pub hash: &'static str,
    pub folder_millis: i64,
    pub sql: &'static str,
}

pub(crate) const DRIZZLE_MIGRATIONS: [DrizzleMigration; 3] = [
    DrizzleMigration {
        name: "20261027233901_curly_gambit",
        hash: "aea67cdce384afe64186ddd90086488df8504a8c57721a3b4dee6f0c8cca926f",
        folder_millis: 1_793_144_341_000,
        sql: include_str!("../migrations/20261027233901_curly_gambit.sql"),
    },
    DrizzleMigration {
        name: "20261028010119_chemical_nocturne",
        hash: "680c1c24e13532ffacb3a388f9a154366c4279a0ddcec0e68b64166f812e570b",
        folder_millis: 1_793_149_279_000,
        sql: include_str!("../migrations/20261028010119_chemical_nocturne.sql"),
    },
    DrizzleMigration {
        name: "20261109041241_fir_case_fields",
        hash: "1ffc7578f7704aa196d80b536911e22f0020e3f82503ae073a93b67c6ebc15b9",
        folder_millis: 1_794_197_561_000,
        sql: include_str!("../migrations/20261109041241_fir_case_fields.sql"),
    },
];

const DRIZZLE_TABLE: &str = "__drizzle_migrations";
const DATA_MIGRATIONS_TABLE: &str = "data_migrations";
const FOLD_PLACEHOLDERS: (i64, &str) = (1, "fold_placeholders_into_fir_fields");

const APP_TABLES: [&str; 6] = [
    "app_settings",
    "fir_documents",
    "fir_placeholder_values",
    "fir_records",
    "placeholders",
    "templates",
];

pub(crate) struct SchemaMigration {
    pub name: &'static str,
    pub apply: fn(&Transaction<'_>) -> rusqlite::Result<()>,
}

pub(crate) const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[];

pub(crate) struct Baseline {
    applied: usize,
    has_table: bool,
    has_name: bool,
}

pub(crate) fn inspect(connection: &Connection) -> Result<Baseline> {
    let tables = tables(connection).map_err(failed("database.migrate"))?;
    let has_table = tables.contains(DRIZZLE_TABLE);
    let has_name = has_table
        && columns(connection, DRIZZLE_TABLE)
            .map_err(failed("database.migrate"))?
            .contains("name");
    let applied = if has_table {
        applied_drizzle_migrations(connection, has_name)?
    } else if tables.is_empty() {
        0
    } else {
        return Err(open_error(
            "This file is not a Missal database, or was made by a version that cannot be upgraded.",
        ));
    };
    verify_shape(connection, applied, &tables)?;
    schema_version(connection, SCHEMA_MIGRATIONS)?;
    Ok(Baseline {
        applied,
        has_table,
        has_name,
    })
}

pub(crate) fn migrate(connection: &mut Connection, baseline: &Baseline) -> Result<()> {
    apply_drizzle_baseline(connection, baseline)?;
    apply_data_migrations(connection)?;
    apply_schema_migrations(connection, SCHEMA_MIGRATIONS)
}

fn tables(connection: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )?;
    statement.query_map([], |row| row.get(0))?.collect()
}

fn columns(connection: &Connection, table: &str) -> rusqlite::Result<HashSet<String>> {
    let mut statement = connection.prepare("SELECT name FROM pragma_table_info(?1)")?;
    statement.query_map([table], |row| row.get(0))?.collect()
}

struct DrizzleRow {
    id: Option<i64>,
    hash: String,
    created_at: Option<Value>,
    name: Option<String>,
}

fn sql_value(value: rusqlite::types::ValueRef<'_>) -> Option<Value> {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => None,
        ValueRef::Integer(int) => Some(Value::from(int)),
        ValueRef::Real(real) => Some(Value::from(real)),
        ValueRef::Text(text) => Some(Value::from(String::from_utf8_lossy(text).into_owned())),
        ValueRef::Blob(_) => None,
    }
}

fn drizzle_millis(created_at: &Value) -> Option<i64> {
    let text = match created_at {
        Value::String(text) => text.clone(),
        Value::Number(number) => match number.as_i64() {
            Some(int) => int.to_string(),
            None => number.to_string(),
        },
        _ => return None,
    };
    let head = text.get(..text.len().checked_sub(3)?)?;
    format!("{head}000").parse().ok()
}

fn match_drizzle_row(row: &DrizzleRow) -> Option<usize> {
    if let Some(name) = &row.name {
        return DRIZZLE_MIGRATIONS
            .iter()
            .position(|migration| migration.name == name);
    }
    let millis = row.created_at.as_ref().and_then(drizzle_millis);
    let by_millis: Vec<usize> = DRIZZLE_MIGRATIONS
        .iter()
        .enumerate()
        .filter(|(_, migration)| Some(migration.folder_millis) == millis)
        .map(|(index, _)| index)
        .collect();
    match by_millis.as_slice() {
        [index] => Some(*index),
        _ => DRIZZLE_MIGRATIONS
            .iter()
            .position(|migration| migration.hash == row.hash),
    }
}

fn applied_drizzle_migrations(connection: &Connection, has_name: bool) -> Result<usize> {
    let query = if has_name {
        "SELECT id, hash, created_at, name FROM __drizzle_migrations ORDER BY id"
    } else {
        "SELECT id, hash, created_at, NULL FROM __drizzle_migrations ORDER BY id"
    };
    let rows = (|| {
        let mut statement = connection.prepare(query)?;
        statement
            .query_map([], |row| {
                Ok(DrizzleRow {
                    id: row.get(0)?,
                    hash: row.get(1)?,
                    created_at: sql_value(row.get_ref(2)?),
                    name: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
    })()
    .map_err(failed("database.migrate"))?;
    let mut applied = HashSet::new();
    for row in &rows {
        let index = match_drizzle_row(row).ok_or_else(|| {
            open_error(&format!(
                "The database records a migration this version of Missal does not know (id {}).",
                row.id.map_or_else(|| "?".to_owned(), |id| id.to_string())
            ))
        })?;
        applied.insert(index);
    }
    let count = applied.len();
    if (0..count).any(|index| !applied.contains(&index)) {
        return Err(open_error(
            "The database's migration history has gaps and cannot be upgraded safely.",
        ));
    }
    Ok(count)
}

fn verify_shape(connection: &Connection, applied: usize, tables: &HashSet<String>) -> Result<()> {
    let unrecognised = || open_error("The database does not match any known Missal schema.");
    let app_tables = APP_TABLES
        .iter()
        .filter(|table| tables.contains(**table))
        .count();
    if applied == 0 {
        return if app_tables == 0 {
            Ok(())
        } else {
            Err(unrecognised())
        };
    }
    if app_tables != APP_TABLES.len() {
        return Err(unrecognised());
    }
    let fir_columns = columns(connection, "fir_records").map_err(failed("database.migrate"))?;
    let settings_columns =
        columns(connection, "app_settings").map_err(failed("database.migrate"))?;
    let case_fields = [
        "complainant",
        "writer",
        "recovery",
        "body",
        "imei_1",
        "imei_2",
        "sim_1",
        "sim_2",
    ];
    let has_case_fields = case_fields
        .iter()
        .all(|column| fir_columns.contains(*column));
    let has_no_case_fields = case_fields
        .iter()
        .all(|column| !fir_columns.contains(*column));
    let has_legacy = fir_columns.contains("nic") && fir_columns.contains("mobile");
    let has_bundle_version = settings_columns.contains("bundled_templates_version");
    let expected = match applied {
        1 => has_legacy && has_no_case_fields && !has_bundle_version,
        2 => has_legacy && has_no_case_fields && has_bundle_version,
        _ => !has_legacy && has_case_fields && has_bundle_version,
    };
    if expected {
        Ok(())
    } else {
        Err(unrecognised())
    }
}

fn apply_drizzle_baseline(connection: &mut Connection, baseline: &Baseline) -> Result<()> {
    let Baseline {
        applied,
        has_table,
        has_name,
    } = *baseline;
    if applied == DRIZZLE_MIGRATIONS.len() {
        return Ok(());
    }
    let transaction = connection
        .transaction()
        .map_err(failed("database.migrate"))?;
    (|| {
        if !has_table {
            transaction.execute_batch(
                "CREATE TABLE IF NOT EXISTS \"__drizzle_migrations\" (\n\t\t\t\tid INTEGER PRIMARY KEY,\n\t\t\t\thash text NOT NULL,\n\t\t\t\tcreated_at numeric,\n\t\t\t\tname text,\n\t\t\t\tapplied_at TEXT\n\t\t\t)",
            )?;
        }
        let record_with_name = !has_table || has_name;
        for migration in &DRIZZLE_MIGRATIONS[applied..] {
            for statement in migration.sql.split("--> statement-breakpoint") {
                if !statement.trim().is_empty() {
                    transaction.execute_batch(statement)?;
                }
            }
            if record_with_name {
                transaction.execute(
                    "INSERT INTO __drizzle_migrations (hash, created_at, name, applied_at) VALUES (?1, ?2, ?3, ?4)",
                    params![migration.hash, migration.folder_millis, migration.name, now().as_str()],
                )?;
            } else {
                transaction.execute(
                    "INSERT INTO __drizzle_migrations (hash, created_at) VALUES (?1, ?2)",
                    params![migration.hash, migration.folder_millis],
                )?;
            }
        }
        Ok(())
    })()
    .map_err(failed("database.migrate"))?;
    transaction.commit().map_err(failed("database.migrate"))
}

fn apply_data_migrations(connection: &mut Connection) -> Result<()> {
    let transaction = connection
        .transaction()
        .map_err(failed("database.migrateData"))?;
    (|| {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS \"data_migrations\" (\n  migration_id integer PRIMARY KEY NOT NULL,\n  created_at datetime NOT NULL DEFAULT current_timestamp,\n  name VARCHAR(255) NOT NULL\n)",
        )?;
        let latest: i64 = transaction
            .query_row(
                &format!("SELECT migration_id FROM {DATA_MIGRATIONS_TABLE} ORDER BY migration_id DESC LIMIT 1"),
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if latest < FOLD_PLACEHOLDERS.0 {
            transaction.execute(
                &format!("INSERT INTO {DATA_MIGRATIONS_TABLE} (migration_id, name) VALUES (?1, ?2)"),
                params![FOLD_PLACEHOLDERS.0, FOLD_PLACEHOLDERS.1],
            )?;
            fold_placeholders_into_fir_fields(&transaction)?;
        }
        Ok(())
    })()
    .map_err(failed("database.migrateData"))?;
    transaction.commit().map_err(failed("database.migrateData"))
}

pub(crate) fn fold_placeholders_into_fir_fields(
    transaction: &Transaction<'_>,
) -> rusqlite::Result<()> {
    let placeholders: Vec<(i64, String)> = transaction
        .prepare("SELECT id, label FROM placeholders")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let replacements: HashMap<i64, FieldReference> = placeholders
        .into_iter()
        .filter_map(|(id, label)| {
            fir_field_by_key(&field_lookup_key(&label))
                .map(|field| (id, FieldReference::Fir(field)))
        })
        .collect();
    if replacements.is_empty() {
        return Ok(());
    }
    let ids: Vec<i64> = replacements.keys().copied().collect();
    let marks = vec!["?"; ids.len()].join(", ");
    let values: Vec<(i64, i64, String)> = transaction
        .prepare(&format!(
            "SELECT fir_id, placeholder_id, value FROM fir_placeholder_values WHERE placeholder_id IN ({marks}) AND value <> ''"
        ))?
        .query_map(params_from_iter(&ids), |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (fir_id, placeholder_id, value) in values {
        let Some(FieldReference::Fir(field)) = replacements.get(&placeholder_id) else {
            continue;
        };
        if field.index.is_some() {
            continue;
        }
        let column = field.property.as_str();
        transaction.execute(
            &format!(
                "UPDATE fir_records SET \"{column}\" = ?1 WHERE id = ?2 AND \"{column}\" = ''"
            ),
            params![value, fir_id],
        )?;
    }
    let replace = |reference: &serde_json::Map<String, Value>| {
        if reference.get("_tag").and_then(Value::as_str) != Some("CatalogField") {
            return None;
        }
        let id = reference
            .get("id")
            .filter(|id| id.is_number())
            .and_then(json_int)?;
        replacements.get(&id).map(FieldReference::to_json)
    };
    for table in ["templates", "fir_documents"] {
        let rows: Vec<(i64, String, String)> = transaction
            .prepare(&format!(
                "SELECT id, document, field_references FROM {table}"
            ))?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, document, references) in rows {
            let parse = |text: &str| {
                serde_json::from_str::<Value>(text)
                    .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
            };
            let mut document_value = parse(&document)?;
            let mut references_value = parse(&references)?;
            let mut changed = false;
            map_references(&mut document_value, &mut |reference| {
                let replacement = replace(reference);
                changed |= replacement.is_some();
                replacement
            });
            if let Value::Array(items) = &mut references_value {
                for item in items {
                    if let Value::Object(reference) = item
                        && let Some(replacement) = replace(reference)
                    {
                        *item = replacement;
                        changed = true;
                    }
                }
            }
            if changed {
                transaction.execute(
                    &format!(
                        "UPDATE {table} SET document = ?1, field_references = ?2 WHERE id = ?3"
                    ),
                    params![document_value.to_string(), references_value.to_string(), id],
                )?;
            }
        }
    }
    transaction.execute(
        &format!("DELETE FROM placeholders WHERE id IN ({marks})"),
        params_from_iter(&ids),
    )?;
    Ok(())
}

fn schema_version(connection: &Connection, migrations: &[SchemaMigration]) -> Result<i64> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(failed("database.migrateSchema"))?;
    if version > migrations.len() as i64 {
        return Err(open_error(
            "This database was written by a newer version of Missal. Update Missal to open it.",
        ));
    }
    Ok(version)
}

pub(crate) fn apply_schema_migrations(
    connection: &mut Connection,
    migrations: &[SchemaMigration],
) -> Result<()> {
    let version = schema_version(connection, migrations)?;
    for (index, migration) in migrations.iter().enumerate().skip(version as usize) {
        let transaction = connection
            .transaction()
            .map_err(failed("database.migrateSchema"))?;
        (migration.apply)(&transaction)
            .and_then(|()| transaction.pragma_update(None, "user_version", index as i64 + 1))
            .map_err(|_| {
                open_error(&format!(
                    "The database could not be upgraded ({}).",
                    migration.name
                ))
            })?;
        transaction
            .commit()
            .map_err(failed("database.migrateSchema"))?;
    }
    Ok(())
}

pub(crate) fn upgrade_document_formats(connection: &mut Connection) -> Result<()> {
    let transaction = connection
        .transaction()
        .map_err(failed("documentFormat.upgrade"))?;
    for table in ["templates", "fir_documents"] {
        let rows: Vec<(i64, String)> = (|| {
            transaction
                .prepare(&format!(
                    "SELECT id, document FROM {table} WHERE json_extract(document, '$.version') < 2"
                ))?
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })()
        .map_err(failed("documentFormat.upgrade"))?;
        for (id, document) in rows {
            let envelope = missal_core::document::Envelope::parse(&document).map_err(|_| {
                missal_core::RepositoryError::storage(
                    "documentFormat.upgrade.decode",
                    missal_core::error::STORED_DATA_INVALID,
                )
            })?;
            transaction
                .execute(
                    &format!("UPDATE {table} SET document = ?1 WHERE id = ?2"),
                    params![envelope.to_json(), id],
                )
                .map_err(failed("documentFormat.upgrade"))?;
        }
    }
    transaction
        .commit()
        .map_err(failed("documentFormat.upgrade"))
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn embedded_migrations_hash_like_drizzle() {
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../drizzle");
        for migration in &DRIZZLE_MIGRATIONS {
            let digest = Sha256::digest(migration.sql.as_bytes());
            let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
            assert_eq!(hex, migration.hash, "{}", migration.name);
            let source = repository.join(migration.name).join("migration.sql");
            if let Ok(original) = std::fs::read_to_string(&source) {
                assert_eq!(
                    original, migration.sql,
                    "{} drifted from the drizzle folder",
                    migration.name
                );
            }
        }
    }

    #[test]
    fn drizzle_rows_are_matched_by_name_millis_or_hash() {
        let row = |name: Option<&str>, created_at: Option<Value>, hash: &str| DrizzleRow {
            id: None,
            hash: hash.to_owned(),
            created_at,
            name: name.map(str::to_owned),
        };
        assert_eq!(
            match_drizzle_row(&row(Some("20261109041241_fir_case_fields"), None, "")),
            Some(2)
        );
        assert_eq!(
            match_drizzle_row(&row(None, Some(Value::from(1_793_149_279_123_i64)), "")),
            Some(1)
        );
        assert_eq!(
            match_drizzle_row(&row(None, Some(Value::from("1793144341000")), "")),
            Some(0)
        );
        assert_eq!(
            match_drizzle_row(&row(None, None, DRIZZLE_MIGRATIONS[2].hash)),
            Some(2)
        );
        assert_eq!(
            match_drizzle_row(&row(Some("20260910191757_sharp_masked_marvel"), None, "")),
            None
        );
    }

    fn add_notes(transaction: &Transaction<'_>) -> rusqlite::Result<()> {
        transaction.execute_batch("ALTER TABLE fir_records ADD notes text DEFAULT '' NOT NULL")
    }

    fn broken(transaction: &Transaction<'_>) -> rusqlite::Result<()> {
        transaction.execute_batch("ALTER TABLE fir_records ADD notes text DEFAULT '' NOT NULL; SELECT * FROM missing_table")
    }

    #[test]
    fn schema_migrations_run_once_in_order_and_roll_back_on_failure() {
        let mut connection = Connection::open_in_memory().unwrap();
        let baseline = inspect(&connection).unwrap();
        migrate(&mut connection, &baseline).unwrap();
        let first = [SchemaMigration {
            name: "add_notes",
            apply: add_notes,
        }];
        apply_schema_migrations(&mut connection, &first).unwrap();
        apply_schema_migrations(&mut connection, &first).unwrap();
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
        assert!(
            columns(&connection, "fir_records")
                .unwrap()
                .contains("notes")
        );
        let mut fresh = Connection::open_in_memory().unwrap();
        let baseline = inspect(&fresh).unwrap();
        migrate(&mut fresh, &baseline).unwrap();
        let failing = [SchemaMigration {
            name: "broken",
            apply: broken,
        }];
        assert!(apply_schema_migrations(&mut fresh, &failing).is_err());
        let version: i64 = fresh
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 0);
        assert!(!columns(&fresh, "fir_records").unwrap().contains("notes"));
        assert!(
            apply_schema_migrations(&mut connection, &[]).is_err(),
            "a newer database is refused"
        );
    }
}
