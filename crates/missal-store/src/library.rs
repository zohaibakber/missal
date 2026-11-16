use std::collections::BTreeSet;

use missal_core::{FieldReference, PlaceholderSource, RepositoryError, Revision, TemplateId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    Result, Store, TemplateBundle, TemplateSummary, bundle, failed, id, placeholders, templates,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryLink {
    pub template_id: TemplateId,
    pub remote_id: String,
    pub version: i64,
    pub revision: Revision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryExport {
    pub name: String,
    pub field_count: i64,
    pub preview_text: String,
    pub revision: Revision,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryInstall {
    pub summary: TemplateSummary,
    pub link: LibraryLink,
    pub replaced: bool,
}

fn invalid_content() -> RepositoryError {
    RepositoryError::storage(
        "library.install",
        "The downloaded template could not be read. Update Missal and try again.",
    )
}

fn links(
    connection: &Connection,
    filter: &str,
    params: impl rusqlite::Params,
    operation: &'static str,
) -> Result<Vec<LibraryLink>> {
    let rows = connection
        .prepare_cached(&format!(
            "SELECT template_id, remote_id, version, revision FROM library_links {filter} ORDER BY template_id"
        ))
        .and_then(|mut statement| {
            statement
                .query_map(params, |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(failed(operation))?;
    rows.into_iter()
        .map(|(template, remote_id, version, revision)| {
            Ok(LibraryLink {
                template_id: id(template, TemplateId::new, operation)?,
                remote_id,
                version,
                revision: id(revision, Revision::new, operation)?,
            })
        })
        .collect()
}

fn write_link(connection: &Connection, link: &LibraryLink, operation: &'static str) -> Result<()> {
    connection
        .execute(
            "DELETE FROM library_links WHERE remote_id = ?1 AND template_id <> ?2",
            params![link.remote_id, link.template_id.get()],
        )
        .and_then(|_| {
            connection.execute(
                "INSERT INTO library_links (template_id, remote_id, version, revision) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (template_id) DO UPDATE SET remote_id = excluded.remote_id, version = excluded.version, revision = excluded.revision",
                params![
                    link.template_id.get(),
                    link.remote_id,
                    link.version,
                    link.revision.get()
                ],
            )
        })
        .map(|_| ())
        .map_err(failed(operation))
}

impl Store {
    pub fn library_links(&self) -> Result<Vec<LibraryLink>> {
        links(&self.connection, "", [], "library.links")
    }

    pub fn export_template(&self, template: TemplateId) -> Result<LibraryExport> {
        let record = self.template(template)?;
        let catalog = placeholders::list(&self.connection, "library.export")?;
        let mut used = BTreeSet::new();
        let mut document = record.document;
        document.for_each_field_mut(&mut |field| {
            if let FieldReference::Catalog(placeholder) = field.reference {
                used.insert(placeholder);
            }
        });
        let (mut global, mut custom) = (Vec::new(), Vec::new());
        for placeholder in catalog.iter().filter(|item| used.contains(&item.id)) {
            match placeholder.source {
                PlaceholderSource::Shared => global.push(placeholder.label.clone()),
                PlaceholderSource::Custom => custom.push(placeholder.label.clone()),
            }
        }
        document.unbind_catalog_fields(&|placeholder| {
            catalog
                .iter()
                .find(|item| item.id == placeholder)
                .map(|item| item.label.clone())
        });
        let projections = document.projections();
        let content =
            TemplateBundle::encode(1, &global, &custom, &[(record.name.clone(), document)]);
        Ok(LibraryExport {
            name: record.name,
            field_count: projections.field_count as i64,
            preview_text: projections.preview_text,
            revision: record.revision,
            content,
        })
    }

    pub fn link_template(
        &mut self,
        template: TemplateId,
        remote_id: &str,
        version: i64,
        revision: Revision,
    ) -> Result<LibraryLink> {
        let operation = "library.link";
        let link = LibraryLink {
            template_id: template,
            remote_id: remote_id.to_owned(),
            version,
            revision,
        };
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        write_link(&transaction, &link, operation)?;
        transaction.commit().map_err(failed(operation))?;
        Ok(link)
    }

    pub fn unlink_library(&mut self, remote_id: &str) -> Result<()> {
        self.connection
            .execute(
                "DELETE FROM library_links WHERE remote_id = ?1",
                [remote_id],
            )
            .map(|_| ())
            .map_err(failed("library.unlink"))
    }

    pub fn install_library_template(
        &mut self,
        remote_id: &str,
        version: i64,
        content: &[u8],
    ) -> Result<LibraryInstall> {
        let operation = "library.install";
        let contents = TemplateBundle::decode(content)
            .and_then(|bundle| bundle.contents())
            .map_err(|_| invalid_content())?;
        let [(name, mut document)] =
            <[_; 1]>::try_from(contents.templates).map_err(|_| invalid_content())?;
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(invalid_content());
        }
        document.for_each_field_mut(&mut |field| {
            if matches!(field.reference, FieldReference::Catalog(_)) {
                field.reference = FieldReference::Unresolved(String::new());
            }
        });
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        let ids =
            bundle::ensure_labels(&transaction, &contents.global, &contents.custom, operation)?;
        document.bind_catalog_fields(&ids);
        let existing = links(&transaction, "WHERE remote_id = ?1", [remote_id], operation)?.pop();
        let present = match &existing {
            Some(link) => transaction
                .query_row(
                    "SELECT 1 FROM templates WHERE id = ?1",
                    [link.template_id.get()],
                    |_| Ok(()),
                )
                .optional()
                .map_err(failed(operation))?
                .map(|()| link.template_id),
            None => None,
        };
        let summary = match present {
            Some(template) => {
                templates::replace(&transaction, template, &name, &document, operation)?
            }
            None => templates::insert(&transaction, &name, &document, operation)?,
        };
        let link = LibraryLink {
            template_id: summary.id,
            remote_id: remote_id.to_owned(),
            version,
            revision: summary.revision,
        };
        write_link(&transaction, &link, operation)?;
        transaction.commit().map_err(failed(operation))?;
        Ok(LibraryInstall {
            summary,
            link,
            replaced: present.is_some(),
        })
    }
}
