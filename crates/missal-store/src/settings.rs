use std::collections::BTreeMap;

use missal_core::{AppSettings, FieldMarkers};
use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

use crate::{Result, Store, failed, rows};

pub(crate) fn read_settings(connection: &Connection) -> Result<AppSettings> {
    let row: Option<(String, String)> = connection
        .query_row(
            "SELECT shared_placeholders, field_markers FROM app_settings",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(failed("settings.get"))?;
    match row {
        None => Ok(AppSettings::default()),
        Some((shared, markers)) => rows::settings(&shared, &markers),
    }
}

fn upsert(
    connection: &Connection,
    column: &str,
    value: String,
    operation: &'static str,
) -> Result<AppSettings> {
    let (shared, markers): (String, String) = connection
        .query_row(
            &format!(
                "INSERT INTO app_settings ({column}) VALUES (?1) ON CONFLICT (id) DO UPDATE SET {column} = excluded.{column} RETURNING shared_placeholders, field_markers"
            ),
            [value],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(failed(operation))?;
    rows::settings(&shared, &markers)
}

pub(crate) fn write_shared(
    connection: &Connection,
    values: &BTreeMap<String, String>,
    operation: &'static str,
) -> Result<AppSettings> {
    upsert(
        connection,
        "shared_placeholders",
        serde_json::to_string(values).unwrap_or_else(|_| "{}".to_owned()),
        operation,
    )
}

impl Store {
    pub fn settings(&self) -> Result<AppSettings> {
        read_settings(&self.connection)
    }

    pub fn save_shared_placeholders(
        &mut self,
        values: &BTreeMap<String, String>,
    ) -> Result<AppSettings> {
        write_shared(&self.connection, values, "settings.save")
    }

    pub fn save_field_markers(&mut self, markers: &FieldMarkers) -> Result<AppSettings> {
        upsert(
            &self.connection,
            "field_markers",
            json!({ "open": markers.open(), "close": markers.close() }).to_string(),
            "settings.saveFieldMarkers",
        )
    }
}
