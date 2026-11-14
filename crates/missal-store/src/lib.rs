mod documents;
mod firs;
mod migrate;
mod placeholders;
mod rows;
mod seed;
mod settings;
mod templates;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use missal_core::Document;
use missal_core::error::STORAGE_FAILED;
use missal_core::{
    FirDocumentId, FirId, FirValues, RepositoryError, Revision, TemplateId, Timestamp,
};
use rusqlite::Connection;

pub use seed::TemplateBundle;

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

    pub fn from_connection(mut connection: Connection, bundle: &TemplateBundle) -> Result<Store> {
        enable_foreign_keys(&connection)?;
        let baseline = migrate::inspect(&connection)?;
        configure_journal(&connection)?;
        migrate::migrate(&mut connection, &baseline)?;
        migrate::upgrade_document_formats(&mut connection)?;
        seed::install_bundled_templates(&mut connection, bundle)?;
        Ok(Store { connection })
    }

    pub fn into_connection(self) -> Connection {
        self.connection
    }
}

fn enable_foreign_keys(connection: &Connection) -> Result<()> {
    let init = failed("database.init");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(&init)?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(&init)?;
    let enabled: i64 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(&init)?;
    if enabled != 1 {
        return Err(RepositoryError::storage(
            "database.init",
            "SQLite foreign keys could not be enabled",
        ));
    }
    Ok(())
}

fn configure_journal(connection: &Connection) -> Result<()> {
    let init = failed("database.init");
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })
        .map_err(&init)?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(&init)
}

pub(crate) fn failed(operation: &'static str) -> impl Fn(rusqlite::Error) -> RepositoryError {
    move |_| RepositoryError::storage(operation, STORAGE_FAILED)
}

pub(crate) fn open_error(message: &str) -> RepositoryError {
    RepositoryError::storage("database.open", message)
}

pub(crate) fn now() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64);
    Timestamp::from_unix_millis(millis)
}

pub(crate) fn new_setting_key() -> String {
    format!("global_{}", uuid::Uuid::new_v4().hyphenated()).replace('-', "_")
}
