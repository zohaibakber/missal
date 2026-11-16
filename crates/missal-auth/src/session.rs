use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const OFFLINE_GRACE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const ACCESS_MARGIN: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Publisher,
    Member,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Member, Role::Publisher, Role::Admin];

    pub fn can_publish(self) -> bool {
        matches!(self, Role::Admin | Role::Publisher)
    }

    pub fn is_admin(self) -> bool {
        self == Role::Admin
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Admin => "Administrator",
            Role::Publisher => "Publisher",
            Role::Member => "Member",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenceStatus {
    Trial,
    Licensed,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Licence {
    pub status: LicenceStatus,
    #[serde(rename = "until")]
    pub until_ms: i64,
}

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

fn millis(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH).map_or(0, |since| {
        i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
    })
}

impl Licence {
    pub fn active(&self, now: SystemTime) -> bool {
        self.status != LicenceStatus::Expired && self.until_ms > millis(now)
    }

    pub fn days_left(&self, now: SystemTime) -> i64 {
        if !self.active(now) {
            return 0;
        }
        (self.until_ms - millis(now) + DAY_MS - 1) / DAY_MS
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: Role,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub created_at: i64,
    pub licence: Licence,
}

impl Account {
    pub fn display_name(&self) -> &str {
        let name = self.name.trim();
        if name.is_empty() { &self.email } else { name }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct AccessToken {
    token: String,
    expires_at: u64,
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
    pub(crate) fn verified(
        account: Account,
        refresh_token: String,
        access_token: String,
        expires_in: u64,
        at: SystemTime,
    ) -> Session {
        let verified_at = seconds(at);
        Session {
            account,
            refresh_token,
            verified_at,
            access: Some(AccessToken {
                token: access_token,
                expires_at: verified_at + expires_in,
            }),
        }
    }

    pub(crate) fn refresh_token(&self) -> &str {
        &self.refresh_token
    }

    pub fn access_token(&self, now: SystemTime) -> Option<&str> {
        self.access
            .as_ref()
            .filter(|access| access.expires_at > seconds(now) + ACCESS_MARGIN)
            .map(|access| access.token.as_str())
    }

    pub fn verified_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.verified_at)
    }

    pub fn offline_until(&self) -> SystemTime {
        self.verified_at() + OFFLINE_GRACE
    }

    pub fn usable_offline(&self, now: SystemTime) -> bool {
        let verified = self.verified_at();
        now >= verified && now < self.offline_until() && self.account.licence.active(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAR: i64 = 253_402_300_799_000;

    fn account(licence: Licence) -> Account {
        Account {
            id: "user_1".into(),
            email: "a@b.c".into(),
            name: String::new(),
            role: Role::Member,
            disabled: false,
            created_at: 0,
            licence,
        }
    }

    fn session_at(at: SystemTime) -> Session {
        let licence = Licence {
            status: LicenceStatus::Licensed,
            until_ms: FAR,
        };
        Session::verified(account(licence), "refresh".into(), "access".into(), 900, at)
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
        assert!(!session_at(verified).usable_offline(verified - Duration::from_secs(60)));
    }

    #[test]
    fn offline_use_ends_when_the_licence_ends() {
        let verified = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let until = verified + Duration::from_secs(3 * 24 * 60 * 60);
        let trial = Licence {
            status: LicenceStatus::Trial,
            until_ms: super::millis(until),
        };
        let session = Session::verified(account(trial), "r".into(), "a".into(), 900, verified);
        assert!(session.usable_offline(until - Duration::from_secs(1)));
        assert!(!session.usable_offline(until));
        let expired = Licence {
            status: LicenceStatus::Expired,
            until_ms: FAR,
        };
        let session = Session::verified(account(expired), "r".into(), "a".into(), 900, verified);
        assert!(!session.usable_offline(verified));
    }

    #[test]
    fn days_left_rounds_a_started_day_up_and_stops_at_zero() {
        let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let trial = |hours: u64| Licence {
            status: LicenceStatus::Trial,
            until_ms: super::millis(now + Duration::from_secs(hours * 60 * 60)),
        };
        assert_eq!(trial(7 * 24).days_left(now), 7);
        assert_eq!(trial(25).days_left(now), 2);
        assert_eq!(trial(1).days_left(now), 1);
        assert_eq!(trial(0).days_left(now), 0);
    }

    #[test]
    fn an_access_token_is_used_until_shortly_before_it_expires() {
        let issued = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let session = session_at(issued);
        assert_eq!(session.access_token(issued), Some("access"));
        assert!(
            session
                .access_token(issued + Duration::from_secs(880))
                .is_none()
        );
    }

    #[test]
    fn tokens_stay_out_of_debug_output_and_the_access_token_out_of_the_keyring() {
        let session = session_at(UNIX_EPOCH);
        let debug = format!("{session:?}");
        assert!(!debug.contains("refresh") && !debug.contains("access\""));
        assert!(
            !serde_json::to_string(&session)
                .unwrap()
                .contains("\"access")
        );
    }

    #[test]
    fn accounts_read_the_server_shape() {
        let account: Account = serde_json::from_str(
            r#"{"id":"u","email":"e@x.y","name":"","role":"publisher","disabled":false,"createdAt":5,"licence":{"status":"trial","until":1790000000000}}"#,
        )
        .unwrap();
        assert_eq!(account.role, Role::Publisher);
        assert_eq!(
            account.licence,
            Licence {
                status: LicenceStatus::Trial,
                until_ms: 1_790_000_000_000
            }
        );
        assert_eq!(account.display_name(), "e@x.y");
    }
}
