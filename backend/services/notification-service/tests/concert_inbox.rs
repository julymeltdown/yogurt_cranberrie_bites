//! Real PostgreSQL persistence. Authority is explicitly fixture-only, never real Auth acceptance.
mod concert_inbox_support;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::notification::{Candidate, Context, Query};
use concert_domain::notification::*;
use notification_service::{
    adapters::out::persistence::notification_repository::PgNotificationRepository,
    application::ports::NotificationRepositoryPort, concert::*,
};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

#[derive(Clone, Copy, Default)]
struct FixtureAuthority {
    corrupt: bool,
    delay: bool,
}
struct FixtureFence(DateTime<Utc>);
impl AuthorityFence for FixtureFence {
    fn check(&self, now: DateTime<Utc>) -> Result<()> {
        if now < self.0 {
            Ok(())
        } else {
            Err(NotificationError::Unavailable)
        }
    }
}
#[async_trait]
impl FencedInboxAuthority for FixtureAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        ctx: Context,
        candidates: &[Candidate],
        now: DateTime<Utc>,
    ) -> Result<Authorization> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from be33_fixture_authority where event_id=$1 and user_id=$2 for share",
        )
        .bind(ctx.event_id)
        .bind(ctx.actor_id)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| NotificationError::Unavailable)?;
        if allowed != Some(true) {
            return Err(NotificationError::PermissionDenied);
        }
        let mut decisions = Vec::new();
        for candidate in candidates {
            let visibility = if let Some(target) = candidate.target {
                let row=sqlx::query("select visible,stale from be33_fixture_targets where event_id=$1 and target_id=$2 for share")
                    .bind(ctx.event_id).bind(target.target_id).fetch_optional(&mut *c).await.map_err(|_|NotificationError::Unavailable)?;
                match row {
                    Some(r) if r.get::<bool, _>("visible") => {
                        if r.get::<bool, _>("stale") {
                            Visibility::Stale
                        } else {
                            Visibility::Visible
                        }
                    }
                    _ => Visibility::Hidden,
                }
            } else {
                Visibility::Stale
            };
            decisions.push((*candidate, visibility));
        }
        if self.corrupt {
            decisions.push((
                Candidate {
                    notification_id: Uuid::new_v4(),
                    source_actor_id: None,
                    target: None,
                },
                Visibility::Visible,
            ));
        }
        let until = if self.delay {
            now + Duration::milliseconds(60)
        } else {
            ctx.expires_at
        };
        if self.delay {
            sqlx::query("select pg_sleep(0.1)")
                .execute(c)
                .await
                .map_err(|_| NotificationError::Unavailable)?;
        }
        Ok(Authorization {
            context: ctx,
            checked_at: now,
            valid_until: until,
            decisions,
            fence: Box::new(FixtureFence(until)),
        })
    }
}
struct Fixture {
    pool: PgPool,
    actor: Uuid,
    event: Uuid,
    source: Uuid,
    target: Uuid,
}
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let pool = concert_inbox_support::owned_pool("mcr_concert_notification").await?;
        let f = Self {
            pool,
            actor: Uuid::new_v4(),
            event: Uuid::new_v4(),
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
        };
        sqlx::query(
            "insert into be33_fixture_authority(event_id,user_id,allowed) values($1,$2,true)",
        )
        .bind(f.event)
        .bind(f.actor)
        .execute(&f.pool)
        .await?;
        sqlx::query("insert into be33_fixture_targets(event_id,target_id,visible,stale) values($1,$2,true,false)").bind(f.event).bind(f.target).execute(&f.pool).await?;
        Ok(f)
    }
    fn context(&self) -> Context {
        let now = Utc::now();
        Context {
            actor_id: self.actor,
            event_id: self.event,
            session_family_id: self.actor,
            request_id: Uuid::new_v4(),
            authority_digest: [33; 32],
            observed_at: now,
            expires_at: now + Duration::seconds(4),
        }
    }
    fn service(&self) -> PgConcertInbox<FixtureAuthority> {
        PgConcertInbox::new(self.pool.clone(), FixtureAuthority::default())
    }
    fn envelope(&self) -> Envelope {
        Envelope {
            outbox_id: Uuid::new_v4(),
            receipt_id: Uuid::new_v4(),
            event_id: self.event,
            resource_id: self.target,
            resource_revision: 1,
        }
    }
    fn recipient(&self) -> Recipient {
        Recipient {
            user_id: self.actor,
            source_actor_id: Some(self.source),
            kind: Kind::Social,
            target: Some(Target {
                target_type: TargetType::SocialRequest,
                target_id: self.target,
            }),
        }
    }
    async fn insert(&self) -> anyhow::Result<Envelope> {
        let e = self.envelope();
        self.service().ingest(e, vec![self.recipient()]).await?;
        Ok(e)
    }
    async fn list(&self) -> anyhow::Result<NotificationList> {
        Ok(self
            .service()
            .list(self.context(), Query::default())
            .await?)
    }
}
#[tokio::test]
async fn durable_delivery_dedup_and_conflicting_envelope() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let e = f.insert().await?;
    f.service().ingest(e, vec![f.recipient()]).await?;
    f.service().ingest(e, vec![]).await?; // first committed fanout is stable
    assert_eq!(f.list().await?.items.len(), 1);
    assert_eq!(
        f.service()
            .ingest(
                Envelope {
                    resource_revision: 2,
                    ..e
                },
                vec![f.recipient()]
            )
            .await,
        Err(NotificationError::IdempotencyConflict)
    );
    assert_eq!(
        f.service()
            .ingest(
                Envelope {
                    outbox_id: Uuid::new_v4(),
                    ..e
                },
                vec![f.recipient()]
            )
            .await,
        Err(NotificationError::IdempotencyConflict)
    );
    Ok(())
}
#[tokio::test]
async fn concurrent_delivery_creates_one_row() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let e = f.envelope();
    let s = f.service();
    let r = f.recipient();
    let (a, b) = tokio::join!(s.ingest(e, vec![r]), s.ingest(e, vec![r]));
    a?;
    b?;
    assert_eq!(f.list().await?.items.len(), 1);
    Ok(())
}
#[tokio::test]
async fn multi_recipient_fanout_and_no_secret_fields() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let e = f.envelope();
    let other = Uuid::new_v4();
    f.service()
        .ingest(
            e,
            vec![
                f.recipient(),
                Recipient {
                    user_id: other,
                    ..f.recipient()
                },
            ],
        )
        .await?;
    let json = serde_json::to_value(&f.list().await?.items[0])?;
    assert_eq!(json.as_object().unwrap().len(), 7);
    let payloads: Vec<String> =
        sqlx::query_scalar("select payload_json from notifications where event_id=$1")
            .bind(e.outbox_id)
            .fetch_all(&f.pool)
            .await?;
    assert_eq!(payloads, vec![String::new(), String::new()]);
    assert!(Kind::Social.safe_summary().contains("Sharing"));
    Ok(())
}
#[tokio::test]
async fn actor_event_and_hidden_target_isolation() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    for ctx in [
        Context {
            actor_id: Uuid::new_v4(),
            ..f.context()
        },
        Context {
            event_id: Uuid::new_v4(),
            ..f.context()
        },
    ] {
        assert_eq!(
            f.service().unread_count(ctx).await,
            Err(NotificationError::PermissionDenied)
        );
    }
    let id = f.list().await?.items[0].notification_id;
    sqlx::query("update be33_fixture_targets set visible=false where event_id=$1")
        .bind(f.event)
        .execute(&f.pool)
        .await?;
    assert!(f.list().await?.items.is_empty());
    assert_eq!(f.service().unread_count(f.context()).await?.unread_total, 0);
    assert_eq!(
        f.service()
            .mark_read(f.context(), id, 1, Uuid::new_v4())
            .await,
        Err(NotificationError::ResourceUnavailable)
    );
    Ok(())
}
#[tokio::test]
async fn stale_target_is_neutral_and_current_auth_denial_wins() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    sqlx::query("update be33_fixture_targets set stale=true where event_id=$1")
        .bind(f.event)
        .execute(&f.pool)
        .await?;
    assert_eq!(f.list().await?.items[0].target, None);
    sqlx::query("update be33_fixture_authority set allowed=false where event_id=$1")
        .bind(f.event)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        f.service().list(f.context(), Query::default()).await,
        Err(NotificationError::PermissionDenied)
    );
    Ok(())
}
#[tokio::test]
async fn full_unread_count_is_independent_of_pages_filter_and_read_state() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for _ in 0..25 {
        f.insert().await?;
    }
    let first = f
        .service()
        .list(
            f.context(),
            Query {
                limit: 3,
                ..Query::default()
            },
        )
        .await?;
    assert_eq!(first.items.len(), 3);
    assert!(first.next_cursor.is_some());
    assert_eq!(
        f.service().unread_count(f.context()).await?.unread_total,
        25
    );
    f.service()
        .mark_read(
            f.context(),
            first.items[0].notification_id,
            1,
            Uuid::new_v4(),
        )
        .await?;
    let unread = f
        .service()
        .list(
            f.context(),
            Query {
                unread_only: true,
                limit: 1,
                cursor: None,
            },
        )
        .await?;
    assert_eq!(unread.items.len(), 1);
    assert_eq!(
        f.service().unread_count(f.context()).await?.unread_total,
        24
    );
    Ok(())
}
#[tokio::test]
async fn opaque_cursor_paging_binding_tamper_and_expiry() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for _ in 0..5 {
        f.insert().await?;
    }
    let p = f
        .service()
        .list(
            f.context(),
            Query {
                limit: 2,
                ..Query::default()
            },
        )
        .await?;
    let token = p.next_cursor.unwrap();
    let next = f
        .service()
        .list(
            f.context(),
            Query {
                limit: 2,
                cursor: Some(token.clone()),
                ..Query::default()
            },
        )
        .await?;
    assert_eq!(next.items.len(), 2);
    assert!(next.items.iter().all(|n| !p.items.contains(n)));
    for query in [
        Query {
            unread_only: true,
            cursor: Some(token.clone()),
            ..Query::default()
        },
        Query {
            cursor: Some(Uuid::new_v4().to_string()),
            ..Query::default()
        },
        Query {
            cursor: Some(format!("{token}x")),
            ..Query::default()
        },
    ] {
        assert_eq!(
            f.service().list(f.context(), query).await,
            Err(NotificationError::InvalidInput)
        );
    }
    sqlx::query("update concert_notification_cursors set expires_at=clock_timestamp()-interval '1 second' where token=$1").bind(Uuid::parse_str(&token)?).execute(&f.pool).await?;
    assert_eq!(
        f.service()
            .list(
                f.context(),
                Query {
                    cursor: Some(token),
                    ..Query::default()
                }
            )
            .await,
        Err(NotificationError::InvalidInput)
    );
    Ok(())
}
#[tokio::test]
async fn single_read_is_monotone_idempotent_and_revision_guarded() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    let id = f.list().await?.items[0].notification_id;
    let key = Uuid::new_v4();
    let one = f.service().mark_read(f.context(), id, 1, key).await?;
    assert_eq!(one.revision, 2);
    assert!(one.read_at.is_some());
    assert_eq!(one, f.service().mark_read(f.context(), id, 1, key).await?);
    assert_eq!(
        f.service().mark_read(f.context(), id, 2, key).await,
        Err(NotificationError::IdempotencyConflict)
    );
    assert_eq!(
        f.service()
            .mark_read(f.context(), id, 1, Uuid::new_v4())
            .await,
        Err(NotificationError::VersionConflict)
    );
    assert_eq!(
        one,
        f.service()
            .mark_read(f.context(), id, 2, Uuid::new_v4())
            .await?
    );
    Ok(())
}
#[tokio::test]
async fn concurrent_single_and_all_reads_preserve_monotone_state() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for _ in 0..4 {
        f.insert().await?;
    }
    let id = f.list().await?.items[0].notification_id;
    let s = f.service();
    let (a, b) = tokio::join!(
        s.mark_read(f.context(), id, 1, Uuid::new_v4()),
        s.mark_all_read(f.context(), Uuid::new_v4())
    );
    assert!(a.is_ok() || b.is_ok());
    if let Err(e) = a {
        assert!(matches!(
            e,
            NotificationError::Unavailable | NotificationError::VersionConflict
        ));
    }
    if let Err(e) = b {
        assert_eq!(e, NotificationError::Unavailable);
    }
    s.mark_all_read(f.context(), Uuid::new_v4()).await?;
    assert_eq!(s.unread_count(f.context()).await?.unread_total, 0);
    assert!(f
        .list()
        .await?
        .items
        .iter()
        .all(|n| n.revision == 2 && n.read_at.is_some()));
    Ok(())
}
#[tokio::test]
async fn read_all_cutoff_receipt_replay_and_event_scope() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    let key = Uuid::new_v4();
    let first = f.service().mark_all_read(f.context(), key).await?;
    assert_eq!(first.read_count, 1);
    f.insert().await?;
    assert_eq!(first, f.service().mark_all_read(f.context(), key).await?);
    assert_eq!(f.service().unread_count(f.context()).await?.unread_total, 1);
    assert_eq!(
        f.service()
            .mark_all_read(f.context(), Uuid::new_v4())
            .await?
            .read_count,
        1
    );
    Ok(())
}
#[tokio::test]
async fn inherited_deletion_suppresses_actor_recipient_late_ingress_and_replay(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let e = f.insert().await?;
    let id = f.list().await?.items[0].notification_id;
    PgNotificationRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            f.source,
            f.source,
            Utc::now() + Duration::days(1),
        )
        .await?;
    assert!(f.list().await?.items.is_empty());
    f.service().ingest(e, vec![f.recipient()]).await?;
    f.insert().await?;
    assert!(f.list().await?.items.is_empty());
    let deleted: Option<DateTime<Utc>> =
        sqlx::query_scalar("select deleted_at from notifications where id=$1")
            .bind(id)
            .fetch_one(&f.pool)
            .await?;
    assert!(deleted.is_some());
    PgNotificationRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            f.actor,
            f.actor,
            Utc::now() + Duration::days(1),
        )
        .await?;
    assert_eq!(
        f.service().unread_count(f.context()).await,
        Err(NotificationError::PermissionDenied)
    );
    Ok(())
}
#[tokio::test]
async fn purge_tombstone_prevents_recreation() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let e = f.insert().await?;
    let id = f.list().await?.items[0].notification_id;
    sqlx::query("delete from notifications where id=$1")
        .bind(id)
        .execute(&f.pool)
        .await?;
    f.service().ingest(e, vec![f.recipient()]).await?;
    assert!(f.list().await?.items.is_empty());
    let n: i64 = sqlx::query_scalar(
        "select count(*) from concert_notification_items where notification_id=$1",
    )
    .bind(id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(n, 0);
    Ok(())
}
#[tokio::test]
async fn unknown_authority_corrupt_binding_and_deadline_fail_closed() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    assert_eq!(
        PgConcertInbox::new(f.pool.clone(), UnavailableAuthority)
            .unread_count(f.context())
            .await,
        Err(NotificationError::Unavailable)
    );
    for authority in [
        FixtureAuthority {
            corrupt: true,
            delay: false,
        },
        FixtureAuthority {
            corrupt: false,
            delay: true,
        },
    ] {
        assert_eq!(
            PgConcertInbox::new(f.pool.clone(), authority)
                .unread_count(f.context())
                .await,
            Err(NotificationError::Unavailable)
        );
    }
    let mut c = f.context();
    c.expires_at = Utc::now() - Duration::seconds(1);
    assert_eq!(
        f.service().unread_count(c).await,
        Err(NotificationError::PermissionDenied)
    );
    Ok(())
}
#[tokio::test]
async fn commit_deadline_rolls_back_read_and_receipt() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    sqlx::query("insert into be33_fixture_delay(actor_id) values($1)")
        .bind(f.actor)
        .execute(&f.pool)
        .await?;
    let mut context = f.context();
    context.expires_at = Utc::now() + Duration::milliseconds(70);
    assert!(f
        .service()
        .mark_all_read(context, Uuid::new_v4())
        .await
        .is_err());
    let n: i64 = sqlx::query_scalar(
        "select count(*) from notifications where user_id=$1 and read_at is not null",
    )
    .bind(f.actor)
    .fetch_one(&f.pool)
    .await?;
    let r: i64 = sqlx::query_scalar(
        "select count(*) from concert_notification_read_receipts where actor_id=$1",
    )
    .bind(f.actor)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!((n, r), (0, 0));
    Ok(())
}
#[tokio::test]
async fn inherited_mark_read_is_reflected_in_concert_revision() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    let id = f.list().await?.items[0].notification_id;
    PgNotificationRepository::new(f.pool.clone())
        .mark_read(f.actor, id, Utc::now())
        .await?;
    assert_eq!(f.list().await?.items[0].revision, 2);
    assert_eq!(f.service().unread_count(f.context()).await?.unread_total, 0);
    Ok(())
}
#[tokio::test]
async fn typed_wire_contract_and_unavailable_no_store() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    let value = wire::execute(&f.service(), f.context(), wire::Operation::UnreadCount).await?;
    assert_eq!(value.as_object().unwrap().len(), 4);
    assert_eq!(value["unread_total"], 1);
    let service = PgConcertInbox::new(f.pool.clone(), UnavailableAuthority);
    let response = wire::response(&service, f.context(), wire::Operation::UnreadCount).await;
    assert_eq!(response.status().as_u16(), 503);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = axum::body::to_bytes(response.into_body(), 65536).await?;
    let error: serde_json::Value = serde_json::from_slice(&body)?;
    assert!(error["timestamp"].as_str().unwrap().ends_with('Z'));
    assert_eq!(error["details"]["reason"], "DEPENDENCY_UNAVAILABLE");
    assert_eq!(wire::OPERATIONS[1].0, "getConcertNotificationUnreadCount");
    Ok(())
}

