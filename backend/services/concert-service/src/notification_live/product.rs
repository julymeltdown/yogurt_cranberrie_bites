//! Projection facade under the caller's one real canonical response scope.
use super::{
    proto,
    protocol::{canonical, valid_cursor},
    NotificationClient, NotificationOperation, NotificationRequest,
};
use crate::{
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    product_rpc::{
        self,
        scoped::{ProductRepresentation, ScopedProductProducer},
        VerifiedProduct,
    },
};
use chrono::{DateTime, Utc};
use concert_domain::notification as domain;
use platform::actor_delegation::exchange::VerifiedParentAuthority;
use prost::Message;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tonic::{Code, Status};
use uuid::Uuid;

#[derive(Clone)]
pub struct NotificationRuntime {
    client: Arc<NotificationClient>,
}
impl NotificationRuntime {
    /// Actual configured transport only; no callback or successful default.
    pub fn new(client: Arc<NotificationClient>) -> Self {
        Self { client }
    }

    pub async fn dispatch_for_scope(
        &self,
        scope: Arc<OwnerGuardScope>,
        request: product_rpc::proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let parent = VerifiedParentAuthority::current().ok_or_else(denied)?;
        let binding = scope.binding();
        binding.check()?;
        let original =
            product_rpc::proto::ProductRequest::decode(parent.message().map_err(|_| denied())?)
                .map_err(|_| denied())?;
        let stored = binding.owner_binding();
        stored.matches_parent(&parent).map_err(|_| denied())?;
        let now = Utc::now().timestamp_micros();
        let observed = request
            .auth_observed_at_unix_micros
            .min(request.profile_observed_at_unix_micros);
        let expires = request
            .authority_expires_unix_micros
            .min(request.access_expires_unix_micros);
        if original != request
            || parent.request().method != "/concert.product.v1.ProductService/Dispatch"
            || stored.parent_digest != parent.parent_digest()
            || stored.parent_body_digest != parent.request_digest()
            || stored.actor_id != canonical(&request.user_id)?
            || stored.event_id != canonical(&request.event_id)?
            || stored.session_family_id != canonical(&request.session_family_id)?
            || stored.request_id != canonical(&request.request_id)?
            || stored.http_method != request.http_method
            || stored.http_target != request.http_target
            || stored.body_digest != <[u8; 32]>::from(Sha256::digest(&request.http_body))
            || stored.expires_unix_micros > expires
            || expires <= now
            || request.auth_observed_at_unix_micros <= 0
            || request.profile_observed_at_unix_micros <= 0
            || request.auth_observed_at_unix_micros > now
            || request.profile_observed_at_unix_micros > now
            || expires <= observed
            || expires.saturating_sub(observed) > product_rpc::BUDGET.as_micros() as i64
        {
            return Err(denied());
        }
        let parsed = NotificationRequest::parse(&request)?;
        if parsed.operation.id() != binding.operation() {
            return Err(denied());
        }
        let op = parsed.operation;
        let limit = match &parsed.input {
            proto::execute_request::Input::List(v) => Some((v.limit as usize, v.unread_only)),
            _ => None,
        };
        // The actual client preallocates/registers and retains its child. Neither
        // success nor error completion here releases it before genuine Root EOF.
        let reply = match self
            .client
            .execute(scope.clone(), op.wire(), parsed.input)
            .await
        {
            Ok(value) => value,
            Err(error) => {
                binding.check()?;
                return error_representation(op, &error, binding.request_id());
            }
        };
        binding.check()?;
        let ceiling = scope.disclosure_ceiling()?.expires_micros();
        if reply.expires_unix_micros > ceiling
            || reply.expires_unix_micros <= Utc::now().timestamp_micros()
        {
            return Err(denied());
        }
        project(op, binding.actor(), binding.event(), limit, reply)
    }
}
fn project(
    op: NotificationOperation,
    actor: Uuid,
    event: Uuid,
    list: Option<(usize, bool)>,
    reply: proto::ExecuteResponse,
) -> Result<ProductRepresentation, Status> {
    use proto::execute_response::Output;
    match (op, reply.output) {
        (NotificationOperation::List, Some(Output::List(v))) => {
            if v.items.len() > list.ok_or_else(denied)?.0
                || v.next_cursor.as_ref().is_some_and(|v| !valid_cursor(v))
            {
                return Err(denied());
            }
            let mut ids = std::collections::BTreeSet::new();
            let mut items = Vec::with_capacity(v.items.len());
            for item in v.items {
                let value = notification(item, event, reply.expires_unix_micros)?;
                if !ids.insert(value.notification_id)
                    || (list.ok_or_else(denied)?.1 && value.read_at.is_some())
                {
                    return Err(denied());
                }
                items.push(value);
            }
            representation(&domain::NotificationList {
                items,
                next_cursor: v.next_cursor,
            })
        }
        (NotificationOperation::UnreadCount, Some(Output::Count(v))) => {
            if canonical(&v.actor_id)? != actor
                || canonical(&v.event_id)? != event
                || !(0..=concert_domain::MAX_REVISION).contains(&v.unread_total)
            {
                return Err(denied());
            }
            representation(&domain::UnreadCount {
                actor_id: actor,
                event_id: event,
                unread_total: v.unread_total,
                as_of: timestamp(v.as_of_unix_micros, reply.expires_unix_micros)?,
            })
        }
        (NotificationOperation::MarkRead(id), Some(Output::MarkRead(v))) => {
            let value = notification(v, event, reply.expires_unix_micros)?;
            if value.notification_id != id || value.read_at.is_none() {
                return Err(denied());
            }
            representation(&value)
        }
        (NotificationOperation::MarkAllRead, Some(Output::MarkAll(v))) => {
            if v.read_count < 0 {
                return Err(denied());
            }
            representation(&domain::ReadAllResult {
                read_count: v.read_count,
                read_at: timestamp(v.read_at_unix_micros, reply.expires_unix_micros)?,
            })
        }
        _ => Err(denied()),
    }
}
#[async_trait::async_trait]
impl ScopedProductProducer for NotificationRuntime {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: product_rpc::proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        context.check()?;
        let binding = scope.binding();
        if (
            context.actor(),
            context.event(),
            context.session(),
            context.request(),
        ) != (
            binding.actor(),
            binding.event(),
            binding.session_family(),
            binding.request_id(),
        ) || context.parent_authority().parent_digest() != binding.parent_digest()
            || binding.deadline().expires_micros() > context.deadline().expires_micros()
        {
            return Err(denied());
        }
        self.dispatch_for_scope(scope, request).await
    }
}
fn notification(
    v: proto::Notification,
    event: Uuid,
    expires: i64,
) -> Result<domain::Notification, Status> {
    let notification_id = canonical(&v.notification_id)?;
    if canonical(&v.event_id)? != event
        || v.revision
            != if v.read_at_unix_micros.is_some() {
                2
            } else {
                1
            }
    {
        return Err(denied());
    }
    let kind = match v.kind {
        1 => domain::Kind::Party,
        2 => domain::Kind::Giveaway,
        3 => domain::Kind::Social,
        4 => domain::Kind::Moderation,
        5 => domain::Kind::Notice,
        6 => domain::Kind::Feed,
        _ => return Err(denied()),
    };
    let target = v
        .target
        .map(|t| {
            Ok::<_, Status>(domain::Target {
                target_type: match t.target_type {
                    1 => domain::TargetType::Party,
                    2 => domain::TargetType::Application,
                    3 => domain::TargetType::Giveaway,
                    4 => domain::TargetType::Reservation,
                    5 => domain::TargetType::SocialRequest,
                    6 => domain::TargetType::Post,
                    7 => domain::TargetType::Notice,
                    8 => domain::TargetType::Report,
                    _ => return Err(denied()),
                },
                target_id: canonical(&t.target_id)?,
            })
        })
        .transpose()?;
    let created_at = timestamp(v.created_at_unix_micros, expires)?;
    let read_at = v
        .read_at_unix_micros
        .map(|v| timestamp(v, expires))
        .transpose()?;
    if read_at.is_some_and(|read| read < created_at) {
        return Err(denied());
    }
    Ok(domain::Notification {
        notification_id,
        event_id: event,
        kind,
        target,
        created_at,
        read_at,
        revision: v.revision,
    })
}
fn timestamp(value: i64, ceiling: i64) -> Result<DateTime<Utc>, Status> {
    if value <= 0 || value > ceiling || value > Utc::now().timestamp_micros() {
        return Err(denied());
    }
    DateTime::from_timestamp_micros(value).ok_or_else(denied)
}
fn representation<T: Serialize>(value: &T) -> Result<ProductRepresentation, Status> {
    Ok(ProductRepresentation {
        status: 200,
        content_type: "application/json".into(),
        body: serde_json::to_vec(value)
            .map_err(|_| Status::internal("Notification representation unavailable"))?,
    })
}
fn error_representation(
    op: NotificationOperation,
    error: &Status,
    request: Uuid,
) -> Result<ProductRepresentation, Status> {
    let (status, reason) = match error.code() {
        Code::InvalidArgument => (400, "INVALID_INPUT"),
        Code::Unauthenticated => (401, "AUTHENTICATION_REQUIRED"),
        Code::PermissionDenied => (403, "PERMISSION_DENIED"),
        Code::NotFound => (404, "RESOURCE_UNAVAILABLE"),
        Code::Aborted if op.mutation() => (409, "VERSION_CONFLICT"),
        Code::AlreadyExists if op.mutation() => (409, "IDEMPOTENCY_CONFLICT"),
        Code::FailedPrecondition
            if op.mutation() && error.message() == "Notification idempotency expired" =>
        {
            (409, "IDEMPOTENCY_EXPIRED")
        }
        Code::ResourceExhausted => (429, "RATE_LIMITED"),
        Code::DeadlineExceeded => (504, "DEADLINE_EXCEEDED"),
        _ => (503, "DEPENDENCY_UNAVAILABLE"),
    };
    Ok(ProductRepresentation { status, content_type: "application/json".into(), body: serde_json::to_vec(&serde_json::json!({
        "code":format!("CONCERT_{status}"), "message":"Concert request unavailable", "traceId":request.to_string(),
        "timestamp":Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true), "details":{"reason":reason}
    })).map_err(|_| Status::internal("Notification representation unavailable"))? })
}
fn denied() -> Status {
    Status::permission_denied("Notification product scope mismatch")
}

#[cfg(test)]
mod tests {
    use super::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/notification_product_support/projection.rs"
    ));
}
