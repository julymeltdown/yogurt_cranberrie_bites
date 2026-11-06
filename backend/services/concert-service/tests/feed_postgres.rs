//! Injected local test authority exercises actual SQL. No live identity qualification.
#[path = "support/feed/mod.rs"]
mod support;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{commands::*, feed::*, Clock};
use concert_domain::feed::*;
use concert_service::feed::queue::*;
use sqlx::PgConnection;
use uuid::Uuid;
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
struct TestPorts;
#[async_trait]
impl FeedPolicy for TestPorts {
    async fn observe(&self, c: Context, s: Uuid, a: Action) -> Result<Observation, FeedError> {
        Ok(Observation {
            context: c,
            subject: s,
            action: a,
            event_revision: 1,
            relationship_revision: 1,
            auth_actor_revision: 1,
            auth_subject_revision: 1,
            friend_revision: 1,
            valid_until: c.expires_at,
        })
    }
    async fn fence(&self, _: Observation) -> Result<(), FeedError> {
        Ok(())
    }
}
#[async_trait]
impl PostReader for TestPorts {
    async fn page(
        &self,
        _: Context,
        _: Window,
        _: Filter,
        _: u16,
    ) -> Result<ScannedPosts, FeedError> {
        Ok(ScannedPosts {
            items: vec![],
            scanned_through: None,
            has_more: false,
        })
    }
    async fn post(&self, c: Context, n: Uuid) -> Result<Option<Post>, FeedError> {
        Ok(Some(Post {
            id: n,
            event: c.event,
            author: c.actor,
            content: Content {
                text: "Owned fixture post".into(),
                media_ids: vec![],
                links: vec![],
            },
            state: ContentState::Published,
            public: true,
            revision: 1,
            created_at: DateTime::from_timestamp(1793710000, 0).unwrap(),
            edited_at: None,
            comment_count: 0,
            reaction_count: 0,
            reacted_by_me: false,
        }))
    }
}
#[async_trait]
impl CommentReader for TestPorts {
    async fn comment(&self, _: Context, _: Uuid) -> Result<Option<Comment>, FeedError> {
        Ok(None)
    }
    async fn comments(
        &self,
        _: Context,
        _: Uuid,
        _: Window,
        _: u16,
    ) -> Result<ScannedComments, FeedError> {
        Ok(ScannedComments {
            items: vec![],
            scanned_through: None,
            has_more: false,
        })
    }
}
#[async_trait]
impl FeedResources for TestPorts {
    async fn author(&self, _: Context, n: Uuid) -> Result<PublicAuthor, FeedError> {
        Ok(PublicAuthor {
            user_id: n,
            json: b"{}".to_vec(),
        })
    }
    async fn public_link(&self, _: Context, _: ResourceLink) -> Result<bool, FeedError> {
        Ok(false)
    }
    async fn public_media(&self, _: Context, _: Uuid, _: Uuid) -> Result<bool, FeedError> {
        Ok(false)
    }
}
struct TestFence;
#[async_trait]
impl FencedFeedAuthority for TestFence {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        p: &PreparedCommand,
    ) -> Result<(), CommandError> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from feed_test_policy where actor=$1 and event=$2 for share",
        )
        .bind(p.context().actor)
        .bind(p.context().event)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn stored_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        _: Scope,
        _: Uuid,
        _: Option<Uuid>,
    ) -> Result<(), CommandError> {
        let allowed: Option<bool> = sqlx::query_scalar(
            "select allowed from feed_test_policy where actor=$1 and event=$2 for share",
        )
        .bind(context.actor)
        .bind(context.event)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
    async fn first_execution_in(
        &self,
        c: &mut PgConnection,
        p: &PreparedCommand,
    ) -> Result<(), CommandError> {
        self.check_in(c, p).await
    }
}

