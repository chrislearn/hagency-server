use dioxus::prelude::*;

use crate::api::client::{api_client, build_url};
use crate::components::ui::card::*;
use crate::components::ui::loading::LoadingSkeleton;
use crate::components::ui::toast::{ToastVariant, show_toast};
use crate::types::ExperimentalFeaturesModel;
use crate::utils::mxid::return_mxid;

#[component]
pub fn ExperimentalFeatures(user_id: String) -> Element {
    let uid = user_id.clone();
    let uid_for_resource = uid.clone();

    let mut features_data = use_resource(move || {
        let user = uid_for_resource.clone();
        async move {
            let mxid = return_mxid(&user);
            let encoded = urlencoding::encode(&mxid);
            let url = build_url(
                &format!("/_palpo/admin/v1/experimental_features/{encoded}"),
                &[],
            )
            .ok()?;
            let result: Result<ExperimentalFeaturesModel, _> = api_client(&url, "GET", None).await;
            result.ok()
        }
    });

    rsx! {
        Card {
            CardHeader {
                CardTitle { "Experimental Features" }
                CardDescription { "Toggle experimental Matrix features for this user" }
            }
            CardContent {
                match &*features_data.read() {
                    Some(Some(features)) => {
                        let features_clone = features.clone();
                        rsx! {
                            div { class: "space-y-3",
                                for (feature, enabled) in features_clone.features.iter() {
                                    {
                                        let feature_name = feature.clone();
                                        let is_enabled = *enabled;
                                        let uid_for_toggle = uid.clone();
                                        rsx! {
                                            div { class: "flex items-center justify-between py-2",
                                                div {
                                                    p { class: "text-sm font-medium", "{feature_name}" }
                                                }
                                                button {
                                                    class: {
                                                        if is_enabled {
                                                            "relative inline-flex h-6 w-11 items-center rounded-full bg-primary"
                                                        } else {
                                                            "relative inline-flex h-6 w-11 items-center rounded-full bg-input"
                                                        }
                                                    },
                                                    onclick: move |_| {
                                                        let user = uid_for_toggle.clone();
                                                        let feat = feature_name.clone();
                                                        let new_val = !is_enabled;
                                                        spawn(async move {
                                                            let mxid = return_mxid(&user);
                                                            let encoded = urlencoding::encode(&mxid);
                                                            if let Ok(url) = build_url(&format!("/_palpo/admin/v1/experimental_features/{encoded}"), &[]) {
                                                                let mut features_map = std::collections::HashMap::new();
                                                                features_map.insert(feat.clone(), new_val);
                                                                let body = serde_json::json!({ "features": features_map }).to_string();
                                                                let result: Result<serde_json::Value, _> = api_client(&url, "PUT", Some(body)).await;
                                                                match result {
                                                                    Ok(_) => {
                                                                        show_toast(&format!("{feat} {}", if new_val { "enabled" } else { "disabled" }), ToastVariant::Success);
                                                                        features_data.restart();
                                                                    }
                                                                    Err(e) => show_toast(&format!("Failed: {}", e.message), ToastVariant::Error),
                                                                }
                                                            }
                                                        });
                                                    },
                                                    span {
                                                        class: {
                                                            if is_enabled {
                                                                "inline-block h-4 w-4 transform rounded-full bg-white transition translate-x-6"
                                                            } else {
                                                                "inline-block h-4 w-4 transform rounded-full bg-white transition translate-x-1"
                                                            }
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(None) => rsx! {
                        p { class: "text-sm text-muted-foreground", "Unable to load experimental features." }
                    },
                    None => rsx! {
                        LoadingSkeleton {}
                    },
                }
            }
        }
    }
}
