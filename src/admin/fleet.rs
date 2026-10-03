use super::api::Admin;
use super::outbound::*;
use super::*;
use reqwest::Method;

pub(crate) fn public_fleet(f: &Value) -> Value {
    let mut out = f.clone();
    let m = out.as_object_mut().unwrap();
    for name in ["agents", "fingerprint", "outboundChange"] {
        m.remove(name);
    }
    let can_queue = s(&f["state"]) == "ready" && proven(f);
    let ready = s(&f["state"]) == "ready"
        && if outbound(f) {
            can_queue && online(f)
        } else {
            timestamp(&f["connection"]["expiresAt"]) > millis()
        };
    if s(&f["state"]) == "ready" && !ready && !can_queue {
        out["state"] = json!("pending_connection");
    }
    out["registration"] = json!({"id":f["registration"]["id"],"url":f["registration"]["url"],"sender_localpart":f["registration"]["sender_localpart"],"namespaces":f["registration"]["namespaces"]});
    out["transport"] = if outbound(f) {
        json!({"mode":"outbound","url":f["transport"]["url"],"generation":f["transport"]["generation"],"online":online(f),"lastSeenAt":f["transport"]["lastSeenAt"]})
    } else {
        json!({"mode":"callback"})
    };
    if outbound(f) {
        out["migration"] = if f["outboundChange"].is_object() {
            json!({"requestId":f["outboundChange"]["requestId"],"state":f["outboundChange"]["state"]})
        } else {
            Value::Null
        };
    }
    out["agentCount"] = json!(f["agents"].as_object().map(|m| m.len()).unwrap_or(0));
    out["readiness"] = json!({
    "ready":ready,"canQueue":can_queue,"identity":if f["representativeVerifiedAt"].is_string(){
    "verified"}
    else{
    "unknown"}
    ,"eventDelivery":if f["connection"]["verifiedAt"].is_string(){
    "verified"}
    else{
    "unverified"}
    ,"reception":if f["reception"]["verifiedAt"].is_string(){
    "verified"}
    else{
    "not_configured"}
    ,"verifiedAt":f["connection"]["verifiedAt"],"expiresAt":f["connection"]["expiresAt"]}
    );

    out
}
pub(crate) fn matches_registration(a: &Value, e: &Value) -> bool {
    [
        "id",
        "url",
        "as_token",
        "hs_token",
        "sender_localpart",
        "rate_limited",
    ]
    .iter()
    .all(|k| a[k] == e[k])
        && ["users", "aliases", "rooms"].iter().all(|k| {
            a["namespaces"][k].as_array().cloned().unwrap_or_default()
                == e["namespaces"][k].as_array().cloned().unwrap_or_default()
        })
        && a["receive_ephemeral"] != true
        && a["io.element.msc4190"] != true
        && a["protocols"].as_array().is_none_or(|p| p.is_empty())
}
impl Admin {
    pub(crate) fn owner(&self, v: &Value) -> Result<String> {
        let mxid = field(v, "Owner Matrix ID", 255)?;
        let (local, server) = mxid.split_once(':').unwrap_or(("", ""));
        if !local.starts_with('@')
            || local.len() < 2
            || local.chars().any(char::is_whitespace)
            || server != self.server_name
        {
            return Err(err(
                400,
                "invalid_owner",
                "The owner must be a full local Matrix ID.",
            ));
        }
        Ok(mxid)
    }
    pub(crate) async fn active_fleet(&self, id: &str) -> Result<Value> {
        let f = self.store.fleet(id).await?;
        if !["ready", "pending_connection"].contains(&s(&f["state"]))
            || f["installation"] != "installed"
        {
            return Err(err(
                409,
                "fleet_inactive",
                "The fleet is not installed or is inactive.",
            ));
        }
        Ok(f)
    }
    pub(crate) async fn fleet_owned(&self, id: &str, actor: &str) -> Result<Value> {
        let f = self.active_fleet(id).await?;
        if f["ownerMxid"] != actor {
            return Err(err(404, "not_found", "Fleet not found."));
        }
        Ok(f)
    }
    pub(crate) fn callback(&self, v: &Value) -> Result<String> {
        let u = url::Url::parse(&field(v, "Callback URL", 1024)?)
            .map_err(|_| err(400, "invalid_callback", "Enter a valid callback URL."))?;
        if !["http", "https"].contains(&u.scheme())
            || !u.username().is_empty()
            || u.password().is_some()
            || u.query().is_some()
            || u.fragment().is_some()
            || !self
                .callback_origins
                .contains(&u.origin().ascii_serialization())
        {
            return Err(err(
                400,
                "callback_policy",
                "The callback origin must be explicitly allowed.",
            ));
        }
        Ok(u.as_str().trim_end_matches('/').into())
    }
    pub(crate) async fn fleet_create(
        &self,
        input: &Value,
        actor: &str,
        token: &str,
    ) -> Result<Value> {
        let request = key(&input["requestId"], "Request ID")?;
        let name = field(&input["name"], "Fleet name", 128)?;
        let owner = self.owner(&input["ownerMxid"])?;
        let mode = input["transportMode"].as_str().unwrap_or("outbound");
        if !["outbound", "callback"].contains(&mode) {
            return Err(err(
                400,
                "invalid_transport",
                "Select outbound or callback transport.",
            ));
        }
        let callback = if mode == "callback" {
            Some(self.callback(&input["callbackUrl"])?)
        } else {
            None
        };
        let fingerprint = digest(&if mode == "callback" {
            json!({"name":name,"ownerMxid":owner,"callbackUrl":callback})
        } else {
            json!({"name":name,"ownerMxid":owner,"transportMode":mode})
        });
        for f in object_values(&self.store.snapshot().await["fleets"]) {
            if f["requestId"] == request {
                if f["fingerprint"] != fingerprint {
                    return Err(err(
                        409,
                        "idempotency_conflict",
                        "This request ID has different content.",
                    ));
                }
                return self.fleet_install(s(&f["id"]), actor, token).await;
            }
        }
        let human = self.palpo.user(&owner, token).await?.ok_or_else(|| {
            err(
                400,
                "invalid_owner",
                "Select an active local human account.",
            )
        })?;
        if human["deactivated"] == true
            || human["locked"] == true
            || human["appservice_id"].is_string()
        {
            return Err(err(
                400,
                "invalid_owner",
                "Select an active local human account.",
            ));
        }
        let id = format!("hf_{}", uuid::Uuid::new_v4().simple());
        let rep = format!("{id}_representative");
        let registration = json!({
        "id":id,"url":callback.clone().unwrap_or_else(||self.relay_url(&id)),"as_token":secret(),"hs_token":secret(),"sender_localpart":rep,"namespaces":{
        "users":[{
        "exclusive":true,"regex":format!("^@{id}_[a-z0-9_]+:{}$",regex::escape(&self.server_name))}
        ],"aliases":[],"rooms":[]}
        ,"rate_limited":true,"receive_ephemeral":false}
        );

        let mut f = json!({
        "id":id,"requestId":request,"fingerprint":fingerprint,"name":name,"ownerMxid":owner,"callbackUrl":callback,"registration":registration,"agents":{
        }
        ,"state":"authorized","installation":"pending","credentialVersion":1,"credentialDeliveredAt":null,"representativeMxid":format!("@{rep}:{}",self.server_name),"createdAt":now(),"createdBy":actor,"lastError":null,"localTaskStop":"unknown"}
        );

        if mode == "outbound" {
            f["transport"] = self.transport(&id, 1);
        }
        self.store
            .change(|st| {
                st["fleets"][&id] = f;
                audit(st, actor, "fleet.authorize", Some(&id), &id, "authorized");
                Ok(())
            })
            .await?;
        self.fleet_install(&id, actor, token).await
    }
    async fn namespace_available(&self, f: &Value, token: &str) -> Result<()> {
        let list = self
            .palpo
            .get("/_palpo/admin/v1/appservices", token)
            .await?;
        let list = list["appservices"]
            .as_array()
            .ok_or_else(|| err(502, "invalid_upstream", "Invalid App Service inventory."))?;
        let prefix = format!("{}_", s(&f["id"]));
        let regex = regex::Regex::new(r"^\^@([a-z0-9_]+)").unwrap();
        for summary in list {
            if summary["id"] == f["id"] {
                continue;
            }
            let other = self
                .palpo
                .get(
                    &format!("/_palpo/admin/v1/appservices/{}", enc(s(&summary["id"]))),
                    token,
                )
                .await?;
            if s(&other["sender_localpart"]).starts_with(&prefix) {
                return Err(err(
                    409,
                    "namespace_conflict",
                    "Representative namespace conflict.",
                ));
            }
            for ns in other["namespaces"]["users"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let pat = s(&ns["regex"]);
                let cap = regex.captures(pat).ok_or_else(|| {
                    err(
                        409,
                        "namespace_policy",
                        "Existing namespace cannot be proven disjoint.",
                    )
                })?;
                let literal = cap.get(1).unwrap().as_str();
                let suffix = &pat[cap.get(0).unwrap().end()..];
                if suffix.starts_with(['?', '*', '+', '{'])
                    || pat.contains('|')
                    || prefix.starts_with(literal)
                    || literal.starts_with(&prefix)
                {
                    return Err(err(
                        409,
                        "namespace_policy",
                        "Existing namespace cannot be proven disjoint.",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(crate) async fn installed(&self, f: &Value, token: &str) -> Result<Value> {
        let a = self
            .palpo
            .get(
                &format!("/_palpo/admin/v1/appservices/{}", enc(s(&f["id"]))),
                token,
            )
            .await?;
        if !matches_registration(&a, &f["registration"]) {
            return Err(err(
                409,
                "registration_drift",
                "The registration differs from the saved operation. No credentials were replaced.",
            ));
        }
        Ok(a)
    }
    pub(crate) async fn identity(&self, f: &Value, mxid: &str, token: &str) -> Result<Value> {
        if let Some(user) = self.palpo.user(mxid, token).await?
            && user["appservice_id"] != f["id"]
        {
            return Err(err(
                409,
                "identity_conflict",
                "Account exists with different ownership.",
            ));
        }
        let identity = self
            .palpo
            .get(
                &format!("/_matrix/client/v3/account/whoami?user_id={}", enc(mxid)),
                s(&f["registration"]["as_token"]),
            )
            .await?;
        let user = self.palpo.user(mxid, token).await?.unwrap_or(Value::Null);
        if identity["user_id"] != mxid
            || user["appservice_id"] != f["id"]
            || user["deactivated"] == true
            || user["locked"] == true
        {
            return Err(err(
                409,
                "identity_unverified",
                "Palpo did not verify active App Service ownership.",
            ));
        }
        Ok(user)
    }
    pub(crate) async fn fleet_install(&self, id: &str, actor: &str, token: &str) -> Result<Value> {
        let mut f = self.store.fleet(id).await?;
        if ["paused", "revoked"].contains(&s(&f["state"])) {
            return Err(err(409, "fleet_inactive", "Fleet is paused or revoked."));
        }
        f["installation"] = json!("installing");
        self.store.save_fleet(id, f.clone()).await?;
        let run: Result<()> = async {
            match self
                .palpo
                .get(&format!("/_palpo/admin/v1/appservices/{}", enc(id)), token)
                .await
            {
                Err(e) if e.status == 404 => {
                    self.namespace_available(&f, token).await?;
                    self.palpo
                        .post("/_palpo/admin/v1/appservices", token, &f["registration"])
                        .await?;
                }
                Err(e) => return Err(e),
                _ => (),
            }
            let actual = self.installed(&f, token).await?;
            if actual["disabled"] == true {
                return Err(err(
                    409,
                    "registration_disabled",
                    "App Service is disabled.",
                ));
            }
            self.identity(&f, s(&f["representativeMxid"]), token)
                .await?;
            Ok(())
        }
        .await;
        match run {
            Ok(()) => {
                f["installation"] = json!("installed");
                f["state"] = json!("pending_connection");
                f["representativeVerifiedAt"] = json!(now());
                f["lastError"] = Value::Null;
            }
            Err(e) => {
                f["installation"] = json!("failed");
                f["lastError"] = json!({"code":e.code,"at":now()});
                self.store.save_fleet(id, f).await?;
                return Err(e);
            }
        }
        self.store.save_fleet(id, f.clone()).await?;
        self.audit(
            actor,
            "fleet.install",
            id,
            id,
            "installed_pending_connection",
        )
        .await?;
        Ok(public_fleet(&f))
    }
    pub(crate) async fn audit(
        &self,
        actor: &str,
        action: &str,
        fleet: &str,
        object: &str,
        result: &str,
    ) -> Result<()> {
        self.store
            .change(|st| {
                audit(st, actor, action, Some(fleet), object, result);
                Ok(())
            })
            .await
    }
    pub(crate) async fn credentials(&self, id: &str, actor: &str) -> Result<Value> {
        let mut f = self.fleet_owned(id, actor).await?;
        let me = self
            .rep(&f, "/_matrix/client/v3/account/whoami", Method::GET, None)
            .await?;
        if me["user_id"] != f["representativeMxid"] {
            return Err(err(
                409,
                "identity_unverified",
                "Representative could not be verified.",
            ));
        }
        if f["credentialDeliveredAt"].is_null() {
            f["credentialDeliveredAt"] = json!(now());
        }
        f["credentialLastDeliveredAt"] = json!(now());
        self.store.save_fleet(id, f.clone()).await?;
        self.audit(
            actor,
            "fleet.credentials.deliver",
            id,
            id,
            "same_version_resumable_delivery",
        )
        .await?;
        let mut out = json!({"fleetId":id,"serverName":self.server_name,"credentialVersion":f["credentialVersion"],"registration":f["registration"]});
        if outbound(&f) {
            out["transport"] = json!({"mode":"outbound","url":f["transport"]["url"],"token":f["transport"]["token"],"generation":f["transport"]["generation"]});
        }
        Ok(out)
    }
    pub(crate) async fn fleet_state(
        &self,
        id: &str,
        action: &str,
        actor: &str,
        token: &str,
    ) -> Result<Value> {
        let mut f = self.store.fleet(id).await?;
        if f["state"] == "revoked" {
            return Err(err(409, "fleet_revoked", "Revocation is final."));
        }
        self.installed(&f, token).await?;
        let disabled = action != "resume";
        self.palpo
            .post(
                &format!(
                    "/_palpo/admin/v1/appservices/{}/{}",
                    enc(id),
                    if disabled { "disable" } else { "enable" }
                ),
                token,
                &json!({}),
            )
            .await?;
        let a = self.installed(&f, token).await?;
        if a["disabled"] != disabled {
            return Err(err(
                502,
                "state_unverified",
                "Palpo did not confirm the registration state.",
            ));
        }
        f["state"] = json!(if action == "revoke" {
            "revoked"
        } else if disabled {
            "paused"
        } else {
            "pending_connection"
        });
        f["localTaskStop"] = json!("unconfirmed");
        f["revocationScope"] = if action == "revoke" {
            json!("appservice_credentials_only")
        } else {
            Value::Null
        };
        self.store.save_fleet(id, f.clone()).await?;
        self.audit(actor, &format!("fleet.{action}"), id, id, s(&f["state"]))
            .await?;
        Ok(public_fleet(&f))
    }
    async fn active_identity_fleet(&self, id: &str, token: &str) -> Result<Value> {
        let f = self.active_fleet(id).await?;
        if self.installed(&f, token).await?["disabled"] == true {
            return Err(err(
                409,
                "registration_disabled",
                "App Service is disabled.",
            ));
        }
        Ok(f)
    }
    pub(crate) async fn agent_create(
        &self,
        id: &str,
        input: &Value,
        actor: &str,
        token: &str,
    ) -> Result<Value> {
        let mut f = self.active_identity_fleet(id, token).await?;
        let aid = key(&input["agentId"], "Agent ID")?.to_lowercase();
        if !aid
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(err(
                400,
                "invalid_input",
                "Use lowercase letters, numbers and underscores.",
            ));
        }
        let name = field(&input["displayName"], "Display name", 128)?;
        let role = field(&input["role"], "Public role", 80)?;
        let request = key(&input["approvedRequestId"], "Approved request reference")?;
        if let Some(a) = f["agents"].get(&aid) {
            if a["approvedRequestId"] != request
                || a["role"] != role
                || a["initialDisplayName"] != name
            {
                return Err(err(
                    409,
                    "idempotency_conflict",
                    "Agent ID is bound to different content.",
                ));
            }
            if a["state"] == "retired" {
                return Err(err(
                    409,
                    "agent_retired",
                    "Retired identities cannot be reused.",
                ));
            }
        } else {
            f["agents"][&aid] = json!({
            "id":aid,"fleetId":id,"mxid":format!("@{id}_agent_{aid}:{}",self.server_name),"role":role,"displayName":name,"initialDisplayName":name,"approvedRequestId":request,"authorization":"administrator_attested","state":"creating","createdAt":now(),"localTaskStop":"unknown"}
            );

            self.store.save_fleet(id, f.clone()).await?;
        }
        let run: Result<()> = async {
            self.identity(&f, s(&f["agents"][&aid]["mxid"]), token)
                .await?;
            self.palpo
                .put(
                    &format!(
                        "/_palpo/admin/v2/users/{}",
                        enc(s(&f["agents"][&aid]["mxid"]))
                    ),
                    token,
                    &json!({"displayname":f["agents"][&aid]["displayName"]}),
                )
                .await?;
            Ok(())
        }
        .await;
        f["agents"][&aid]["state"] = json!(if run.is_ok() { "registered" } else { "failed" });
        f["agents"][&aid]["lastError"] = run
            .as_ref()
            .err()
            .map(|e| json!({"code":e.code,"at":now()}))
            .unwrap_or(Value::Null);
        self.store.save_fleet(id, f.clone()).await?;
        run?;
        self.audit(actor, "agent.create", id, &aid, "registered")
            .await?;
        self.agent_observe(&f, &f["agents"][&aid], token).await
    }
    pub(crate) async fn agent_observe(
        &self,
        f: &Value,
        agent: &Value,
        token: &str,
    ) -> Result<Value> {
        let mut out = agent.clone();
        out["runtimeHealth"] = json!("unknown");
        out["observedAt"] = json!(now());
        let Some(user) = self.palpo.user(s(&agent["mxid"]), token).await? else {
            out["matrixIdentity"] = json!("missing");
            out["joinedRooms"] = Value::Null;
            return Ok(out);
        };
        if user["appservice_id"] != f["id"] {
            return Err(err(
                409,
                "identity_conflict",
                "Observed identity belongs to another fleet.",
            ));
        }
        let rooms = self
            .palpo
            .get(
                &format!(
                    "/_palpo/admin/v1/users/{}/joined_rooms",
                    enc(s(&agent["mxid"]))
                ),
                token,
            )
            .await?;
        if !rooms["joined_rooms"].is_array() {
            return Err(err(
                502,
                "invalid_upstream",
                "Membership observations are unavailable.",
            ));
        }
        out["displayName"] = user["displayname"].clone();
        out["matrixIdentity"] = json!(if user["deactivated"] == true {
            "deactivated"
        } else if user["locked"] == true {
            "locked"
        } else {
            "active"
        });
        out["joinedRooms"] = rooms["joined_rooms"].clone();
        Ok(out)
    }
    pub(crate) async fn agents(&self, id: &str, token: &str) -> Result<Value> {
        let f = self.store.fleet(id).await?;
        let mut out = Vec::new();
        for a in object_values(&f["agents"]) {
            match self.agent_observe(&f, &a, token).await {
                Ok(v) => out.push(v),
                Err(e) => {
                    let mut a = a;
                    a["matrixIdentity"] = json!("unknown");
                    a["joinedRooms"] = Value::Null;
                    a["runtimeHealth"] = json!("unknown");
                    a["observationError"] = json!(e.code);
                    a["observedAt"] = json!(now());
                    out.push(a);
                }
            }
        }
        Ok(json!(out))
    }
    pub(crate) async fn agent_update(
        &self,
        id: &str,
        aid: &str,
        input: &Value,
        actor: &str,
        token: &str,
    ) -> Result<Value> {
        let mut f = self.active_identity_fleet(id, token).await?;
        let mut a = f["agents"]
            .get(aid)
            .cloned()
            .ok_or_else(|| err(404, "not_found", "Agent not found."))?;
        if a["state"] != "registered" {
            return Err(err(
                409,
                "agent_inactive",
                "Only a registered identity can be edited.",
            ));
        }
        if input
            .as_object()
            .unwrap()
            .keys()
            .any(|k| k != "displayName")
        {
            return Err(err(
                400,
                "immutable_identity",
                "Only the display name is editable.",
            ));
        }
        let name = field(&input["displayName"], "Display name", 128)?;
        self.identity(&f, s(&a["mxid"]), token).await?;
        self.palpo
            .put(
                &format!("/_palpo/admin/v2/users/{}", enc(s(&a["mxid"]))),
                token,
                &json!({"displayname":name}),
            )
            .await?;
        let observed = self.agent_observe(&f, &a, token).await?;
        if observed["displayName"] != name {
            return Err(err(
                502,
                "profile_unverified",
                "Profile update was not verified.",
            ));
        }
        a["displayName"] = json!(name);
        f["agents"][aid] = a;
        self.store.save_fleet(id, f).await?;
        self.audit(actor, "agent.profile.update", id, aid, "updated")
            .await?;
        Ok(observed)
    }
    pub(crate) async fn agent_retire(
        &self,
        id: &str,
        aid: &str,
        actor: &str,
        token: &str,
    ) -> Result<Value> {
        let mut f = self.store.fleet(id).await?;
        let mut a = f["agents"]
            .get(aid)
            .cloned()
            .ok_or_else(|| err(404, "not_found", "Agent not found."))?;
        let user = self
            .palpo
            .user(s(&a["mxid"]), token)
            .await?
            .ok_or_else(|| err(409, "identity_conflict", "Managed identity is missing."))?;
        if user["appservice_id"] != id {
            return Err(err(409, "identity_conflict", "Identity ownership differs."));
        }
        a["state"] = json!("retiring");
        f["agents"][aid] = a.clone();
        self.store.save_fleet(id, f.clone()).await?;
        let run: Result<Value> = async {
            self.palpo
                .post(
                    &format!("/_palpo/admin/v1/deactivate/{}", enc(s(&a["mxid"]))),
                    token,
                    &json!({"erase":false}),
                )
                .await?;
            let observed = self.agent_observe(&f, &a, token).await?;
            if observed["matrixIdentity"] != "deactivated"
                || observed["joinedRooms"]
                    .as_array()
                    .is_none_or(|r| !r.is_empty())
            {
                return Err(err(
                    502,
                    "retirement_unverified",
                    "Deactivation or room removal is incomplete.",
                ));
            }
            match self
                .palpo
                .get(
                    &format!(
                        "/_matrix/client/v3/account/whoami?user_id={}",
                        enc(s(&a["mxid"]))
                    ),
                    s(&f["registration"]["as_token"]),
                )
                .await
            {
                Err(e) if [401, 403].contains(&e.status) => (),
                Err(e) => return Err(e),
                Ok(_) => {
                    return Err(err(
                        502,
                        "retirement_unverified",
                        "Retired identity still authenticates.",
                    ));
                }
            };
            Ok(observed)
        }
        .await;
        match run {
            Ok(mut out) => {
                a["state"] = json!("retired");
                a["localTaskStop"] = json!("unconfirmed");
                if a["retiredAt"].is_null() {
                    a["retiredAt"] = json!(now());
                }
                a["lastError"] = Value::Null;
                for (k, v) in a.as_object().unwrap() {
                    out[k] = v.clone();
                }
                f["agents"][aid] = a;
                self.store.save_fleet(id, f).await?;
                self.audit(
                    actor,
                    "agent.retire",
                    id,
                    aid,
                    "matrix_access_revoked_local_stop_unconfirmed",
                )
                .await?;
                Ok(out)
            }
            Err(e) => {
                a["lastError"] = json!({"code":e.code,"at":now()});
                f["agents"][aid] = a;
                self.store.save_fleet(id, f).await?;
                Err(e)
            }
        }
    }
}
