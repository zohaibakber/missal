use std::path::Path;

use crate::session::Session;

const SERVICE: &str = "Missal";

pub struct SessionStore {
    entry: Result<keyring::Entry, String>,
}

impl SessionStore {
    pub fn for_data_directory(directory: &Path) -> SessionStore {
        let user = format!("session:{}", directory.display());
        SessionStore {
            entry: keyring::Entry::new(SERVICE, &user).map_err(|error| error.to_string()),
        }
    }

    fn entry(&self) -> Result<&keyring::Entry, String> {
        self.entry.as_ref().map_err(Clone::clone)
    }

    pub fn load(&self) -> Result<Option<Session>, String> {
        match self.entry()?.get_password() {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(session) => Ok(Some(session)),
                Err(_) => {
                    self.clear()?;
                    Ok(None)
                }
            },
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn save(&self, session: &Session) -> Result<(), String> {
        let json = serde_json::to_string(session).map_err(|error| error.to_string())?;
        self.entry()?
            .set_password(&json)
            .map_err(|error| error.to_string())
    }

    pub fn clear(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}
