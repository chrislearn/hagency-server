//! Hagency business operations mounted by hagency-server. Matrix is accessed
//! through HTTP; no Palpo internals or JavaScript backend are required.
pub mod api;
mod intents;
pub mod machine;
pub mod matrix;
pub mod notifications;
pub mod outbound;
pub mod store;
pub mod updates;
mod views;
pub mod workflow;

use serde_json::Value;

#[derive(Debug, thiserror::Error)]
#[error("{code}")]
pub struct Error {
    pub status: u16,
    pub code: String,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn fail(status: u16, code: &'static str) -> Error {
    Error {
        status,
        code: code.into(),
        message: code.into(),
    }
}

impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        fail(503, "workflow_store_unavailable")
    }
}

impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        fail(400, "invalid_arguments")
    }
}

impl From<hagency_contract::Error> for Error {
    fn from(value: hagency_contract::Error) -> Self {
        use hagency_contract::Error as C;
        match value {
            C::Forbidden | C::SelfApproval => fail(403, "coordinator_required"),
            C::Unsupported => fail(501, "coordinator_protocol_unavailable"),
            C::BindingMismatch => fail(409, "workflow_binding_changed"),
            C::Expired => fail(409, "workflow_authority_expired"),
            C::EngagementUnavailable => fail(409, "engagement_unavailable"),
            C::ProjectUnavailable => fail(409, "project_not_ready"),
            C::ResourceNotGranted => fail(403, "resource_not_granted"),
            C::Unallocated | C::InsufficientCapacity => fail(409, "insufficient_capacity"),
            _ => fail(400, "invalid_arguments"),
        }
    }
}

pub fn digest(value: &Value) -> Result<String> {
    Ok(hagency_contract::canonical::digest(value)?)
}

pub fn secret() -> String {
    rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
