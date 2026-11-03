//! BE15 producer: block state, pair revision and invalidation commit atomically.
use super::*;
use crate::adapters::out::repository::receipt::CommandProducer;
use concert_application::commands::*;

pub struct BlockCommand<'a, A> {
    pub policy: &'a PgRelationshipPolicy<A>,
    pub query: PolicyQuery,
    pub blocked: bool,
}
fn error(e: RelationshipError) -> CommandError {
    match e {
        RelationshipError::Denied => CommandError::Denied,
        RelationshipError::Unavailable => CommandError::Unavailable,
        RelationshipError::Stale => CommandError::Conflict,
    }
}
#[async_trait]
impl<A: FencedRelationshipAuthority> CommandProducer for BlockCommand<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: uuid::Uuid,
    ) -> Result<(), CommandError> {
        if self.query.actor == self.query.subject
            || scope.actor != self.query.actor.uuid()
            || scope.event != self.query.event.uuid()
            || target != self.query.subject.uuid()
            || scope.operation.id()
                != if self.blocked {
                    "blockConcertUser"
                } else {
                    "unblockConcertUser"
                }
        {
            return Err(CommandError::Denied);
        }
        sqlx::query("select event_id from concert_events where event_id=$1 for update")
            .bind(scope.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        self.policy
            .safety_authority(c, self.query)
            .await
            .map_err(error)?;
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: uuid::Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        self.authorize(c, scope, target).await?;
        let (low, high) = ordered(self.query);
        let column = if self.query.actor.uuid() == low {
            "low_blocks_high"
        } else {
            "high_blocks_low"
        };
        // Closed column chosen internally, never interpolated user input.
        let sql = format!("update public.concert_relationship_pairs set {column}=$4,revision=revision+1 where event_id=$1 and low_user=$2 and high_user=$3 returning revision");
        let revision: i64 = sqlx::query_scalar(&sql)
            .bind(scope.event)
            .bind(low)
            .bind(high)
            .bind(self.blocked)
            .fetch_one(&mut *c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        if self.blocked {
            sqlx::query("update concert_social_grants set state='REVOKED',revision=revision+1 where event_id=$1 and state='ACTIVE' and ((owner_id=$2 and viewer_id=$3) or (owner_id=$3 and viewer_id=$2))")
                .bind(scope.event).bind(scope.actor).bind(target).execute(&mut *c).await.map_err(|_|CommandError::Unavailable)?;
            sqlx::query("update concert_social_requests set state='REVOKED',revision=revision+1 where event_id=$1 and state in ('PENDING','APPROVED') and ((requester_id=$2 and recipient_id=$3) or (requester_id=$3 and recipient_id=$2))")
                .bind(scope.event).bind(scope.actor).bind(target).execute(&mut *c).await.map_err(|_|CommandError::Unavailable)?;
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
