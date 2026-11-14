mod bundle;
mod documents;
mod firs;
mod placeholders;
mod schema;
mod settings;
mod templates;

use std::io::{Read, Write};
use std::path::Path;

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use missal_core::error::{STORAGE_FAILED, STORED_DATA_INVALID};
use missal_core::{
    Document, FirDocumentId, FirId, FirValues, RepositoryError, Revision, TemplateId, Timestamp,
};
use rusqlite::{Connection, Row};

pub use bundle::TemplateBundle;

pub type Result<T, E = RepositoryError> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSummary {
    pub id: TemplateId,
    pub name: String,
    pub revision: Revision,
    pub preview_text: String,
    pub field_count: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRecord {
    pub id: TemplateId,
    pub name: String,
    pub document: Document,
    pub revision: Revision,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveAck<Id> {
    pub id: Id,
    pub revision: Revision,
    pub updated_at: Timestamp,
    pub preview_text: String,
    pub field_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirDocumentSummary {
    pub id: FirDocumentId,
    pub fir_id: FirId,
    pub template_id: TemplateId,
    pub source_template_revision: Revision,
    pub title: String,
    pub revision: Revision,
    pub position: i64,
    pub preview_text: String,
    pub field_count: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FirDocumentRecord {
    pub id: FirDocumentId,
    pub fir_id: FirId,
    pub template_id: TemplateId,
    pub source_template_revision: Revision,
    pub title: String,
    pub document: Document,
    pub revision: Revision,
    pub position: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirWorkspace {
    pub values: FirValues,
    pub documents: Vec<FirDocumentSummary>,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Store> {
        Store::open_with(path, &TemplateBundle::embedded())
    }

    pub fn open_with(path: impl AsRef<Path>, bundle: &TemplateBundle) -> Result<Store> {
        let connection = Connection::open(path).map_err(|_| open_error(STORAGE_FAILED))?;
        Store::from_connection(connection, bundle)
    }

    pub fn in_memory() -> Result<Store> {
        let connection = Connection::open_in_memory().map_err(|_| open_error(STORAGE_FAILED))?;
        Store::from_connection(connection, &TemplateBundle::embedded())
    }

    fn from_connection(mut connection: Connection, bundle: &TemplateBundle) -> Result<Store> {
        schema::configure(&connection)?;
        schema::migrate(&mut connection)?;
        bundle::install(&mut connection, bundle)?;
        Ok(Store { connection })
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.connection.execute_batch("PRAGMA optimize");
    }
}

pub(crate) fn failed(operation: &'static str) -> impl Fn(rusqlite::Error) -> RepositoryError {
    move |_| RepositoryError::storage(operation, STORAGE_FAILED)
}

pub(crate) fn invalid(operation: &str) -> RepositoryError {
    RepositoryError::storage(operation, STORED_DATA_INVALID)
}

pub(crate) fn open_error(message: &str) -> RepositoryError {
    RepositoryError::storage("database.open", message)
}

pub(crate) fn pack(document: &Document) -> Vec<u8> {
    let raw = document.encode();
    let mut encoder = DeflateEncoder::new(Vec::with_capacity(raw.len() / 4), Compression::new(6));
    encoder
        .write_all(&raw)
        .and_then(|()| encoder.finish())
        .expect("compressing into memory cannot fail")
}

pub(crate) fn unpack(bytes: &[u8], operation: &str) -> Result<Document> {
    let mut raw = Vec::with_capacity(bytes.len() * 4);
    DeflateDecoder::new(bytes)
        .read_to_end(&mut raw)
        .map_err(|_| invalid(operation))?;
    Document::decode(&raw).map_err(|_| invalid(operation))
}

pub(crate) fn id<T>(value: i64, make: fn(i64) -> Option<T>, operation: &str) -> Result<T> {
    make(value).ok_or_else(|| invalid(operation))
}

pub(crate) fn timestamp(row: &Row<'_>, index: usize) -> rusqlite::Result<Timestamp> {
    row.get(index).map(Timestamp::from_unix_millis)
}

pub(crate) fn is_constraint(error: &rusqlite::Error) -> bool {
    error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation)
}

pub(crate) fn is_unique(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.extended_code,
                rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE | rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
            )
    )
}
