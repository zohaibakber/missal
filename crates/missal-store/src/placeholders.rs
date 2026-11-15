use std::collections::BTreeMap;

use missal_core::{
    CustomFieldDraft, Entity, FieldCatalog, GlobalPlaceholder, GlobalPlaceholderDraft, Placeholder,
    PlaceholderId, PlaceholderSource, RepositoryError, placeholder_name,
};
use rusqlite::{Connection, params};

use crate::{Result, Store, failed, id, is_unique};

fn valid_label(label: &str) -> Result<String> {
    placeholder_name(label).map_err(|reason| RepositoryError::Invalid {
        entity: Entity::Placeholder,
        reason,
    })
}

fn unique(operation: &'static str, label: &str) -> impl Fn(rusqlite::Error) -> RepositoryError {
    move |error| {
        if is_unique(&error) {
            RepositoryError::conflict(Entity::Placeholder, "name", label)
        } else {
            failed(operation)(error)
        }
    }
}

fn source(shared: bool) -> PlaceholderSource {
    if shared {
        PlaceholderSource::Shared
    } else {
        PlaceholderSource::Custom
    }
}

pub(crate) fn list(connection: &Connection, operation: &'static str) -> Result<Vec<Placeholder>> {
    let rows = connection
        .prepare_cached("SELECT id, label, shared FROM placeholders ORDER BY id")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(failed(operation))?;
    rows.into_iter()
        .map(|(raw_id, label, shared)| {
            Ok(Placeholder {
                id: id(raw_id, PlaceholderId::new, operation)?,
                label,
                source: source(shared),
            })
        })
        .collect()
}

pub(crate) fn shared_values(
    connection: &Connection,
    operation: &'static str,
) -> Result<BTreeMap<PlaceholderId, String>> {
    let rows = connection
        .prepare_cached("SELECT id, value FROM placeholders WHERE shared = 1 AND value <> ''")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(failed(operation))?;
    rows.into_iter()
        .map(|(raw_id, value)| Ok((id(raw_id, PlaceholderId::new, operation)?, value)))
        .collect()
}

fn find(
    connection: &Connection,
    placeholder: PlaceholderId,
    operation: &'static str,
) -> Result<Placeholder> {
    connection
        .query_row(
            "SELECT label, shared FROM placeholders WHERE id = ?1",
            [placeholder.get()],
            |row| Ok((row.get::<_, String>(0)?, row.get(1)?)),
        )
        .map(|(label, shared)| Placeholder {
            id: placeholder,
            label,
            source: source(shared),
        })
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => {
                RepositoryError::not_found(Entity::Placeholder, placeholder)
            }
            other => failed(operation)(other),
        })
}

pub(crate) fn insert(
    connection: &Connection,
    label: &str,
    shared: bool,
    value: &str,
    operation: &'static str,
) -> Result<PlaceholderId> {
    let raw: i64 = connection
        .query_row(
            "INSERT INTO placeholders (label, shared, value) VALUES (?1, ?2, ?3) RETURNING id",
            params![label, shared, value],
            |row| row.get(0),
        )
        .map_err(unique(operation, label))?;
    id(raw, PlaceholderId::new, operation)
}

fn update(
    connection: &Connection,
    placeholder: PlaceholderId,
    label: &str,
    value: Option<&str>,
    operation: &'static str,
) -> Result<()> {
    let changed = match value {
        Some(value) => connection.execute(
            "UPDATE placeholders SET label = ?1, value = ?2 WHERE id = ?3",
            params![label, value, placeholder.get()],
        ),
        None => connection.execute(
            "UPDATE placeholders SET label = ?1 WHERE id = ?2",
            params![label, placeholder.get()],
        ),
    }
    .map_err(unique(operation, label))?;
    if changed == 0 {
        return Err(RepositoryError::not_found(Entity::Placeholder, placeholder));
    }
    Ok(())
}

fn require(found: &Placeholder, expected: PlaceholderSource, operation: &str) -> Result<()> {
    if found.source == expected {
        return Ok(());
    }
    Err(RepositoryError::storage(
        operation,
        match expected {
            PlaceholderSource::Shared => "This field belongs to an FIR, not global values.",
            PlaceholderSource::Custom => "This field is a global value, not an FIR field.",
        },
    ))
}

