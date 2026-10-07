use crate::{Error, Result, model::{Actor, AdminFacts, CreationPolicy, RoomCreationPolicy, RoomFacts}};

pub const MAX_OBSERVATION_AGE: u64 = 30;

pub fn fresh(observed: u64, now: u64) -> Result<()> {
    if observed > now || now - observed > MAX_OBSERVATION_AGE { return Err(Error::Denied("state_unavailable")); }
    Ok(())
}

pub fn administer(actor: &Actor, room: &str, facts: &AdminFacts, now: u64) -> Result<()> {
    fresh(facts.observed_at, now)?;
    if facts.actor_mxid != actor.mxid() || facts.room_id != room || !facts.joined || !facts.can_manage_policy {
        return Err(Error::Denied("policy_admin_required"));
    }
    Ok(())
}

pub fn running(actor: &Actor, facts: &RoomFacts, room: &str, space: &str, now: u64) -> Result<()> {
    fresh(facts.observed_at, now)?;
    if facts.owner_mxid != actor.mxid() || facts.room_id != room || facts.space_id != space || !facts.owner_in_space || !facts.owner_in_room {
        return Err(Error::Denied("membership_required"));
    }
    Ok(())
}

pub fn creating(actor: &Actor, project: &CreationPolicy, room: &RoomCreationPolicy,
                facts: &RoomFacts, room_id: &str, space_id: &str, now: u64) -> Result<()> {
    running(actor, facts, room_id, space_id, now)?;
    if !project.allows(actor.mxid()) { return Err(Error::Denied("project_create_denied")); }
    if !room.allows(actor.mxid()) { return Err(Error::Denied("room_create_denied")); }
    if !facts.service_can_invite && !facts.puppet_in_room { return Err(Error::Denied("matrix_permission_missing")); }
    // This capability is enabled only when the client crypto path is implemented.
    if facts.encrypted { return Err(Error::Denied("encryption_not_supported")); }
    Ok(())
}
