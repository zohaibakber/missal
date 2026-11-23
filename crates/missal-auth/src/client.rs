use std::time::{Duration, SystemTime};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ureq::Agent;
use ureq::http::Response;

use crate::config::Config;
use crate::error::AuthError;
use crate::session::{Account, ManagedAccount, Role, Session};

const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionReply {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    account: Account,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Problem {
    #[serde(rename = "_tag", default)]
    tag: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    retry_after: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CodePurpose {
    Verify,
    Reset,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserChanges {
    pub name: Option<String>,
    pub role: Option<Role>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUser {
    pub email: String,
    pub name: String,
    pub role: Role,
    pub password: String,
    pub months: u8,
}

fn problem(status: u16, body: &str) -> AuthError {
    let problem = serde_json::from_str::<Problem>(body).ok();
    match problem.as_ref().map(|problem| problem.tag.as_str()) {
        Some("InvalidCredentials") => AuthError::InvalidCredentials,
        Some("AccountLocked") => AuthError::Locked {
            retry_after: problem.map_or(0, |problem| problem.retry_after),
        },
        Some("SessionExpired" | "Unauthorized") => AuthError::SessionEnded,
        Some("WrongPassword") => AuthError::WrongPassword,
        Some("InvalidCode") => AuthError::InvalidCode,
        Some("EmailNotVerified") => AuthError::EmailNotVerified,
        Some("LicenceRequired") => AuthError::LicenceRequired,
        Some("OwnAccess") => AuthError::OwnAccess,
        Some("EmailTaken") => AuthError::EmailTaken,
        Some("NotAdmin" | "NotPublisher") => AuthError::NotAllowed,
        Some("UserNotFound") => AuthError::NotFound,
        _ => match status {
            400 => AuthError::Invalid,
            401 => AuthError::SessionEnded,
            402 => AuthError::LicenceRequired,
            403 => AuthError::NotAllowed,
            404 => AuthError::NotFound,
            429 | 500.. => AuthError::Offline(format!("the server answered {status}")),
            _ => AuthError::Failed(
                problem
                    .map(|problem| problem.error)
                    .filter(|error| !error.is_empty())
                    .unwrap_or_else(|| format!("the server answered {status}")),
            ),
        },
    }
}

fn transport(error: ureq::Error) -> AuthError {
    match error {
        ureq::Error::Io(_)
        | ureq::Error::Timeout(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::Tls(_) => AuthError::Offline(error.to_string()),
        other => AuthError::Failed(other.to_string()),
    }
}

fn valid_id(id: &str) -> Result<&str, AuthError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    ok.then_some(id).ok_or(AuthError::NotFound)
}

pub(crate) struct Api {
    base_url: String,
    agent: Agent,
}

enum Method {
    Get,
    Post,
    Patch,
    Delete,
}

impl Api {
    pub(crate) fn new(config: &Config) -> Api {
        let agent = Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .user_agent(format!("Missal/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        Api {
            base_url: config.base_url.clone(),
            agent,
        }
    }

    fn call(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Result<String, AuthError> {
        let url = format!("{}/v1/{path}", self.base_url);
        let bearer = token.map(|token| format!("Bearer {token}"));
        let response: Result<Response<ureq::Body>, ureq::Error> = match (method, body) {
            (Method::Delete, _) => {
                let mut request = self.agent.delete(&url);
                if let Some(bearer) = &bearer {
                    request = request.header("Authorization", bearer);
                }
                request.call()
            }
            (Method::Get, _) => {
                let mut request = self.agent.get(&url);
                if let Some(bearer) = &bearer {
                    request = request.header("Authorization", bearer);
                }
                request.call()
            }
            (method, body) => {
                let mut request = match method {
                    Method::Patch => self.agent.patch(&url),
                    _ => self.agent.post(&url),
                };
                if let Some(bearer) = &bearer {
                    request = request.header("Authorization", bearer);
                }
                let body = body.unwrap_or_else(|| json!({})).to_string();
                request
                    .content_type("application/json")
                    .send(body.as_bytes())
            }
        };
        let mut response = response.map_err(transport)?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().unwrap_or_default();
        if (200..300).contains(&status) {
            Ok(text)
        } else {
            Err(problem(status, &text))
        }
    }

    fn json<T: DeserializeOwned>(text: &str) -> Result<T, AuthError> {
        serde_json::from_str(text)
            .map_err(|error| AuthError::Failed(format!("unexpected reply: {error}")))
    }

    fn session(&self, path: &str, token: Option<&str>, body: Value) -> Result<Session, AuthError> {
        let reply: SessionReply = Self::json(&self.call(Method::Post, path, token, Some(body))?)?;
        Ok(Session::verified(
            reply.account,
            reply.refresh_token,
            reply.access_token,
            reply.expires_in,
            SystemTime::now(),
        ))
    }

    pub(crate) fn sign_in(&self, email: &str, password: &str) -> Result<Session, AuthError> {
        self.session(
            "auth/sign-in",
            None,
            json!({ "email": email.trim(), "password": password }),
        )
    }

    pub(crate) fn sign_up(&self, email: &str, name: &str, password: &str) -> Result<(), AuthError> {
        let body = json!({ "email": email.trim(), "name": name.trim(), "password": password });
        self.call(Method::Post, "auth/sign-up", None, Some(body))
            .map(|_| ())
    }

    pub(crate) fn request_code(&self, email: &str, purpose: CodePurpose) -> Result<(), AuthError> {
        let body = json!({ "email": email.trim(), "purpose": purpose });
        self.call(Method::Post, "auth/codes", None, Some(body))
            .map(|_| ())
    }

    pub(crate) fn verify_email(&self, email: &str, code: &str) -> Result<Session, AuthError> {
        self.session(
            "auth/verify-email",
            None,
            json!({ "email": email.trim(), "code": code.trim() }),
        )
    }

    pub(crate) fn reset_password(
        &self,
        email: &str,
        code: &str,
        password: &str,
    ) -> Result<Session, AuthError> {
        self.session(
            "auth/reset-password",
            None,
            json!({ "email": email.trim(), "code": code.trim(), "password": password }),
        )
    }

    pub(crate) fn refresh(&self, session: &Session) -> Result<Session, AuthError> {
        self.session(
            "auth/refresh",
            None,
            json!({ "refreshToken": session.refresh_token() }),
        )
    }

    pub(crate) fn sign_out(&self, session: &Session) -> Result<(), AuthError> {
        self.call(
            Method::Post,
            "auth/sign-out",
            None,
            Some(json!({ "refreshToken": session.refresh_token() })),
        )
        .map(|_| ())
    }

    pub(crate) fn change_password(
        &self,
        token: &str,
        current: &str,
        new: &str,
    ) -> Result<Session, AuthError> {
        self.session(
            "account/password",
            Some(token),
            json!({ "currentPassword": current, "newPassword": new }),
        )
    }

    pub(crate) fn users(&self, token: &str) -> Result<Vec<ManagedAccount>, AuthError> {
        Self::json(&self.call(Method::Get, "users", Some(token), None)?)
    }

    fn user_call(
        &self,
        method: Method,
        token: &str,
        id: &str,
        action: &str,
        body: Option<Value>,
    ) -> Result<String, AuthError> {
        let path = format!("users/{}{action}", valid_id(id)?);
        self.call(method, &path, Some(token), body)
    }

    pub(crate) fn create_user(
        &self,
        token: &str,
        user: &NewUser,
    ) -> Result<ManagedAccount, AuthError> {
        let body = json!({
            "email": user.email.trim(),
            "name": user.name.trim(),
            "role": user.role,
            "password": user.password,
            "months": user.months,
        });
        Self::json(&self.call(Method::Post, "users", Some(token), Some(body))?)
    }

    pub(crate) fn update_user(
        &self,
        token: &str,
        id: &str,
        changes: &UserChanges,
    ) -> Result<ManagedAccount, AuthError> {
        let mut body = serde_json::Map::new();
        if let Some(name) = &changes.name {
            body.insert("name".into(), json!(name.trim()));
        }
        if let Some(role) = changes.role {
            body.insert("role".into(), json!(role));
        }
        if let Some(disabled) = changes.disabled {
            body.insert("disabled".into(), json!(disabled));
        }
        Self::json(&self.user_call(Method::Patch, token, id, "", Some(Value::Object(body)))?)
    }

    pub(crate) fn delete_user(&self, token: &str, id: &str) -> Result<(), AuthError> {
        self.user_call(Method::Delete, token, id, "", None)
            .map(|_| ())
    }

    pub(crate) fn verify_user(&self, token: &str, id: &str) -> Result<ManagedAccount, AuthError> {
        Self::json(&self.user_call(Method::Post, token, id, "/verify", None)?)
    }

    pub(crate) fn grant_licence(
        &self,
        token: &str,
        id: &str,
        months: u8,
    ) -> Result<ManagedAccount, AuthError> {
        let body = json!({ "months": months });
        Self::json(&self.user_call(Method::Post, token, id, "/licence", Some(body))?)
    }

    pub(crate) fn end_licence(&self, token: &str, id: &str) -> Result<ManagedAccount, AuthError> {
        Self::json(&self.user_call(Method::Delete, token, id, "/licence", None)?)
    }

    pub(crate) fn extend_trial(
        &self,
        token: &str,
        id: &str,
        days: u8,
    ) -> Result<ManagedAccount, AuthError> {
        let body = json!({ "days": days });
        Self::json(&self.user_call(Method::Post, token, id, "/trial", Some(body))?)
    }

    pub(crate) fn set_password(
        &self,
        token: &str,
        id: &str,
        password: &str,
    ) -> Result<ManagedAccount, AuthError> {
        let body = json!({ "password": password });
        Self::json(&self.user_call(Method::Post, token, id, "/password", Some(body))?)
    }

    pub(crate) fn send_reset(&self, token: &str, id: &str) -> Result<(), AuthError> {
        self.user_call(Method::Post, token, id, "/reset-email", None)
            .map(|_| ())
    }

    pub(crate) fn sign_out_user(&self, token: &str, id: &str) -> Result<ManagedAccount, AuthError> {
        Self::json(&self.user_call(Method::Post, token, id, "/sign-out", None)?)
    }
}
