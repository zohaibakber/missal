use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    Offline(String),
    Rejected(String),
    Cancelled,
    TimedOut,
    Failed(String),
}

impl AuthError {
    pub(crate) fn from_workos(error: workos::Error) -> AuthError {
        match &error {
            workos::Error::Network(_) => AuthError::Offline(error.to_string()),
            workos::Error::Api(api) if api.status == 429 || api.status >= 500 => {
                AuthError::Offline(error.to_string())
            }
            workos::Error::Api(api) if (400..500).contains(&api.status) => {
                AuthError::Rejected(api.message.clone())
            }
            _ => AuthError::Failed(error.to_string()),
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::Offline(detail) => write!(f, "could not reach WorkOS: {detail}"),
            AuthError::Rejected(detail) => write!(f, "WorkOS rejected the session: {detail}"),
            AuthError::Cancelled => write!(f, "sign-in was cancelled"),
            AuthError::TimedOut => write!(f, "sign-in timed out"),
            AuthError::Failed(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for AuthError {}
