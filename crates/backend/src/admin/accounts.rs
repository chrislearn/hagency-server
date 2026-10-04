use super::api::Admin;
use super::*;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use std::sync::atomic::Ordering;
const TERMINAL: [&str; 4] = ["registered", "rejected", "expired", "name_unavailable"];
fn account_view(r: &Value) -> Value {
    json!({"id":r["id"],"userId":r["userId"],"status":r["status"],"createdAt":r["createdAt"],"expiresAt":r["expiresAt"]})
}
impl Admin {
    pub(crate) async fn account_setup(&self) -> anyhow::Result<()> {
        let Some(c) = &self.account_config else {
            return Ok(());
        };
        self.owner(&c["botMxid"])?;
        let valid = !s(&c["botToken"]).is_empty()
            && !s(&c["adminToken"]).is_empty()
            && !s(&c["registrationToken"]).is_empty()
            && regex::Regex::new(r"^[a-f0-9]{64}$")
                .unwrap()
                .is_match(s(&c["passwordKey"]))
            && c["approvers"]
                .as_array()
                .is_some_and(|a| !a.is_empty() && !a.contains(&c["botMxid"]));
        anyhow::ensure!(valid, "invalid account approval configuration");
        let mut approvers = Vec::new();
        for approver in c["approvers"].as_array().unwrap() {
            approvers.push(self.owner(approver)?);
        }
        approvers.sort();
        approvers.dedup();
        let binding = digest(&json!([
            c["botMxid"],
            approvers,
            hash(s(&c["passwordKey"]))
        ]));
        self.store
            .change(|st| {
                if !st["accountAccess"].is_object() {
                    st["accountAccess"] = json!({"requests":{},"cursor":null});
                }
                if st["accountAccess"]["binding"].is_string()
                    && st["accountAccess"]["binding"] != binding
                {
                    return Err(err(
                        409,
                        "account_binding_changed",
                        "Migrate pending account requests before changing identity or key.",
                    ));
                }
                st["accountAccess"]["binding"] = json!(binding);
                Ok(())
            })
            .await?;
        Ok(())
    }
    pub(crate) fn account_public(&self) -> Value {
        json!({"enabled":self.account_config.is_some(),"ready":self.accounts_ready.load(Ordering::Relaxed),"serverName":self.server_name})
    }
    pub(crate) async fn account_admin_view(&self) -> Value {
        let st = self.store.snapshot().await;
        let mut out = self.account_public();
        out["roomId"] = st["accountAccess"]["roomId"].clone();
        out["botMxid"] = self
            .account_config
            .as_ref()
            .map(|c| c["botMxid"].clone())
            .unwrap_or(Value::Null);
        out["lastError"] = json!(*self.account_error.lock().await);
        out["requests"] = json!(
            object_values(&st["accountAccess"]["requests"])
                .iter()
                .map(|r| {
                    let mut v = account_view(r);
                    for k in ["displayName", "reason", "decidedBy", "lastError"] {
                        v[k] = r[k].clone();
                    }
                    v
                })
                .collect::<Vec<_>>()
        );
        out
    }
    fn cipher(&self) -> Result<Aes256Gcm> {
        let c = self.account_config.as_ref().ok_or_else(|| {
            err(
                503,
                "account_requests_unavailable",
                "Account requests are unavailable.",
            )
        })?;
        let key = hex::decode(s(&c["passwordKey"]))
            .map_err(|_| err(500, "account_key_invalid", "Invalid password key."))?;
        Aes256Gcm::new_from_slice(&key)
            .map_err(|_| err(500, "account_key_invalid", "Invalid password key."))
    }
    fn seal(&self, password: &str, id: &str) -> Result<Value> {
        let iv: [u8; 12] = rand::random();
        let mut data = self
            .cipher()?
            .encrypt(
                Nonce::from_slice(&iv),
                Payload {
                    msg: password.as_bytes(),
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| {
                err(
                    500,
                    "password_seal_failed",
                    "Could not seal pending password.",
                )
            })?;
        let tag = data.split_off(data.len() - 16);
        Ok(json!({"iv":hex::encode(iv),"data":hex::encode(data),"tag":hex::encode(tag)}))
    }
    fn unseal(&self, row: &Value) -> Result<String> {
        let iv = hex::decode(s(&row["password"]["iv"]))
            .map_err(|_| err(500, "password_seal_invalid", "Invalid pending password."))?;
        if iv.len() != 12 {
            return Err(err(
                500,
                "password_seal_invalid",
                "Invalid pending password.",
            ));
        }
        let mut data = hex::decode(s(&row["password"]["data"]))
            .map_err(|_| err(500, "password_seal_invalid", "Invalid pending password."))?;
        data.extend(
            hex::decode(s(&row["password"]["tag"]))
                .map_err(|_| err(500, "password_seal_invalid", "Invalid pending password."))?,
        );
        let plaintext = self
            .cipher()?
            .decrypt(
                Nonce::from_slice(&iv),
                Payload {
                    msg: &data,
                    aad: s(&row["id"]).as_bytes(),
                },
            )
            .map_err(|_| err(500, "password_seal_invalid", "Invalid pending password."))?;
        String::from_utf8(plaintext)
            .map_err(|_| err(500, "password_seal_invalid", "Invalid pending password."))
    }
    fn account_get<'a>(&self, st: &'a Value, id: &Value, receipt: &Value) -> Result<&'a Value> {
        let r = st["accountAccess"]["requests"].get(s(id)).ok_or_else(|| {
            err(
                404,
                "account_request_missing",
                "Account request not found. Use the browser that submitted it.",
            )
        })?;
        if !regex::Regex::new(r"^[a-f0-9]{64}$")
            .unwrap()
            .is_match(s(receipt))
            || !same(s(&r["receiptHash"]), &hash(s(receipt)))
        {
            return Err(err(
                404,
                "account_request_missing",
                "Account request not found. Use the browser that submitted it.",
            ));
        }
        Ok(r)
    }
    fn account_finish_in(&self, st: &mut Value, id: &str, status: &str) {
        let r = &mut st["accountAccess"]["requests"][id];
        r["status"] = json!(status);
        r["finishedAt"] = json!(millis());
        r.as_object_mut().unwrap().remove("password");
        r.as_object_mut().unwrap().remove("lastError");
        let actor = r["decidedBy"]
            .as_str()
            .unwrap_or("account-worker")
            .to_string();
        audit(st, &actor, &format!("account.{status}"), None, id, status);
    }
    fn account_expire_in(&self, st: &mut Value) {
        let ids: Vec<_> = object_values(&st["accountAccess"]["requests"])
            .iter()
            .filter(|r| {
                ["notification_pending", "pending"].contains(&s(&r["status"]))
                    && r["expiresAt"].as_i64().unwrap_or(0) <= millis()
            })
            .map(|r| s(&r["id"]).to_string())
            .collect();
        for id in ids {
            self.account_finish_in(st, &id, "expired");
        }
    }
    pub(crate) async fn account_submit(&self, input: &Value) -> Result<Value> {
        if self.account_config.is_none() || !self.accounts_ready.load(Ordering::Relaxed) {
            return Err(err(
                503,
                "account_requests_unavailable",
                "Account requests are temporarily unavailable.",
            ));
        }
        let id = s(&input["id"]);
        let receipt = s(&input["receipt"]);
        if !regex::Regex::new(r"^[a-f0-9]{32}$").unwrap().is_match(id)
            || !regex::Regex::new(r"^[a-f0-9]{64}$")
                .unwrap()
                .is_match(receipt)
        {
            return Err(err(
                400,
                "invalid_request",
                "A valid request receipt is required.",
            ));
        }
        let username = s(&input["username"]);
        if !regex::Regex::new(r"^[a-z][a-z0-9_.=-]{0,63}$")
            .unwrap()
            .is_match(username)
        {
            return Err(err(
                400,
                "invalid_username",
                "Use 1–64 lowercase letters, digits, dots, underscores, equals signs or hyphens.",
            ));
        }
        let password = s(&input["password"]);
        if !(12..=256).contains(&password.chars().count()) {
            return Err(err(
                400,
                "invalid_password",
                "Use a password between 12 and 256 characters.",
            ));
        }
        let name = field(&input["displayName"], "Name", 128)
            .map_err(|_| err(400, "invalid_details", "Enter your name and reason."))?;
        let reason = field(&input["reason"], "Reason", 1000)
            .map_err(|_| err(400, "invalid_details", "Enter your name and reason."))?;
        let user = format!("@{username}:{}", self.server_name);
        let fp = digest(&json!([username, name, reason]));
        let sealed = self.seal(password, id)?;
        self.store
            .change(|st| {
                if st["accountAccess"]["requests"].get(id).is_some() {
                    let r = self.account_get(st, &input["id"], &input["receipt"])?;
                    if r["fingerprint"] != fp
                        || (r["password"].is_object() && self.unseal(r)? != password)
                    {
                        return Err(err(
                            409,
                            "request_changed",
                            "Request has different details.",
                        ));
                    }
                    return Ok(account_view(r));
                }
                self.account_expire_in(st);
                let rows = object_values(&st["accountAccess"]["requests"]);
                if rows
                    .iter()
                    .any(|r| r["userId"] == user && !TERMINAL.contains(&s(&r["status"])))
                {
                    return Err(err(
                        409,
                        "username_pending",
                        "An account request for this username is pending.",
                    ));
                }
                if rows.len() >= 5000
                    || rows
                        .iter()
                        .filter(|r| !TERMINAL.contains(&s(&r["status"])))
                        .count()
                        >= 200
                {
                    return Err(err(
                        429,
                        "account_queue_full",
                        "Account request queue is full.",
                    ));
                }
                let created = millis();
                let expires = created + 7 * 86400000;
                let r = json!(
                    { "id" : id, "userId" : user, "username" : username, "displayName" :
                    name, "reason" : reason, "fingerprint" : fp, "receiptHash" :
                    hash(receipt), "password" : sealed, "status" : "notification_pending",
                    "createdAt" : created, "expiresAt" : expires, "deviceId" :
                    format!("PALPO_SIGNUP_{}", hex::encode(rand::random::< [u8; 24] > ())),
                    "digest" : digest(& json!({ "id" : id, "userId" : user, "displayName" :
                    name, "reason" : reason, "expiresAt" : expires })) }
                );
                let view = account_view(&r);
                st["accountAccess"]["requests"][id] = r;
                audit(st, "applicant", "account.request", None, id, "pending");
                Ok(view)
            })
            .await
    }
    pub(crate) async fn account_status(&self, input: &Value) -> Result<Value> {
        self.store
            .change(|st| {
                self.account_get(st, &input["id"], &input["receipt"])?;
                self.account_expire_in(st);
                Ok(account_view(self.account_get(
                    st,
                    &input["id"],
                    &input["receipt"],
                )?))
            })
            .await
    }
    async fn account_is_admin(&self, mxid: &str, c: &Value) -> Result<bool> {
        Ok(self
            .palpo
            .user(mxid, s(&c["adminToken"]))
            .await?
            .is_some_and(|u| {
                u["admin"] == true
                    && u["deactivated"] != true
                    && u["is_guest"] != true
                    && !u["appservice_id"].is_string()
            }))
    }
    async fn account_room(&self, c: &Value) -> Result<Value> {
        let st = self.store.snapshot().await;
        let state = self
            .room_state(s(&st["accountAccess"]["roomId"]), s(&c["botToken"]), None)
            .await?;
        let mut allowed = vec![s(&c["botMxid"])];
        allowed.extend(c["approvers"].as_array().unwrap().iter().map(s));
        if self.state_content(&state, "m.room.join_rules", "")["join_rule"] != "invite"
            || !["joined", "invited"].contains(&s(&self.state_content(
                &state,
                "m.room.history_visibility",
                "",
            )["history_visibility"]))
            || !self
                .state_content(&state, "m.room.encryption", "")
                .is_null()
            || self.state_content(&state, "m.room.member", s(&c["botMxid"]))["membership"] != "join"
            || state.as_array().unwrap().iter().any(|e| {
                e["type"] == "m.room.member"
                    && ["join", "invite"].contains(&s(&e["content"]["membership"]))
                    && !allowed.contains(&s(&e["state_key"]))
            })
        {
            return Err(err(
                409,
                "account_room_not_private",
                "Restore the private administrator room membership and settings.",
            ));
        }
        Ok(state)
    }
    async fn account_initialize(&self, c: &Value) -> Result<()> {
        let me = self
            .palpo
            .get("/_matrix/client/v3/account/whoami", s(&c["botToken"]))
            .await?;
        if me["user_id"] != c["botMxid"] || me["is_guest"] == true {
            return Err(err(
                403,
                "account_bot_mismatch",
                "Account service identity mismatch.",
            ));
        }
        self.palpo.require_admin(s(&c["adminToken"])).await?;
        let mut allowed = Vec::new();
        for a in c["approvers"].as_array().unwrap() {
            if self.account_is_admin(s(a), c).await? {
                allowed.push(a.clone());
            }
        }
        if allowed.is_empty() {
            return Err(err(
                403,
                "account_admin_unavailable",
                "No configured approver has administrator authority.",
            ));
        }
        let st = self.store.snapshot().await;
        if !st["accountAccess"]["roomId"].is_string() {
            let local = format!("palpo_account_approvals_{}", &hash(s(&c["botMxid"]))[..16]);
            let alias = format!("#{local}:{}", self.server_name);
            let room = match self
                .palpo
                .get(
                    &format!("/_matrix/client/v3/directory/room/{}", enc(&alias)),
                    s(&c["botToken"]),
                )
                .await
            {
                Ok(v) => v["room_id"].clone(),
                Err(e) if e.status == 404 => {
                    let r=self.palpo.post("/_matrix/client/v3/createRoom",s(&c["botToken"]),&json!({
"room_alias_name":local,"name":"Palpo · Account approvals","visibility":"private","preset":"private_chat","is_direct":false,"invite":allowed,"creation_content":{
"m.federate":false}
,"power_level_content_override":{
"users":{
s(&c["botMxid"]):100}
,"users_default":0,"invite":100,"events_default":0,"state_default":100}
,"initial_state":[{
"type":"m.room.history_visibility","state_key":"","content":{
"history_visibility":"invited"}
}
]}
)).await?;

                    r["room_id"].clone()
                }
                Err(e) => return Err(e),
            };
            if !room.is_string() {
                return Err(err(
                    502,
                    "invalid_account_room",
                    "Account room identity is missing.",
                ));
            }
            self.store
                .change(|st| {
                    st["accountAccess"]["roomId"] = room;
                    Ok(())
                })
                .await?;
        }
        let state = self.account_room(c).await?;
        let st = self.store.snapshot().await;
        if !st["accountAccess"]["historyUpgrade"].is_object()
            && self.state_content(&state, "m.room.history_visibility", "")["history_visibility"]
                == "joined"
        {
            self.store
                .change(|st| {
                    let ids: Vec<_> = object_values(&st["accountAccess"]["requests"])
                        .iter()
                        .filter(|r| r["status"] == "pending")
                        .map(|r| r["id"].clone())
                        .collect();
                    st["accountAccess"]["historyUpgrade"] = json!({"pending":true,"requests":ids});
                    Ok(())
                })
                .await?;
        }
        let st = self.store.snapshot().await;
        if st["accountAccess"]["historyUpgrade"]["pending"] == true {
            self.palpo
                .put(
                    &format!(
                        "/_matrix/client/v3/rooms/{}/state/m.room.history_visibility",
                        enc(s(&st["accountAccess"]["roomId"]))
                    ),
                    s(&c["botToken"]),
                    &json!({"history_visibility":"invited"}),
                )
                .await?;
            if self.state_content(
                &self.account_room(c).await?,
                "m.room.history_visibility",
                "",
            )["history_visibility"]
                != "invited"
            {
                return Err(err(
                    409,
                    "account_history_unconfirmed",
                    "Invitation history was not confirmed.",
                ));
            }
            self.store
                .change(|st| {
                    let ids = st["accountAccess"]["historyUpgrade"]["requests"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    for id in ids {
                        let r = &mut st["accountAccess"]["requests"][s(&id)];
                        if r["status"] != "pending" {
                            continue;
                        }
                        if !r["supersededSourceEventIds"].is_array() {
                            r["supersededSourceEventIds"] = json!([]);
                        }
                        let old = r["sourceEventId"].clone();
                        r["supersededSourceEventIds"]
                            .as_array_mut()
                            .unwrap()
                            .push(old);
                        r["notificationVersion"] =
                            json!(r["notificationVersion"].as_u64().unwrap_or(0) + 1);
                        r["status"] = json!("notification_pending");
                        r.as_object_mut().unwrap().remove("nextAttemptAt");
                    }
                    st["accountAccess"]["historyUpgrade"]["pending"] = json!(false);
                    Ok(())
                })
                .await?;
        }
        self.accounts_ready.store(true, Ordering::Relaxed);
        Ok(())
    }
    fn account_card(&self, r: &Value, c: &Value) -> Value {
        let expires = chrono::DateTime::from_timestamp_millis(r["expiresAt"].as_i64().unwrap())
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        json!({
        "msgtype":"m.text","body":format!("Account request: {}\nName: {}\nReason: {}\nApprove creates an ordinary Matrix account. Agent resources still require Hagency approval.",s(&r["userId"]),s(&r["displayName"]),s(&r["reason"])),"org.octos.approval_request":{
        "request_id":r["id"],"tool_name":"palpo.register_account","tool_args_digest":r["digest"],"title":format!("Register {}",s(&r["userId"])),"summary":format!("{}\n{}\nOrdinary user account; no administrator privileges.",s(&r["displayName"]),s(&r["reason"])),"risk_level":"normal","authorized_approvers":c["approvers"],"expires_at":expires,"on_timeout":"notify"}
        ,"org.octos.actions":[{
        "id":"approve","label":"Approve","style":"primary"}
        ,{
        "id":"deny","label":"Reject","style":"danger"}
        ]}
        )
    }
    async fn account_send(
        &self,
        r: &Value,
        c: &Value,
        kind: &str,
        content: &Value,
    ) -> Result<Value> {
        let st = self.store.snapshot().await;
        self.palpo
            .put(
                &format!(
                    "/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
                    enc(s(&st["accountAccess"]["roomId"])),
                    enc(&format!("account_{}_{kind}", s(&r["id"])))
                ),
                s(&c["botToken"]),
                content,
            )
            .await
    }
    async fn account_notify(&self, r: &Value, c: &Value) -> Result<()> {
        if self
            .palpo
            .user(s(&r["userId"]), s(&c["adminToken"]))
            .await?
            .is_some()
        {
            return self
                .store
                .change(|st| {
                    self.account_finish_in(st, s(&r["id"]), "name_unavailable");
                    Ok(())
                })
                .await;
        }
        self.account_room(c).await?;
        let version = r["notificationVersion"].as_u64().unwrap_or(0);
        let sent = self
            .account_send(
                r,
                c,
                &if version > 0 {
                    format!("request_v{version}")
                } else {
                    "request".into()
                },
                &self.account_card(r, c),
            )
            .await?;
        if !sent["event_id"].is_string() {
            return Err(err(
                502,
                "invalid_account_event",
                "Account request event was not confirmed.",
            ));
        }
        self.store
            .change(|st| {
                let row = &mut st["accountAccess"]["requests"][s(&r["id"])];
                if row["status"] == "notification_pending" {
                    row["sourceEventId"] = sent["event_id"].clone();
                    row["status"] = json!("pending");
                }
                Ok(())
            })
            .await
    }
    async fn account_decide(&self, event: &Value, c: &Value) -> Result<()> {
        let response = &event["content"]["org.octos.approval_response"];
        if event["type"] != "m.room.message"
            || !response.is_object()
            || event["sender"] == c["botMxid"]
        {
            return Ok(());
        }
        let st = self.store.snapshot().await;
        let Some(r) = st["accountAccess"]["requests"]
            .get(s(&response["request_id"]))
            .cloned()
        else {
            return Ok(());
        };
        if r["status"] != "pending" {
            return Ok(());
        }
        let id = s(&r["id"]);
        let valid = event["room_id"] == st["accountAccess"]["roomId"]
            && millis() < r["expiresAt"].as_i64().unwrap_or(0)
            && c["approvers"]
                .as_array()
                .unwrap()
                .contains(&event["sender"])
            && ["approve", "deny"].contains(&s(&response["decision"]))
            && response["source_event_id"] == r["sourceEventId"]
            && response["tool_args_digest"] == r["digest"]
            && event["content"]["m.relates_to"]["m.in_reply_to"]["event_id"] == r["sourceEventId"];
        let action = if !valid {
            Some("account.invalid_decision")
        } else {
            let state = self.account_room(c).await?;
            if self.state_content(&state, "m.room.member", s(&event["sender"]))["membership"]
                != "join"
                || !self.account_is_admin(s(&event["sender"]), c).await?
            {
                Some("account.unauthorized_decision")
            } else {
                None
            }
        };
        if let Some(action) = action {
            return self
                .store
                .change(|st| {
                    audit(st, s(&event["sender"]), action, None, id, "refused");
                    Ok(())
                })
                .await;
        }
        self.store
            .change(|st| {
                let row = &mut st["accountAccess"]["requests"][id];
                if row["status"] != "pending" {
                    return Ok(());
                }
                if millis() >= row["expiresAt"].as_i64().unwrap_or(0) {
                    self.account_finish_in(st, id, "expired");
                    return Ok(());
                }
                row["decidedBy"] = event["sender"].clone();
                row["decisionEventId"] = event["event_id"].clone();
                row["decidedAt"] = json!(millis());
                if response["decision"] == "deny" {
                    self.account_finish_in(st, id, "rejected");
                } else {
                    row["status"] = json!("approved");
                    row.as_object_mut().unwrap().remove("nextAttemptAt");
                    audit(
                        st,
                        s(&event["sender"]),
                        "account.approve",
                        None,
                        id,
                        "approved",
                    );
                }
                Ok(())
            })
            .await
    }
    async fn account_register_call(&self, body: &Value) -> Result<Value> {
        let (status, data) = self
            .palpo
            .raw(
                "/_matrix/client/v3/register",
                None,
                reqwest::Method::POST,
                Some(body),
            )
            .await?;
        if status == 401 && data["flows"].is_array() && data["session"].is_string() {
            return Ok(json!({"challenge":data}));
        }
        if !(200..300).contains(&status) {
            return Err(err(
                status,
                if s(&data["errcode"]).starts_with("M_") {
                    s(&data["errcode"])
                } else {
                    "registration_failed"
                },
                "Palpo could not complete account registration.",
            ));
        }
        Ok(data)
    }
    async fn account_provision(&self, r: &Value, c: &Value) -> Result<()> {
        let id = s(&r["id"]);
        if let Some(existing) = self
            .palpo
            .user(s(&r["userId"]), s(&c["adminToken"]))
            .await?
        {
            let mut status = "name_unavailable";
            if r["attempted"] == true
                && existing["admin"] != true
                && !existing["appservice_id"].is_string()
                && existing["deactivated"] != true
            {
                let proof = self
                    .palpo
                    .get(
                        &format!("/_palpo/admin/v1/whois/{}", enc(s(&r["userId"]))),
                        s(&c["adminToken"]),
                    )
                    .await?;
                if proof["devices"].get(s(&r["deviceId"])).is_some() {
                    status = "registered";
                }
            }
            return self
                .store
                .change(|st| {
                    self.account_finish_in(st, id, status);
                    Ok(())
                })
                .await;
        }
        self.account_room(c).await?;
        if !self.account_is_admin(s(&r["decidedBy"]), c).await? {
            return Err(err(
                403,
                "account_approver_revoked",
                "Approver no longer has administrator authority.",
            ));
        }
        let mut body = json!({"username":r["username"],"password":self.unseal(r)?,"device_id":r["deviceId"],"initial_device_display_name":"Palpo account registration"});
        self.store
            .change(|st| {
                st["accountAccess"]["requests"][id]["attempted"] = json!(true);
                st["accountAccess"]["requests"][id]["status"] = json!("registering");
                Ok(())
            })
            .await?;
        let mut result = self.account_register_call(&body).await?;
        if result["challenge"].is_object() {
            let flow = result["challenge"]["flows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| {
                    f["stages"].as_array().is_some_and(|a| {
                        a.len() == 1
                            && ["m.login.registration_token", "m.login.dummy"].contains(&s(&a[0]))
                    })
                })
                .ok_or_else(|| {
                    err(
                        409,
                        "unsupported_registration",
                        "Unsupported registration flow.",
                    )
                })?;
            let typ = s(&flow["stages"][0]);
            body["auth"] = json!({"type":typ,"session":result["challenge"]["session"]});
            if typ == "m.login.registration_token" {
                body["auth"]["token"] = c["registrationToken"].clone();
            }
            result = self.account_register_call(&body).await?;
        }
        if result["user_id"] != r["userId"] || !result["access_token"].is_string() {
            return Err(err(
                502,
                "registration_identity_unconfirmed",
                "Registered identity was not confirmed.",
            ));
        }
        self.store
            .change(|st| {
                self.account_finish_in(st, id, "registered");
                Ok(())
            })
            .await?;
        let token = s(&result["access_token"]);
        let _ = self
            .palpo
            .put(
                &format!(
                    "/_matrix/client/v3/profile/{}/displayname",
                    enc(s(&r["userId"]))
                ),
                token,
                &json!({"displayname":r["displayName"]}),
            )
            .await;
        let _ = self
            .palpo
            .post("/_matrix/client/v3/logout", token, &json!({}))
            .await;
        Ok(())
    }
    pub async fn account_tick(&self) -> Result<()> {
        let Some(c) = &self.account_config else {
            return Ok(());
        };
        let _worker = self.mutations.lock().await;
        let run: Result<()>=async{
if !self.accounts_ready.load(Ordering::Relaxed){
self.account_initialize(c).await?;
}
self.store.change(|st|{
self.account_expire_in(st);
Ok(())}
).await?;
self.account_room(c).await?;
for mut r in object_values(&self.store.snapshot().await["accountAccess"]["requests"]){
if r["nextAttemptAt"].as_i64().unwrap_or(0)>millis(){
continue;
}
let id=s(&r["id"]).to_string();
let step: Result<()>=async{
if r["status"]=="notification_pending"{
self.account_notify(&r,c).await?;
}

if ["approved","registering"].contains(&s(&r["status"])){
self.account_provision(&r,c).await?;
}
r=self.store.snapshot().await["accountAccess"]["requests"][&id].clone();
if TERMINAL.contains(&s(&r["status"]))&&r["sourceEventId"].is_string()&&!r["resultEventId"].is_string(){
let sent=self.account_send(&r,c,"result",&json!({
"msgtype":"m.notice","body":format!("{}: {}.",s(&r["userId"]),s(&r["status"]).replace('_'," ")),"m.relates_to":{
"m.in_reply_to":{
"event_id":r["sourceEventId"]}
}
}
)).await?;
self.store.change(|st|{
st["accountAccess"]["requests"][&id]["resultEventId"]=sent["event_id"].clone();
Ok(())}
).await?;
}
Ok(())}
.await;

            self.store
    .change(|st| {
        if let Err(e) = step {
            if ["M_EXCLUSIVE", "M_INVALID_USERNAME"].contains(&e.code.as_str()) {
                self.account_finish_in(st, &id, "name_unavailable");
            } else {
                let row = &mut st["accountAccess"]["requests"][&id];
                let retry = row["retryCount"].as_u64().unwrap_or(0) + 1;
                row["lastError"] = json!(e.code);
                row["retryCount"] = json!(retry);
                row["nextAttemptAt"] = json!(
                    millis() + (2500_i64 * 2_i64.pow((retry - 1).min(5) as u32))
                    .min(60000)
                );
            }
        } else {
            let row = &mut st["accountAccess"]["requests"][&id];
            row.as_object_mut().unwrap().remove("nextAttemptAt");
            row.as_object_mut().unwrap().remove("retryCount");
        }
        Ok(())
    }).await?;

        }
let st=self.store.snapshot().await;
let query={
let mut query=url::form_urlencoded::Serializer::new(String::new());
query.append_pair("dir","f").append_pair("limit","100");
if st["accountAccess"]["cursor"].is_string(){
query.append_pair("from",s(&st["accountAccess"]["cursor"]));
}
query.finish()}
;
let batch=self.palpo.get(&format!("/_matrix/client/v3/rooms/{}/messages?{}",enc(s(&st["accountAccess"]["roomId"])),query),s(&c["botToken"])).await?;
let events=batch["chunk"].as_array().ok_or_else(||err(502,"invalid_account_history","Account history could not be read."))?;
for event in events{
let mut event=event.clone();
event["room_id"]=st["accountAccess"]["roomId"].clone();
self.account_decide(&event,c).await?;
}

if batch["end"].is_string(){self.store.change(|st|{st["accountAccess"]["cursor"]=batch["end"].clone();Ok(())}).await?;}Ok(())}.await;
        if let Err(e) = &run {
            *self.account_error.lock().await = Some(e.code.clone());
            self.accounts_ready.store(false, Ordering::Relaxed);
        } else {
            *self.account_error.lock().await = None;
        }
        run
    }
    pub fn start_account_worker(&self) -> tokio::task::JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_millis(2500));
            loop {
                tick.tick().await;
                if let Err(e) = this.account_tick().await {
                    tracing::warn!(code=%e.code,"account worker waiting for recovery");
                }
            }
        })
    }
}
