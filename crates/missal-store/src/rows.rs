use std::collections::BTreeMap;

use missal_core::error::STORED_DATA_INVALID;
use missal_core::text::js_trim;
use missal_core::{
    AppSettings, Document, FieldMarkers, Fir, FirDocumentId, FirId, FirRecord, FirStatus,
    FirSummary, Placeholder, PlaceholderId, PlaceholderSource, PlaceholderValue, RepositoryError,
    Revision, TemplateId, Timestamp, placeholder_name,
};
use rusqlite::Row;
use serde_json::Value;

use crate::{FirDocumentRecord, FirDocumentSummary, Result, TemplateRecord, TemplateSummary};

pub(crate) fn invalid(operation: &str) -> RepositoryError {
    RepositoryError::storage(operation, STORED_DATA_INVALID)
}

pub(crate) struct Decode<'a> {
    pub operation: &'a str,
}

impl Decode<'_> {
    fn fail(&self) -> RepositoryError {
        invalid(self.operation)
    }

    fn some<T>(&self, value: Option<T>) -> Result<T> {
        value.ok_or_else(|| self.fail())
    }

    pub fn id<T>(&self, value: i64, make: fn(i64) -> Option<T>) -> Result<T> {
        self.some(make(value))
    }

    pub fn revision(&self, value: i64) -> Result<Revision> {
        self.id(value, Revision::new)
    }

    pub fn timestamp(&self, value: &str) -> Result<Timestamp> {
        self.some(Timestamp::parse(value))
    }

    pub fn required(&self, value: &str) -> Result<String> {
        let trimmed = js_trim(value);
        if trimmed.is_empty() {
            Err(self.fail())
        } else {
            Ok(trimmed.to_owned())
        }
    }

    pub fn status(&self, value: &str) -> Result<FirStatus> {
        self.some(FirStatus::parse(value))
    }

    pub fn document(&self, value: &str) -> Result<Document> {
        Document::decode(value).map_err(|_| self.fail())
    }

    pub fn strings(&self, value: &str) -> Result<Vec<String>> {
        serde_json::from_str(value).map_err(|_| self.fail())
    }
}

pub(crate) const FIR_COLUMNS: &str = "id, fir_no, complainant, date, offence, accused, witness, zimni, incident_date, arrest_date, investigation_officer, writer, recovery, body, imei_1, imei_2, sim_1, sim_2, status";

pub(crate) struct FirRow {
    id: i64,
    fir: Fir,
    lists: [String; 3],
    status: String,
}

pub(crate) fn fir_row(row: &Row<'_>) -> rusqlite::Result<FirRow> {
    let text = |index: usize| row.get::<_, String>(index);
    Ok(FirRow {
        id: row.get(0)?,
        fir: Fir {
            fir_no: text(1)?,
            complainant: text(2)?,
            date: text(3)?,
            offence: text(4)?,
            incident_date: text(8)?,
            arrest_date: text(9)?,
            investigation_officer: text(10)?,
            writer: text(11)?,
            recovery: text(12)?,
            body: text(13)?,
            imei_1: text(14)?,
            imei_2: text(15)?,
            sim_1: text(16)?,
            sim_2: text(17)?,
            ..Fir::default()
        },
        lists: [text(5)?, text(6)?, text(7)?],
        status: text(18)?,
    })
}

impl FirRow {
    pub fn decode(self) -> Result<FirRecord> {
        let decode = Decode {
            operation: "fir.decode",
        };
        let [accused, witness, zimni] = self.lists;
        let fir = Fir {
            accused: decode.strings(&accused)?,
            witness: decode.strings(&witness)?,
            zimni: decode.strings(&zimni)?,
            status: decode.status(&self.status)?,
            ..self.fir
        };
        Ok(FirRecord {
            id: decode.id(self.id, FirId::new)?,
            fir: fir.validated().map_err(|_| decode.fail())?,
        })
    }
}

pub(crate) const FIR_SUMMARY_COLUMNS: &str =
    "id, fir_no, date, offence, incident_date, arrest_date, status, accused";

