use super::mutation_receipts;
use crate::application::dto::friend_dto::{FollowActionDto, FollowEdgeDto};
use crate::application::dto::mutation_dto::{
    MutationKind, MutationOperationDto, MutationOutcomeDto, MutationReceiptStatusDto,
};
use crate::application::ports::{
    FollowRepositoryPort, FollowRequestRepositoryPort, MutationReceiptMaintenancePort,
    RelationshipPolicyRepositoryPort, UserLifecycleRepositoryPort,
};
use crate::domain::error::FriendError;
use crate::domain::follow_counts::FollowCounts;
use crate::domain::follow_edge::{FollowCursor, FollowEdge};
use crate::domain::follow_request::{
    AcceptFollowRequestOutcome, FollowRequest, FollowRequestStatus,
};
use crate::domain::relationship_policy::RelationshipPolicy;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgFollowRepository {
    pool: PgPool,
}

impl PgFollowRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl MutationReceiptMaintenancePort for PgFollowRepository {
    async fn purge_expired_outcomes(&self, limit: i32) -> Result<i64> {
        mutation_receipts::purge_expired_outcomes(&self.pool, limit).await
    }
}

#[async_trait]
impl FollowRepositoryPort for PgFollowRepository {
    async fn mutation_receipt_status(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        mutation_receipts::status(&self.pool, operation_id).await
    }

    async fn lookup_mutation(
        &self,
        operation: &MutationOperationDto,
    ) -> Result<Option<FollowActionDto>> {
        mutation_receipts::lookup(&mut *self.pool.acquire().await?, operation).await
    }

