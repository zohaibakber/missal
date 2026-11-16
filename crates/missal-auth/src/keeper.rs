use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use crate::config::Config;
use crate::error::AuthError;
use crate::session::Session;
use crate::store::SessionStore;
use crate::workos_api;

pub struct SessionKeeper {
    config: Config,
    store: Option<Arc<SessionStore>>,
    session: Mutex<Option<Session>>,
}

impl SessionKeeper {
    pub fn new(config: Config, store: Option<Arc<SessionStore>>) -> SessionKeeper {
        SessionKeeper {
            config,
            store,
            session: Mutex::new(None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Session>> {
        self.session
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub fn replace(&self, session: Option<Session>) {
        *self.lock() = session;
    }

    pub fn refresh(&self) -> Result<Session, AuthError> {
        let mut held = self.lock();
        self.refresh_held(&mut held)
    }

    pub fn access_token(&self) -> Result<String, AuthError> {
        let mut held = self.lock();
        if let Some(token) = held
            .as_ref()
            .and_then(|session| session.access_token(SystemTime::now()))
        {
            return Ok(token.to_owned());
        }
        let fresh = self.refresh_held(&mut held)?;
        fresh
            .access_token(SystemTime::now())
            .map(str::to_owned)
            .ok_or_else(|| AuthError::Failed("WorkOS returned an expired access token".into()))
    }

    fn refresh_held(&self, held: &mut Option<Session>) -> Result<Session, AuthError> {
        let current = held
            .clone()
            .ok_or_else(|| AuthError::Failed("not signed in".into()))?;
        let fresh = match workos_api::refresh(&self.config, &current) {
            Ok(fresh) => fresh,
            Err(error) => {
                if matches!(error, AuthError::Rejected(_)) {
                    *held = None;
                }
                return Err(error);
            }
        };
        if let Some(store) = self.store.as_deref()
            && let Err(error) = store.save(&fresh)
        {
            eprintln!("missal: could not save the session: {error}");
        }
        *held = Some(fresh.clone());
        Ok(fresh)
    }
}
