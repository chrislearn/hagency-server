//! Owner-owned stable device selection; lease takeover never grants another device.
use super::*;
use diesel::QueryableByName;
#[derive(Debug, Serialize, QueryableByName)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionInstance {
    #[diesel(sql_type=Text)]
    pub id: String,
    #[diesel(sql_type=Text)]
    pub agent_id: String,
    #[diesel(sql_type=Text)]
    pub device_id: String,
    #[diesel(sql_type=Text)]
    pub name: String,
    #[diesel(sql_type=BigInt)]
    pub generation: i64,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetExecutionInstance {
    pub device_id: String,
    pub name: String,
    pub expected_generation: i64,
}
#[derive(Serialize, QueryableByName)]
#[serde(rename_all = "camelCase")]
pub struct OwnerDevice {
    #[diesel(sql_type=Text)]
    pub id: String,
    #[diesel(sql_type=Text)]
    pub name: String,
    #[diesel(sql_type=BigInt)]
    pub generation: i64,
    #[diesel(sql_type=Bool)]
    pub revoked: bool,
}
pub(crate) async fn require_assigned(
    db: &mut AsyncPgConnection,
    p: &Principal,
    agent: &str,
) -> Result<()> {
    let device = p
        .device_id
        .as_deref()
        .ok_or(Error::Unauthorized("device_authorization_required"))?;
    sql_query("SELECT true AS matched FROM hagency_agent_v1.execution_instances WHERE agent_id=$1 AND owner_user_id=$2 AND device_id=$3 FOR SHARE").bind::<Text,_>(agent).bind::<Text,_>(&p.user_id).bind::<Text,_>(device).get_result::<Flag>(db).await.map_err(|_|Error::Unauthorized("execution_instance_device_required"))?;
    Ok(())
}
impl DomainStore {
    pub async fn owner_devices(&self, p: &Principal, now: i64) -> Result<Vec<OwnerDevice>> {
        let mut db = self.db.lock().await;
        (*db).transaction::<_,Error,_>(async |db:&mut AsyncPgConnection| {
            Self::authorize(db,p,now).await?;
            Ok(sql_query("SELECT id,name,generation,revoked FROM hagency_agent_v1.devices WHERE user_id=$1 ORDER BY id").bind::<Text,_>(&p.user_id).load(db).await?)
        }).await
    }
    pub async fn execution_instance(
        &self,
        p: &Principal,
        agent: &str,
        now: i64,
    ) -> Result<Option<ExecutionInstance>> {
        let mut db = self.db.lock().await;
        (*db).transaction::<_,Error,_>(async |db:&mut AsyncPgConnection| {
            Self::authorize(db,p,now).await?;Self::agent_db(db,p,agent).await?;
            Ok(sql_query("SELECT id,agent_id,device_id,name,generation FROM hagency_agent_v1.execution_instances WHERE agent_id=$1 AND owner_user_id=$2").bind::<Text,_>(agent).bind::<Text,_>(&p.user_id).get_result(db).await.optional()?)
        }).await
    }
    pub async fn set_execution_instance(
        &self,
        p: &Principal,
        agent: &str,
        input: SetExecutionInstance,
        now: i64,
    ) -> Result<ExecutionInstance> {
        key(&input.device_id)?;
        if input.name.trim().is_empty()
            || input.name.chars().count() > 64
            || input.name.chars().any(char::is_control)
            || input.expected_generation < 0
        {
            return Err(Error::Invalid("invalid_execution_instance"));
        }
        let mut db = self.db.lock().await;
        (*db).transaction::<_,Error,_>(async |db:&mut AsyncPgConnection| {
            Self::authorize(db,p,now).await?;let current_agent=Self::agent_db(db,p,agent).await?;
            if !matches!(current_agent.state.as_str(),"creating"|"active"|"suspended") {return Err(Error::Conflict("agent_retired"));}
            sql_query("SELECT true AS matched FROM hagency_agent_v1.devices WHERE id=$1 AND user_id=$2 AND NOT revoked FOR SHARE").bind::<Text,_>(&input.device_id).bind::<Text,_>(&p.user_id).get_result::<Flag>(db).await.map_err(|_|Error::Unauthorized("owner_device_required"))?;
            let current=sql_query("SELECT id,agent_id,device_id,name,generation FROM hagency_agent_v1.execution_instances WHERE agent_id=$1 FOR UPDATE").bind::<Text,_>(agent).get_result::<ExecutionInstance>(db).await.optional()?;
            if current.as_ref().map_or(0,|i|i.generation)!=input.expected_generation {return Err(Error::Conflict("execution_instance_changed"));}
            let changed=current.as_ref().is_some_and(|i|i.device_id!=input.device_id);
            if changed {
                sql_query("UPDATE hagency_agent_v1.agents SET generation=generation+1 WHERE id=$1").bind::<Text,_>(agent).execute(db).await?;
                // Existing local tools cannot be forced to stop. Preserve uncertainty.
                sql_query("UPDATE hagency_agent_v1.owner_events SET state=CASE WHEN state='running' THEN 'unknown' ELSE 'pending' END WHERE agent_id=$1 AND state IN ('offered','acknowledged','running')").bind::<Text,_>(agent).execute(db).await?;
                sql_query("UPDATE hagency_agent_v1.reply_outbox SET state='cancelled' WHERE agent_id=$1 AND state='pending'").bind::<Text,_>(agent).execute(db).await?;
                sql_query("UPDATE hagency_agent_v1.execution_leases SET expires_at_ms=0 WHERE agent_id=$1").bind::<Text,_>(agent).execute(db).await?;
            }
            let id=match &current {Some(instance)=>instance.id.clone(),None=>format!("ins_{}",entity_id()?)};
            let next=current.as_ref().map_or(1,|i|i.generation+1);
            let result=sql_query("INSERT INTO hagency_agent_v1.execution_instances(id,agent_id,owner_user_id,device_id,name,generation) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(agent_id) DO UPDATE SET device_id=EXCLUDED.device_id,name=EXCLUDED.name,generation=EXCLUDED.generation RETURNING id,agent_id,device_id,name,generation").bind::<Text,_>(id).bind::<Text,_>(agent).bind::<Text,_>(&p.user_id).bind::<Text,_>(&input.device_id).bind::<Text,_>(input.name.trim()).bind::<BigInt,_>(next).get_result(db).await?;
            Self::audit(db,p,"agent.execution_instance",agent,now).await?;Ok(result)
        }).await
    }
}
