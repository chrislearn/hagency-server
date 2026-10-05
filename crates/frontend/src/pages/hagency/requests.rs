use super::common::*;
use crate::{api::hagency, router::Route};
use dioxus::prelude::*;
use serde_json::{Value, json};

#[component]
pub fn RequestAgent() -> Element {
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
    let mut project_id = use_signal(String::new);
    let mut resource_id = use_signal(String::new);
    let mut role = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut tokens = use_signal(|| "100000".to_owned());
    let mut per_day = use_signal(|| "20000".to_owned());
    let mut operation = use_signal(|| saved_operation("request"));
    let mut busy = use_signal(|| false);
    let mut notice = use_signal(|| None);
    let loaded = data(resource);
    let projects = rows(&loaded, "projects");
    let project = projects
        .iter()
        .find(|p| p["id"] == project_id())
        .cloned()
        .unwrap_or(Value::Null);
    let fleet = rows(&loaded, "fleets")
        .into_iter()
        .find(|f| f["id"] == project["fleetId"])
        .unwrap_or(Value::Null);
    let offers = resource_pool(&fleet);
    let chosen = offers
        .iter()
        .find(|o| o["id"] == resource_id())
        .cloned()
        .unwrap_or(Value::Null);
    let roles = chosen["roles"].as_array().cloned().unwrap_or_default();
    let valid_role = roles.iter().any(|r| r.as_str() == Some(&role()));
    let provider_ready = if fleet["transport"]["mode"] == "outbound" {
        fleet["readiness"]["canQueue"] == true
    } else {
        fleet["readiness"]["ready"] == true
            && js_sys::Date::parse(&text(&fleet["readiness"], "expiresAt")) > js_sys::Date::now()
    };
    let usable = project["canRequest"] == true
        && provider_ready
        && fleet["capabilityRead"]["state"] != "failed"
        && !chosen.is_null()
        && valid_role;
    rsx! { div { class: "hg-page",
        Heading { title: "Request an agent", description: "Choose a project, then a published resource and role. The Fleet owner reviews the requested allocation.", resource }
        Message { notice } Status { resource }
        RenewalWarnings { fleets:rows(&loaded,"fleets") }
        if fleet["transport"]["mode"] == "outbound" && fleet["transport"]["online"] == false && provider_ready {
            p { class: "hg-notice", "This Fleet is offline. Requests use its last published resources and will be queued until it reconnects. Its owner still needs to approve the allocation." }
        }
        if projects.is_empty() && resource().is_some_and(|v|v.is_ok()) { div { class: "hg-card",
            p { "Create a project before requesting an agent." }
            Link { to: Route::HagencyProjects {}, class: "hg-link", "Go to projects" }
        } }
        form { class: "hg-card hg-form", onsubmit: move |event| {
            event.prevent_default(); if busy() || !usable { return; }
            let body = json!({"requestId":operation(),"projectId":project_id(),"resourceId":resource_id(),
                "agentName":name(),"role":role(),"requestedTokens":tokens(),"ratePerDay":per_day()});
            busy.set(true);
            spawn(async move {
                match hagency::call("/requests", "POST", Some(body)).await {
                    Ok(result) => {
                        let state = text(&result["request"],"state");
                        notice.set(Some((false,format!("Agent request saved ({state}). The Fleet owner must approve it before the agent is available."))));
                        operation.set(next_operation("request")); name.set(String::new()); resource.restart();
                    },
                    Err(e) => notice.set(Some((true,format!("{} Your fields and operation reference are kept for retry.",e.message)))),
                } busy.set(false);
            });
        },
            label { class: "hg-field", "Project" select { class: "hg-input", required: true, value: project_id(), onchange: move |e| { project_id.set(e.value()); resource_id.set(String::new()); role.set(String::new()); },
                option { value: "", "Choose a ready project" }
                for p in projects { option { value: text(&p,"id"), disabled: p["canRequest"] != true, {text(&p,"name")} } }
            } }
            label { class: "hg-field", "Resource" select { class: "hg-input", required: true, value: resource_id(), onchange: move |e|{ resource_id.set(e.value()); role.set(String::new()); },
                option { value: "", "Choose a published resource" }
                for offer in &offers { option { value: text(offer,"id"), {format!("{} · {}",text(offer,"name"),text(offer,"model"))} } }
            } }
            label { class: "hg-field", "Role" select { class: "hg-input", required: true, value: role(), onchange: move |e|role.set(e.value()),
                option { value: "", "Choose a supported role" }
                for r in roles { option { value: r.as_str().unwrap_or_default(), "{r.as_str().unwrap_or_default()}" } }
            } }
            label { class: "hg-field", "Agent name" input { class: "hg-input", required: true, maxlength: 64, value: name(), placeholder: "Edison or 小白", oninput: move |e|name.set(e.value()) } }
            label { class: "hg-field", "Total token allowance" input { class: "hg-input", r#type: "number", min: 1, step: 1, required: true, value: tokens(), oninput: move |e|tokens.set(e.value()) } }
            label { class: "hg-field", "Daily token allowance" input { class: "hg-input", r#type: "number", min: 1, step: 1, required: true, value: per_day(), oninput: move |e|per_day.set(e.value()) } }
            if !usable { p { class: "hg-note hg-wide", "Select a ready project, an available resource and its role. If connection verification is missing or expired, ask the provider owner to verify it in My Fleets. An established outbound provider can queue requests while offline." } }
            details { class: "hg-wide", summary { "Operation reference" } code { "{operation()}" } }
            button { class: "hg-button", r#type: "submit", disabled: busy() || !usable, if busy() { "Submitting…" } else { "Send agent request" } }
        }
        if !offers.is_empty() { section { class: "hg-grid", aria_label: "Available resources",
            for offer in offers { article { class: "hg-card",
                h2 { {text(&offer,"name")} }
                Facts { rows: vec![("Model".into(),text(&offer,"model")),("Framework".into(),text(&offer,"framework")),("Reasoning".into(),text(&offer,"reasoning")),("Roles".into(),text(&offer,"roles"))] }
                button { class:"hg-button hg-secondary",r#type:"button",onclick:move|_| {
                    resource_id.set(text(&offer,"id"));
                    role.set(offer["roles"].as_array().and_then(|r|r.first()).and_then(Value::as_str).unwrap_or_default().into());
                },"Define Agent on this resource" }
            } }
        } }
    } }
}

#[component]
pub fn Requests() -> Element {
    let mut resource = use_resource(|| async { get("/requests").await });
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
    let busy = use_signal(|| false);
    let notice = use_signal(|| None);
    let requests = rows(&data(resource), "requests");
    rsx! { div { class: "hg-page",
        Heading { title: "Agent requests", description: "Follow delivery, owner approval and actual agent availability for your projects.", resource }
        Message { notice } Status { resource }
        Link { to: Route::HagencyRequestAgent {}, class: "hg-link", "Request an agent" }
        if requests.iter().any(|r|r["lastError"]["code"]=="outbound_status_stale") {
            p { class:"hg-notice", "Some requests have not received a recent status from the Fleet. They are not usable until current status and room membership are verified." }
        }
        for (index,label) in ["Needs attention","Waiting for agent to join","In review","Ready to use","Ended"].into_iter().enumerate() {
            {
                let group: Vec<_> = requests.iter().rev().filter(|r|request_group(r)==index).cloned().collect();
                rsx! { if !group.is_empty() { section { class: "hg-stack",
                    h2 { "{label}" }
                    for request in group {
                        article { class: "hg-card",
                            h3 { {if request["agentDefinition"]["name"].is_string() { text(&request["agentDefinition"],"name") } else { text(&request,"role") }} }
                            span { class: "hg-badge", {text(&request,"state")} }
                            Facts { rows: vec![("Project".into(),text(&request,"projectId")),("Role".into(),text(&request,"role")),("Token allowance".into(),text(&request,"requestedTokens")),("Daily allowance".into(),text(&request,"ratePerDay")),("Request".into(),text(&request,"requestId")),("Agent".into(),text(&request["provider"],"agentMxid")),("Model".into(),text(&request["resource"],"model"))] }
                            if request["provider"]["serving"]["model"].is_string() {
                                Facts { rows:vec![("Running framework".into(),text(&request["provider"]["serving"],"framework")),("Running model".into(),text(&request["provider"]["serving"],"model")),("Reasoning".into(),text(&request["provider"]["serving"],"reasoning")),("Tier".into(),text(&request["provider"]["serving"],"tier"))] }
                            }
                            if request["provider"]["fulfillment"]["phase"].is_string() && request["provider"]["fulfillment"]["phase"] != "complete" {
                                p { class:"hg-note",{format!("Preparation: {}",text(&request["provider"]["fulfillment"],"phase"))} }
                            }
                            if request["provider"]["fulfillment"]["error"].is_string() {
                                p { class:"hg-notice hg-error",{text(&request["provider"]["fulfillment"],"error")} }
                            }
                            if request["state"]=="queued" { p { class:"hg-note","Stored on this server. Waiting for Fleet delivery and its owner's allocation decision." } }
                            if request["state"]=="pending" { p { class:"hg-note","Waiting for the Fleet owner's resource decision." } }
                            if request["state"]=="submission_pending" { p { class:"hg-notice","The request has not been delivered yet. Retry uses the same request reference." } }
                            if request["usable"] == true { RoomLink { room: text(&request,"targetRoomId"), label: "Open project and use agent" } }
                            if !request["lastError"].is_null() { p { class: "hg-note", {text(&request["lastError"],"code")} } }
                            if request["state"] == "submission_pending" {
                                {
                                    let body = json!({"requestId":request["requestId"],"projectId":request["projectId"],"role":request["role"],"requestedTokens":request["requestedTokens"],"ratePerDay":request["ratePerDay"],"agentDefinition":request["agentDefinition"]});
                                    rsx! { button { class: "hg-button hg-secondary", disabled: busy(), onclick: move |_|action("/requests".into(),"POST",body.clone(),"Submission retried; refresh to check delivery.".into(),resource,busy,notice), "Retry submission" } }
                                }
                            }
                        }
                    }
                } } }
            }
        }
        if requests.is_empty() && resource().is_some_and(|v|v.is_ok()) { p { class: "hg-note", "No agent requests yet." } }
    } }
}
