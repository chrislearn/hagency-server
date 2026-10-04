use dioxus::prelude::*;

use crate::api::pasion;
use crate::components::ui::badge::{Badge, BadgeVariant};
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::card::{Card, CardContent, CardHeader, CardTitle};
use crate::components::ui::error_banner::ErrorBanner;
use crate::components::ui::loading::PageSkeleton;
use crate::components::ui::page_header::PageHeader;
use crate::utils::i18n::t;

fn status_badge(status: &str) -> Element {
    let (variant, label) = match status {
        "healthy" | "ok" => (BadgeVariant::Success, t("pasion.status_healthy")),
        "degraded" => (BadgeVariant::Secondary, t("pasion.status_degraded")),
        "unhealthy" | "down" => (BadgeVariant::Destructive, t("pasion.status_unhealthy")),
        _ => (BadgeVariant::Outline, status.to_string()),
    };
    rsx! {
        Badge { variant, "{label}" }
    }
}

#[component]
pub fn ConnectorHealthPage() -> Element {
    let mut health_data = use_resource(|| async { pasion::pasion_get_connector_health().await });

    rsx! {
        div { class: "space-y-6",
            PageHeader {
                title: t("pasion.connector_health.title"),
                description: t("pasion.connector_health.description"),
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| health_data.restart(),
                    {t("common.refresh")}
                }
            }

            match &*health_data.read() {
                Some(Ok(connectors)) => rsx! {
                    if connectors.is_empty() {
                        div { class: "rounded-md border p-8 text-center text-muted-foreground",
                            {t("pasion.connector_health.empty")}
                        }
                    } else {
                        div { class: "grid gap-4 md:grid-cols-2 lg:grid-cols-3",
                            for connector in connectors.iter() {
                                {
                                    let provider = connector.provider.clone();
                                    let homeserver = connector.homeserver.clone();
                                    let status = connector.status.clone();
                                    let detail = connector.error.clone().unwrap_or_default();
                                    let border_class = match status.as_str() {
                                        "healthy" | "ok" => "border-green-500/30",
                                        "degraded" => "border-yellow-500/30",
                                        "unhealthy" | "down" => "border-destructive/30",
                                        _ => "",
                                    };

                                    rsx! {
                                        Card { class: border_class.to_string(),
                                            key: "{provider}",
                                            CardHeader {
                                                div { class: "flex items-center justify-between",
                                                    CardTitle { class: "text-base".to_string(), "{provider}" }
                                                    {status_badge(&status)}
                                                }
                                            }
                                            CardContent {
                                                div { class: "space-y-1",
                                                    div { class: "flex items-center justify-between text-sm",
                                                        span { class: "text-muted-foreground", {t("pasion.connector_health.homeserver")} }
                                                        span { class: "font-mono text-sm", "{homeserver}" }
                                                    }
                                                    if !detail.is_empty() {
                                                        p { class: "text-xs text-destructive mt-2 pt-2 border-t",
                                                            "{detail}"
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
                Some(Err(e)) => rsx! {
                    ErrorBanner {
                        message: e.message.clone(),
                        on_retry: move |_| health_data.restart(),
                    }
                },
                None => rsx! { PageSkeleton {} },
            }
        }
    }
}
