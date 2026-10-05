use std::{collections::BTreeMap, sync::Arc};

use hagency_operations::{
    api::App, matrix::Matrix, notifications::Notifications, now_ms, store::Store,
    workflow::Workflows,
};
use salvo::{conn::Acceptor, prelude::*};
use serde_json::{Value, json};
use tokio::sync::Mutex;

#[derive(Default)]
struct MatrixState {
    aliases: BTreeMap<String, String>,
    rooms: BTreeMap<String, Value>,
    events: BTreeMap<String, Value>,
    lose_reply: bool,
}
#[handler]
async fn notification_matrix(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let state = depot.get_typed::<Arc<Mutex<MatrixState>>>().unwrap();
    let mut state = state.lock().await;
    let path = req.uri().path().to_owned();
    if req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        != Some("Bearer notification-fixture")
    {
        res.status_code(StatusCode::UNAUTHORIZED);
        res.render(Json(json!({})));
        return;
    }
    if path.ends_with("whoami") {
        res.render(Json(json!({"user_id":"@notifier:example.test"})));
        return;
    }
    if path.contains("/directory/room/") {
        let alias = path.split("/directory/room/").nth(1).unwrap();
        let alias = url::form_urlencoded::parse(format!("v={alias}").as_bytes())
            .next()
            .unwrap()
            .1
            .to_string();
        if let Some(room) = state.aliases.get(&alias) {
            res.render(Json(json!({"room_id":room})));
        } else {
            res.status_code(StatusCode::NOT_FOUND);
            res.render(Json(json!({})));
        }
        return;
    }
    if path.ends_with("createRoom") {
        let body = req.parse_json::<Value>().await.unwrap();
        let room = format!("!room{}:example.test", state.rooms.len());
        let mut events = body["initial_state"].as_array().unwrap().clone();
        events.extend([
            json!({"type":"m.room.create","state_key":"","content":body["creation_content"]}),
            json!({"type":"m.room.power_levels","state_key":"","content":body["power_level_content_override"]}),
            json!({"type":"m.room.join_rules","state_key":"","content":{"join_rule":"invite"}}),
            json!({"type":"m.room.member","state_key":"@notifier:example.test","content":{"membership":"join"}}),
            json!({"type":"m.room.member","state_key":body["invite"][0],"content":{"membership":"invite"}}),
        ]);
        state.aliases.insert(
            format!(
                "#{}:example.test",
                body["room_alias_name"].as_str().unwrap()
            ),
            room.clone(),
        );
        state.rooms.insert(room.clone(), json!(events));
        res.render(Json(json!({"room_id":room})));
        return;
    }
    if path.ends_with("/state") {
        let room = path
            .split("/rooms/")
            .nth(1)
            .unwrap()
            .trim_end_matches("/state");
        let room = url::form_urlencoded::parse(format!("v={room}").as_bytes())
            .next()
            .unwrap()
            .1
            .to_string();
        res.render(Json(state.rooms[&room].clone()));
        return;
    }
    if path.contains("/send/m.room.message/") {
        let body = req.parse_json::<Value>().await.unwrap();
        state.events.entry(path).or_insert(body);
        if state.lose_reply {
            state.lose_reply = false;
            res.status_code(StatusCode::SERVICE_UNAVAILABLE);
        }
        res.render(Json(json!({"event_id":"$notice"})));
        return;
    }
    panic!("unexpected Matrix route {path}");
}
#[tokio::test]
async fn lost_reply_reuses_transaction_and_seen_snooze_stop_reminders() {
    let matrix_state = Arc::new(Mutex::new(MatrixState::default()));
    let acceptor = TcpListener::new("127.0.0.1:0").bind().await;
    let addr = acceptor.holdings()[0]
        .local_addr
        .clone()
        .into_std()
        .unwrap();
    let router = Router::new()
        .hoop(affix_state::inject(matrix_state.clone()))
        .push(Router::with_path("{**path}").goal(notification_matrix));
    let server = tokio::spawn(async move {
        Server::new(acceptor).serve(router).await;
    });
    let matrix = Matrix::new(
        &format!("http://{addr}"),
        "example.test".to_owned().try_into().unwrap(),
    )
    .unwrap();
    let store = Store::memory().unwrap();
    let now = now_ms();
    let action=store.transaction(|state| {
        let mut w=Workflows { authority:serde_json::from_value(json!({"engagements":{"engagement":{"id":"engagement","server":"example.test","owner":"@provider:example.test","coordinator":"@coordinator:example.test","registrationGeneration":1,"delegationRevision":1,"delegationExpiresAtMs":now+3600000,"state":"verified","allowSelfApproval":false,"coordinatorApprovalV1":true}},"resources":{"resource":{"id":"resource","serverEngagementId":"engagement","revision":1,"allocatedTokens":100000,"eligibleManagers":["@manager:example.test"]}},"projects":{}}))?, ..Default::default() };
        let action=w.submit(serde_json::from_value(json!({"kind":"project","request":{"id":"request","revision":1,"serverEngagementId":"engagement","projectId":"project","owner":"@manager:example.test","requester":"@manager:example.test","definitionDigest":"a".repeat(64),"resourceAllocations":["resource"]}}))?,&"@manager:example.test".to_owned().try_into()?,now)?;
        w.save(state)?;Ok(action["id"].as_str().unwrap().to_owned())
    }).await.unwrap();
    let app = App::new(matrix, store.clone(), "https://operations.test", 900000)
        .await
        .unwrap();
    let notifications = Notifications::new(
        app,
        "@notifier:example.test".into(),
        "notification-fixture".into(),
        "https://operations.test".into(),
    )
    .unwrap();
    matrix_state.lock().await.lose_reply = true;
    notifications.tick().await.unwrap();
    assert_eq!(matrix_state.lock().await.events.len(), 2);
    assert_eq!(
        Workflows::load(&store.read().await.unwrap())
            .unwrap()
            .notices
            .values()
            .map(|n| n["delivered"].as_u64().unwrap())
            .sum::<u64>(),
        1
    );
    store
        .transaction(|state| {
            let mut w = Workflows::load(state)?;
            for n in w.notices.values_mut().filter(|n| n["delivered"] == 0) {
                n["dueAt"] = json!(0);
            }
            w.save(state)
        })
        .await
        .unwrap();
    notifications.tick().await.unwrap();
    assert_eq!(
        matrix_state.lock().await.events.len(),
        2,
        "lost send response must reuse the Matrix transaction ID"
    );
    let messages = serde_json::to_string(&matrix_state.lock().await.events).unwrap();
    assert!(!messages.contains("notification-fixture"));
    assert!(!messages.contains("resourceAllocations"));
    assert!(messages.contains("/hagency/inbox?action="));
    store
        .transaction(|state| {
            let mut w = Workflows::load(state)?;
            let actor = "@coordinator:example.test".to_owned().try_into()?;
            w.snooze(&action, 60, &actor, now_ms())?;
            for n in w
                .notices
                .values()
                .filter(|n| n["recipient"] == "@coordinator:example.test")
            {
                assert!(n["dueAt"].as_u64().unwrap() > now_ms());
            }
            w.seen(&action, &actor, now_ms())?;
            for n in w.notices.values_mut() {
                n["dueAt"] = json!(0);
            }
            w.save(state)
        })
        .await
        .unwrap();
    notifications.tick().await.unwrap();
    assert_eq!(
        matrix_state.lock().await.events.len(),
        2,
        "seen only suppresses reminders; it does not approve the action"
    );
    let w = Workflows::load(&store.read().await.unwrap()).unwrap();
    assert_eq!(w.actions[&action].state, "requested");
    assert!(
        w.notices
            .values()
            .all(|n| n["cancelled"] == true || n["finished"] == true)
    );
    server.abort();
}