#[tokio::test]
async fn durable_restart_replays_exact_delivery_and_read_all_receipt() -> anyhow::Result<()> {
    let pool = concert_inbox_support::owned_pool("mcr_concert_notification").await?;
    let id = |n: u128| Uuid::from_u128(0x00000000000040008000000000330000 + n);
    let f = Fixture {
        pool,
        actor: id(1),
        event: id(2),
        source: id(3),
        target: id(4),
    };
    sqlx::query("insert into be33_fixture_authority(event_id,user_id,allowed) values($1,$2,true) on conflict do nothing").bind(f.event).bind(f.actor).execute(&f.pool).await?;
    sqlx::query("insert into be33_fixture_targets(event_id,target_id,visible,stale) values($1,$2,true,false) on conflict do nothing").bind(f.event).bind(f.target).execute(&f.pool).await?;
    let e = Envelope {
        outbox_id: id(5),
        receipt_id: id(6),
        event_id: f.event,
        resource_id: f.target,
        resource_revision: 1,
    };
    f.service().ingest(e, vec![f.recipient()]).await?;
    let r = f.service().mark_all_read(f.context(), id(7)).await?;
    assert_eq!(r.read_count, 1);
    let items = f.list().await?.items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].read_at, Some(r.read_at));
    assert_eq!(f.service().unread_count(f.context()).await?.unread_total, 0);
    Ok(())
}

