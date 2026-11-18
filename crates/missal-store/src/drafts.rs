use missal_core::{Document, FirDocumentId, Revision, TemplateId, Timestamp};
use rusqlite::{OptionalExtension, params};

use crate::{Result, Store, failed, id, pack, timestamp, unpack};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraftKey {
    FirDocument(FirDocumentId),
    Template(TemplateId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub key: DraftKey,
    pub base_revision: Revision,
    pub document: Document,
    pub saved_at: Timestamp,
}

struct Tables {
    drafts: &'static str,
    key: &'static str,
    parent: &'static str,
    id: i64,
}

impl DraftKey {
    fn tables(self) -> Tables {
        match self {
            DraftKey::FirDocument(document) => Tables {
                drafts: "document_drafts",
                key: "document_id",
                parent: "documents",
                id: document.get(),
            },
            DraftKey::Template(template) => Tables {
                drafts: "template_drafts",
                key: "template_id",
                parent: "templates",
                id: template.get(),
            },
        }
    }
}

impl Store {
    pub fn put_draft(
        &mut self,
        key: DraftKey,
        base_revision: Revision,
        document: &Document,
    ) -> Result<bool> {
        let Tables {
            drafts,
            key: column,
            parent,
            id,
        } = key.tables();
        let written = self
            .connection
            .prepare_cached(&format!(
                "INSERT INTO {drafts} ({column}, base_revision, saved_at, body) SELECT id, revision, ?1, ?2 FROM {parent} WHERE id = ?3 AND revision = ?4 ON CONFLICT ({column}) DO UPDATE SET base_revision = excluded.base_revision, saved_at = excluded.saved_at, body = excluded.body"
            ))
            .and_then(|mut statement| {
                statement.execute(params![
                    Timestamp::now().unix_millis(),
                    pack(document),
                    id,
                    base_revision.get(),
                ])
            })
            .map_err(failed("draft.put"))?;
        Ok(written > 0)
    }

    pub fn draft(&self, key: DraftKey) -> Result<Option<Draft>> {
        let operation = "draft.get";
        let Tables {
            drafts,
            key: column,
            parent,
            id: target,
        } = key.tables();
        let found = self
            .connection
            .prepare_cached(&format!(
                "SELECT d.base_revision, d.saved_at, d.body FROM {drafts} d JOIN {parent} p ON p.id = d.{column} AND p.revision = d.base_revision WHERE d.{column} = ?1"
            ))
            .and_then(|mut statement| {
                statement
                    .query_row([target], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            timestamp(row, 1)?,
                            row.get::<_, Vec<u8>>(2)?,
                        ))
                    })
                    .optional()
            })
            .map_err(failed(operation))?;
        found
            .map(|(revision, saved_at, body)| {
                Ok(Draft {
                    key,
                    base_revision: id(revision, Revision::new, operation)?,
                    document: unpack(&body, operation)?,
                    saved_at,
                })
            })
            .transpose()
    }

    pub fn discard_draft(&mut self, key: DraftKey) -> Result<()> {
        let Tables {
            drafts,
            key: column,
            id,
            ..
        } = key.tables();
        self.connection
            .prepare_cached(&format!("DELETE FROM {drafts} WHERE {column} = ?1"))
            .and_then(|mut statement| statement.execute([id]))
            .map_err(failed("draft.discard"))?;
        Ok(())
    }
}
