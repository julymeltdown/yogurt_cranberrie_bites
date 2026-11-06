//! Current public-source revision and removal checks in an owned PG fixture.
#[path = "support/event_lifecycle/mod.rs"]
mod support;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{commands::CommandError, event_lifecycle::CloseCommand};
use concert_domain::event_lifecycle::JobState;
use concert_service::event_lifecycle::{archive::*, *};
use sqlx::{PgConnection, Row};
use support::*;
use uuid::Uuid;
struct PublicFixture;
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
#[async_trait]
impl PublicArchiveOwner for PublicFixture {
    async fn page_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        upper: DateTime<Utc>,
        after: Option<Position>,
        limit: u16,
    ) -> Result<PublicPage, CommandError> {
        let source_revision = sqlx::query_scalar(
            "select revision from be41_public_revision where event_id=$1 for share",
        )
        .bind(event)
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
        let rows=sqlx::query("select post_id,text,created_at from be41_public_posts where event_id=$1 and permitted and created_at<=$2 and ($3::timestamptz is null or (created_at,post_id)<($3,$4)) order by created_at desc,post_id desc limit $5")
            .bind(event).bind(upper).bind(after.map(|p|p.created_at)).bind(after.map(|p|p.post_id)).bind(i64::from(limit)+1).fetch_all(c).await.map_err(db)?;
        let has_more = rows.len() > usize::from(limit);
        let items = rows
            .into_iter()
            .take(usize::from(limit))
            .map(|r| ArchivePost {
                post_id: r.get("post_id"),
                text: r.get("text"),
                media_ids: vec![],
                created_at: r.get("created_at"),
            })
            .collect();
        Ok(PublicPage {
            source_revision,
            items,
            has_more,
        })
    }
    async fn fence_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        revision: i64,
    ) -> Result<(), CommandError> {
        let actual: i64 = sqlx::query_scalar(
            "select revision from be41_public_revision where event_id=$1 for share",
        )
        .bind(event)
        .fetch_one(c)
        .await
        .map_err(db)?;
        if actual == revision {
            Ok(())
        } else {
            Err(CommandError::Unavailable)
        }
    }
}
#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn event_lifecycle_archive_deletion_invalidates_cursor_and_never_reintroduces_content() {
    let pool = pool().await;
    let actor = actor(&pool).await;
    let lifecycle = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        Owners::healthy(),
    );
    let job = lifecycle
        .command(
            actor,
            Uuid::new_v4(),
            CloseCommand::Start {
                expected_revision: 1,
                reason: "Archive fixture".into(),
            },
            &keys(),
        )
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource;
    while lifecycle.job(actor, job).await.unwrap().state != JobState::Completed {
        lifecycle.run_next(actor, job).await.unwrap();
    }
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS be41_public_revision(event_id uuid PRIMARY KEY,revision bigint);CREATE TABLE IF NOT EXISTS be41_public_posts(event_id uuid,post_id uuid PRIMARY KEY,text text,created_at timestamptz,permitted boolean);").execute(&pool).await.unwrap();
    sqlx::query("insert into be41_public_revision values($1,1)")
        .bind(actor.event)
        .execute(&pool)
        .await
        .unwrap();
    let at = Utc::now() - Duration::seconds(1);
    let ids = [
        Uuid::from_u128(100),
        Uuid::from_u128(200),
        Uuid::from_u128(300),
    ];
    for id in ids {
        sqlx::query(
            "insert into be41_public_posts values($1,$2,'Reviewed public fixture',$3,true)",
        )
        .bind(actor.event)
        .bind(id)
        .bind(at)
        .execute(&pool)
        .await
        .unwrap();
    }
    let mural = ArchiveMural {
        snapshot_id: Uuid::new_v4(),
        event_id: actor.event,
        revision: 9,
        state: "FROZEN".into(),
        width: 128,
        height: 128,
        palette: vec!["#000000".into(); 16],
        pixels: vec![0; 16384],
        created_at: at,
        server_time: at,
        cooldown_ms: 0,
        next_allowed_at: None,
    };
    sqlx::query("insert into concert_closure_archives(event_id,job_id,source_revision,final_mural) values($1,$2,1,$3)").bind(actor.event).bind(job).bind(serde_json::to_value(mural).unwrap()).execute(&pool).await.unwrap();
    let archive = PgPublicArchive::new(
        pool.clone(),
        PublicFixture,
        ArchiveCursor::new(&[0x41; 32]).unwrap(),
    );
    let first = archive.page(actor.event, 2, None).await.unwrap();
    assert_eq!(
        first
            .feed
            .items
            .iter()
            .map(|p| p.post_id)
            .collect::<Vec<_>>(),
        vec![ids[2], ids[1]]
    );
    let token = first.feed.next_cursor.unwrap();
    let second = archive.page(actor.event, 2, Some(&token)).await.unwrap();
    assert_eq!(second.feed.items[0].post_id, ids[0]);
    assert!(second.feed.next_cursor.is_none());
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("update be41_public_revision set revision=revision+1 where event_id=$1")
        .bind(actor.event)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("update be41_public_posts set permitted=false where post_id=$1")
        .bind(ids[1])
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(
        archive.page(actor.event, 2, Some(&token)).await,
        Err(CommandError::Conflict)
    ));
    let refreshed = archive.page(actor.event, 2, None).await.unwrap();
    assert_eq!(refreshed.revision, first.revision + 1);
    assert!(!refreshed.feed.items.iter().any(|p| p.post_id == ids[1]));
    let value = serde_json::to_value(refreshed).unwrap();
    concert_service::event_lifecycle::http::validate("Domain_admin_Archive", &value).unwrap();
    for forbidden in ["author", "handle", "beacon", "recipient", "reacted_by_me"] {
        assert!(!value.to_string().contains(forbidden));
    }
}

