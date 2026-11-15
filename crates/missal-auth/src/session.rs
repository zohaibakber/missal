use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub account: Account,
    refresh_token: String,
    verified_at: u64,
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
        }
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
    fn debug_output_hides_the_refresh_token() {
        let session = session_at(UNIX_EPOCH);
        assert!(!format!("{session:?}").contains("token\""));
    }
}