#[tokio::test]
async fn ingress_failure_rolls_back_entire_fanout_and_tombstone() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let e = f.envelope();
    let other = Uuid::new_v4();
    sqlx::query("insert into be33_fixture_ingress_failure(user_id) values($1)")
        .bind(other)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        f.service()
            .ingest(
                e,
                vec![
                    f.recipient(),
                    Recipient {
                        user_id: other,
                        ..f.recipient()
                    }
                ]
            )
            .await,
        Err(NotificationError::Unavailable)
    );
    let n: i64 = sqlx::query_scalar("select count(*) from notifications where event_id=$1")
        .bind(e.outbox_id)
        .fetch_one(&f.pool)
        .await?;
    let d: i64 = sqlx::query_scalar(
        "select count(*) from concert_notification_deliveries where outbox_id=$1",
    )
    .bind(e.outbox_id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!((n, d), (0, 0));
    sqlx::query("delete from be33_fixture_ingress_failure where user_id=$1")
        .bind(other)
        .execute(&f.pool)
        .await?;
    f.service()
        .ingest(
            e,
            vec![
                f.recipient(),
                Recipient {
                    user_id: other,
                    ..f.recipient()
                },
            ],
        )
        .await?;
    let n: i64 = sqlx::query_scalar("select count(*) from notifications where event_id=$1")
        .bind(e.outbox_id)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(n, 2);
    Ok(())
}

