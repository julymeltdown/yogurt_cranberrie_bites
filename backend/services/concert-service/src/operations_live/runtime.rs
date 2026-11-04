//! C04-ready producer composition; the retained owners enforce business invariants.
use super::{
    authority::CanonicalAuthority,
    cursor::OperationsCursor,
    owners::{Evidence, Places, Review},
    wire::{self, Operation},
    OperationsOwners, OperationsReply, OperationsRequest,
};
use crate::adapters::out::{
    relationship_authority::guarded::OwnerGuardScope,
    repository::{
        event_operations::{Notice, PgEventOperations},
        report::PgReports,
    },
};
use chrono::Utc;
use concert_application::{
    admin::{notices::NoticeState, Actor},
    commands::{CommandError, FingerprintKeys, ReceiptState, SafeFailure, SafeReceipt},
    report::ReportFilter,
};
use concert_domain::report::{ReportState, TargetType};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::sync::Arc;

#[derive(Debug)]
pub enum OperationsError {
    Command(CommandError),
    ResourceUnavailable,
}
impl From<CommandError> for OperationsError {
    fn from(error: CommandError) -> Self {
        Self::Command(error)
    }
}
impl std::fmt::Display for OperationsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(error) => std::fmt::Display::fmt(error, f),
            Self::ResourceUnavailable => f.write_str("Operations resource unavailable"),
        }
    }
}
impl std::error::Error for OperationsError {}

