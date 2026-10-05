//! Matrix notifications are projections of durable Inbox actions. Messages carry
//! an opaque reference only; they cannot approve an action or contain credentials.
use std::sync::Arc;

use hagency_contract::MatrixUserId;
use reqwest::Method;
use serde_json::{Value, json};

use crate::api::App;
use crate::workflow::Workflows;
use crate::{Result, digest, fail, now_ms};

const INTERVALS: [u64; 3] = [3_600_000, 86_400_000, 172_800_000];

pub struct Notifications {
    app: Arc<App>,
    bot: MatrixUserId,
    token: String,
    origin: String,
}
impl Notifications {
    pub fn new(app: Arc<App>, bot: String, token: String, origin: String) -> Result<Self> {
        let bot: MatrixUserId = bot.try_into()?;
        if !bot.belongs_to(app.matrix.server()) || token.trim().is_empty() {
            return Err(fail(400, "invalid_notification_identity"));
        }
        let origin_url =
            reqwest::Url::parse(&origin).map_err(|_| fail(400, "invalid_public_origin"))?;
        if origin_url.origin().ascii_serialization() != app.public_origin {
            return Err(fail(400, "invalid_public_origin"));
        }
        app.notifications_enabled
            .store(true, std::sync::atomic::Ordering::Release);
        Ok(Self {
            app,
            bot,
            token,
            origin: origin_url.origin().ascii_serialization(),
        })
    }
    async fn room(&self, recipient: &str) -> Result<String> {
        let expected = json!({"v":1,"purpose":"my_actions","ownerMxid":recipient,"botMxid":self.bot,"serverName":self.app.matrix.server()});
        let key = digest(&json!(recipient))?;
        let state = self.app.store.lock().await.read().await?;
        let saved = &state["actionRooms"][&key];
        if !saved.is_null() && saved["binding"] != expected {
            return Err(fail(409, "action_room_binding_changed"));
        }
        let room = if let Some(room) = saved["roomId"].as_str() {
            room.to_owned()
        } else {
            // The deterministic alias recovers a lost createRoom response.
            let alias_local = format!("hagency_actions_{}", &digest(&expected)?[..24]);
            let alias = format!("#{alias_local}:{}", self.app.matrix.server().as_str());
            let resolve = self
                .app
                .matrix
                .call(
                    Method::GET,
                    &format!("/_matrix/client/v3/directory/room/{}", enc(&alias)),
                    &self.token,
                    None,
                )
                .await;
            let result = match resolve {
                Ok(room) => room,
                Err(e) if e.status == 404 => self.app.matrix.call(Method::POST, "/_matrix/client/v3/createRoom", &self.token,
                    Some(&json!({"name":"My Actions","room_alias_name":alias_local,"visibility":"private","preset":"private_chat",
                        "creation_content":{"m.federate":false},"invite":[recipient],
                        "power_level_content_override":{"users":{self.bot.as_str():100},"users_default":0,"invite":100,"state_default":100,"events_default":0},
                        "initial_state":[{"type":"im.hagency.actions.v1","state_key":"","content":expected},
                            {"type":"m.room.history_visibility","state_key":"","content":{"history_visibility":"invited"}}]}))).await?,
                Err(e) => return Err(e),
            };
            result["room_id"]
                .as_str()
                .filter(|r| r.starts_with('!'))
                .ok_or_else(|| fail(502, "invalid_action_room"))?
                .to_owned()
        };
        let events = self
            .app
            .matrix
            .call(
                Method::GET,
                &format!("/_matrix/client/v3/rooms/{}/state", enc(&room)),
                &self.token,
                None,
            )
            .await?;
        private_room(&events, &expected, recipient, self.bot.as_str())?;
        self.app
            .store
            .lock()
            .await
            .transaction(|state| {
                if state["actionRooms"].is_null() {
                    state["actionRooms"] = json!({});
                }
                if !state["actionRooms"].is_object() {
                    return Err(fail(503, "workflow_state_invalid"));
                }
                if !state["actionRooms"][&key].is_null()
                    && state["actionRooms"][&key]["binding"] != expected
                {
                    return Err(fail(409, "action_room_binding_changed"));
                }
                state["actionRooms"][&key] = json!({"roomId":room,"binding":expected});
                Ok(())
            })
            .await?;
        Ok(room)
    }
    pub async fn tick(&self) -> Result<()> {
        let me = self
            .app
            .matrix
            .call(
                Method::GET,
                "/_matrix/client/v3/account/whoami",
                &self.token,
                None,
            )
            .await?;
        if me["user_id"] != self.bot.as_str() || me["is_guest"] == true {
            return Err(fail(403, "notification_identity_changed"));
        }
        let state = self.app.store.lock().await.read().await?;
        let workflows = Workflows::load(&state)?;
        let due: Vec<_> = workflows
            .notices
            .values()
            .filter(|n| {
                n["cancelled"] != true
                    && n["finished"] != true
                    && n["dueAt"].as_u64().is_some_and(|t| t <= now_ms())
            })
            .take(20)
            .cloned()
            .collect();
        for notice in due {
            let result = self.deliver(&notice).await;
            if let Err(error) = result {
                self.app
                    .store
                    .lock()
                    .await
                    .transaction(|state| {
                        let mut w = Workflows::load(state)?;
                        let Some(n) = w.notices.get_mut(notice["id"].as_str().unwrap_or_default())
                        else {
                            return Ok(());
                        };
                        // Do not overwrite a new revision's schedule or a snooze.
                        if n != &notice {
                            return Ok(());
                        }
                        let attempts = n["attempt"].as_u64().unwrap_or(0).saturating_add(1);
                        n["attempt"] = json!(attempts);
                        n["lastError"] = json!(error.code);
                        n["dueAt"] = json!(
                            now_ms().saturating_add(
                                1000_u64
                                    .saturating_mul(1_u64 << attempts.min(10))
                                    .min(3_600_000)
                            )
                        );
                        w.save(state)
                    })
                    .await?;
            }
        }
        Ok(())
    }
    async fn deliver(&self, notice: &Value) -> Result<()> {
        // Keep decisions, imports and observation updates from changing this
        // action between the final authority check and its notification send.
        let _writer = self.app.mutation.lock().await;
        let id = notice["id"]
            .as_str()
            .ok_or_else(|| fail(503, "workflow_state_invalid"))?;
        let action_id = notice["actionId"]
            .as_str()
            .ok_or_else(|| fail(503, "workflow_state_invalid"))?;
        let recipient: MatrixUserId = notice["recipient"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
            .try_into()?;
        let current = self.app.store.lock().await.read().await?;
        let w = Workflows::load(&current)?;
        if w.notices.get(id) != Some(notice) {
            return Ok(());
        }
        let visible = w.view(action_id, &recipient, now_ms());
        let pending = visible.as_ref().is_ok_and(|a| a["needsMyAction"] == true);
        if !visible
            .as_ref()
            .is_ok_and(|a| a["revision"] == notice["revision"])
            || notice["delivered"].as_u64().unwrap_or(0) > 0
                && (!pending || !notice["seenAt"].is_null())
        {
            return self
                .app
                .store
                .lock()
                .await
                .transaction(|state| {
                    let mut w = Workflows::load(state)?;
                    if let Some(n) = w.notices.get_mut(id) {
                        n["cancelled"] = json!(true);
                    }
                    w.save(state)
                })
                .await;
        }
        let room = self.room(recipient.as_str()).await?;
        let delivered = notice["delivered"].as_u64().unwrap_or(0);
        let label = if pending {
            "A Hagency action needs your attention."
        } else {
            "A Hagency action has an update."
        };
        let link = format!("{}/hagency/inbox?action={}", self.origin, enc(action_id));
        self.app.matrix.call(Method::PUT,
            &format!("/_matrix/client/v3/rooms/{}/send/m.room.message/{}",enc(&room),enc(&format!("{id}_{delivered}"))), &self.token,
            Some(&json!({"msgtype":"m.notice","body":format!("{label}\n{link}"),
                "im.hagency.action.v1":{"v":1,"id":action_id,"revision":notice["revision"],"needsAction":pending}}))).await?;
        self.app
            .store
            .lock()
            .await
            .transaction(|state| {
                let mut w = Workflows::load(state)?;
                if let Some(n) = w.notices.get_mut(id)
                    && n == notice
                {
                    n["delivered"] = json!(delivered + 1);
                    n["attempt"] = json!(0);
                    n["lastError"] = Value::Null;
                    n["sentAt"] = json!(now_ms());
                    if pending && delivered < INTERVALS.len() as u64 && n["seenAt"].is_null() {
                        n["dueAt"] = json!(now_ms().saturating_add(INTERVALS[delivered as usize]));
                    } else {
                        n["finished"] = json!(true);
                    }
                }
                w.save(state)
            })
            .await
    }
}
fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>()
}
fn private_room(events: &Value, expected: &Value, recipient: &str, bot: &str) -> Result<()> {
    let events = events
        .as_array()
        .ok_or_else(|| fail(409, "action_room_not_private"))?;
    let content = |typ: &str, key: &str| {
        events
            .iter()
            .find(|e| e["type"] == typ && e["state_key"] == key)
            .map(|e| e["content"].clone())
            .unwrap_or(Value::Null)
    };
    let powers = content("m.room.power_levels", "");
    if content("im.hagency.actions.v1", "") != *expected
        || content("m.room.join_rules", "")["join_rule"] != "invite"
        || content("m.room.history_visibility", "")["history_visibility"] != "invited"
        || !content("m.room.encryption", "").is_null()
        || content("m.room.create", "")["m.federate"] != false
        || powers["invite"] != 100
        || powers["state_default"] != 100
        || powers["users"][bot] != 100
        || powers["users_default"].as_i64().unwrap_or(0) != 0
        || powers["users"]
            .as_object()
            .is_none_or(|u| u.iter().any(|(id, p)| id != bot && p != &json!(0)))
        || [
            "m.room.power_levels",
            "m.room.join_rules",
            "m.room.history_visibility",
            "m.room.encryption",
            "im.hagency.actions.v1",
        ]
        .iter()
        .any(|typ| powers["events"][*typ].as_i64().unwrap_or(100) != 100)
        || content("m.room.member", bot)["membership"] != "join"
        || !matches!(
            content("m.room.member", recipient)["membership"].as_str(),
            Some("invite" | "join")
        )
        || events.iter().any(|e| {
            e["type"] == "m.room.member"
                && matches!(e["content"]["membership"].as_str(), Some("invite" | "join"))
                && e["state_key"] != bot
                && e["state_key"] != recipient
        })
    {
        return Err(fail(409, "action_room_not_private"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_room_rejects_federation_extra_members_and_changed_permissions() {
        let expected = json!({"purpose":"my_actions"});
        let mut events = json!([
            {"type":"im.hagency.actions.v1","state_key":"","content":expected},
            {"type":"m.room.join_rules","state_key":"","content":{"join_rule":"invite"}},
            {"type":"m.room.history_visibility","state_key":"","content":{"history_visibility":"invited"}},
            {"type":"m.room.create","state_key":"","content":{"m.federate":false}},
            {"type":"m.room.power_levels","state_key":"","content":{"invite":100,"state_default":100,"users":{"@bot:test":100}}},
            {"type":"m.room.member","state_key":"@bot:test","content":{"membership":"join"}},
            {"type":"m.room.member","state_key":"@user:test","content":{"membership":"invite"}}
        ]);
        assert!(private_room(&events, &expected, "@user:test", "@bot:test").is_ok());
        events[3]["content"]["m.federate"] = json!(true);
        assert!(private_room(&events, &expected, "@user:test", "@bot:test").is_err());
        events[3]["content"]["m.federate"] = json!(false);
        events[4]["content"]["invite"] = json!(0);
        assert!(private_room(&events, &expected, "@user:test", "@bot:test").is_err());
        events[4]["content"]["invite"] = json!(100);
        events.as_array_mut().unwrap().push(json!({"type":"m.room.member","state_key":"@other:test","content":{"membership":"join"}}));
        assert!(private_room(&events, &expected, "@user:test", "@bot:test").is_err());
    }
}
