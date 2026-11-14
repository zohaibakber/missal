use std::collections::{HashMap, HashSet};

use missal_core::{
    Document, Entity, FirDocumentId, FirId, RepositoryError, Revision, TemplateId, Timestamp,
};
use rusqlite::{Connection, Row, params};

use crate::firs::find_fir;
use crate::templates::revision_conflict;
use crate::{
    FirDocumentRecord, FirDocumentSummary, Result, SaveAck, Store, failed, id, pack, timestamp,
    unpack,
};

const SUMMARY_COLUMNS: &str = "id, fir_id, template_id, source_revision, title, revision, position, preview, field_count, created_at, updated_at";

struct RawSummary {
    ids: [i64; 4],
    title: String,
    revision: i64,
    position: i64,
    preview_text: String,
    field_count: i64,
    created_at: Timestamp,
    updated_at: Timestamp,
}

fn raw_summary(row: &Row<'_>) -> rusqlite::Result<RawSummary> {
    Ok(RawSummary {
        ids: [row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?],
        title: row.get(4)?,
        revision: row.get(5)?,
        position: row.get(6)?,
        preview_text: row.get(7)?,
        field_count: row.get(8)?,
        created_at: timestamp(row, 9)?,
        updated_at: timestamp(row, 10)?,
    })
}

fn summary(raw: RawSummary, operation: &str) -> Result<FirDocumentSummary> {
    let [document, fir, template, source] = raw.ids;
    Ok(FirDocumentSummary {
        id: id(document, FirDocumentId::new, operation)?,
        fir_id: id(fir, FirId::new, operation)?,
        template_id: id(template, TemplateId::new, operation)?,
        source_template_revision: id(source, Revision::new, operation)?,
        title: raw.title,
        revision: id(raw.revision, Revision::new, operation)?,
        position: raw.position,
        preview_text: raw.preview_text,
        field_count: raw.field_count,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    })
}