pub struct OperationsRuntime {
    pool: PgPool,
    owners: OperationsOwners,
    keys: Arc<FingerprintKeys>,
    cursors: OperationsCursor,
}
impl OperationsRuntime {
    pub fn new(
        pool: PgPool,
        owners: OperationsOwners,
        keys: Arc<FingerprintKeys>,
        cursor_key: &[u8; 32],
    ) -> Result<Self, CommandError> {
        wire::warm_contracts()?;
        Ok(Self {
            pool,
            owners,
            keys,
            cursors: OperationsCursor::new(cursor_key)?,
        })
    }
    /// Root passes the exact verified Product frame and retains this real scope
    /// beyond dispatch/serialization until Gateway reports HTTP body terminal.
    pub async fn dispatch(
        &self,
        scope: Arc<OwnerGuardScope>,
        frame: crate::product_rpc::proto::ProductRequest,
    ) -> Result<OperationsReply, OperationsError> {
        let parsed = OperationsRequest::from_owner_scope(scope.binding(), &frame)?;
        // Returning a response never marks serialization, seals, or releases it.
        scope
            .run(async { Ok(self.execute(scope.clone(), parsed).await) })
            .await
            .map_err(super::authority_error)?
    }
    async fn execute(
        &self,
        scope: Arc<OwnerGuardScope>,
        request: OperationsRequest,
    ) -> Result<OperationsReply, OperationsError> {
        let actor = Actor {
            actor: scope.binding().actor(),
            event: scope.binding().event(),
        };
        let authority = CanonicalAuthority {
            scope: scope.clone(),
            roles: self.owners.roles.clone(),
            request_digest: request.digest,
            request_key: request.key,
            operation: request.operation,
        };
        authority.check_scope().await?;
        let reports = PgReports::new(
            self.pool.clone(),
            authority.clone(),
            Evidence {
                scope: scope.clone(),
                provider: self.owners.evidence.clone(),
            },
        );
        let operations = PgEventOperations::new(
            self.pool.clone(),
            authority.clone(),
            Review(self.owners.review.clone()),
            Places(self.owners.places.clone().unwrap_or_else(|| {
                Arc::new(
                    crate::adapters::out::repository::place::PgPlaceOperations::new(
                        authority.clone(),
                    ),
                )
            })),
        );
        let binding = request
            .query
            .binding(actor.actor, actor.event, request.operation);
        let after = request
            .query
            .cursor
            .as_ref()
            .map(|t| self.cursors.open(&binding, t, Utc::now()))
            .transpose()?;
        let limit = request.query.limit;
        let (status, schema, body) = match request.action {
            Operation::CreateReport(input) => {
                let receipt = match reports
                    .create(
                        actor,
                        request.key.ok_or(CommandError::InvalidInput)?,
                        input,
                        &self.keys,
                    )
                    .await
                {
                    Ok(receipt) => Some(receipt),
                    Err(CommandError::Capacity) => None,
                    Err(error) => return Err(error.into()),
                };
                match receipt {
                    None => (
                        429,
                        "Domain_common_DomainError",
                        report_error(429, "RATE_LIMITED", scope.binding().request_id()),
                    ),
                    Some(receipt) => match receipt.state {
                        ReceiptState::Pending => {
                            (202, "Domain_common_CommandReceipt", receipt_value(receipt))
                        }
                        ReceiptState::Succeeded => {
                            let reference = receipt.reference.ok_or(CommandError::InvalidState)?;
                            if receipt.result_status != Some(200) {
                                return Err(CommandError::InvalidState.into());
                            }
                            // Read the actual committed resource under the SAME retained
                            // canonical scope. Never synthesize a Report from the request
                            // or return a terminal SUCCEEDED receipt as pending HTTP 202.
                            let row = sqlx::query("select report_id,event_id,target_type,target_id,state,revision,created_at from concert_reports where event_id=$1 and reporter_id=$2 and report_id=$3 and revision=$4")
                                .bind(actor.event).bind(actor.actor).bind(reference.resource).bind(reference.revision)
                                .fetch_optional(&self.pool).await.map_err(super::unavailable)?
                                .ok_or(OperationsError::ResourceUnavailable)?;
                            let report = concert_domain::report::Report {
                                report_id: row.try_get("report_id").map_err(super::unavailable)?,
                                event_id: row.try_get("event_id").map_err(super::unavailable)?,
                                target_type: TargetType::parse(
                                    &row.try_get::<String, _>("target_type")
                                        .map_err(super::unavailable)?,
                                )
                                .map_err(|_| CommandError::InvalidState)?,
                                target_id: row.try_get("target_id").map_err(super::unavailable)?,
                                state: ReportState::parse(
                                    &row.try_get::<String, _>("state")
                                        .map_err(super::unavailable)?,
                                )
                                .map_err(|_| CommandError::InvalidState)?,
                                revision: row.try_get("revision").map_err(super::unavailable)?,
                                created_at: row
                                    .try_get("created_at")
                                    .map_err(super::unavailable)?,
                            };
                            (200, "Domain_admin_Report", json!(report))
                        }
                        ReceiptState::Failed => {
                            let (status, reason) = match receipt
                                .failure
                                .ok_or(CommandError::InvalidState)?
                            {
                                SafeFailure::ResourceUnavailable => (404, "RESOURCE_UNAVAILABLE"),
                                SafeFailure::EventReadOnly => (409, "EVENT_READ_ONLY"),
                                SafeFailure::InvalidInput => (400, "INVALID_INPUT"),
                                SafeFailure::InvalidTransition => (409, "INVALID_TRANSITION"),
                                SafeFailure::VersionConflict => (409, "VERSION_CONFLICT"),
                                SafeFailure::DependencyUnavailable => {
                                    (503, "DEPENDENCY_UNAVAILABLE")
                                }
                                _ => (500, "INTERNAL_ERROR"),
                            };
                            (
                                status,
                                "Domain_common_DomainError",
                                report_error(status, reason, scope.binding().request_id()),
                            )
                        }
                        ReceiptState::Retired => return Err(CommandError::Expired.into()),
                    },
                }
            }
            Operation::Event(command, id) => {
                let receipt = operations
                    .execute(
                        actor,
                        request.key.ok_or(CommandError::InvalidInput)?,
                        id,
                        command,
                        &self.keys,
                    )
                    .await?;
                (202, "Domain_common_CommandReceipt", receipt_value(receipt))
            }
            Operation::OwnReports | Operation::ReportQueue => {
                let filter = ReportFilter {
                    state: request
                        .query
                        .filters
                        .get("state")
                        .map(|v| ReportState::parse(v).map_err(|_| CommandError::InvalidInput))
                        .transpose()?,
                    target_type: request
                        .query
                        .filters
                        .get("target_type")
                        .map(|v| TargetType::parse(v).map_err(|_| CommandError::InvalidInput))
                        .transpose()?,
                    after,
                    limit,
                };
                let values = if matches!(request.action, Operation::OwnReports) {
                    reports.own(actor, filter).await?
                } else {
                    reports.queue(actor, filter).await?
                };
                let cursor = if values.len() == limit as usize {
                    values
                        .last()
                        .map(|v| self.cursors.seal(&binding, v.report_id, Utc::now()))
                        .transpose()?
                } else {
                    None
                };
                (
                    200,
                    "Domain_admin_ReportList",
                    json!({"items":values,"next_cursor":cursor}),
                )
            }
            Operation::ReportDetail(id) => {
                let detail = reports
                    .detail(actor, id)
                    .await?
                    .ok_or(OperationsError::ResourceUnavailable)?;
                (
                    200,
                    "Domain_admin_ReportDetail",
                    json!({"report":detail.report,"reporter_id":detail.reporter_id,"reason":detail.reason,"text":detail.text,"evidence_media_ids":detail.evidence_media_ids}),
                )
            }
            Operation::Settings => {
                let s = operations.settings(actor).await?;
                let e = s.event;
                let f = s.settings.features;
                (
                    200,
                    "Domain_admin_EventSettings",
                    json!({"event":{"event_id":e.id.uuid(),"slug":e.slug.as_str(),"title":e.title.as_str(),"phase":e.phase.as_str(),"timezone":"Asia/Seoul","revision":e.revision.get()},"revision":e.revision.get(),"features":{"parties":f.parties,"giveaways":f.giveaways,"intros":f.intros,"social":f.social,"mural":f.mural},"beacon_ttl_seconds":s.settings.beacon_ttl_seconds,"pickup_grace_seconds":s.settings.pickup_grace_seconds,"social_grant_ttl_seconds":s.settings.social_grant_ttl_seconds,"mural_cooldown_ms":s.settings.mural_cooldown_ms}),
                )
            }
            Operation::Notice(id) => {
                let n = operations
                    .public_notice(actor, id)
                    .await?
                    .ok_or(OperationsError::ResourceUnavailable)?;
                (200, "Domain_feed_Notice", notice_value(n))
            }
            Operation::Notices | Operation::AdminNotices => {
                let state = request
                    .query
                    .filters
                    .get("state")
                    .map(|v| match v.as_str() {
                        "DRAFT" => Ok(NoticeState::Draft),
                        "PUBLISHED" => Ok(NoticeState::Published),
                        "WITHDRAWN" => Ok(NoticeState::Withdrawn),
                        _ => Err(CommandError::InvalidInput),
                    })
                    .transpose()?;
                let notices = if matches!(request.action, Operation::AdminNotices) {
                    operations.admin_notices(actor, after, state, limit).await?
                } else {
                    operations.notices(actor, after, limit).await?
                };
                let cursor = if notices.len() == limit as usize {
                    notices
                        .last()
                        .map(|v| self.cursors.seal(&binding, v.notice_id, Utc::now()))
                        .transpose()?
                } else {
                    None
                };
                (
                    200,
                    "Domain_feed_NoticeList",
                    json!({"items":notices.into_iter().map(notice_value).collect::<Vec<_>>(),"next_cursor":cursor}),
                )
            }
            Operation::Places => {
                let closed = request.query.filters.get("closed").map(|v| v == "true");
                let places = operations.places(actor, after, closed, limit).await?;
                let cursor = if places.len() == limit as usize {
                    places
                        .last()
                        .map(|v| self.cursors.seal(&binding, v.place.id.uuid(), Utc::now()))
                        .transpose()?
                } else {
                    None
                };
                let values=places.into_iter().map(|v|{let p=v.place;let source=p.provenance;json!({"place":{"place_id":p.id.uuid(),"label":p.label,"place_kind":p.kind.as_str(),"verification_state":p.verification.as_str(),"closure_state":p.closure.as_str(),"accessibility_notes":p.accessibility_notes,"provenance":{"source_id":source.source_id,"source_kind":source.source_kind.as_str(),"source_revision":source.source_revision,"source_uri":source.source_uri,"author":source.author,"license":source.license,"review_state":source.review_state.as_str(),"reviewed_by":source.reviewed_by,"reviewed_at":source.reviewed_at,"photo_permission":source.photo_permission.as_str()}},"revision":v.revision.get()})}).collect::<Vec<_>>();
                (
                    200,
                    "Domain_admin_PlaceAdminList",
                    json!({"items":values,"next_cursor":cursor}),
                )
            }
        };
        wire::validate_value(schema, &body).map_err(|_| CommandError::InvalidState)?;
        authority.check_scope().await?;
        // Scope remains held, with its original owner/child disclosure ceiling.
        scope.disclosure_ceiling().map_err(super::authority_error)?;
        Ok(OperationsReply { status, body })
    }
}
pub fn receipt_value(r: SafeReceipt) -> Value {
    json!({"receipt_id":r.id,"event_id":r.scope.event,"actor_id":r.scope.actor,"operation_id":r.scope.operation.id(),"idempotency_key":r.scope.key,"state":r.state.as_str(),"resource_id":r.reference.map(|v|v.resource),"resource_revision":r.reference.map(|v|v.revision),"failure_reason":r.failure.map(|v|v.as_str()),"created_at":r.created_at,"expires_at":r.expires_at,"result_status":r.result_status})
}
fn notice_value(n: Notice) -> Value {
    json!({"notice_id":n.notice_id,"event_id":n.event_id,"title":n.title,"text":n.text,"source_uri":n.source_uri,"confirmed_at":n.confirmed_at,"created_at":n.created_at,"state":n.state,"revision":n.revision})
}

