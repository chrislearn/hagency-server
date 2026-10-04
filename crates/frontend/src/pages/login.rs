use super::hagency::common::*;
use crate::{
    api::{auth, hagency},
    router::{self, Route},
    utils::{config, storage},
};
use dioxus::prelude::*;

#[component]
pub fn LoginPage() -> Element {
    let config = use_resource(|| async {
        let cfg = config::load_runtime_config().await;
        storage::set_item("server_name", &cfg.server_name);
        storage::set_item("pasion_public_url", &cfg.pasion_public_url);
        storage::set_item("oauth_client_id", &cfg.oauth_client_id);
        storage::set_item(
            "oauth_enabled",
            if cfg.oauth_enabled { "true" } else { "false" },
        );
        storage::set_item(
            "pasion_enabled",
            if cfg.pasion_enabled { "true" } else { "false" },
        );
        storage::set_item(
            "legacy_account_approval_enabled",
            if cfg.legacy_account_approval_enabled {
                "true"
            } else {
                "false"
            },
        );
        cfg
    });
    let account_access =
        use_resource(|| async { hagency::public("/account-access", None).await.ok() });
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut notice = use_signal(|| None);
    let nav = use_navigator();
    let cfg = config().unwrap_or_default();
    let ready = config().is_some();
    rsx! { div { class:"hg-public hg-page",
        div { class:"text-center space-y-2",
            h1 { class:"text-2xl font-bold tracking-tight","Hagency Server" }
            p { class:"text-muted-foreground","Manage your Matrix server, projects and agent providers." }
            if !cfg.server_name.is_empty() { p { class:"hg-note","Signing in to {cfg.server_name}" } }
        }
        Message { notice }
        if cfg.oauth_enabled {
            div { class:"hg-card hg-stack",
                p { "Sign in through Pasion using your server account." }
                button { class:"hg-button",disabled:busy() || !ready,onclick:move|_| {
                    busy.set(true);notice.set(None);spawn(async move {
                        if let Err(e)=auth::start_login().await { notice.set(Some((true,e.message))); }
                        busy.set(false);
                    });
                },"Sign in with Pasion" }
            }
        } else {
            form { class:"hg-card hg-stack",onsubmit:move|event| {
                event.prevent_default();if busy() || !ready { return; }
                let user=username().trim().to_owned();let secret=password();password.set(String::new());
                busy.set(true);notice.set(None);
                spawn(async move { match hagency::native_login(&user,&secret).await {
                    Ok(session)=> { router::set_admin_verdict(session.is_admin);
                        nav.replace(if session.is_admin { Route::Dashboard {} } else { Route::HagencyProjects {} });
                    },
                    Err(e)=>notice.set(Some((true,e.message))),
                } busy.set(false); });
            },
                label { class:"hg-field","Matrix ID or username" input { class:"hg-input",required:true,autocomplete:"username",value:username(),placeholder:"@you:server",oninput:move|e|username.set(e.value()) } }
                label { class:"hg-field","Password" input { class:"hg-input",r#type:"password",required:true,autocomplete:"current-password",value:password(),oninput:move|e|password.set(e.value()) } }
                button { class:"hg-button",r#type:"submit",disabled:busy() || !ready,if busy() { "Signing in…" } else { "Sign in" } }
            }
        }
        div { class:"hg-actions",
            if cfg.pasion_enabled { a { href:"/_pasion/",class:"hg-link","Account center" } a { href:"/_pasion/register",class:"hg-link","Create account" } }
            if account_access().flatten().is_some_and(|v|v["enabled"]==true) { Link { to:Route::AccountRequest {},class:"hg-link","Request a Matrix account" } }
        }
    } }
}
