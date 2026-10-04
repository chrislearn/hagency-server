//! Users the homeserver has seen in its rooms, local or remote
//! (`/_palpo/admin/v1/known_users`). Read-only: remote accounts belong to
//! their own homeservers.

use serde::Deserialize;

use crate::api::client::*;
use crate::types::ListResponse;
use crate::utils::error::HttpError;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KnownUser {
    pub user_id: String,
    pub server_name: String,
    pub is_local: bool,
    #[serde(default)]
    pub displayname: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    pub joined_rooms: u64,
    pub invited_rooms: u64,
    pub left_rooms: u64,
    pub banned_rooms: u64,
    pub total_rooms: u64,
    /// Milliseconds since the Unix epoch.
    pub last_membership_ts: u64,
}

#[derive(Debug, Deserialize)]
struct KnownUsersResponse {
    users: Vec<KnownUser>,
    total: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KnownUserRoom {
    pub room_id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub membership: String,
    pub sender: String,
    pub event_id: String,
    pub updated_ts: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KnownUserDetail {
    pub user_id: String,
    pub server_name: String,
    pub is_local: bool,
    #[serde(default)]
    pub displayname: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    pub has_account: bool,
    pub rooms: Vec<KnownUserRoom>,
}

/// Which users to list, by where their account lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    All,
    Local,
    Remote,
}

impl Origin {
    fn query_value(self) -> Option<&'static str> {
        match self {
            Origin::All => None,
            Origin::Local => Some("true"),
            Origin::Remote => Some("false"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn get_known_users(
    page: u64,
    per_page: u64,
    search_term: &str,
    server_name: &str,
    origin: Origin,
    order_by: &str,
    descending: bool,
) -> Result<ListResponse<KnownUser>, HttpError> {
    let from = ((page.max(1) - 1) * per_page).to_string();
    let limit = per_page.to_string();
    let mut params = vec![
        ("from", from.as_str()),
        ("limit", limit.as_str()),
        ("order_by", order_by),
        ("dir", if descending { "b" } else { "f" }),
    ];
    if !search_term.is_empty() {
        params.push(("search_term", search_term));
    }
    if !server_name.is_empty() {
        params.push(("server_name", server_name));
    }
    if let Some(local) = origin.query_value() {
        params.push(("local", local));
    }
    let url = build_url("/_palpo/admin/v1/known_users", &params)?;
    let response: KnownUsersResponse = api_client(&url, "GET", None).await?;
    Ok(ListResponse {
        data: response.users,
        total: response.total,
    })
}

pub async fn get_known_user(user_id: &str) -> Result<KnownUserDetail, HttpError> {
    let encoded = urlencoding::encode(user_id);
    let url = build_url(&format!("/_palpo/admin/v1/known_users/{encoded}"), &[])?;
    api_client(&url, "GET", None).await
}
