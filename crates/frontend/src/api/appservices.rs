//! Palpo admin appservice management.
//!
//! Wraps the `/_palpo/admin/v1/appservices` endpoints added in palpo PR #119.
//! These endpoints are mounted under the homeserver itself (not the separate
//! palpo_admin sidecar), so we use the standard `api_client` which targets the
//! same-origin Nginx proxy and reuses the user's Matrix access token.

use crate::api::client::api_client;
use crate::types::{AppserviceRegistration, AppserviceSummary, ListAppservicesResponse};
use crate::utils::error::HttpError;

const BASE: &str = "/_palpo/admin/v1/appservices";

pub async fn list_appservices() -> Result<Vec<AppserviceSummary>, HttpError> {
    let resp: ListAppservicesResponse = api_client(BASE, "GET", None).await?;
    Ok(resp.appservices)
}

pub async fn get_appservice(id: &str) -> Result<AppserviceRegistration, HttpError> {
    let path = format!("{BASE}/{}", urlencoding::encode(id));
    api_client(&path, "GET", None).await
}

pub async fn register_appservice(
    reg: &AppserviceRegistration,
) -> Result<serde_json::Value, HttpError> {
    let body = serde_json::to_string(reg)
        .map_err(|e| HttpError::message(format!("Failed to serialize registration: {e}")))?;
    api_client(BASE, "POST", Some(body)).await
}

/// Replace an existing registration in place. The `id` in `reg` selects the
/// target; the homeserver treats the path id as authoritative and preserves
/// the administrative `disabled` flag.
pub async fn update_appservice(
    id: &str,
    reg: &AppserviceRegistration,
) -> Result<serde_json::Value, HttpError> {
    let path = format!("{BASE}/{}", urlencoding::encode(id));
    let body = serde_json::to_string(reg)
        .map_err(|e| HttpError::message(format!("Failed to serialize registration: {e}")))?;
    api_client(&path, "PUT", Some(body)).await
}

pub async fn delete_appservice(id: &str) -> Result<(), HttpError> {
    let path = format!("{BASE}/{}", urlencoding::encode(id));
    let _: serde_json::Value = api_client(&path, "DELETE", None).await?;
    Ok(())
}

pub async fn disable_appservice(id: &str) -> Result<(), HttpError> {
    let path = format!("{BASE}/{}/disable", urlencoding::encode(id));
    let _: serde_json::Value = api_client(&path, "POST", None).await?;
    Ok(())
}

pub async fn enable_appservice(id: &str) -> Result<(), HttpError> {
    let path = format!("{BASE}/{}/enable", urlencoding::encode(id));
    let _: serde_json::Value = api_client(&path, "POST", None).await?;
    Ok(())
}