async fn intent(c: Context, key: Uuid, text: &str) -> PreparedCommand {
    Feed {
        reader: TestPorts,
        policy: TestPorts,
        resources: TestPorts,
        clock: Fixed,
    }
    .prepare(
        c,
        key,
        FeedCommand::CreatePost {
            content: Content {
                text: text.into(),
                media_ids: vec![],
                links: vec![],
            },
        },
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn pending_work_replay_conflict_fences_owner_ack_outbox_and_rollback() -> anyhow::Result<()> {
    let pool = support::owned_pool().await?;
    sqlx::query("create table feed_test_policy(actor uuid,event uuid,allowed boolean,primary key(actor,event))").execute(&pool).await?;
    let c = Context {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        session_family: Uuid::new_v4(),
        request: Uuid::new_v4(),
        authority_digest: [9; 32],
        expires_at: Utc::now() + chrono::Duration::seconds(30),
    };
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,'feed-owned-test','Feed test','OPEN')").bind(c.event).execute(&pool).await?;
    sqlx::query("insert into feed_test_policy values($1,$2,true)")
        .bind(c.actor)
        .bind(c.event)
        .execute(&pool)
        .await?;
    let keys = FingerprintKeys::new(SigningKey::new("native-test", &[41; 32])?, vec![])?;
    let queue = FeedQueue::new(pool.clone());
    let key = Uuid::new_v4();
    let command = intent(c, key, "private-work-text").await;
    assert!(matches!(
        queue
            .enqueue(&command, &keys, &UnavailableFeedAuthority)
            .await,
        Err(CommandError::Unavailable)
    ));
    let count: i64 = sqlx::query_scalar("select count(*) from concert_command_receipts")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);
    let receipt = queue.enqueue(&command, &keys, &TestFence).await?;
    assert_eq!(receipt.state, ReceiptState::Pending);
    assert!(receipt.reference.is_none());
    let repeat = queue.enqueue(&command, &keys, &TestFence).await?;
    assert_eq!(receipt.id, repeat.id);
    let changed = intent(c, key, "changed input").await;
    assert!(matches!(
        queue.enqueue(&changed, &keys, &TestFence).await,
        Err(CommandError::Conflict)
    ));
    let jobs: i64 = sqlx::query_scalar("select count(*) from concert_feed_owner_work")
        .fetch_one(&pool)
        .await?;
    assert_eq!(jobs, 1);
    let outbox: i64 = sqlx::query_scalar("select count(*) from concert_outbox")
        .fetch_one(&pool)
        .await?;
    assert_eq!(outbox, 0);
    use concert_service::feed::owner::{PostOwner, UnavailablePostTransport};
    assert!(matches!(
        PostOwner {
            transport: UnavailablePostTransport
        }
        .execute(&command, &receipt)
        .await,
        Err(FeedError::DependencyUnavailable)
    ));
    let replacement = intent(
        Context {
            request: Uuid::new_v4(),
            ..c
        },
        key,
        "private-work-text",
    )
    .await;
    assert!(matches!(
        queue
            .acknowledge(
                &replacement,
                OwnerCommit {
                    scope: command.scope()?,
                    resource: command.target(),
                    revision: 1
                },
                &TestFence
            )
            .await,
        Err(CommandError::Denied)
    ));
    let commit = PostOwner {
        transport: SimulatedOwner { wrong_actor: false },
    }
    .execute(&command, &receipt)
    .await?;
    assert!(matches!(
        PostOwner {
            transport: SimulatedOwner { wrong_actor: true }
        }
        .execute(&command, &receipt)
        .await,
        Err(FeedError::DependencyUnavailable)
    ));
    assert!(matches!(
        queue
            .acknowledge(
                &command,
                OwnerCommit {
                    resource: Uuid::new_v4(),
                    ..commit
                },
                &TestFence
            )
            .await,
        Err(CommandError::Denied)
    ));
    sqlx::query("update feed_test_policy set allowed=false")
        .execute(&pool)
        .await?;
    assert!(matches!(
        queue.acknowledge(&command, commit, &TestFence).await,
        Err(CommandError::Denied)
    ));
    assert!(matches!(
        queue.enqueue(&command, &keys, &TestFence).await,
        Err(CommandError::Denied)
    ));
    sqlx::query("update feed_test_policy set allowed=true")
        .execute(&pool)
        .await?;
    queue.acknowledge(&command, commit, &TestFence).await?;
    queue.acknowledge(&command, commit, &TestFence).await?;
    let replay = queue.enqueue(&command, &keys, &TestFence).await?;
    assert_eq!(replay.state, ReceiptState::Succeeded);
    assert_eq!(replay.result_status, Some(200));
    assert_eq!(replay.reference.unwrap().resource, command.target());
    let stored = queue
        .load(command.scope()?)
        .await?
        .ok_or_else(|| anyhow::anyhow!("acknowledged work must remain reloadable"))?;
    assert_eq!(stored.target, command.target());
    assert!(stored.command == *command.command());
    assert_eq!(stored.receipt.state, ReceiptState::Succeeded);
    assert_eq!(stored.receipt.result_status, Some(200));
    assert_eq!(stored.receipt.reference.unwrap().resource, command.target());
    assert_eq!(stored.parent, command.parent());
    assert_eq!(stored.parent_revision, command.parent_revision());
    let outbox: i64 = sqlx::query_scalar("select count(*) from concert_outbox")
        .fetch_one(&pool)
        .await?;
    assert_eq!(outbox, 1);
    let leak:i64=sqlx::query_scalar("select count(*) from concert_outbox where to_jsonb(concert_outbox)::text like '%private-work-text%'").fetch_one(&pool).await?;
    assert_eq!(leak, 0);
    let rollback = intent(c, Uuid::new_v4(), "rollback only").await;
    // Force a late work-table failure after BE15 has tentatively inserted receipt.
    sqlx::query(&format!("alter table concert_feed_owner_work add constraint feed_test_reject check(target_id<>'{}'::uuid)",rollback.target())).execute(&pool).await?;
    assert!(matches!(
        queue.enqueue(&rollback, &keys, &TestFence).await,
        Err(CommandError::Unavailable)
    ));
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where idempotency_key=$1",
    )
    .bind(rollback.scope()?.key)
    .fetch_one(&pool)
    .await?;
    assert_eq!(rows, 0);
    let budget: i64 = sqlx::query_scalar("select receipts_used from concert_command_budget")
        .fetch_one(&pool)
        .await?;
    assert_eq!(budget, 1);
    sqlx::query("alter table concert_feed_owner_work drop constraint feed_test_reject")
        .execute(&pool)
        .await?;
    let pending = queue.enqueue(&rollback, &keys, &TestFence).await?;
    assert_eq!(pending.state, ReceiptState::Pending);
    let edit_key = Uuid::new_v4();
    let edit = Feed {
        reader: TestPorts,
        policy: TestPorts,
        resources: TestPorts,
        clock: Fixed,
    }
    .prepare(
        c,
        edit_key,
        FeedCommand::EditPost {
            post: Uuid::from_u128(12),
            expected_revision: 1,
            content: Content {
                text: "Edit".into(),
                media_ids: vec![],
                links: vec![],
            },
        },
    )
    .await?;
    queue.enqueue(&edit, &keys, &TestFence).await?;
    let other = Feed {
        reader: TestPorts,
        policy: TestPorts,
        resources: TestPorts,
        clock: Fixed,
    }
    .prepare(
        c,
        edit_key,
        FeedCommand::EditPost {
            post: Uuid::from_u128(13),
            expected_revision: 1,
            content: Content {
                text: "Edit".into(),
                media_ids: vec![],
                links: vec![],
            },
        },
    )
    .await?;
    assert!(matches!(
        queue.enqueue(&other, &keys, &TestFence).await,
        Err(CommandError::Conflict)
    ));
    let expired = Feed {
        reader: TestPorts,
        policy: TestPorts,
        resources: TestPorts,
        clock: Fixed,
    }
    .prepare(
        Context {
            expires_at: Utc::now() + chrono::Duration::seconds(1),
            ..c
        },
        Uuid::new_v4(),
        FeedCommand::CreatePost {
            content: Content {
                text: "Expires before ack".into(),
                media_ids: vec![],
                links: vec![],
            },
        },
    )
    .await?;
    let exp_receipt = queue.enqueue(&expired, &keys, &TestFence).await?;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert!(matches!(
        queue
            .acknowledge(
                &expired,
                OwnerCommit {
                    scope: expired.scope()?,
                    resource: expired.target(),
                    revision: 1
                },
                &TestFence
            )
            .await,
        Err(CommandError::Denied)
    ));
    let state: String =
        sqlx::query_scalar("select state from concert_command_receipts where receipt_id=$1")
            .bind(exp_receipt.id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(state, "PENDING");
    pool.close().await;
    Ok(())
}

