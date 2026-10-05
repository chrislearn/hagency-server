mod accounts;
mod api;
mod fleet;
mod native_client;
mod outbound;
mod store;
mod upstream;
mod workflow;
pub use api::Admin;
pub use store::Store;
pub use upstream::{ApiError, Upstream};

use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub(crate) type Result<T> = std::result::Result<T, ApiError>;
pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
pub(crate) fn millis() -> i64 {
    Utc::now().timestamp_millis()
}
pub(crate) fn timestamp(v: &Value) -> i64 {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.timestamp_millis())
        .unwrap_or(0)
}
pub(crate) fn secret() -> String {
    use base64::Engine;
    let bytes: [u8; 32] = rand::random();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
pub(crate) fn hash(v: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(v.as_ref()))
}
pub(crate) fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let sorted: std::collections::BTreeMap<_, _> =
                m.iter().map(|(k, v)| (k.clone(), canonical(v))).collect();
            serde_json::to_value(sorted).unwrap()
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        _ => v.clone(),
    }
}
pub(crate) fn digest(v: &Value) -> String {
    hash(serde_json::to_vec(v).expect("JSON value"))
}
pub(crate) fn canonical_digest(v: &Value) -> String {
    digest(&canonical(v))
}
pub(crate) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}
pub(crate) fn err(status: u16, code: &str, message: &str) -> ApiError {
    ApiError::new(status, code, message)
}
pub(crate) fn field(v: &Value, name: &str, max: usize) -> Result<String> {
    let s = v.as_str().unwrap_or("");
    if s.trim().is_empty() || s.chars().count() > max {
        return Err(err(
            400,
            "invalid_input",
            &format!("{name} is required (maximum {max} characters)."),
        ));
    }
    Ok(s.trim().into())
}
pub(crate) fn key(v: &Value, name: &str) -> Result<String> {
    let s = field(v, name, 80)?;
    if !s
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(err(
            400,
            "invalid_input",
            "Use letters, numbers, underscores or hyphens.",
        ));
    }
    Ok(s)
}
pub(crate) fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}
pub(crate) fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
pub(crate) fn object_values(v: &Value) -> Vec<Value> {
    v.as_object()
        .map(|m| m.values().cloned().collect())
        .unwrap_or_default()
}
pub(crate) fn audit(
    state: &mut Value,
    actor: &str,
    action: &str,
    fleet: Option<&str>,
    object: &str,
    result: &str,
) {
    state["audit"].as_array_mut().unwrap().push(json!({"at":now(),"actor":actor,"action":action,"fleetId":fleet,"objectId":object,"result":result}));
}
