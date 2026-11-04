use async_trait::async_trait;
use concert_application::commands::*;
use concert_domain::{AuthUserId, EventId, Revision};
use concert_service::{
    adapters::out::repository::receipt::{CommandProducer, PgCommandRepository},
    relationship::social::revoke_intro_consent_in,
};
use sqlx::{PgConnection, Row};
use uuid::Uuid;
struct Edit {
    fail_after: bool,
}
#[async_trait]
impl CommandProducer for Edit {
    async fn authorize(&self, c: &mut PgConnection, _: Scope, _: Uuid) -> Result<(), CommandError> {
        // Synthetic fixture authority, never a canonical owner grant.
        sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)")
            .execute(c).await.map_err(|_| CommandError::Unavailable)?;
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        sqlx::query("select event_id from concert_events where event_id=$1 for update")
            .bind(s.event)
            .fetch_one(&mut *c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let revision:i64=sqlx::query_scalar("update concert_intros set revision=revision+1,updated_at=clock_timestamp() where event_id=$1 and intro_id=$2 and owner_id=$3 returning revision").bind(s.event).bind(target).bind(s.actor).fetch_one(&mut *c).await.map_err(|_|CommandError::Unavailable)?;
        revoke_intro_consent_in(
            c,
            EventId::from_uuid(s.event),
            target,
            AuthUserId::from_uuid(s.actor),
            Revision::new(revision).unwrap(),
        )
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if self.fail_after {
            return Err(CommandError::Unavailable);
        }
        let reference = SafeReference {
            resource: target,
            revision,
        };
        Ok((
            Completion::Succeeded {
                reference,
                status: 200,
            },
            Some(Invalidation { reference }),
        ))
    }
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn intro_edit_consent_revocation_outbox_replay_and_rollback_are_atomic() {
    let p = crate::owned::pool().await;
    let event = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let peer = Uuid::new_v4();
    let intro = Uuid::new_v4();
    let request = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Intro consent','LIVE')").bind(event).bind(event.to_string()).execute(&p).await.unwrap();
    sqlx::query("insert into concert_intros(event_id,intro_id,owner_id,revision,state,text,created_at,updated_at) values($1,$2,$3,1,'PUBLISHED','Intro',clock_timestamp(),clock_timestamp())").bind(event).bind(intro).bind(owner).execute(&p).await.unwrap();
    let mut seed = p.begin().await.unwrap();
    sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,true)").execute(&mut *seed).await.unwrap();
    sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,authority_revision,relationship_revision,state,expires_at) values($1,$2,$3,$4,$5,1,1,1,'PENDING',clock_timestamp()+interval '1 hour')").bind(event).bind(request).bind(peer).bind(owner).bind(intro).execute(&mut *seed).await.unwrap();
    sqlx::query("update concert_social_requests set state='APPROVED',revision=revision+1 where event_id=$1 and request_id=$2").bind(event).bind(request).execute(&mut *seed).await.unwrap();
    sqlx::query("insert into concert_social_bindings(event_id,request_id,platform,requester_version,recipient_version,requester_consents,approved) values($1,$2,'INSTAGRAM',1,1,true,true)").bind(event).bind(request).execute(&mut *seed).await.unwrap();
    for (a, b) in [(owner, peer), (peer, owner)] {
        sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) select $1,$2,$3,$4,'INSTAGRAM',1,1,'ACTIVE',expires_at from concert_social_requests where event_id=$1 and request_id=$2").bind(event).bind(request).bind(a).bind(b).execute(&mut *seed).await.unwrap();
    }
    seed.commit().await.unwrap();
    let keys =
        FingerprintKeys::new(SigningKey::new("fixture", &[37; 32]).unwrap(), vec![]).unwrap();
    let repo = PgCommandRepository::new(p.clone());
    let scope = Scope {
        actor: owner,
        event,
        operation: RegisteredOperation::find("setConcertIntro").unwrap(),
        key: Uuid::new_v4(),
    };
    let event_text = event.to_string();
    let body = CanonicalValue::Object(vec![]);
    let canonical =
        CanonicalRequest::json(scope, &[("eventId", &event_text)], &[], Some(&body)).unwrap();
    assert!(repo
        .execute(&canonical, intro, &keys, &Edit { fail_after: true })
        .await
        .is_err());
    let state: String = sqlx::query_scalar(
        "select state from concert_social_requests where event_id=$1 and request_id=$2",
    )
    .bind(event)
    .bind(request)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(state, "APPROVED");
    let active: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_grants where event_id=$1 and state='ACTIVE'",
    )
    .bind(event)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(active, 2);
    let receipt = repo
        .execute(&canonical, intro, &keys, &Edit { fail_after: false })
        .await
        .unwrap();
    let replay = repo
        .execute(&canonical, intro, &keys, &Edit { fail_after: false })
        .await
        .unwrap();
    assert_eq!(receipt, replay);
    let row=sqlx::query("select r.state,r.revision,(select count(*) from concert_social_grants g where g.event_id=r.event_id and g.state='ACTIVE') active,(select count(*) from concert_outbox o where o.receipt_id=$3 and o.resource_id=$4 and o.resource_revision=2) queued from concert_social_requests r where event_id=$1 and request_id=$2")
        .bind(event).bind(request).bind(receipt.id).bind(intro).fetch_one(&p).await.unwrap();
    assert_eq!(row.get::<String, _>("state"), "REVOKED");
    assert_eq!(row.get::<i64, _>("revision"), 3);
    assert_eq!(row.get::<i64, _>("active"), 0);
    assert_eq!(row.get::<i64, _>("queued"), 1);
    // A later successful source revision cannot restore old explicit consent.
    let scope = Scope {
        key: Uuid::new_v4(),
        ..scope
    };
    let canonical =
        CanonicalRequest::json(scope, &[("eventId", &event_text)], &[], Some(&body)).unwrap();
    repo.execute(&canonical, intro, &keys, &Edit { fail_after: false })
        .await
        .unwrap();
    let state: String = sqlx::query_scalar(
        "select state from concert_social_requests where event_id=$1 and request_id=$2",
    )
    .bind(event)
    .bind(request)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(state, "REVOKED");
    p.close().await;
}