#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn operator_role_fence_is_event_scoped_revisioned_and_revocation_aware() {
    use concert_service::config::closure_runtime::RootOperatorAuthority;
    let pool = pool().await;
    let operator = actor(&pool).await;
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS concert_event_operator_roles(event_id uuid NOT NULL,user_id uuid NOT NULL,role text NOT NULL,revision bigint NOT NULL DEFAULT 1,granted_by uuid NOT NULL,granted_at timestamptz NOT NULL DEFAULT clock_timestamp(),revoked_at timestamptz,PRIMARY KEY(event_id,user_id,role))").execute(&pool).await.unwrap();
    let authority = RootOperatorAuthority::new(24 * 30, 7);
    let query = AuthorityQuery {
        actor: operator,
        job: Uuid::new_v4(),
        access: Access::Start,
    };
    let mut tx = pool.begin().await.unwrap();
    assert!(authority
        .check_in(&mut tx, query, Utc::now())
        .await
        .is_err());
    tx.rollback().await.unwrap();
    sqlx::query("insert into concert_event_operator_roles(event_id,user_id,role,revision,granted_by) values($1,$2,'OPERATOR',3,$2)")
        .bind(operator.event)
        .bind(operator.actor)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let at = Utc::now();
    let fence = authority.check_in(&mut tx, query, at).await.unwrap();
    assert_eq!(fence.role_revision, 3);
    assert_eq!(fence.auth_revision, 3);
    assert_eq!(fence.policy_revision, 7);
    assert!(fence.expires_at > at);
    let retention = authority.retention_in(&mut tx, operator, at).await.unwrap();
    assert_eq!(retention.closed_at, at);
    assert_eq!(retention.policy_revision, 7);
    assert_eq!(retention.cleanup_after, at + Duration::days(30));
    tx.rollback().await.unwrap();
    let member = actor(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    assert!(authority
        .check_in(
            &mut tx,
            AuthorityQuery {
                actor: member,
                job: query.job,
                access: Access::Start,
            },
            Utc::now(),
        )
        .await
        .is_err());
    tx.rollback().await.unwrap();
    sqlx::query("update concert_event_operator_roles set revoked_at=clock_timestamp() where event_id=$1 and user_id=$2")
        .bind(operator.event)
        .bind(operator.actor)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(authority
        .check_in(&mut tx, query, Utc::now())
        .await
        .is_err());
    tx.rollback().await.unwrap();
}

