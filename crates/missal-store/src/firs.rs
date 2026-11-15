use std::collections::BTreeSet;

use missal_core::{
    Entity, Fir, FirId, FirRecord, FirStatus, FirSummary, FirValueContext, FirValues,
    PlaceholderId, PlaceholderValue, RepositoryError,
};
use rusqlite::{Connection, Row, ToSql, params};

use crate::documents::summaries_for;
use crate::placeholders::{self, shared_values};
use crate::{FirWorkspace, Result, Store, failed, id, invalid, is_constraint};

const LIST_SEPARATOR: char = '\u{1F}';

const COLUMNS: &str = "id, fir_no, date, offence, incident_date, arrest_date, status, accused, complainant, witness, zimni, investigation_officer, writer, recovery, imei_1, imei_2, sim_1, sim_2, body";
const WRITE_COLUMNS: &str = "fir_no, date, offence, incident_date, arrest_date, status, accused, complainant, witness, zimni, investigation_officer, writer, recovery, imei_1, imei_2, sim_1, sim_2, body";
const SUMMARY_COLUMNS: &str =
    "id, fir_no, date, offence, incident_date, arrest_date, status, accused";

fn join(items: &[String]) -> String {
    let mut joined = String::with_capacity(items.iter().map(|item| item.len() + 1).sum());
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            joined.push(LIST_SEPARATOR);
        }
        if item.contains(LIST_SEPARATOR) {
            joined.push_str(&item.replace(LIST_SEPARATOR, " "));
        } else {
            joined.push_str(item);
        }
    }
    joined
}

fn split(joined: &str) -> Vec<String> {
    if joined.is_empty() {
        Vec::new()
    } else {
        joined.split(LIST_SEPARATOR).map(str::to_owned).collect()
    }
}

fn status_code(status: FirStatus) -> i64 {
    FirStatus::ALL
        .iter()
        .position(|candidate| *candidate == status)
        .unwrap_or(0) as i64
}

fn status(code: i64) -> Result<FirStatus> {
    usize::try_from(code)
        .ok()
        .and_then(|index| FirStatus::ALL.get(index).copied())
        .ok_or_else(|| invalid("fir.decode"))
}

fn validated(fir: &Fir) -> Result<Fir> {
    fir.validated().map_err(|reason| RepositoryError::Invalid {
        entity: Entity::Fir,
        reason,
    })
}

fn write_values(fir: &Fir) -> [Box<dyn ToSql + '_>; 18] {
    [
        Box::new(&fir.fir_no),
        Box::new(&fir.date),
        Box::new(&fir.offence),
        Box::new(&fir.incident_date),
        Box::new(&fir.arrest_date),
        Box::new(status_code(fir.status)),
        Box::new(join(&fir.accused)),
        Box::new(&fir.complainant),
        Box::new(join(&fir.witness)),
        Box::new(join(&fir.zimni)),
        Box::new(&fir.investigation_officer),
        Box::new(&fir.writer),
        Box::new(&fir.recovery),
        Box::new(&fir.imei_1),
        Box::new(&fir.imei_2),
        Box::new(&fir.sim_1),
        Box::new(&fir.sim_2),
        Box::new(&fir.body),
    ]
}

struct RawFir {
    id: i64,
    status: i64,
    fir: Fir,
}

fn raw_fir(row: &Row<'_>) -> rusqlite::Result<RawFir> {
    let text = |index: usize| row.get::<_, String>(index);
    let list = |index: usize| -> rusqlite::Result<Vec<String>> {
        Ok(split(row.get_ref(index)?.as_str()?))
    };
    Ok(RawFir {
        id: row.get(0)?,
        status: row.get(6)?,
        fir: Fir {
            fir_no: text(1)?,
            date: text(2)?,
            offence: text(3)?,
            incident_date: text(4)?,
            arrest_date: text(5)?,
            accused: list(7)?,
            complainant: text(8)?,
            witness: list(9)?,
            zimni: list(10)?,
            investigation_officer: text(11)?,
            writer: text(12)?,
            recovery: text(13)?,
            imei_1: text(14)?,
            imei_2: text(15)?,
            sim_1: text(16)?,
            sim_2: text(17)?,
            body: text(18)?,
            status: FirStatus::Open,
        },
    })
}

