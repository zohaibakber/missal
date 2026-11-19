use std::path::{Path, PathBuf};

use crate::platform;

pub const DATABASE_FILE: &str = "missal.db";
pub const PREFERENCES_FILE: &str = "missal-preferences.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub directory: PathBuf,
    pub database: PathBuf,
    pub preferences: PathBuf,
    pub is_default: bool,
}

impl Paths {
    pub fn in_directory(directory: impl Into<PathBuf>, is_default: bool) -> Paths {
        let directory = directory.into();
        Paths {
            database: directory.join(DATABASE_FILE),
            preferences: directory.join(PREFERENCES_FILE),
            directory,
            is_default,
        }
    }

    pub fn resolve(directory_override: Option<&Path>) -> Paths {
        match directory_override {
            Some(directory) => Paths::in_directory(directory, false),
            None => {
                let root = platform::app_data_root().unwrap_or_else(std::env::temp_dir);
                Paths::in_directory(root.join(platform::PRODUCT_NAME), true)
            }
        }
    }

    pub fn ensure_directory(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.directory)
    }
}
