//! Native port of web-admin/lib/outbound.mjs's durable leased delivery lane.
//! Custody acknowledgement never marks an agent ready or refunds a reservation.
use chrono::{DateTime, SecondsFormat};
use hagency_contract::canonical::{encode_transport, transport_digest};
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

use crate::{Result, digest, fail, secret};

#[derive(Clone, Copy)]
pub struct Limits {
    pub lease_ms: u64,
    pub records: u64,
    pub pending: u64,
    pub bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            lease_ms: 30000,
            records: 10000,
            pending: 1000,
            bytes: 16 * 1024 * 1024,
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 255 && !id.chars().any(char::is_control)
}
fn lane(value: &str) -> bool {
    value == "matrix" || value == "work"
}
fn same_secret(left: &str, right: &str) -> Result<bool> {
    // Hashes have fixed length, including when credentials have different sizes.
    let a = digest(&json!(left))?;
    let b = digest(&json!(right))?;
    Ok(bool::from(a.as_bytes().ct_eq(b.as_bytes())))
}
pub fn authenticate(
    state: &Value,
    id: &str,
    token: &str,
    generation: Option<u64>,
    relay: bool,
) -> Result<Value> {
    let fleet = &state["fleets"][id];
    let expected = if relay {
        fleet.pointer("/registration/hs_token")
    } else {
        fleet.pointer("/transport/token")
    };
    if token.is_empty()
        || token.len() > 8192
        || fleet["id"] != id
        || fleet.pointer("/transport/mode") != Some(&json!("outbound"))
        || !matches!(
            fleet["state"].as_str(),
            Some("pending_connection" | "ready")
        )
        || fleet["installation"] != "installed"
        || !same_secret(expected.and_then(Value::as_str).unwrap_or_default(), token)?
    {
        return Err(fail(401, "transport_unauthorized"));
    }
    if !relay
        && (generation.is_none()
            || generation == Some(0)
            || generation
                != fleet
                    .pointer("/transport/generation")
                    .and_then(Value::as_u64))
    {
        return Err(fail(409, "generation_conflict"));
    }
    Ok(fleet.clone())
}

/// Delivery rows live in the same PostgreSQL JSONB commit as workflows.
pub fn enqueue(
    state: &mut Value,
    fleet: &Value,
    lane_name: &str,
    kind: &str,
    id: &str,
    payload: &Value,
    limits: Limits,
) -> Result<()> {
    if !valid_id(id)
        || !lane(lane_name)
        || !payload.is_object()
        || (lane_name == "matrix" && kind != "transaction")
        || (lane_name == "work" && !matches!(kind, "request" | "probe"))
    {
        return Err(fail(400, "invalid_delivery"));
    }
    let fid = fleet["id"]
        .as_str()
        .ok_or_else(|| fail(400, "invalid_fleet"))?;
    let generation = fleet["transport"]["generation"]
        .as_u64()
        .ok_or_else(|| fail(400, "invalid_generation"))?;
    let fingerprint = transport_digest(payload)?;
    let rows = state["deliveries"]
        .as_array_mut()
        .ok_or_else(|| fail(503, "workflow_state_invalid"))?;
    if let Some(prior) = rows.iter().rev().find(|r| {
        r["fleet"] == fid
            && r["lane"] == lane_name
            && r["id"] == id
            && (lane_name == "matrix" || r["generation"] == generation)
    }) {
        return if prior["digest"] == fingerprint {
            Ok(())
        } else {
            Err(fail(409, "delivery_conflict"))
        };
    }
    let raw = encode_transport(payload)?;
    if raw.len() > 4 * 1024 * 1024 {
        return Err(fail(400, "delivery_too_large"));
    }
    let fleet_rows: Vec<_> = rows.iter().filter(|r| r["fleet"] == fid).collect();
    let pending: Vec<_> = fleet_rows.iter().filter(|r| r["acked"].is_null()).collect();
    let bytes: u64 = pending
        .iter()
        .map(|r| r["bytes"].as_u64().unwrap_or(0))
        .sum();
    if fleet_rows.len() as u64 >= limits.records
        || pending.len() as u64 >= limits.pending
        || bytes.saturating_add(raw.len() as u64) > limits.bytes
    {
        return Err(fail(503, "queue_full"));
    }
    rows.push(json!({"fleet":fid,"generation":generation,"lane":lane_name,"id":id,"kind":kind,"digest":fingerprint,
        "payload":payload,"bytes":raw.len(),"consumer":null,"token":null,"expires":null,"acked":null}));
    Ok(())
}

