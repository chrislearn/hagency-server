use super::common::*;
use crate::{api::hagency, router::Route, utils::storage};
use dioxus::prelude::*;
use serde_json::{Value, json};

#[component]
pub fn Approvals() -> Element {
    let mut resource = use_resource(|| async { get("/account-requests").await });
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
    let loaded = data(resource);
    rsx! { div { class:"hg-page",
        Heading { title:"Account approvals", description:"Approve or reject requests in the private administrator room. Palpo verifies the administrator who submits each decision.",resource }
        Status { resource }
        if loaded["enabled"] == false { p { class:"hg-note","Account approval requests are not enabled on this server." } }
        if loaded["enabled"] == true { article { class:"hg-card",
            h2 { "Administrator approval room" }
            RoomLink { room:text(&loaded,"roomId"),label:"Open private approval room" }
            Facts { rows:vec![("Approval account".into(),text(&loaded,"botMxid")),("Ready".into(),text(&loaded,"ready")),("Last error".into(),text(&loaded,"lastError"))] }
        } }
        div { class:"hg-grid",for request in rows(&loaded,"requests") { article { class:"hg-card",
            h2 { {text(&request,"displayName")} } span { class:"hg-badge",{text(&request,"status")} }
            p { {text(&request,"reason")} }
            Facts { rows:vec![("Matrix ID".into(),text(&request,"userId")),("Decided by".into(),text(&request,"decidedBy")),("Last error".into(),text(&request,"lastError"))] }
        } } }
        if loaded["enabled"] == true && rows(&loaded,"requests").is_empty() { p { class:"hg-note","No account requests." } }
    } }
}

