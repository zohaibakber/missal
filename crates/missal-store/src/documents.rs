use std::collections::{HashMap, HashSet};

use missal_core::{Document, Entity, FirDocumentId, FirId, RepositoryError, Revision, TemplateId};
use rusqlite::{Connection, params};

use crate::firs::find_fir;
use crate::rows::{self, FIR_DOCUMENT_RECORD_COLUMNS, FIR_DOCUMENT_SUMMARY_COLUMNS};
use crate::templates::revision_conflict;
use crate::{FirDocumentRecord, FirDocumentSummary, Result, SaveAck, Store, failed, now};

pub(crate) fn summaries_for(
    connection: &Connection,
    fir_id: FirId,
    operation: &'static str,
) -> Result<Vec<FirDocumentSummary>> {
    rows::all(
        connection,
        &format!(
            "SELECT {FIR_DOCUMENT_SUMMARY_COLUMNS} FROM fir_documents WHERE fir_id = ?1 ORDER BY position ASC, id ASC"
        ),
        [fir_id.get()],
        rows::fir_document_summary,
        operation,
    )
}

impl Store {
    pub fn fir_documents(&self, fir_id: FirId) -> Result<Vec<FirDocumentSummary>> {
        summaries_for(&self.connection, fir_id, "firDocument.listForFir")
    }

    pub fn fir_document(&self, id: FirDocumentId) -> Result<FirDocumentRecord> {
        rows::one(
            &self.connection,
            &format!("SELECT {FIR_DOCUMENT_RECORD_COLUMNS} FROM fir_documents WHERE id = ?1"),
            [id.get()],
            rows::fir_document_record,
            "firDocument.get",
        )?
        .ok_or_else(|| RepositoryError::not_found(Entity::FirDocument, id))
    }

