use super::common::*;
use crate::{api::hagency, router::Route};
use dioxus::prelude::*;
use serde_json::json;

#[component]
pub fn Projects() -> Element {
    let mut resource = use_resource(|| async {
        let projects = get("/projects").await?;
        let catalog = get("/catalog").await?;
        Ok(json!({"projects":projects["projects"],"fleets":catalog["fleets"]}))
    });
    use_future(move || async move {
        loop {
            gloo_timers::future::sleep(std::time::Duration::from_secs(10)).await;
            if web_sys::window()
                .and_then(|w| w.document())
                .is_some_and(|d| !d.hidden())
            {
                resource.restart();
            }
        }
    });
    let mut provider = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut room = use_signal(String::new);
    let mut operation = use_signal(|| saved_operation("project"));
    let mut busy = use_signal(|| false);
    let mut notice = use_signal(|| None);
    let loaded = data(resource);
    let fleets = rows(&loaded, "fleets");
    let projects = rows(&loaded, "projects");
    rsx! { div { class: "hg-page",
        Heading { title: "Projects", description: "Create a project room or register an existing room you own. Agent allocations are approved by the Hagency owner.", resource }
        Message { notice } Status { resource }
        form { class: "hg-card hg-form", onsubmit: move |event| {
            event.prevent_default(); if busy() { return; }
            let body = json!({"requestId":operation(),"fleetId":provider(),"name":name(),"roomId":room()});
            busy.set(true);
            spawn(async move {
                match hagency::call("/projects", "POST", Some(body)).await {
                    Ok(_) => { notice.set(Some((false,"Project saved. Its private approval room must be ready before requesting an agent.".into()))); operation.set(next_operation("project")); name.set(String::new()); room.set(String::new()); resource.restart(); },
                    Err(e) => notice.set(Some((true,e.message))),
                } busy.set(false);
            });
        },
            h2 { "Create a project" }
            label { class: "hg-field", "Hagency provider" select { class: "hg-input", required: true, value: provider(), onchange: move |e| provider.set(e.value()),
                option { value: "", "Choose a provider" }
                for fleet in fleets { option { value: text(&fleet,"id"), {text(&fleet,"name")} } }
            } }
            label { class: "hg-field", "Project name" input { class: "hg-input", required: true, maxlength: 128, value: name(), oninput: move |e|name.set(e.value()) } }
            label { class: "hg-field", "Existing room ID (optional)" input { class: "hg-input", placeholder: "!room:server", value: room(), oninput: move |e|room.set(e.value()) } }
            details { class: "hg-wide", summary { "Operation reference" } code { "{operation()}" } p { class: "hg-note", "A failed submission keeps this reference and your fields for a safe retry." } }
            button { class: "hg-button", r#type: "submit", disabled: busy(), if busy() { "Creating…" } else { "Create project" } }
        }
        div { class: "hg-grid",
            for project in projects {
                article { class: "hg-card",
                    h2 { {text(&project,"name")} }
                    span { class: "hg-badge", if project["canRequest"] == true { "Ready for agent requests" } else { "Waiting for approval-room setup" } }
                    Facts { rows: vec![("Owner".into(),text(&project,"ownerMxid")),("Provider".into(),text(&project,"fleetName")),("Agent requests".into(),text(&project,"requestCount"))] }
                    if project["readinessError"].is_string() { p { class: "hg-note", {text(&project,"readinessError")} } }
                    div { class: "hg-actions",
                        RoomLink { room: text(&project,"roomId"), label: "Open project room" }
                        RoomLink { room: text(&project,"ownerDmRoomId"), label: "Open approval room" }
                        Link { to: Route::HagencyRequestAgent {}, class: "hg-link", "Request an agent" }
                    }
                }
            }
        }
        if rows(&loaded,"projects").is_empty() && resource().is_some_and(|v|v.is_ok()) { p { class: "hg-note", "No projects yet." } }
    } }
}