pub(crate) fn fir_summary(row: &Row<'_>) -> rusqlite::Result<Result<FirSummary>> {
    let decode = Decode {
        operation: "fir.decode",
    };
    let id: i64 = row.get(0)?;
    let text = |index: usize| row.get::<_, String>(index);
    let (fir_no, date, offence, incident_date, arrest_date, status, accused) = (
        text(1)?,
        text(2)?,
        text(3)?,
        text(4)?,
        text(5)?,
        text(6)?,
        text(7)?,
    );
    Ok((|| {
        Ok(FirSummary {
            id: decode.id(id, FirId::new)?,
            fir_no: decode.required(&fir_no)?,
            date: decode.required(&date)?,
            offence: decode.required(&offence)?,
            incident_date: decode.required(&incident_date)?,
            arrest_date: js_trim(&arrest_date).to_owned(),
            status: decode.status(&status)?,
            accused: decode.strings(&accused)?,
        })
    })())
}

pub(crate) fn placeholder(row: &Row<'_>) -> rusqlite::Result<Result<Placeholder>> {
    let id: i64 = row.get(0)?;
    let label: String = row.get(1)?;
    let source: String = row.get(2)?;
    Ok((|| {
        let decode = Decode {
            operation: "placeholder.decode",
        };
        Ok(Placeholder {
            id: decode.id(id, PlaceholderId::new)?,
            label: placeholder_name(&label).map_err(|_| decode.fail())?,
            source: decode.some(
                serde_json::from_str::<Value>(&source)
                    .ok()
                    .as_ref()
                    .and_then(PlaceholderSource::from_json),
            )?,
        })
    })())
}

pub(crate) fn placeholder_value(row: &Row<'_>) -> rusqlite::Result<Result<PlaceholderValue>> {
    let fir_id: i64 = row.get(0)?;
    let placeholder_id: i64 = row.get(1)?;
    let value: String = row.get(2)?;
    let updated_at: String = row.get(3)?;
    Ok((|| {
        let decode = Decode {
            operation: "firPlaceholderValue.decode",
        };
        Ok(PlaceholderValue {
            fir_id: decode.id(fir_id, FirId::new)?,
            placeholder_id: decode.id(placeholder_id, PlaceholderId::new)?,
            value,
            updated_at: decode.timestamp(&updated_at)?,
        })
    })())
}

pub(crate) const TEMPLATE_SUMMARY_COLUMNS: &str =
    "id, name, revision, preview_text, field_count, created_at, updated_at";

pub(crate) fn template_summary(row: &Row<'_>) -> rusqlite::Result<Result<TemplateSummary>> {
    let id: i64 = row.get(0)?;
    let name: String = row.get(1)?;
    let revision: i64 = row.get(2)?;
    let preview_text: String = row.get(3)?;
    let field_count: i64 = row.get(4)?;
    let created_at: String = row.get(5)?;
    let updated_at: String = row.get(6)?;
    Ok((|| {
        let decode = Decode {
            operation: "template.summary.decode",
        };
        Ok(TemplateSummary {
            id: decode.id(id, TemplateId::new)?,
            name: decode.required(&name)?,
            revision: decode.revision(revision)?,
            preview_text,
            field_count,
            created_at: decode.timestamp(&created_at)?,
            updated_at: decode.timestamp(&updated_at)?,
        })
    })())
}

pub(crate) const TEMPLATE_RECORD_COLUMNS: &str =
    "id, name, document, revision, created_at, updated_at";

pub(crate) fn template_record(row: &Row<'_>) -> rusqlite::Result<Result<TemplateRecord>> {
    let id: i64 = row.get(0)?;
    let name: String = row.get(1)?;
    let document: String = row.get(2)?;
    let revision: i64 = row.get(3)?;
    let created_at: String = row.get(4)?;
    let updated_at: String = row.get(5)?;
    Ok((|| {
        let decode = Decode {
            operation: "template.decode",
        };
        Ok(TemplateRecord {
            id: decode.id(id, TemplateId::new)?,
            name: decode.required(&name)?,
            document: decode.document(&document)?,
            revision: decode.revision(revision)?,
            created_at: decode.timestamp(&created_at)?,
            updated_at: decode.timestamp(&updated_at)?,
        })
    })())
}

pub(crate) const FIR_DOCUMENT_SUMMARY_COLUMNS: &str = "id, fir_id, template_id, source_template_revision, title, revision, position, preview_text, field_count, created_at, updated_at";

