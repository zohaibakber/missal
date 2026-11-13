use missal_core::text::js_trim;
use missal_core::{Document, Entity, Invalid, RepositoryError, Revision, TemplateId};
use rusqlite::{Connection, params};

use crate::rows::{self, TEMPLATE_RECORD_COLUMNS, TEMPLATE_SUMMARY_COLUMNS, constraint};
use crate::{Result, SaveAck, Store, TemplateRecord, TemplateSummary, failed, now};

pub(crate) fn template_name(name: &str) -> Result<String> {
    let name = js_trim(name);
    if name.is_empty() {
        return Err(RepositoryError::Invalid {
            entity: Entity::Template,
            reason: Invalid::Required("name"),
        });
    }
    Ok(name.to_owned())
}

pub(crate) fn like_needle(query: &str) -> Option<String> {
    let needle: String = js_trim(query)
        .chars()
        .filter(|c| !matches!(c, '%' | '_'))
        .collect();
    (!needle.is_empty()).then(|| format!("%{needle}%"))
}

pub(crate) fn revision_conflict(
    connection: &Connection,
    table: &str,
    entity: Entity,
    id: i64,
    expected: Revision,
    operation: &'static str,
) -> RepositoryError {
    let exists = connection
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
            [id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(failed(operation));
    match exists {
        Ok(true) => RepositoryError::conflict(entity, "revision", expected),
        Ok(false) => RepositoryError::not_found(entity, id),
        Err(error) => error,
    }
}

impl Store {
    pub fn templates(&self) -> Result<Vec<TemplateSummary>> {
        rows::all(
            &self.connection,
            &format!(
                "SELECT {TEMPLATE_SUMMARY_COLUMNS} FROM templates ORDER BY updated_at DESC, id DESC"
            ),
            [],
            rows::template_summary,
            "template.list",
        )
    }

    pub fn search_templates(&self, query: &str) -> Result<Vec<TemplateSummary>> {
        let Some(needle) = like_needle(query) else {
            return self.templates();
        };
        rows::all(
            &self.connection,
            &format!(
                "SELECT {TEMPLATE_SUMMARY_COLUMNS} FROM templates WHERE name LIKE ?1 OR plain_text LIKE ?1 ORDER BY updated_at DESC, id DESC"
            ),
            [needle],
            rows::template_summary,
            "template.search",
        )
    }

    pub fn template(&self, id: TemplateId) -> Result<TemplateRecord> {
        rows::one(
            &self.connection,
            &format!("SELECT {TEMPLATE_RECORD_COLUMNS} FROM templates WHERE id = ?1"),
            [id.get()],
            rows::template_record,
            "template.get",
        )?
        .ok_or_else(|| RepositoryError::not_found(Entity::Template, id))
    }

    pub fn create_template(&mut self, name: &str, document: &Document) -> Result<TemplateSummary> {
        let name = template_name(name)?;
        let projections = document.projections();
        let timestamp = now();
        rows::one(
            &self.connection,
            &format!(
                "INSERT INTO templates (name, document, revision, plain_text, preview_text, field_references, field_count, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?7) RETURNING {TEMPLATE_SUMMARY_COLUMNS}"
            ),
            params![
                name,
                document.encode(),
                projections.plain_text,
                projections.preview_text,
                projections.field_references_json(),
                projections.field_count as i64,
                timestamp.as_str(),
            ],
            rows::template_summary,
            "template.create",
        )?
        .ok_or_else(|| {
            RepositoryError::storage("template.create", missal_core::error::STORED_DATA_NOT_WRITTEN)
        })
    }

    pub fn save_template(
        &mut self,
        id: TemplateId,
        expected_revision: Revision,
        name: &str,
        document: &Document,
    ) -> Result<SaveAck<TemplateId>> {
        let name = template_name(name)?;
        let projections = document.projections();
        let timestamp = now();
        let saved = rows::one(
            &self.connection,
            &format!(
                "UPDATE templates SET name = ?1, revision = ?2, updated_at = ?3, document = ?4, plain_text = ?5, preview_text = ?6, field_references = ?7, field_count = ?8 WHERE id = ?9 AND revision = ?10 RETURNING {TEMPLATE_SUMMARY_COLUMNS}"
            ),
            params![
                name,
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
            rows::template_summary,
            "template.save",
        )?;
        let Some(summary) = saved else {
            return Err(revision_conflict(
                &self.connection,
                "templates",
                Entity::Template,
                id.get(),
                expected_revision,
                "template.save",
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

    pub fn remove_template(&mut self, id: TemplateId) -> Result<()> {
        let removed = self
            .connection
            .execute("DELETE FROM templates WHERE id = ?1", [id.get()]);
        match removed {
            Ok(0) => Err(RepositoryError::not_found(Entity::Template, id)),
            Ok(_) => Ok(()),
            Err(error) if constraint(&error).is_some() => {
                let count: i64 = self
                    .connection
                    .query_row(
                        "SELECT count(*) FROM fir_documents WHERE template_id = ?1",
                        [id.get()],
                        |row| row.get(0),
                    )
                    .map_err(failed("template.remove"))?;
                Err(RepositoryError::InUse {
                    entity: Entity::Template,
                    id: id.to_string(),
                    used_by: Entity::FirDocument,
                    count: count as u64,
                })
            }
            Err(error) => Err(failed("template.remove")(error)),
        }
    }
}
