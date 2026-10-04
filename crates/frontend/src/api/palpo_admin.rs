use serde::de::DeserializeOwned;

use crate::api::client::raw_fetch;
use crate::types::*;
use crate::utils::error::{HttpError, MatrixError};

/// Format a palpo_admin sidecar error. The sidecar returns plain text rather
/// than a Matrix errcode, so we surface the raw body keyed off the HTTP
/// status. 503 is treated specially as "server in maintenance mode" (mirrors
/// `pasion::format_pasion_error`).
fn format_palpo_error(status: u16, text: &str) -> (String, Option<MatrixError>) {
    if status == 503 {
        return ("Server is in maintenance mode".to_string(), None);
    }
    (format!("Palpo admin error ({status}): {text}"), None)
}

async fn palpo_admin_fetch<T: DeserializeOwned>(
    palpo_admin_url: &str,
    path: &str,
    method: &str,
    body: Option<String>,
) -> Result<T, HttpError> {
    let url = format!("{palpo_admin_url}{path}");
    raw_fetch::<T, _>(&url, method, body, format_palpo_error).await
}

pub async fn get_server_status(palpo_admin_url: &str) -> Result<ServerStatusResponse, HttpError> {
    palpo_admin_fetch(palpo_admin_url, "/status", "GET", None).await
}

pub async fn get_server_running_process(
    palpo_admin_url: &str,
) -> Result<ServerProcessResponse, HttpError> {
    palpo_admin_fetch(palpo_admin_url, "/lock", "GET", None).await
}

pub async fn get_server_notifications(
    palpo_admin_url: &str,
) -> Result<ServerNotificationsResponse, HttpError> {
    palpo_admin_fetch(palpo_admin_url, "/notifications", "GET", None).await
}

pub async fn delete_server_notifications(palpo_admin_url: &str) -> Result<(), HttpError> {
    let _: serde_json::Value =
        palpo_admin_fetch(palpo_admin_url, "/notifications", "DELETE", None).await?;
    Ok(())
}

pub async fn get_server_commands(
    palpo_admin_url: &str,
) -> Result<std::collections::HashMap<String, ServerCommand>, HttpError> {
    palpo_admin_fetch(palpo_admin_url, "/commands", "GET", None).await
}

pub async fn run_server_command(
    palpo_admin_url: &str,
    command: &str,
    additional_args: Option<serde_json::Value>,
) -> Result<serde_json::Value, HttpError> {
    let mut body = serde_json::json!({ "command": command });
    if let Some(args) = additional_args {
        body["additionalArgs"] = args;
    }
    palpo_admin_fetch(palpo_admin_url, "/commands", "POST", Some(body.to_string())).await
}

pub async fn get_scheduled_commands(
    palpo_admin_url: &str,
) -> Result<Vec<ScheduledCommand>, HttpError> {
    palpo_admin_fetch(palpo_admin_url, "/schedules", "GET", None).await
}

pub async fn create_scheduled_command(
    palpo_admin_url: &str,
    command: &ScheduledCommand,
) -> Result<ScheduledCommand, HttpError> {
    let body = serde_json::to_string(command).unwrap_or_default();
    palpo_admin_fetch(palpo_admin_url, "/schedules", "POST", Some(body)).await
}

pub async fn update_scheduled_command(
    palpo_admin_url: &str,
    command: &ScheduledCommand,
) -> Result<ScheduledCommand, HttpError> {
    let body = serde_json::to_string(command).unwrap_or_default();
    palpo_admin_fetch(palpo_admin_url, "/schedules", "PUT", Some(body)).await
}

pub async fn delete_scheduled_command(palpo_admin_url: &str, id: &str) -> Result<(), HttpError> {
    let _: serde_json::Value =
        palpo_admin_fetch(palpo_admin_url, &format!("/schedules/{id}"), "DELETE", None).await?;
    Ok(())
}

pub async fn get_recurring_commands(
    palpo_admin_url: &str,
) -> Result<Vec<RecurringCommand>, HttpError> {
    palpo_admin_fetch(palpo_admin_url, "/recurrings", "GET", None).await
}

pub async fn create_recurring_command(
    palpo_admin_url: &str,
    command: &RecurringCommand,
) -> Result<RecurringCommand, HttpError> {
    let body = serde_json::to_string(command).unwrap_or_default();
    palpo_admin_fetch(palpo_admin_url, "/recurrings", "POST", Some(body)).await
}

pub async fn update_recurring_command(
    palpo_admin_url: &str,
    command: &RecurringCommand,
) -> Result<RecurringCommand, HttpError> {
    let body = serde_json::to_string(command).unwrap_or_default();
    palpo_admin_fetch(palpo_admin_url, "/recurrings", "PUT", Some(body)).await
}

pub async fn delete_recurring_command(palpo_admin_url: &str, id: &str) -> Result<(), HttpError> {
    let _: serde_json::Value = palpo_admin_fetch(
        palpo_admin_url,
        &format!("/recurrings/{id}"),
        "DELETE",
        None,
    )
    .await?;
    Ok(())
}