#[tokio::test]
async fn ingress_racing_inherited_deletion_never_leaves_a_live_notification() -> anyhow::Result<()>
{
    let f = Fixture::new().await?;
    let e = f.envelope();
    let service = f.service();
    let legacy = PgNotificationRepository::new(f.pool.clone());
    let (a, b) = tokio::join!(
        service.ingest(e, vec![f.recipient()]),
        legacy.apply_user_deletion(
            Uuid::new_v4(),
            f.source,
            f.source,
            Utc::now() + Duration::days(1)
        )
    );
    a?;
    b?;
    let n: i64 = sqlx::query_scalar(
        "select count(*) from notifications where event_id=$1 and deleted_at is null",
    )
    .bind(e.outbox_id)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(n, 0);
    service.ingest(e, vec![f.recipient()]).await?;
    assert!(f.list().await?.items.is_empty());
    Ok(())
}

#[tokio::test]
async fn expired_read_receipt_is_retained_and_cannot_mark_new_ingress() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    f.insert().await?;
    let key = Uuid::new_v4();
    f.service().mark_all_read(f.context(), key).await?;
    sqlx::query("update concert_notification_read_receipts set expires_at=clock_timestamp()-interval '1 second' where actor_id=$1 and idempotency_key=$2").bind(f.actor).bind(key).execute(&f.pool).await?;
    f.insert().await?;
    assert_eq!(
        f.service().mark_all_read(f.context(), key).await,
        Err(NotificationError::IdempotencyExpired)
    );
    assert_eq!(f.service().unread_count(f.context()).await?.unread_total, 1);
    Ok(())
}

