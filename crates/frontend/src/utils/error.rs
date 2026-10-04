use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixError {
    pub errcode: String,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HttpError {
    pub message: String,
    pub status: u16,
    pub body: Option<MatrixError>,
    pub request_id: Option<String>,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref rid) = self.request_id {
            write!(f, "{} (ref: {})", self.message, rid)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for HttpError {}

impl HttpError {
    /// Build an `HttpError` from a free-form message with no HTTP status
    /// (used for client-side / transport / serialization failures).
    pub fn message(msg: impl Into<String>) -> Self {
        HttpError {
            message: msg.into(),
            status: 0,
            body: None,
            request_id: None,
        }
    }

    /// Build an `HttpError` carrying an HTTP status code and message.
    pub fn from_status(status: u16, msg: impl Into<String>) -> Self {
        HttpError {
            message: msg.into(),
            status,
            body: None,
            request_id: None,
        }
    }
}

pub fn display_error(errcode: &str, status: u16, message: &str) -> String {
    format!("{errcode} ({status}): {message}")
}
