use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    Offline(String),
    SignedOut,
    NotAllowed,
    NotFound,
    TooLarge,
    Failed(String),
}

impl LibraryError {
    pub(crate) fn from_status(status: u16, detail: String) -> LibraryError {
        match status {
            401 => LibraryError::SignedOut,
            403 => LibraryError::NotAllowed,
            404 => LibraryError::NotFound,
            413 => LibraryError::TooLarge,
            429 | 500.. => LibraryError::Offline(format!("the library answered {status}")),
            _ if detail.is_empty() => {
                LibraryError::Failed(format!("the library answered {status}"))
            }
            _ => LibraryError::Failed(detail),
        }
    }

    pub(crate) fn from_transport(error: ureq::Error) -> LibraryError {
        match error {
            ureq::Error::BodyExceedsLimit(_) => LibraryError::TooLarge,
            ureq::Error::Io(_)
            | ureq::Error::Timeout(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
            | ureq::Error::Tls(_) => LibraryError::Offline(error.to_string()),
            other => LibraryError::Failed(other.to_string()),
        }
    }
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::Offline(_) => write!(
                f,
                "Could not reach the template library. Check the internet connection and try again."
            ),
            LibraryError::SignedOut => write!(f, "Your session has ended. Sign in again."),
            LibraryError::NotAllowed => {
                write!(f, "This account is not allowed to publish templates.")
            }
            LibraryError::NotFound => {
                write!(f, "That template is no longer in the library.")
            }
            LibraryError::TooLarge => write!(f, "This template is too large for the library."),
            LibraryError::Failed(detail) => write!(f, "The template library failed: {detail}"),
        }
    }
}

impl std::error::Error for LibraryError {}
