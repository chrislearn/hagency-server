use super::common::*;
use crate::{api::hagency, router::Route};
use dioxus::prelude::*;
use serde_json::{Value, json};

#[component]
pub fn Connections() -> Element {
    let mut resource = use_resource(|| async { get("/fleets").await });
    let mut name = use_signal(String::new);
    let mut owner = use_signal(String::new);
    let mut mode = use_signal(|| "outbound".to_owned());
    let mut callback = use_signal(String::new);
    let session = use_resource(|| async { hagency::ensure_session().await });
    let callback_origins = session()
        .and_then(Result::ok)
        .map(|s| s.callback_origins)
        .unwrap_or_default();
    let mut operation = use_signal(|| saved_operation("authorization"));
    let mut busy = use_signal(|| false);
    let mut notice = use_signal(|| None);
    rsx! { div { class: "hg-page",
        Heading { title: "Fleet connections", description: "Authorize agent providers and manage their Matrix service accounts. Owners download configuration and verify their connection.", resource }
        Message { notice } Status { resource }
        form { class: "hg-card hg-form", onsubmit: move |event| {
            event.prevent_default(); if busy() { return; }
            let mut body = json!({"requestId":operation(),"name":name(),"ownerMxid":owner(),"transportMode":mode()});
            if mode() == "callback" { body["callbackUrl"] = json!(callback()); }
            busy.set(true);
            spawn(async move {
                match hagency::call("/fleets","POST",Some(body)).await {
                    Ok(_) => { notice.set(Some((false,"Fleet authorized. Its owner can now download the configuration and verify the connection.".into()))); operation.set(next_operation("authorization")); name.set(String::new()); owner.set(String::new()); resource.restart(); },
                    Err(e) => notice.set(Some((true,format!("{} Keep the same operation reference to resume a partial installation.",e.message)))),
                } busy.set(false);
            });
        },
            h2 { "Authorize a Fleet" }
            label { class: "hg-field", "Name" input { class: "hg-input", required: true, maxlength: 128, value: name(), oninput: move |e|name.set(e.value()) } }
            label { class: "hg-field", "Owner Matrix ID" input { class: "hg-input", required: true, placeholder: "@owner:server", value: owner(), oninput: move |e|owner.set(e.value()) } }
            label { class: "hg-field", "Connection" select { class: "hg-input", value: mode(), onchange: move |e|mode.set(e.value()),
                option { value: "outbound", "Outbound — owner connects to this server" }
                option { value: "callback", disabled:callback_origins.is_empty(), "Callback — use an allowed public URL" }
            } }
            if mode() == "callback" { label { class: "hg-field", "Callback URL" input { class: "hg-input", r#type: "url", required: true, value: callback(), oninput: move |e|callback.set(e.value()) } } }
            p { class:"hg-note hg-wide", {if callback_origins.is_empty() { "No callback origins are allowed. Use outbound, or configure the server callback policy.".into() } else { format!("Allowed callback origins: {}",callback_origins.join(", ")) }} }
            details { class: "hg-wide", summary { "Operation reference" } code { "{operation()}" } }
            button { class: "hg-button", r#type: "submit", disabled: busy(), if busy() { "Authorizing…" } else { "Authorize Fleet" } }
        }
        div { class: "hg-grid", for fleet in rows(&data(resource),"fleets") { FleetCard { key:"{fleet[\"id\"]}", fleet, resource, busy, notice } } }
        if rows(&data(resource),"fleets").is_empty() && resource().is_some_and(|v|v.is_ok()) { p { class: "hg-note", "No fleets are registered." } }
    } }
}

#[component]
fn FleetCard(fleet: Value, resource: Data, busy: Signal<bool>, notice: Notice) -> Element {
    let id = text(&fleet, "id");
    let key = format!("hagency.transport_operation.{}", id);
    let mut transport_operation = use_signal(move || {
        crate::utils::storage::get_session_item(&key).unwrap_or_else(hagency::operation_id)
    });
    let operation_key = format!("hagency.transport_operation.{}", id);
    let revoked = fleet["state"] == "revoked";
    let paused = fleet["state"] == "paused";
    let installed = fleet["installation"] == "installed";
    let outbound = fleet["transport"]["mode"] == "outbound";
    let path = format!("/fleets/{id}/outbound");
    let queue_path = path.clone();
    let rotate_path = path.clone();
    let mut notice = notice;
    let mut busy = busy;
    rsx! { article { class: "hg-card",
        h2 { {text(&fleet,"name")} } span { class: "hg-badge", {text(&fleet,"state")} }
        Facts { rows: vec![("Owner".into(),text(&fleet,"ownerMxid")),("Installation".into(),text(&fleet,"installation")),("Representative".into(),text(&fleet,"representativeMxid")),("Transport".into(),text(&fleet["transport"],"mode")),("Online".into(),text(&fleet["transport"],"online")),("Agent identities".into(),text(&fleet,"agentCount")),("Matrix event delivery".into(),text(&fleet["readiness"],"eventDelivery")),("Reception".into(),text(&fleet["readiness"],"reception"))] }
        if !fleet["lastError"].is_null() { p { class: "hg-note", {format!("Last check: {}",text(&fleet["lastError"],"code"))} } }
        div { class: "hg-actions",
            Link { to: Route::HagencyAgents { fleet_id: id.clone() }, class: "hg-link", "Manage agent identities" }
            for (operation,label,visible) in [
                ("install","Retry installation",!installed && !paused && !revoked),
                ("pause","Pause",installed && !paused && !revoked),
                ("resume","Resume",installed && paused && !revoked),
                ("revoke","Revoke service",!revoked),
            ] {
                if visible {
                    {
                        let path = format!("/fleets/{id}/{operation}");
                        rsx! { button { class: "hg-button hg-secondary", disabled: busy(), onclick: move |_| {
                            if operation != "revoke" || confirm("Revoke this Matrix service? Its service credential will stop working. Retire identities separately; local agent tasks require confirmation from the Fleet.") {
                                action(path.clone(),"POST",json!({}),"Service state updated.".into(),resource,busy,notice);
                            }
                        }, "{label}" } }
                    }
                }
            }
            if installed && !paused && !revoked {
                button { class: "hg-button hg-secondary", disabled: busy(), onclick: move |_| {
                    if !outbound || confirm("Rotate the transport credential? The owner must download and import the new configuration.") {
                        if busy() { return; }
                        crate::utils::storage::set_session_item(&operation_key,&transport_operation());
                        let key=operation_key.clone(); let path=rotate_path.clone(); let operation=transport_operation();
                        busy.set(true); spawn(async move {
                            match hagency::call(&path,"POST",Some(json!({"requestId":operation,"rotate":outbound}))).await {
                                Ok(_) => { crate::utils::storage::remove_session_item(&key); transport_operation.set(next_operation("authorization")); notice.set(Some((false,"Transport updated. The owner must download and verify the new configuration.".into()))); resource.restart(); },
                                Err(e)=>notice.set(Some((true,format!("{} The operation reference is kept for retry.",e.message))))
                            } busy.set(false);
                        });
                    }
                }, if outbound { "Rotate transport credential" } else { "Migrate to outbound" } }
            }
            if outbound { button { class: "hg-button hg-secondary", disabled: busy(), onclick: move |_| {
                if busy() { return; } busy.set(true);
                let path = queue_path.clone(); spawn(async move {
                    match hagency::call(&path,"GET",None).await {
                        Ok(value) => { let q=&value["queue"]; notice.set(Some((false,format!("Delivery queue: {} pending / {} limit; {} retained records; {} pending bytes.",text(q,"pending"),text(&q["limits"],"pending"),text(q,"records"),text(q,"bytes"))))); },
                        Err(e) => notice.set(Some((true,e.message))),
                    } busy.set(false);
                });
            }, "Inspect delivery capacity" } }
        }
    } }
}

#[component]
pub fn MyFleets() -> Element {
    let mut resource = use_resource(|| async { get("/my/fleets").await });
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
    rsx! { div { class: "hg-page",
        Heading { title: "My Fleets", description: "Sign in from hagency-client to configure your Fleet automatically, or import an existing configuration and verify the connection.", resource }
        Message { notice } Status { resource }
        RenewalWarnings { fleets:rows(&data(resource),"fleets") }
        div { class: "hg-grid", for fleet in rows(&data(resource),"fleets") { OwnedFleet { key:"{fleet[\"id\"]}", fleet, resource, busy, notice } } }
        if rows(&data(resource),"fleets").is_empty() && resource().is_some_and(|v|v.is_ok()) { p { class: "hg-note", "No fleets are assigned to your account. You can still create projects and request agents from available providers." } }
    } }
}

#[component]
fn OwnedFleet(
    fleet: Value,
    mut resource: Data,
    mut busy: Signal<bool>,
    mut notice: Notice,
) -> Element {
    let id = text(&fleet, "id");
    let download_path = format!("/my/fleets/{id}/pair");
    let connect_path = format!("/my/fleets/{id}/connect");
    let filename = format!("{id}-registration.json");
    let revoked = fleet["state"] == "revoked";
    rsx! { article { class: "hg-card",
        h2 { {text(&fleet,"name")} } span { class: "hg-badge", {text(&fleet,"state")} }
        Facts { rows: vec![("Configuration".into(),if fleet["credentialDeliveredAt"].is_null() { "Not downloaded".into() } else { "Downloaded".into() }), ("Connection".into(),if fleet["readiness"]["ready"] == true { "Verified".into() } else { "Needs verification".into() }), ("Representative".into(),text(&fleet,"representativeMxid")), ("Online".into(),text(&fleet["transport"],"online"))] }
        RoomLink { room: text(&fleet["reception"],"roomId"), label: "Open reception room" }
        if !fleet["lastError"].is_null() { p { class: "hg-note", {text(&fleet["lastError"],"code")} } }
        if !revoked { div { class: "hg-actions",
            button { class: "hg-button", disabled: busy(), onclick: move |_| {
                if busy() { return; } busy.set(true); let path=download_path.clone();let filename=filename.clone();
                spawn(async move {
                    match hagency::call(&path,"POST",Some(json!({}))).await {
                        Ok(value) => match download(&value,&filename) {
                            Ok(()) => { notice.set(Some((false,"Configuration downloaded. Import it into the hagency-rs credential store, then verify the connection.".into()))); resource.restart(); },
                            Err(e) => notice.set(Some((true,e))),
                        },
                        Err(e) => notice.set(Some((true,e.message))),
                    } busy.set(false);
                });
            }, "Download configuration" }
            button { class: "hg-button hg-secondary", disabled: busy(), onclick: move |_| action(connect_path.clone(),"POST",json!({}),"Verification requested. Refresh to check the event receipt and reception room.".into(),resource,busy,notice), "Verify connection & create reception" }
        } }
    } }
}

#[component]
pub fn Agents(fleet_id: String) -> Element {
    let path = format!("/fleets/{fleet_id}/agents");
    let read_path = path.clone();
    let mut resource = use_resource(move || {
        let path = read_path.clone();
        async move { get(&path).await }
    });
    let mut agent_id = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut role = use_signal(|| "coding".into());
    let mut approved = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut notice = use_signal(|| None);
    rsx! { div { class: "hg-page",
        Heading { title: "Agent identities", description: "Manage Matrix identities for approved agent requests. Agent allocation and execution are handled by the Fleet provider.", resource }
        Message { notice } Status { resource }
        Link { to: Route::HagencyConnections {}, class: "hg-link", "Back to connections" }
        form { class: "hg-card hg-form", onsubmit: move |event| {
            event.prevent_default();if busy() { return; }busy.set(true);let path=path.clone();
            let body=json!({"agentId":agent_id(),"displayName":name(),"role":role(),"approvedRequestId":approved()});
            spawn(async move { match hagency::call(&path,"POST",Some(body)).await {
                Ok(_) => { notice.set(Some((false,"Matrix identity created.".into())));agent_id.set(String::new());name.set(String::new());approved.set(String::new());resource.restart(); },
                Err(e) => notice.set(Some((true,e.message))),
            }busy.set(false); });
        },
            h2 { "Create an approved identity" }
            label { class: "hg-field", "Stable agent ID" input { class: "hg-input", required:true, pattern:"[a-z0-9_]+", value:agent_id(), oninput:move|e|agent_id.set(e.value()) } }
            label { class: "hg-field", "Display name" input { class:"hg-input", required:true, maxlength:128,value:name(),oninput:move|e|name.set(e.value()) } }
            label { class: "hg-field", "Role" input { class:"hg-input", required:true,maxlength:80,value:role(),oninput:move|e|role.set(e.value()) } }
            label { class: "hg-field", "Approved request reference" input { class:"hg-input",required:true,pattern:"[a-zA-Z0-9_-]+",value:approved(),oninput:move|e|approved.set(e.value()) } }
            button { class:"hg-button",r#type:"submit",disabled:busy(),"Create identity" }
        }
        div { class:"hg-grid",for agent in rows(&data(resource),"agents") { AgentCard { key:"{agent[\"id\"]}", agent,fleet_id:fleet_id.clone(),resource,busy,notice } } }
        if rows(&data(resource),"agents").is_empty() && resource().is_some_and(|v|v.is_ok()) { p { class:"hg-note","No managed agent identities." } }
    } }
}

#[component]
fn AgentCard(
    agent: Value,
    fleet_id: String,
    resource: Data,
    busy: Signal<bool>,
    notice: Notice,
) -> Element {
    let edit_path = format!("/fleets/{fleet_id}/agents/{}", text(&agent, "id"));
    let retire_path = format!("{edit_path}/retire");
    rsx! { article { class:"hg-card",
        h2 { {text(&agent,"displayName")} } span { class:"hg-badge",{text(&agent,"state")} }
        Facts { rows:vec![("Matrix ID".into(),text(&agent,"mxid")),("Role".into(),text(&agent,"role")),("Approved request".into(),text(&agent,"approvedRequestId")),("Matrix identity".into(),text(&agent,"matrixIdentity")),("Local task stop".into(),text(&agent,"localTaskStop"))] }
        if agent["observationError"].is_string() { p { class:"hg-note",{text(&agent,"observationError")} } }
        div { class:"hg-actions",
            if agent["state"] == "registered" { button { class:"hg-button hg-secondary",disabled:busy(),onclick:move|_| {
                if let Some(name)=web_sys::window().and_then(|w|w.prompt_with_message("New display name").ok().flatten()).filter(|s|!s.trim().is_empty()) {
                    action(edit_path.clone(),"PATCH",json!({"displayName":name}),"Display name updated.".into(),resource,busy,notice);
                }
            },"Edit display name" } }
            if agent["state"] != "retired" { button { class:"hg-button hg-secondary",disabled:busy(),onclick:move|_| {
                if confirm("Retire this Matrix identity and remove its room memberships? History is preserved; local agent tasks require confirmation from the Fleet.") {
                    action(retire_path.clone(),"POST",json!({}),"Identity retirement requested. Refresh to inspect Matrix state.".into(),resource,busy,notice);
                }
            },"Retire identity" } }
        }
    } }
}

#[component]
pub fn Activity() -> Element {
    let resource = use_resource(|| async { get("/audit").await });
    rsx! { div { class:"hg-page",
        Heading { title:"Hagency activity",description:"The most recent 200 management operations.",resource }
        Status { resource }
        div { class:"hg-stack",for event in rows(&data(resource),"events") { article { class:"hg-card",
            h2 { {text(&event,"action")} }
            Facts { rows:vec![("Time".into(),text(&event,"at")),("Actor".into(),text(&event,"actor")),("Fleet".into(),text(&event,"fleetId")),("Object".into(),text(&event,"objectId")),("Result".into(),text(&event,"result"))] }
        } } }
        if rows(&data(resource),"events").is_empty() && resource().is_some_and(|v|v.is_ok()) { p { class:"hg-note","No operations recorded yet." } }
    } }
}