#[tokio::test]
async fn cursor_cannot_cross_even_authorized_actor_or_event() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for _ in 0..3 {
        f.insert().await?;
    }
    let token = f
        .service()
        .list(
            f.context(),
            Query {
                limit: 1,
                ..Query::default()
            },
        )
        .await?
        .next_cursor;
    for ctx in [
        Context {
            actor_id: Uuid::new_v4(),
            ..f.context()
        },
        Context {
            event_id: Uuid::new_v4(),
            ..f.context()
        },
    ] {
        sqlx::query(
            "insert into be33_fixture_authority(event_id,user_id,allowed) values($1,$2,true)",
        )
        .bind(ctx.event_id)
        .bind(ctx.actor_id)
        .execute(&f.pool)
        .await?;
        assert_eq!(
            f.service()
                .list(
                    ctx,
                    Query {
                        cursor: token.clone(),
                        ..Query::default()
                    }
                )
                .await,
            Err(NotificationError::InvalidInput)
        );
    }
    Ok(())
}

#[tokio::test]
async fn inherited_creation_and_concert_ingress_share_two_phase_subject_lock_order(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let mut subjects = [f.actor, f.source];
    subjects.sort_unstable();
    let [low, high] = subjects;
    sqlx::query("insert into notification_subject_states(subject_id) values($1)")
        .bind(low)
        .execute(&f.pool)
        .await?;
    let gate_key = (Uuid::new_v4().as_u128() as i64) & i64::MAX;
    sqlx::query("insert into be33_fixture_subject_gate(subject_id,gate_key) values($1,$2)")
        .bind(high)
        .bind(gate_key)
        .execute(&f.pool)
        .await?;
    let mut control = f.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(gate_key)
        .execute(&mut *control)
        .await?;
    let legacy_name = format!("be33_legacy_{}", Uuid::new_v4().simple());
    let ingress_name = format!("be33_ingress_{}", Uuid::new_v4().simple());
    let make_pool = |name: &str| {
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with((*f.pool.connect_options()).clone().application_name(name))
    };
    let legacy_pool = make_pool(&legacy_name).await?;
    let ingress_pool = make_pool(&ingress_name).await?;
    let legacy = notification_service::application::dto::repository_dto::NotificationRecordDto {
        id: Uuid::new_v4(),
        user_id: low,
        notification_type: "BE33_LEGACY_FIXTURE".into(),
        payload_json: String::new(),
        created_at: Utc::now(),
        read_at: None,
        event_id: Uuid::new_v4(),
        actor_id: Some(high),
        occurred_at: Utc::now(),
    };
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(async move {
        PgNotificationRepository::new(legacy_pool)
            .create_consistent(&legacy)
            .await?;
        Ok::<_, anyhow::Error>(())
    });
    let wait_for = |name: String, event: &'static str| {
        let p = f.pool.clone();
        async move {
            tokio::time::timeout(std::time::Duration::from_secs(3),async {
            loop {
                let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where application_name=$1 and wait_event=$2)").bind(&name).bind(event).fetch_one(&p).await?;
                if waiting {return Ok::<_,anyhow::Error>(());}
                tokio::task::yield_now().await;
            }
        }).await??;
            Ok::<_, anyhow::Error>(())
        }
    };
    wait_for(legacy_name, "advisory").await?;
    let envelope = f.envelope();
    let recipient = Recipient {
        user_id: low,
        source_actor_id: Some(high),
        ..f.recipient()
    };
    tasks.spawn(async move {
        PgConcertInbox::new(ingress_pool, UnavailableAuthority)
            .ingest(envelope, vec![recipient])
            .await?;
        Ok::<_, anyhow::Error>(())
    });
    // The legacy transaction owns the high subject insertion. Concert may wait
    // for it, but must not already own the low subject's row lock.
    wait_for(ingress_name, "transactionid").await?;
    control.rollback().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(result) = tasks.join_next().await {
            result??;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    let count: i64 =
        sqlx::query_scalar("select count(*) from notifications where user_id=$1 and actor_id=$2")
            .bind(low)
            .bind(high)
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(count, 2);
    Ok(())
}