fn globals(connection: &Connection) -> Result<Vec<GlobalPlaceholder>> {
    let operation = "globalPlaceholder.list";
    let rows = connection
        .prepare_cached("SELECT id, label, value FROM placeholders WHERE shared = 1 ORDER BY id")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(failed(operation))?;
    rows.into_iter()
        .map(|(raw_id, label, value)| {
            Ok(GlobalPlaceholder {
                id: id(raw_id, PlaceholderId::new, operation)?,
                label,
                value,
            })
        })
        .collect()
}

impl Store {
    pub fn placeholders(&self) -> Result<Vec<Placeholder>> {
        list(&self.connection, "placeholder.list")
    }

    pub fn field_catalog(&self) -> Result<FieldCatalog> {
        Ok(FieldCatalog::new(self.placeholders()?))
    }

    pub fn create_placeholder(&mut self, label: &str) -> Result<Placeholder> {
        let label = valid_label(label)?;
        let placeholder = insert(&self.connection, &label, false, "", "placeholder.create")?;
        Ok(Placeholder {
            id: placeholder,
            label,
            source: PlaceholderSource::Custom,
        })
    }

    pub fn rename_placeholder(
        &mut self,
        placeholder: PlaceholderId,
        label: &str,
    ) -> Result<Placeholder> {
        let label = valid_label(label)?;
        update(
            &self.connection,
            placeholder,
            &label,
            None,
            "placeholder.update",
        )?;
        find(&self.connection, placeholder, "placeholder.update")
    }

    pub fn remove_placeholder(&mut self, placeholder: PlaceholderId) -> Result<()> {
        let removed = self
            .connection
            .execute(
                "DELETE FROM placeholders WHERE id = ?1",
                [placeholder.get()],
            )
            .map_err(failed("placeholder.remove"))?;
        if removed == 0 {
            return Err(RepositoryError::not_found(Entity::Placeholder, placeholder));
        }
        Ok(())
    }

    pub fn global_placeholders(&self) -> Result<Vec<GlobalPlaceholder>> {
        globals(&self.connection)
    }

    pub fn set_shared_value(&mut self, placeholder: PlaceholderId, value: &str) -> Result<()> {
        let operation = "globalPlaceholder.save";
        let found = find(&self.connection, placeholder, operation)?;
        require(&found, PlaceholderSource::Shared, operation)?;
        update(
            &self.connection,
            placeholder,
            &found.label,
            Some(value.trim()),
            operation,
        )
    }

    pub fn save_global_placeholders(
        &mut self,
        entries: &[GlobalPlaceholderDraft],
    ) -> Result<Vec<GlobalPlaceholder>> {
        let operation = "globalPlaceholder.save";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        for entry in entries {
            match entry {
                GlobalPlaceholderDraft::New { label, value } => {
                    insert(
                        &transaction,
                        &valid_label(label)?,
                        true,
                        value.trim(),
                        operation,
                    )?;
                }
                GlobalPlaceholderDraft::Existing {
                    id: placeholder,
                    label,
                    value,
                } => {
                    let label = valid_label(label)?;
                    require(
                        &find(&transaction, *placeholder, operation)?,
                        PlaceholderSource::Shared,
                        operation,
                    )?;
                    update(
                        &transaction,
                        *placeholder,
                        &label,
                        Some(value.trim()),
                        operation,
                    )?;
                }
            }
        }
        transaction.commit().map_err(failed(operation))?;
        globals(&self.connection)
    }

    pub fn save_custom_fields(&mut self, drafts: &[CustomFieldDraft]) -> Result<Vec<Placeholder>> {
        let operation = "placeholder.saveCustom";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        for draft in drafts {
            match draft {
                CustomFieldDraft::Rename {
                    id: placeholder,
                    label,
                } => {
                    let label = valid_label(label)?;
                    require(
                        &find(&transaction, *placeholder, operation)?,
                        PlaceholderSource::Custom,
                        operation,
                    )?;
                    update(&transaction, *placeholder, &label, None, operation)?;
                }
                CustomFieldDraft::New { label } => {
                    insert(&transaction, &valid_label(label)?, false, "", operation)?;
                }
            }
        }
        transaction.commit().map_err(failed(operation))?;
        self.placeholders()
    }
}