pub(crate) fn fir_document_summary(row: &Row<'_>) -> rusqlite::Result<Result<FirDocumentSummary>> {
    let ids: (i64, i64, i64, i64) = (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
    let title: String = row.get(4)?;
    let revision: i64 = row.get(5)?;
    let position: i64 = row.get(6)?;
    let preview_text: String = row.get(7)?;
    let field_count: i64 = row.get(8)?;
    let created_at: String = row.get(9)?;
    let updated_at: String = row.get(10)?;
    Ok((|| {
        let decode = Decode {
            operation: "firDocument.summary.decode",
        };
        if position < 0 {
            return Err(decode.fail());
        }
        Ok(FirDocumentSummary {
            id: decode.id(ids.0, FirDocumentId::new)?,
            fir_id: decode.id(ids.1, FirId::new)?,
            template_id: decode.id(ids.2, TemplateId::new)?,
            source_template_revision: decode.revision(ids.3)?,
            title: decode.required(&title)?,
            revision: decode.revision(revision)?,
            position,
            preview_text,
            field_count,
            created_at: decode.timestamp(&created_at)?,
            updated_at: decode.timestamp(&updated_at)?,
        })
    })())
}

pub(crate) const FIR_DOCUMENT_RECORD_COLUMNS: &str = "id, fir_id, template_id, source_template_revision, title, document, revision, position, created_at, updated_at";

pub(crate) fn fir_document_record(row: &Row<'_>) -> rusqlite::Result<Result<FirDocumentRecord>> {
    let ids: (i64, i64, i64, i64) = (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
    let title: String = row.get(4)?;
    let document: String = row.get(5)?;
    let revision: i64 = row.get(6)?;
    let position: i64 = row.get(7)?;
    let created_at: String = row.get(8)?;
    let updated_at: String = row.get(9)?;
    Ok((|| {
        let decode = Decode {
            operation: "firDocument.decode",
        };
        if position < 0 {
            return Err(decode.fail());
        }
        Ok(FirDocumentRecord {
            id: decode.id(ids.0, FirDocumentId::new)?,
            fir_id: decode.id(ids.1, FirId::new)?,
            template_id: decode.id(ids.2, TemplateId::new)?,
            source_template_revision: decode.revision(ids.3)?,
            title: decode.required(&title)?,
            document: decode.document(&document)?,
            revision: decode.revision(revision)?,
            position,
            created_at: decode.timestamp(&created_at)?,
            updated_at: decode.timestamp(&updated_at)?,
        })
    })())
}

pub(crate) fn settings(shared: &str, markers: &str) -> Result<AppSettings> {
    let decode = Decode {
        operation: "settings.decode",
    };
    let shared: BTreeMap<String, String> =
        serde_json::from_str(shared).map_err(|_| decode.fail())?;
    let markers: Value = serde_json::from_str(markers).map_err(|_| decode.fail())?;
    let marker = |key: &str| decode.some(markers.get(key).and_then(Value::as_str));
    Ok(AppSettings {
        shared_placeholders: shared,
        field_markers: FieldMarkers::new(marker("open")?, marker("close")?)
            .map_err(|_| decode.fail())?,
    })
}

pub(crate) type RowMap<T> = fn(&Row<'_>) -> rusqlite::Result<Result<T>>;

pub(crate) fn all<T>(
    connection: &rusqlite::Connection,
    sql: &str,
    params: impl rusqlite::Params,
    map: RowMap<T>,
    operation: &'static str,
) -> Result<Vec<T>> {
    let rows = (|| {
        let mut statement = connection.prepare_cached(sql)?;
        statement
            .query_map(params, map)?
            .collect::<rusqlite::Result<Vec<_>>>()
    })()
    .map_err(crate::failed(operation))?;
    rows.into_iter().collect()
}

pub(crate) fn one<T>(
    connection: &rusqlite::Connection,
    sql: &str,
    params: impl rusqlite::Params,
    map: RowMap<T>,
    operation: &'static str,
) -> Result<Option<T>> {
    Ok(all(connection, sql, params, map, operation)?
        .into_iter()
        .next())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Constraint {
    Unique,
    Other,
}

pub(crate) fn constraint(error: &rusqlite::Error) -> Option<Constraint> {
    let rusqlite::Error::SqliteFailure(failure, _) = error else {
        return None;
    };
    if failure.code != rusqlite::ErrorCode::ConstraintViolation {
        return None;
    }
    Some(
        if matches!(
            failure.extended_code,
            rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE | rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
        ) {
            Constraint::Unique
        } else {
            Constraint::Other
        },
    )
}
