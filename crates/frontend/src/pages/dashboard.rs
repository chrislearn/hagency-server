use dioxus::prelude::*;

use crate::api::{reports, rooms, server_info, users};
use crate::components::ui::card::*;
use crate::components::ui::icons::Icon;
use crate::components::ui::loading::StatsSkeleton;
use crate::utils::cache::{get_cached, set_cached};
use crate::utils::i18n::t;
use crate::utils::perf;

/// Cache TTL in milliseconds (5 minutes)
const CACHE_TTL_MS: f64 = 300_000.0;

#[component]
pub fn Dashboard() -> Element {
    let server_version = use_resource(|| async {
        if let Some(cached) = get_cached("dashboard_server_version", CACHE_TTL_MS) {
            return Some(cached);
        }
        match server_info::get_server_version().await.ok() {
            Some(val) => {
                set_cached("dashboard_server_version", &val);
                Some(val)
            }
            None => None,
        }
    });
    let features = use_resource(|| async {
        if let Some(cached) = get_cached("dashboard_features", CACHE_TTL_MS) {
            return serde_json::from_str(&cached).ok();
        }
        match server_info::get_supported_features().await.ok() {
            Some(val) => {
                if let Ok(json) = serde_json::to_string(&val) {
                    set_cached("dashboard_features", &json);
                }
                Some(val)
            }
            None => None,
        }
    });
    let user_count = use_resource(|| async {
        if let Some(cached) = get_cached("dashboard_user_count", CACHE_TTL_MS) {
            return cached.parse::<u64>().ok();
        }
        match users::get_user_count().await.ok() {
            Some(val) => {
                set_cached("dashboard_user_count", &val.to_string());
                Some(val)
            }
            None => None,
        }
    });
    let room_count = use_resource(|| async {
        if let Some(cached) = get_cached("dashboard_room_count", CACHE_TTL_MS) {
            return cached.parse::<u64>().ok();
        }
        match rooms::get_room_count().await.ok() {
            Some(val) => {
                set_cached("dashboard_room_count", &val.to_string());
                Some(val)
            }
            None => None,
        }
    });
    let report_count = use_resource(|| async {
        if let Some(cached) = get_cached("dashboard_report_count", CACHE_TTL_MS) {
            return cached.parse::<u64>().ok();
        }
        match reports::get_report_count().await.ok() {
            Some(val) => {
                set_cached("dashboard_report_count", &val.to_string());
                Some(val)
            }
            None => None,
        }
    });
    let active_user_count = use_resource(|| async {
        if let Some(cached) = get_cached("dashboard_active_user_count", CACHE_TTL_MS) {
            return cached.parse::<u64>().ok();
        }
        match users::get_active_user_count().await.ok() {
            Some(val) => {
                set_cached("dashboard_active_user_count", &val.to_string());
                Some(val)
            }
            None => None,
        }
    });

    let is_loading = server_version.read().is_none() || features.read().is_none();

    if is_loading {
        return rsx! {
            div { class: "space-y-6",
                div {
                    h1 { class: "text-2xl font-bold tracking-tight", {t("dashboard.title")} }
                    p { class: "text-muted-foreground", {t("dashboard.welcome")} }
                }
                StatsSkeleton { count: 5 }
            }
        };
    }

    let version_str = server_version
        .read()
        .as_ref()
        .and_then(|v| v.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let features_data = features.read().clone().flatten();

    rsx! {
        div { class: "space-y-6",
            div {
                h1 { class: "text-2xl font-bold tracking-tight", {t("dashboard.title")} }
                p { class: "text-muted-foreground", {t("dashboard.welcome")} }
            }

            // Stats grid — compact cells sharing a single bordered container
            {
                let fmt_count = |r: &Resource<Option<u64>>| r.read().as_ref().and_then(|c| c.as_ref()).map(|c| c.to_string()).unwrap_or_else(|| "-".to_string());
                let users_val = fmt_count(&user_count);
                let rooms_val = fmt_count(&room_count);
                let reports_val = fmt_count(&report_count);
                let active_val = fmt_count(&active_user_count);
                let avg = perf::average_latency();
                let latency_val = if avg > 0.0 { format!("{:.0}ms", avg) } else { "-".to_string() };
                rsx! {
                    div { class: "grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 rounded-lg border glass-panel overflow-hidden",
                        // Server Version
                        div { class: "p-3 border-b border-r lg:border-b-0 border-border/50",
                            div { class: "flex items-center justify-between mb-1.5",
                                span { class: "text-[11px] font-medium uppercase tracking-wider text-muted-foreground", {t("server.version")} }
                                Icon { name: "server".to_string(), class: "h-3.5 w-3.5 text-muted-foreground".to_string() }
                            }
                            div { class: "text-xl font-bold leading-none", "{version_str}" }
                            div { class: "flex items-center gap-1.5 mt-1.5",
                                span { class: "relative flex h-1.5 w-1.5",
                                    span { class: "animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75" }
                                    span { class: "relative inline-flex rounded-full h-1.5 w-1.5 bg-green-500" }
                                }
                                p { class: "text-[10px] text-green-600 dark:text-green-400 font-medium uppercase tracking-wide", {t("dashboard.server_online")} }
                            }
                        }
                        // Users
                        div { class: "p-3 border-b lg:border-b-0 sm:border-r border-border/50",
                            div { class: "flex items-center justify-between mb-1.5",
                                span { class: "text-[11px] font-medium uppercase tracking-wider text-muted-foreground", {t("dashboard.total_users")} }
                                Icon { name: "users".to_string(), class: "h-3.5 w-3.5 text-muted-foreground".to_string() }
                            }
                            div { class: "text-xl font-bold leading-none", "{users_val}" }
                            p { class: "text-[10px] text-muted-foreground mt-1.5 truncate", {t("dashboard.total_registered_users")} }
                        }
                        // Rooms
                        div { class: "p-3 border-b sm:border-b-0 border-r border-border/50",
                            div { class: "flex items-center justify-between mb-1.5",
                                span { class: "text-[11px] font-medium uppercase tracking-wider text-muted-foreground", {t("dashboard.total_rooms")} }
                                Icon { name: "message-square".to_string(), class: "h-3.5 w-3.5 text-muted-foreground".to_string() }
                            }
                            div { class: "text-xl font-bold leading-none", "{rooms_val}" }
                            p { class: "text-[10px] text-muted-foreground mt-1.5 truncate", {t("dashboard.total_rooms_on_server")} }
                        }
                        // Reports
                        div { class: "p-3 border-b sm:border-b-0 lg:border-r border-border/50",
                            div { class: "flex items-center justify-between mb-1.5",
                                span { class: "text-[11px] font-medium uppercase tracking-wider text-muted-foreground", {t("dashboard.total_reports")} }
                                Icon { name: "flag".to_string(), class: "h-3.5 w-3.5 text-muted-foreground".to_string() }
                            }
                            div { class: "text-xl font-bold leading-none", "{reports_val}" }
                            p { class: "text-[10px] text-muted-foreground mt-1.5 truncate", {t("dashboard.pending_reports")} }
                        }
                        // Active Users
                        div { class: "p-3 border-r border-border/50",
                            div { class: "flex items-center justify-between mb-1.5",
                                span { class: "text-[11px] font-medium uppercase tracking-wider text-muted-foreground", {t("dashboard.active_users")} }
                                Icon { name: "user-check".to_string(), class: "h-3.5 w-3.5 text-muted-foreground".to_string() }
                            }
                            div { class: "text-xl font-bold leading-none", "{active_val}" }
                            p { class: "text-[10px] text-muted-foreground mt-1.5 truncate", {t("dashboard.non_guest_non_deactivated")} }
                        }
                        // API Latency
                        div { class: "p-3",
                            div { class: "flex items-center justify-between mb-1.5",
                                span { class: "text-[11px] font-medium uppercase tracking-wider text-muted-foreground", {t("dashboard.api_latency")} }
                                Icon { name: "activity".to_string(), class: "h-3.5 w-3.5 text-muted-foreground".to_string() }
                            }
                            div { class: "text-xl font-bold leading-none", "{latency_val}" }
                            p { class: "text-[10px] text-muted-foreground mt-1.5 truncate", {t("dashboard.avg_api_response")} }
                        }
                    }
                }
            }

            // Server Features
            if let Some(ref feat) = features_data {
                Card {
                    CardHeader {
                        CardTitle { class: "flex items-center gap-2".to_string(),
                            Icon { name: "activity".to_string(), class: "h-5 w-5".to_string() }
                            {t("server.features")}
                        }
                        CardDescription {
                            {t("dashboard.spec_description")}
                        }
                    }
                    CardContent {
                        div { class: "space-y-4",
                            if !feat.versions.is_empty() {
                                div {
                                    h4 { class: "text-sm font-medium mb-2", {t("dashboard.supported_versions")} }
                                    div { class: "flex flex-wrap gap-2",
                                        for version in feat.versions.iter() {
                                            span {
                                                class: "inline-flex items-center rounded-md bg-primary/10 px-2 py-1 text-xs font-medium text-primary",
                                                "{version}"
                                            }
                                        }
                                    }
                                }
                            }
                            {
                                let enabled_features: Vec<_> = feat.unstable_features.iter()
                                    .filter(|(_, enabled)| **enabled)
                                    .take(12)
                                    .collect();
                                if !enabled_features.is_empty() {
                                    rsx! {
                                        div {
                                            h4 { class: "text-sm font-medium mb-2", {t("dashboard.unstable_features")} }
                                            div { class: "grid gap-2 md:grid-cols-2 lg:grid-cols-3",
                                                for (feature, _) in enabled_features.iter() {
                                                    span {
                                                        class: "inline-flex items-center rounded-md bg-green-500/10 px-2 py-1 text-xs font-medium text-green-600 dark:text-green-400",
                                                        "{feature}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    rsx! {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
