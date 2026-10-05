use super::api::Admin;
use super::fleet::public_fleet;
use super::*;

pub(crate) fn outbound(f: &Value) -> bool {
    f["transport"]["mode"] == "outbound"
}
pub(crate) fn proven(f: &Value) -> bool {
    outbound(f)
        && f["connection"]["generation"] == f["transport"]["generation"]
        && f["connection"]["verifiedAt"].is_string()
}
pub(crate) fn online(f: &Value) -> bool {
    let seen = timestamp(&f["transport"]["lastSeenAt"]);
    seen > 0 && millis() - seen < 90000
}
pub(crate) fn current(f: &Value, r: &Value) -> bool {
    let received = timestamp(&r["outboundStatus"]["receivedAt"]);
    let observed = timestamp(&r["outboundStatus"]["observedAt"]);
    r["outboundStatus"]["generation"] == f["transport"]["generation"]
        && observed > 0
        && observed <= received + 5000
        && millis() - observed.min(received) < 90000
}
fn valid_id(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|s| !s.is_empty() && s.len() <= 255 && !s.chars().any(char::is_control))
}
impl Admin {
    pub(crate) fn transport(&self, id: &str, generation: u64) -> Value {
        json!({"mode":"outbound","url":format!("{}/api/fleet/v2/{id}",self.public_origin.origin().ascii_serialization()),"token":secret(),"generation":generation,"sequence":0})
    }
    pub(crate) fn relay_url(&self, id: &str) -> String {
        format!(
            "{}/api/relay/v2/{id}",
            self.palpo.url.origin().ascii_serialization()
        )
    }
    pub(crate) async fn authenticate(
        &self,
        id: &str,
        token: &str,
        generation: &str,
        relay: bool,
    ) -> Result<Value> {
        let f = self.store.fleet(id).await.map_err(|_| {
            err(
                401,
                "transport_unauthorized",
                "The fleet credential is not active.",
            )
        })?;
        if !outbound(&f)
            || !["ready", "pending_connection"].contains(&s(&f["state"]))
            || f["installation"] != "installed"
            || !same(
                token,
                s(if relay {
                    &f["registration"]["hs_token"]
                } else {
                    &f["transport"]["token"]
                }),
            )
        {
            return Err(err(
                401,
                "transport_unauthorized",
                "The fleet credential is not active.",
            ));
        }
        if !relay
            && (generation.is_empty()
                || generation.starts_with('0')
                || generation.parse::<u64>().ok() != f["transport"]["generation"].as_u64())
        {
            return Err(err(
                409,
                "generation_conflict",
                "Import the current transport generation.",
            ));
        }
        Ok(f)
    }
    pub(crate) fn usage_in(&self, st: &Value, id: &str) -> Value {
        let rows: Vec<_> = st["deliveries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["fleet"] == id)
            .collect();
        let pending: Vec<_> = rows.iter().filter(|r| r["acked"].is_null()).collect();
        json!({
        "records":rows.len(),"pending":pending.len(),"bytes":pending.iter().map(|r|r["bytes"].as_u64().unwrap_or(0)).sum::<u64>(),"limits":{
        "records":self.queue.max_records,"pending":self.queue.max_pending,"bytes":self.queue.max_bytes}
        }
        )
    }
    pub(crate) fn enqueue_in(
        &self,
        st: &mut Value,
        f: &Value,
        lane: &str,
        kind: &str,
        id: &str,
        payload: &Value,
    ) -> Result<()> {
        if !valid_id(&json!(id)) || !["matrix", "work"].contains(&lane) {
            return Err(err(
                400,
                "invalid_delivery",
                "Invalid delivery identity or lane.",
            ));
        }
        let hash = canonical_digest(payload);
        if let Some(row) = st["deliveries"].as_array().unwrap().iter().rev().find(|r| {
            r["fleet"] == f["id"]
                && r["lane"] == lane
                && r["id"] == id
                && (lane == "matrix" || r["generation"] == f["transport"]["generation"])
        }) {
            if row["digest"] != hash {
                return Err(err(
                    409,
                    "delivery_conflict",
                    "Delivery identity has different content.",
                ));
            }
            return Ok(());
        }
        let bytes = serde_json::to_vec(&canonical(payload)).unwrap().len();
        let usage = self.usage_in(st, s(&f["id"]));
        if usage["records"].as_u64().unwrap() >= self.queue.max_records as u64
            || usage["pending"].as_u64().unwrap() >= self.queue.max_pending as u64
            || usage["bytes"].as_u64().unwrap() + bytes as u64 > self.queue.max_bytes as u64
        {
            return Err(err(
                503,
                "queue_full",
                "The durable fleet queue is full. Retry the same operation after delivery.",
            ));
        }
        st["deliveries"].as_array_mut().unwrap().push(json!({
"fleet":f["id"],"generation":f["transport"]["generation"],"lane":lane,"id":id,"kind":kind,"digest":hash,"payload":payload,"bytes":bytes,"consumer":null,"token":null,"expires":null,"acked":null}
));

        Ok(())
    }
    pub(crate) async fn enqueue(
        &self,
        f: &Value,
        lane: &str,
        kind: &str,
        id: &str,
        payload: &Value,
    ) -> Result<()> {
        self.store
            .change(|st| self.enqueue_in(st, f, lane, kind, id, payload))
            .await
    }
    pub(crate) async fn transaction(
        &self,
        id: &str,
        token: &str,
        tid: &str,
        input: &Value,
    ) -> Result<Value> {
        let events = input["events"]
            .as_array()
            .filter(|a| a.len() <= 1000 && a.iter().all(Value::is_object))
            .ok_or_else(|| {
                err(
                    400,
                    "invalid_transaction",
                    "A bounded Matrix events array is required.",
                )
            })?;
        self.store
            .change(|st| {
                let f = st["fleets"][id].clone();
                if !outbound(&f)
                    || !["ready", "pending_connection"].contains(&s(&f["state"]))
                    || f["installation"] != "installed"
                    || !same(token, s(&f["registration"]["hs_token"]))
                {
                    return Err(err(
                        401,
                        "transport_unauthorized",
                        "Relay credential is not active.",
                    ));
                }
                self.enqueue_in(
                    st,
                    &f,
                    "matrix",
                    "transaction",
                    tid,
                    &json!({"transactionId":tid,"body":input}),
                )?;
                for e in events {
                    let p = &st["fleets"][id]["probe"];
                    if !p.is_object()
                        || e["type"] != "com.hagency.connection.probe.v1"
                        || e["room_id"] != p["roomId"]
                        || !valid_id(&e["event_id"])
                        || (!p["eventId"].is_null() && e["event_id"] != p["eventId"])
                        || e["sender"] != f["representativeMxid"]
                        || e["content"]["fleetId"] != id
                        || e["content"]["challenge"] != p["challenge"]
                    {
                        continue;
                    }
                    if !p["matrixTransactionId"].is_null() && p["matrixEventId"] == e["event_id"] {
                        continue;
                    }
                    st["fleets"][id]["probe"]["matrixTransactionId"] = json!(tid);
                    st["fleets"][id]["probe"]["matrixEventId"] = e["event_id"].clone();
                }
                Ok(json!({}))
            })
            .await
    }
    pub(crate) async fn claim(
        &self,
        id: &str,
        token: &str,
        generation: &str,
        lane: &str,
        consumer: &str,
    ) -> Result<Value> {
        if !["matrix", "work"].contains(&lane)
            || uuid::Uuid::parse_str(consumer).is_err()
            || consumer.len() != 36
        {
            return Err(err(
                400,
                "invalid_poll",
                "Use a valid lane and stable UUID consumer.",
            ));
        }
        self.store
            .change(|st| {
                let f = st["fleets"][id].clone();
                if !["ready", "pending_connection"].contains(&s(&f["state"]))
                    || f["installation"] != "installed"
                    || !same(token, s(&f["transport"]["token"]))
                {
                    return Err(err(
                        401,
                        "transport_unauthorized",
                        "Fleet credential is not active.",
                    ));
                }
                if generation.parse::<u64>().ok() != f["transport"]["generation"].as_u64() {
                    return Err(err(
                        409,
                        "generation_conflict",
                        "Import the current transport generation.",
                    ));
                }
                let row = st["deliveries"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|r| {
                        r["fleet"] == id
                            && r["generation"] == f["transport"]["generation"]
                            && r["lane"] == lane
                            && r["acked"].is_null()
                    });
                let Some(row) = row else {
                    return Ok(Value::Null);
                };
                if row["expires"].as_i64().unwrap_or(0) > millis() {
                    return Ok(Value::Null);
                }
                let token = secret();
                let expires = millis() + self.queue.lease_ms;
                row["consumer"] = json!(consumer);
                row["token"] = json!(token);
                row["expires"] = json!(expires);
                Ok(json!(
                    { "id" : row["id"], "lane" : lane, "token" : token, "expiresAt" :
                    chrono::DateTime::from_timestamp_millis(expires).unwrap()
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true), "kind" :
                    row["kind"], "payload" : row["payload"] }
                ))
            })
            .await
    }
    pub(crate) async fn poll(
        &self,
        id: &str,
        token: &str,
        generation: &str,
        lane: &str,
        consumer: &str,
        wait: &str,
    ) -> Result<Value> {
        let wait = wait
            .parse::<u64>()
            .ok()
            .filter(|w| *w <= 25000)
            .ok_or_else(|| {
                err(
                    400,
                    "invalid_poll",
                    "Poll wait must be between zero and 25000 milliseconds.",
                )
            })?;
        let until = tokio::time::Instant::now() + std::time::Duration::from_millis(wait);
        loop {
            let f = self.authenticate(id, token, generation, false).await?;
            let delivery = self.claim(id, token, generation, lane, consumer).await?;
            if !delivery.is_null() || tokio::time::Instant::now() >= until {
                return Ok(
                    json!({"v":2,"generation":f["transport"]["generation"],"delivery":delivery}),
                );
            }
            tokio::time::sleep(
                std::time::Duration::from_millis(100)
                    .min(until.saturating_duration_since(tokio::time::Instant::now())),
            )
            .await;
        }
    }
    pub(crate) async fn ack(
        &self,
        id: &str,
        token: &str,
        generation: &str,
        input: &Value,
    ) -> Result<Value> {
        self.authenticate(id, token, generation, false).await?;
        self.store
            .change(|st| {
                let f = st["fleets"][id].clone();
                if !same(token, s(&f["transport"]["token"]))
                    || generation.parse::<u64>().ok() != f["transport"]["generation"].as_u64()
                    || !["ready", "pending_connection"].contains(&s(&f["state"]))
                {
                    return Err(err(
                        401,
                        "transport_unauthorized",
                        "Fleet credential is not active.",
                    ));
                }
                let row = st["deliveries"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|r| {
                        r["fleet"] == id
                            && r["generation"] == f["transport"]["generation"]
                            && r["lane"] == input["lane"]
                            && r["id"] == input["id"]
                    })
                    .ok_or_else(|| err(409, "stale_lease", "This lease is no longer current."))?;
                if !same(s(&row["token"]), s(&input["token"]))
                    || row["token"].is_null()
                    || (row["acked"].is_null() && row["expires"].as_i64().unwrap_or(0) <= millis())
                {
                    return Err(err(409, "stale_lease", "This lease is no longer current."));
                }
                if row["acked"].is_null() {
                    row["acked"] = json!(millis());
                    row["payload"] = Value::Null;
                }
                Ok(json!({"ok":true}))
            })
            .await
    }
    pub(crate) async fn updates(
        &self,
        id: &str,
        token: &str,
        generation: &str,
        input: &Value,
    ) -> Result<Value> {
        let snapshot = self.store.snapshot().await;
        let coordinator_status = input["statuses"].as_array().is_some_and(|rows| {
            rows.iter().any(|r| {
                snapshot["rustWorkflows"]["actions"]
                    .as_object()
                    .is_some_and(|actions| {
                        actions.values().any(|a| {
                            a["request"]["request"]["id"] == r["requestId"]
                                && a["request"]["request"]["serverEngagementId"] == id
                        })
                    })
            })
        });
        if input.get("coordinatorUpdates").is_some() || coordinator_status {
            let generation = generation
                .parse::<u64>()
                .map_err(|_| err(409, "generation_conflict", "Use the current generation."))?;
            return self
                .operations
                .apply_updates(id, token, generation, input)
                .await
                .map_err(Into::into);
        }
        let mut f = self.authenticate(id, token, generation, false).await?;
        let seq = input["sequence"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
            .ok_or_else(|| err(400, "invalid_update", "Use a positive safe sequence."))?;
        if input["v"] != 2
            || input["generation"] != f["transport"]["generation"]
            || input["heartbeat"] != true
            || (!input["statuses"].is_null()
                && input["statuses"].as_array().is_none_or(|a| a.len() > 200))
            || (!input["probeReceipts"].is_null()
                && input["probeReceipts"]
                    .as_array()
                    .is_none_or(|a| a.len() > 10))
        {
            return Err(err(
                400,
                "invalid_update",
                "Use a bounded v2 update with heartbeat.",
            ));
        }
        let hash = canonical_digest(input);
        let last = f["transport"]["sequence"].as_u64().unwrap_or(0);
        if seq < last {
            return Err(err(409, "stale_sequence", "A newer update is committed."));
        }
        if seq == last {
            if f["transport"]["updateDigest"] != hash {
                return Err(err(
                    409,
                    "sequence_conflict",
                    "Sequence has different content.",
                ));
            }
            return Ok(json!({"ok":true}));
        }
        if !input["capabilities"].is_null() {
            self.apply_capabilities(&mut f, &input["capabilities"])?;
        }
        let st = self.store.snapshot().await;
        let mut records = Vec::new();
        for status in input["statuses"].as_array().into_iter().flatten() {
            if !status.is_object() || status["v"] != 1 {
                return Err(err(
                    400,
                    "invalid_status",
                    "Each status must be a version 1 observation.",
                ));
            }
            let rid = format!("{id}:{}", s(&status["requestId"]));
            let mut r = st["requests"]
                .get(&rid)
                .cloned()
                .ok_or_else(|| err(409, "unknown_request", "Unknown fleet request."))?;
            if status["fleetId"] != id
                || status["role"] != r["payload"]["role"]
                || status["requestedTokens"] != r["payload"]["requestedTokens"]
            {
                return Err(err(
                    409,
                    "request_binding_conflict",
                    "Role or quota does not match.",
                ));
            }
            self.apply_status(&mut r, status)?;
            if status["agentMxid"].is_string() && !self.in_namespace(&f, s(&status["agentMxid"])) {
                return Err(err(
                    409,
                    "agent_namespace_conflict",
                    "Identity is outside the fleet namespace.",
                ));
            }
            let observed = timestamp(&status["observedAt"]);
            r["outboundStatus"] = json!({
            "generation":f["transport"]["generation"],"receivedAt":now(),"observedAt":if observed>0{
            json!(chrono::DateTime::from_timestamp_millis(observed).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis,true))}
            else{
            Value::Null}
            }
            );

            r["usable"] = json!(false);
            r["statusVerified"] = json!(true);
            r["lastError"] = Value::Null;
            records.push(r);
        }
        for receipt in input["probeReceipts"].as_array().into_iter().flatten() {
            let p = &f["probe"];
            if !receipt.is_object() {
                return Err(err(400, "invalid_receipt", "Receipt must be an object."));
            }
            if !p.is_object()
                || receipt["received"] != true
                || receipt["fleetId"] != id
                || receipt["sourceRoomId"] != p["roomId"]
                || p["matrixEventId"] != p["eventId"]
                || receipt["sourceEventId"] != p["eventId"]
                || receipt["challenge"] != p["challenge"]
            {
                return Err(err(
                    409,
                    "probe_binding_conflict",
                    "Receipt does not identify the exact Matrix probe.",
                ));
            }
            if !st["deliveries"].as_array().unwrap().iter().any(|r| {
                r["fleet"] == id
                    && r["generation"] == f["transport"]["generation"]
                    && r["lane"] == "matrix"
                    && r["id"] == p["matrixTransactionId"]
                    && !r["acked"].is_null()
            }) {
                return Err(err(
                    409,
                    "matrix_receipt_pending",
                    "Original Matrix transaction must be acknowledged first.",
                ));
            }
        }
        if input["probeReceipts"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
        {
            let state = self
                .room_state(
                    s(&f["probe"]["roomId"]),
                    s(&f["registration"]["as_token"]),
                    Some(s(&f["representativeMxid"])),
                )
                .await?;
            if self.state_content(&state, "m.room.member", s(&f["ownerMxid"]))["membership"]
                != "join"
                || self.state_content(&state, "m.room.member", s(&f["representativeMxid"]))["membership"]
                    != "join"
            {
                return Err(err(
                    409,
                    "reception_membership_pending",
                    "Owner and representative must be joined.",
                ));
            }
            let completed = now();
            f["probe"]["completedAt"] = json!(completed);
            f["connection"] = json!({"verifiedAt":completed,"generation":f["transport"]["generation"],"sourceRoomId":f["probe"]["roomId"],"sourceEventId":f["probe"]["eventId"],"challenge":f["probe"]["challenge"]});
            f["state"] = json!("ready");
            f["lastError"] = Value::Null;
        }
        f["transport"]["sequence"] = json!(seq);
        f["transport"]["updateDigest"] = json!(hash);
        f["transport"]["lastSeenAt"] = json!(now());
        self.store
            .change(|st| {
                st["fleets"][id] = f;
                for r in records {
                    let rid = s(&r["id"]).to_string();
                    st["requests"][rid] = r;
                }
                Ok(json!({"ok":true}))
            })
            .await
    }
    pub(crate) fn in_namespace(&self, f: &Value, mxid: &str) -> bool {
        f["registration"]["namespaces"]["users"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|ns| regex::Regex::new(s(&ns["regex"])).is_ok_and(|r| r.is_match(mxid)))
    }
    pub(crate) async fn migrate(
        &self,
        id: &str,
        input: &Value,
        actor: &str,
        token: &str,
    ) -> Result<Value> {
        let mut f = self.active_fleet(id).await?;
        let request = key(&input["requestId"], "Migration operation ID")?;
        let rotate = input["rotate"] == true;
        let mut plan = f["outboundChange"].clone();
        if plan["requestId"] == request && plan["rotate"] != rotate {
            return Err(err(
                409,
                "idempotency_conflict",
                "Migration has different content.",
            ));
        }
        if plan.is_null() || plan["requestId"] != request {
            if plan.is_object() && plan["state"] != "done" {
                return Err(err(
                    409,
                    "migration_pending",
                    "Retry the existing migration first.",
                ));
            }
            if outbound(&f) && !rotate {
                return Ok(public_fleet(&f));
            }
            plan = json!({"requestId":request,"rotate":rotate,"state":"pending","previousRegistration":f["registration"],"transport":self.transport(id,f["transport"]["generation"].as_u64().unwrap_or(0)+1)});
            f["outboundChange"] = plan.clone();
            self.store.save_fleet(id, f.clone()).await?;
        }
        if plan["state"] == "done" {
            return Ok(public_fleet(&f));
        }
        let mut desired = plan["previousRegistration"].clone();
        desired["url"] = json!(self.relay_url(id));
        let path = format!("/_palpo/admin/v1/appservices/{}", enc(id));
        let actual = self.palpo.get(&path, token).await?;
        if actual["disabled"] == true {
            return Err(err(
                409,
                "registration_disabled",
                "App Service is disabled.",
            ));
        }
        if !super::fleet::matches_registration(&actual, &desired) {
            if !super::fleet::matches_registration(&actual, &plan["previousRegistration"]) {
                return Err(err(
                    409,
                    "registration_drift",
                    "Registration changed outside the migration.",
                ));
            }
            self.palpo
                .put(
                    &format!("{path}/url"),
                    token,
                    &json!({"url":desired["url"],"expected_url":actual["url"]}),
                )
                .await?;
        }
        let verified = self.palpo.get(&path, token).await?;
        if verified["disabled"] == true || !super::fleet::matches_registration(&verified, &desired)
        {
            return Err(err(
                409,
                "registration_drift",
                "Migration could not be verified.",
            ));
        }
        self.store
            .change(|st| {
                let old = f["transport"]["generation"].clone();
                for r in st["deliveries"].as_array_mut().unwrap() {
                    if r["fleet"] == id
                        && r["generation"] == old
                        && r["acked"].is_null()
                        && r["kind"] != "probe"
                    {
                        r["generation"] = plan["transport"]["generation"].clone();
                        r["consumer"] = Value::Null;
                        r["token"] = Value::Null;
                        r["expires"] = Value::Null;
                    }
                }
                f["registration"] = desired;
                f["callbackUrl"] = Value::Null;
                f["transport"] = plan["transport"].clone();
                f["state"] = json!("pending_connection");
                f["connection"] = Value::Null;
                f["probe"] = Value::Null;
                f["outboundChange"]["state"] = json!("done");
                for r in object_values(&st["requests"]) {
                    if r["fleetId"] == id && r["sourceEventId"].is_string() {
                        let mut p = r["payload"].clone();
                        p["sourceEventId"] = r["sourceEventId"].clone();
                        self.enqueue_in(st, &f, "work", "request", s(&r["requestId"]), &p)?;
                    }
                }
                st["fleets"][id] = f.clone();
                audit(
                    st,
                    actor,
                    "fleet.outbound.migrate",
                    Some(id),
                    &request,
                    "done",
                );
                Ok(public_fleet(&f))
            })
            .await
    }
    pub(crate) async fn retire_allocated(&self, id: &str, input: &Value) -> Result<Value> {
        let token = self.retirement_token.as_deref().ok_or_else(|| {
            err(
                503,
                "retirement_unconfigured",
                "Configure an administrator credential for retirement.",
            )
        })?;
        let mut f = self.active_fleet(id).await?;
        let st = self.store.snapshot().await;
        let rid = format!("{id}:{}", s(&input["requestId"]));
        let mut r = st["requests"]
            .get(&rid)
            .cloned()
            .ok_or_else(|| err(403, "retirement_scope_mismatch", "Unknown allocation."))?;
        let mxid = s(&input["agentMxid"]);
        let prefix = format!("@{id}_agent_");
        let suffix = format!(":{}", self.server_name);
        let aid = mxid
            .strip_prefix(&prefix)
            .and_then(|t| t.strip_suffix(&suffix))
            .filter(|t| {
                !t.is_empty()
                    && t.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            })
            .ok_or_else(|| {
                err(
                    403,
                    "retirement_scope_mismatch",
                    "Invalid managed identity.",
                )
            })?;
        if r["provider"]["agentMxid"] != mxid || input["endedAt"].as_i64().is_none_or(|v| v <= 0) {
            return Err(err(
                403,
                "retirement_scope_mismatch",
                "Retirement must identify the exact allocated Agent.",
            ));
        }
        if object_values(&st["requests"]).iter().any(|other| {
            other["id"] != r["id"]
                && other["fleetId"] == id
                && other["provider"]["agentMxid"] == mxid
                && ["active", "pending"].contains(&s(&other["state"]))
        }) {
            return Err(err(
                409,
                "agent_still_allocated",
                "Another allocation still uses this Agent.",
            ));
        }
        self.palpo.require_admin(token).await?;
        let user = self
            .palpo
            .user(mxid, token)
            .await?
            .ok_or_else(|| err(409, "identity_conflict", "Agent identity is missing."))?;
        if user["appservice_id"] != id || user["admin"] == true {
            return Err(err(
                409,
                "identity_conflict",
                "Agent must belong to this App Service.",
            ));
        }
        let aid = f["agents"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, a)| a["mxid"] == mxid)
            .map(|(k, _)| k.clone())
            .unwrap_or(aid.to_string());
        if f["agents"].get(&aid).is_none() {
            f["agents"][&aid] = json!({
            "id":aid,"fleetId":id,"mxid":mxid,"displayName":user["displayname"],"role":r["payload"]["role"],"approvedRequestId":r["requestId"],"authorization":"verified_fleet_request","createdAt":now(),"state":"retiring"}
            );
        }
        for a in f["agents"].as_object_mut().unwrap().values_mut() {
            if a["mxid"] == mxid {
                a["state"] = json!("retiring");
            }
        }
        if r["retirement"].is_null() {
            r["retirement"] = json!({"state":"pending","mxid":mxid,"endedAt":input["endedAt"]});
        }
        r["state"] = json!("ended");
        r["provider"]["state"] = json!("ended");
        r["provider"]["ready"] = json!(false);
        r["provider"]["bound"] = json!(false);
        r["provider"]["endedAt"] = r["retirement"]["endedAt"].clone();
        r["usable"] = json!(false);
        self.store
            .change(|st| {
                st["fleets"][id] = f;
                st["requests"][&rid] = r.clone();
                Ok(())
            })
            .await?;
        let result = self
            .agent_retire(id, &aid, &format!("fleet:{id}"), token)
            .await;
        self.store
            .change(|st| {
                st["requests"][&rid]["retirement"]["state"] =
                    json!(if result.is_ok() { "complete" } else { "failed" });
                if let Ok(a) = &result {
                    for record in st["fleets"][id]["agents"]
                        .as_object_mut()
                        .unwrap()
                        .values_mut()
                    {
                        if record["mxid"] == mxid {
                            record["state"] = json!("retired");
                            if record["retiredAt"].is_null() {
                                record["retiredAt"] = a["retiredAt"].clone();
                            }
                            record["localTaskStop"] = json!(if input["localStopped"] == true {
                                "confirmed"
                            } else {
                                "unconfirmed"
                            });
                            record["lastError"] = Value::Null;
                        }
                    }
                }
                Ok(())
            })
            .await?;
        let mut a = result?;
        a["localTaskStop"] = json!(if input["localStopped"] == true {
            "confirmed"
        } else {
            "unconfirmed"
        });
        a["appserviceAccess"] = json!("revoked");
        Ok(json!({"ok":true,"fleetId":id,"requestId":r["requestId"],"agent":a}))
    }
}