    async fn follow_with_receipt(
        &self,
        edge: &FollowEdge,
        create_request: bool,
        operation: &MutationOperationDto,
    ) -> Result<FollowActionDto> {
        anyhow::ensure!(operation.kind == MutationKind::Follow);
        let mut tx = self.pool.begin().await?;
        if let Some(action) = mutation_receipts::claim(&mut tx, operation).await? {
            tx.commit().await?;
            return Ok(action);
        }

        let blocked: bool = sqlx::query_scalar(
            "select exists(select 1 from user_blocks where deleted_at is null and ((blocker_id=$1 and blocked_id=$2) or (blocker_id=$2 and blocked_id=$1)))",
        )
        .bind(edge.follower_id)
        .bind(edge.followee_id)
        .fetch_one(&mut *tx)
        .await?;
        if blocked {
            tx.rollback().await?;
            return Err(FriendError::PermissionDenied("Blocked relationship".into()).into());
        }

        let action = if create_request {
            let row = sqlx::query(
                r#"
                insert into follow_requests (id, follower_id, followee_id, status, created_at)
                values ($1, $2, $3, 'PENDING', $4)
                on conflict (follower_id, followee_id) where status = 'PENDING'
                do update set created_at = follow_requests.created_at
                returning id, status
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(edge.follower_id)
            .bind(edge.followee_id)
            .bind(edge.created_at)
            .fetch_one(&mut *tx)
            .await?;
            FollowActionDto {
                edge: None,
                following: false,
                status: row.get("status"),
                request_id: Some(row.get::<Uuid, _>("id").to_string()),
            }
        } else {
            let row = sqlx::query(
                r#"
                insert into follows (follower_id, followee_id, created_at)
                values ($1, $2, $3)
                on conflict (follower_id, followee_id) do update
                set created_at = case
                        when follows.deleted_at is null then follows.created_at
                        else excluded.created_at
                    end,
                    deleted_at = null,
                    deleted_by = null,
                    delete_reason = null,
                    retention_until = null
                returning follower_id, followee_id, created_at
                "#,
            )
            .bind(edge.follower_id)
            .bind(edge.followee_id)
            .bind(edge.created_at)
            .fetch_one(&mut *tx)
            .await?;
            FollowActionDto {
                edge: Some(FollowEdgeDto {
                    follower_id: row.get::<Uuid, _>("follower_id").to_string(),
                    followee_id: row.get::<Uuid, _>("followee_id").to_string(),
                    created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
                }),
                following: true,
                status: "ACTIVE".into(),
                request_id: None,
            }
        };
        mutation_receipts::complete(
            &mut tx,
            operation,
            MutationOutcomeDto::Follow(action.clone()),
        )
        .await?;
        tx.commit().await?;
        Ok(action)
    }

    async fn unfollow_with_receipt(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
        operation: &MutationOperationDto,
    ) -> Result<FollowActionDto> {
        anyhow::ensure!(operation.kind == MutationKind::Unfollow);
        let mut tx = self.pool.begin().await?;
        if let Some(action) = mutation_receipts::claim(&mut tx, operation).await? {
            tx.commit().await?;
            return Ok(action);
        }
        sqlx::query(
            r#"
            update follows
            set deleted_at = now(),
                deleted_by = $1,
                delete_reason = 'unfollow',
                retention_until = now() + interval '30 days'
            where follower_id = $1
              and followee_id = $2
              and deleted_at is null
            "#,
        )
        .bind(follower_id)
        .bind(followee_id)
        .execute(&mut *tx)
        .await?;
        let action = FollowActionDto {
            edge: None,
            following: false,
            status: "INACTIVE".into(),
            request_id: None,
        };
        mutation_receipts::complete(
            &mut tx,
            operation,
            MutationOutcomeDto::Unfollow(action.clone()),
        )
        .await?;
        tx.commit().await?;
        Ok(action)
    }

    async fn follow(&self, edge: &FollowEdge) -> Result<FollowEdge> {
        let inserted = sqlx::query(
            r#"
            insert into follows (follower_id, followee_id, created_at)
            values ($1, $2, $3)
            on conflict (follower_id, followee_id) do update
            set created_at = case
                    when follows.deleted_at is null then follows.created_at
                    else excluded.created_at
                end,
                deleted_at = null,
                deleted_by = null,
                delete_reason = null,
                retention_until = null
            returning follower_id, followee_id, created_at
            "#,
        )
        .bind(edge.follower_id)
        .bind(edge.followee_id)
        .bind(edge.created_at)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = inserted {
            return Ok(FollowEdge {
                follower_id: row.get("follower_id"),
                followee_id: row.get("followee_id"),
                created_at: row.get("created_at"),
            });
        }

        let row = sqlx::query(
            "select follower_id, followee_id, created_at from follows where follower_id = $1 and followee_id = $2",
        )
        .bind(edge.follower_id)
        .bind(edge.followee_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(FollowEdge {
            follower_id: row.get("follower_id"),
            followee_id: row.get("followee_id"),
            created_at: row.get("created_at"),
        })
    }

    async fn unfollow(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            update follows
            set deleted_at = now(),
                deleted_by = $1,
                delete_reason = 'unfollow',
                retention_until = now() + interval '30 days'
            where follower_id = $1
              and followee_id = $2
              and deleted_at is null
            "#,
        )
        .bind(follower_id)
        .bind(followee_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn list_followers(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)> {
        let total_count: i64 = sqlx::query_scalar(
            r#"
            select count(*) from follows f
            where f.followee_id = $1 and f.deleted_at is null
              and not exists (select 1 from user_blocks b where b.deleted_at is null and ((b.blocker_id=f.follower_id and b.blocked_id=f.followee_id) or (b.blocker_id=f.followee_id and b.blocked_id=f.follower_id)))
              and not exists (select 1 from user_mutes m where m.deleted_at is null and m.muter_id=f.follower_id and m.muted_id=f.followee_id)
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            r#"
            select follower_id, followee_id, created_at
            from follows
            where followee_id = $1
              and deleted_at is null
              and not exists (select 1 from user_blocks b where b.deleted_at is null and ((b.blocker_id=follows.follower_id and b.blocked_id=follows.followee_id) or (b.blocker_id=follows.followee_id and b.blocked_id=follows.follower_id)))
              and not exists (select 1 from user_mutes m where m.deleted_at is null and m.muter_id=follows.follower_id and m.muted_id=follows.followee_id)
              and (
                $2::timestamptz is null
                or (created_at, follower_id) < ($2, $3)
              )
            order by created_at desc, follower_id desc
            limit $4
            "#,
        )
        .bind(user_id)
        .bind(cursor.map(|value| value.created_at))
        .bind(cursor.map(|value| value.tie_breaker_id))
        .bind(i64::from(limit.max(1)) + 1)
        .fetch_all(&self.pool)
        .await?;
        let mut edges: Vec<FollowEdge> = rows
            .into_iter()
            .map(|row| FollowEdge {
                follower_id: row.get("follower_id"),
                followee_id: row.get("followee_id"),
                created_at: row.get("created_at"),
            })
            .collect();
        let next_cursor = if edges.len() > limit as usize {
            edges
                .get(limit as usize - 1)
                .map(|edge| FollowCursor::new(edge.created_at, edge.follower_id).encode())
        } else {
            None
        };
        if edges.len() > limit as usize {
            edges.truncate(limit as usize);
        }
        Ok((edges, next_cursor, total_count))
    }

    async fn list_following(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowEdge>, Option<String>, i64)> {
        let total_count: i64 = sqlx::query_scalar(
            r#"
            select count(*) from follows f
            where f.follower_id = $1 and f.deleted_at is null
              and not exists (select 1 from user_blocks b where b.deleted_at is null and ((b.blocker_id=f.follower_id and b.blocked_id=f.followee_id) or (b.blocker_id=f.followee_id and b.blocked_id=f.follower_id)))
              and not exists (select 1 from user_mutes m where m.deleted_at is null and m.muter_id=f.follower_id and m.muted_id=f.followee_id)
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            r#"
            select follower_id, followee_id, created_at
            from follows
            where follower_id = $1
              and deleted_at is null
              and not exists (select 1 from user_blocks b where b.deleted_at is null and ((b.blocker_id=follows.follower_id and b.blocked_id=follows.followee_id) or (b.blocker_id=follows.followee_id and b.blocked_id=follows.follower_id)))
              and not exists (select 1 from user_mutes m where m.deleted_at is null and m.muter_id=follows.follower_id and m.muted_id=follows.followee_id)
              and (
                $2::timestamptz is null
                or (created_at, followee_id) < ($2, $3)
              )
            order by created_at desc, followee_id desc
            limit $4
            "#,
        )
        .bind(user_id)
        .bind(cursor.map(|value| value.created_at))
        .bind(cursor.map(|value| value.tie_breaker_id))
        .bind(i64::from(limit.max(1)) + 1)
        .fetch_all(&self.pool)
        .await?;
        let mut edges: Vec<FollowEdge> = rows
            .into_iter()
            .map(|row| FollowEdge {
                follower_id: row.get("follower_id"),
                followee_id: row.get("followee_id"),
                created_at: row.get("created_at"),
            })
            .collect();
        let next_cursor = if edges.len() > limit as usize {
            edges
                .get(limit as usize - 1)
                .map(|edge| FollowCursor::new(edge.created_at, edge.followee_id).encode())
        } else {
            None
        };
        if edges.len() > limit as usize {
            edges.truncate(limit as usize);
        }
        Ok((edges, next_cursor, total_count))
    }

    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        let exists: Option<i32> =
            sqlx::query_scalar(
                r#"
                select 1 from follows f
                where f.follower_id = $1 and f.followee_id = $2 and f.deleted_at is null
                  and not exists (select 1 from user_blocks b where b.deleted_at is null and ((b.blocker_id=$1 and b.blocked_id=$2) or (b.blocker_id=$2 and b.blocked_id=$1)))
                "#,
            )
            .bind(follower_id)
            .bind(followee_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(exists.is_some())
    }

    async fn counts(&self, user_id: Uuid) -> Result<FollowCounts> {
        let row = sqlx::query(
            r#"
            select
              (select count(*) from follows where followee_id = $1 and deleted_at is null)::bigint
                as follower_count,
              (select count(*) from follows where follower_id = $1 and deleted_at is null)::bigint
                as following_count
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(FollowCounts {
            follower_count: row.get("follower_count"),
            following_count: row.get("following_count"),
        })
    }

    async fn batch_counts(&self, user_ids: &[Uuid]) -> Result<HashMap<Uuid, FollowCounts>> {
        if user_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut output = user_ids
            .iter()
            .copied()
            .map(|user_id| (user_id, FollowCounts::default()))
            .collect::<HashMap<_, _>>();
        let rows = sqlx::query(
            r#"
            select user_id,
                   sum(follower_count)::bigint as follower_count,
                   sum(following_count)::bigint as following_count
            from (
              select followee_id as user_id, count(*)::bigint as follower_count, 0::bigint as following_count
              from follows
              where followee_id = any($1) and deleted_at is null
              group by followee_id
              union all
              select follower_id as user_id, 0::bigint as follower_count, count(*)::bigint as following_count
              from follows
              where follower_id = any($1) and deleted_at is null
              group by follower_id
            ) aggregated
            group by user_id
            "#,
        )
        .bind(user_ids)
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            output.insert(
                row.get("user_id"),
                FollowCounts {
                    follower_count: row.get("follower_count"),
                    following_count: row.get("following_count"),
                },
            );
        }
        Ok(output)
    }
}

fn map_follow_request(row: sqlx::postgres::PgRow) -> FollowRequest {
    FollowRequest {
        id: row.get("id"),
        follower_id: row.get("follower_id"),
        followee_id: row.get("followee_id"),
        status: FollowRequestStatus::parse(&row.get::<String, _>("status")),
        created_at: row.get("created_at"),
        decided_at: row.get("decided_at"),
    }
}

#[async_trait]
impl FollowRequestRepositoryPort for PgFollowRepository {
    async fn create_request(
        &self,
        follower_id: Uuid,
        followee_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<FollowRequest> {
        let row = sqlx::query(
            r#"
            insert into follow_requests (id, follower_id, followee_id, status, created_at)
            values ($1, $2, $3, 'PENDING', $4)
            on conflict (follower_id, followee_id) where status = 'PENDING'
            do update set created_at = follow_requests.created_at
            returning id, follower_id, followee_id, status, created_at, decided_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(follower_id)
        .bind(followee_id)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(map_follow_request(row))
    }

    async fn accept_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<AcceptFollowRequestOutcome> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            select id, follower_id, followee_id, status, created_at, decided_at
            from follow_requests
            where id = $1 and followee_id = $2
            for update
            "#,
        )
        .bind(request_id)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(AcceptFollowRequestOutcome::NotFound);
        };
        let mut request = map_follow_request(row);
        if request.status == FollowRequestStatus::Accepted {
            let edge_row = sqlx::query(
                "select follower_id, followee_id, created_at from follows where follower_id=$1 and followee_id=$2 and deleted_at is null",
            )
            .bind(request.follower_id)
            .bind(request.followee_id)
            .fetch_optional(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(match edge_row {
                Some(row) => AcceptFollowRequestOutcome::Accepted {
                    request,
                    edge: FollowEdge {
                        follower_id: row.get("follower_id"),
                        followee_id: row.get("followee_id"),
                        created_at: row.get("created_at"),
                    },
                },
                None => AcceptFollowRequestOutcome::NotFound,
            });
        }
        if request.status != FollowRequestStatus::Pending {
            tx.rollback().await?;
            return Ok(AcceptFollowRequestOutcome::NotFound);
        }
        let blocked: bool = sqlx::query_scalar(
            r#"select exists(select 1 from user_blocks where deleted_at is null and ((blocker_id=$1 and blocked_id=$2) or (blocker_id=$2 and blocked_id=$1)))"#,
        )
        .bind(request.follower_id)
        .bind(request.followee_id)
        .fetch_one(&mut *tx)
        .await?;
        if blocked {
            tx.rollback().await?;
            return Ok(AcceptFollowRequestOutcome::Blocked);
        }
        sqlx::query("update follow_requests set status='ACCEPTED', decided_at=$2 where id=$1")
            .bind(request_id)
            .bind(decided_at)
            .execute(&mut *tx)
            .await?;
        let edge_row = sqlx::query(
            r#"
            insert into follows (follower_id, followee_id, created_at)
            values ($1,$2,$3)
            on conflict (follower_id, followee_id) do update
            set deleted_at=null, deleted_by=null, delete_reason=null, retention_until=null,
                created_at=case when follows.deleted_at is null then follows.created_at else excluded.created_at end
            returning follower_id, followee_id, created_at
            "#,
        )
        .bind(request.follower_id).bind(request.followee_id).bind(decided_at)
        .fetch_one(&mut *tx).await?;
        request.status = FollowRequestStatus::Accepted;
        request.decided_at = Some(decided_at);
        let edge = FollowEdge {
            follower_id: edge_row.get("follower_id"),
            followee_id: edge_row.get("followee_id"),
            created_at: edge_row.get("created_at"),
        };
        tx.commit().await?;
        Ok(AcceptFollowRequestOutcome::Accepted { request, edge })
    }

    async fn reject_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<Option<FollowRequest>> {
        transition_request(
            &self.pool, request_id, actor_id, true, "REJECTED", decided_at,
        )
        .await
    }

    async fn cancel_request(
        &self,
        request_id: Uuid,
        actor_id: Uuid,
        decided_at: DateTime<Utc>,
    ) -> Result<Option<FollowRequest>> {
        transition_request(
            &self.pool,
            request_id,
            actor_id,
            false,
            "CANCELLED",
            decided_at,
        )
        .await
    }

    async fn list_incoming_requests(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowRequest>, Option<String>)> {
        list_requests(&self.pool, user_id, cursor, limit, true).await
    }

    async fn list_outgoing_requests(
        &self,
        user_id: Uuid,
        cursor: Option<FollowCursor>,
        limit: i32,
    ) -> Result<(Vec<FollowRequest>, Option<String>)> {
        list_requests(&self.pool, user_id, cursor, limit, false).await
    }
}

async fn transition_request(
    pool: &PgPool,
    request_id: Uuid,
    actor_id: Uuid,
    incoming: bool,
    status: &str,
    decided_at: DateTime<Utc>,
) -> Result<Option<FollowRequest>> {
    let owner_column = if incoming {
        "followee_id"
    } else {
        "follower_id"
    };
    let query = format!(
        "update follow_requests set status=$3, decided_at=$4 where id=$1 and {owner_column}=$2 and status='PENDING' returning id, follower_id, followee_id, status, created_at, decided_at"
    );
    let row = sqlx::query(&query)
        .bind(request_id)
        .bind(actor_id)
        .bind(status)
        .bind(decided_at)
        .fetch_optional(pool)
        .await?;
    if let Some(row) = row {
        return Ok(Some(map_follow_request(row)));
    }
    let query = format!(
        "select id, follower_id, followee_id, status, created_at, decided_at from follow_requests where id=$1 and {owner_column}=$2 and status=$3"
    );
    let row = sqlx::query(&query)
        .bind(request_id)
        .bind(actor_id)
        .bind(status)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(map_follow_request))
}

async fn list_requests(
    pool: &PgPool,
    user_id: Uuid,
    cursor: Option<FollowCursor>,
    limit: i32,
    incoming: bool,
) -> Result<(Vec<FollowRequest>, Option<String>)> {
    let owner_column = if incoming {
        "followee_id"
    } else {
        "follower_id"
    };
    let query = format!(
        r#"select id, follower_id, followee_id, status, created_at, decided_at
           from follow_requests
           where {owner_column}=$1 and status='PENDING'
             and ($2::timestamptz is null or (created_at,id)<($2,$3))
           order by created_at desc,id desc limit $4"#
    );
    let rows = sqlx::query(&query)
        .bind(user_id)
        .bind(cursor.map(|value| value.created_at))
        .bind(cursor.map(|value| value.tie_breaker_id))
        .bind(i64::from(limit.max(1)) + 1)
        .fetch_all(pool)
        .await?;
    let mut requests = rows.into_iter().map(map_follow_request).collect::<Vec<_>>();
    let next_cursor = if requests.len() > limit as usize {
        requests
            .get(limit as usize - 1)
            .map(|value| FollowCursor::new(value.created_at, value.id).encode())
    } else {
        None
    };
    if requests.len() > limit as usize {
        requests.truncate(limit as usize);
    }
    Ok((requests, next_cursor))
}

#[async_trait]
impl RelationshipPolicyRepositoryPort for PgFollowRepository {
    async fn block(
        &self,
        blocker_id: Uuid,
        blocked_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"insert into user_blocks (blocker_id,blocked_id,created_at,deleted_at)
               values ($1,$2,$3,null)
               on conflict (blocker_id,blocked_id) do update set deleted_at=null,created_at=excluded.created_at"#,
        ).bind(blocker_id).bind(blocked_id).bind(created_at).execute(&mut *tx).await?;
        sqlx::query(
            r#"update follows set deleted_at=now(),deleted_by=$1,delete_reason='block',retention_until=now()+interval '30 days'
               where deleted_at is null and ((follower_id=$1 and followee_id=$2) or (follower_id=$2 and followee_id=$1))"#,
        ).bind(blocker_id).bind(blocked_id).execute(&mut *tx).await?;
        sqlx::query(
            r#"update follow_requests set status='CANCELLED',decided_at=now()
               where status='PENDING' and ((follower_id=$1 and followee_id=$2) or (follower_id=$2 and followee_id=$1))"#,
        ).bind(blocker_id).bind(blocked_id).execute(&mut *tx).await?;
        tx.commit().await?;
        self.policy(blocker_id, blocked_id).await
    }

    async fn unblock(&self, blocker_id: Uuid, blocked_id: Uuid) -> Result<RelationshipPolicy> {
        sqlx::query("update user_blocks set deleted_at=now() where blocker_id=$1 and blocked_id=$2 and deleted_at is null")
            .bind(blocker_id).bind(blocked_id).execute(&self.pool).await?;
        self.policy(blocker_id, blocked_id).await
    }

    async fn mute(
        &self,
        muter_id: Uuid,
        muted_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> Result<RelationshipPolicy> {
        sqlx::query(
            r#"insert into user_mutes (muter_id,muted_id,created_at,deleted_at)
               values ($1,$2,$3,null)
               on conflict (muter_id,muted_id) do update set deleted_at=null,created_at=excluded.created_at"#,
        ).bind(muter_id).bind(muted_id).bind(created_at).execute(&self.pool).await?;
        self.policy(muter_id, muted_id).await
    }

    async fn unmute(&self, muter_id: Uuid, muted_id: Uuid) -> Result<RelationshipPolicy> {
        sqlx::query("update user_mutes set deleted_at=now() where muter_id=$1 and muted_id=$2 and deleted_at is null")
            .bind(muter_id).bind(muted_id).execute(&self.pool).await?;
        self.policy(muter_id, muted_id).await
    }

    async fn policy(&self, viewer_id: Uuid, target_id: Uuid) -> Result<RelationshipPolicy> {
        let row = sqlx::query(
            r#"select
              exists(select 1 from user_blocks where blocker_id=$1 and blocked_id=$2 and deleted_at is null) as blocked_by_viewer,
              exists(select 1 from user_blocks where blocker_id=$2 and blocked_id=$1 and deleted_at is null) as blocked_viewer,
              exists(select 1 from user_mutes where muter_id=$1 and muted_id=$2 and deleted_at is null) as muted_by_viewer,
              exists(select 1 from follows where follower_id=$1 and followee_id=$2 and deleted_at is null) as following"#,
        ).bind(viewer_id).bind(target_id).fetch_one(&self.pool).await?;
        Ok(RelationshipPolicy {
            blocked_by_viewer: row.get("blocked_by_viewer"),
            blocked_viewer: row.get("blocked_viewer"),
            muted_by_viewer: row.get("muted_by_viewer"),
            following: row.get("following"),
        })
    }

    async fn batch_policy(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, RelationshipPolicy>> {
        if target_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query(
            r#"select target_id,
              exists(select 1 from user_blocks where blocker_id=$1 and blocked_id=target_id and deleted_at is null) as blocked_by_viewer,
              exists(select 1 from user_blocks where blocker_id=target_id and blocked_id=$1 and deleted_at is null) as blocked_viewer,
              exists(select 1 from user_mutes where muter_id=$1 and muted_id=target_id and deleted_at is null) as muted_by_viewer,
              exists(select 1 from follows where follower_id=$1 and followee_id=target_id and deleted_at is null) as following
              from unnest($2::uuid[]) target_id"#,
        ).bind(viewer_id).bind(target_ids).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let target_id: Uuid = row.get("target_id");
                (
                    target_id,
                    RelationshipPolicy {
                        blocked_by_viewer: row.get("blocked_by_viewer"),
                        blocked_viewer: row.get("blocked_viewer"),
                        muted_by_viewer: row.get("muted_by_viewer"),
                        following: row.get("following"),
                    },
                )
            })
            .collect())
    }
}

