use missal_core::{Document, Entity, Invalid, RepositoryError, Revision, TemplateId, Timestamp};
use rusqlite::{Connection, Row, params};

use crate::{
    Result, SaveAck, Store, TemplateRecord, TemplateSummary, failed, id, is_constraint, pack,
    timestamp, unpack,
};

const SUMMARY_COLUMNS: &str = "id, name, revision, preview, field_count, created_at, updated_at";

fn template_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(RepositoryError::Invalid {
            entity: Entity::Template,
            reason: Invalid::Required("name"),
        });
    }
    Ok(name.to_owned())
}

fn like_needle(query: &str) -> Option<String> {
    let needle: String = query
        .trim()
        .chars()
        .filter(|c| !matches!(c, '%' | '_'))
        .collect();
    (!needle.is_empty()).then(|| format!("%{needle}%"))
}

type RawSummary = (i64, String, i64, String, i64, Timestamp, Timestamp);

fn raw_summary(row: &Row<'_>) -> rusqlite::Result<RawSummary> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        timestamp(row, 5)?,
        timestamp(row, 6)?,
    ))
}

fn summary(raw: RawSummary, operation: &str) -> Result<TemplateSummary> {
    let (raw_id, name, revision, preview_text, field_count, created_at, updated_at) = raw;
    Ok(TemplateSummary {
        id: id(raw_id, TemplateId::new, operation)?,
        name,
        revision: id(revision, Revision::new, operation)?,
        preview_text,
        field_count,
        created_at,
        updated_at,
    })
}

