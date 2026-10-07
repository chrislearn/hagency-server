use super::common::*;
use dioxus::prelude::*;
use serde_json::json;

#[component]
pub fn ProjectControls(project_id: Signal<String>, room: Signal<String>) -> Element {
    let mut mode = use_signal(|| "inherit_project".to_string());
    let mut revision = use_signal(|| "1".to_string());
    let mut allow = use_signal(String::new);
    let mut deny = use_signal(String::new);
    let mut room_scope = use_signal(|| false);
    let mut message = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut paused = use_signal(|| None::<bool>);
    let selected_path = move || {
        let project = project_id();
        if project.is_empty() {
            return None;
        }
        if room_scope() {
            if room().is_empty() {
                return None;
            }
            Some(format!(
                "/projects/{}/rooms/{}",
                urlencoding::encode(&project),
                urlencoding::encode(&room())
            ))
        } else {
            Some(format!("/projects/{}", urlencoding::encode(&project)))
        }
    };
    rsx! {
        div { class: "hg-card hg-stack",
            h2 { "Room creation rights" }
            p { "Uses the Project ID and Room ID above. Room restrictions also apply to new agents and bindings; changing creation rights does not stop existing agents." }
            Notice { message }
            form { class: "hg-stack", onsubmit: move |e| {
                e.prevent_default();
                if busy() || project_id().is_empty() || room().is_empty() { return; }
                let Ok(expected_revision) = revision().parse::<i64>() else { message.set("Enter the current Room revision.".into()); return; };
                let policy = match mode().as_str() {
                    "disabled" => json!({"mode":"disabled"}),
                    "allow_list" => json!({"mode":"allow_list","allow":members(&allow()),"deny":members(&deny())}),
                    _ => json!({"mode":"inherit_project","deny":members(&deny())}),
                };
                let path = format!("/projects/{}/rooms/{}/creation-policy",urlencoding::encode(&project_id()),urlencoding::encode(&room()));
                busy.set(true);
                spawn(async move {
                    match browser_auth::call(&path,"PUT",Some(json!({"expectedRevision":expected_revision,"policy":policy}))).await {
                        Ok(result) => {
                            if let Some(next) = result["room"]["revision"].as_i64() { revision.set(next.to_string()); }
                            message.set("Room creation rights saved. Existing agents keep their runtime state.".into());
                        }, Err(e) => message.set(e.message),
                    }
                    busy.set(false);
                });
            },
                label { "Room policy" select { class: "hg-input", value: mode(), onchange: move |e| mode.set(e.value()),
                    option { value: "inherit_project", "Inherit Project with Room deny list" }
                    option { value: "allow_list", "Only listed Room members" }
                    option { value: "disabled", "Disable creation in this Room" }
                } }
                label { "Current Room revision" input { class: "hg-input", value: revision(), oninput: move |e| revision.set(e.value()), required: true } }
                if mode() == "allow_list" { label { "Allowed Matrix users" textarea { class: "hg-input", value: allow(), oninput: move |e| allow.set(e.value()) } } }
                if mode() != "disabled" { label { "Denied Matrix users" textarea { class: "hg-input", value: deny(), oninput: move |e| deny.set(e.value()) } } }
                button { class: "hg-button", r#type: "submit", disabled: busy() || project_id().is_empty() || room().is_empty(), "Save Room creation policy" }
            }
        }
        div { class: "hg-card hg-stack",
            h2 { "Administrator service pause" }
            p { "A service pause blocks new agents and bindings and suspends existing bindings in this scope. Clearing it permits owners to resume; it never resumes their agents automatically." }
            label { input { r#type: "checkbox", checked: room_scope(), onchange: move |e| { room_scope.set(e.checked()); paused.set(None); } } " Limit to the Room above" }
            if let Some(value) = paused() { p { {if value { "Service is paused" } else { "No administrator service pause; owners may still need to resume their bindings" }} } }
            button { class: "hg-button", disabled: busy() || selected_path().is_none(), onclick: move |_| {
                let Some(path) = selected_path() else { return; }; busy.set(true);
                spawn(async move {
                    match browser_auth::call(&format!("{path}/service-state"),"GET",None).await {
                        Ok(state) => {
                            paused.set(state["servicePaused"].as_bool());
                            if let Some(rev) = state["revision"].as_i64() { if room_scope() { revision.set(rev.to_string()); } }
                            message.set("Service state refreshed.".into());
                        }, Err(e) => message.set(e.message),
                    }
                    busy.set(false);
                });
            }, "Check service state" }
            for (action,label) in [("pause-service","Pause service"),("clear-service-pause","Clear administrator pause")] {
                button { class: "hg-button", disabled: busy() || selected_path().is_none(), onclick: move |_| {
                    let Some(path) = selected_path() else { return; }; busy.set(true);
                    spawn(async move {
                        match browser_auth::call(&format!("{path}/{action}"),"POST",None).await {
                            Ok(_) => {
                                paused.set(None);
                                if let Ok(state) = browser_auth::call(&format!("{path}/service-state"),"GET",None).await { paused.set(state["servicePaused"].as_bool()); }
                                message.set(if action == "pause-service" { "Service paused in the selected scope." } else { "Administrator pause cleared. Each owner must explicitly resume their bindings." }.into()); },
                            Err(e) => message.set(e.message),
                        }
                        busy.set(false);
                    });
                }, "{label}" }
            }
        }
    }
}
