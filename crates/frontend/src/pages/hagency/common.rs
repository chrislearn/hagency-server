pub use crate::api::browser_auth;
use dioxus::prelude::*;
use serde_json::Value;
pub fn rows(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}
pub fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}
pub fn name(value: &Value, fallback: &str) -> String {
    let label = text(value, "name");
    if label.trim().is_empty() {
        text(value, fallback)
    } else {
        label
    }
}
pub fn policy(value: &Value) -> Result<Value, String> {
    let raw = &value["creationPolicy"];
    let policy = if let Some(raw) = raw.as_str() {
        serde_json::from_str(raw)
            .map_err(|_| "Unable to read the current access policy.".to_string())?
    } else {
        raw.clone()
    };
    if !policy.is_object() || value["revision"].as_i64().is_none() {
        return Err("Unable to read the current access policy. Refresh before editing.".into());
    }
    let recognized = policy["defaultAllow"].is_boolean()
        || matches!(
            policy["mode"].as_str(),
            Some("inherit_project" | "allow_list" | "disabled")
        );
    if !recognized {
        return Err("Unknown access policy. Refresh before editing.".into());
    }
    Ok(policy)
}
pub fn member_lines(value: &Value, key: &str) -> String {
    rows(value, key)
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n")
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn access_editing_requires_readable_current_policy_and_revision() {
        assert!(policy(&json!({"revision":4,"creationPolicy":"{\"defaultAllow\":false,\"deny\":[\"@bob:s\"]}"})).is_ok());
        assert!(
            policy(
                &json!({"revision":3,"creationPolicy":{"mode":"allow_list","allow":["@alice:s"]}})
            )
            .is_ok()
        );
        for value in [
            json!({"revision":1}),
            json!({"creationPolicy":{"defaultAllow":true}}),
            json!({"revision":2,"creationPolicy":"broken"}),
            json!({"revision":2,"creationPolicy":{"mode":"future_policy"}}),
        ] {
            assert!(policy(&value).is_err());
        }
    }
}
