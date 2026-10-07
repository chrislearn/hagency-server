use super::*;
use std::collections::BTreeSet;

const NOW:u64=100;
fn admin(actor:&Actor,room:&str,is_space:bool,space:Option<&str>)->AdminFacts {
    AdminFacts{actor_mxid:actor.mxid().into(),room_id:room.into(),observed_at:NOW,joined:true,can_manage_policy:true,is_space,linked_space_id:space.map(str::to_owned)}
}
fn facts(actor:&Actor,space:&str,room:&str)->RoomFacts {
    RoomFacts{owner_mxid:actor.mxid().into(),puppet_mxid:None,space_id:space.into(),room_id:room.into(),observed_at:NOW,owner_in_space:true,owner_in_room:true,puppet_in_room:false,service_can_invite:true,encrypted:false}
}
fn fixture()->(tempfile::TempDir,Store,Actor,Actor,String,RoomFacts) {
    let dir=tempfile::tempdir().unwrap();
    let mut s=Store::open(&dir.path().join("server.sqlite"),"example.test","_hagency_test_").unwrap();
    let a=s.authenticated_human("@alice:example.test").unwrap();
    let b=s.authenticated_human("@bob:example.test").unwrap();
    let p=s.register_project(&a,"!space:example.test",&admin(&a,"!space:example.test",true,None),NOW).unwrap();
    s.register_room(&a,&p,"!room:example.test",&admin(&a,"!space:example.test",true,None),&admin(&a,"!room:example.test",false,Some("!space:example.test")),NOW).unwrap();
    let f=facts(&a,"!space:example.test","!room:example.test");
    (dir,s,a,b,p,f)
}
fn request(project:&str)->CreateAgent {CreateAgent{project_id:project.into(),room_id:"!room:example.test".into(),display_name:"研究助理".into(),idempotency_key:"create-1".into()}}
fn active(s:&mut Store,a:&Actor,p:&str,f:&mut RoomFacts)->(Agent,Binding) {
    let (agent,b)=s.create_agent(a,&request(p),f,NOW).unwrap();
    f.puppet_in_room=true;f.puppet_mxid=Some(agent.puppet_mxid.clone());
    let b=s.activate_binding(a,&b.id,f,NOW).unwrap();
    (s.agent(a,&agent.id).unwrap(),b)
}

