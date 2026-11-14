use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use missal_core::error::STORED_DATA_INVALID;
use missal_core::{Document, PlaceholderId, RepositoryError, field_lookup_key};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{Result, failed, placeholders, templates};

const MAGIC: &[u8; 4] = b"MSB\x01";
const EMBEDDED: &[u8] = include_bytes!("../bundle/templates.bin");

#[derive(Debug, Clone)]
pub struct TemplateBundle {
    version: i64,
    global: Vec<String>,
    custom: Vec<String>,
    templates: Vec<(String, Vec<u8>)>,
}

fn invalid() -> RepositoryError {
    RepositoryError::storage("bundledTemplates.decode", STORED_DATA_INVALID)
}

struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn bytes(&mut self) -> Result<&'a [u8]> {
        let (len, rest) = self.0.split_first_chunk::<4>().ok_or_else(invalid)?;
        let len = u32::from_le_bytes(*len) as usize;
        if rest.len() < len {
            return Err(invalid());
        }
        let (head, tail) = rest.split_at(len);
        self.0 = tail;
        Ok(head)
    }

    fn count(&mut self) -> Result<usize> {
        let (count, rest) = self.0.split_first_chunk::<4>().ok_or_else(invalid)?;
        self.0 = rest;
        Ok(u32::from_le_bytes(*count) as usize)
    }

    fn text(&mut self) -> Result<String> {
        std::str::from_utf8(self.bytes()?)
            .map(str::to_owned)
            .map_err(|_| invalid())
    }

    fn texts(&mut self) -> Result<Vec<String>> {
        (0..self.count()?).map(|_| self.text()).collect()
    }
}

fn put(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn put_count(out: &mut Vec<u8>, count: usize) {
    out.extend_from_slice(&(count as u32).to_le_bytes());
}

impl TemplateBundle {
    pub fn embedded() -> TemplateBundle {
        TemplateBundle::decode(EMBEDDED).expect("the embedded template bundle is valid")
    }

    pub fn decode(compressed: &[u8]) -> Result<TemplateBundle> {
        let mut raw = Vec::new();
        DeflateDecoder::new(compressed)
            .read_to_end(&mut raw)
            .map_err(|_| invalid())?;
        let body = raw.strip_prefix(MAGIC.as_slice()).ok_or_else(invalid)?;
        let (version, body) = body.split_first_chunk::<4>().ok_or_else(invalid)?;
        let mut cursor = Cursor(body);
        let global = cursor.texts()?;
        let custom = cursor.texts()?;
        let templates = (0..cursor.count()?)
            .map(|_| Ok((cursor.text()?, cursor.bytes()?.to_vec())))
            .collect::<Result<Vec<_>>>()?;
        Ok(TemplateBundle {
            version: i64::from(u32::from_le_bytes(*version)),
            global,
            custom,
            templates,
        })
    }

    pub fn encode(
        version: u32,
        global: &[String],
        custom: &[String],
        templates: &[(String, Document)],
    ) -> Vec<u8> {
        let mut raw = MAGIC.to_vec();
        raw.extend_from_slice(&version.to_le_bytes());
        for labels in [global, custom] {
            put_count(&mut raw, labels.len());
            for label in labels {
                put(&mut raw, label.as_bytes());
            }
        }
        put_count(&mut raw, templates.len());
        for (name, document) in templates {
            put(&mut raw, name.as_bytes());
            put(&mut raw, &document.encode());
        }
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
        encoder
            .write_all(&raw)
            .and_then(|()| encoder.finish())
            .expect("compressing into memory cannot fail")
    }

    pub fn version(&self) -> i64 {
        self.version
    }

    pub fn labels(&self) -> (&[String], &[String]) {
        (&self.global, &self.custom)
    }

    pub fn templates(&self) -> Result<Vec<(String, Document)>> {
        self.templates
            .iter()
            .map(|(name, bytes)| {
                Ok((
                    name.clone(),
                    Document::decode(bytes).map_err(|_| invalid())?,
                ))
            })
            .collect()
    }
}

pub(crate) fn install(connection: &mut Connection, bundle: &TemplateBundle) -> Result<()> {
    let operation = "bundledTemplates.install";
    let installed: i64 = connection
        .query_row(
            "SELECT bundle_version FROM settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(failed(operation))?
        .unwrap_or(0);
    if installed >= bundle.version {
        return Ok(());
    }
    let documents = bundle.templates()?;
    let transaction = connection.transaction().map_err(failed(operation))?;
    let mut ids: HashMap<String, PlaceholderId> = placeholders::list(&transaction, operation)?
        .into_iter()
        .map(|placeholder| (field_lookup_key(&placeholder.label), placeholder.id))
        .collect();
    let labels = bundle
        .global
        .iter()
        .map(|label| (label, true))
        .chain(bundle.custom.iter().map(|label| (label, false)));
    for (label, shared) in labels {
        if let Entry::Vacant(entry) = ids.entry(field_lookup_key(label)) {
            entry.insert(placeholders::insert(
                &transaction,
                label,
                shared,
                "",
                operation,
            )?);
        }
    }
    let existing: HashSet<String> = transaction
        .prepare("SELECT name FROM templates")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .map_err(failed(operation))?;
    for (name, mut document) in documents {
        if existing.contains(&name) {
            continue;
        }
        document.bind_catalog_fields(&ids);
        templates::insert(&transaction, &name, &document, operation)?;
    }
    transaction
        .execute(
            "UPDATE settings SET bundle_version = ?1 WHERE id = 1",
            params![bundle.version],
        )
        .map_err(failed(operation))?;
    transaction.commit().map_err(failed(operation))
}
