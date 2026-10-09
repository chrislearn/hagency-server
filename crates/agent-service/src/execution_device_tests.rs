use super::*;
#[tokio::test]
#[ignore = "requires dedicated PostgreSQL database via HAGENCY_AGENT_TEST_DATABASE_URL"]
async fn postgres_agent_execution_device_is_current_owned_cas_and_blocks_other_device_takeover() {
    fn copy_reply(input: &SubmitReply) -> SubmitReply {
        SubmitReply {
            dispatch_id: input.dispatch_id.clone(),
            execution_id: input.execution_id.clone(),
            body: input.body.clone(),
        }
    }
    let f = fixture().await;
    let stranger = fixture().await;
    let now = crate::api::now_ms();
    let original = f.domain.agent(&f.p, &f.agent, now).await.unwrap();
    assert_eq!(
        original.execution_device_id.as_deref().unwrap(),
        f.p.device_id.as_deref().unwrap()
    );
    crate::assert_entity_id(&original.id, "agt_");
    assert!(
        f.transport
            .acquire_for_test(&f.second, &f.agent, 60000, true, now)
            .await
            .is_err(),
        "same owner takeover is not a device assignment"
    );
    assert!(f.domain.agent(&stranger.p, &f.agent, now).await.is_err());
    assert!(
        f.domain
            .set_execution_device(
                &stranger.p,
                &f.agent,
                crate::domain::SetExecutionDevice {
                    expected_generation: original.generation
                },
                now
            )
            .await
            .is_err()
    );
    let devices = f.domain.owner_devices(&f.p, now).await.unwrap();
    assert_eq!(devices.len(), 2);
    assert!(
        devices
            .iter()
            .all(|d| d.id != stranger.p.device_id.as_deref().unwrap())
    );
    let old = f
        .transport
        .acquire_for_test(&f.p, &f.agent, 60000, false, now)
        .await
        .unwrap();
    let old_ref = LeaseRef {
        agent_id: f.agent.clone(),
        epoch: old.epoch,
    };
    let unchanged = f
        .domain
        .set_execution_device(
            &f.p,
            &f.agent,
            crate::domain::SetExecutionDevice {
                expected_generation: original.generation,
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(unchanged.generation, original.generation);
    assert_eq!(unchanged.execution_device_id, original.execution_device_id);
    let renewed = f
        .transport
        .renew_lease(&f.p, &old_ref, 60000, now)
        .await
        .unwrap();
    assert_eq!(
        renewed.epoch, old.epoch,
        "same-device save must not invalidate lease"
    );
    let event = enqueue(&f, "instance-old-running").await;
    running(&f, &old_ref, &event, "instance-running-execution").await;
    assert!(
        f.domain
            .set_execution_device(
                &f.p,
                &f.agent,
                crate::domain::SetExecutionDevice {
                    expected_generation: 0
                },
                now
            )
            .await
            .is_err()
    );
    // Preserve each durable delivery state under the old designated device.
    let mut old_replies = Vec::new();
    for state in ["pending", "unknown", "sending"] {
        let dispatch = enqueue(&f, &format!("instance-old-{state}")).await;
        let execution = format!("instance-{state}-execution");
        running(&f, &old_ref, &dispatch, &execution).await;
        let request = SubmitReply {
            dispatch_id: dispatch,
            execution_id: execution,
            body: format!("known old {state} output"),
        };
        let mut reply = f
            .transport
            .submit_reply(&f.p, &old_ref, copy_reply(&request), &f.facts, now)
            .await
            .unwrap();
        if state != "pending" {
            reply = f
                .transport
                .claim_reply(&reply.id, &f.facts, now)
                .await
                .unwrap();
            if state == "unknown" {
                reply = f
                    .transport
                    .confirm_reply(&reply.id, reply.worker_token.as_deref().unwrap(), None, now)
                    .await
                    .unwrap();
            }
        }
        assert_eq!(reply.state, state);
        old_replies.push((reply, request));
    }
    let next = f
        .domain
        .set_execution_device(
            &f.second,
            &f.agent,
            crate::domain::SetExecutionDevice {
                expected_generation: original.generation,
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(next.id, original.id);
    assert_eq!(next.generation, original.generation + 1);
    assert!(
        f.transport
            .renew_lease(&f.p, &old_ref, 60000, now)
            .await
            .is_err()
    );
    assert!(
        f.transport
            .acquire_for_test(&f.p, &f.agent, 60000, true, now)
            .await
            .is_err()
    );
    for result in [
        f.transport
            .acknowledge(&f.p, &old_ref, &event, now)
            .await
            .map(|_| ()),
        f.transport
            .authorize_tool_execution(
                &f.p,
                &old_ref,
                &event,
                "instance-running-execution",
                &f.facts,
                now,
            )
            .await
            .map(|_| ()),
        f.transport
            .finish_without_reply(
                &f.p,
                &old_ref,
                &event,
                "instance-running-execution",
                "unknown",
                now,
            )
            .await
            .map(|_| ()),
        f.transport
            .submit_reply(&f.p, &old_ref, copy_reply(&old_replies[0].1), &f.facts, now)
            .await
            .map(|_| ()),
        f.transport
            .reconcile_known_reply(&f.p, &old_ref, copy_reply(&old_replies[0].1), &f.facts, now)
            .await
            .map(|_| ()),
    ] {
        assert!(
            matches!(
                result,
                Err(Error::Unauthorized("execution_device_required"))
            ),
            "every old-device execution action must hit assignment fencing"
        );
    }
    // Worker leases may expire, but no fresh sender may inherit old execution
    // authority. Sending means a prior HTTP could already be in flight: retain
    // the immutable transaction/body and uncertainty, never issue another send.
    let mut db = diesel_async::AsyncPgConnection::establish(
        &std::env::var("HAGENCY_AGENT_TEST_DATABASE_URL").unwrap(),
    )
    .await
    .unwrap();
    // Advance only the worker deadline in this isolated fixture, preserving
    // sending state, original transaction/body and old execution authority.
    diesel::sql_query("UPDATE hagency_agent_v1.reply_outbox SET worker_until_ms=0 WHERE id=$1 AND state='sending'")
        .bind::<diesel::sql_types::Text,_>(&old_replies[2].0.id).execute(&mut db).await.unwrap();
    let fresh = f.facts.clone();
    for (reply, _) in &old_replies[1..] {
        assert!(matches!(
            f.transport.claim_reply(&reply.id, &fresh, now).await,
            Err(Error::Unauthorized("reply_lease_expired"))
        ));
    }
    assert!(
        f.transport
            .reply_candidates(16, now)
            .await
            .unwrap()
            .iter()
            .all(|r| r.agent_id != f.agent)
    );
    let new = f
        .transport
        .acquire_for_test(&f.second, &f.agent, 60000, false, now)
        .await
        .unwrap();
    assert!(new.epoch > old.epoch);
    let new_ref = LeaseRef {
        agent_id: f.agent.clone(),
        epoch: new.epoch,
    };
    assert!(
        f.transport
            .start_execution(
                &f.second,
                &new_ref,
                &event,
                "instance-running-execution",
                &f.facts,
                now
            )
            .await
            .is_err(),
        "reassignment must not rerun uncertain old inference/tools"
    );
    let history = f
        .transport
        .execution_history(&f.second, &f.agent, None, None, now)
        .await
        .unwrap();
    assert_eq!(history.snapshot.count, 4);
    assert!(
        f.transport
            .reply_candidates(16, now)
            .await
            .unwrap()
            .iter()
            .all(|r| r.agent_id != f.agent),
        "new device lease must not inherit old outbox delivery epoch"
    );
    for (reply, _) in &old_replies[1..] {
        assert!(matches!(
            f.transport.claim_reply(&reply.id, &fresh, now).await,
            Err(Error::Unauthorized("reply_lease_expired"))
        ));
    }
    let mut db = diesel_async::AsyncPgConnection::establish(
        &std::env::var("HAGENCY_AGENT_TEST_DATABASE_URL").unwrap(),
    )
    .await
    .unwrap();
    assert!(
        diesel::sql_query(
            "UPDATE hagency_agent_v1.agents SET execution_device_id=NULL WHERE id=$1"
        )
        .bind::<diesel::sql_types::Text, _>(&f.agent)
        .execute(&mut db)
        .await
        .is_err()
    );
}
