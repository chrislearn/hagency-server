use std::sync::Mutex;
use std::sync::OnceLock;

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

use crate::utils::storage;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisableFeatures {
    #[serde(default)]
    pub support: bool,
    #[serde(default)]
    pub actions: bool,
    #[serde(default)]
    pub attributions: bool,
    #[serde(default)]
    pub federation: bool,
    #[serde(default)]
    pub monitoring: bool,
    #[serde(default)]
    pub notifications: bool,
    #[serde(default)]
    pub registration_tokens: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstanceConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub favicon_url: Option<String>,
    #[serde(default)]
    pub background_url: Option<String>,
    #[serde(default)]
    pub disabled: DisableFeatures,
}

static INSTANCE_CONFIG: OnceLock<Mutex<InstanceConfig>> = OnceLock::new();

fn config_mutex() -> &'static Mutex<InstanceConfig> {
    INSTANCE_CONFIG.get_or_init(|| Mutex::new(InstanceConfig::default()))
}

pub fn get_instance_config() -> InstanceConfig {
    config_mutex().lock().unwrap().clone()
}

pub fn set_instance_config(config: InstanceConfig) {
    *config_mutex().lock().unwrap() = config;
}

pub fn clear_instance_config() {
    *config_mutex().lock().unwrap() = InstanceConfig::default();
}

pub async fn fetch_instance_config() {
    let palpo_admin_url = storage::get_item("palpo_admin_url");
    if let Some(url) = palpo_admin_url {
        let config_url = format!("{url}/config");
        if let Ok(response) = Request::get(&config_url)
            .header("Accept", "application/json")
            .send()
            .await
        {
            if let Ok(config) = response.json::<InstanceConfig>().await {
                set_instance_config(config);
            }
        }
    }
}

pub fn is_palpo_admin_enabled() -> bool {
    storage::get_item("palpo_admin_url").is_some()
}

pub fn get_palpo_admin_url() -> Option<String> {
    storage::get_item("palpo_admin_url")
}
