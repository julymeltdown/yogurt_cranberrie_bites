//! Actual canonical mTLS owner/SQL coverage. Only the private test verifier and
//! immutable fixture records are synthetic; no fixture can construct a grant.
#[path = "intro_product_support/capture.rs"]
mod capture;
#[path = "intro_product_support/environment.rs"]
mod environment;
#[path = "intro_product_support/fixture.rs"]
mod fixture;
#[path = "intro_product_support/owners.rs"]
mod owners;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    commands::{FingerprintKeys, SigningKey},
    Clock,
};
use concert_service::{
    adapters::out::{
        age_provider::{
            runtime::{AgeRuntime, ProviderMode},
            trust::ClaimTrust,
        },
        relationship_authority::guarded::{proto::TransactionStatus, OwnerGuardScope},
    },
    config::native_resolver::{NativeResolverDomain, NativeResolverUnion},
    intro_live::{protocol, IntroCursor, IntroRuntime},
    product_rpc::{
        proto,
        scope_ledger::{ProducerTransaction, ScopeKey},
        settlement::NativeTransactionResolver,
    },
};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
struct RealClock;
impl Clock for RealClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
fn runtime(f: &fixture::Fixture, trust: bool) -> IntroRuntime {
    let keys = Arc::new(
        FingerprintKeys::new(
            SigningKey::new("intro-native-fixture-v1", &[92; 32]).unwrap(),
            vec![],
        )
        .unwrap(),
    );
    let age = AgeRuntime::new(
        f.concert.clone(),
        Arc::new(RealClock),
        keys.clone(),
        ProviderMode::Unavailable,
    )
    .with_claim_trust(if trust {
        ClaimTrust::new(&[("fixture", "fixture-v1")]).unwrap()
    } else {
        ClaimTrust::none()
    });
    IntroRuntime::new(
        f.concert.clone(),
        f.reader.clone(),
        IntroCursor::new([93; 32]).unwrap(),
        keys,
        Arc::new(age),
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
async fn acquire(
    f: &fixture::Fixture,
    op: &str,
    raw: proto::ProductRequest,
) -> anyhow::Result<(
    Arc<OwnerGuardScope>,
    platform::actor_delegation::exchange::VerifiedParentAuthority,
    proto::ProductRequest,
)> {
    let (b, parent, raw) = capture::capture(raw, op).await?;
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
    Ok((scope, parent, raw))
}
async fn call(
    f: &fixture::Fixture,
    r: &IntroRuntime,
    op: &str,
    raw: proto::ProductRequest,
) -> anyhow::Result<(u32, Value)> {
    let (s, p, raw) = acquire(f, op, raw).await?;
    let owned = s.clone();
    let r = r.clone();
    let v = s
        .run_owned(async move { p.scope(r.dispatch_for_scope(owned, raw)).await })
        .await?;
    let body = serde_json::from_slice(&v.body)?;
    // Test Root terminal callback: production feature code owns no such method.
    s.mark_serialized().await?;
    s.finish_response().await?;
    f.assert_clean().await?;
    Ok((v.status, body))
}
fn draft(rev: i64, text: &str, restricted: bool) -> Value {
    json!({"expected_revision":rev,"text":text,"age_bounds":{"minimum_age":if restricted{Some(20)}else{None},"maximum_age_exclusive":if restricted{Some(40)}else{None}},"expires_at":null})
}
async fn publish(f: &fixture::Fixture, r: &IntroRuntime, restricted: bool) -> anyhow::Result<Uuid> {
    let (status, v) = call(
        f,
        r,
        "setConcertIntro",
        raw(
            f,
            "PUT",
            "me/intro",
            draft(0, "Meet fellow fans", restricted),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{v}");
    let id = Uuid::parse_str(v["intro_id"].as_str().unwrap())?;
    let (status, v) = call(
        f,
        r,
        "publishConcertIntro",
        raw(
            f,
            "POST",
            "me/intro/publish",
            json!({"expected_revision":1}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(status, 200, "{v}");
    Ok(id)
}
async fn seed_claim(f: &fixture::Fixture, expires: &str, source: &str) -> anyhow::Result<()> {
    let id = Uuid::new_v4();
    sqlx::query("insert into concert_age_owners(user_id) values($1) on conflict do nothing")
        .bind(f.actor)
        .execute(&f.concert)
        .await?;
    sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) values($1,$2,$3,$4,'fixture',$5,$6,'AGE_PRIVACY',clock_timestamp(),clock_timestamp()+interval '5 minutes','PENDING')").bind(id).bind(f.actor).bind(f.event).bind(f.family).bind(source).bind(vec![94_u8;32]).execute(&f.concert).await?;
    sqlx::query("insert into concert_age_claims(claim_id,user_id,event_id,verifier_id,source_revision,status,verified_at,expires_at,age_at_verification,next_birthday_at) values($1,$2,$3,'fixture',$4,'VERIFIED',clock_timestamp()-interval '1 minute',clock_timestamp()+$5::interval,26,(clock_timestamp() at time zone 'Asia/Seoul')::date+1)").bind(id).bind(f.actor).bind(f.event).bind(source).bind(expires).execute(&f.concert).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "owned PG16/four actual mTLS owners required"]
async fn canonical_authoring_discovery_detail_idempotency_and_close() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let r = runtime(&f, false);
    let id = publish(&f, &r, false).await?;
    let (status, owner) = call(
        &f,
        &r,
        "getOwnConcertIntro",
        raw(&f, "GET", "me/intro", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200);
    assert_eq!(owner["revision"], 2);
    let (status, list) = call(
        &f,
        &r,
        "listConcertIntros",
        raw(&f, "GET", "intros?limit=1", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{list}");
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let (status, detail) = call(
        &f,
        &r,
        "getConcertIntro",
        raw(&f, "GET", &format!("intros/{id}"), Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{detail}");
    assert_eq!(detail["author"]["user_id"], f.actor.to_string());
    for secret in [
        "minimum_age",
        "maximum_age_exclusive",
        "expires_at",
        "provider",
        "birthyear",
        "private-avatar-canary",
    ] {
        assert!(!detail.to_string().contains(secret), "{secret}");
    }
    let k = Uuid::new_v4();
    let body = draft(2, "Edited fan intro", false);
    let (a, _) = call(
        &f,
        &r,
        "setConcertIntro",
        raw(&f, "PUT", "me/intro", body.clone(), Some(k)),
    )
    .await?;
    assert_eq!(a, 200);
    let (a, v) = call(
        &f,
        &r,
        "setConcertIntro",
        raw(&f, "PUT", "me/intro", body, Some(k)),
    )
    .await?;
    assert_eq!(a, 200, "{v}");
    assert_eq!(v["revision"], 3);
    let (a, _) = call(
        &f,
        &r,
        "setConcertIntro",
        raw(
            &f,
            "PUT",
            "me/intro",
            draft(2, "Conflicting body", false),
            Some(k),
        ),
    )
    .await?;
    assert_eq!(a, 409);
    let (a, _) = call(
        &f,
        &r,
        "setConcertIntro",
        raw(
            &f,
            "PUT",
            "me/intro",
            draft(2, "Stale revision", false),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(a, 409);
    // Explicit expired private receipt fixture; the real retained repository
    // checks current canonical authority and returns the precise frozen reason.
    let expired_key = Uuid::new_v4();
    let expired_body = draft(3, "Expired request", false);
    let command = protocol::parse_intro_command(
        concert_domain::AuthUserId::from_uuid(f.actor),
        concert_domain::EventId::from_uuid(f.event),
        expired_key,
        "setConcertIntro",
        &serde_json::to_vec(&expired_body)?,
    )?;
    let keys = FingerprintKeys::new(
        SigningKey::new("intro-native-fixture-v1", &[92; 32])?,
        vec![],
    )?;
    let fingerprint = keys.sign(&command.request()?)?.as_storage();
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,created_at,expires_at) select $1,$2,$3,'setConcertIntro',$4,$5,$6,'RETIRED',t-interval '2 days',t-interval '1 day' from (select clock_timestamp() t) source").bind(Uuid::new_v4()).bind(f.actor).bind(f.event).bind(expired_key).bind(fingerprint).bind(id).execute(&f.concert).await?;
    let (status, v) = call(
        &f,
        &r,
        "setConcertIntro",
        raw(&f, "PUT", "me/intro", expired_body, Some(expired_key)),
    )
    .await?;
    assert_eq!(status, 409, "{v}");
    assert_eq!(v["details"]["reason"], "IDEMPOTENCY_EXPIRED");
    let (a, v) = call(
        &f,
        &r,
        "closeConcertIntro",
        raw(
            &f,
            "POST",
            "me/intro/close",
            json!({"expected_revision":3}),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    assert_eq!(a, 200, "{v}");
    let (a, _) = call(
        &f,
        &r,
        "getConcertIntro",
        raw(&f, "GET", &format!("intros/{id}"), Value::Null, None),
    )
    .await?;
    assert_eq!(a, 404);
    Ok(())
}
#[tokio::test]
#[ignore = "owned PG16/four actual mTLS owners required"]
async fn canonical_restricted_age_outage_trust_and_expiry_parity() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let r = runtime(&f, true);
    let id = publish(&f, &r, true).await?;
    for allowed in [false, true] {
        if allowed {
            seed_claim(&f, "10 minutes", "fixture-v1").await?;
        }
        let (a, v) = call(
            &f,
            &r,
            "listConcertIntros",
            raw(&f, "GET", "intros", Value::Null, None),
        )
        .await?;
        assert_eq!(a, 200, "{v}");
        assert_eq!(v["items"].as_array().unwrap().len(), usize::from(allowed));
        let (a, _) = call(
            &f,
            &r,
            "getConcertIntro",
            raw(&f, "GET", &format!("intros/{id}"), Value::Null, None),
        )
        .await?;
        assert_eq!(a, if allowed { 200 } else { 404 });
    }
    let denied = runtime(&f, false);
    let (a, _) = call(
        &f,
        &denied,
        "getConcertIntro",
        raw(&f, "GET", &format!("intros/{id}"), Value::Null, None),
    )
    .await?;
    assert_eq!(a, 404);
    sqlx::query("update concert_age_claims set expires_at=clock_timestamp()-interval '1 second' where user_id=$1").bind(f.actor).execute(&f.concert).await?;
    let (a, _) = call(
        &f,
        &r,
        "getConcertIntro",
        raw(&f, "GET", &format!("intros/{id}"), Value::Null, None),
    )
    .await?;
    assert_eq!(a, 404);
    Ok(())
}
#[tokio::test]
#[ignore = "owned PG16/four actual mTLS owners required"]
async fn canonical_full_frame_rebinding_and_missing_parent_fail_closed() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let r = runtime(&f, false);
    let original = raw(&f, "GET", "intros", Value::Null, None);
    let (s, p, original) = acquire(&f, "listConcertIntros", original).await?;
    assert!(r
        .dispatch_for_scope(s.clone(), original.clone())
        .await
        .is_err());
    let mut forged = original.clone();
    forged.content_type = "forged-content-type".into();
    assert!(p
        .clone()
        .scope(r.dispatch_for_scope(s.clone(), forged))
        .await
        .is_err());
    let mut forged = original;
    forged.idempotency_key = Uuid::new_v4().to_string();
    assert!(p
        .scope(r.dispatch_for_scope(s.clone(), forged))
        .await
        .is_err());
    s.mark_serialized().await?;
    s.finish_response().await?;
    f.assert_clean().await?;
    Ok(())
}
#[test]
fn closed_protocol_rejects_ambiguous_target_query_and_write_shape() {
    let e = Uuid::new_v4();
    let base = format!("/api/concert/events/{e}/");
    for suffix in [
        "intros/../x",
        "intros/00000000-0000-0000-0000-000000000000",
        "me/intro?",
        "me/intro#x",
    ] {
        assert!(protocol::parse_intro_target(e, "GET", &(base.clone() + suffix)).is_err());
    }
    for q in [
        "?",
        "?limit=1&limit=2",
        "?q=%xx",
        "?unknown=x",
        "?cursor=a+b",
        "?q=",
        "?limit=-1",
    ] {
        assert!(
            protocol::parse_query(&(base.clone() + "intros" + q)).is_err(),
            "{q}"
        );
    }
    assert!(protocol::parse_query(&(base + "intros?limit=100&q=fan+club")).is_ok());
    assert!(protocol::parse_intro_command(concert_domain::AuthUserId::from_uuid(Uuid::new_v4()),concert_domain::EventId::from_uuid(e),Uuid::new_v4(),"setConcertIntro",br#"{"expected_revision":0,"text":"a","age_bounds":{"minimum_age":null,"maximum_age_exclusive":null}}"#).is_err());
}

async fn become_peer(f: &mut fixture::Fixture) -> anyhow::Result<()> {
    std::mem::swap(&mut f.actor, &mut f.subject);
    f.family = Uuid::new_v4();
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,$2,$3,$4,clock_timestamp()+interval '1 hour')")
        .bind(Uuid::new_v4().to_string()).bind(Uuid::new_v4().to_string()).bind(f.actor).bind(f.family).execute(&f.auth).await?;
    Ok(())
}
async fn hidden(
    f: &fixture::Fixture,
    r: &IntroRuntime,
    id: Uuid,
    stage: &str,
) -> anyhow::Result<()> {
    let (status, page) = call(
        f,
        r,
        "listConcertIntros",
        raw(f, "GET", "intros", Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{stage}: {page}");
    assert!(page["items"].as_array().unwrap().is_empty(), "{page}");
    let (status, detail) = call(
        f,
        r,
        "getConcertIntro",
        raw(f, "GET", &format!("intros/{id}"), Value::Null, None),
    )
    .await?;
    assert_eq!(status, 404, "{detail}");
    Ok(())
}
#[tokio::test]
#[ignore = "owned PG16/four actual mTLS owners required"]
async fn canonical_block_departure_suspension_and_profile_outage() -> anyhow::Result<()> {
    let mut f = fixture::Fixture::new().await?;
    let r = runtime(&f, false);
    let id = publish(&f, &r, false).await?;
    become_peer(&mut f).await?;
    let (status, v) = call(
        &f,
        &r,
        "getConcertIntro",
        raw(&f, "GET", &format!("intros/{id}"), Value::Null, None),
    )
    .await?;
    assert_eq!(status, 200, "{v}");
    sqlx::query(
        "insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(f.actor)
    .bind(f.subject)
    .execute(&f.friend)
    .await?;
    hidden(&f, &r, id, "inherited Friend block").await?;
    sqlx::query(
        "update user_blocks set deleted_at=clock_timestamp() where blocker_id=$1 and blocked_id=$2",
    )
    .bind(f.actor)
    .bind(f.subject)
    .execute(&f.friend)
    .await?;
    sqlx::query("delete from concert_event_admissions where event_id=$1 and user_id=$2")
        .bind(f.event)
        .bind(f.subject)
        .execute(&f.concert)
        .await?;
    hidden(&f, &r, id, "local admission departure").await?;
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(f.event)
    .bind(f.subject)
    .execute(&f.concert)
    .await?;
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(f.subject)
        .execute(&f.auth)
        .await?;
    hidden(&f, &r, id, "Auth suspension").await?;
    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(f.subject)
        .execute(&f.auth)
        .await?;
    f.stop_profile().await?;
    let (scope, parent, request) = acquire(
        &f,
        "listConcertIntros",
        raw(&f, "GET", "intros", Value::Null, None),
    )
    .await?;
    let owned = scope.clone();
    let producer = r.clone();
    let value = scope
        .run_owned(async move {
            parent
                .scope(producer.dispatch_for_scope(owned, request))
                .await
        })
        .await?;
    assert_eq!(
        value.status, 503,
        "Profile outage must abort the whole projection"
    );
    scope.mark_serialized().await?;
    // Lost/unreceived Profile work is not guessed complete while its actual
    // owner is unavailable. Test Root retains terminal settlement for recovery.
    assert!(scope.finish_response().await.is_err());
    f.restart_profile().await?;
    scope.finish_response().await?;
    f.assert_clean().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "owned PG16/four actual mTLS owners required"]
async fn canonical_claim_expiry_bounds_root_delivery_after_commit() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let r = runtime(&f, true);
    let id = publish(&f, &r, true).await?;
    seed_claim(&f, "2 seconds", "fixture-v1").await?;
    let expiry: DateTime<Utc> = sqlx::query_scalar(
        "select expires_at from concert_age_claims where user_id=$1 and status='VERIFIED'",
    )
    .bind(f.actor)
    .fetch_one(&f.concert)
    .await?;
    let (s, p, req) = acquire(
        &f,
        "getConcertIntro",
        raw(&f, "GET", &format!("intros/{id}"), Value::Null, None),
    )
    .await?;
    let owned = s.clone();
    let r = r.clone();
    let output = s
        .run_owned(async move { p.scope(r.dispatch_for_scope(owned, req)).await })
        .await?;
    assert_eq!(output.status, 200);
    assert!(s.disclosure_ceiling()?.expires_micros() <= expiry.timestamp_micros());
    s.mark_serialized().await?;
    tokio::time::sleep(
        (expiry - Utc::now() + Duration::milliseconds(25))
            .to_std()
            .unwrap_or_default(),
    )
    .await;
    assert!(
        s.disclosure_ceiling().is_err(),
        "SQL commit must not renew the original claim disclosure ceiling"
    );

    // System cleanup settles the held real transaction and prepared children;
    // an expired representation is never sent to an HTTP client by this harness.
    s.finish_response().await?;
    f.assert_clean().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "owned PG16/four actual mTLS owners required"]
async fn canonical_native_witness_requires_exact_registered_transaction() -> anyhow::Result<()> {
    let f = fixture::Fixture::new().await?;
    let r = runtime(&f, false);
    let (s, p, req) = acquire(
        &f,
        "setConcertIntro",
        raw(
            &f,
            "PUT",
            "me/intro",
            draft(0, "Native witness", false),
            Some(Uuid::new_v4()),
        ),
    )
    .await?;
    let owned = s.clone();
    let result = s
        .run_owned(async move { p.scope(r.dispatch_for_scope(owned, req)).await })
        .await?;
    assert_eq!(
        result.status,
        200,
        "{}",
        String::from_utf8_lossy(&result.body)
    );
    let identities: Vec<Value> = sqlx::query_scalar(
        "select native_identity from concert_intro_native_witness where effect_digest=$1",
    )
    .bind(s.binding().digest().to_vec())
    .fetch_all(&f.concert)
    .await?;
    assert!(
        identities.len() >= 2,
        "write and owner projection must each have actual registered native transactions"
    );
    let resolver = NativeResolverUnion::new(f.concert.clone(), f.instance)?;
    assert_eq!(
        NativeResolverUnion::domain(&s.binding().owner_binding())?,
        NativeResolverDomain::Intro
    );
    for value in identities {
        let identity: ProducerTransaction = serde_json::from_value(value)?;
        let resolved = resolver
            .resolve(&s.binding().owner_binding(), &identity)
            .await?;
        assert_eq!(resolved.status, TransactionStatus::Committed);
        assert!(!resolved.receipt_evidence.is_empty());
        let mut foreign = identity.clone();
        foreign.producer_instance = Uuid::new_v4();
        assert_eq!(
            resolver
                .resolve(&s.binding().owner_binding(), &foreign)
                .await?
                .status,
            TransactionStatus::Unknown
        );
        let mut wrong = identity.clone();
        wrong.settlement_reference =
            serde_json::to_vec(&json!({"kind":"intro_native_v1","witness_id":Uuid::new_v4()}))?;
        assert_eq!(
            resolver
                .resolve(&s.binding().owner_binding(), &wrong)
                .await?
                .status,
            TransactionStatus::Unknown
        );
    }
    for statement in [
        "update concert_intro_native_witness set authority_until=authority_until",
        "delete from concert_intro_native_witness",
        "truncate concert_intro_native_witness",
    ] {
        assert!(
            sqlx::query(statement).execute(&f.concert).await.is_err(),
            "{statement}"
        );
    }
    s.mark_serialized().await?;
    s.finish_response().await?;
    f.assert_clean().await?;
    let mut unfenced = f.concert.begin().await?;
    sqlx::query("select set_config('mcr.intro_authority_until',(clock_timestamp()+interval '1 minute')::text,true)").execute(&mut *unfenced).await?;
    sqlx::query("insert into concert_intros(event_id,intro_id,owner_id,revision,state,text,created_at,updated_at) values($1,$2,$3,1,'DRAFT','Unfenced fixture',clock_timestamp(),clock_timestamp())")
        .bind(f.event).bind(Uuid::new_v4()).bind(f.subject).execute(&mut *unfenced).await?;
    assert!(
        unfenced.commit().await.is_err(),
        "a fabricated deadline is not a registered native witness"
    );
    // Native commit alone is insufficient; a genuine committed foreign effect
    // without the registered same-transaction Intro witness remains UNKNOWN.
    let (s, _, _) = acquire(
        &f,
        "listConcertIntros",
        raw(&f, "GET", "intros", Value::Null, None),
    )
    .await?;
    for committed in [false, true] {
        let mut tx = f.concert.begin().await?;
        let id=concert_service::adapters::out::relationship_authority::guarded::ProducerTransaction::capture_concert_in(&mut tx,s.binding().digest(),f.instance,serde_json::to_vec(&json!({"kind":"intro_native_v1","witness_id":Uuid::new_v4()}))?).await?;
        let wire = id.wire()?;
        let identity = ProducerTransaction {
            producer: "CONCERT".into(),
            system_identifier: id.system_identifier.clone(),
            database_oid: id.database_oid,
            full_xid: id.full_xid.clone(),
            backend_pid: id.backend_pid,
            backend_started_unix_micros: wire.backend_started_unix_micros,
            effect_binding_digest: id.effect_binding_digest,
            producer_instance: f.instance,
            settlement_reference: id.settlement_reference.clone(),
        };
        s.track_transaction(id).await?;
        assert_eq!(
            resolver
                .resolve(&s.binding().owner_binding(), &identity)
                .await?
                .status,
            TransactionStatus::InProgress
        );
        if committed {
            tx.commit().await?;
        } else {
            tx.rollback().await?;
        }
        assert_eq!(
            resolver
                .resolve(&s.binding().owner_binding(), &identity)
                .await?
                .status,
            if committed {
                TransactionStatus::Unknown
            } else {
                TransactionStatus::Aborted
            }
        );
    }
    // Deliberately unknown committed effect is quarantined, not declared done.
    s.mark_serialized().await?;
    assert!(s.finish_response().await.is_err());
    Ok(())
}

#[tokio::test]
#[ignore = "owned PG16/four actual mTLS owners required"]
async fn canonical_intro_edit_revokes_consent_atomically_with_receipt_outbox() -> anyhow::Result<()>
{
    let f = fixture::Fixture::new().await?;
    let r = runtime(&f, false);
    let intro = publish(&f, &r, false).await?;
    let request = Uuid::new_v4();
    // Private SQL fixture metadata. It cannot produce an owner grant, a public
    // handle or an alternative authority; only the real Intro edit below runs.
    let mut seed = f.concert.begin().await?;
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)").execute(&mut *seed).await?;
    sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,2,1,1,'PENDING',clock_timestamp()+interval '1 hour')").bind(f.event).bind(request).bind(f.subject).bind(f.actor).bind(intro).execute(&mut *seed).await?;
    sqlx::query("update concert_social_requests set state='APPROVED',revision=revision+1 where request_id=$1").bind(request).execute(&mut *seed).await?;
    sqlx::query("insert into concert_social_bindings(event_id,request_id,platform,requester_version,recipient_version,requester_consents,approved) values($1,$2,'INSTAGRAM',1,1,true,true)").bind(f.event).bind(request).execute(&mut *seed).await?;
    for (a, b) in [(f.actor, f.subject), (f.subject, f.actor)] {
        sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) select $1,$2,$3,$4,'INSTAGRAM',1,1,'ACTIVE',expires_at from concert_social_requests where request_id=$2").bind(f.event).bind(request).bind(a).bind(b).execute(&mut *seed).await?;
    }
    seed.commit().await?;
    // Force a failure in the actual late BE15 outbox stage, after consent edits.
    sqlx::raw_sql("create function intro_product_fail_outbox() returns trigger language plpgsql as $$ begin raise exception 'owned fixture late outbox failure'; end $$; create trigger intro_product_fail_outbox before insert on concert_outbox for each row execute function intro_product_fail_outbox();").execute(&f.concert).await?;
    let key = Uuid::new_v4();
    let edit = draft(2, "Revised safe introduction", false);
    let (status, v) = call(
        &f,
        &r,
        "setConcertIntro",
        raw(&f, "PUT", "me/intro", edit.clone(), Some(key)),
    )
    .await?;
    assert_eq!(status, 503, "{v}");
    let revision: i64 = sqlx::query_scalar("select revision from concert_intros where intro_id=$1")
        .bind(intro)
        .fetch_one(&f.concert)
        .await?;
    assert_eq!(revision, 2);
    let state: String =
        sqlx::query_scalar("select state from concert_social_requests where request_id=$1")
            .bind(request)
            .fetch_one(&f.concert)
            .await?;
    assert_eq!(state, "APPROVED");
    let active: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where request_id=$1 and state='ACTIVE'",
    )
    .bind(request)
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(active, 2);
    sqlx::raw_sql("drop trigger intro_product_fail_outbox on concert_outbox; drop function intro_product_fail_outbox();").execute(&f.concert).await?;
    for _ in 0..2 {
        let (status, v) = call(
            &f,
            &r,
            "setConcertIntro",
            raw(&f, "PUT", "me/intro", edit.clone(), Some(key)),
        )
        .await?;
        assert_eq!(status, 200, "{v}");
        assert_eq!(v["revision"], 3);
    }
    let state: String =
        sqlx::query_scalar("select state from concert_social_requests where request_id=$1")
            .bind(request)
            .fetch_one(&f.concert)
            .await?;
    assert_eq!(state, "REVOKED");
    let active: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where request_id=$1 and state='ACTIVE'",
    )
    .bind(request)
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(active, 0);
    let queued: i64 = sqlx::query_scalar(
        "select count(*) from concert_outbox where resource_id=$1 and resource_revision=3",
    )
    .bind(intro)
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(queued, 1);
    Ok(())
}

#[test]
fn cursor_encryption_binding_and_original_fence_expiry() {
    use concert_application::intro::cursor::{CursorBinding, CursorCodec, CursorWindow};
    let codec = IntroCursor::new([71; 32]).unwrap();
    let now = DateTime::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap();
    let b = CursorBinding {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        operation: "listConcertIntros",
        collection: "intros",
        query: Some(3),
    };
    let window = CursorWindow {
        upper: now,
        after: None,
    };
    let token = codec.seal(&b, window, now).unwrap();
    assert!(!token.contains(&b.actor.to_string()));
    assert_ne!(token, codec.seal(&b, window, now).unwrap());
    assert_eq!(codec.open(&b, &token, now).unwrap(), window);
    let later = now + Duration::minutes(14);
    let continued = codec.seal(&b, window, later).unwrap();
    assert!(codec
        .open(&b, &continued, now + Duration::minutes(15))
        .is_err());
    assert!(codec
        .open(
            &CursorBinding {
                actor: Uuid::new_v4(),
                ..b
            },
            &token,
            now
        )
        .is_err());
    let mut tampered = token.into_bytes();
    tampered[5] = if tampered[5] == b'A' { b'B' } else { b'A' };
    assert!(codec
        .open(&b, std::str::from_utf8(&tampered).unwrap(), now)
        .is_err());
    assert!(codec.open(&b, &format!("{continued}="), now).is_err());
}