fn summaries(
    connection: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    operation: &'static str,
) -> Result<Vec<TemplateSummary>> {
    let rows = connection
        .prepare_cached(sql)
        .and_then(|mut statement| {
            statement
                .query_map(params, raw_summary)?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(failed(operation))?;
    rows.into_iter()
        .map(|raw| summary(raw, operation))
        .collect()
}

pub(crate) fn insert(
    connection: &Connection,
    name: &str,
    document: &Document,
    operation: &'static str,
) -> Result<TemplateSummary> {
    let projections = document.projections();
    let now = Timestamp::now().unix_millis();
    let raw = connection
        .prepare_cached(&format!(
            "INSERT INTO templates (name, revision, field_count, created_at, updated_at, preview) VALUES (?1, 1, ?2, ?3, ?3, ?4) RETURNING {SUMMARY_COLUMNS}"
        ))
        .and_then(|mut statement| {
            statement.query_row(
                params![
                    name,
                    projections.field_count as i64,
                    now,
                    projections.preview_text
                ],
                raw_summary,
            )
        })
        .map_err(failed(operation))?;
    connection
        .prepare_cached(
            "INSERT INTO template_bodies (template_id, plain_text, body) VALUES (?1, ?2, ?3)",
        )
        .and_then(|mut statement| {
            statement.execute(params![raw.0, projections.plain_text, pack(document)])
        })
        .map_err(failed(operation))?;
    summary(raw, operation)
}

pub(crate) fn revision_conflict(
    connection: &Connection,
    table: &str,
    entity: Entity,
    raw_id: i64,
    expected: Revision,
    operation: &'static str,
) -> RepositoryError {
    let exists = connection
        .query_row(
            &format!("SELECT EXISTS (SELECT 1 FROM {table} WHERE id = ?1)"),
            [raw_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(failed(operation));
    match exists {
        Ok(true) => RepositoryError::conflict(entity, "revision", expected),
        Ok(false) => RepositoryError::not_found(entity, raw_id),
        Err(error) => error,
    }
}

impl Store {
    pub fn templates(&self) -> Result<Vec<TemplateSummary>> {
        summaries(
            &self.connection,
            &format!("SELECT {SUMMARY_COLUMNS} FROM templates ORDER BY updated_at DESC, id DESC"),
            [],
            "template.list",
        )
    }

    pub fn search_templates(&self, query: &str) -> Result<Vec<TemplateSummary>> {
        let Some(needle) = like_needle(query) else {
            return self.templates();
        };
        summaries(
            &self.connection,
            &format!(
                "SELECT {SUMMARY_COLUMNS} FROM templates t WHERE t.name LIKE ?1 OR EXISTS (SELECT 1 FROM template_bodies b WHERE b.template_id = t.id AND b.plain_text LIKE ?1) ORDER BY updated_at DESC, id DESC"
            ),
            [needle],
            "template.search",
        )
    }

    pub fn template(&self, template: TemplateId) -> Result<TemplateRecord> {
        let operation = "template.get";
        let (name, revision, created_at, updated_at, bytes): (
            String,
            i64,
            Timestamp,
            Timestamp,
            Vec<u8>,
        ) = self
            .connection
            .prepare_cached(
                "SELECT t.name, t.revision, t.created_at, t.updated_at, b.body FROM templates t JOIN template_bodies b ON b.template_id = t.id WHERE t.id = ?1",
            )
            .and_then(|mut statement| {
                statement.query_row([template.get()], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        timestamp(row, 2)?,
                        timestamp(row, 3)?,
                        row.get(4)?,
                    ))
                })
            })
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => {
                    RepositoryError::not_found(Entity::Template, template)
                }
                other => failed(operation)(other),
            })?;
        Ok(TemplateRecord {
            id: template,
            name,
            document: unpack(&bytes, operation)?,
            revision: id(revision, Revision::new, operation)?,
            created_at,
            updated_at,
        })
    }

    pub fn create_template(&mut self, name: &str, document: &Document) -> Result<TemplateSummary> {
        let name = template_name(name)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(failed("template.create"))?;
        let created = insert(&transaction, &name, document, "template.create")?;
        transaction.commit().map_err(failed("template.create"))?;
        Ok(created)
    }

    pub fn save_template(
        &mut self,
        template: TemplateId,
        expected_revision: Revision,
        name: &str,
        document: &Document,
    ) -> Result<SaveAck<TemplateId>> {
        let operation = "template.save";
        let name = template_name(name)?;
        let projections = document.projections();
        let now = Timestamp::now();
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        let changed = transaction
            .prepare_cached(
                "UPDATE templates SET name = ?1, revision = ?2, updated_at = ?3, preview = ?4, field_count = ?5 WHERE id = ?6 AND revision = ?7",
            )
            .and_then(|mut statement| {
                statement.execute(params![
                    name,
                    expected_revision.next().get(),
                    now.unix_millis(),
                    projections.preview_text,
                    projections.field_count as i64,
                    template.get(),
                    expected_revision.get(),
                ])
            })
            .map_err(failed(operation))?;
        if changed == 0 {
            return Err(revision_conflict(
                &transaction,
                "templates",
                Entity::Template,
                template.get(),
                expected_revision,
                operation,
            ));
        }
        transaction
            .prepare_cached(
                "UPDATE template_bodies SET plain_text = ?1, body = ?2 WHERE template_id = ?3",
            )
            .and_then(|mut statement| {
                statement.execute(params![
                    projections.plain_text,
                    pack(document),
                    template.get()
                ])
            })
            .map_err(failed(operation))?;
        transaction.commit().map_err(failed(operation))?;
        Ok(SaveAck {
            id: template,
            revision: expected_revision.next(),
            updated_at: now,
            preview_text: projections.preview_text,
            field_count: projections.field_count as i64,
        })
    }

    pub fn remove_template(&mut self, template: TemplateId) -> Result<()> {
        let removed = self
            .connection
            .execute("DELETE FROM templates WHERE id = ?1", [template.get()]);
        match removed {
            Ok(0) => Err(RepositoryError::not_found(Entity::Template, template)),
            Ok(_) => Ok(()),
            Err(error) if is_constraint(&error) => {
                let count: i64 = self
                    .connection
                    .query_row(
                        "SELECT count(*) FROM documents WHERE template_id = ?1",
                        [template.get()],
                        |row| row.get(0),
                    )
                    .map_err(failed("template.remove"))?;
                Err(RepositoryError::InUse {
                    entity: Entity::Template,
                    id: template.to_string(),
                    used_by: Entity::FirDocument,
                    count: count as u64,
                })
            }
            Err(error) => Err(failed("template.remove")(error)),
        }
    }
}
