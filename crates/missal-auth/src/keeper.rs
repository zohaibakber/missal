use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use crate::client::{Api, CodePurpose, NewUser, UserChanges};
use crate::config::Config;
use crate::error::AuthError;
use crate::session::{ManagedAccount, Session};
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

    fn hold(&self, session: Result<Session, AuthError>) -> Result<Session, AuthError> {
        let session = session?;
        let mut held = self.lock();
        self.save(&session);
        *held = Some(session.clone());
        Ok(session)
    }

    pub fn sign_in(&self, email: &str, password: &str) -> Result<Session, AuthError> {
        self.hold(self.api.sign_in(email, password))
    }

    pub fn sign_up(&self, email: &str, name: &str, password: &str) -> Result<(), AuthError> {
        self.api.sign_up(email, name, password)
    }

    pub fn request_code(&self, email: &str, purpose: CodePurpose) -> Result<(), AuthError> {
        self.api.request_code(email, purpose)
    }

    pub fn verify_email(&self, email: &str, code: &str) -> Result<Session, AuthError> {
        self.hold(self.api.verify_email(email, code))
    }

    pub fn reset_password(
        &self,
        email: &str,
        code: &str,
        password: &str,
    ) -> Result<Session, AuthError> {
        self.hold(self.api.reset_password(email, code, password))
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

    pub fn users(&self) -> Result<Vec<ManagedAccount>, AuthError> {
        let token = self.access_token()?;
        self.api.users(&token)
    }

    pub fn create_user(&self, user: &NewUser) -> Result<ManagedAccount, AuthError> {
        let token = self.access_token()?;
        self.api.create_user(&token, user)
    }

    pub fn update_user(
        &self,
        id: &str,
        changes: &UserChanges,
    ) -> Result<ManagedAccount, AuthError> {
        let token = self.access_token()?;
        self.api.update_user(&token, id, changes)
    }

    pub fn delete_user(&self, id: &str) -> Result<(), AuthError> {
        let token = self.access_token()?;
        self.api.delete_user(&token, id)
    }

    pub fn verify_user(&self, id: &str) -> Result<ManagedAccount, AuthError> {
        let token = self.access_token()?;
        self.api.verify_user(&token, id)
    }

    pub fn grant_licence(&self, id: &str, months: u8) -> Result<ManagedAccount, AuthError> {
        let token = self.access_token()?;
        self.api.grant_licence(&token, id, months)
    }

    pub fn end_licence(&self, id: &str) -> Result<ManagedAccount, AuthError> {
        let token = self.access_token()?;
        self.api.end_licence(&token, id)
    }

    pub fn extend_trial(&self, id: &str, days: u8) -> Result<ManagedAccount, AuthError> {
        let token = self.access_token()?;
        self.api.extend_trial(&token, id, days)
    }

    pub fn set_password(&self, id: &str, password: &str) -> Result<ManagedAccount, AuthError> {
        let token = self.access_token()?;
        self.api.set_password(&token, id, password)
    }

    pub fn send_reset(&self, id: &str) -> Result<(), AuthError> {
        let token = self.access_token()?;
        self.api.send_reset(&token, id)
    }

    pub fn sign_out_user(&self, id: &str) -> Result<ManagedAccount, AuthError> {
        let token = self.access_token()?;
        self.api.sign_out_user(&token, id)
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
