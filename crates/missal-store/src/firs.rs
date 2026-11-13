use std::collections::BTreeSet;

use missal_core::{
    Entity, Fir, FirId, FirRecord, FirSummary, FirValueContext, FirValues, PlaceholderId,
    PlaceholderValue, RepositoryError,
};
use rusqlite::{Connection, ToSql, Transaction, params};

use crate::documents::summaries_for;
use crate::rows::{self, FIR_COLUMNS, FIR_SUMMARY_COLUMNS, constraint};
use crate::settings::read_settings;
use crate::{FirWorkspace, Result, Store, failed, now};

const WRITE_COLUMNS: &str = "fir_no, complainant, date, offence, accused, witness, zimni, incident_date, arrest_date, investigation_officer, writer, recovery, body, imei_1, imei_2, sim_1, sim_2, status";

fn validated(fir: &Fir) -> Result<Fir> {
    fir.validated().map_err(|reason| RepositoryError::Invalid {
        entity: Entity::Fir,
        reason,
    })
}

fn list_json(items: &[String]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".to_owned())
}

fn write_values(fir: &Fir) -> [Box<dyn ToSql + '_>; 18] {
    [
        Box::new(&fir.fir_no),
        Box::new(&fir.complainant),
        Box::new(&fir.date),
        Box::new(&fir.offence),
        Box::new(list_json(&fir.accused)),
        Box::new(list_json(&fir.witness)),
        Box::new(list_json(&fir.zimni)),
        Box::new(&fir.incident_date),
        Box::new(&fir.arrest_date),
        Box::new(&fir.investigation_officer),
        Box::new(&fir.writer),
        Box::new(&fir.recovery),
        Box::new(&fir.body),
        Box::new(&fir.imei_1),
        Box::new(&fir.imei_2),
        Box::new(&fir.sim_1),
        Box::new(&fir.sim_2),
        Box::new(fir.status.as_str()),
    ]
}

fn fir_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<FirRecord>> {
    rows::fir_row(row).map(rows::FirRow::decode)
}

pub(crate) fn find_fir(
    connection: &Connection,
    id: FirId,
    operation: &'static str,
) -> Result<FirRecord> {
    rows::one(
        connection,
        &format!("SELECT {FIR_COLUMNS} FROM fir_records WHERE id = ?1"),
        [id.get()],
        fir_record,
        operation,
    )?
    .ok_or_else(|| RepositoryError::not_found(Entity::Fir, id))
}

fn overrides(
    connection: &Connection,
    fir_id: FirId,
    operation: &'static str,
) -> Result<Vec<PlaceholderValue>> {
    rows::all(
        connection,
        "SELECT fir_id, placeholder_id, value, updated_at FROM fir_placeholder_values WHERE fir_id = ?1",
        [fir_id.get()],
        rows::placeholder_value,
        operation,
    )
}

fn fir_values(connection: &Connection, id: FirId, operation: &'static str) -> Result<FirValues> {
    Ok(FirValues {
        fir: find_fir(connection, id, operation)?,
        overrides: overrides(connection, id, operation)?,
        shared_settings: read_settings(connection)?.shared_placeholders,
    })
}

fn upsert_value(
    connection: &Connection,
    fir_id: FirId,
    placeholder_id: PlaceholderId,
    value: &str,
) -> Result<PlaceholderValue> {
    let operation = "firPlaceholderValue.upsert";
    let timestamp = now();
    connection
        .query_row(
            "INSERT INTO fir_placeholder_values (fir_id, placeholder_id, value, updated_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (fir_id, placeholder_id) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at RETURNING fir_id, placeholder_id, value, updated_at",
            params![fir_id.get(), placeholder_id.get(), value, timestamp.as_str()],
            rows::placeholder_value,
        )
        .map_err(|error| {
            if constraint(&error).is_none() {
                return failed(operation)(error);
            }
            match find_fir(connection, fir_id, operation) {
                Ok(_) => RepositoryError::not_found(Entity::Placeholder, placeholder_id),
                Err(error) => error,
            }
        })?
}

