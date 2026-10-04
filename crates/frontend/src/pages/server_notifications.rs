use dioxus::prelude::*;

use crate::api::palpo_admin;
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::card::*;
use crate::components::ui::loading::PageSkeleton;
use crate::components::ui::page_header::PageHeader;
use crate::components::ui::toast::{ToastVariant, show_toast};
use crate::utils::date::get_time_since;
use crate::utils::instance_config::get_palpo_admin_url;

#[component]
pub fn ServerNotifications() -> Element {
    let palpo_url = get_palpo_admin_url();

    let palpo_url_for_resource = palpo_url.clone();
    let mut notifications_data = use_resource(move || {
        let url = palpo_url_for_resource.clone();
        async move {
            if let Some(url) = url {
                palpo_admin::get_server_notifications(&url).await.ok()
            } else {
                None
            }
        }
    });

    if palpo_url.is_none() {
        return rsx! {
            div { class: "text-center p-8",
                p { class: "text-muted-foreground", "Palpo admin is not configured." }
            }
        };
    }

    let palpo_url_for_clear = palpo_url.clone();
    let handle_clear_all = move |_: MouseEvent| {
        let url = palpo_url_for_clear.clone();
        spawn(async move {
            if let Some(url) = url {
                match palpo_admin::delete_server_notifications(&url).await {
                    Ok(_) => {
                        show_toast("All notifications cleared", ToastVariant::Success);
                        notifications_data.restart();
                    }
                    Err(e) => {
                        show_toast(&format!("Failed: {}", e.message), ToastVariant::Error);
                    }
                }
            }
        });
    };

    rsx! {
        div { class: "space-y-6",
            PageHeader {
                title: "Notifications".to_string(),
                description: "Server notifications and alerts".to_string(),
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: handle_clear_all,
                    "Clear All"
                }
            }

            match &*notifications_data.read() {
                Some(Some(data)) => {
                    if data.notifications.is_empty() {
                        rsx! {
                            div { class: "text-center p-12",
                                div { class: "mx-auto h-12 w-12 rounded-full bg-muted flex items-center justify-center mb-4",
                                    crate::components::ui::icons::Icon {
                                        name: "flag".to_string(),
                                        class: "h-6 w-6 text-muted-foreground".to_string(),
                                    }
                                }
                                p { class: "text-muted-foreground", "No notifications" }
                            }
                        }
                    } else {
                        rsx! {
                            div { class: "space-y-4",
                                for notification in data.notifications.iter() {
                                    Card {
                                        CardContent { class: "p-4".to_string(),
                                            div { class: "space-y-2",
                                                p { class: "text-sm whitespace-pre-wrap", "{notification.output}" }
                                                if let Some(ref sent_at) = notification.sent_at {
                                                    p { class: "text-xs text-muted-foreground",
                                                        {
                                                            let ts = sent_at.parse::<u64>().unwrap_or(0);
                                                            get_time_since(ts)
                                                        }
                                                    }
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
                    div { class: "text-center p-8",
                        p { class: "text-muted-foreground", "Unable to fetch notifications." }
                    }
                },
                None => rsx! { PageSkeleton {} },
            }
        }
    }
}
