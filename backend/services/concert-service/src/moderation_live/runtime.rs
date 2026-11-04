use super::{
    authority::CanonicalModerationAuthority,
    cursor::ModerationCursor,
    ports::*,
    wire::{self, ModerationRequest, Query},
};
use crate::adapters::out::{
    relationship_authority::guarded::OwnerGuardScope,
    repository::audit::{authorize, PgModeration},
};
use chrono::{DateTime, SecondsFormat, Utc};
use concert_application::{
    admin::{Action, Actor},
    commands::*,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;
pub struct ModerationReply {
    pub status: u16,
    pub body: Value,
}
pub struct ModerationRuntime {
    pool: PgPool,
    owners: ModerationOwners,
    roles: Option<Arc<dyn EventRoleProvider>>,
    store: PgModeration,
    cursors: ModerationCursor,
}
impl ModerationRuntime {
    pub fn new(
        pool: PgPool,
        owners: ModerationOwners,
        roles: Option<Arc<dyn EventRoleProvider>>,
        keys: Arc<FingerprintKeys>,
        cursor_key: &[u8; 32],
    ) -> Result<Self, CommandError> {
        wire::warm_contracts()?;
        Ok(Self {
            store: PgModeration::new(pool.clone(), owners.clone(), keys),
            pool,
            owners,
            roles,
            cursors: ModerationCursor::new(cursor_key)?,
        })
    }
    /// Root must retain both canonical scope and actual role-owner fence through
    /// HTTP terminal. Neither dispatch return nor serialization completes them.
    pub async fn dispatch(
        &self,
        scope: Arc<OwnerGuardScope>,
        frame: crate::product_rpc::proto::ProductRequest,
    ) -> Result<ModerationReply, CommandError> {
        let request = ModerationRequest::from_owner_scope(scope.binding(), &frame)?;
        let roles = self
            .roles
            .as_ref()
            .ok_or(CommandError::Unavailable)?
            .acquire_for_root_terminal(scope.clone())
            .await?;
        let authority = CanonicalModerationAuthority::new(scope.clone(), roles, &request)?;
        scope
            .run(async {
                Ok(self
                    .execute(
                        Actor {
                            actor: scope.binding().actor(),
                            event: scope.binding().event(),
                        },
                        scope.binding().request_id(),
                        request,
                        &authority,
                    )
                    .await)
            })
            .await
            .map_err(|_| CommandError::Unavailable)?
    }
    async fn execute(
        &self,
        actor: Actor,
        trace: Uuid,
        request: ModerationRequest,
        authority: &dyn ModerationAuthority,
    ) -> Result<ModerationReply, CommandError> {
        authority.check_disclosure().await?;
        let reply = if let Some(change) = request.change {
            let receipt = self
                .store
                .submit(
                    actor,
                    request.key.ok_or(CommandError::InvalidInput)?,
                    &change,
                    authority,
                )
                .await?;
            match receipt.state {
                ReceiptState::Pending => {
                    let body = receipt_value(receipt);
                    wire::validate_value("Domain_common_CommandReceipt", &body)
                        .map_err(|_| CommandError::Unavailable)?;
                    ModerationReply { status: 202, body }
                }
                ReceiptState::Failed => {
                    let reason = receipt.failure.ok_or(CommandError::InvalidState)?;
                    let status = match reason {
                        SafeFailure::InvalidInput => 400,
                        SafeFailure::ResourceUnavailable => 404,
                        SafeFailure::DependencyUnavailable => 503,
                        SafeFailure::InternalError => 500,
                        _ => 409,
                    };
                    error_reply(status, reason.as_str(), trace)
                }
                ReceiptState::Retired => return Err(CommandError::Expired),
                ReceiptState::Succeeded => {
                    let view = self
                        .owners
                        .port(change.steps()[0])?
                        .review(authority, actor, change.target())
                        .await?;
                    let (schema, id) = if request.action == Action::ModerateUser {
                        ("Domain_admin_UserReview", "user_id")
                    } else {
                        ("Domain_admin_ContentReview", "target_id")
                    };
                    wire::validate_value(schema, &view).map_err(|_| CommandError::Unavailable)?;
                    if view[id].as_str() != Some(change.target().to_string().as_str())
                        || view["revision"].as_i64().unwrap_or(0)
                            < receipt
                                .reference
                                .ok_or(CommandError::InvalidState)?
                                .revision
                    {
                        return Err(CommandError::Unavailable);
                    }
                    if let concert_application::admin::moderation::Change::Content {
                        kind, ..
                    } = &change
                    {
                        if view["target_type"].as_str() != Some(kind.as_str()) {
                            return Err(CommandError::Unavailable);
                        }
                    }
                    if request.action == Action::ModerateUser
                        && view["profile"]["user_id"] != view["user_id"]
                    {
                        return Err(CommandError::Unavailable);
                    }
                    ModerationReply {
                        status: 200,
                        body: view,
                    }
                }
            }
        } else {
            let q = request.query;
            let op = request.action.operation();
            let now = Utc::now();
            let opened = q
                .cursor
                .as_ref()
                .map(|c| self.cursors.open(actor, op, &q, c, now))
                .transpose()?;
            let snapshot = opened.as_ref().map(|p| p.snapshot).unwrap_or(now);
            let after = opened.as_ref().map(|p| p.after.as_str());
            let (mut rows, schema) = if request.action == Action::AuditRead {
                (
                    self.store
                        .audit_page(actor, authority, &q, after, snapshot)
                        .await?,
                    "Domain_admin_AuditList",
                )
            } else {
                let mut tx = self
                    .pool
                    .begin()
                    .await
                    .map_err(|_| CommandError::Unavailable)?;
                authorize(&mut tx, authority, actor, request.action, actor.event).await?;
                let rows = self
                    .owners
                    .directory
                    .as_ref()
                    .ok_or(CommandError::Unavailable)?
                    .page_in(authority, &mut tx, actor, op, &q, after, snapshot)
                    .await?;
                authority.check_disclosure().await?;
                crate::adapters::out::repository::event_operations::finish_read(&mut tx).await?;
                tx.commit().await.map_err(|_| CommandError::Unavailable)?;
                (
                    rows,
                    if request.action == Action::AdminUsers {
                        "Domain_admin_UserReviewList"
                    } else {
                        "Domain_admin_ContentReviewList"
                    },
                )
            };
            if rows.len() > usize::from(q.limit) + 1 {
                return Err(CommandError::Unavailable);
            }
            // Validate the look-ahead row too; the closed public list has a
            // 100-item maximum while a 100-row page may fetch 101 internally.
            for chunk in rows.chunks(100) {
                wire::validate_value(schema, &json!({"items":chunk,"next_cursor":null}))
                    .map_err(|_| CommandError::Unavailable)?;
            }
            validate_rows(actor, request.action, &q, &rows, after, snapshot)?;
            let more = rows.len() > usize::from(q.limit);
            rows.truncate(q.limit.into());
            let next = if more {
                let last = rows.last().ok_or(CommandError::Unavailable)?;
                Some(self.cursors.seal(
                    actor,
                    op,
                    &q,
                    position(request.action, last)?,
                    snapshot,
                    Utc::now(),
                )?)
            } else {
                None
            };
            let body = json!({"items":rows,"next_cursor":next});
            wire::validate_value(schema, &body).map_err(|_| CommandError::Unavailable)?;
            ModerationReply { status: 200, body }
        };
        authority.check_disclosure().await?;
        Ok(reply)
    }
}
fn validate_rows(
    actor: Actor,
    action: Action,
    q: &Query,
    rows: &[Value],
    after: Option<&str>,
    snapshot: DateTime<Utc>,
) -> Result<(), CommandError> {
    let mut prior = after.map(str::to_owned);
    for row in rows {
        let p = position(action, row)?;
        if let Some(old) = &prior {
            if (action == Action::AuditRead && p >= *old)
                || (action != Action::AuditRead && p <= *old)
            {
                return Err(CommandError::Unavailable);
            }
        }
        prior = Some(p);
        if action == Action::AuditRead {
            let created = DateTime::parse_from_rfc3339(
                row["created_at"]
                    .as_str()
                    .ok_or(CommandError::Unavailable)?,
            )
            .map_err(|_| CommandError::Unavailable)?
            .with_timezone(&Utc);
            if row["event_id"].as_str() != Some(actor.event.to_string().as_str())
                || created > snapshot
            {
                return Err(CommandError::Unavailable);
            }
        } else {
            for (name, filter) in &q.filters {
                if row[name].as_str() != Some(filter) {
                    return Err(CommandError::Unavailable);
                }
            }
            if action == Action::AdminUsers && row["profile"]["user_id"] != row["user_id"] {
                return Err(CommandError::Unavailable);
            }
        }
    }
    Ok(())
}
fn position(action: Action, v: &Value) -> Result<String, CommandError> {
    match action {
        Action::AdminUsers => Ok(v["user_id"]
            .as_str()
            .ok_or(CommandError::Unavailable)?
            .into()),
        Action::AdminContent => Ok(format!(
            "{}/{}",
            v["target_type"].as_str().ok_or(CommandError::Unavailable)?,
            v["target_id"].as_str().ok_or(CommandError::Unavailable)?
        )),
        Action::AuditRead => {
            let time = DateTime::parse_from_rfc3339(
                v["created_at"].as_str().ok_or(CommandError::Unavailable)?,
            )
            .map_err(|_| CommandError::Unavailable)?
            .with_timezone(&Utc);
            Ok(format!(
                "{}/{}",
                time.to_rfc3339_opts(SecondsFormat::Micros, true),
                v["audit_id"].as_str().ok_or(CommandError::Unavailable)?
            ))
        }
        _ => Err(CommandError::InvalidInput),
    }
}
fn receipt_value(r: SafeReceipt) -> Value {
    json!({"receipt_id":r.id,"event_id":r.scope.event,"actor_id":r.scope.actor,"operation_id":r.scope.operation.id(),"idempotency_key":r.scope.key,"state":r.state.as_str(),"resource_id":r.reference.map(|v|v.resource),"resource_revision":r.reference.map(|v|v.revision),"failure_reason":r.failure.map(|v|v.as_str()),"created_at":r.created_at,"expires_at":r.expires_at,"result_status":r.result_status})
}
fn error_reply(status: u16, reason: &str, trace: Uuid) -> ModerationReply {
    ModerationReply {
        status,
        body: json!({"code":format!("CONCERT_{status}"),"message":"Moderation request unavailable","traceId":trace,"timestamp":Utc::now().to_rfc3339_opts(SecondsFormat::Millis,true),"details":{"reason":reason}}),
    }
}
#[async_trait::async_trait]
impl crate::product_rpc::scoped::ScopedProductProducer for ModerationRuntime {
    async fn dispatch(
        &self,
        context: &crate::product_rpc::VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        frame: crate::product_rpc::proto::ProductRequest,
    ) -> Result<crate::product_rpc::scoped::ProductRepresentation, tonic::Status> {
        context.check()?;
        if context.operation().contract_id() != scope.binding().operation() {
            return Err(tonic::Status::permission_denied(
                "Moderation scope mismatch",
            ));
        }
        let reply = match ModerationRuntime::dispatch(self, scope, frame).await {
            Ok(r) => r,
            Err(e) => {
                let (status, reason) = match e {
                    CommandError::Denied => (403, "PERMISSION_DENIED"),
                    CommandError::InvalidInput => (400, "INVALID_INPUT"),
                    CommandError::Conflict => (409, "IDEMPOTENCY_CONFLICT"),
                    CommandError::Expired => (409, "IDEMPOTENCY_EXPIRED"),
                    CommandError::Capacity => (429, "RATE_LIMITED"),
                    CommandError::InvalidState => (500, "INTERNAL_ERROR"),
                    CommandError::Unavailable => (503, "DEPENDENCY_UNAVAILABLE"),
                };
                error_reply(status, reason, context.request())
            }
        };
        Ok(crate::product_rpc::scoped::ProductRepresentation {
            status: u32::from(reply.status),
            content_type: "application/json".into(),
            body: serde_json::to_vec(&reply.body)
                .map_err(|_| tonic::Status::internal("Moderation representation unavailable"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_rejects_cross_profile_wrong_filter_and_unsorted_rows() {
        let actor = Actor {
            actor: Uuid::new_v4(),
            event: Uuid::new_v4(),
        };
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let now = Utc::now();
        let row = |id| json!({"user_id":id,"profile":{"user_id":id},"state":"ACTIVE"});
        let q = Query {
            limit: 20,
            ..Default::default()
        };
        assert!(validate_rows(
            actor,
            Action::AdminUsers,
            &q,
            &[row(first), row(second)],
            None,
            now
        )
        .is_ok());
        assert!(validate_rows(
            actor,
            Action::AdminUsers,
            &q,
            &[row(second), row(first)],
            None,
            now
        )
        .is_err());
        let mut wrong = row(first);
        wrong["profile"]["user_id"] = json!(second);
        assert!(validate_rows(actor, Action::AdminUsers, &q, &[wrong], None, now).is_err());
        let mut q = q;
        q.filters.insert("state".into(), "SUSPENDED".into());
        assert!(validate_rows(actor, Action::AdminUsers, &q, &[row(first)], None, now).is_err());
    }
    #[test]
    fn audit_rejects_cross_event_and_future_snapshot() {
        let actor = Actor {
            actor: Uuid::new_v4(),
            event: Uuid::new_v4(),
        };
        let now = Utc::now();
        let mut row = json!({"audit_id":Uuid::new_v4(),"event_id":actor.event,"created_at":now});
        let q = Query {
            limit: 20,
            ..Default::default()
        };
        assert!(validate_rows(actor, Action::AuditRead, &q, &[row.clone()], None, now).is_ok());
        row["event_id"] = json!(Uuid::new_v4());
        assert!(validate_rows(actor, Action::AuditRead, &q, &[row.clone()], None, now).is_err());
        row["event_id"] = json!(actor.event);
        row["created_at"] = json!(now + chrono::Duration::seconds(1));
        assert!(validate_rows(actor, Action::AuditRead, &q, &[row], None, now).is_err());
    }
}
