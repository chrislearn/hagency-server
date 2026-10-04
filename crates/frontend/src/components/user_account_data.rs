use dioxus::prelude::*;

use crate::api::client::{api_client, build_url};
use crate::components::ui::card::*;
use crate::components::ui::loading::LoadingSkeleton;
use crate::types::AccountDataModel;
use crate::utils::mxid::return_mxid;

#[component]
pub fn UserAccountData(user_id: String) -> Element {
    let uid_for_resource = user_id.clone();

    let account_data = use_resource(move || {
        let user = uid_for_resource.clone();
        async move {
            let mxid = return_mxid(&user);
            let encoded = urlencoding::encode(&mxid);
            let url = build_url(
                &format!("/_palpo/admin/v1/users/{encoded}/accountdata"),
                &[],
            )
            .ok()?;
            let result: Result<AccountDataModel, _> = api_client(&url, "GET", None).await;
            result.ok()
        }
    });

    rsx! {
        Card {
            CardHeader {
                CardTitle { "Account Data" }
                CardDescription { "Raw account data stored for this user" }
            }
            CardContent {
                match &*account_data.read() {
                    Some(Some(data)) => rsx! {
                        pre {
                            class: "text-xs font-mono bg-muted p-4 rounded-md overflow-auto max-h-96",
                            {serde_json::to_string_pretty(&data.account_data).unwrap_or_else(|_| "{}".to_string())}
                        }
                    },
                    Some(None) => rsx! {
                        p { class: "text-sm text-muted-foreground", "Unable to load account data." }
                    },
                    None => rsx! {
                        LoadingSkeleton {}
                    },
                }
            }
        }
    }
}
