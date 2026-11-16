use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use crate::client::{Api, NewUser, UserChanges};
use crate::config::Config;
use crate::error::AuthError;
use crate::session::{Account, Session};
use crate::store::SessionStore;

pub struct SessionKeeper {
    api: Api,
    store: Option<Arc<SessionStore>>,
    session: Mutex<Option<Session>>,
}

impl SessionKeeper {
    pub fn new(config: &Config, store: Option<Arc<SessionStore>>) -> SessionKeeper {
        SessionKeeper {
            api: Api::new(config),
            store,
            session: Mutex::new(None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Session>> {
        self.session
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn save(&self, session: &Session) {
        if let Some(store) = self.store.as_deref()
            && let Err(error) = store.save(session)
        {
            eprintln!("missal: could not save the session: {error}");
        }
    }

    fn forget(&self, held: &mut Option<Session>) {
        *held = None;
        if let Some(store) = self.store.as_deref()
            && let Err(error) = store.clear()
        {
            eprintln!("missal: could not remove the saved session: {error}");
        }
    }

    pub fn replace(&self, session: Option<Session>) {
        *self.lock() = session;
    }

    pub fn sign_in(&self, email: &str, password: &str) -> Result<Session, AuthError> {
        let session = self.api.sign_in(email, password)?;
        let mut held = self.lock();
        self.save(&session);
        *held = Some(session.clone());
        Ok(session)
    }

    pub fn sign_out(&self) {
        let mut held = self.lock();
        if let Some(session) = held.as_ref()
            && let Err(error) = self.api.sign_out(session)
        {
            eprintln!("missal: could not end the session on the server: {error}");
        }
        self.forget(&mut held);
    }

    pub fn refresh(&self) -> Result<Session, AuthError> {
        let mut held = self.lock();
        self.refresh_held(&mut held)
    }

    pub fn access_token(&self) -> Result<String, AuthError> {
        let mut held = self.lock();
        self.token_held(&mut held)
    }

    pub fn change_password(&self, current: &str, new: &str) -> Result<Session, AuthError> {
        let mut held = self.lock();
        let token = self.token_held(&mut held)?;
        let session = self.api.change_password(&token, current, new)?;
        self.save(&session);
        *held = Some(session.clone());
        Ok(session)
    }

    pub fn users(&self) -> Result<Vec<Account>, AuthError> {
        let token = self.access_token()?;
        self.api.users(&token)
    }

    pub fn create_user(&self, user: &NewUser) -> Result<Account, AuthError> {
        let token = self.access_token()?;
        self.api.create_user(&token, user)
    }

    pub fn update_user(&self, id: &str, changes: &UserChanges) -> Result<Account, AuthError> {
        let token = self.access_token()?;
        self.api.update_user(&token, id, changes)
    }

    pub fn reset_password(&self, id: &str, password: &str) -> Result<Account, AuthError> {
        let token = self.access_token()?;
        self.api.reset_password(&token, id, password)
    }

    fn token_held(&self, held: &mut Option<Session>) -> Result<String, AuthError> {
        if let Some(token) = held
            .as_ref()
            .and_then(|session| session.access_token(SystemTime::now()))
        {
            return Ok(token.to_owned());
        }
        let fresh = self.refresh_held(held)?;
        fresh
            .access_token(SystemTime::now())
            .map(str::to_owned)
            .ok_or_else(|| AuthError::Failed("the server issued an expired access token".into()))
    }

    fn refresh_held(&self, held: &mut Option<Session>) -> Result<Session, AuthError> {
        let current = held.clone().ok_or(AuthError::SessionEnded)?;
        match self.api.refresh(&current) {
            Ok(fresh) => {
                self.save(&fresh);
                *held = Some(fresh.clone());
                Ok(fresh)
            }
            Err(error) => {
                if error.ends_session() {
                    self.forget(held);
                }
                Err(error)
            }
        }
    }
}
