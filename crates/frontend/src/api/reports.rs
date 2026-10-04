use crate::api::client::*;
use crate::types::*;
use crate::utils::cache::{cached, invalidate_cached_prefix, remove_cached};
use crate::utils::error::HttpError;

const REPORT_LIST_CACHE_TTL_MS: f64 = 30_000.0;
const REPORT_LIST_CACHE_PREFIX: &str = "reports:list:";
const DASHBOARD_REPORT_COUNT_CACHE_KEY: &str = "dashboard_report_count";

fn report_list_cache_key(page: u64, per_page: u64, order_by: &str, order: &str) -> String {
    format!("{REPORT_LIST_CACHE_PREFIX}{page}:{per_page}:{order_by}:{order}")
}

pub fn invalidate_report_related_caches() {
    invalidate_cached_prefix(REPORT_LIST_CACHE_PREFIX);
    remove_cached(DASHBOARD_REPORT_COUNT_CACHE_KEY);
}

pub async fn get_report_count() -> Result<u64, HttpError> {
    let url = build_url("/_palpo/admin/v1/event_reports", &[("limit", "1")])?;
    let response: EventReportsResponse = api_client(&url, "GET", None).await?;
    Ok(response.total)
}

pub async fn get_reports(
    page: u64,
    per_page: u64,
    order_by: &str,
    order: &str,
) -> Result<ListResponse<EventReport>, HttpError> {
    let from = (page - 1) * per_page;
    let from_str = from.to_string();
    let limit_str = per_page.to_string();
    let dir = get_search_order(order);

    let url = build_url(
        "/_palpo/admin/v1/event_reports",
        &[
            ("from", &from_str),
            ("limit", &limit_str),
            ("order_by", order_by),
            ("dir", dir),
        ],
    )?;

    let response: EventReportsResponse = api_client(&url, "GET", None).await?;

    Ok(ListResponse {
        data: response.event_reports,
        total: response.total,
    })
}

pub async fn get_reports_cached(
    page: u64,
    per_page: u64,
    order_by: &str,
    order: &str,
) -> Result<ListResponse<EventReport>, HttpError> {
    let cache_key = report_list_cache_key(page, per_page, order_by, order);
    cached(
        &cache_key,
        REPORT_LIST_CACHE_TTL_MS,
        get_reports(page, per_page, order_by, order),
    )
    .await
}

pub async fn get_report(id: u64) -> Result<EventReport, HttpError> {
    let url = build_url(&format!("/_palpo/admin/v1/event_reports/{id}"), &[])?;
    api_client(&url, "GET", None).await
}

pub async fn update_report_status(id: u64, status: &str) -> Result<EventReport, HttpError> {
    let url = build_url(&format!("/_palpo/admin/v1/event_reports/{id}"), &[])?;
    let body = serde_json::json!({ "status": status }).to_string();
    let report: EventReport = api_client(&url, "PUT", Some(body)).await?;
    invalidate_report_related_caches();
    Ok(report)
}

pub async fn delete_report(id: u64) -> Result<(), HttpError> {
    let url = build_url(&format!("/_palpo/admin/v1/event_reports/{id}"), &[])?;
    let _: serde_json::Value = api_client(&url, "DELETE", None).await?;
    invalidate_report_related_caches();
    Ok(())
}