const RECEIPT_KEY: &str = "hagency.account_request_receipt";
fn saved_receipt() -> Option<Value> {
    let value: Value = serde_json::from_str(&storage::get_item(RECEIPT_KEY)?).ok()?;
    let valid = |key: &str, len: usize| {
        value[key]
            .as_str()
            .is_some_and(|s| s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit()))
    };
    (valid("id", 32) && valid("receipt", 64)).then_some(value)
}
fn random_hex(len: usize) -> Result<String, String> {
    let mut bytes = vec![0; len];
    web_sys::window()
        .ok_or("Browser unavailable")?
        .crypto()
        .map_err(|_| "Secure browser randomness unavailable")?
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| "Secure browser randomness unavailable")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[component]
pub fn AccountRequest() -> Element {
    let availability = use_resource(|| async {
        hagency::public("/account-access", None)
            .await
            .map_err(|e| e.message)
    });
    let mut username = use_signal(String::new);
    let mut display = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut reason = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut repeat = use_signal(String::new);
    let mut receipt = use_signal(saved_receipt);
    let mut request = use_signal(|| Value::Null);
    let mut busy = use_signal(|| false);
    let mut notice = use_signal(|| None);
    let mut refresh = move || {
        if *busy.peek() {
            return;
        }
        let Some(body) = receipt() else {
            return;
        };
        busy.set(true);
        spawn(async move {
            match hagency::public("/account-requests/status", Some(body)).await {
                Ok(v) => request.set(v["request"].clone()),
                Err(e) => notice.set(Some((
                    true,
                    format!("{} Your receipt is kept for retry.", e.message),
                ))),
            }
            busy.set(false);
        });
    };
    use_effect(move || {
        if receipt().is_some() {
            refresh();
        }
    });
    use_future(move || async move {
        loop {
            gloo_timers::future::sleep(std::time::Duration::from_secs(10)).await;
            let terminal = ["registered", "rejected", "expired", "name_unavailable"]
                .contains(&text(&request(), "status").as_str());
            if receipt().is_some()
                && !terminal
                && web_sys::window()
                    .and_then(|w| w.document())
                    .is_some_and(|d| !d.hidden())
            {
                refresh();
            }
        }
    });
    let config = data(availability);
    let enabled = config["enabled"] == true;
    rsx! { div { class:"hg-page hg-public",
        h1 { "Request a Matrix account" }
        p { class:"hg-note","An administrator reviews your request before the account is created." }
        Message { notice } Status { resource:availability }
        if !enabled && availability().is_some_and(|v|v.is_ok()) { p { class:"hg-note","Account approval requests are unavailable on this server." } }
        if request().is_null() && enabled {
            form { class:"hg-card hg-form",onsubmit:move|event| {
                event.prevent_default();if busy() { return; }
                if password()!=repeat() { notice.set(Some((true,"Passwords do not match.".into())));return; }
                let capability=match receipt() { Some(v)=>v,None=>match (random_hex(16),random_hex(32)) {
                    (Ok(id),Ok(token))=>json!({"id":id,"receipt":token}), _=>{notice.set(Some((true,"Secure browser randomness unavailable.".into())));return;}
                } };
                storage::set_item(RECEIPT_KEY,&capability.to_string());
                if storage::get_item(RECEIPT_KEY).as_deref()!=Some(&capability.to_string()) { notice.set(Some((true,"Allow local storage so your request receipt can be saved.".into())));return; }
                receipt.set(Some(capability.clone()));let mut body=capability;
                for (key,value) in [("username",username()),("displayName",display()),("email",email()),("reason",reason()),("password",password())] { body[key]=json!(value); }
                busy.set(true);spawn(async move { match hagency::public("/account-requests",Some(body)).await {
                    Ok(v)=>{request.set(v["request"].clone());password.set(String::new());repeat.set(String::new());notice.set(None);},
                    Err(e)=>notice.set(Some((true,format!("{} Your request reference is kept for retry.",e.message))))
                }busy.set(false); });
            },
                label { class:"hg-field","Username" input { class:"hg-input",required:true,pattern: r"[a-z][a-z0-9_.=\-]{{0,63}}",maxlength:64,autocomplete:"username",value:username(),oninput:move|e|username.set(e.value()) } }
                label { class:"hg-field","Display name" input { class:"hg-input",required:true,maxlength:128,value:display(),oninput:move|e|display.set(e.value()) } }
                p { class:"hg-note hg-wide",{format!("Matrix ID: @{}:{}", username(),text(&config,"serverName"))} }
                label { class:"hg-field","Email" input { class:"hg-input",r#type:"email",required:true,value:email(),oninput:move|e|email.set(e.value()) } }
                label { class:"hg-field","New password" input { class:"hg-input",r#type:"password",required:true,minlength:12,autocomplete:"new-password",value:password(),oninput:move|e|password.set(e.value()) } }
                label { class:"hg-field","Confirm password" input { class:"hg-input",r#type:"password",required:true,minlength:12,autocomplete:"new-password",value:repeat(),oninput:move|e|repeat.set(e.value()) } }
                label { class:"hg-field hg-wide","Why would you like an account?" textarea { class:"hg-input",required:true,maxlength:1000,rows:3,value:reason(),oninput:move|e|reason.set(e.value()) } }
                button { class:"hg-button",r#type:"submit",disabled:busy(),"Send account request" }
            }
        }
        if !request().is_null() { article { class:"hg-card",
            h2 { {text(&request(),"userId")} } span { class:"hg-badge",{text(&request(),"status")} }
            p { class:"hg-note","Refresh to follow administrator approval and account creation. A registered account can sign in with the password you chose." }
            div { class:"hg-actions",
                button { class:"hg-button hg-secondary",disabled:busy(),onclick:move|_|refresh(),"Refresh status" }
                if ["registered","rejected","expired","name_unavailable"].contains(&text(&request(),"status").as_str()) {
                    button { class:"hg-button hg-secondary",onclick:move|_|{receipt.set(None);request.set(Value::Null);storage::remove_item(RECEIPT_KEY);notice.set(None);},"Request another account" }
                }
            }
        } }
        Link { to:Route::LoginPage {},class:"hg-link","Back to sign in" }
    } }
}
