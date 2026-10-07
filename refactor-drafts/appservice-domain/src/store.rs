//! Trusted-host domain writer. Matrix facts must originate in the authenticated
//! gateway, never in a network request body. Each mutation owns one SQL transaction.
use crate::{Error, Result, authorization, identifier, model::*};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};

const APPLICATION_ID: i64 = 0x48415332;
const SCHEMA_VERSION: i64 = 1;

pub struct Store { db: Connection, server_name: String, namespace: String }
impl Store {
    pub fn open(path: &Path, server_name: &str, namespace: &str) -> Result<Self> {
        matrix_id(&format!("@probe:{server_name}"), '@')?;
        if !namespace.starts_with("_hagency_") || namespace.len()>64
            || !namespace.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c==b'_') {
            return Err(Error::Invalid("use a bounded service namespace starting _hagency_"));
        }
        // Inspect before enabling WAL, changing pragmas, or initializing tables.
        let db = Connection::open(path)?;
        let app: i64 = db.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let tables: i64 = db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'", [], |r| r.get(0))?;
        if tables == 0 && app == 0 && version == 0 {
            db.execute_batch("BEGIN IMMEDIATE")?;
            let initialized = (|| -> Result<()> {
                db.execute_batch(include_str!("schema.sql"))?;
                db.execute("INSERT INTO metadata VALUES('server_name',?1),('namespace',?2)", params![server_name,namespace])?;
                db.pragma_update(None,"application_id",APPLICATION_ID)?;
                db.pragma_update(None,"user_version",SCHEMA_VERSION)?;
                Ok(())
            })();
            match initialized { Ok(())=>db.execute_batch("COMMIT")?, Err(e)=>{let _=db.execute_batch("ROLLBACK");return Err(e);} }
        } else if app != APPLICATION_ID || version != SCHEMA_VERSION { return Err(Error::ForeignDatabase); }
        let identity: Option<(String,String)> = db.query_row(
            "SELECT a.value,b.value FROM metadata a JOIN metadata b WHERE a.key='server_name' AND b.key='namespace'",[],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if identity.as_ref() != Some(&(server_name.to_owned(),namespace.to_owned())) { return Err(Error::ForeignDatabase); }
        db.pragma_update(None,"foreign_keys",true)?;
        db.busy_timeout(Duration::from_secs(2))?;
        Ok(Self { db, server_name: server_name.into(), namespace: namespace.into() })
    }