fn record(raw: RawFir) -> Result<FirRecord> {
    Ok(FirRecord {
        id: id(raw.id, FirId::new, "fir.decode")?,
        fir: Fir {
            status: status(raw.status)?,
            ..raw.fir
        },
    })
}

pub(crate) fn find_fir(
    connection: &Connection,
    fir_id: FirId,
    operation: &'static str,
) -> Result<FirRecord> {
    let raw = connection
        .prepare_cached(&format!("SELECT {COLUMNS} FROM firs WHERE id = ?1"))
        .and_then(|mut statement| statement.query_row([fir_id.get()], raw_fir))
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => RepositoryError::not_found(Entity::Fir, fir_id),
            other => failed(operation)(other),
        })?;
    record(raw)
}

fn overrides(
    connection: &Connection,
    fir_id: FirId,
    operation: &'static str,
) -> Result<Vec<PlaceholderValue>> {
    let rows = connection
        .prepare_cached("SELECT placeholder_id, value FROM fir_values WHERE fir_id = ?1")
        .and_then(|mut statement| {
            statement
                .query_map([fir_id.get()], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(failed(operation))?;
    rows.into_iter()
        .map(|(placeholder_id, value)| {
            Ok(PlaceholderValue {
                fir_id,
                placeholder_id: id(placeholder_id, PlaceholderId::new, operation)?,
                value,
            })
        })
        .collect()
}

fn fir_values(
    connection: &Connection,
    fir_id: FirId,
    operation: &'static str,
) -> Result<FirValues> {
    Ok(FirValues {
        fir: find_fir(connection, fir_id, operation)?,
        overrides: overrides(connection, fir_id, operation)?,
        shared_values: shared_values(connection, operation)?,
    })
}

fn insert(connection: &Connection, fir: &Fir) -> Result<FirRecord> {
    let fir = validated(fir)?;
    let marks = (1..=18)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let raw = connection
        .prepare_cached(&format!(
            "INSERT INTO firs ({WRITE_COLUMNS}) VALUES ({marks}) RETURNING {COLUMNS}"
        ))
        .and_then(|mut statement| {
            statement.query_row(
                rusqlite::params_from_iter(write_values(&fir).iter()),
                raw_fir,
            )
        })
        .map_err(failed("fir.create"))?;
    record(raw)
}

fn upsert_value(
    connection: &Connection,
    fir_id: FirId,
    placeholder_id: PlaceholderId,
    value: &str,
) -> Result<()> {
    let operation = "firValue.save";
    let result = if value.is_empty() {
        connection.execute(
            "DELETE FROM fir_values WHERE fir_id = ?1 AND placeholder_id = ?2",
            params![fir_id.get(), placeholder_id.get()],
        )
    } else {
        connection.execute(
            "INSERT INTO fir_values (fir_id, placeholder_id, value) VALUES (?1, ?2, ?3) ON CONFLICT (fir_id, placeholder_id) DO UPDATE SET value = excluded.value",
            params![fir_id.get(), placeholder_id.get(), value],
        )
    };
    match result {
        Ok(_) => Ok(()),
        Err(error) if is_constraint(&error) => match find_fir(connection, fir_id, operation) {
            Ok(_) => Err(RepositoryError::not_found(
                Entity::Placeholder,
                placeholder_id,
            )),
            Err(error) => Err(error),
        },
        Err(error) => Err(failed(operation)(error)),
    }
}

impl Store {
    pub fn firs(&self) -> Result<Vec<FirSummary>> {
        let operation = "fir.list";
        let mut statement = self
            .connection
            .prepare_cached(&format!(
                "SELECT {SUMMARY_COLUMNS} FROM firs ORDER BY id DESC"
            ))
            .map_err(failed(operation))?;
        let mut rows = statement.query([]).map_err(failed(operation))?;
        let mut firs = Vec::new();
        while let Some(row) = rows.next().map_err(failed(operation))? {
            let text = |index: usize| row.get::<_, String>(index).map_err(failed(operation));
            firs.push(FirSummary {
                id: id(
                    row.get(0).map_err(failed(operation))?,
                    FirId::new,
                    operation,
                )?,
                fir_no: text(1)?,
                date: text(2)?,
                offence: text(3)?,
                incident_date: text(4)?,
                arrest_date: text(5)?,
                status: status(row.get(6).map_err(failed(operation))?)?,
                accused: split(
                    row.get_ref(7)
                        .and_then(|value| Ok(value.as_str()?))
                        .map_err(failed(operation))?,
                ),
            });
        }
        Ok(firs)
    }

    pub fn fir(&self, fir_id: FirId) -> Result<FirRecord> {
        find_fir(&self.connection, fir_id, "fir.get")
    }

    pub fn create_fir(&mut self, fir: &Fir) -> Result<FirRecord> {
        insert(&self.connection, fir)
    }

    pub fn create_firs(&mut self, firs: &[Fir]) -> Result<Vec<FirId>> {
        let transaction = self
            .connection
            .transaction()
            .map_err(failed("fir.create"))?;
        let ids = firs
            .iter()
            .map(|fir| insert(&transaction, fir).map(|record| record.id))
            .collect::<Result<Vec<_>>>()?;
        transaction.commit().map_err(failed("fir.create"))?;
        Ok(ids)
    }

    pub fn update_fir(&mut self, record: &FirRecord) -> Result<FirRecord> {
        let fir = validated(&record.fir)?;
        let assignments = WRITE_COLUMNS
            .split(", ")
            .enumerate()
            .map(|(index, column)| format!("{column} = ?{}", index + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let mut values: Vec<Box<dyn ToSql + '_>> = write_values(&fir).into_iter().collect();
        values.push(Box::new(record.id.get()));
        let raw = self
            .connection
            .prepare_cached(&format!(
                "UPDATE firs SET {assignments} WHERE id = ?19 RETURNING {COLUMNS}"
            ))
            .and_then(|mut statement| {
                statement.query_row(rusqlite::params_from_iter(values.iter()), raw_fir)
            })
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => {
                    RepositoryError::not_found(Entity::Fir, record.id)
                }
                other => failed("fir.update")(other),
            })?;
        crate::firs::record(raw)
    }

    pub fn remove_fir(&mut self, fir_id: FirId) -> Result<()> {
        self.remove_firs(&[fir_id])
    }

    pub fn remove_firs(&mut self, ids: &[FirId]) -> Result<()> {
        let operation = "fir.remove";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        let mut seen = BTreeSet::new();
        for fir_id in ids.iter().filter(|fir_id| seen.insert(**fir_id)) {
            let removed = transaction
                .execute("DELETE FROM firs WHERE id = ?1", [fir_id.get()])
                .map_err(failed(operation))?;
            if removed == 0 {
                return Err(RepositoryError::not_found(Entity::Fir, fir_id));
            }
        }
        transaction.commit().map_err(failed(operation))
    }

    pub fn fir_values(&self, fir_id: FirId) -> Result<FirValues> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(failed("fir.values"))?;
        fir_values(&transaction, fir_id, "fir.values")
    }

    pub fn fir_value_context(&self, fir_id: FirId) -> Result<FirValueContext> {
        let operation = "fir.valueContext";
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(failed(operation))?;
        Ok(FirValueContext {
            values: fir_values(&transaction, fir_id, operation)?,
            catalog: placeholders::list(&transaction, operation)?,
        })
    }

    pub fn fir_workspace(&self, fir_id: FirId) -> Result<FirWorkspace> {
        let operation = "fir.workspace";
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(failed(operation))?;
        Ok(FirWorkspace {
            values: fir_values(&transaction, fir_id, operation)?,
            documents: summaries_for(&transaction, fir_id, operation)?,
        })
    }

    pub fn placeholder_values(&self, fir_id: FirId) -> Result<Vec<PlaceholderValue>> {
        overrides(&self.connection, fir_id, "firValue.list")
    }

    pub fn set_placeholder_values(
        &mut self,
        fir_id: FirId,
        values: &[(PlaceholderId, String)],
    ) -> Result<Vec<PlaceholderValue>> {
        let operation = "firValue.save";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        for (placeholder_id, value) in values {
            upsert_value(&transaction, fir_id, *placeholder_id, value.trim())?;
        }
        let saved = overrides(&transaction, fir_id, operation)?;
        transaction.commit().map_err(failed(operation))?;
        Ok(saved)
    }
}