#[async_trait::async_trait]
impl crate::product_rpc::scoped::ScopedProductProducer for OperationsRuntime {
    async fn dispatch(
        &self,
        context: &crate::product_rpc::VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        frame: crate::product_rpc::proto::ProductRequest,
    ) -> Result<crate::product_rpc::scoped::ProductRepresentation, tonic::Status> {
        context.check()?;
        if context.operation().contract_id() != scope.binding().operation() {
            return Err(tonic::Status::permission_denied(
                "Operations scope mismatch",
            ));
        }
        let reply = match OperationsRuntime::dispatch(self, scope, frame).await {
            Ok(reply) => reply,
            Err(error) => error_reply(error, context.request()),
        };
        Ok(crate::product_rpc::scoped::ProductRepresentation {
            status: u32::from(reply.status),
            content_type: "application/json".into(),
            body: serde_json::to_vec(&reply.body)
                .map_err(|_| tonic::Status::internal("Invalid Operations representation"))?,
        })
    }
}
fn error_reply(error: OperationsError, request: uuid::Uuid) -> OperationsReply {
    let (status, reason) = match error {
        OperationsError::ResourceUnavailable => (404, "RESOURCE_UNAVAILABLE"),
        OperationsError::Command(CommandError::InvalidInput) => (400, "INVALID_INPUT"),
        OperationsError::Command(CommandError::Denied) => (403, "PERMISSION_DENIED"),
        OperationsError::Command(CommandError::Conflict) => (409, "IDEMPOTENCY_CONFLICT"),
        OperationsError::Command(CommandError::Expired) => (409, "IDEMPOTENCY_EXPIRED"),
        OperationsError::Command(CommandError::Capacity) => (429, "CAPACITY_EXCEEDED"),
        OperationsError::Command(CommandError::Unavailable) => (503, "DEPENDENCY_UNAVAILABLE"),
        OperationsError::Command(CommandError::InvalidState) => (500, "INTERNAL_ERROR"),
    };
    OperationsReply {
        status,
        body: json!({
            "code":format!("CONCERT_{status}"),"message":"Operations request unavailable",
            "traceId":request,"timestamp":Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),
            "details":{"reason":reason},
        }),
    }
}

/// Report-only public terminal; never includes private input or receipt internals.
fn report_error(status: u16, reason: &str, request: uuid::Uuid) -> Value {
    json!({"code":format!("CONCERT_{status}"),"message":"Report request unavailable",
        "traceId":request,"timestamp":Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),
        "details":{"reason":reason}})
}