pub fn claim(
    state: &mut Value,
    fleet: &Value,
    lane_name: &str,
    consumer: &str,
    now: u64,
    limits: Limits,
) -> Result<Value> {
    let valid_consumer = consumer.len() == 36
        && consumer.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        });
    if !lane(lane_name) || !valid_consumer {
        return Err(fail(400, "invalid_poll"));
    }
    let generation = fleet["transport"]["generation"]
        .as_u64()
        .ok_or_else(|| fail(400, "invalid_generation"))?;
    let rows = state["deliveries"]
        .as_array_mut()
        .ok_or_else(|| fail(503, "workflow_state_invalid"))?;
    let Some(row) = rows
        .iter_mut()
        .find(|r| {
            r["fleet"] == fleet["id"]
                && r["generation"] == generation
                && r["lane"] == lane_name
                && r["acked"].is_null()
        })
        .filter(|r| r["expires"].as_u64().is_none_or(|n| n <= now))
    else {
        return Ok(json!({"v":2,"generation":generation,"delivery":null}));
    };
    let until = now
        .checked_add(limits.lease_ms)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or_else(|| fail(400, "invalid_clock"))?;
    let expires = DateTime::from_timestamp_millis(until)
        .ok_or_else(|| fail(400, "invalid_clock"))?
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    row["consumer"] = json!(consumer);
    row["token"] = json!(secret());
    row["expires"] = json!(until);
    Ok(
        json!({"v":2,"generation":generation,"delivery":{"id":row["id"],"lane":lane_name,"token":row["token"],
        "expiresAt":expires,"kind":row["kind"],"payload":row["payload"]}}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ack {
    pub lane: String,
    pub id: String,
    pub token: String,
}
pub fn ack(state: &mut Value, fleet: &Value, input: &Ack, now: u64) -> Result<Value> {
    let row = state["deliveries"]
        .as_array_mut()
        .ok_or_else(|| fail(503, "workflow_state_invalid"))?
        .iter_mut()
        .find(|r| {
            r["fleet"] == fleet["id"]
                && r["generation"] == fleet["transport"]["generation"]
                && r["lane"] == input.lane
                && r["id"] == input.id
        })
        .ok_or_else(|| fail(409, "stale_lease"))?;
    if input.token.is_empty()
        || !same_secret(row["token"].as_str().unwrap_or_default(), &input.token)?
        || row["acked"].is_null() && row["expires"].as_u64().is_none_or(|n| n <= now)
    {
        return Err(fail(409, "stale_lease"));
    }
    if row["acked"].is_null() {
        row["acked"] = json!(now);
        row["payload"] = Value::Null;
    }
    Ok(json!({"ok":true}))
}

/// Matrix event deduplication and connection evidence are committed atomically.
pub fn relay(
    state: &mut Value,
    fleet: &mut Value,
    id: &str,
    body: Value,
    limits: Limits,
) -> Result<()> {
    let events = body["events"]
        .as_array()
        .filter(|e| e.len() <= 1000 && e.iter().all(Value::is_object))
        .ok_or_else(|| fail(400, "invalid_transaction"))?;
    enqueue(
        state,
        fleet,
        "matrix",
        "transaction",
        id,
        &json!({"transactionId":id,"body":body}),
        limits,
    )?;
    for event in events {
        let probe = &fleet["probe"];
        if probe.is_object()
            && event["type"] == "com.hagency.connection.probe.v1"
            && event["room_id"] == probe["roomId"]
            && event["sender"] == fleet["representativeMxid"]
            && event["content"]["fleetId"] == fleet["id"]
            && event["content"]["challenge"] == probe["challenge"]
            && event["event_id"].as_str().is_some_and(valid_id)
            && (probe["eventId"].is_null() || probe["eventId"] == event["event_id"])
        {
            fleet["probe"]["matrixTransactionId"] = json!(id);
            fleet["probe"]["matrixEventId"] = event["event_id"].clone();
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    fn fleet() -> Value {
        json!({"id":"hf_test","installation":"installed","state":"ready","transport":{"mode":"outbound","generation":1,"token":"fixture-machine"},"registration":{"hs_token":"fixture-relay"}})
    }
    #[tokio::test]
    async fn custody_replays_conflicts_expiry_and_ack_never_change_workflow_readiness() {
        let store = Store::memory().unwrap();
        let f = fleet();
        let l = Limits::default();
        let consumer = "01234567-0123-0123-0123-0123456789ab";
        let lease = store
            .transaction(|state| {
                state["agentState"] = json!("pending");
                enqueue(
                    state,
                    &f,
                    "work",
                    "request",
                    "request1",
                    &json!({"agent":"a"}),
                    l,
                )?;
                enqueue(
                    state,
                    &f,
                    "work",
                    "request",
                    "request1",
                    &json!({"agent":"a"}),
                    l,
                )?;
                claim(state, &f, "work", consumer, 1000, l)
            })
            .await
            .unwrap();
        assert!(
            store
                .transaction(|state| enqueue(
                    state,
                    &f,
                    "work",
                    "request",
                    "request1",
                    &json!({"agent":"b"}),
                    l
                ))
                .await
                .is_err()
        );
        assert!(
            store
                .transaction(|state| claim(state, &f, "work", consumer, 1001, l))
                .await
                .unwrap()["delivery"]
                .is_null()
        );
        let old = Ack {
            lane: "work".into(),
            id: "request1".into(),
            token: lease["delivery"]["token"].as_str().unwrap().into(),
        };
        assert!(
            store
                .transaction(|state| ack(state, &f, &old, 31000))
                .await
                .is_err()
        );
        let replacement = store
            .transaction(|state| claim(state, &f, "work", consumer, 31000, l))
            .await
            .unwrap();
        assert!(
            store
                .transaction(|state| ack(state, &f, &old, 31001))
                .await
                .is_err()
        );
        let current = Ack {
            token: replacement["delivery"]["token"].as_str().unwrap().into(),
            ..old
        };
        store
            .transaction(|state| ack(state, &f, &current, 31001))
            .await
            .unwrap();
        store
            .transaction(|state| ack(state, &f, &current, 99000))
            .await
            .unwrap();
        assert_eq!(store.read().await.unwrap()["agentState"], "pending");
        assert!(
            store
                .transaction(|state| claim(state, &f, "work", consumer, 99000, l))
                .await
                .unwrap()["delivery"]
                .is_null()
        );
    }
    #[tokio::test]
    async fn authentication_generation_and_relay_credentials_are_distinct() {
        let mut state = json!({"fleets":{"hf_test":fleet()}});
        assert!(authenticate(&state, "hf_test", "fixture-machine", Some(1), false).is_ok());
        assert!(authenticate(&state, "hf_test", "fixture-relay", Some(1), false).is_err());
        assert!(authenticate(&state, "hf_test", "fixture-machine", None, true).is_err());
        assert!(authenticate(&state, "hf_test", "fixture-machine", Some(2), false).is_err());
        state["fleets"]["hf_test"]["state"] = json!("revoked");
        assert!(authenticate(&state, "hf_test", "fixture-machine", Some(1), false).is_err());
    }
    #[tokio::test]
    async fn matrix_dedup_survives_rotation_and_failed_write_rolls_back_both_tables() {
        let store = Store::memory().unwrap();
        let l = Limits::default();
        let mut f = fleet();
        let body = json!({"events":[]});
        store
            .transaction(|state| {
                relay(state, &mut f, "txn", body.clone(), l)?;
                state["value"] = json!(1);
                Ok(())
            })
            .await
            .unwrap();
        f["transport"]["generation"] = json!(2);
        store
            .transaction(|state| relay(state, &mut f, "txn", body.clone(), l))
            .await
            .unwrap();
        assert!(
            store
                .transaction::<()>(|state| {
                    relay(state, &mut f, "txn2", body.clone(), l)?;
                    state["value"] = json!(2);
                    Err(fail(409, "fixture_failure"))
                })
                .await
                .is_err()
        );
        store
            .transaction(|state| {
                assert_eq!(state["value"], 1);
                assert_eq!(state["deliveries"].as_array().unwrap().len(), 1);
                Ok(())
            })
            .await
            .unwrap();
    }
}
