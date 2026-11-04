//! Canonical Auth/Friend scopes and native commit witnesses, not allow-authority fixtures.
#[path = "mural_tools_b_support/capture.rs"]
mod capture;
#[path = "mural_tools_b_support/environment.rs"]
mod environment;
#[path = "mural_tools_b_support/fixture.rs"]
mod fixture;
#[path = "mural_tools_b_support/owners.rs"]
mod owners;
use chrono::Utc;
use concert_application::commands::{FingerprintKeys, SigningKey};
use concert_service::{
    mural_tools_b::MuralToolsRuntime,
    product_rpc::{proto, scope_ledger::ScopeKey},
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;
fn runtime(f: &fixture::Fixture) -> MuralToolsRuntime {
    MuralToolsRuntime::new(
        f.concert.clone(),
        Arc::new(
            FingerprintKeys::new(
                SigningKey::new("wall-native-v1", &[92; 32]).unwrap(),
                vec![],
            )
            .unwrap(),
        ),
    )
}
fn raw(
    f: &fixture::Fixture,
    method: &str,
    suffix: &str,
    body: Value,
    key: Option<Uuid>,
) -> proto::ProductRequest {
    let now = Utc::now().timestamp_micros();
    proto::ProductRequest {
        user_id: f.actor.to_string(),
        event_id: f.event.to_string(),
        session_family_id: f.family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: method.into(),
        http_target: format!("/api/concert/events/{}/{}", f.event, suffix),
        http_body: if method == "GET" {
            vec![]
        } else {
            serde_json::to_vec(&body).unwrap()
        },
        content_type: if method == "GET" {
            ""
        } else {
            "application/json"
        }
        .into(),
        idempotency_key: key.map(|k| k.to_string()).unwrap_or_default(),
        authority_expires_unix_micros: now + 4_800_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        access_expires_unix_micros: now + 60_000_000,
        ..Default::default()
    }
}
async fn call(
    f: &fixture::Fixture,
    r: &MuralToolsRuntime,
    op: &str,
    req: proto::ProductRequest,
) -> anyhow::Result<(u32, Value)> {
    let captured = json!({"operation_id":op,"actor_id":req.user_id,"event_id":req.event_id,"target":req.http_target,"idempotency_key":req.idempotency_key,"original_body":if req.http_body.is_empty(){Value::Null}else{serde_json::from_slice::<Value>(&req.http_body)?}});
    let (b, parent, req) = capture::capture(req, op).await?;
    let scope = f.fence.acquire(b).await?;
    let p = scope.producer_scope();
    f.ledger
        .bind_delivery(
            &ScopeKey {
                instance: f.instance,
                scope: Uuid::parse_str(&p.scope_id)?,
                binding_digest: p.binding_digest.as_slice().try_into()?,
            },
            &[90; 32],
        )
        .await?;
    let owned = scope.clone();
    let response = scope
        .run(parent.scope(r.dispatch_for_scope(owned, req)))
        .await?;
    let body: Value = serde_json::from_slice(&response.body)?;
    println!(
        "WALL_B_RESPONSE={}",
        json!({"request":captured,"status":response.status,"body":body})
    );
    scope.mark_serialized().await?;
    scope.finish_response().await?;
    f.assert_clean().await?;
    Ok((response.status, body))
}
async fn setup(n: usize) -> anyhow::Result<fixture::Fixture> {
    let f = fixture::Fixture::new().await?;
    let mut grid = vec![1u8; 16384];
    grid[..n].fill(0);
    let palette: Vec<String> = (0..16).map(|n| format!("#{n:06X}")).collect();
    sqlx::query("insert into concert_mural_state(event_id,snapshot_id,state,pixels,palette,cooldown_ms) values($1,$2,'EDITABLE',$3,$4,0)").bind(f.event).bind(Uuid::new_v4()).bind(grid).bind(palette).execute(&f.concert).await?;
    sqlx::query("insert into concert_mural_tool_config(event_id,background_index) values($1,0)")
        .bind(f.event)
        .execute(&f.concert)
        .await?;
    Ok(f)
}
async fn state(f: &fixture::Fixture) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar("select jsonb_build_object('mural',(select to_jsonb(s) from concert_mural_state s where event_id=$1),'actions',(select count(*) from concert_mural_tool_actions where event_id=$1),'charges',(select coalesce(sum(changed_cells),0) from concert_mural_tool_charges where event_id=$1),'operations',(select count(*) from concert_mural_operations where event_id=$1),'limits',(select jsonb_agg(to_jsonb(l)) from concert_mural_limits l where event_id=$1))").bind(f.event).fetch_one(&f.concert).await?)
}
#[tokio::test]
#[ignore = "owned allocated PG16 and actual mTLS owner services required"]
async fn canonical_fill_undo_budget_recovery_and_restart() -> anyhow::Result<()> {
    let f = setup(256).await?;
    let r = runtime(&f);
    let key = Uuid::new_v4();
    let body = json!({"expected_revision":1,"x":0,"y":0,"color_index":2});
    let (status, action) = call(
        &f,
        &r,
        "fillConcertMuralRegion",
        raw(&f, "POST", "mural/fill", body.clone(), Some(key)),
    )
    .await?;
    assert_eq!(status, 200, "{action}");
    assert_eq!(action["changed_cells"], 256);
    assert_eq!(action["reset_required"], true);
    let baseline = state(&f).await?;
    assert_eq!(baseline["charges"], 256);
    assert_eq!(baseline["operations"], 0);
    let (status, replayed) = call(
        &f,
        &r,
        "fillConcertMuralRegion",
        raw(&f, "POST", "mural/fill", body.clone(), Some(key)),
    )
    .await?;
    assert_eq!(status, 200, "{replayed}");
    assert_eq!(action, replayed);
    assert_eq!(state(&f).await?, baseline);
    let (status, receipt) = call(
        &f,
        &r,
        "getConcertCommandStatus",
        raw(
            &f,
            "GET",
            &format!("commands?operation_id=fillConcertMuralRegion&idempotency_key={key}"),
            Value::Null,
            None,
        ),
    )
    .await?;
    assert_eq!(status, 200, "{receipt}");
    assert_eq!(receipt["receipt_id"], action["receipt_id"]);
    assert_eq!(receipt["state"], "SUCCEEDED");
    drop(r);
    let r = runtime(&f);
    let (status, recovered) = call(
        &f,
        &r,
        "getOwnConcertMuralAction",
        raw(
            &f,
            "GET",
            &format!("mural/actions/{key}"),
            Value::Null,
            None,
        ),
    )
    .await?;
    assert_eq!(status, 200, "{recovered}");
    assert_eq!(recovered, action);
    let (status, delta) = call(
        &f,
        &r,
        "getConcertMuralDelta",
        raw(
            &f,
            "GET",
            "mural/operations?%61fter_revision=%31",
            Value::Null,
            None,
        ),
    )
    .await?;
    assert_eq!(status, 200, "{delta}");
    assert_eq!(delta["reset_required"], true);
    let (status, undo) = call(
        &f,
        &r,
        "undoConcertMuralAction",
        raw(
            &f,
            "POST",
            "mural/undo",
            json!({"expected_revision":2,"target_action_id":action["action_id"]}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{undo}");
    assert_eq!(undo["changed_cells"], 256);
    assert_eq!(undo["revision"], 3);
    let baseline = state(&f).await?;
    assert_eq!(baseline["charges"], 512);
    let (status, error) = call(
        &f,
        &r,
        "eraseConcertMuralPixel",
        raw(
            &f,
            "POST",
            "mural/erase",
            json!({"expected_revision":3,"x":0,"y":2}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 429, "{error}");
    assert_eq!(error["details"]["reason"], "MURAL_BUDGET_EXCEEDED");
    assert_eq!(state(&f).await?, baseline);
    let (status, caps) = call(
        &f,
        &r,
        "getConcertMuralTools",
        raw(&f, "GET", "mural/tools", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{caps}");
    assert_eq!(caps["remaining_cells"], 0);
    assert_eq!(caps["undo"], Value::Null);
    assert_eq!(caps["available"]["pencil"], false);
    Ok(())
}
#[tokio::test]
#[ignore = "owned allocated PG16 and actual mTLS owner services required"]
async fn canonical_fill_257_noop_region_lock_and_failures_are_atomic() -> anyhow::Result<()> {
    let f = setup(257).await?;
    let r = runtime(&f);
    let baseline = state(&f).await?;
    for (color, reason) in [(2, "MURAL_FILL_TOO_LARGE"), (0, "MURAL_NO_CHANGE")] {
        let (status, v) = call(
            &f,
            &r,
            "fillConcertMuralRegion",
            raw(
                &f,
                "POST",
                "mural/fill",
                json!({"expected_revision":1,"x":0,"y":0,"color_index":color}),
                Some(Uuid::new_v4()),
            ),
        )
        .await?;
        assert_eq!(status, 409, "{v}");
        assert_eq!(v["details"]["reason"], reason);
        assert_eq!(state(&f).await?, baseline);
    }
    sqlx::query("insert into concert_mural_locks(event_id,lock_id,x_min,y_min,x_max_exclusive,y_max_exclusive) values($1,$2,0,0,1,1)").bind(f.event).bind(Uuid::new_v4()).execute(&f.concert).await?;
    let (status, v) = call(
        &f,
        &r,
        "eraseConcertMuralPixel",
        raw(
            &f,
            "POST",
            "mural/erase",
            json!({"expected_revision":1,"x":0,"y":2}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 409, "{v}");
    assert_eq!(v["details"]["reason"], "MURAL_NO_CHANGE");
    assert_eq!(state(&f).await?, baseline);
    let (status, v) = call(
        &f,
        &r,
        "placeConcertPixel",
        raw(
            &f,
            "POST",
            "mural/pixels",
            json!({"expected_revision":1,"x":0,"y":0,"color_index":3}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 409, "{v}");
    assert_eq!(v["details"]["reason"], "MURAL_REGION_LOCKED");
    assert_eq!(state(&f).await?, baseline);
    Ok(())
}
#[test]
fn wall_protocol_rejects_ambiguity_and_binds_recovery_operation() {
    use concert_service::mural_tools_b::protocol::{self, WallOperation};
    let event = Uuid::from_u128(7);
    let key = Uuid::from_u128(8);
    for query in [
        "after_revision=1",
        "after_revision=%31",
        "%61fter_revision=1",
    ] {
        assert_eq!(
            protocol::delta_after(
                event,
                &format!("/api/concert/events/{event}/mural/operations?{query}")
            )
            .unwrap(),
            1
        );
    }
    for query in [
        "after_revision=01",
        "after_revision=+1",
        "after_revision=1&%61fter_revision=2",
        "after_revision=%",
        "after_revision=9007199254740992",
    ] {
        assert!(protocol::delta_after(
            event,
            &format!("/api/concert/events/{event}/mural/operations?{query}")
        )
        .is_err());
    }

    assert!(protocol::body(
        WallOperation::Fill,
        br#"{"expected_revision":1,"x":0,"y":0,"color_index":2,"x":1}"#
    )
    .is_err());
    assert!(protocol::body(
        WallOperation::Erase,
        br#"{"expected_revision":1,"x":0,"y":0,"color_index":0}"#
    )
    .is_err());
    assert!(protocol::status_query(event,&format!("/api/concert/events/{event}/commands?operation_id=fillConcertMuralRegion&idempotency_key={key}")).is_ok());
    for extra in [
        "&operation_id=placeConcertPixel",
        "&target_id=untrusted",
        "&idempotency_key=bad",
    ] {
        assert!(protocol::status_query(event,&format!("/api/concert/events/{event}/commands?operation_id=fillConcertMuralRegion&idempotency_key={key}{extra}")).is_err());
    }
    assert!(protocol::status_query(event,&format!("/api/concert/events/{event}/commands?operation_id=createConcertParty&idempotency_key={key}")).is_err());
    assert!(WallOperation::parse(
        event,
        "GET",
        &format!("/api/concert/events/{event}/mural/tools?actor=other")
    )
    .is_err());
}

#[tokio::test]
#[ignore = "owned allocated PG16 and actual mTLS owner services required"]
async fn canonical_pencil_shares_undo_budget_and_storage_failure_rolls_back() -> anyhow::Result<()>
{
    let f = setup(1).await?;
    let r = runtime(&f);
    let key = Uuid::new_v4();
    let (status, ack) = call(
        &f,
        &r,
        "placeConcertPixel",
        raw(
            &f,
            "POST",
            "mural/pixels",
            json!({"expected_revision":1,"x":0,"y":0,"color_index":3}),
            Some(key),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{ack}");
    assert!(ack.get("command").is_none());
    assert_eq!(state(&f).await?["charges"], 1);
    let (status, action) = call(
        &f,
        &r,
        "getOwnConcertMuralAction",
        raw(
            &f,
            "GET",
            &format!("mural/actions/{key}"),
            Value::Null,
            None,
        ),
    )
    .await?;
    assert_eq!(status, 200, "{action}");
    assert_eq!(action["kind"], "PENCIL");
    assert_eq!(action["reset_required"], false);
    let (status, undo) = call(
        &f,
        &r,
        "undoConcertMuralAction",
        raw(
            &f,
            "POST",
            "mural/undo",
            json!({"expected_revision":2,"target_action_id":action["action_id"]}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{undo}");
    assert_eq!(state(&f).await?["charges"], 2);
    let baseline = state(&f).await?;
    sqlx::raw_sql("create function wall_b_fixture_abort_charge() returns trigger language plpgsql as $$ begin raise exception 'Wall B injected storage failure'; end $$; create trigger wall_b_fixture_abort_charge before insert on concert_mural_tool_charges for each row execute function wall_b_fixture_abort_charge();").execute(&f.concert).await?;
    let fail_key = Uuid::new_v4();
    let (status, error) = call(
        &f,
        &r,
        "eraseConcertMuralPixel",
        raw(
            &f,
            "POST",
            "mural/erase",
            json!({"expected_revision":3,"x":1,"y":0}),
            Some(fail_key),
        ),
    )
    .await?;
    assert_eq!(status, 503, "{error}");
    assert_eq!(state(&f).await?, baseline);
    let receipts:i64=sqlx::query_scalar("select count(*) from concert_command_receipts where event_id=$1 and actor_id=$2 and idempotency_key=$3").bind(f.event).bind(f.actor).bind(fail_key).fetch_one(&f.concert).await?;
    assert_eq!(receipts, 0);
    Ok(())
}

#[tokio::test]
#[ignore = "owned allocated PG16 and actual mTLS owner services required"]
async fn canonical_tabs_revision_cooldown_pause_and_admission_are_server_owned(
) -> anyhow::Result<()> {
    let f = setup(1).await?;
    let r = runtime(&f);
    sqlx::query("update concert_mural_state set revision=revision+1,reset_revision=revision+1,snapshot_id=$2,cooldown_ms=60000 where event_id=$1").bind(f.event).bind(Uuid::new_v4()).execute(&f.concert).await?;
    let second_family = Uuid::new_v4();
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,$2,$3,$4,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(Uuid::new_v4().to_string()).bind(f.actor).bind(second_family).execute(&f.auth).await?;
    let a = raw(
        &f,
        "POST",
        "mural/fill",
        json!({"expected_revision":2,"x":0,"y":0,"color_index":2}),
        Some(Uuid::new_v4()),
    );
    let mut b = raw(
        &f,
        "POST",
        "mural/fill",
        json!({"expected_revision":2,"x":0,"y":0,"color_index":3}),
        Some(Uuid::new_v4()),
    );
    b.session_family_id = second_family.to_string();
    let (a, b) = tokio::join!(
        call(&f, &r, "fillConcertMuralRegion", a),
        call(&f, &r, "fillConcertMuralRegion", b)
    );
    let a = a?;
    let b = b?;
    assert!(
        matches!((a.0, b.0), (200, 409) | (409, 200)),
        "a={a:?}, b={b:?}"
    );
    assert_eq!(state(&f).await?["charges"], 1);
    let baseline = state(&f).await?;
    let (status, error) = call(
        &f,
        &r,
        "eraseConcertMuralPixel",
        raw(
            &f,
            "POST",
            "mural/erase",
            json!({"expected_revision":3,"x":0,"y":0}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 429, "{error}");
    assert_eq!(error["details"]["reason"], "MURAL_COOLDOWN");
    assert_eq!(state(&f).await?, baseline);
    sqlx::query("update concert_mural_state set state='PAUSED',revision=revision+1,reset_revision=revision+1,snapshot_id=$2 where event_id=$1").bind(f.event).bind(Uuid::new_v4()).execute(&f.concert).await?;
    let baseline = state(&f).await?;
    let (status, error) = call(
        &f,
        &r,
        "eraseConcertMuralPixel",
        raw(
            &f,
            "POST",
            "mural/erase",
            json!({"expected_revision":4,"x":0,"y":0}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 409, "{error}");
    assert_eq!(error["details"]["reason"], "MURAL_PAUSED");
    assert_eq!(state(&f).await?, baseline);
    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(f.event).bind(f.actor).execute(&f.concert).await?;
    let (status, error) = call(
        &f,
        &r,
        "getConcertMuralTools",
        raw(&f, "GET", "mural/tools", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 404, "{error}");
    assert_eq!(state(&f).await?, baseline);
    Ok(())
}

#[tokio::test]
#[ignore = "owned allocated PG16 and actual mTLS owner services required"]
async fn canonical_activation_cannot_forgive_recent_unaccounted_pencil() -> anyhow::Result<()> {
    let f = setup(1).await?;
    let r = runtime(&f);
    sqlx::query("delete from concert_mural_tool_config where event_id=$1")
        .bind(f.event)
        .execute(&f.concert)
        .await?;
    let (status, ack) = call(
        &f,
        &r,
        "placeConcertPixel",
        raw(
            &f,
            "POST",
            "mural/pixels",
            json!({"expected_revision":1,"x":0,"y":0,"color_index":3}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{ack}");
    let baseline = state(&f).await?;
    assert_eq!(baseline["charges"], 0);
    let denied = sqlx::query(
        "insert into concert_mural_tool_config(event_id,background_index) values($1,0)",
    )
    .bind(f.event)
    .execute(&f.concert)
    .await;
    assert!(denied.is_err());
    assert_eq!(state(&f).await?, baseline);
    let (status, caps) = call(
        &f,
        &r,
        "getConcertMuralTools",
        raw(&f, "GET", "mural/tools", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{caps}");
    assert_eq!(caps["background_index"], Value::Null);
    assert_eq!(caps["editable"], false);
    assert_eq!(caps["available"]["fill"], false);
    Ok(())
}

#[tokio::test]
#[ignore = "owned allocated PG16 and actual mTLS owner services required"]
async fn canonical_actor_budget_and_private_recovery_are_isolated() -> anyhow::Result<()> {
    let mut f = setup(256).await?;
    let r = runtime(&f);
    let first = f.actor;
    let first_family = f.family;
    let first_key = Uuid::new_v4();
    let (status, first_action) = call(
        &f,
        &r,
        "fillConcertMuralRegion",
        raw(
            &f,
            "POST",
            "mural/fill",
            json!({"expected_revision":1,"x":0,"y":0,"color_index":2}),
            Some(first_key),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{first_action}");
    f.actor = f.subject;
    f.family = Uuid::new_v4();
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,$2,$3,$4,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(Uuid::new_v4().to_string()).bind(f.actor).bind(f.family).execute(&f.auth).await?;
    let (status, error) = call(
        &f,
        &r,
        "getOwnConcertMuralAction",
        raw(
            &f,
            "GET",
            &format!("mural/actions/{first_key}"),
            Value::Null,
            None,
        ),
    )
    .await?;
    assert_eq!(status, 404, "{error}");
    let (status, caps) = call(
        &f,
        &r,
        "getConcertMuralTools",
        raw(&f, "GET", "mural/tools", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{caps}");
    assert_eq!(caps["remaining_cells"], 512);
    assert_eq!(caps["latest_action"], Value::Null);
    let (status, other) = call(
        &f,
        &r,
        "eraseConcertMuralPixel",
        raw(
            &f,
            "POST",
            "mural/erase",
            json!({"expected_revision":2,"x":0,"y":0}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{other}");
    assert_eq!(other["changed_cells"], 1);
    let (status, caps) = call(
        &f,
        &r,
        "getConcertMuralTools",
        raw(&f, "GET", "mural/tools", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{caps}");
    assert_eq!(caps["remaining_cells"], 511);
    f.actor = first;
    f.family = first_family;
    let baseline = state(&f).await?;
    let (status, error) = call(
        &f,
        &r,
        "undoConcertMuralAction",
        raw(
            &f,
            "POST",
            "mural/undo",
            json!({"expected_revision":3,"target_action_id":first_action["action_id"]}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 409, "{error}");
    assert_eq!(error["details"]["reason"], "MURAL_UNDO_NOT_LATEST");
    assert_eq!(state(&f).await?, baseline);
    let (status, caps) = call(
        &f,
        &r,
        "getConcertMuralTools",
        raw(&f, "GET", "mural/tools", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{caps}");
    assert_eq!(caps["remaining_cells"], 256);
    assert_eq!(caps["undo"], Value::Null);
    Ok(())
}
