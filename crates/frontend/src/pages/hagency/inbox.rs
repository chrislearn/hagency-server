use super::common::*;
use crate::api::hagency;
use dioxus::prelude::*;
use serde_json::{Value, json};

#[component]
pub fn Inbox() -> Element {
    let mut view = use_signal(|| {
        if web_sys::window().is_some_and(|w| {
            w.location()
                .search()
                .unwrap_or_default()
                .contains("action=")
        }) {
            "all".to_owned()
        } else {
            "needs_action".to_owned()
        }
    });
    let mut offset = use_signal(|| 0_u64);
    let mut resource = use_resource(move || {
        let view = view();
        let offset = offset();
        async move {
            hagency::call("/operations/call", "POST", Some(json!({"service":"hagency.inbox.list","args":{"view":view,"offset":offset,"limit":50}})))
            .await.map_err(|e| e.message)
        }
    });
    let busy = use_signal(|| false);
    let notice = use_signal(|| None);
    let loaded = data(resource);
    rsx! { div { class: "hg-page",
        Heading { title:"Inbox", description:"Project, agent and token requests. Approval and execution are tracked separately.", resource }
        Message { notice } Status { resource }
        div { class:"hg-actions",
            for (value,label) in [("needs_action","Needs my action"),("waiting","Waiting"),("history","History"),("all","All")] {
                button { class:"hg-button hg-secondary", onclick:move |_| { view.set(value.into()); offset.set(0); }, "{label}" }
            }
        }
        for row in rows(&loaded,"actions") { InboxAction { key: "{row[\"id\"]}", row, resource, busy, notice } }
        if rows(&loaded,"actions").is_empty() && resource().is_some_and(|r| r.is_ok()) { p { class:"hg-note", "No actions in this view." } }
        div { class:"hg-actions",
            button { class:"hg-button hg-secondary", disabled:offset()==0, onclick:move |_|offset.set(offset().saturating_sub(50)), "Previous" }
            button { class:"hg-button hg-secondary", disabled:offset()+50>=loaded["total"].as_u64().unwrap_or(0), onclick:move |_|offset.set(offset()+50), "Next" }
        }
    } }
}

#[component]
fn InboxAction(row: Value, resource: Data, busy: Signal<bool>, notice: Notice) -> Element {
    let mut reason = use_signal(String::new);
    // Stable across retries of the same revision; a refresh preserves the ID.
    let command = saved_operation(&format!(
        "inbox.{}.{}",
        text(&row, "id"),
        text(&row, "revision")
    ));
    let approve = row.clone();
    let reject = row.clone();
    let seen = row.clone();
    let snooze = row.clone();
    let approve_command = command.clone();
    rsx! { article { class:"hg-card",
        h2 { {text(&row["payload"],"name")} }
        Facts { rows:vec![("Reference".into(),text(&row,"id")),("Kind".into(),text(&row,"kind")),("Owner".into(),text(&row,"ownerMxid")),("Project".into(),text(&row["request"]["request"],"projectId")),("Resource".into(),text(&row["request"]["request"],"resourceAllocationId")),("Requested tokens".into(),text(&row["request"]["request"],"requestedTokens")),("Existing allocation".into(),text(&row["request"]["request"],"expectedAllocatedTokens")),("Additional tokens".into(),text(&row["request"]["request"],"requestedAdditionalTokens")),("Role".into(),text(&row["payload"],"role")),("Room".into(),text(&row["payload"],"targetRoomId")),("Decision".into(),text(&row,"state")),("Execution".into(),text(&row,"execution"))] }
        if !text(&row["payload"],"reason").is_empty() { p { class:"hg-note", {text(&row["payload"],"reason")} } }
        if row["needsMyAction"]==true {
            label { class:"hg-field", "Reason" input { class:"hg-input", value:reason(), maxlength:1000, oninput:move |e|reason.set(e.value()) } }
            div { class:"hg-actions",
                button { class:"hg-button", disabled:busy(), onclick:move |_| action("/operations/call".into(),"POST",json!({"service":"hagency.inbox.decide","args":{"id":approve["id"],"expectedRevision":approve["revision"],"decision":"approve","commandId":approve_command,"reason":reason()}}),"Approved. Execution is tracked separately.".into(),resource,busy,notice),"Approve" }
                button { class:"hg-button hg-secondary", disabled:busy()||reason().trim().is_empty(), onclick:move |_| action("/operations/call".into(),"POST",json!({"service":"hagency.inbox.decide","args":{"id":reject["id"],"expectedRevision":reject["revision"],"decision":"reject","commandId":format!("r_{}",command),"reason":reason()}}),"Rejected.".into(),resource,busy,notice),"Reject" }
                button { class:"hg-button hg-secondary", disabled:busy(), onclick:move |_| action("/operations/call".into(),"POST",json!({"service":"hagency.inbox.snooze","args":{"id":snooze["id"],"minutes":60}}),"Reminders paused for one hour.".into(),resource,busy,notice),"Remind me in one hour" }
            }
        }
        button { class:"hg-button hg-secondary", disabled:busy(), onclick:move |_| action("/operations/call".into(),"POST",json!({"service":"hagency.inbox.seen","args":{"id":seen["id"]}}),"Marked as seen.".into(),resource,busy,notice),"Mark seen" }
    } }
}
