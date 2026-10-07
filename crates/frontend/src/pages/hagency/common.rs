pub use crate::api::browser_auth;
use dioxus::prelude::*;
use serde_json::Value;
pub fn rows(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}
pub fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}
pub fn members(value: &str) -> Vec<String> {
    value
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
pub async fn mutate(path: &str, method: &str, body: Value) -> Result<String, String> {
    browser_auth::call(path, method, Some(body))
        .await
        .map(|value| {
            let state = value["commandState"].as_str().unwrap_or("accepted");
            let reason = value["pendingReason"].as_str().unwrap_or("");
            if reason.is_empty() {
                format!("Request {state}.")
            } else {
                format!("Request {state}: {reason}")
            }
        })
        .map_err(|e| e.message)
}
#[component]
pub fn Notice(message: Signal<String>) -> Element {
    rsx! {if !message().is_empty(){p{role:"status",class:"hg-note whitespace-pre-wrap", "{message}"}}}
}
