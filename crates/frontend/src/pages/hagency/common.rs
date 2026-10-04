use crate::api::hagency;
use dioxus::prelude::*;
use serde_json::Value;
use wasm_bindgen::JsCast;

thread_local! { static RENEWAL: std::cell::RefCell<std::collections::HashMap<String,f64>> = std::cell::RefCell::new(std::collections::HashMap::new()); }

pub type Data = Resource<Result<Value, String>>;
pub type Notice = Signal<Option<(bool, String)>>;

pub fn text(value: &Value, key: &str) -> String {
    match &value[key] {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
pub fn rows(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}
pub fn resource_pool(fleet: &Value) -> Vec<Value> {
    let mut pool = std::collections::BTreeMap::<String, Value>::new();
    for offer in rows(&fleet["capabilities"], "offers") {
        for mut resource in rows(&offer, "resources") {
            let id = text(&resource, "id");
            if id.is_empty() {
                continue;
            }
            resource["roles"] = serde_json::json!([]);
            let resource = pool.entry(id).or_insert(resource);
            let roles = resource["roles"].as_array_mut().unwrap();
            if !roles.contains(&offer["role"]) {
                roles.push(offer["role"].clone());
            }
        }
    }
    pool.into_values().collect()
}
pub fn data(resource: Data) -> Value {
    resource().and_then(Result::ok).unwrap_or(Value::Null)
}
pub async fn get(path: &str) -> Result<Value, String> {
    let mut value = hagency::call(path, "GET", None)
        .await
        .map_err(|e| e.message)?;
    if matches!(path, "/catalog" | "/my/fleets") {
        for fleet in rows(&value, "fleets") {
            let owned = path == "/my/fleets" || fleet["owned"] == true;
            let expiry = js_sys::Date::parse(&text(&fleet["readiness"], "expiresAt"));
            if owned
                && fleet["transport"]["mode"] != "outbound"
                && fleet["installation"] == "installed"
                && matches!(
                    text(&fleet, "state").as_str(),
                    "ready" | "pending_connection"
                )
                && fleet["connection"]["verifiedAt"].is_string()
                && fleet["reception"]["roomId"].is_string()
                && !(fleet["readiness"]["ready"] == true && expiry - js_sys::Date::now() > 60000.0)
            {
                let id = text(&fleet, "id");
                let now = js_sys::Date::now();
                let retry = RENEWAL.with(|map| {
                    let mut map = map.borrow_mut();
                    if map.get(&id).is_some_and(|t| now - t < 60000.0) {
                        false
                    } else {
                        map.insert(id.clone(), now);
                        true
                    }
                });
                if retry {
                    let _ = hagency::call(
                        &format!("/my/fleets/{id}/connect"),
                        "POST",
                        Some(serde_json::json!({})),
                    )
                    .await;
                    value = hagency::call(path, "GET", None)
                        .await
                        .map_err(|e| e.message)?;
                }
            }
        }
    }
    Ok(value)
}
pub fn saved_operation(kind: &str) -> String {
    let key = format!(
        "hagency.operation.{}.{}",
        crate::utils::storage::get_item("user_id").unwrap_or_default(),
        kind
    );
    if let Some(id) = crate::utils::storage::get_session_item(&key) {
        return id;
    }
    let id = hagency::operation_id();
    crate::utils::storage::set_session_item(&key, &id);
    id
}
pub fn next_operation(kind: &str) -> String {
    let key = format!(
        "hagency.operation.{}.{}",
        crate::utils::storage::get_item("user_id").unwrap_or_default(),
        kind
    );
    crate::utils::storage::remove_session_item(&key);
    saved_operation(kind)
}

pub fn room_url(room: &str) -> String {
    format!("https://matrix.to/#/{}", urlencoding::encode(room))
}

#[component]
pub fn Status(resource: Data) -> Element {
    match resource() {
        None => rsx! { p { class: "hg-note", role: "status", "Loading…" } },
        Some(Err(message)) => {
            rsx! { div { class: "hg-notice hg-error", role: "alert", "{message}" } }
        }
        Some(Ok(_)) => rsx! {},
    }
}

#[component]
pub fn Message(notice: Notice) -> Element {
    match notice() {
        Some((error, message)) => rsx! {
            div { class: if error { "hg-notice hg-error" } else { "hg-notice" }, role: "status", "{message}" }
        },
        None => rsx! {},
    }
}

#[component]
pub fn Heading(title: String, description: String, mut resource: Data) -> Element {
    rsx! { div { class: "hg-heading",
        div { h1 { "{title}" } p { class: "hg-note", "{description}" } }
        button { class: "hg-button hg-secondary", onclick: move |_| resource.restart(), "Refresh" }
    } }
}

#[component]
pub fn Facts(rows: Vec<(String, String)>) -> Element {
    rsx! { dl { class: "hg-facts",
        for (label, value) in rows.into_iter().filter(|(_,v)| !v.is_empty()) {
            dt { "{label}" } dd { "{value}" }
        }
    } }
}

#[component]
pub fn RoomLink(room: String, label: String) -> Element {
    rsx! { if !room.is_empty() { a { href: room_url(&room), target: "_blank", rel: "noopener noreferrer", class: "hg-link", "{label}" } } }
}

pub fn action(
    path: String,
    method: &'static str,
    body: Value,
    message: String,
    mut resource: Data,
    mut busy: Signal<bool>,
    mut notice: Notice,
) {
    if busy() {
        return;
    }
    busy.set(true);
    spawn(async move {
        match hagency::call(&path, method, Some(body)).await {
            Ok(_) => {
                notice.set(Some((false, message)));
                resource.restart();
            }
            Err(e) => notice.set(Some((true, e.message))),
        }
        busy.set(false);
    });
}

pub fn confirm(message: &str) -> bool {
    web_sys::window().is_some_and(|w| w.confirm_with_message(message).unwrap_or(false))
}

pub fn download(value: &Value, filename: &str) -> Result<(), String> {
    let strings = js_sys::Array::new();
    strings.push(
        &serde_json::to_string_pretty(value)
            .map_err(|e| e.to_string())?
            .into(),
    );
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/json");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&strings, &options)
        .map_err(|e| format!("{e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("{e:?}"))?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("Browser document unavailable")?;
    let anchor = document
        .create_element("a")
        .map_err(|e| format!("{e:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|e| format!("{e:?}"))?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    spawn(async move {
        gloo_timers::future::sleep(std::time::Duration::from_secs(1)).await;
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resources_merge_their_published_roles() {
        let fleet = serde_json::json!({"capabilities":{"offers":[
            {"role":"coding","resources":[{"id":"codex","name":"Codex"}]},
            {"role":"review","resources":[{"id":"codex","name":"Codex"}]}
        ]}});
        let pool = resource_pool(&fleet);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0]["roles"], serde_json::json!(["coding", "review"]));
        assert!(resource_pool(&serde_json::json!({})).is_empty());
    }
}