#[async_trait]
impl UserLifecycleRepositoryPort for PgFollowRepository {
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        // Serialize deletion with every active Friend write involving this
        // identity. The database trigger takes the same advisory lock for
        // relationship and recommendation writes, so a late event cannot pass
        // a pre-delete check and commit after the tombstone.
        sqlx::query("select friend_lock_subject_ids($1::uuid[])")
            .bind(vec![user_id])
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            insert into friend_user_deletion_tombstones (
                user_id, saga_id, actor_id, retention_until, applied_at
            ) values ($1, $2, $3, $4, now())
            on conflict (user_id) do nothing
            "#,
        )
        .bind(user_id)
        .bind(saga_id)
        .bind(actor_id)
        .bind(retention_until)
        .execute(&mut *tx)
        .await?;
        let receipt_matches: bool = sqlx::query_scalar(
            r#"
            select saga_id = $2 and actor_id = $3 and retention_until = $4
            from friend_user_deletion_tombstones
            where user_id = $1
            "#,
        )
        .bind(user_id)
        .bind(saga_id)
        .bind(actor_id)
        .bind(retention_until)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(
            receipt_matches,
            "Friend deletion identity was reused with different saga data"
        );

        sqlx::query(
            r#"
            update follows
            set deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3
            where follower_id=$1 or followee_id=$1
            "#,
        )
        .bind(user_id)
        .bind(actor_id)
        .bind(retention_until)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            update follow_requests
            set status=case when status='PENDING' then 'CANCELLED' else status end,
                decided_at=case when status='PENDING' then coalesce(decided_at, now()) else decided_at end,
                deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3
            where follower_id=$1 or followee_id=$1
            "#,
        )
        .bind(user_id)
        .bind(actor_id)
        .bind(retention_until)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            update user_blocks
            set deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3
            where blocker_id=$1 or blocked_id=$1
            "#,
        )
        .bind(user_id)
        .bind(actor_id)
        .bind(retention_until)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            update user_mutes
            set deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3
            where muter_id=$1 or muted_id=$1
            "#,
        )
        .bind(user_id)
        .bind(actor_id)
        .bind(retention_until)
        .execute(&mut *tx)
        .await?;

        for query in [
            "delete from people_recommendation_impressions where viewer_id=$1 or candidate_user_id=$1",
            "delete from people_recommendation_snapshots where viewer_id=$1",
            "delete from people_recommendation_candidates where viewer_id=$1 or candidate_user_id=$1",
            "delete from people_recommendation_events where viewer_id=$1 or actor_id=$1 or subject_id=$1",
            "delete from people_recommendation_refresh_jobs where viewer_id=$1",
            "delete from people_experiment_assignments where viewer_id=$1",
            "delete from people_embeddings where subject_id=$1",
        ] {
            sqlx::query(query).bind(user_id).execute(&mut *tx).await?;
        }
        sqlx::query(
            r#"
            insert into friend_audit_events (
                id, actor_id, follower_id, followee_id, action, outcome,
                request_id, metadata_json, occurred_at
            ) values ($1,$2,$3,null,'ACCOUNT_DELETION_APPLIED','SUCCESS',$4,$5,now())
            on conflict (id) do nothing
            "#,
        )
        .bind(saga_id)
        .bind(actor_id)
        .bind(user_id)
        .bind(saga_id.to_string())
        .bind(serde_json::json!({"retentionUntil": retention_until}).to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