    /// Only the authenticated Matrix gateway calls this after whoami verification.
    pub fn authenticated_human(&mut self, mxid: &str) -> Result<Actor> {
        matrix_id(mxid,'@')?;
        if mxid[1..].split_once(':').map(|(_,s)|s) != Some(self.server_name.as_str()) || mxid.starts_with(&format!("@{}",self.namespace)) {
            return Err(Error::Denied("local_human_required"));
        }
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing:Option<(String,bool)>=tx.query_row("SELECT id,active FROM users WHERE mxid=?1",[mxid],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let id=match existing {
            Some((id,true))=>id,
            Some((_,false))=>return Err(Error::Denied("account_inactive")),
            None=>{let id=identifier("user_")?;tx.execute("INSERT INTO users VALUES(?1,?2,1)",params![id,mxid])?;id}
        };
        tx.commit()?;
        Ok(Actor{id,mxid:mxid.into()})
    }
    fn user(db:&Connection,actor:&Actor)->Result<()> {
        let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND mxid=?2 AND active=1)",params![actor.id,actor.mxid],|r|r.get(0))?;
        if !valid { return Err(Error::Denied("account_inactive")); }
        Ok(())
    }
    pub fn register_project(&mut self,actor:&Actor,space:&str,facts:&AdminFacts,now:u64)->Result<String> {
        matrix_id(space,'!')?;
        authorization::administer(actor,space,facts,now)?;
        if !facts.is_space { return Err(Error::Invalid("project requires a Matrix Space")); }
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;
        let existing:Option<String>=tx.query_row("SELECT id FROM projects WHERE space_id=?1",[space],|r|r.get(0)).optional()?;
        if let Some(id)=existing { return Ok(id); }
        let id=identifier("project_")?;
        tx.execute("INSERT INTO projects VALUES(?1,?2,1,?3,1)",params![id,space,serde_json::to_string(&CreationPolicy::default())?])?;
        Self::audit(&tx,actor,"project.register",&id,now)?;
        tx.commit()?;
        Ok(id)
    }
    /// The gateway must additionally verify the actual m.space.child relation.
    pub fn register_room(&mut self,actor:&Actor,project:&str,room:&str,space_facts:&AdminFacts,room_facts:&AdminFacts,now:u64)->Result<()> {
        matrix_id(room,'!')?;
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;
        let space=Self::space(&tx,project)?;
        authorization::administer(actor,&space,space_facts,now)?;
        authorization::administer(actor,room,room_facts,now)?;
        if room_facts.is_space { return Err(Error::Invalid("discussion room must not be a Space")); }
        if room_facts.linked_space_id.as_deref()!=Some(space.as_str()) { return Err(Error::Denied("space_relationship_required")); }
        let existing:Option<String>=tx.query_row("SELECT project_id FROM rooms WHERE room_id=?1",[room],|r|r.get(0)).optional()?;
        if let Some(id)=existing { return if id==project {Ok(())} else {Err(Error::Conflict)}; }
        tx.execute("INSERT INTO rooms VALUES(?1,?2,1,?3,1)",params![room,project,serde_json::to_string(&RoomCreationPolicy::default())?])?;
        Self::audit(&tx,actor,"room.register",room,now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn set_project_policy(&mut self,actor:&Actor,project:&str,expected:u64,policy:&CreationPolicy,facts:&AdminFacts,now:u64)->Result<u64> {
        policy.validate()?;
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;
        authorization::administer(actor,&Self::space(&tx,project)?,facts,now)?;
        if tx.execute("UPDATE projects SET creation_policy=?1,revision=revision+1 WHERE id=?2 AND revision=?3",params![serde_json::to_string(policy)?,project,expected])? !=1 { return Err(Error::StaleRevision); }
        Self::audit(&tx,actor,"project.creation_policy",project,now)?;
        tx.commit()?;
        Ok(expected+1)
    }
    pub fn set_room_policy(&mut self,actor:&Actor,room:&str,expected:u64,policy:&RoomCreationPolicy,facts:&AdminFacts,now:u64)->Result<u64> {
        policy.validate()?;
        authorization::administer(actor,room,facts,now)?;
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;
        if tx.execute("UPDATE rooms SET creation_policy=?1,revision=revision+1 WHERE room_id=?2 AND revision=?3",params![serde_json::to_string(policy)?,room,expected])? !=1 { return Err(Error::StaleRevision); }
        Self::audit(&tx,actor,"room.creation_policy",room,now)?;
        tx.commit()?;
        Ok(expected+1)
    }
    fn space(db:&Connection,project:&str)->Result<String> {
        db.query_row("SELECT space_id FROM projects WHERE id=?1 AND active=1",[project],|r|r.get(0)).optional()?.ok_or(Error::NotFound)
    }
    fn policies(db:&Connection,project:&str,room:&str)->Result<(String,CreationPolicy,RoomCreationPolicy)> {
        let row:Option<(String,String,String)>=db.query_row("SELECT p.space_id,p.creation_policy,r.creation_policy FROM projects p JOIN rooms r ON r.project_id=p.id WHERE p.id=?1 AND r.room_id=?2 AND p.active=1 AND r.active=1",params![project,room],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let (space,p,r)=row.ok_or(Error::NotFound)?;
        Ok((space,serde_json::from_str(&p)?,serde_json::from_str(&r)?))
    }
    fn audit(db:&Connection,actor:&Actor,operation:&str,object:&str,now:u64)->Result<()> {
        db.execute("INSERT INTO audit(actor,operation,object_id,at) VALUES(?1,?2,?3,?4)",params![actor.id,operation,object,now])?;Ok(())
    }
    pub fn create_agent(&mut self,actor:&Actor,request:&CreateAgent,facts:&RoomFacts,now:u64)->Result<(Agent,Binding)> {
        request.validate()?;
        let digest=format!("{:x}",Sha256::digest(serde_json::to_vec(request)?));
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;
        let previous:Option<(String,String)>=tx.query_row("SELECT digest,result FROM commands WHERE actor=?1 AND operation='agent.create' AND key=?2",params![actor.id,request.idempotency_key],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((old,result))=previous {
            if old!=digest { return Err(Error::Conflict); }
            // Replay returns the identity, not a renewed permission to execute.
            let (agent_id,binding_id):(String,String)=serde_json::from_str(&result)?;
            return Ok((Self::agent_in(&tx,actor,&agent_id)?,Self::binding_in(&tx,actor,&binding_id)?));
        }
        let (space,project_policy,room_policy)=Self::policies(&tx,&request.project_id,&request.room_id)?;
        authorization::creating(actor,&project_policy,&room_policy,facts,&request.room_id,&space,now)?;
        let id=identifier("agent_")?;
        let agent=Agent{id:id.clone(),owner_user_id:actor.id.clone(),puppet_mxid:format!("@{}{id}:{}",self.namespace,self.server_name),display_name:request.display_name.clone(),state:"creating".into()};
        tx.execute("INSERT INTO agents VALUES(?1,?2,?3,?4,'creating')",params![agent.id,agent.owner_user_id,agent.puppet_mxid,agent.display_name])?;
        let binding=Binding{id:identifier("binding_")?,agent_id:agent.id.clone(),project_id:request.project_id.clone(),room_id:request.room_id.clone(),state:"joining".into(),generation:1};
        Self::insert_binding(&tx,&binding)?;
        tx.execute("INSERT INTO commands VALUES(?1,'agent.create',?2,?3,?4)",params![actor.id,request.idempotency_key,digest,serde_json::to_string(&(agent.id.clone(),binding.id.clone()))?])?;
        Self::audit(&tx,actor,"agent.create",&agent.id,now)?;
        tx.commit()?;
        Ok((agent,binding))
    }
    fn insert_binding(db:&Connection,binding:&Binding)->Result<()> {
        db.execute("INSERT INTO bindings VALUES(?1,?2,?3,?4,?5,?6)",params![binding.id,binding.agent_id,binding.project_id,binding.room_id,binding.state,binding.generation])?;Ok(())
    }
    pub fn bind_room(&mut self,actor:&Actor,agent_id:&str,project:&str,room:&str,facts:&RoomFacts,now:u64)->Result<Binding> {
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;
        let agent=Self::agent_in(&tx,actor,agent_id)?;
        if !matches!(agent.state.as_str(),"active"|"creating") { return Err(Error::Denied("agent_inactive")); }
        let (space,p,r)=Self::policies(&tx,project,room)?;
        authorization::creating(actor,&p,&r,facts,room,&space,now)?;
        let old:Option<String>=tx.query_row("SELECT id FROM bindings WHERE agent_id=?1 AND room_id=?2",params![agent_id,room],|r|r.get(0)).optional()?;
        if let Some(id)=old { return Self::binding_in(&tx,actor,&id); }
        let binding=Binding{id:identifier("binding_")?,agent_id:agent_id.into(),project_id:project.into(),room_id:room.into(),state:"joining".into(),generation:1};
        Self::insert_binding(&tx,&binding)?;
        Self::audit(&tx,actor,"agent.bind",&binding.id,now)?;
        tx.commit()?;
        Ok(binding)
    }
    /// Activation is based on fresh Matrix membership, not merely account creation.
    pub fn activate_binding(&mut self,actor:&Actor,id:&str,facts:&RoomFacts,now:u64)->Result<Binding> {
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;
        let b=Self::binding_in(&tx,actor,id)?;
        if b.state!="joining" {return Err(Error::Denied("binding_not_joining"));}
        let agent=Self::agent_in(&tx,actor,&b.agent_id)?;
        if !matches!(agent.state.as_str(),"active"|"creating") {return Err(Error::Denied("agent_inactive"));}
        let (space,p,r)=Self::policies(&tx,&b.project_id,&b.room_id)?;
        authorization::creating(actor,&p,&r,facts,&b.room_id,&space,now)?;
        if !facts.puppet_in_room || facts.puppet_mxid.as_deref()!=Some(agent.puppet_mxid.as_str()) {return Err(Error::Denied("puppet_membership_required"));}
        tx.execute("UPDATE bindings SET state='active' WHERE id=?1",[id])?;
        tx.execute("UPDATE agents SET state='active' WHERE id=?1 AND state='creating'",[&b.agent_id])?;
        let result=Self::binding_in(&tx,actor,id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn runnable(&self,actor:&Actor,id:&str,facts:&RoomFacts,now:u64)->Result<Binding> {
        Self::user(&self.db,actor)?;
        let b=Self::binding_in(&self.db,actor,id)?;
        if b.state!="active" || Self::agent_in(&self.db,actor,&b.agent_id)?.state!="active" {return Err(Error::Denied("binding_revoked"));}
        let (space,_,_)=Self::policies(&self.db,&b.project_id,&b.room_id)?;
        authorization::running(actor,facts,&b.room_id,&space,now)?;
        let agent=Self::agent_in(&self.db,actor,&b.agent_id)?;
        if !facts.puppet_in_room || facts.puppet_mxid.as_deref()!=Some(agent.puppet_mxid.as_str()) {return Err(Error::Denied("puppet_membership_required"));}
        Ok(b)
    }
    pub fn suspend_project_bindings(&mut self,actor:&Actor,project:&str,facts:&AdminFacts,now:u64)->Result<()> {
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;
        authorization::administer(actor,&Self::space(&tx,project)?,facts,now)?;
        tx.execute("UPDATE bindings SET state='suspended',generation=generation+1 WHERE project_id=?1 AND state IN('joining','active')",[project])?;
        Self::audit(&tx,actor,"project.suspend_service",project,now)?;
        tx.commit()?;Ok(())
    }
    pub fn retire_agent(&mut self,actor:&Actor,id:&str,now:u64)->Result<()> {
        let tx=self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::user(&tx,actor)?;Self::agent_in(&tx,actor,id)?;
        tx.execute("UPDATE agents SET state='retiring' WHERE id=?1 AND state NOT IN('retiring','retired')",[id])?;
        tx.execute("UPDATE bindings SET state='revoked',generation=generation+1 WHERE agent_id=?1 AND state NOT IN('left','revoked')",[id])?;
        Self::audit(&tx,actor,"agent.retire",id,now)?;tx.commit()?;Ok(())
    }
    fn agent_in(db:&Connection,actor:&Actor,id:&str)->Result<Agent> {
        db.query_row("SELECT id,owner_user_id,puppet_mxid,display_name,state FROM agents WHERE id=?1 AND owner_user_id=?2",params![id,actor.id],|r|Ok(Agent{id:r.get(0)?,owner_user_id:r.get(1)?,puppet_mxid:r.get(2)?,display_name:r.get(3)?,state:r.get(4)?})).optional()?.ok_or(Error::NotFound)
    }
    pub fn agent(&self,actor:&Actor,id:&str)->Result<Agent> {Self::user(&self.db,actor)?;Self::agent_in(&self.db,actor,id)}
    fn binding_in(db:&Connection,actor:&Actor,id:&str)->Result<Binding> {
        db.query_row("SELECT b.id,b.agent_id,b.project_id,b.room_id,b.state,b.generation FROM bindings b JOIN agents a ON a.id=b.agent_id WHERE b.id=?1 AND a.owner_user_id=?2",params![id,actor.id],|r|Ok(Binding{id:r.get(0)?,agent_id:r.get(1)?,project_id:r.get(2)?,room_id:r.get(3)?,state:r.get(4)?,generation:r.get(5)?})).optional()?.ok_or(Error::NotFound)
    }
}

#[cfg(test)]
mod tests;