#[test]
fn ownership_is_immutable_even_via_sql_and_retired_ids_are_reserved() {
    let (_d,mut s,a,b,p,mut f)=fixture();
    let (agent,_)=active(&mut s,&a,&p,&mut f);
    assert!(s.db.execute("UPDATE agents SET owner_user_id=?1 WHERE id=?2",params![b.id,agent.id]).is_err());
    assert!(s.db.execute("UPDATE agents SET puppet_mxid='@other:example.test' WHERE id=?1",[&agent.id]).is_err());
    assert!(s.db.execute("DELETE FROM agents WHERE id=?1",[&agent.id]).is_err());
    s.retire_agent(&a,&agent.id,NOW).unwrap();
    assert_eq!(s.agent(&a,&agent.id).unwrap().owner_user_id,a.id);
    assert!(matches!(s.agent(&b,&agent.id),Err(Error::NotFound)));
    assert!(matches!(s.retire_agent(&b,&agent.id,NOW),Err(Error::NotFound)));
}
#[test]
fn creation_ban_does_not_revoke_existing_service() {
    let (_d,mut s,a,_b,p,mut f)=fixture();
    let (_agent,b)=active(&mut s,&a,&p,&mut f);
    let deny=CreationPolicy{default_allow:true,allow:BTreeSet::from([a.mxid.clone()]),deny:BTreeSet::from([a.mxid.clone()])};
    s.set_project_policy(&a,&p,1,&deny,&admin(&a,"!space:example.test",true,None),NOW).unwrap();
    assert!(s.runnable(&a,&b.id,&f,NOW).is_ok());
    let mut req=request(&p);req.idempotency_key="create-2".into();
    assert!(matches!(s.create_agent(&a,&req,&f,NOW),Err(Error::Denied("project_create_denied"))));
    s.set_room_policy(&a,"!room:example.test",1,&RoomCreationPolicy::Disabled,&admin(&a,"!room:example.test",false,Some("!space:example.test")),NOW).unwrap();
    assert!(s.runnable(&a,&b.id,&f,NOW).is_ok());
}
#[test]
fn same_agent_crosses_projects_and_revocation_is_local() {
    let (_d,mut s,a,_b,p,mut f)=fixture();
    let (agent,b1)=active(&mut s,&a,&p,&mut f);
    let p2=s.register_project(&a,"!space2:example.test",&admin(&a,"!space2:example.test",true,None),NOW).unwrap();
    s.register_room(&a,&p2,"!room2:example.test",&admin(&a,"!space2:example.test",true,None),&admin(&a,"!room2:example.test",false,Some("!space2:example.test")),NOW).unwrap();
    let mut f2=facts(&a,"!space2:example.test","!room2:example.test");
    let b2=s.bind_room(&a,&agent.id,&p2,&f2.room_id,&f2,NOW).unwrap();
    f2.puppet_in_room=true;f2.puppet_mxid=Some(agent.puppet_mxid.clone());
    s.activate_binding(&a,&b2.id,&f2,NOW).unwrap();
    s.suspend_project_bindings(&a,&p,&admin(&a,"!space:example.test",true,None),NOW).unwrap();
    assert!(matches!(s.runnable(&a,&b1.id,&f,NOW),Err(Error::Denied("binding_revoked"))));
    assert!(s.runnable(&a,&b2.id,&f2,NOW).is_ok());
    assert_eq!(s.agent(&a,&agent.id).unwrap().puppet_mxid,agent.puppet_mxid);
}
#[test]
fn idempotency_survives_reopen_and_refuses_changed_content() {
    let (d,mut s,a,_b,p,f)=fixture();
    let expected=s.create_agent(&a,&request(&p),&f,NOW).unwrap();
    drop(s);
    let mut s=Store::open(&d.path().join("server.sqlite"),"example.test","_hagency_test_").unwrap();
    let a=s.authenticated_human(a.mxid()).unwrap();
    assert_eq!(s.create_agent(&a,&request(&p),&f,NOW).unwrap(),expected);
    let mut req=request(&p);req.display_name="different".into();
    assert!(matches!(s.create_agent(&a,&req,&f,NOW),Err(Error::Conflict)));
}
#[test]
fn refuses_old_database_without_changing_tables_or_identity() {
    let d=tempfile::tempdir().unwrap();let path=d.path().join("legacy.sqlite");
    let db=Connection::open(&path).unwrap();db.execute_batch("CREATE TABLE engagements(id TEXT); INSERT INTO engagements VALUES('legacy');").unwrap();drop(db);
    assert!(matches!(Store::open(&path,"example.test","_hagency_test_"),Err(Error::ForeignDatabase)));
    let db=Connection::open(&path).unwrap();
    assert_eq!(db.query_row("SELECT id FROM engagements",[],|r|r.get::<_,String>(0)).unwrap(),"legacy");
    assert_eq!(db.pragma_query_value(None,"application_id",|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='agents'",[],|r|r.get::<_,u64>(0)).unwrap(),0);
}
#[test]
fn invalid_authority_never_creates_a_puppet() {
    let (_d,mut s,a,b,p,f)=fixture();
    for mutate in 0..5 {
        let mut f=f.clone();match mutate {0=>f.owner_in_space=false,1=>f.owner_in_room=false,2=>f.observed_at=0,3=>f.owner_mxid=b.mxid.clone(),_=>f.service_can_invite=false};
        assert!(s.create_agent(&a,&request(&p),&f,NOW).is_err());
    }
    let mut encrypted=f.clone();encrypted.encrypted=true;
    assert!(matches!(s.create_agent(&a,&request(&p),&encrypted,NOW),Err(Error::Denied("encryption_not_supported"))));
    assert_eq!(s.db.query_row("SELECT COUNT(*) FROM agents",[],|r|r.get::<_,u64>(0)).unwrap(),0);
}
#[test]
fn room_cannot_bypass_project_policy_and_policy_edits_require_current_admin() {
    let (_d,mut s,a,b,p,f)=fixture();
    let closed=CreationPolicy{default_allow:false,allow:BTreeSet::new(),deny:BTreeSet::new()};
    let af=admin(&a,"!space:example.test",true,None);
    s.set_project_policy(&a,&p,1,&closed,&af,NOW).unwrap();
    assert!(matches!(s.create_agent(&a,&request(&p),&f,NOW),Err(Error::Denied("project_create_denied"))));
    assert!(matches!(s.set_project_policy(&a,&p,1,&CreationPolicy::default(),&af,NOW),Err(Error::StaleRevision)));
    assert!(s.set_project_policy(&b,&p,2,&CreationPolicy::default(),&af,NOW).is_err());
    let mut stale=af;stale.observed_at=0;
    assert!(s.set_project_policy(&a,&p,2,&CreationPolicy::default(),&stale,NOW).is_err());
}
#[test]
fn puppet_and_foreign_users_cannot_enroll_as_humans() {
    let (_d,mut s,_a,_b,_p,_f)=fixture();
    assert!(s.authenticated_human("@_hagency_test_agent:example.test").is_err());
    assert!(s.authenticated_human("@foo:evil:example.test").is_err());
    assert!(s.authenticated_human("@foo:other.test").is_err());
}
#[test]
fn payload_cannot_choose_owner_or_include_execution_secrets() {
    assert!(serde_json::from_value::<CreateAgent>(serde_json::json!({"projectId":"p","roomId":"!r:example.test","displayName":"x","idempotencyKey":"k","ownerMxid":"@bob:example.test"})).is_err());
    assert!(serde_json::from_value::<CreateAgent>(serde_json::json!({"projectId":"p","roomId":"!r:example.test","displayName":"x","idempotencyKey":"k","modelKey":"secret"})).is_err());
}
#[test]
fn database_is_bound_to_homeserver_and_namespace() {
    let (d,s,_a,_b,_p,_f)=fixture();drop(s);
    assert!(matches!(Store::open(&d.path().join("server.sqlite"),"other.test","_hagency_test_"),Err(Error::ForeignDatabase)));
    assert!(matches!(Store::open(&d.path().join("server.sqlite"),"example.test","_hagency_other_"),Err(Error::ForeignDatabase)));
}
