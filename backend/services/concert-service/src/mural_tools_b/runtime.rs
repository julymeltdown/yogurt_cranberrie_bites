//! Original scoped Product producer; response settlement stays with Root.
use super::{
    protocol::{self, WallOperation},
    PgMuralTools, ScopedMuralAuthority,
};
use crate::{
    adapters::out::{
        relationship_authority::guarded::OwnerGuardScope, repository::mural::PgMuralRepository,
    },
    product_rpc::{
        proto,
        scoped::{ProductRepresentation, ScopedProductProducer},
        VerifiedProduct,
    },
};
use concert_application::{
    commands::{CommandError, FingerprintKeys},
    mural::{
        tools_b::{MuralTools, ToolError},
        MuralActor, MuralError, MuralReader,
    },
};
use concert_domain::mural::tools_b::ActionBody;
use prost::Message;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

pub struct MuralToolsRuntime {
    pool: PgPool,
    keys: Arc<FingerprintKeys>,
}
impl MuralToolsRuntime {
    pub fn new(pool: PgPool, keys: Arc<FingerprintKeys>) -> Self {
        Self { pool, keys }
    }
    pub async fn dispatch_for_scope(
        &self,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let parent = platform::actor_delegation::exchange::VerifiedParentAuthority::current()
            .ok_or_else(|| Status::permission_denied("Original Wall parent required"))?;
        let original = proto::ProductRequest::decode(
            parent
                .message()
                .map_err(|_| Status::permission_denied("Invalid Wall parent"))?,
        )
        .map_err(|_| Status::permission_denied("Invalid Wall parent"))?;
        let b = scope.binding();
        b.check()?;
        b.owner_binding()
            .matches_parent(&parent)
            .map_err(|_| Status::permission_denied("Wall parent mismatch"))?;
        if original != request
            || parent.parent_digest() != b.parent_digest()
            || parent.actor() != b.actor()
            || request.user_id != b.actor().to_string()
            || request.event_id != b.event().to_string()
            || request.session_family_id != b.session_family().to_string()
            || request.request_id != b.request_id().to_string()
            || request.http_method != b.http_method()
            || request.http_target != b.http_target()
            || b.body_digest() != <[u8; 32]>::from(Sha256::digest(&request.http_body))
            || request.authority_expires_unix_micros < b.deadline().expires_micros()
        {
            return Err(Status::permission_denied("Wall scope binding mismatch"));
        }
        let op = WallOperation::parse(b.event(), &request.http_method, &request.http_target)?;
        if op.id() != b.operation() {
            return Err(Status::permission_denied("Wall operation mismatch"));
        }
        let actor = MuralActor {
            event: b.event(),
            actor: b.actor(),
        };
        let legacy = PgMuralRepository::new(
            self.pool.clone(),
            ScopedMuralAuthority::new(self.pool.clone(), scope.clone()),
        );
        let tools = PgMuralTools::new(
            self.pool.clone(),
            ScopedMuralAuthority::new(self.pool.clone(), scope.clone()),
            self.keys.clone(),
        );
        let admin = super::admin::MuralAdmin::new(self.pool.clone(), self.keys.clone());
        let outcome: Result<Value, ToolError> = async {
            if op.admin() && op.mutation() {
                if request.content_type != "application/json" {
                    return Err(MuralError::InvalidInput.into());
                }
                let key = crate::product_rpc::canonical_uuid(&request.idempotency_key)
                    .map_err(|_| ToolError::Mural(MuralError::InvalidInput))?;
                let body = super::admin::body(op, &request.http_body)
                    .map_err(|_| ToolError::Mural(MuralError::InvalidInput))?;
                admin.execute(actor, op, key, body).await
            } else if op.mutation() {
                if request.content_type != "application/json" {
                    return Err(MuralError::InvalidInput.into());
                }
                let key = crate::product_rpc::canonical_uuid(&request.idempotency_key)
                    .map_err(|_| ToolError::Mural(MuralError::InvalidInput))?;
                let body = protocol::body(op, &request.http_body)
                    .map_err(|_| ToolError::Mural(MuralError::InvalidInput))?;
                match body {
                    ActionBody::Pencil(input) => {
                        serde_json::to_value(legacy.place(actor, key, input, &self.keys).await?)
                            .map_err(|_| ToolError::Mural(MuralError::InvalidState))
                    }
                    _ => serde_json::to_value(
                        tools
                            .execute(
                                actor,
                                key,
                                protocol::command(body)
                                    .map_err(|_| ToolError::Mural(MuralError::InvalidInput))?,
                            )
                            .await?,
                    )
                    .map_err(|_| ToolError::Mural(MuralError::InvalidState)),
                }
            } else {
                if !request.http_body.is_empty()
                    || !request.content_type.is_empty()
                    || !request.idempotency_key.is_empty()
                {
                    return Err(MuralError::InvalidInput.into());
                }
                let value = match op {
                    WallOperation::Status => {
                        let q = protocol::status_query(actor.event, &request.http_target)
                            .map_err(|_| MuralError::InvalidInput)?;
                        if matches!(
                            q.operation.id(),
                            "controlConcertMural"
                                | "lockConcertMuralRegion"
                                | "restoreConcertMural"
                        ) {
                            return admin.status(actor, q.operation, q.key).await;
                        }
                        return tools.command_status(actor, q.operation, q.key).await;
                    }
                    WallOperation::Snapshot => serde_json::to_value(legacy.snapshot(actor).await?),
                    WallOperation::AdminState => return admin.state(actor).await,
                    WallOperation::Delta => {
                        let after = protocol::delta_after(actor.event, &request.http_target)
                            .map_err(|_| MuralError::InvalidInput)?;
                        serde_json::to_value(legacy.delta(actor, after).await?)
                    }
                    WallOperation::PixelResult(key) => {
                        serde_json::to_value(legacy.recover(actor, key).await?)
                    }
                    WallOperation::Tools => serde_json::to_value(tools.capabilities(actor).await?),
                    WallOperation::ActionResult(key) => {
                        serde_json::to_value(tools.action_result(actor, key).await?)
                    }
                    _ => return Err(MuralError::InvalidInput.into()),
                };
                value.map_err(|_| ToolError::Mural(MuralError::InvalidState))
            }
        }
        .await;
        let (status, body) = match outcome {
            Ok(v) => (200, v),
            Err(e) => error(e, b.request_id()),
        };
        b.check()?;
        scope.disclosure_ceiling()?;
        Ok(ProductRepresentation {
            status,
            content_type: "application/json".into(),
            body: serde_json::to_vec(&body)
                .map_err(|_| Status::unavailable("Wall projection unavailable"))?,
        })
    }
}
#[async_trait::async_trait]
impl ScopedProductProducer for MuralToolsRuntime {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        context.check()?;
        let b = scope.binding();
        if context.operation().contract_id() != b.operation()
            || context.parent_authority().parent_digest() != b.parent_digest()
        {
            return Err(Status::permission_denied(
                "Wall verified operation mismatch",
            ));
        }
        b.assert_context(
            context.actor(),
            context.event(),
            context.session(),
            context.request(),
            context.parent_authority().parent_digest(),
            chrono::DateTime::from_timestamp_micros(b.deadline().expires_micros())
                .ok_or_else(|| Status::permission_denied("Wall deadline invalid"))?,
        )?;
        let response = self.dispatch_for_scope(scope, request).await?;
        context.check()?;
        Ok(response)
    }
}
fn error(error: ToolError, request: Uuid) -> (u32, Value) {
    let (status, reason) = match error {
        ToolError::NoChange => (409, "MURAL_NO_CHANGE"),
        ToolError::FillTooLarge => (409, "MURAL_FILL_TOO_LARGE"),
        ToolError::CellBudget(_) => (429, "MURAL_BUDGET_EXCEEDED"),
        ToolError::UndoExpired => (409, "MURAL_UNDO_EXPIRED"),
        ToolError::UndoNotLatest => (409, "MURAL_UNDO_NOT_LATEST"),
        ToolError::UndoUnavailable => (409, "MURAL_UNDO_UNAVAILABLE"),
        ToolError::Unavailable => (503, "MURAL_TOOL_UNAVAILABLE"),
        ToolError::Unresolved => (409, "MURAL_ACTION_UNRESOLVED"),
        ToolError::InvalidTransition => (409, "INVALID_TRANSITION"),
        ToolError::Forbidden => (403, "PERMISSION_DENIED"),
        ToolError::Mural(e) => match e {
            MuralError::InvalidInput => (400, "INVALID_INPUT"),
            MuralError::Denied => (404, "RESOURCE_UNAVAILABLE"),
            MuralError::RevisionConflict => (409, "VERSION_CONFLICT"),
            MuralError::EventReadOnly => (409, "EVENT_READ_ONLY"),
            MuralError::Paused => (409, "MURAL_PAUSED"),
            MuralError::Frozen => (409, "MURAL_FROZEN"),
            MuralError::Cooldown(_) => (429, "MURAL_COOLDOWN"),
            MuralError::RegionLocked => (409, "MURAL_REGION_LOCKED"),
            MuralError::Command(c) => match c {
                CommandError::InvalidInput => (400, "INVALID_INPUT"),
                CommandError::Denied => (404, "RESOURCE_UNAVAILABLE"),
                CommandError::Conflict => (409, "IDEMPOTENCY_CONFLICT"),
                CommandError::Expired => (409, "IDEMPOTENCY_EXPIRED"),
                CommandError::Capacity => (429, "RATE_LIMITED"),
                _ => (503, "DEPENDENCY_UNAVAILABLE"),
            },
            _ => (503, "DEPENDENCY_UNAVAILABLE"),
        },
    };
    let mut details = json!({"reason":reason});
    if reason == "RATE_LIMITED" {
        details["retry_after_ms"] = 1000.into();
    }
    (
        status,
        json!({"code":format!("CONCERT_{status}"),"message":"Concert request unavailable","traceId":request.to_string(),"timestamp":chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),"details":details}),
    )
}