    pub fn fir_documents_by_id(&self, ids: &[FirDocumentId]) -> Result<Vec<FirDocumentRecord>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let marks = vec!["?"; ids.len()].join(", ");
        let found = rows::all(
            &self.connection,
            &format!(
                "SELECT {FIR_DOCUMENT_RECORD_COLUMNS} FROM fir_documents WHERE id IN ({marks})"
            ),
            rusqlite::params_from_iter(ids.iter().map(|id| id.get())),
            rows::fir_document_record,
            "firDocument.getMany",
        )?;
        let by_id: HashMap<FirDocumentId, FirDocumentRecord> = found
            .into_iter()
            .map(|record| (record.id, record))
            .collect();
        ids.iter()
            .map(|id| {
                by_id
                    .get(id)
                    .cloned()
                    .ok_or_else(|| RepositoryError::not_found(Entity::FirDocument, id))
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
        let attached: HashSet<TemplateId> = existing
            .iter()
            .map(|document| document.template_id)
            .collect();
        let mut seen = HashSet::new();
        let missing: Vec<TemplateId> = template_ids
            .iter()
            .copied()
            .filter(|id| seen.insert(*id) && !attached.contains(id))
            .collect();
        if !missing.is_empty() {
            let mut templates = Vec::with_capacity(missing.len());
            for id in &missing {
                let row: Option<(String, String, i64)> = transaction
                    .query_row(
                        "SELECT name, document, revision FROM templates WHERE id = ?1",
                        [id.get()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map(Some)
                    .or_else(|error| match error {
                        rusqlite::Error::QueryReturnedNoRows => Ok(None),
                        other => Err(other),
                    })
                    .map_err(failed(operation))?;
                let (name, document, revision) =
                    row.ok_or_else(|| RepositoryError::not_found(Entity::Template, id))?;
                templates.push((*id, name, document, revision));
            }
            let mut position = existing
                .iter()
                .map(|document| document.position)
                .max()
                .unwrap_or(-1);
            let timestamp = now();
            for (template_id, name, document, revision) in templates {
                let decode = rows::Decode {
                    operation: "template.document.decode",
                };
                let document = decode.document(&document)?;
                let title = decode.required(&name)?;
                let projections = document.projections();
                position += 1;
                transaction
                    .execute(
                        "INSERT INTO fir_documents (fir_id, template_id, source_template_revision, title, document, revision, position, plain_text, preview_text, field_references, field_count, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                        params![
                            fir_id.get(),
                            template_id.get(),
                            revision,
                            title,
                            document.encode(),
                            position,
                            projections.plain_text,
                            projections.preview_text,
                            projections.field_references_json(),
                            projections.field_count as i64,
                            timestamp.as_str(),
                        ],
                    )
                    .map_err(failed(operation))?;
            }
        }
        let documents = summaries_for(&transaction, fir_id, operation)?;
        transaction.commit().map_err(failed(operation))?;
        Ok(documents)
    }

    pub fn save_fir_document(
        &mut self,
        id: FirDocumentId,
        expected_revision: Revision,
        document: &Document,
    ) -> Result<SaveAck<FirDocumentId>> {
        let operation = "firDocument.save";
        let projections = document.projections();
        let timestamp = now();
        let saved = rows::one(
            &self.connection,
            &format!(
                "UPDATE fir_documents SET revision = ?1, updated_at = ?2, document = ?3, plain_text = ?4, preview_text = ?5, field_references = ?6, field_count = ?7 WHERE id = ?8 AND revision = ?9 RETURNING {FIR_DOCUMENT_SUMMARY_COLUMNS}"
            ),
            params![
                expected_revision.next().get(),
                timestamp.as_str(),
                document.encode(),
                projections.plain_text,
                projections.preview_text,
                projections.field_references_json(),
                projections.field_count as i64,
                id.get(),
                expected_revision.get(),
            ],
            rows::fir_document_summary,
            operation,
        )?;
        let Some(summary) = saved else {
            return Err(revision_conflict(
                &self.connection,
                "fir_documents",
                Entity::FirDocument,
                id.get(),
                expected_revision,
                operation,
            ));
        };
        Ok(SaveAck {
            id: summary.id,
            revision: summary.revision,
            updated_at: summary.updated_at,
            preview_text: summary.preview_text,
            field_count: summary.field_count,
        })
    }

    pub fn reorder_fir_documents(
        &mut self,
        fir_id: FirId,
        document_ids: &[FirDocumentId],
    ) -> Result<Vec<FirDocumentSummary>> {
        let operation = "firDocument.reorder";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        let existing: HashSet<i64> = transaction
            .prepare("SELECT id FROM fir_documents WHERE fir_id = ?1")
            .and_then(|mut statement| {
                statement
                    .query_map([fir_id.get()], |row| row.get(0))?
                    .collect()
            })
            .map_err(failed(operation))?;
        let distinct: HashSet<FirDocumentId> = document_ids.iter().copied().collect();
        if existing.len() != document_ids.len() || distinct.len() != document_ids.len() {
            return Err(RepositoryError::conflict(
                Entity::FirDocument,
                "order",
                fir_id,
            ));
        }
        if let Some(unknown) = document_ids.iter().find(|id| !existing.contains(&id.get())) {
            return Err(RepositoryError::not_found(Entity::FirDocument, unknown));
        }
        let timestamp = now();
        for (position, id) in document_ids.iter().enumerate() {
            transaction
                .execute(
                    "UPDATE fir_documents SET position = ?1, updated_at = ?2 WHERE id = ?3",
                    params![position as i64, timestamp.as_str(), id.get()],
                )
                .map_err(failed(operation))?;
        }
        let documents = summaries_for(&transaction, fir_id, operation)?;
        transaction.commit().map_err(failed(operation))?;
        Ok(documents)
    }

    pub fn remove_fir_document(&mut self, id: FirDocumentId) -> Result<()> {
        let removed = self
            .connection
            .execute("DELETE FROM fir_documents WHERE id = ?1", [id.get()])
            .map_err(failed("firDocument.remove"))?;
        if removed == 0 {
            return Err(RepositoryError::not_found(Entity::FirDocument, id));
        }
        Ok(())
    }
}
