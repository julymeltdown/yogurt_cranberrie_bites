//! Context scope and cancellation checks; native tests use the retained owned PG harness.
use super::*;
use crate::adapters::out::relationship_authority::{world_projection::*, RequestBinding};

fn observed(request: &WorldOwnerRequest) -> WorldAuthObservation {
    WorldAuthObservation {
        subject: request.subject.clone(),
        request_id: request.request_id,
        credential_expires_at: request.credential_expires_at,
        observed_at: Utc::now(),
        valid_until: DateTime::from_timestamp_micros(request.deadline.expires_micros()).unwrap(),
        revision: OwnerRevision {
            epoch: Uuid::new_v4(),
            revision: 1,
        },
    }
}
fn parent(
    c: &TicketClaim,
    stop: AuthorityRpcDeadline,
) -> (WorldOwnerRequest, WorldProjectionParent) {
    let current = owned(c);
    let request = WorldOwnerRequest::from_current(
        &current,
        &current.owner,
        &c.record.subject,
        c.connection_id,
        Uuid::new_v4(),
        stop,
    )
    .unwrap();
    let parent = request.projection_parent(&observed(&request)).unwrap();
    (request, parent)
}
fn candidate(parent: &WorldProjectionParent) -> (CurrentOwnedMembership, WorldSubject) {
    let mut c = parent.current_membership().unwrap().clone();
    c.membership.key.user_id = AuthUserId::from_uuid(Uuid::new_v4());
    c.membership.session_id = SessionId::new(Uuid::new_v4().to_string()).unwrap();
    c.membership.connection_id = ConnectionId::from_uuid(Uuid::new_v4()).unwrap();
    let subject = WorldSubject {
        key: c.membership.key,
        event_id: c.membership.key.event_id,
        session_id: c.membership.session_id.clone(),
    };
    (c, subject)
}
fn presence(
    parent: &WorldProjectionParent,
    c: &CurrentOwnedMembership,
    subject: &WorldSubject,
) -> WorldPresenceProjection {
    parent
        .presence_projection(c, &c.owner, subject, c.membership.connection_id)
        .unwrap()
}
#[tokio::test]
async fn profile_is_self_bound_and_rederivation_cannot_reset_attempts_or_auth() {
    let c = claim();
    let request = binding(&c);
    let auth = observed(&request);
    let parent = request.projection_parent(&auth).unwrap();
    assert!(parent.current_membership().is_none());
    assert_eq!(parent.consumed_ticket(), Some(&c));
    let profile = parent.profile_projection().unwrap();
    assert_eq!(profile.actor(), c.record.subject.key.user_id.uuid());
    assert_eq!(profile.subject(), profile.actor());
    assert_eq!(profile.event_id(), c.record.subject.event_id.uuid());
    assert_eq!(
        profile.session_family_id().to_string(),
        c.record.subject.session_id.as_str()
    );
    assert_eq!(profile.request_id(), request.request_id());
    assert_eq!(profile.purpose(), "WORLD_SELF_PROFILE");
    assert_eq!(profile.credential_expires_at(), c.record.access_expires_at);
    assert_eq!(profile.source_expires_at(), c.record.expires_at);
    let token = profile.begin_read().unwrap();
    drop(token);
    assert_eq!(
        profile.clone().begin_read().unwrap_err(),
        WorldError::CredentialConsumed
    );
    assert_eq!(
        request
            .projection_parent(&auth)
            .unwrap()
            .profile_projection()
            .unwrap()
            .begin_read()
            .unwrap_err(),
        WorldError::CredentialConsumed
    );
    // Child gates neither burn nor reset the independent original Auth slot.
    request.attempt().unwrap();
    assert_eq!(
        request.attempt().unwrap_err(),
        WorldError::CredentialConsumed
    );
    let mut wrong = observed(&request);
    wrong.request_id = Uuid::new_v4();
    assert!(request.projection_parent(&wrong).is_err());
    let current = owned(&c);
    assert!(parent
        .presence_projection(&current, &current.owner, &c.record.subject, c.connection_id)
        .is_err());
}
#[tokio::test]
async fn presence_targets_share_replay_slots_but_fanout_is_independent() {
    let c = claim();
    let (request, p) = parent(&c, deadline());
    let (a, sa) = candidate(&p);
    let (b, sb) = candidate(&p);
    let child = presence(&p, &a, &sa);
    let binding = RequestBinding::from_world(&child).unwrap();
    assert_eq!(binding.query(), child.query());
    assert_eq!(binding.session_family(), p.session_family_id());
    assert_eq!(binding.request_id(), p.request_id());
    assert_eq!(child.parent().current_membership(), p.current_membership());
    assert_eq!(child.candidate(), &a);
    let token = child.begin_auth_read().unwrap();
    assert_eq!(token.subject(), sa.key.user_id.uuid());
    assert_eq!(token.run(async { Ok(7) }).await.unwrap(), 7);
    assert!(matches!(
        child
            .begin_friend_read()
            .unwrap()
            .run(async { Err::<(), _>(WorldError::Denied) })
            .await,
        Err(WorldError::Denied)
    ));
    assert_eq!(
        child.clone().begin_auth_read().unwrap_err(),
        WorldError::CredentialConsumed
    );
    let again = request.projection_parent(&observed(&request)).unwrap();
    assert_eq!(
        presence(&again, &a, &sa).begin_friend_read().unwrap_err(),
        WorldError::CredentialConsumed
    );
    // Same canonical wire target with a newer lease or connection still cannot reset.
    let mut newer = a.clone();
    newer.membership.lease_until += chrono::Duration::seconds(1);
    newer.membership.connection_id = ConnectionId::from_uuid(Uuid::new_v4()).unwrap();
    assert_eq!(
        presence(&p, &newer, &sa).begin_auth_read().unwrap_err(),
        WorldError::CredentialConsumed
    );
    let other = presence(&p, &b, &sb);
    other
        .begin_auth_read()
        .unwrap()
        .run(async { Ok(()) })
        .await
        .unwrap();
    other
        .begin_friend_read()
        .unwrap()
        .run(async { Ok(()) })
        .await
        .unwrap();
    p.profile_projection()
        .unwrap()
        .begin_read()
        .unwrap()
        .run(async { Ok(()) })
        .await
        .unwrap();
}
#[tokio::test]
async fn candidate_binding_rejects_owner_session_connection_room_pin_and_expiry_drift() {
    let (_, p) = parent(&claim(), deadline());
    let (good, subject) = candidate(&p);
    let mut cases = vec![];
    let mut c = good.clone();
    c.owner.epoch = Uuid::new_v4();
    cases.push(c);
    let mut c = good.clone();
    c.membership.session_id = SessionId::new(Uuid::new_v4().to_string()).unwrap();
    cases.push(c);
    let mut c = good.clone();
    c.membership.connection_id = ConnectionId::from_uuid(Uuid::new_v4()).unwrap();
    cases.push(c);
    let mut c = good.clone();
    c.membership.room_id = RoomId::from_uuid(Uuid::new_v4());
    cases.push(c);
    let mut c = good.clone();
    c.membership.pin.map.revision = Revision::new(2).unwrap();
    cases.push(c);
    let mut c = good.clone();
    c.membership.pin.avatar.manifest_version = Revision::new(2).unwrap();
    cases.push(c);
    let mut c = good.clone();
    c.membership.key.event_id = EventId::from_uuid(Uuid::new_v4());
    cases.push(c);
    let mut c = good.clone();
    c.membership.lease_until = Utc::now() - chrono::Duration::seconds(1);
    cases.push(c);
    let mut c = good.clone();
    c.membership.access_expires_at = Utc::now() - chrono::Duration::seconds(1);
    cases.push(c);
    for bad in cases {
        assert!(p
            .presence_projection(&bad, &good.owner, &subject, good.membership.connection_id)
            .is_err());
    }
    let child = presence(&p, &good, &subject);
    assert!(child.deadline().stop() <= p.deadline().stop());
    assert!(child.deadline().expires_micros() <= p.deadline().expires_micros());
    assert_eq!(child.credential_expires_at(), p.credential_expires_at());
    assert_eq!(
        child.candidate_credential_expires_at(),
        good.membership.access_expires_at
    );
}
#[tokio::test]
async fn original_monotonic_stop_and_earlier_ambient_stop_survive_derivation() {
    let stop = tokio::time::Instant::now() + Duration::from_millis(80);
    let original = AuthorityRpcDeadline::within_original(
        Utc::now().timestamp_micros() + 60_000_000,
        stop.into_std(),
        Duration::from_secs(60),
    )
    .unwrap();
    let (_, p) = parent(&claim(), original);
    let child = p.profile_projection().unwrap();
    assert_eq!(child.original_deadline().stop(), stop);
    assert_eq!(child.deadline().stop(), stop);
    assert!(child.deadline().expires_micros() > Utc::now().timestamp_micros() + 1_000_000);
    let token = child.begin_read().unwrap();
    tokio::time::sleep_until(stop + Duration::from_millis(5)).await;
    assert_eq!(
        token.run(async { Ok(()) }).await.unwrap_err(),
        WorldError::Unavailable
    );
    let (_, p) = parent(&claim(), deadline());
    let child = p.profile_projection().unwrap();
    let earlier = std::time::Instant::now() + Duration::from_millis(35);
    let token =
        platform::call_deadline::scope(earlier, async { child.begin_read().unwrap() }).await;
    assert!(token.deadline().stop().into_std() <= earlier);
    tokio::time::sleep(Duration::from_millis(45)).await;
    assert_eq!(
        token.run(async { Ok(()) }).await.unwrap_err(),
        WorldError::Unavailable
    );
}
#[tokio::test]
async fn cancellation_and_drop_burn_slots_and_reject_noncooperative_late_results() {
    let (request, p) = parent(&claim(), deadline());
    let profile = p.profile_projection().unwrap();
    let token = profile.begin_read().unwrap();
    let (c, subject) = candidate(&p);
    let child = presence(&p, &c, &subject);
    let auth = child.begin_auth_read().unwrap();
    {
        let future = auth.run(std::future::pending::<Result<(), WorldError>>());
        tokio::pin!(future);
        tokio::select! { _ = &mut future => panic!("unexpected completion"), _ = tokio::time::sleep(Duration::from_millis(10)) => {} }
    } // Drop the actual running future, not just its pinned reference.
    assert_eq!(
        child.begin_auth_read().unwrap_err(),
        WorldError::CredentialConsumed
    );
    let cancelling = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(15)).await;
        request.cancel();
    });
    assert_eq!(
        token
            .run(std::future::pending::<Result<(), WorldError>>())
            .await
            .unwrap_err(),
        WorldError::Unavailable
    );
    cancelling.await.unwrap();
    assert!(p.check().is_err());
    assert!(child.check().is_err());
    assert!(RequestBinding::from_world(&child).is_err());
    assert!(child.begin_friend_read().is_err());
    assert_eq!(
        child.begin_friend_read().unwrap_err(),
        WorldError::CredentialConsumed
    );
    // A blocking poll that returns success beyond the original stop is refused.
    let original = AuthorityRpcDeadline::within_original(
        Utc::now().timestamp_micros() + 60_000_000,
        std::time::Instant::now() + Duration::from_millis(25),
        Duration::from_secs(60),
    )
    .unwrap();
    let (_, p) = parent(&claim(), original);
    let result = p
        .profile_projection()
        .unwrap()
        .begin_read()
        .unwrap()
        .run(async {
            std::thread::sleep(Duration::from_millis(40));
            Ok(())
        })
        .await;
    assert_eq!(result.unwrap_err(), WorldError::Unavailable);
}

