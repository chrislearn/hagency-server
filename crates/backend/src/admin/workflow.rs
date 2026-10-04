use super::*;
use super::{api::Admin, fleet::public_fleet, outbound::*};
use reqwest::Method;
use unicode_normalization::UnicodeNormalization;
impl Admin {
    pub(crate) async fn rep(
        &self,
        f: &Value,
        path: &str,
        method: Method,
        body: Option<&Value>,
    ) -> Result<Value> {
        let path = format!(
            "{path}{}user_id={}",
            if path.contains('?') { '&' } else { '?' },
            enc(s(&f["representativeMxid"]))
        );
        self.palpo
            .call(&path, s(&f["registration"]["as_token"]), method, body)
            .await
    }
    async fn provider(
        &self,
        f: &Value,
        path: &str,
        method: Method,
        body: Option<&Value>,
    ) -> Result<Value> {
        if outbound(f) {
            return Err(err(
                409,
                "outbound_callback_forbidden",
                "Outbound fleets publish snapshots and poll delivery.",
            ));
        }
        let url = url::Url::parse(s(&f["callbackUrl"]))
            .map_err(|_| err(500, "invalid_callback", "Invalid saved callback."))?;
        super::upstream::Upstream::with_label(url, "Hagency callback")
            .call(
                &format!("/api/fleet/v1{path}"),
                s(&f["registration"]["hs_token"]),
                method,
                body,
            )
            .await
    }
    pub(crate) fn apply_capabilities(&self, f: &mut Value, data: &Value) -> Result<()> {
        if !data.is_object() {
            return Err(err(
                400,
                "invalid_capabilities",
                "Capabilities must be an object.",
            ));
        }
        if data["v"] != 1
            || data["fleetId"] != f["id"]
            || data["serverName"] != self.server_name
            || data["representativeMxid"] != f["representativeMxid"]
        {
            return Err(err(
                409,
                "provider_identity_mismatch",
                "Provider does not identify this fleet and server.",
            ));
        }
        let offers = data["offers"].as_array().ok_or_else(|| {
            err(
                409,
                "provider_capability_missing",
                "Missing roles and approval identity.",
            )
        })?;
        let bot = s(&data["approvalBotMxid"]);
        if !regex::Regex::new(r"^@[^\s:]+:.+$").unwrap().is_match(bot) {
            return Err(err(
                409,
                "provider_capability_missing",
                "Missing approval identity.",
            ));
        }
        let mut filtered = Vec::new();
        let resource_id = regex::Regex::new(r"^resource_[a-f0-9]{24}$").unwrap();
        for offer in offers {
            if !offer.is_object() {
                return Err(err(409, "provider_capability_missing", "Invalid role."));
            }
            if offer["published"] == false {
                continue;
            }
            let role = field(&offer["role"], "Role", 80)?;
            let mut out = json!({"role":role});
            if let Some(d) = offer["description"].as_str() {
                out["description"] = json!(d.chars().take(500).collect::<String>());
            }
            if let Some(resources) = offer["resources"].as_array() {
                let mut list = Vec::new();
                for r in resources.iter().take(200) {
                    if !r.is_object() {
                        return Err(err(409, "provider_capability_missing", "Invalid resource."));
                    }
                    if !resource_id.is_match(s(&r["id"])) {
                        continue;
                    }
                    list.push(json!({
"id":r["id"],"name":field(&r["name"],"Resource name",128)?,"framework":field(&r["framework"],"Framework",32)?,"model":field(&r["model"],"Model",256)?,"reasoning":r["reasoning"].as_str().map(|s|s.chars().take(64).collect::<String>())}
));
                }
                out["resources"] = json!(list);
            }
            filtered.push(out);
        }
        let t = now();
        f["capabilities"] = json!({"v":1,"fleetId":data["fleetId"],"serverName":data["serverName"],"representativeMxid":data["representativeMxid"],"approvalBotMxid":bot,"offers":filtered,"observedAt":t});
        f["capabilityRead"] = json!({"state":"current","observedAt":t});
        Ok(())
    }
    async fn capabilities(&self, f: &mut Value) -> Result<Value> {
        if outbound(f) {
            if !f["capabilities"].is_object() {
                return Err(err(
                    409,
                    "capabilities_pending",
                    "Waiting for Hagency to publish its catalog.",
                ));
            }
            return Ok(f["capabilities"].clone());
        }
        let data = self.provider(f, "/capabilities", Method::GET, None).await?;
        self.apply_capabilities(f, &data)?;
        let id = s(&f["id"]).to_string();
        self.store.save_fleet(&id, f.clone()).await?;
        Ok(f["capabilities"].clone())
    }
    pub(crate) async fn catalog(&self, actor: &str) -> Result<Value> {
        let before = self.store.snapshot().await;
        let mut work = tokio::task::JoinSet::new();
        let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(3));
        for f in object_values(&before["fleets"]).into_iter().filter(|f| {
            !outbound(f)
                && f["installation"] == "installed"
                && !["paused", "revoked"].contains(&s(&f["state"]))
        }) {
            let admin = self.clone();
            let gate = gate.clone();
            work.spawn(async move {
                let _permit = gate.acquire().await.unwrap();
                let id = s(&f["id"]).to_string();
                let data = admin.provider(&f, "/capabilities", Method::GET, None).await;
                let mut copy = f.clone();
                let result = data.and_then(|v| admin.apply_capabilities(&mut copy, &v));
                admin
                    .store
                    .change(|st| {
                        if st["fleets"][&id] == f {
                            if let Err(e) = result {
                                copy["capabilityRead"] = json!(
                                    { "state" : "failed", "code" : e.code, "failedAt" : now(),
                                    "lastSuccessAt" : f["capabilities"] ["observedAt"] }
                                );
                            }
                            st["fleets"][&id] = copy;
                        }
                        Ok(())
                    })
                    .await
            });
        }
        while let Some(r) = work.join_next().await {
            r.map_err(|_| err(500, "internal_error", "Catalog refresh failed."))??;
        }
        let mut out = Vec::new();
        for f in object_values(&self.store.snapshot().await["fleets"])
            .into_iter()
            .filter(|f| {
                f["installation"] == "installed" && !["paused", "revoked"].contains(&s(&f["state"]))
            })
        {
            let mut v = public_fleet(&f);
            v["owned"] = json!(f["ownerMxid"] == actor);
            out.push(v);
        }
        Ok(json!(out))
    }
    pub(crate) async fn room_state(
        &self,
        id: &str,
        token: &str,
        as_user: Option<&str>,
    ) -> Result<Value> {
        let path = format!(
            "/_matrix/client/v3/rooms/{}/state{}",
            enc(id),
            as_user
                .map(|s| format!("?user_id={}", enc(s)))
                .unwrap_or_default()
        );
        let data = self.palpo.get(&path, token).await?;
        if !data.is_array() {
            return Err(err(
                502,
                "invalid_room_state",
                "Matrix room state could not be verified.",
            ));
        }
        Ok(data)
    }
    pub(crate) fn state_content(&self, state: &Value, typ: &str, key: &str) -> Value {
        state
            .as_array()
            .into_iter()
            .flatten()
            .find(|e| e["type"] == typ && e["state_key"] == key)
            .map(|e| e["content"].clone())
            .unwrap_or(Value::Null)
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the immutable Matrix room plan.
    async fn ensure_room(
        &self,
        plan: &mut Value,
        name: &str,
        actor: &str,
        token: &str,
        as_user: Option<&str>,
        invite: Vec<String>,
        encrypted: bool,
        binding: &Value,
    ) -> Result<Value> {
        let query = as_user
            .map(|s| format!("?user_id={}", enc(s)))
            .unwrap_or_default();
        if !plan["roomId"].is_string() {
            let alias = format!("#{}:{}", s(&plan["aliasLocalpart"]), self.server_name);
            match self
                .palpo
                .get(
                    &format!("/_matrix/client/v3/directory/room/{}", enc(&alias)),
                    token,
                )
                .await
            {
                Ok(r) => plan["roomId"] = r["room_id"].clone(),
                Err(e) if e.status == 404 => (),
                Err(e) => return Err(e),
            }
            if !plan["roomId"].is_string() {
                let rooms = self
                    .palpo
                    .get(&format!("/_matrix/client/v3/joined_rooms{query}"), token)
                    .await?;
                let rooms = rooms["joined_rooms"]
                    .as_array()
                    .filter(|a| a.len() <= 1000)
                    .ok_or_else(|| {
                        err(
                            409,
                            "room_recovery_required",
                            "Joined room inventory cannot be safely reconciled.",
                        )
                    })?;
                let mut matches = Vec::new();
                for room in rooms {
                    let st = self.room_state(s(room), token, as_user).await?;
                    if canonical(&self.state_content(&st, "com.hagency.admin.binding.v1", ""))
                        == canonical(binding)
                    {
                        matches.push(room.clone());
                    }
                }
                if matches.len() > 1 {
                    return Err(err(
                        409,
                        "room_binding_conflict",
                        "Multiple rooms claim this operation.",
                    ));
                }
                if let Some(id) = matches.pop() {
                    plan["roomId"] = id;
                }
            }
            if !plan["roomId"].is_string() {
                let mut initial = vec![
                    json!({"type":"com.hagency.admin.binding.v1","state_key":"","content":binding}),
                ];
                if encrypted {
                    initial.push(json!({"type":"m.room.encryption","state_key":"","content":{"algorithm":"m.megolm.v1.aes-sha2"}}));
                }
                let r=self.palpo.post(&format!("/_matrix/client/v3/createRoom{query}"),token,&json!({
"name":name,"room_alias_name":plan["aliasLocalpart"],"visibility":"private","preset":"private_chat","room_version":"11","invite":invite,"initial_state":initial}
)).await?;

                if !r["room_id"].is_string() {
                    return Err(err(
                        502,
                        "invalid_room_identity",
                        "Matrix did not return a room ID.",
                    ));
                }
                plan["roomId"] = r["room_id"].clone();
            }
        }
        let state = self.room_state(s(&plan["roomId"]), token, as_user).await?;
        let create = self.state_content(&state, "m.room.create", "");
        let creator = create["creator"]
            .as_str()
            .or_else(|| {
                state
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|e| e["type"] == "m.room.create")
                    .and_then(|e| e["sender"].as_str())
            })
            .unwrap_or("");
        if creator != actor
            || canonical(&self.state_content(&state, "com.hagency.admin.binding.v1", ""))
                != canonical(binding)
        {
            return Err(err(
                409,
                "room_binding_conflict",
                "Room creator or operation binding differs.",
            ));
        }
        if self.state_content(&state, "m.room.join_rules", "")["join_rule"] != "invite" {
            return Err(err(
                409,
                "room_privacy_required",
                "Managed room must be invite-only.",
            ));
        }
        let crypt = self.state_content(&state, "m.room.encryption", "");
        if if encrypted {
            crypt["algorithm"] != "m.megolm.v1.aes-sha2"
        } else {
            !crypt.is_null()
        } {
            return Err(err(
                409,
                "room_encryption_mismatch",
                "Managed room encryption differs.",
            ));
        }
        Ok(state)
    }
    pub(crate) async fn connect(&self, id: &str, actor: &str, token: &str) -> Result<Value> {
        let mut f = self.fleet_owned(id, actor).await?;
        let run: Result<()>=async{
        self.capabilities(&mut f).await?;
if !f["reception"].is_object(){
f["reception"]=json!({
"aliasLocalpart":format!("{id}_reception"),"createdAt":now()}
);
}
self.store.save_fleet(id,f.clone()).await?;
let mut plan=f["reception"].clone();
self.ensure_room(&mut plan,&format!("{} · Reception",s(&f["name"])),s(&f["representativeMxid"]),s(&f["registration"]["as_token"]),Some(s(&f["representativeMxid"])),vec![actor.into()],false,&json!({
"v":1,"fleetId":id,"purpose":"reception"}
)).await?;
f["reception"]=plan;
self.store.save_fleet(id,f.clone()).await?;
let room=s(&f["reception"]["roomId"]).to_string();
self.palpo.post(&format!("/_matrix/client/v3/join/{}",enc(&room)),token,&json!({
}
)).await?;
let st=self.room_state(&room,token,None).await?;
if self.state_content(&st,"m.room.member",actor)["membership"]!="join"||self.state_content(&st,"m.room.member",s(&f["representativeMxid"]))["membership"]!="join"{
return Err(err(409,"reception_membership_pending","Owner and representative must both join reception."));
}
f["reception"]["verifiedAt"]=json!(now());

        if !f["probe"].is_object()||(!outbound(&f)&&f["probe"]["completedAt"].is_string()){f["probe"]=json!({"challenge":uuid::Uuid::new_v4().to_string(),"roomId":room,"startedAt":now()});}
if f["probe"]["roomId"]!=room{
return Err(err(409,"probe_binding_conflict","Probe belongs to another room."));
}
self.store.save_fleet(id,f.clone()).await?;
if !f["probe"]["eventId"].is_string(){
let r=self.rep(&f,&format!("/_matrix/client/v3/rooms/{}/send/com.hagency.connection.probe.v1/{}",enc(&room),enc(&format!("probe_{}",s(&f["probe"]["challenge"])))),Method::PUT,Some(&json!({
"v":1,"fleetId":id,"challenge":f["probe"]["challenge"]}
))).await?;
if !r["event_id"].is_string(){
return Err(err(502,"probe_event_missing","Matrix did not acknowledge the connectivity event."));
}
f["probe"]["eventId"]=r["event_id"].clone();
self.store.save_fleet(id,f.clone()).await?;
}

        let payload=json!({
"fleetId":id,"sourceRoomId":room,"sourceEventId":f["probe"]["eventId"],"challenge":f["probe"]["challenge"]}
);
if outbound(&f){
self.enqueue(&f,"work","probe",&format!("probe_{}",s(&f["probe"]["challenge"])),&payload).await?;
if proven(&f)&&f["probe"]["completedAt"].is_string(){
f["state"]=json!("ready");
f["lastError"]=Value::Null;
}
return Ok(());
}

        let mut receipt=Value::Null;
for attempt in 0..20{
self.active_fleet(id).await?;
match self.provider(&f,"/probe",Method::POST,Some(&payload)).await {
Ok(r)=>{
receipt=r;
break}
,Err(e) if e.code=="probe_pending"&&attempt<19=>tokio::time::sleep(std::time::Duration::from_millis(500)).await,Err(e)=>return Err(e)}
}

if receipt["received"]!=true||["fleetId","sourceRoomId","sourceEventId","challenge"].iter().any(|k|receipt[k]!=payload[k]){
return Err(err(409,"event_delivery_unverified","Provider did not confirm the exact event receipt."));
}
f["probe"]["completedAt"]=json!(now());
f["connection"]=json!({
"verifiedAt":now(),"expiresAt":(chrono::Utc::now()+chrono::Duration::minutes(30)).to_rfc3339_opts(chrono::SecondsFormat::Millis,true),"sourceRoomId":room,"sourceEventId":f["probe"]["eventId"],"challenge":f["probe"]["challenge"]}
);
f["state"]=json!("ready");
f["lastError"]=Value::Null;
Ok(())}
.await;

        if let Err(e) = run {
            if ["ready", "pending_connection"].contains(&s(&f["state"])) {
                f["state"] = json!("pending_connection");
            }
            f["lastError"] = json!({"code":e.code,"at":now()});
            self.store.save_fleet(id, f).await?;
            return Err(e);
        }
        self.store.save_fleet(id, f.clone()).await?;
        Ok(public_fleet(&f))
    }
    async fn validate_project_room(
        &self,
        id: &str,
        actor: &str,
        token: &str,
        owner: &str,
        authority: bool,
    ) -> Result<Value> {
        let state = self.room_state(id, token, None).await?;
        if !self
            .state_content(&state, "m.room.encryption", "")
            .is_null()
        {
            return Err(err(
                409,
                "project_encrypted",
                "Project channel requires an unencrypted room.",
            ));
        }
        if self.state_content(&state, "m.room.join_rules", "")["join_rule"] != "invite" {
            return Err(err(
                409,
                "project_privacy_required",
                "Project room must be invite-only.",
            ));
        }
        if self.state_content(&state, "m.room.member", actor)["membership"] != "join"
            || self.state_content(&state, "m.room.member", owner)["membership"] != "join"
        {
            return Err(err(
                403,
                "project_membership_required",
                "Requester and owner must be joined.",
            ));
        }
        let power = self.state_content(&state, "m.room.power_levels", "");
        let level = |u: &str| {
            power["users"][u]
                .as_i64()
                .unwrap_or(power["users_default"].as_i64().unwrap_or(0))
        };
        let invite = power["invite"].as_i64().unwrap_or(0);
        if level(actor) < invite {
            return Err(err(
                403,
                "project_invite_required",
                "Requester must have invite authority.",
            ));
        }
        if authority
            && level(owner)
                < 100
                    .max(invite)
                    .max(power["state_default"].as_i64().unwrap_or(50))
        {
            return Err(err(
                403,
                "project_owner_authority_required",
                "Owner requires power level 100.",
            ));
        }
        Ok(state)
    }
    async fn validate_dm(&self, p: &Value, token: &str) -> Result<()> {
        let st = self.room_state(s(&p["ownerDmRoomId"]), token, None).await?;
        if self.state_content(&st, "m.room.encryption", "")["algorithm"] != "m.megolm.v1.aes-sha2"
            || self.state_content(&st, "m.room.join_rules", "")["join_rule"] != "invite"
        {
            return Err(err(
                409,
                "owner_dm_privacy_required",
                "Owner approval requires an encrypted invite-only room.",
            ));
        }
        let allowed = [s(&p["ownerMxid"]), s(&p["approvalBotMxid"])];
        let mut joined = Vec::new();
        for e in st.as_array().unwrap() {
            if e["type"] == "m.room.member" {
                let member = s(&e["state_key"]);
                let membership = s(&e["content"]["membership"]);
                if ["join", "invite"].contains(&membership) && !allowed.contains(&member) {
                    return Err(err(
                        409,
                        "owner_dm_not_private",
                        "Approval room has an unrelated member.",
                    ));
                }
                if membership == "join" {
                    joined.push(member);
                }
            }
        }
        if joined.len() != 2 || allowed.iter().any(|u| !joined.contains(u)) {
            return Err(err(
                409,
                "owner_dm_join_pending",
                "Waiting for approval identity to join.",
            ));
        }
        Ok(())
    }
    pub(crate) async fn create_project(
        &self,
        input: &Value,
        actor: &str,
        token: &str,
    ) -> Result<Value> {
        let mut f = self
            .active_fleet(&field(&input["fleetId"], "Fleet ID", 128)?)
            .await?;
        let request = key(&input["requestId"], "Project operation ID")?;
        let name = field(&input["name"], "Project name", 128)?;
        let existing = if input["roomId"]
            .as_str()
            .is_some_and(|r| !r.trim().is_empty())
        {
            Some(field(&input["roomId"], "Room ID", 255)?)
        } else {
            None
        };
        let id = format!(
            "project_{}",
            &digest(&json!({"actor":actor,"requestId":request}))[..24]
        );
        let fp =
            digest(&json!({"actor":actor,"fleetId":f["id"],"name":name,"existingRoom":existing}));
        let mut p = self.store.snapshot().await["projects"][&id].clone();
        if p.is_object() && p["fingerprint"] != fp {
            return Err(err(
                409,
                "idempotency_conflict",
                "Project operation has different content.",
            ));
        }
        let cap = self.capabilities(&mut f).await?;
        if !p.is_object() {
            p = json!({
            "id":id,"requestId":request,"fingerprint":fp,"fleetId":f["id"],"name":name,"ownerMxid":actor,"approvalBotMxid":cap["approvalBotMxid"],"authVersion":1,"createdAt":now(),"roomId":existing,"state":"creating","room":{
            "aliasLocalpart":format!("hf_{id}")}
            ,"ownerDm":{
            "aliasLocalpart":format!("hf_{id}_approvals")}
            }
            );

            self.save_project(&p).await?;
        }
        let run: Result<()>=async{
if !p["roomId"].is_string(){
let mut plan=p["room"].clone();
self.ensure_room(&mut plan,&name,actor,token,None,vec![s(&f["representativeMxid"]).into()],false,&json!({
"v":1,"projectId":id,"purpose":"project","ownerMxid":actor}
)).await?;
p["roomId"]=plan["roomId"].clone();
p["room"]=plan;
self.save_project(&p).await?;
}
let room=s(&p["roomId"]).to_string();
self.validate_project_room(&room,actor,token,actor,true).await?;
self.palpo.put(&format!("/_matrix/client/v3/rooms/{}/state/com.hagency.admin.binding.v1/{}",enc(&room),enc(s(&f["id"]))),token,&json!({
"v":1,"fleetId":f["id"],"purpose":"project","projectId":id,"ownerMxid":actor,"authVersion":p["authVersion"]}
)).await?;
let st=self.room_state(&room,token,None).await?;
if !["join","invite"].contains(&s(&self.state_content(&st,"m.room.member",s(&f["representativeMxid"]))["membership"])){
self.palpo.post(&format!("/_matrix/client/v3/rooms/{}/invite",enc(&room)),token,&json!({
"user_id":f["representativeMxid"]}
)).await?;
}
self.rep(&f,&format!("/_matrix/client/v3/join/{}",enc(&room)),Method::POST,Some(&json!({
}
))).await?;
let mut plan=p["ownerDm"].clone();
self.ensure_room(&mut plan,&format!("{name} · Private approvals"),actor,token,None,vec![s(&p["approvalBotMxid"]).into()],true,&json!({
"v":1,"projectId":id,"purpose":"owner_approval","ownerMxid":actor,"approvalBotMxid":p["approvalBotMxid"]}
)).await?;
p["ownerDmRoomId"]=plan["roomId"].clone();
p["ownerDm"]=plan;
p["state"]=json!("registered");
p["lastError"]=Value::Null;
Ok(())}
.await;

        if let Err(e) = run {
            p["state"] = json!("partial");
            p["lastError"] = json!({"code":e.code,"at":now()});
            self.save_project(&p).await?;
            return Err(e);
        }
        self.save_project(&p).await?;
        self.audit(actor, "project.register", s(&f["id"]), &id, "registered")
            .await?;
        Ok(self.project_view(&p, actor, token).await)
    }
    async fn save_project(&self, p: &Value) -> Result<()> {
        self.store
            .change(|st| {
                st["projects"][s(&p["id"])] = p.clone();
                Ok(())
            })
            .await
    }
    async fn project_view(&self, p: &Value, actor: &str, token: &str) -> Value {
        let mut out = p.clone();
        for k in ["fingerprint", "room", "ownerDm"] {
            out.as_object_mut().unwrap().remove(k);
        }
        if p["ownerMxid"] != actor {
            out.as_object_mut().unwrap().remove("ownerDmRoomId");
        }
        let run: Result<()> = async {
            self.validate_project_room(s(&p["roomId"]), actor, token, s(&p["ownerMxid"]), true)
                .await?;
            out["canRequest"] = json!(true);
            if p["ownerMxid"] == actor {
                self.validate_dm(p, token).await?;
                out["ownerApproval"] = json!("ready");
            } else {
                out["ownerApproval"] = json!("verified_by_provider_on_submission");
            }
            Ok(())
        }
        .await;
        if let Err(e) = run {
            out["canRequest"] = json!(false);
            out["readinessError"] = json!(e.code);
            out["ownerApproval"] = json!("pending");
        }
        out
    }
    pub(crate) async fn projects(&self, actor: &str, token: &str) -> Result<Value> {
        let mut out = Vec::new();
        for p in object_values(&self.store.snapshot().await["projects"]) {
            if p["ownerMxid"] == actor
                || self
                    .validate_project_room(s(&p["roomId"]), actor, token, s(&p["ownerMxid"]), false)
                    .await
                    .is_ok()
            {
                out.push(self.project_view(&p, actor, token).await);
            }
        }
        Ok(json!(out))
    }
    async fn save_request(&self, r: &Value) -> Result<()> {
        self.store
            .change(|st| {
                st["requests"][s(&r["id"])] = r.clone();
                Ok(())
            })
            .await
    }
    pub(crate) async fn request(&self, input: &Value, actor: &str, token: &str) -> Result<Value> {
        let request = key(&input["requestId"], "Request ID")?;
        let st = self.store.snapshot().await;
        let p = st["projects"]
            .get(&field(&input["projectId"], "Project ID", 128)?)
            .cloned()
            .ok_or_else(|| err(404, "not_found", "Project not found."))?;
        let mut f = self.active_fleet(s(&p["fleetId"])).await?;
        if !if outbound(&f) {
            proven(&f)
        } else {
            public_fleet(&f)["readiness"]["ready"] == true
        } {
            return Err(err(
                409,
                "fleet_not_ready",
                "Fleet owner must verify connection first.",
            ));
        }
        self.validate_project_room(s(&p["roomId"]), actor, token, s(&p["ownerMxid"]), true)
            .await?;
        if p["ownerMxid"] == actor {
            self.validate_dm(&p, token).await?;
        }
        let cap = self.capabilities(&mut f).await?;
        let role = field(&input["role"], "Role", 80)?;
        let quota = |v: &Value| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
                .ok_or_else(|| {
                    err(
                        400,
                        "invalid_quota",
                        "Quota and daily rate must be positive safe integers.",
                    )
                })
        };
        let tokens = quota(&input["requestedTokens"])?;
        let rate = quota(&input["ratePerDay"])?;
        let mut definition = Value::Null;
        if !input["agentDefinition"].is_null()
            || !input["agentName"].is_null()
            || !input["resourceId"].is_null()
        {
            let d = if input["agentDefinition"].is_null() {
                json!({"name":input["agentName"],"resourceId":input["resourceId"]})
            } else {
                input["agentDefinition"].clone()
            };
            let name = s(&d["name"]).nfc().collect::<String>();
            if !d.is_object()
                || name.chars().count() > 64
                || !regex::Regex::new(r"^\p{L}[\p{L}\p{M}\p{N}_-]*$")
                    .unwrap()
                    .is_match(&name)
                || !regex::Regex::new(r"^resource_[a-f0-9]{24}$")
                    .unwrap()
                    .is_match(s(&d["resourceId"]))
                || d.as_object()
                    .unwrap()
                    .keys()
                    .any(|k| !["name", "resourceId"].contains(&k.as_str()))
            {
                return Err(err(
                    400,
                    "invalid_agent_definition",
                    "Use a valid Agent name and published resource.",
                ));
            }
            definition = json!({"name":name,"resourceId":d["resourceId"]});
        }
        let mut payload = json!({
        "v":1,"fleetId":f["id"],"requestId":request,"requesterMxid":actor,"sourceRoomId":f["reception"]["roomId"],"targetProjectId":p["id"],"targetRoomId":p["roomId"],"ownerMxid":p["ownerMxid"],"ownerDmRoomId":p["ownerDmRoomId"],"role":role,"requestedTokens":tokens,"ratePerDay":rate,"authVersion":p["authVersion"]}
        );

        if definition.is_object() {
            payload["agentDefinition"] = definition.clone();
        }
        let id = format!("{}:{request}", s(&f["id"]));
        let fp = digest(&payload);
        let mut r = st["requests"][&id].clone();
        if r.is_object() && r["fingerprint"] != fp {
            return Err(err(
                409,
                "idempotency_conflict",
                "Request ID has different content.",
            ));
        }
        if !r.is_object() {
            let offer = cap["offers"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["role"] == role)
                .ok_or_else(|| err(409, "role_unavailable", "Role is not currently published."))?;
            let resource = offer["resources"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|r| r["id"] == definition["resourceId"]);
            if definition.is_object() && resource.is_none() {
                return Err(err(
                    409,
                    "resource_unavailable",
                    "Resource is not published for this role.",
                ));
            }
            if definition.is_object()
                && object_values(&st["requests"]).iter().any(|o| {
                    o["projectId"] == p["id"]
                        && !["ended", "rejected"].contains(&s(&o["state"]))
                        && o["payload"]["agentDefinition"]["name"] == definition["name"]
                })
            {
                return Err(err(
                    409,
                    "agent_name_conflict",
                    "Project already has an Agent request with that name.",
                ));
            }
            r = json!({"id":id,"requestId":request,"fleetId":f["id"],"projectId":p["id"],"requesterMxid":actor,"payload":payload,"fingerprint":fp,"state":"sending","createdAt":now()});
            if let Some(res) = resource {
                r["resource"] = res.clone();
            }
            self.save_request(&r).await?;
        }
        let run: Result<()> = async {
            let room = s(&payload["sourceRoomId"]);
            let st = self
                .room_state(
                    room,
                    s(&f["registration"]["as_token"]),
                    Some(s(&f["representativeMxid"])),
                )
                .await?;
            if !["join", "invite"].contains(&s(
                &self.state_content(&st, "m.room.member", actor)["membership"],
            )) {
                self.rep(
                    &f,
                    &format!("/_matrix/client/v3/rooms/{}/invite", enc(room)),
                    Method::POST,
                    Some(&json!({"user_id":actor})),
                )
                .await?;
            }
            self.palpo
                .post(
                    &format!("/_matrix/client/v3/join/{}", enc(room)),
                    token,
                    &json!({}),
                )
                .await?;
            if !r["sourceEventId"].is_string() {
                let mut event = payload.clone();
                event.as_object_mut().unwrap().remove("ownerDmRoomId");
                let txn = format!(
                    "request_{}",
                    digest(&json!({"fleetId":f["id"],"actor":actor,"requestId":request}))
                );
                let sent = self
                    .palpo
                    .put(
                        &format!(
                            "/_matrix/client/v3/rooms/{}/send/com.hagency.engagement.request.v1/{}",
                            enc(room),
                            enc(&txn)
                        ),
                        token,
                        &event,
                    )
                    .await?;
                if !sent["event_id"].is_string() {
                    return Err(err(
                        502,
                        "request_event_missing",
                        "Matrix did not acknowledge request event.",
                    ));
                }
                r["sourceEventId"] = sent["event_id"].clone();
                self.save_request(&r).await?;
            }
            let mut body = payload.clone();
            body["sourceEventId"] = r["sourceEventId"].clone();
            if outbound(&f) {
                self.enqueue(&f, "work", "request", &request, &body).await?;
                r["usable"] = json!(false);
                r["statusVerified"] = json!(false);
                if !r["provider"].is_object() {
                    r["state"] = json!("queued");
                }
            } else {
                let result = self
                    .provider(&f, "/requests", Method::POST, Some(&body))
                    .await?;
                self.apply_status(&mut r, &result)?;
            }
            r["lastError"] = Value::Null;
            Ok(())
        }
        .await;
        if let Err(e) = run {
            r["lastError"] = json!({"code":e.code,"at":now()});
            r["state"] = json!("submission_pending");
            self.save_request(&r).await?;
            return Err(e);
        }
        self.save_request(&r).await?;
        self.audit(
            actor,
            "request.submit",
            s(&f["id"]),
            &request,
            s(&r["state"]),
        )
        .await?;
        Ok(self.request_view(&r, actor))
    }
    pub(crate) fn apply_status(&self, r: &mut Value, result: &Value) -> Result<()> {
        if result["requestId"] != r["requestId"]
            || result["fleetId"] != r["fleetId"]
            || ["targetProjectId", "targetRoomId", "sourceRoomId"]
                .iter()
                .any(|k| result[k] != r["payload"][k])
            || result["sourceEventId"] != r["sourceEventId"]
            || (!r["payload"]["agentDefinition"].is_null()
                && canonical(&result["agentDefinition"])
                    != canonical(&r["payload"]["agentDefinition"]))
        {
            return Err(err(
                409,
                "request_binding_conflict",
                "Provider returned a different source, target or Agent binding.",
            ));
        }
        let mut provider = json!({});
        for k in [
            "v",
            "fleetId",
            "requestId",
            "engagementId",
            "state",
            "targetProjectId",
            "targetRoomId",
            "sourceRoomId",
            "sourceEventId",
            "role",
            "requestedTokens",
            "allocatedTokens",
            "agentMxid",
            "bound",
            "ready",
            "decidedAt",
            "endedAt",
        ] {
            if !result[k].is_null() {
                provider[k] = result[k].clone();
            }
        }
        if !r["payload"]["agentDefinition"].is_null() {
            provider["agentDefinition"] = r["payload"]["agentDefinition"].clone();
        }
        provider["serving"] = Value::Null;
        if result["serving"].is_object() {
            let mut serving = json!({});
            for k in ["framework", "model", "reasoning", "tier"] {
                if let Some(v) = result["serving"][k].as_str() {
                    serving[k] = json!(v.chars().take(128).collect::<String>());
                }
            }
            provider["serving"] = serving;
        }
        if result["fulfillment"].is_object() {
            provider["fulfillment"] = json!({"phase":result["fulfillment"]["phase"],"incomplete":result["fulfillment"]["incomplete"]});
            if let Some(e) = result["fulfillment"]["error"].as_str() {
                provider["fulfillment"]["error"] = json!(e.chars().take(500).collect::<String>());
            }
        }
        r["state"] = result["state"].clone();
        if r["retirement"].is_object() {
            r["state"] = json!("ended");
            provider["state"] = json!("ended");
            provider["ready"] = json!(false);
            provider["bound"] = json!(false);
            provider["endedAt"] = r["retirement"]["endedAt"].clone();
        }
        r["provider"] = provider;
        r["observedAt"] = json!(now());
        Ok(())
    }
    fn request_view(&self, r: &Value, actor: &str) -> Value {
        let mut out = r.clone();
        out.as_object_mut().unwrap().remove("fingerprint");
        out.as_object_mut().unwrap().remove("payload");
        for k in ["role", "requestedTokens", "ratePerDay"] {
            out[k] = r["payload"][k].clone();
        }
        out["targetRoomId"] = r["payload"]["targetRoomId"].clone();
        if r["payload"]["agentDefinition"].is_object() {
            out["agentDefinition"] = r["payload"]["agentDefinition"].clone();
        }
        if r["payload"]["ownerMxid"] == actor {
            out["ownerDmRoomId"] = r["payload"]["ownerDmRoomId"].clone();
        }
        out
    }
    pub(crate) async fn requests(&self, actor: &str, token: &str) -> Result<Value> {
        let st = self.store.snapshot().await;
        let mut out = Vec::new();
        for before in object_values(&st["requests"]).into_iter().filter(|r| {
            r["requesterMxid"] == actor || st["projects"][s(&r["projectId"])]["ownerMxid"] == actor
        }) {
            let p = &st["projects"][s(&before["projectId"])];
            let f = &st["fleets"][s(&before["fleetId"])];
            let mut r = before.clone();
            let mut agent = None;
            let run: Result<()>=async{
self.validate_project_room(s(&p["roomId"]),actor,token,s(&p["ownerMxid"]),true).await?;
self.active_fleet(s(&r["fleetId"])).await?;
r["usable"]=json!(false);
r["statusVerified"]=json!(false);
if outbound(f){
if r["provider"].is_object(){
let old=r["observedAt"].clone();
let provider=r["provider"].clone();
self.apply_status(&mut r,&provider)?;
r["observedAt"]=old;
}
}
else{
let status=self.provider(f,&format!("/requests/{}",enc(s(&r["requestId"]))),Method::GET,None).await?;
self.apply_status(&mut r,&status)?;
}

                if r["provider"]["agentMxid"].is_string()&&["active","ready"].contains(&s(&r["provider"]["state"])){
let mxid=s(&r["provider"]["agentMxid"]).to_string();
if !self.in_namespace(f,&mxid){
return Err(err(409,"agent_namespace_conflict","Fulfilled identity is outside namespace."));
}
let state=self.room_state(s(&p["roomId"]),token,None).await?;
let joined=self.state_content(&state,"m.room.member",&mxid)["membership"]=="join";
r["agentJoined"]=json!(joined);
if !joined{
r["state"]=json!("admission_pending");
}
else if r["provider"]["ready"]==true&&r["provider"]["bound"]==true&&r["provider"]["fulfillment"]["incomplete"]!=true&&!s(&r["provider"]["serving"]["framework"]).is_empty()&&!s(&r["provider"]["serving"]["model"]).is_empty(){
let aid=format!("fulfilled_{}",&digest(&json!({
"requestId":r["requestId"]}
))[..20]);
agent=Some(json!({
"id":aid,"fleetId":f["id"],"mxid":mxid,"role":r["payload"]["role"],"displayName":r["payload"]["agentDefinition"]["name"].as_str().unwrap_or(&mxid),"approvedRequestId":r["requestId"],"engagementId":r["provider"]["engagementId"],"projectId":p["id"],"authorization":"verified_hagency_fulfillment","state":"registered","createdAt":now(),"localTaskStop":"unknown"}
));
r["usable"]=json!(true);
}
else{
r["state"]=json!("preparing");
}
}

                r["statusVerified"]=json!(r["provider"].is_object());r["lastError"]=Value::Null;if outbound(f){if !proven(f)||!online(f){r["usable"]=json!(false);}
if r["provider"].is_object()&&!current(f,&r){r["usable"]=json!(false);r["statusVerified"]=json!(false);r["lastError"]=json!({"code":"outbound_status_stale","at":now()});}}Ok(())}.await;
            if let Err(e) = run {
                r["usable"] = json!(false);
                r["statusVerified"] = json!(false);
                r["lastError"] = json!({"code":e.code,"at":now()});
            }
            let rid = s(&r["id"]).to_string();
            let fid = s(&f["id"]).to_string();
            let pid = s(&p["id"]).to_string();
            let view = self
                .store
                .change(|state| {
                    if state["requests"][&rid] != before
                        || state["projects"][&pid] != *p
                        || state["fleets"][&fid] != *f
                    {
                        let mut current = state["requests"][&rid].clone();
                        current["usable"] = json!(false);
                        current["statusVerified"] = json!(false);
                        current["lastError"] = json!({"code":"status_refresh_pending","at":now()});
                        return Ok(self.request_view(&current, actor));
                    }
                    state["requests"][&rid] = r.clone();
                    if r["usable"] == true
                        && let Some(a) = agent
                    {
                        let aid = s(&a["id"]).to_string();
                        if state["fleets"][&fid]["agents"].get(&aid).is_none() {
                            state["fleets"][&fid]["agents"][aid] = a;
                        }
                    }
                    Ok(self.request_view(&r, actor))
                })
                .await?;
            out.push(view);
        }
        Ok(json!(out))
    }
}
