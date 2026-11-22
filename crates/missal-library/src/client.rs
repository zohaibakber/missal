use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use ureq::Agent;
use ureq::http::Response;

use crate::error::LibraryError;

const TIMEOUT: Duration = Duration::from_secs(30);
const CONTENT_LIMIT: u64 = 16 * 1024 * 1024;
const VERSION_HEADER: &str = "x-missal-version";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub field_count: i64,
    #[serde(default)]
    pub preview: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Catalog {
    pub publisher: bool,
    pub templates: Vec<LibraryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedTemplate {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub field_count: i64,
    #[serde(default)]
    pub preview: String,
    pub size: i64,
    pub publisher_name: Option<String>,
    pub publisher_email: Option<String>,
    pub version_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVersion {
    pub version: i64,
    pub name: String,
    pub field_count: i64,
    pub size: i64,
    pub publisher_name: Option<String>,
    pub publisher_email: Option<String>,
    pub published_at: i64,
    pub restored_from: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub version: i64,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Published {
    pub id: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upload {
    pub name: String,
    pub field_count: i64,
    pub preview: String,
    pub content: Vec<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadBody<'a> {
    name: &'a str,
    field_count: i64,
    preview: &'a str,
    content: String,
}

#[derive(Deserialize)]
struct Problem {
    #[serde(default)]
    error: String,
}

fn valid_id(id: &str) -> Result<&str, LibraryError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    ok.then_some(id).ok_or(LibraryError::NotFound)
}

pub struct Library {
    base_url: String,
    agent: Agent,
}

impl Library {
    pub fn new(base_url: &str) -> Library {
        let agent = Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .user_agent(format!("Missal/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        Library {
            base_url: base_url.trim_end_matches('/').to_owned(),
            agent,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/v1/{path}", self.base_url)
    }

    fn checked(
        response: Result<Response<ureq::Body>, ureq::Error>,
    ) -> Result<Response<ureq::Body>, LibraryError> {
        let mut response = response.map_err(LibraryError::from_transport)?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(response);
        }
        let detail = response
            .body_mut()
            .read_to_string()
            .ok()
            .and_then(|text| serde_json::from_str::<Problem>(&text).ok())
            .map(|problem| problem.error)
            .unwrap_or_default();
        Err(LibraryError::from_status(status, detail))
    }

    fn json<T: for<'de> Deserialize<'de>>(
        mut response: Response<ureq::Body>,
    ) -> Result<T, LibraryError> {
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(LibraryError::from_transport)?;
        serde_json::from_str(&text)
            .map_err(|error| LibraryError::Failed(format!("unexpected reply: {error}")))
    }

    pub fn catalog(&self, token: &str) -> Result<Catalog, LibraryError> {
        let response = self
            .agent
            .get(self.url("templates"))
            .header("Authorization", format!("Bearer {token}"))
            .call();
        Self::json(Self::checked(response)?)
    }

    pub fn download(&self, token: &str, id: &str) -> Result<Download, LibraryError> {
        let id = valid_id(id)?;
        let mut response = Self::checked(
            self.agent
                .get(self.url(&format!("templates/{id}/content")))
                .header("Authorization", format!("Bearer {token}"))
                .call(),
        )?;
        let version = response
            .headers()
            .get(VERSION_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| LibraryError::Failed("the download has no version".into()))?;
        let content = response
            .body_mut()
            .with_config()
            .limit(CONTENT_LIMIT)
            .read_to_vec()
            .map_err(LibraryError::from_transport)?;
        Ok(Download { version, content })
    }

    pub fn publish(
        &self,
        token: &str,
        id: Option<&str>,
        upload: &Upload,
    ) -> Result<Published, LibraryError> {
        if upload.content.len() as u64 > CONTENT_LIMIT {
            return Err(LibraryError::TooLarge);
        }
        let body = serde_json::to_vec(&UploadBody {
            name: &upload.name,
            field_count: upload.field_count,
            preview: &upload.preview,
            content: STANDARD.encode(&upload.content),
        })
        .map_err(|error| LibraryError::Failed(error.to_string()))?;
        let request = match id {
            Some(id) => self
                .agent
                .put(self.url(&format!("templates/{}", valid_id(id)?))),
            None => self.agent.post(self.url("templates")),
        };
        let response = request
            .header("Authorization", format!("Bearer {token}"))
            .content_type("application/json")
            .send(&body[..]);
        Self::json(Self::checked(response)?)
    }

    pub fn managed_templates(&self, token: &str) -> Result<Vec<ManagedTemplate>, LibraryError> {
        let response = self
            .agent
            .get(self.url("admin/templates"))
            .header("Authorization", format!("Bearer {token}"))
            .call();
        Self::json(Self::checked(response)?)
    }

    pub fn template_versions(
        &self,
        token: &str,
        id: &str,
    ) -> Result<Vec<TemplateVersion>, LibraryError> {
        let response = self
            .agent
            .get(self.url(&format!("admin/templates/{}/versions", valid_id(id)?)))
            .header("Authorization", format!("Bearer {token}"))
            .call();
        Self::json(Self::checked(response)?)
    }

    pub fn restore_version(
        &self,
        token: &str,
        id: &str,
        version: i64,
    ) -> Result<Published, LibraryError> {
        let body = serde_json::json!({ "version": version }).to_string();
        let response = self
            .agent
            .post(self.url(&format!("admin/templates/{}/restore", valid_id(id)?)))
            .header("Authorization", format!("Bearer {token}"))
            .content_type("application/json")
            .send(body.as_bytes());
        Self::json(Self::checked(response)?)
    }

    pub fn unpublish(&self, token: &str, id: &str) -> Result<(), LibraryError> {
        let response = self
            .agent
            .delete(self.url(&format!("templates/{}", valid_id(id)?)))
            .header("Authorization", format!("Bearer {token}"))
            .call();
        Self::checked(response).map(|_| ())
    }
}