#[tokio::test]
async fn fanout_bound_and_execution_ambient_stop_cannot_restore_owner_slots() {
    let (request, p) = parent(&claim(), deadline());
    for _ in 0..800 {
        let (c, subject) = candidate(&p);
        presence(&p, &c, &subject);
    }
    let (extra, subject) = candidate(&p);
    assert!(p
        .presence_projection(
            &extra,
            &extra.owner,
            &subject,
            extra.membership.connection_id
        )
        .is_err());
    let profile = p.profile_projection().unwrap();
    let token = profile.begin_read().unwrap();
    let stop = std::time::Instant::now() + Duration::from_millis(20);
    let result = platform::call_deadline::scope(
        stop,
        token.run(std::future::pending::<Result<(), WorldError>>()),
    )
    .await;
    assert_eq!(result.unwrap_err(), WorldError::Unavailable);
    assert_eq!(
        profile.begin_read().unwrap_err(),
        WorldError::CredentialConsumed
    );
    // Cancellation from a derivative propagates to its original Auth parent too.
    profile.cancel();
    assert!(request.check().is_err());
}

#[tokio::test]
#[ignore = "requires explicitly marker/inode-owned Auth and Concert PostgreSQL"]
async fn native_context_uses_consumed_and_current_pg_records_with_signed_auth() -> anyhow::Result<()>
{
    let auth = pool("auth").await?;
    let concert = pool("concert").await?;
    let mut c = fixture(&auth).await?;
    // The retained native regression has already consumed claim()'s fixed digest.
    // This is a separate real ticket, not a replay or relaxed repository invariant.
    let mut entropy = [0u8; 32];
    entropy[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    entropy[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    c.record.digest = AdmissionTicket::from_entropy(&entropy)?.digest();
    let repo = PgWorldRepository::new(concert.clone());
    let event = c.record.subject.event_id.uuid();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Context fixture','OPEN')").bind(event).bind(format!("ctx-{}",event.simple())).execute(&concert).await?;
    sqlx::query("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values($1,$2,'Context',2,'map_world_auth',1,'avatar_manifest',1,true)").bind(event).bind(c.record.pin.room_id.uuid()).execute(&concert).await?;
    assert_eq!(repo.issue(c.record.clone()).await?, IssueOutcome::Issued);
    let ConsumeOutcome::Consumed(consumed) = repo
        .consume(ConsumeClaim {
            digest: c.record.digest.clone(),
            connection_id: c.connection_id,
            requested: c.record.pin.clone(),
            now: Utc::now(),
        })
        .await?
    else {
        anyhow::bail!("consume required")
    };
    let original = deadline();
    let request = WorldOwnerRequest::from_consumed(&consumed, Uuid::new_v4(), original)?;
    let server = native_server(auth.clone()).await;
    let reader = client(&server.endpoint);
    let checked = reader.observe(&request).await?;
    let ticket_parent = request.projection_parent(&checked)?;
    ticket_parent
        .profile_projection()?
        .begin_read()?
        .run(async {
            sqlx::query("select 1")
                .execute(&concert)
                .await
                .map_err(|_| WorldError::Unavailable)?;
            Ok(())
        })
        .await?;
    assert_eq!(
        reader.observe(&request).await.unwrap_err(),
        WorldError::CredentialConsumed
    );
    let owner = OwnerIdentity::new("context-native-owner".into(), Uuid::new_v4())?;
    repo.claim_owner(
        c.record.subject.event_id,
        c.record.pin.room_id,
        &owner,
        Utc::now(),
        chrono::Duration::seconds(20),
    )
    .await?;
    assert!(matches!(
        repo.admit(&owner, &consumed, Utc::now()).await?,
        AdmissionOutcome::Admitted(_)
    ));
    let current = repo
        .current_owned_membership(c.record.subject.key, Utc::now())
        .await?
        .expect("authoritative current required");
    assert!(ticket_parent
        .presence_projection(&current, &owner, &c.record.subject, c.connection_id)
        .is_err());
    let current_request = WorldOwnerRequest::from_current(
        &current,
        &owner,
        &c.record.subject,
        c.connection_id,
        Uuid::new_v4(),
        original,
    )?;
    let observation = reader.observe(&current_request).await?;
    let parent = current_request.projection_parent(&observation)?;
    let child = presence(&parent, &current, &c.record.subject);
    assert!(child.deadline().stop() <= original.stop());
    let a = child.begin_auth_read()?;
    let f = child.begin_friend_read()?;
    a.run(f.run(async {
        sqlx::query("select 1")
            .execute(&concert)
            .await
            .map_err(|_| WorldError::Unavailable)?;
        Ok(())
    }))
    .await?;
    assert_eq!(
        presence(&parent, &current, &c.record.subject)
            .begin_auth_read()
            .unwrap_err(),
        WorldError::CredentialConsumed
    );
    // Shared cancellation interrupts a real blocked PG read; dropping SQLx's
    // future does not prove immediate server cancellation. Pool close waits it out.
    let token = parent.profile_projection()?.begin_read()?;
    let cancel = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        current_request.cancel();
    });
    assert_eq!(
        token
            .run(async {
                sqlx::query("select pg_sleep(0.15)")
                    .execute(&concert)
                    .await
                    .map_err(|_| WorldError::Unavailable)?;
                Ok(())
            })
            .await
            .unwrap_err(),
        WorldError::Unavailable
    );
    cancel.await?;
    assert!(child.check().is_err());
    drop(server);
    concert.close().await;
    auth.close().await;
    Ok(())
}