/// DTO adapter fixture only: no claim that this response is a real Post commit.
struct SimulatedOwner {
    wrong_actor: bool,
}
#[async_trait]
impl concert_service::feed::owner::PostTransport for SimulatedOwner {
    async fn execute(
        &self,
        r: concert_service::feed::owner::proto::ExecuteCommandRequest,
    ) -> Result<concert_service::feed::owner::proto::Receipt, tonic::Status> {
        use concert_service::feed::owner::proto;
        let ctx = r.context.unwrap();
        let c = r.command.unwrap();
        assert_eq!(ctx.actor_id, c.actor_id);
        assert_eq!(ctx.event_id, c.event_id);
        assert_eq!(ctx.original_authority_digest, vec![9; 32]);
        assert!(matches!(
            c.mutation,
            Some(proto::command::Mutation::CreatePost(_))
        ));
        Ok(proto::Receipt {
            operation_id: c.operation_id,
            event_id: c.event_id,
            post_id: c.post_id,
            comment_id: None,
            revision: 1,
            post_revision: 1,
            deleted: false,
            reacted_by_me: false,
            reaction_count: 0,
            committed_at: Utc::now().to_rfc3339(),
            actor_id: if self.wrong_actor {
                Uuid::new_v4().to_string()
            } else {
                c.actor_id
            },
            idempotency_key: c.idempotency_key,
        })
    }
}
