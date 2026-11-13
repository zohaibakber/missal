use missal_core::{
    CustomFieldDraft, Entity, FieldCatalog, GlobalPlaceholder, GlobalPlaceholderDraft, Placeholder,
    PlaceholderId, PlaceholderSource, RepositoryError, placeholder_name,
};
use rusqlite::{Connection, params};

use crate::rows::{self, Constraint, constraint};
use crate::settings::{read_settings, write_shared};
use crate::{Result, Store, failed, new_setting_key};

const SELECT: &str = "SELECT id, label, source FROM placeholders";

fn valid_label(label: &str) -> Result<String> {
    placeholder_name(label).map_err(|reason| RepositoryError::Invalid {
        entity: Entity::Placeholder,
        reason,
    })
}

fn unique(operation: &'static str, label: &str) -> impl Fn(rusqlite::Error) -> RepositoryError {
    move |error| match constraint(&error) {
        Some(Constraint::Unique) => RepositoryError::conflict(Entity::Placeholder, "name", label),
        _ => failed(operation)(error),
    }
}

pub(crate) fn list(connection: &Connection, operation: &'static str) -> Result<Vec<Placeholder>> {
    rows::all(
        connection,
        &format!("{SELECT} ORDER BY id"),
        [],
        rows::placeholder,
        operation,
    )
}

fn find(
    connection: &Connection,
    id: PlaceholderId,
    operation: &'static str,
) -> Result<Placeholder> {
    rows::one(
        connection,
        &format!("{SELECT} WHERE id = ?1"),
        [id.get()],
        rows::placeholder,
        operation,
    )?
    .ok_or_else(|| RepositoryError::not_found(Entity::Placeholder, id))
}

fn insert(
    connection: &Connection,
    label: &str,
    source: &PlaceholderSource,
    operation: &'static str,
) -> Result<Placeholder> {
    connection
        .query_row(
            "INSERT INTO placeholders (label, source) VALUES (?1, ?2) RETURNING id, label, source",
            params![label, source.to_json().to_string()],
            rows::placeholder,
        )
        .map_err(unique(operation, label))?
}

fn relabel(
    connection: &Connection,
    id: PlaceholderId,
    label: &str,
    operation: &'static str,
) -> Result<()> {
    let changed = connection
        .execute(
            "UPDATE placeholders SET label = ?1 WHERE id = ?2",
            params![label, id.get()],
        )
        .map_err(unique(operation, label))?;
    if changed == 0 {
        return Err(RepositoryError::not_found(Entity::Placeholder, id));
    }
    Ok(())
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
        insert(
            &self.connection,
            &label,
            &PlaceholderSource::Custom,
            "placeholder.create",
        )
    }

    pub fn rename_placeholder(&mut self, id: PlaceholderId, label: &str) -> Result<Placeholder> {
        let label = valid_label(label)?;
        relabel(&self.connection, id, &label, "placeholder.update")?;
        find(&self.connection, id, "placeholder.update")
    }

    pub fn remove_placeholder(&mut self, id: PlaceholderId) -> Result<()> {
        let removed = self
            .connection
            .execute("DELETE FROM placeholders WHERE id = ?1", [id.get()])
            .map_err(failed("placeholder.remove"))?;
        if removed == 0 {
            return Err(RepositoryError::not_found(Entity::Placeholder, id));
        }
        Ok(())
    }

    pub fn global_placeholders(&self) -> Result<Vec<GlobalPlaceholder>> {
        globals(&self.connection)
    }

    pub fn save_global_placeholders(
        &mut self,
        entries: &[GlobalPlaceholderDraft],
    ) -> Result<Vec<GlobalPlaceholder>> {
        let operation = "globalPlaceholder.save";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        let mut values = read_settings(&transaction)?.shared_placeholders;
        for entry in entries {
            match entry {
                GlobalPlaceholderDraft::New { label, value } => {
                    let label = valid_label(label)?;
                    let setting = new_setting_key();
                    insert(
                        &transaction,
                        &label,
                        &PlaceholderSource::SharedSetting {
                            setting: setting.clone(),
                        },
                        operation,
                    )?;
                    values.insert(setting, value.clone());
                }
                GlobalPlaceholderDraft::Existing { id, label, value } => {
                    let label = valid_label(label)?;
                    let field = find(&transaction, *id, "globalPlaceholder.decode")?;
                    let PlaceholderSource::SharedSetting { setting } = field.source else {
                        return Err(RepositoryError::storage(
                            operation,
                            "This field belongs to an FIR, not global values.",
                        ));
                    };
                    relabel(&transaction, *id, &label, operation)?;
                    values.insert(setting, value.clone());
                }
            }
        }
        write_shared(&transaction, &values, operation)?;
        transaction.commit().map_err(failed(operation))?;
        globals(&self.connection)
    }

    pub fn save_custom_fields(&mut self, drafts: &[CustomFieldDraft]) -> Result<Vec<Placeholder>> {
        let operation = "placeholder.saveCustom";
        let transaction = self.connection.transaction().map_err(failed(operation))?;
        for draft in drafts {
            match draft {
                CustomFieldDraft::Rename { id, label } => {
                    let label = valid_label(label)?;
                    let field = find(&transaction, *id, operation)?;
                    if field.source != PlaceholderSource::Custom {
                        return Err(RepositoryError::storage(
                            operation,
                            "This field is a global value, not an FIR field.",
                        ));
                    }
                    relabel(&transaction, *id, &label, operation)?;
                }
                CustomFieldDraft::New { label } => {
                    let label = valid_label(label)?;
                    insert(&transaction, &label, &PlaceholderSource::Custom, operation)?;
                }
            }
        }
        transaction.commit().map_err(failed(operation))?;
        self.placeholders()
    }
}

fn globals(connection: &Connection) -> Result<Vec<GlobalPlaceholder>> {
    let catalog = list(connection, "globalPlaceholder.list")?;
    let settings = read_settings(connection)?;
    Ok(catalog
        .into_iter()
        .filter_map(|field| match field.source {
            PlaceholderSource::SharedSetting { setting } => Some(GlobalPlaceholder {
                id: field.id,
                label: field.label,
                value: settings
                    .shared_placeholders
                    .get(&setting)
                    .cloned()
                    .unwrap_or_default(),
            }),
            PlaceholderSource::Custom => None,
        })
        .collect())
}
