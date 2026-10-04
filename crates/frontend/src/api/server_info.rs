use crate::api::client::*;
use crate::types::*;
use crate::utils::error::HttpError;

pub async fn get_server_version() -> Result<String, HttpError> {
    let url = build_url("/_palpo/admin/v1/server_version", &[])?;
    let response: ServerVersionResponse = api_client(&url, "GET", None).await?;
    Ok(response.server_version)
}

pub async fn get_supported_features() -> Result<SupportedFeatures, HttpError> {
    let base_url = get_base_url()?;
    let url = format!("{base_url}/_matrix/client/versions");

    use gloo_net::http::Request;
    let response = Request::get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| HttpError::message(e.to_string()))?;

    response
        .json()
        .await
        .map_err(|e| HttpError::message(e.to_string()))
}

pub async fn get_server_version_unauthenticated(base_url: &str) -> Result<String, HttpError> {
    let url = format!("{base_url}/_palpo/admin/v1/server_version");

    use gloo_net::http::Request;
    let response = Request::get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| HttpError::message(e.to_string()))?;

    let resp: ServerVersionResponse = response
        .json()
        .await
        .map_err(|e| HttpError::message(e.to_string()))?;

    Ok(resp.server_version)
}
