mod format;
mod package;
mod props;
mod styles;
mod xml;

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    NotDocx,
    MissingDocument,
    InvalidDocument(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::NotDocx => f.write_str("This file is not a readable Word (.docx) document."),
            ImportError::MissingDocument => f.write_str("This Word file has no document body."),
            ImportError::InvalidDocument(reason) => {
                write!(f, "This Word file's document XML is unreadable: {reason}")
            }
        }
    }
}

impl std::error::Error for ImportError {}
