use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A host-authenticated human, never deserialized from a request body.
#[derive(Clone, Debug)]
pub struct Actor {
    pub(crate) id: String,
    pub(crate) mxid: String,
}
impl Actor {
    pub fn id(&self) -> &str { &self.id }
    pub fn mxid(&self) -> &str { &self.mxid }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreationPolicy {
    pub default_allow: bool,
    #[serde(default)]
    pub allow: BTreeSet<String>,
    #[serde(default)]
    pub deny: BTreeSet<String>,
}
impl Default for CreationPolicy {
    fn default() -> Self { Self { default_allow: true, allow: BTreeSet::new(), deny: BTreeSet::new() } }
}
impl CreationPolicy {
    pub fn allows(&self, mxid: &str) -> bool {
        !self.deny.contains(mxid) && (self.default_allow || self.allow.contains(mxid))
    }
    pub fn validate(&self) -> Result<()> {
        if self.allow.len() + self.deny.len() > 10_000 { return Err(Error::Invalid("policy too large")); }
        for mxid in self.allow.iter().chain(&self.deny) { matrix_id(mxid, '@')?; }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoomCreationPolicy {
    InheritProject { #[serde(default)] deny: BTreeSet<String> },
    AllowList { allow: BTreeSet<String>, #[serde(default)] deny: BTreeSet<String> },
    Disabled,
}
impl Default for RoomCreationPolicy {
    fn default() -> Self { Self::InheritProject { deny: BTreeSet::new() } }
}
impl RoomCreationPolicy {
    pub fn allows(&self, mxid: &str) -> bool {
        match self {
            Self::InheritProject { deny } => !deny.contains(mxid),
            Self::AllowList { allow, deny } => allow.contains(mxid) && !deny.contains(mxid),
            Self::Disabled => false,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::InheritProject { deny } => CreationPolicy { default_allow: false, allow: BTreeSet::new(), deny: deny.clone() }.validate(),
            Self::AllowList { allow, deny } => CreationPolicy { default_allow: false, allow: allow.clone(), deny: deny.clone() }.validate(),
            Self::Disabled => Ok(()),
        }
    }
}

/// Authenticated Matrix observations from the host gateway. No Deserialize:
/// clients cannot upload a membership or power-level assertion as authority.
#[derive(Clone, Debug)]
pub struct RoomFacts {
    pub owner_mxid: String,
    pub puppet_mxid: Option<String>,
    pub room_id: String,
    pub space_id: String,
    pub observed_at: u64,
    pub owner_in_space: bool,
    pub owner_in_room: bool,
    pub puppet_in_room: bool,
    pub service_can_invite: bool,
    pub encrypted: bool,
}

#[derive(Clone, Debug)]
pub struct AdminFacts {
    pub actor_mxid: String,
    pub room_id: String,
    pub observed_at: u64,
    pub joined: bool,
    /// The gateway evaluates actual Matrix state-event power requirements.
    pub can_manage_policy: bool,
    pub is_space: bool,
    pub linked_space_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAgent {
    pub project_id: String,
    pub room_id: String,
    pub display_name: String,
    pub idempotency_key: String,
}
impl CreateAgent {
    pub fn validate(&self) -> Result<()> {
        bounded_key(&self.project_id)?;
        bounded_key(&self.idempotency_key)?;
        matrix_id(&self.room_id, '!')?;
        if self.display_name.trim().is_empty() || self.display_name.chars().count() > 64
            || self.display_name.chars().any(char::is_control) {
            return Err(Error::Invalid("invalid display name"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub owner_user_id: String,
    pub puppet_mxid: String,
    pub display_name: String,
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub id: String,
    pub agent_id: String,
    pub project_id: String,
    pub room_id: String,
    pub state: String,
    pub generation: u64,
}

pub fn matrix_id(value: &str, sigil: char) -> Result<()> {
    if value.len() > 255 || !value.starts_with(sigil) || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control) {
        return Err(Error::Invalid("invalid Matrix identifier"));
    }
    let Some((local, server)) = value[1..].split_once(':') else { return Err(Error::Invalid("invalid Matrix identifier")); };
    if local.is_empty() || server.is_empty() { return Err(Error::Invalid("invalid Matrix identifier")); }
    Ok(())
}
pub fn bounded_key(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 128 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return Err(Error::Invalid("invalid opaque key"));
    }
    Ok(())
}
