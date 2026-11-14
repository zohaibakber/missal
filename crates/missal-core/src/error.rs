use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entity {
    Placeholder,
    Template,
    Fir,
    FirDocument,
    FirPlaceholderValue,
    Settings,
}

impl Entity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Entity::Placeholder => "placeholder",
            Entity::Template => "template",
            Entity::Fir => "fir",
            Entity::FirDocument => "firDocument",
            Entity::FirPlaceholderValue => "firPlaceholderValue",
            Entity::Settings => "settings",
        }
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub const STORAGE_FAILED: &str = "Storage operation failed";
pub const STORED_DATA_INVALID: &str = "Stored data is invalid";
pub const STORED_DATA_NOT_WRITTEN: &str = "Stored data was not written";
pub const SAVED_ELSEWHERE: &str = "This was saved elsewhere. Your draft is still here.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    Required(&'static str),
    EmptyEntry(&'static str),
    NoAccused,
    EmptyName,
    FirFieldName,
    FieldMarker,
}

impl Invalid {
    pub fn message(&self) -> &'static str {
        match self {
            Invalid::Required(_) => "Required",
            Invalid::EmptyEntry(_) => "Fill in or remove empty entries",
            Invalid::NoAccused => "Add at least one accused",
            Invalid::EmptyName => "Enter a name",
            Invalid::FirFieldName => "This name belongs to an FIR field",
            Invalid::FieldMarker => "Use 1–3 symbols, no letters or numbers.",
        }
    }

    pub fn field(&self) -> Option<&'static str> {
        match self {
            Invalid::Required(field) | Invalid::EmptyEntry(field) => Some(field),
            Invalid::NoAccused => Some("accused"),
            _ => None,
        }
    }
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Invalid {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    NotFound {
        entity: Entity,
        id: String,
    },
    Conflict {
        entity: Entity,
        field: String,
        value: String,
    },
    InUse {
        entity: Entity,
        id: String,
        used_by: Entity,
        count: u64,
    },
    Storage {
        operation: String,
        message: String,
    },
    Invalid {
        entity: Entity,
        reason: Invalid,
    },
}

impl RepositoryError {
    pub fn not_found(entity: Entity, id: impl fmt::Display) -> Self {
        RepositoryError::NotFound {
            entity,
            id: id.to_string(),
        }
    }

    pub fn conflict(entity: Entity, field: &str, value: impl fmt::Display) -> Self {
        RepositoryError::Conflict {
            entity,
            field: field.to_owned(),
            value: value.to_string(),
        }
    }

    pub fn storage(operation: &str, message: &str) -> Self {
        RepositoryError::Storage {
            operation: operation.to_owned(),
            message: message.to_owned(),
        }
    }

    pub fn is_revision_conflict(&self) -> bool {
        matches!(self, RepositoryError::Conflict { field, .. } if field == "revision")
    }

    pub fn message(&self) -> String {
        match self {
            RepositoryError::NotFound { entity, .. } => format!("{entity} not found"),
            RepositoryError::Conflict { field, .. } if field == "revision" => {
                SAVED_ELSEWHERE.to_owned()
            }
            RepositoryError::Conflict { field, .. } => format!("{field} is already in use"),
            RepositoryError::InUse { entity, .. } => {
                format!("{entity} is in use and cannot be deleted")
            }
            RepositoryError::Storage { message, .. } => message.clone(),
            RepositoryError::Invalid { reason, .. } => reason.message().to_owned(),
        }
    }
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for RepositoryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_match_the_desktop_app() {
        assert_eq!(
            RepositoryError::not_found(Entity::FirDocument, 3).message(),
            "firDocument not found"
        );
        assert_eq!(
            RepositoryError::conflict(Entity::Template, "revision", 2).message(),
            SAVED_ELSEWHERE
        );
        assert_eq!(
            RepositoryError::conflict(Entity::Placeholder, "name", "x").message(),
            "name is already in use"
        );
        assert_eq!(
            RepositoryError::InUse {
                entity: Entity::Template,
                id: "1".into(),
                used_by: Entity::FirDocument,
                count: 2
            }
            .message(),
            "template is in use and cannot be deleted"
        );
        assert_eq!(
            RepositoryError::storage("fir.list", STORAGE_FAILED).message(),
            STORAGE_FAILED
        );
    }
}