pub(crate) fn summaries_for(
    connection: &Connection,
    fir_id: FirId,
    operation: &'static str,
) -> Result<Vec<FirDocumentSummary>> {
    let rows = connection
        .prepare_cached(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM documents WHERE fir_id = ?1 ORDER BY position, id"
        ))
        .and_then(|mut statement| {
            statement
                .query_map([fir_id.get()], raw_summary)?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(failed(operation))?;
    rows.into_iter()
        .map(|raw| summary(raw, operation))
        .collect()
}

fn record(summary: FirDocumentSummary, document: Document) -> FirDocumentRecord {
    FirDocumentRecord {
        id: summary.id,
        fir_id: summary.fir_id,
        template_id: summary.template_id,
        source_template_revision: summary.source_template_revision,
        title: summary.title,
        document,
        revision: summary.revision,
        position: summary.position,
        created_at: summary.created_at,
        updated_at: summary.updated_at,
    }
}

impl Store {
    pub fn fir_documents(&self, fir_id: FirId) -> Result<Vec<FirDocumentSummary>> {
        summaries_for(&self.connection, fir_id, "firDocument.listForFir")
    }

    pub fn fir_document(&self, document: FirDocumentId) -> Result<FirDocumentRecord> {
        self.fir_documents_by_id(&[document])
            .map(|mut found| found.remove(0))
    }

    pub fn fir_documents_by_id(&self, ids: &[FirDocumentId]) -> Result<Vec<FirDocumentRecord>> {
        let operation = "firDocument.get";
        let mut statement = self
            .connection
            .prepare_cached(&format!(
                "SELECT {SUMMARY_COLUMNS}, b.body FROM documents d JOIN document_bodies b ON b.document_id = d.id WHERE d.id = ?1"
            ))
            .map_err(failed(operation))?;
        let mut loaded: HashMap<FirDocumentId, FirDocumentRecord> = HashMap::new();
        ids.iter()
            .map(|document| {
                if let Some(found) = loaded.get(document) {
                    return Ok(found.clone());
                }
                let (raw, bytes) = statement
                    .query_row([document.get()], |row| {
                        Ok((raw_summary(row)?, row.get::<_, Vec<u8>>(11)?))
                    })
                    .map_err(|error| match error {
                        rusqlite::Error::QueryReturnedNoRows => {
                            RepositoryError::not_found(Entity::FirDocument, document)
                        }
                        other => failed(operation)(other),
                    })?;
                let found = record(summary(raw, operation)?, unpack(&bytes, operation)?);
                loaded.insert(*document, found.clone());
                Ok(found)
            })
            .collect()
    }

    pub fn add_templates_to_fir(
        &mut self,
        fir_id: FirId,
        template_ids: &[TemplateId],
    ) -> Result<Vec<FirDocumentSummary>> {
        let operation = "firDocument.addTemplates";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        find_fir(&transaction, fir_id, operation)?;
        let existing = summaries_for(&transaction, fir_id, operation)?;
        let mut seen: HashSet<TemplateId> = existing.iter().map(|d| d.template_id).collect();
        let start = existing.iter().map(|d| d.position).max().unwrap_or(-1) + 1;
        let now = Timestamp::now().unix_millis();
        let added = template_ids.iter().copied().filter(|t| seen.insert(*t));
        for (position, template) in (start..).zip(added) {
            let document_id: i64 = transaction
                .prepare_cached(
                    "INSERT INTO documents (fir_id, template_id, source_revision, title, revision, position, field_count, created_at, updated_at, preview) SELECT ?1, id, revision, name, 1, ?2, field_count, ?3, ?3, preview FROM templates WHERE id = ?4 RETURNING id",
                )
                .and_then(|mut statement| {
                    statement.query_row(
                        params![fir_id.get(), position, now, template.get()],
                        |row| row.get(0),
                    )
                })
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => {
                        RepositoryError::not_found(Entity::Template, template)
                    }
                    other => failed(operation)(other),
                })?;
            transaction
                .prepare_cached(
                    "INSERT INTO document_bodies (document_id, body) SELECT ?1, body FROM template_bodies WHERE template_id = ?2",
                )
                .and_then(|mut statement| statement.execute(params![document_id, template.get()]))
                .map_err(failed(operation))?;
        }
        let documents = summaries_for(&transaction, fir_id, operation)?;
        transaction.commit().map_err(failed(operation))?;
        Ok(documents)
    }

    pub fn save_fir_document(
        &mut self,
        document_id: FirDocumentId,
        expected_revision: Revision,
        document: &Document,
    ) -> Result<SaveAck<FirDocumentId>> {
        let operation = "firDocument.save";
        let projections = document.projections();
        let now = Timestamp::now();
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        let changed = transaction
            .prepare_cached(
                "UPDATE documents SET revision = ?1, updated_at = ?2, preview = ?3, field_count = ?4 WHERE id = ?5 AND revision = ?6",
            )
            .and_then(|mut statement| {
                statement.execute(params![
                    expected_revision.next().get(),
                    now.unix_millis(),
                    projections.preview_text,
                    projections.field_count as i64,
                    document_id.get(),
                    expected_revision.get(),
                ])
            })
            .map_err(failed(operation))?;
        if changed == 0 {
            return Err(revision_conflict(
                &transaction,
                "documents",
                Entity::FirDocument,
                document_id.get(),
                expected_revision,
                operation,
            ));
        }
        transaction
            .prepare_cached("UPDATE document_bodies SET body = ?1 WHERE document_id = ?2")
            .and_then(|mut statement| statement.execute(params![pack(document), document_id.get()]))
            .map_err(failed(operation))?;
        transaction.commit().map_err(failed(operation))?;
        Ok(SaveAck {
            id: document_id,
            revision: expected_revision.next(),
            updated_at: now,
            preview_text: projections.preview_text,
            field_count: projections.field_count as i64,
        })
    }

    pub fn reorder_fir_documents(
        &mut self,
        fir_id: FirId,
        document_ids: &[FirDocumentId],
    ) -> Result<Vec<FirDocumentSummary>> {
        let operation = "firDocument.reorder";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        let existing: HashSet<FirDocumentId> = summaries_for(&transaction, fir_id, operation)?
            .into_iter()
            .map(|document| document.id)
            .collect();
        let distinct: HashSet<FirDocumentId> = document_ids.iter().copied().collect();
        if existing.len() != document_ids.len() || distinct.len() != document_ids.len() {
            return Err(RepositoryError::conflict(
                Entity::FirDocument,
                "order",
                fir_id,
            ));
        }
        if let Some(unknown) = document_ids.iter().find(|id| !existing.contains(id)) {
            return Err(RepositoryError::not_found(Entity::FirDocument, unknown));
        }
        for (position, document) in document_ids.iter().enumerate() {
            transaction
                .prepare_cached("UPDATE documents SET position = ?1 WHERE id = ?2")
                .and_then(|mut statement| {
                    statement.execute(params![position as i64, document.get()])
                })
                .map_err(failed(operation))?;
        }
        let documents = summaries_for(&transaction, fir_id, operation)?;
        transaction.commit().map_err(failed(operation))?;
        Ok(documents)
    }

    pub fn remove_fir_document(&mut self, document: FirDocumentId) -> Result<()> {
        let removed = self
            .connection
            .execute("DELETE FROM documents WHERE id = ?1", [document.get()])
            .map_err(failed("firDocument.remove"))?;
        if removed == 0 {
            return Err(RepositoryError::not_found(Entity::FirDocument, document));
        }
        Ok(())
    }
}
