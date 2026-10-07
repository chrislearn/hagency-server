//! New Hagency server domain. No legacy database, execution runtime or model secrets.
pub mod authorization;
pub mod model;
pub mod store;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid input: {0}")]
    Invalid(&'static str),
    #[error("{0}")]
    Denied(&'static str),
    #[error("object not found in this user's scope")]
    NotFound,
    #[error("idempotency conflict")]
    Conflict,
    #[error("stale policy revision")]
    StaleRevision,
    #[error("unsupported or foreign database; use a new state directory")]
    ForeignDatabase,
    #[error("storage operation failed")]
    Storage(#[from] rusqlite::Error),
    #[error("state directory is unavailable")]
    Io(#[from] std::io::Error),
    #[error("random source failed")]
    Random,
    #[error("invalid persisted state")]
    Encoding(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn identifier(prefix: &str) -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Random)?;
    Ok(format!("{prefix}{}", bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()))
}
