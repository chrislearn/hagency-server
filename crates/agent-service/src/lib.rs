//! New Agent-domain identity. No Fleet credentials or legacy storage imports.
pub mod api;
#[cfg(test)]
mod api_discovery_tests;
mod api_transport;
pub mod appservice;
pub mod domain;
pub mod gateway;
pub mod identity;
pub mod matrix_client;
pub mod store;
pub mod transport;
mod workers;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("{0}")]
    Unauthorized(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    #[error("{0}")]
    NotFound(&'static str),
    #[error("{0}")]
    Unavailable(&'static str),
    #[error("storage_unavailable")]
    Database(#[from] diesel::result::Error),
}
impl Error {
    pub fn status(&self) -> u16 {
        match self {
            Self::Invalid(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Conflict(_) => 409,
            Self::NotFound(_) => 404,
            Self::Unavailable(_) | Self::Database(_) => 503,
        }
    }
}
pub(crate) fn token() -> String {
    hex::encode(rand::random::<[u8; 32]>())
}
pub(crate) fn hash(value: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub(crate) fn key(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(Error::Invalid("invalid_identifier"));
    }
    Ok(())
}
