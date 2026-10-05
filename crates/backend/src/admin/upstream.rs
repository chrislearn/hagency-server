use super::*;
use reqwest::{Client, Method, redirect::Policy};
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}
impl ApiError {
    pub fn new(status: u16, code: &str, message: &str) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for ApiError {}
impl From<diesel::result::Error> for ApiError {
    fn from(e: diesel::result::Error) -> Self {
        tracing::error!(error=%e,"admin persistence failed");
        err(
            503,
            "storage_unavailable",
            "Persistent storage is unavailable. Retry the same operation.",
        )
    }
}

#[derive(Clone)]
pub struct Upstream {
    pub url: Url,
    client: Client,
    label: &'static str,
}
impl Upstream {
    pub fn new(url: Url) -> Self {
        Self::with_label(url, "Palpo")
    }
    pub(crate) fn with_label(url: Url, label: &'static str) -> Self {
        Self {
            url,
            client: Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(12))
                .build()
                .expect("HTTP client"),
            label,
        }
    }
    pub(crate) async fn raw(
        &self,
        path: &str,
        token: Option<&str>,
        method: Method,
        body: Option<&Value>,
    ) -> Result<(u16, Value)> {
        let url = self
            .url
            .join(path)
            .map_err(|_| err(500, "invalid_upstream", "Invalid upstream route."))?;
        let mut req = self
            .client
            .request(method, url)
            .header("Content-Type", "application/json");
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }
        if let Some(body) = body {
            req = req.json(body);
        }
        let response = req.send().await.map_err(|_| {
            err(
                502,
                if self.label == "Palpo" {
                    "palpo_unreachable"
                } else {
                    "hagency_unreachable"
                },
                "The configured upstream did not respond.",
            )
        })?;
        let status = response.status().as_u16();
        let bytes = response.bytes().await.map_err(|_| {
            err(
                502,
                "upstream_unreachable",
                "The upstream response was interrupted.",
            )
        })?;
        let body = serde_json::from_slice(&bytes).unwrap_or(json!({}));
        Ok((status, body))
    }
    pub(crate) async fn call(
        &self,
        path: &str,
        token: &str,
        method: Method,
        body: Option<&Value>,
    ) -> Result<Value> {
        let (status, data) = self
            .raw(path, (!token.is_empty()).then_some(token), method, body)
            .await?;
        if !(200..300).contains(&status) {
            let c = s(&data["errcode"]);
            let c = if c.starts_with("M_") && c.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
            {
                c
            } else {
                let c = s(&data["code"]);
                if !c.is_empty()
                    && c.len() <= 80
                    && c.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                {
                    c
                } else {
                    "upstream_error"
                }
            };
            return Err(err(
                status,
                c,
                &format!("{} request failed (HTTP {status}, {c}).", self.label),
            ));
        }
        Ok(data)
    }
    pub(crate) async fn get(&self, path: &str, token: &str) -> Result<Value> {
        self.call(path, token, Method::GET, None).await
    }
    pub(crate) async fn post(&self, path: &str, token: &str, body: &Value) -> Result<Value> {
        self.call(path, token, Method::POST, Some(body)).await
    }
    pub(crate) async fn put(&self, path: &str, token: &str, body: &Value) -> Result<Value> {
        self.call(path, token, Method::PUT, Some(body)).await
    }
    pub(crate) async fn require_admin(&self, token: &str) -> Result<()> {
        match self.get("/_palpo/admin/v1/appservices", token).await {
            Err(e) if e.status == 404 => Err(err(
                501,
                "capability_unavailable",
                "Palpo App Service admin API is unavailable.",
            )),
            r => r.map(|_| ()),
        }
    }
    pub(crate) async fn user(&self, mxid: &str, token: &str) -> Result<Option<Value>> {
        match self
            .get(&format!("/_palpo/admin/v2/users/{}", enc(mxid)), token)
            .await
        {
            Err(e) if e.status == 404 => Ok(None),
            r => r.map(Some),
        }
    }
}

impl From<hagency_operations::Error> for ApiError {
    fn from(e: hagency_operations::Error) -> Self {
        Self {
            status: e.status,
            code: e.code,
            message: e.message,
        }
    }
}
