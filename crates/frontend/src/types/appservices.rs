use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppserviceSummary {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sender_localpart: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListAppservicesResponse {
    #[serde(default)]
    pub appservices: Vec<AppserviceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppserviceRegistration {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub as_token: String,
    #[serde(default)]
    pub hs_token: String,
    #[serde(default)]
    pub sender_localpart: String,
    /// Free-form JSON object — `{ "users": [...], "aliases": [...], "rooms": [...] }`.
    #[serde(default)]
    pub namespaces: serde_json::Value,
    #[serde(default)]
    pub rate_limited: Option<bool>,
    #[serde(default)]
    pub protocols: Option<Vec<String>>,
    #[serde(default)]
    pub receive_ephemeral: bool,
    #[serde(default, rename = "io.element.msc4190")]
    pub device_management: bool,
    #[serde(default)]
    pub disabled: bool,
}