fn read<T>(
    store: &Store,
    operation: &'static str,
    read: impl FnOnce(&Transaction<'_>) -> Result<T>,
) -> Result<T> {
    let transaction = store
        .connection
        .unchecked_transaction()
        .map_err(failed(operation))?;
    read(&transaction)
}

impl Store {
    pub fn firs(&self) -> Result<Vec<FirSummary>> {
        rows::all(
            &self.connection,
            &format!("SELECT {FIR_SUMMARY_COLUMNS} FROM fir_records ORDER BY id DESC"),
            [],
            rows::fir_summary,
            "fir.list",
        )
    }

    pub fn fir(&self, id: FirId) -> Result<FirRecord> {
        find_fir(&self.connection, id, "fir.get")
    }

    pub fn create_fir(&mut self, fir: &Fir) -> Result<FirRecord> {
        let fir = validated(fir)?;
        let values = write_values(&fir);
        rows::one(
            &self.connection,
            &format!(
                "INSERT INTO fir_records ({WRITE_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18) RETURNING {FIR_COLUMNS}"
            ),
            rusqlite::params_from_iter(values.iter()),
            fir_record,
            "fir.create",
        )?
        .ok_or_else(|| RepositoryError::storage("fir.create", missal_core::error::STORED_DATA_NOT_WRITTEN))
    }

    pub fn update_fir(&mut self, record: &FirRecord) -> Result<FirRecord> {
        let fir = validated(&record.fir)?;
        let mut values: Vec<Box<dyn ToSql + '_>> = write_values(&fir).into_iter().collect();
        values.push(Box::new(record.id.get()));
        let assignments = WRITE_COLUMNS
            .split(", ")
            .enumerate()
            .map(|(index, column)| format!("{column} = ?{}", index + 1))
            .collect::<Vec<_>>()
            .join(", ");
        rows::one(
            &self.connection,
            &format!("UPDATE fir_records SET {assignments} WHERE id = ?19 RETURNING {FIR_COLUMNS}"),
            rusqlite::params_from_iter(values.iter()),
            fir_record,
            "fir.update",
        )?
        .ok_or_else(|| RepositoryError::not_found(Entity::Fir, record.id))
    }

    pub fn remove_fir(&mut self, id: FirId) -> Result<()> {
        self.remove_firs(&[id])
    }

    pub fn remove_firs(&mut self, ids: &[FirId]) -> Result<()> {
        let operation = "fir.remove";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        let mut seen = BTreeSet::new();
        for id in ids.iter().filter(|id| seen.insert(**id)) {
            let removed = transaction
                .execute("DELETE FROM fir_records WHERE id = ?1", [id.get()])
                .map_err(failed(operation))?;
            if removed == 0 {
                return Err(RepositoryError::not_found(Entity::Fir, id));
            }
        }
        transaction.commit().map_err(failed(operation))
    }

    pub fn fir_values(&self, id: FirId) -> Result<FirValues> {
        read(self, "fir.values", |transaction| {
            fir_values(transaction, id, "fir.values")
        })
    }

    pub fn fir_value_context(&self, id: FirId) -> Result<FirValueContext> {
        read(self, "fir.valueContext", |transaction| {
            Ok(FirValueContext {
                values: fir_values(transaction, id, "fir.valueContext")?,
                catalog: crate::placeholders::list(transaction, "fir.valueContext")?,
            })
        })
    }

    pub fn fir_workspace(&self, id: FirId) -> Result<FirWorkspace> {
        read(self, "fir.workspace", |transaction| {
            Ok(FirWorkspace {
                values: fir_values(transaction, id, "fir.workspace")?,
                documents: summaries_for(transaction, id, "fir.workspace")?,
            })
        })
    }

    pub fn placeholder_values(&self, fir_id: FirId) -> Result<Vec<PlaceholderValue>> {
        overrides(&self.connection, fir_id, "firPlaceholderValue.listForFir")
    }

    pub fn set_placeholder_value(
        &mut self,
        fir_id: FirId,
        placeholder_id: PlaceholderId,
        value: &str,
    ) -> Result<PlaceholderValue> {
        upsert_value(&self.connection, fir_id, placeholder_id, value)
    }

    pub fn set_placeholder_values(
        &mut self,
        fir_id: FirId,
        values: &[(PlaceholderId, String)],
    ) -> Result<Vec<PlaceholderValue>> {
        let operation = "firPlaceholderValue.upsert";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        for (placeholder_id, value) in values {
            upsert_value(&transaction, fir_id, *placeholder_id, value)?;
        }
        let saved = overrides(&transaction, fir_id, operation)?;
        transaction.commit().map_err(failed(operation))?;
        Ok(saved)
    }

    pub fn remove_placeholder_value(
        &mut self,
        fir_id: FirId,
        placeholder_id: PlaceholderId,
    ) -> Result<()> {
        let removed = self
            .connection
            .execute(
                "DELETE FROM fir_placeholder_values WHERE fir_id = ?1 AND placeholder_id = ?2",
                [fir_id.get(), placeholder_id.get()],
            )
            .map_err(failed("firPlaceholderValue.remove"))?;
        if removed == 0 {
            return Err(RepositoryError::not_found(
                Entity::FirPlaceholderValue,
                format!("{fir_id}:{placeholder_id}"),
            ));
        }
        Ok(())
    }
}
