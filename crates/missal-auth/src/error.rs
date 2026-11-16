use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    Offline(String),
    InvalidCredentials,
    Locked { retry_after: u64 },
    SessionEnded,
    PasswordChangeRequired,
    WrongPassword,
    EmailTaken,
    OwnAccess,
    NotAllowed,
    NotFound,
    Invalid,
    Failed(String),
}

impl AuthError {
    pub fn ends_session(&self) -> bool {
        matches!(self, AuthError::SessionEnded)
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::Offline(_) => write!(
                f,
                "Could not reach Missal's server. Check the internet connection and try again."
            ),
            AuthError::InvalidCredentials => write!(f, "Wrong email or password."),
            AuthError::Locked { retry_after } => {
                let minutes = retry_after.div_ceil(60).max(1);
                let unit = if minutes == 1 { "minute" } else { "minutes" };
                write!(
                    f,
                    "Too many failed sign-ins. Try again in {minutes} {unit}."
                )
            }
            AuthError::SessionEnded => write!(f, "Your session has ended. Sign in again."),
            AuthError::PasswordChangeRequired => write!(f, "Choose a new password first."),
            AuthError::WrongPassword => write!(f, "The current password is wrong."),
            AuthError::EmailTaken => write!(f, "An account with this email already exists."),
            AuthError::OwnAccess => {
                write!(f, "You cannot remove your own administrator access.")
            }
            AuthError::NotAllowed => write!(f, "This account is not allowed to do that."),
            AuthError::NotFound => write!(f, "That account no longer exists."),
            AuthError::Invalid => write!(f, "Check the details and try again."),
            AuthError::Failed(detail) => write!(f, "Something went wrong: {detail}"),
        }
    }
}

impl std::error::Error for AuthError {}
