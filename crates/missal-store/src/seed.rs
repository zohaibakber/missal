use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::Read;

use flate2::read::GzDecoder;
use missal_core::document::Envelope;
use missal_core::error::STORED_DATA_INVALID;
use missal_core::text::js_trim;
use missal_core::{Document, PlaceholderId, PlaceholderSource, RepositoryError, field_lookup_key};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::{Result, failed, new_setting_key, now};

const EMBEDDED_FIELDS: &str = include_str!("../../../templates/fields.json");
const EMBEDDED_BUNDLE: &[u8] = include_bytes!("../../../templates/bundle.json.gz");

#[derive(Debug, Clone)]
pub struct TemplateBundle {
    version: i64,
    global: Vec<String>,
    custom: Vec<String>,
    templates_gz: Cow<'static, [u8]>,
}

fn invalid() -> RepositoryError {
    RepositoryError::storage("bundledTemplates.decode", STORED_DATA_INVALID)
}

fn labels(fields: &Value, key: &str) -> Result<Vec<String>> {
    fields
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(invalid)?
        .iter()
        .map(|label| {
            let label = js_trim(label.as_str().ok_or_else(invalid)?);
            if label.is_empty() {
                Err(invalid())
            } else {
                Ok(label.to_owned())
            }
        })
        .collect()
}

impl TemplateBundle {
    pub fn embedded() -> TemplateBundle {
        TemplateBundle::parse(EMBEDDED_FIELDS, Cow::Borrowed(EMBEDDED_BUNDLE))
            .expect("the embedded template bundle is valid")
    }

    pub fn new(fields_json: &str, bundle_gz: Vec<u8>) -> Result<TemplateBundle> {
        TemplateBundle::parse(fields_json, Cow::Owned(bundle_gz))
    }

    fn parse(fields_json: &str, templates_gz: Cow<'static, [u8]>) -> Result<TemplateBundle> {
        let fields: Value = serde_json::from_str(fields_json).map_err(|_| invalid())?;
        let version = fields
            .get("version")
            .and_then(Value::as_i64)
            .filter(|version| *version > 0)
            .ok_or_else(invalid)?;
        Ok(TemplateBundle {
            version,
            global: labels(&fields, "global")?,
            custom: labels(&fields, "custom")?,
            templates_gz,
        })
    }

    pub fn version(&self) -> i64 {
        self.version
    }

    pub fn templates(&self) -> Result<Vec<(String, Document)>> {
        let mut json = String::new();
        GzDecoder::new(self.templates_gz.as_ref())
            .read_to_string(&mut json)
            .map_err(|_| invalid())?;
        let Value::Array(items) = serde_json::from_str(&json).map_err(|_| invalid())? else {
            return Err(invalid());
        };
        items
            .into_iter()
            .map(|mut item| {
                let name = js_trim(
                    item.get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(invalid)?,
                )
                .to_owned();
                if name.is_empty() {
                    return Err(invalid());
                }
                let document = item
                    .get_mut("document")
                    .map(Value::take)
                    .ok_or_else(invalid)?;
                if document.get("version").and_then(Value::as_f64) != Some(2.0) {
                    return Err(invalid());
                }
                let envelope = Envelope::from_value(document).map_err(|_| invalid())?;
                Ok((
                    name,
                    Document::from_envelope(&envelope).map_err(|_| invalid())?,
                ))
            })
            .collect()
    }
}

pub(crate) fn install_bundled_templates(
    connection: &mut Connection,
    bundle: &TemplateBundle,
) -> Result<()> {
    let operation = "bundledTemplates.install";
    let transaction = connection.transaction().map_err(failed(operation))?;
    let installed: i64 = transaction
        .query_row(
            "SELECT bundled_templates_version FROM app_settings",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(failed(operation))?
        .unwrap_or(0);
    if installed >= bundle.version {
        return Ok(());
    }
    let templates = bundle.templates()?;
    (|| {
        let mut ids: HashMap<String, PlaceholderId> = HashMap::new();
        {
            let mut statement = transaction.prepare("SELECT id, label FROM placeholders")?;
            let rows = statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
            for row in rows {
                let (id, label) = row?;
                if let Some(id) = PlaceholderId::new(id) {
                    ids.insert(field_lookup_key(&label), id);
                }
            }
        }
        let sources = bundle
            .global
            .iter()
            .map(|label| {
                (
                    label,
                    PlaceholderSource::SharedSetting {
                        setting: new_setting_key(),
                    },
                )
            })
            .chain(bundle.custom.iter().map(|label| (label, PlaceholderSource::Custom)));
        for (label, source) in sources {
            let key = field_lookup_key(label);
            if ids.contains_key(&key) {
                continue;
            }
            let id: i64 = transaction.query_row(
                "INSERT INTO placeholders (label, source) VALUES (?1, ?2) RETURNING id",
                params![label, source.to_json().to_string()],
                |row| row.get(0),
            )?;
            if let Some(id) = PlaceholderId::new(id) {
                ids.insert(key, id);
            }
        }
        let existing: HashSet<String> = transaction
            .prepare("SELECT name FROM templates")?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let timestamp = now();
        for (name, mut document) in templates {
            if existing.contains(&name) {
                continue;
            }
            document.bind_catalog_fields(&ids);
            let projections = document.projections();
            transaction.execute(
                "INSERT INTO templates (name, document, revision, plain_text, preview_text, field_references, field_count, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    name,
                    document.encode(),
                    projections.plain_text,
                    projections.preview_text,
                    projections.field_references_json(),
                    projections.field_count as i64,
                    timestamp.as_str(),
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO app_settings (bundled_templates_version) VALUES (?1) ON CONFLICT (id) DO UPDATE SET bundled_templates_version = excluded.bundled_templates_version",
            [bundle.version],
        )?;
        Ok(())
    })()
    .map_err(failed(operation))?;
    transaction.commit().map_err(failed(operation))
}