#[tokio::test]
#[ignore = "requires event_lifecycle_pg.py owned fixture"]
async fn public_post_archive_projects_only_current_permitted_content() {
    use concert_service::config::closure_runtime::PostPublicArchive;
    let pool = pool().await;
    let actor = actor(&pool).await;
    let lifecycle = PgEventLifecycle::new(
        pool.clone(),
        Authority {
            delay: Duration::zero(),
        },
        Owners::healthy(),
    );
    let job = lifecycle
        .command(
            actor,
            Uuid::new_v4(),
            CloseCommand::Start {
                expected_revision: 1,
                reason: "Post archive fixture".into(),
            },
            &keys(),
        )
        .await
        .unwrap()
        .reference
        .unwrap()
        .resource;
    while lifecycle.job(actor, job).await.unwrap().state != JobState::Completed {
        lifecycle.run_next(actor, job).await.unwrap();
    }
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS post_concert_posts(post_id uuid PRIMARY KEY,event_id uuid NOT NULL,author_id uuid NOT NULL,content jsonb NOT NULL,revision bigint NOT NULL,state text NOT NULL,created_at timestamptz NOT NULL,edited_at timestamptz,comment_count bigint NOT NULL DEFAULT 0,reaction_count bigint NOT NULL DEFAULT 0)").execute(&pool).await.unwrap();
    let mural = serde_json::json!({"snapshot_id":Uuid::new_v4(),"event_id":actor.event,"revision":1,"state":"FROZEN","width":128,"height":128,"palette":vec!["#000000";16],"pixels":vec![0;16384],"created_at":Utc::now(),"server_time":Utc::now(),"cooldown_ms":0,"next_allowed_at":null});
    sqlx::query("insert into concert_closure_archives(event_id,job_id,source_revision,final_mural) values($1,$2,1,$3)")
        .bind(actor.event).bind(job).bind(mural.clone()).execute(&pool).await.unwrap();
    let at = Utc::now() - Duration::minutes(2);
    let ids = [
        Uuid::from_u128(11),
        Uuid::from_u128(12),
        Uuid::from_u128(13),
    ];
    for (index, id) in ids.iter().enumerate() {
        let content = serde_json::json!({"text":format!("Reviewed public post {index}"),"media_ids":[Uuid::from_u128(90+index as u128)],"handle":"@private"});
        sqlx::query("insert into post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at) values($1,$2,$3,$4,1,'PUBLISHED',$5)")
            .bind(id).bind(actor.event).bind(Uuid::new_v4()).bind(content).bind(at + Duration::seconds(index as i64))
            .execute(&pool).await.unwrap();
    }
    sqlx::query(
        "update post_concert_posts set state='DELETED',revision=revision+1 where post_id=$1",
    )
    .bind(ids[1])
    .execute(&pool)
    .await
    .unwrap();
    let archive = PgPublicArchive::new(
        pool.clone(),
        PostPublicArchive,
        ArchiveCursor::new(&[0x42; 32]).unwrap(),
    );
    let first = archive.page(actor.event, 1, None).await.unwrap();
    assert_eq!(first.feed.items.len(), 1);
    assert_eq!(first.feed.items[0].post_id, ids[2]);
    assert!(first.feed.items[0].media_ids.is_empty());
    assert!(!first.feed.items[0].text.contains("private"));
    let token = first.feed.next_cursor.clone().unwrap();
    let second = archive.page(actor.event, 1, Some(&token)).await.unwrap();
    assert_eq!(second.feed.items[0].post_id, ids[0]);
    assert!(second.feed.next_cursor.is_none());
    assert!(!second.feed.items.iter().any(|post| post.post_id == ids[1]));
    sqlx::query("update post_concert_posts set content=jsonb_set(content,'{text}',to_jsonb('Edited public post'::text)),revision=revision+1 where post_id=$1")
        .bind(ids[0]).execute(&pool).await.unwrap();
    assert!(matches!(
        archive.page(actor.event, 1, Some(&token)).await,
        Err(CommandError::Conflict)
    ));
    let refreshed = archive.page(actor.event, 1, None).await.unwrap();
    assert_eq!(refreshed.revision, first.revision + 1);
    let value = serde_json::to_value(refreshed).unwrap();
    concert_service::event_lifecycle::http::validate("Domain_admin_Archive", &value).unwrap();
    for forbidden in ["handle", "author", "media_ids\":[9", "recipient"] {
        assert!(!value.to_string().contains(forbidden));
    }
}
