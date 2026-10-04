use dioxus::prelude::*;

use crate::api::pasion;
use crate::components::ui::badge::{Badge, BadgeVariant};
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::error_banner::ErrorBanner;
use crate::components::ui::loading::PageSkeleton;
use crate::components::ui::page_header::PageHeader;
use crate::components::ui::table::*;
use crate::utils::i18n::t;

fn channel_badge(channel: &str) -> Element {
    let variant = match channel {
        "email" => BadgeVariant::Default,
        "sms" => BadgeVariant::Secondary,
        _ => BadgeVariant::Outline,
    };
    rsx! { Badge { variant, "{channel}" } }
}

fn configuration_badge(configured: bool) -> Element {
    let (variant, label) = if configured {
        (BadgeVariant::Success, "Configured")
    } else {
        (BadgeVariant::Secondary, "Not configured")
    };
    rsx! { Badge { variant, "{label}" } }
}

#[component]
pub fn NotificationChannelsPage() -> Element {
    let mut channels_data =
        use_resource(|| async { pasion::pasion_get_notification_channels().await });

    rsx! {
        div { class: "space-y-6",
            PageHeader {
                title: t("pasion.notification_channels.title"),
                description: t("pasion.notification_channels.description"),
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| channels_data.restart(),
                    {t("common.refresh")}
                }
            }

            match &*channels_data.read() {
                Some(Ok(channels)) => rsx! {
                    div { class: "rounded-md border",
                        Table {
                            TableHeader {
                                TableRow {
                                    TableHead { "Channel" }
                                    TableHead { "Status" }
                                }
                            }
                            TableBody {
                                if channels.is_empty() {
                                    EmptyRow { colspan: 2, message: t("pasion.notification_channels.empty") }
                                } else {
                                    for channel in channels.iter() {
                                        {
                                            let channel_name = channel.channel.clone();
                                            let configured = channel.configured;

                                            rsx! {
                                                TableRow { key: "{channel_name}",
                                                    TableCell {
                                                        {channel_badge(&channel_name)}
                                                    }
                                                    TableCell {
                                                        {configuration_badge(configured)}
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
                Some(Err(e)) => rsx! {
                    ErrorBanner {
                        message: e.message.clone(),
                        on_retry: move |_| channels_data.restart(),
                    }
                },
                None => rsx! { PageSkeleton {} },
            }
        }
    }
}
