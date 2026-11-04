//! Native local-source qualification with explicitly synthetic owner/provider
//! fences. Never constructs a positive GuardedSubject or certifies a real join.
#[path = "social_remaining_support/owned.rs"]
mod owned;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::age_policy::Context;
use concert_domain::{
    relationship::{social::*, FreshAuthority, PolicyQuery, RelationshipError},
    AuthUserId, EventId, Revision,
};
use concert_service::relationship::{
    eligibility::*, social::SocialEligibility, FencedRelationshipAuthority,
};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
struct FixtureAuthority;
#[async_trait]
impl FencedRelationshipAuthority for FixtureAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        sqlx::query("select id from fixture_social_owner where id=1 for share")
            .fetch_one(c)
            .await
            .map_err(|_| RelationshipError::Unavailable)?;
        Ok(FreshAuthority {
            query: q,
            checked_at: now,
            valid_until: now + Duration::seconds(10),
            revision: Revision::initial(),
            actor_allowed: true,
            subject_allowed: true,
            inherited_block: false,
        })
    }
}
struct FixtureVerifier;
#[async_trait]
impl SharingVerifierStatus for FixtureVerifier {
    async fn current_in(
        &self,
        c: &mut PgConnection,
        _: Context,
    ) -> Result<VerifierStatus, RelationshipError> {
        let r=sqlx::query("select verifier,revision,available,expires_at from fixture_social_verifier where id=1 for share").fetch_one(c).await.map_err(|_|RelationshipError::Unavailable)?;
        if !r.get::<bool, _>("available") {
            return Err(RelationshipError::Unavailable);
        }
        Ok(VerifierStatus {
            verifier: r.get("verifier"),
            source_revision: r.get("revision"),
            valid_until: r.get("expires_at"),
        })
    }
}
struct Fixture {
    p: PgPool,
    request: Context,
    sharing: SharingContext,
}
impl Fixture {
    async fn new() -> Self {
        let p = owned::pool().await;
        let (event, a, b, intro) = (
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        );
        let now = Utc::now();
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Social fixture','LIVE')").bind(event).bind(event.to_string()).execute(&p).await.unwrap();
        for u in [a, b] {
            sqlx::query("insert into concert_age_owners(user_id) values($1)")
                .bind(u)
                .execute(&p)
                .await
                .unwrap();
            Self::claim(&p, event, u, "VERIFIED", now - Duration::minutes(1)).await;
        }
        sqlx::query("insert into concert_intros(event_id,intro_id,owner_id,revision,state,text,minimum_age,maximum_age_exclusive,expires_at,created_at,updated_at) values($1,$2,$3,1,'PUBLISHED','A genuine fixture offer',18,40,clock_timestamp()+interval '1 hour',clock_timestamp(),clock_timestamp())")
            .bind(event).bind(intro).bind(b).execute(&p).await.unwrap();
        Self {
            p,
            request: Context {
                actor: AuthUserId::from_uuid(a),
                event: EventId::from_uuid(event),
                session: Uuid::new_v4(),
                request: Uuid::new_v4(),
                expires_at: now + Duration::seconds(10),
            },
            sharing: SharingContext {
                event: EventId::from_uuid(event),
                requester: AuthUserId::from_uuid(a),
                recipient: AuthUserId::from_uuid(b),
                intro_id: intro,
                intro_revision: Revision::initial(),
            },
        }
    }
    async fn claim(p: &PgPool, event: Uuid, user: Uuid, status: &str, verified: DateTime<Utc>) {
        let id = Uuid::new_v4();
        sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) values($1,$2,$3,$4,'fixture','v1',decode(repeat('a1',32),'hex'),'AGE_PRIVACY',clock_timestamp(),clock_timestamp()+interval '5 minutes','PENDING')")
            .bind(id).bind(user).bind(event).bind(Uuid::new_v4()).execute(p).await.unwrap();
        sqlx::query("insert into concert_age_claims(claim_id,user_id,event_id,verifier_id,source_revision,status,verified_at,expires_at,age_at_verification,next_birthday_at) values($1,$2,$3,'fixture','v1',$4,$5,$5+interval '1 day',case when $4='UNSUPPORTED' then null else 25 end,case when $4='UNSUPPORTED' then null else ($5 at time zone 'Asia/Seoul')::date+100 end)")
            .bind(id).bind(user).bind(event).bind(status).bind(verified).execute(p).await.unwrap();
    }
    fn adapter(&self) -> PgSocialEligibility<FixtureAuthority, FixtureVerifier> {
        PgSocialEligibility::new(self.request, FixtureAuthority, FixtureVerifier)
    }
    async fn check(&self, sharing: SharingContext) -> Result<FreshSharing, RelationshipError> {
        let mut tx = self.p.begin().await.unwrap();
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        let r = self.adapter().check_in(&mut tx, sharing, now).await;
        tx.rollback().await.unwrap();
        r
    }
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn actual_intro_and_canonical_claim_rows_bind_exact_sharing_context() {
    let f = Fixture::new().await;
    let proof = f.check(f.sharing).await.unwrap();
    assert!(proof.allowed);
    assert_eq!(proof.context, f.sharing);
    assert!(proof.expires_at <= f.request.expires_at);
    for index in 0..4 {
        let mut s = f.sharing;
        match index {
            0 => s.intro_revision = Revision::new(2).unwrap(),
            1 => s.intro_id = Uuid::new_v4(),
            2 => s.event = EventId::from_uuid(Uuid::new_v4()),
            _ => s.recipient = s.requester,
        };
        assert!(f.check(s).await.is_err());
    }
    sqlx::query("update concert_intros set state='CLOSED',revision=revision+1 where intro_id=$1")
        .bind(f.sharing.intro_id)
        .execute(&f.p)
        .await
        .unwrap();
    assert!(f.check(f.sharing).await.is_err());
    f.p.close().await;
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn newest_unsupported_claim_never_falls_back_to_older_verified_proof() {
    let f = Fixture::new().await;
    f.check(f.sharing).await.unwrap();
    Fixture::claim(
        &f.p,
        f.request.event.uuid(),
        f.sharing.requester.uuid(),
        "UNSUPPORTED",
        Utc::now(),
    )
    .await;
    assert!(matches!(
        f.check(f.sharing).await,
        Err(RelationshipError::Denied)
    ));
    f.p.close().await;
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn unbounded_offer_still_requires_both_verified_ages_and_live_verifier() {
    let f = Fixture::new().await;
    sqlx::query("update concert_intros set minimum_age=null,maximum_age_exclusive=null,revision=revision+1 where intro_id=$1").bind(f.sharing.intro_id).execute(&f.p).await.unwrap();
    let mut sharing = f.sharing;
    sharing.intro_revision = Revision::new(2).unwrap();
    f.check(sharing).await.unwrap();
    Fixture::claim(
        &f.p,
        f.request.event.uuid(),
        sharing.recipient.uuid(),
        "UNSUPPORTED",
        Utc::now(),
    )
    .await;
    assert!(f.check(sharing).await.is_err());
    // No SQL schema or owner fallback can turn provider unavailability positive.
    let mut tx = f.p.begin().await.unwrap();
    let unavailable =
        PgSocialEligibility::new(f.request, FixtureAuthority, UnavailableSharingVerifier);
    assert!(matches!(
        unavailable.check_in(&mut tx, sharing, Utc::now()).await,
        Err(RelationshipError::Unavailable)
    ));
    tx.rollback().await.unwrap();
    f.p.close().await;
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn source_locks_hold_through_caller_transaction_and_release_after_rollback() {
    let f = Fixture::new().await;
    for kind in ["intro", "age"] {
        let mut tx = f.p.begin().await.unwrap();
        let mut sharing = f.sharing;
        if kind == "age" {
            sharing.intro_revision = Revision::new(2).unwrap();
        }
        f.adapter()
            .check_in(&mut tx, sharing, Utc::now())
            .await
            .unwrap();
        let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        let p = f.p.clone();
        let s = f.sharing;
        let task = tokio::spawn(async move {
            if kind == "intro" {
                sqlx::query("update concert_intros set revision=revision+1 where intro_id=$1")
                    .bind(s.intro_id)
                    .execute(&p)
                    .await
                    .unwrap();
            } else {
                sqlx::query("select user_id from concert_age_owners where user_id=$1 for update")
                    .bind(s.requester.uuid())
                    .execute(&p)
                    .await
                    .unwrap();
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(3),async{loop{
            let waits:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where $1=any(pg_blocking_pids(pid)))").bind(pid).fetch_one(&f.p).await.unwrap();
            if waits{break;}tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }}).await.unwrap();
        assert!(!task.is_finished());
        tx.rollback().await.unwrap();
        task.await.unwrap();
    }
    f.p.close().await;
}

#[tokio::test]
#[ignore = "private social_remaining_support/native.py fixture required"]
async fn lock_wait_cannot_outlive_original_request_deadline() {
    let mut f = Fixture::new().await;
    f.request.expires_at = Utc::now() + Duration::milliseconds(120);
    let mut blocker = f.p.begin().await.unwrap();
    sqlx::query("select intro_id from concert_intros where intro_id=$1 for update")
        .bind(f.sharing.intro_id)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let p = f.p.clone();
    let s = f.sharing;
    let adapter = f.adapter();
    let task = tokio::spawn(async move {
        let mut tx = p.begin().await.unwrap();
        let r = adapter.check_in(&mut tx, s, Utc::now()).await;
        tx.rollback().await.unwrap();
        r
    });
    tokio::time::sleep(std::time::Duration::from_millis(160)).await;
    blocker.rollback().await.unwrap();
    assert!(task.await.unwrap().is_err());
    f.p.close().await;
}
