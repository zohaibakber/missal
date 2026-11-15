use missal_core::{AppSettings, FieldMarkers};
use rusqlite::params;

use crate::placeholders::shared_values;
use crate::{Result, Store, failed, invalid};

impl Store {
    pub fn settings(&self) -> Result<AppSettings> {
        let operation = "settings.get";
        let (open, close): (String, String) = self
            .connection
            .query_row(
                "SELECT open_marker, close_marker FROM settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(failed(operation))?;
        Ok(AppSettings {
            shared_values: shared_values(&self.connection, operation)?,
            field_markers: FieldMarkers::new(&open, &close).map_err(|_| invalid(operation))?,
        })
    }

    pub fn save_field_markers(&mut self, markers: &FieldMarkers) -> Result<AppSettings> {
        self.connection
            .execute(
                "UPDATE settings SET open_marker = ?1, close_marker = ?2 WHERE id = 1",
                params![markers.open(), markers.close()],
            )
            .map_err(failed("settings.saveFieldMarkers"))?;
        self.settings()
    }
}
