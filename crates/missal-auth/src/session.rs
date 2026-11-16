use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};

pub const OFFLINE_GRACE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

impl Account {
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.email)
    }

    pub(crate) fn from_user(user: &workos::User) -> Account {
        let joined = [user.first_name.as_deref(), user.last_name.as_deref()]
            .into_iter()
            .flatten()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let name = user
            .name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .or((!joined.is_empty()).then_some(joined));
        Account {
            id: user.id.clone(),
            email: user.email.clone(),
            name,
        }
    }
}

const ACCESS_MARGIN: u64 = 30;
const ACCESS_FALLBACK: u64 = 60;

#[derive(Clone, PartialEq, Eq)]
struct AccessToken {
    token: String,
    expires_at: u64,
}

fn token_expiry(token: &str) -> Option<u64> {
    let payload = URL_SAFE_NO_PAD.decode(token.split('.').nth(1)?).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    claims.get("exp")?.as_u64()
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub account: Account,
    refresh_token: String,
    verified_at: u64,
    #[serde(skip)]
    access: Option<AccessToken>,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("account", &self.account)
            .field("verified_at", &self.verified_at)
            .finish_non_exhaustive()
    }
}

fn seconds(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

impl Session {
    pub(crate) fn verified(account: Account, refresh_token: String, at: SystemTime) -> Session {
        Session {
            account,
            refresh_token,
            verified_at: seconds(at),
            access: None,
        }
    }

    pub(crate) fn with_access_token(mut self, token: String) -> Session {
        let expires_at = token_expiry(&token).unwrap_or(self.verified_at + ACCESS_FALLBACK);
        self.access = Some(AccessToken { token, expires_at });
        self
    }

    pub fn access_token(&self, now: SystemTime) -> Option<&str> {
        self.access
            .as_ref()
            .filter(|access| access.expires_at > seconds(now) + ACCESS_MARGIN)
            .map(|access| access.token.as_str())
    }

    pub(crate) fn refresh_token(&self) -> &str {
        &self.refresh_token
    }

    pub fn verified_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.verified_at)
    }

    pub fn offline_until(&self) -> SystemTime {
        self.verified_at() + OFFLINE_GRACE
    }

    pub fn usable_offline(&self, now: SystemTime) -> bool {
        let verified = self.verified_at();
        now >= verified && now < self.offline_until()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_at(at: SystemTime) -> Session {
        Session::verified(
            Account {
                id: "user_1".into(),
                email: "a@b.c".into(),
                name: None,
            },
            "token".into(),
            at,
        )
    }

    #[test]
    fn offline_use_ends_after_the_grace_period() {
        let verified = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let session = session_at(verified);
        assert!(session.usable_offline(verified + Duration::from_secs(29 * 24 * 60 * 60)));
        assert!(!session.usable_offline(verified + OFFLINE_GRACE));
    }

    #[test]
    fn a_clock_set_before_the_last_check_does_not_extend_offline_use() {
        let verified = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let session = session_at(verified);
        assert!(!session.usable_offline(verified - Duration::from_secs(60)));
    }

    #[test]
    fn an_access_token_is_used_until_shortly_before_it_expires() {
        let issued = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let claims = URL_SAFE_NO_PAD.encode(br#"{"sub":"user_1","exp":1800000300}"#);
        let session = session_at(issued).with_access_token(format!("e30.{claims}.sig"));
        assert!(session.access_token(issued).is_some());
        assert!(
            session
                .access_token(issued + Duration::from_secs(280))
                .is_none()
        );
        let opaque = session_at(issued).with_access_token("opaque".into());
        assert!(
            opaque
                .access_token(issued + Duration::from_secs(20))
                .is_some()
        );
        assert!(
            opaque
                .access_token(issued + Duration::from_secs(40))
                .is_none()
        );
    }

    #[test]
    fn the_access_token_is_never_written_to_the_keyring() {
        let session = session_at(UNIX_EPOCH).with_access_token("secret-access".into());
        assert!(
            !serde_json::to_string(&session)
                .unwrap()
                .contains("secret-access")
        );
    }

    #[test]
    fn debug_output_hides_the_refresh_token() {
        let session = session_at(UNIX_EPOCH);
        assert!(!format!("{session:?}").contains("token\""));
    }
}
