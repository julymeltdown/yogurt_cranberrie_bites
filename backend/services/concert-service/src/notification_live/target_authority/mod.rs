//! Production Notification callback at the ORIGINAL canonical Root registry.
//! Mount with mTLS and guard_without_delegation(POLICY). The opaque original
//! RootReadView, not actor text or a new signature, admits this private callback.
mod native;
mod protocol;
use super::proto;
use crate::adapters::out::relationship_authority::guarded::{OwnerGuardScope, ScopeRegistry};
use chrono::Utc;
use concert_application::notification::{Candidate, Context};
use concert_domain::notification::{TargetType, Visibility};
pub use native::TargetReadSettler;
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use sqlx::{PgConnection, PgPool, Row};
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};
use uuid::Uuid;
const MAX_FRAME: usize = 1024 * 1024;
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "notification.concert.v1.ConcertNotificationTargetAuthority",
    workload: Some(Workload::Concert),
    methods: &[("Authorize", &[Workload::Notification])],
    actor_fields: &[],
};
pub struct TargetAuthorityService {
    pool: PgPool,
    registry: Arc<ScopeRegistry>,
    capacity: Arc<tokio::sync::Semaphore>,
}
impl TargetAuthorityService {
    pub fn new(pool: PgPool, registry: Arc<ScopeRegistry>) -> Self {
        Self {
            pool,
            registry,
            capacity: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }
    pub fn server(self)->proto::concert_notification_target_authority_server::ConcertNotificationTargetAuthorityServer<Self>{
        proto::concert_notification_target_authority_server::ConcertNotificationTargetAuthorityServer::new(self)
            .max_decoding_message_size(MAX_FRAME).max_encoding_message_size(MAX_FRAME)
    }
}
#[tonic::async_trait]
impl proto::concert_notification_target_authority_server::ConcertNotificationTargetAuthority
    for TargetAuthorityService
{
    async fn authorize(
        &self,
        request: Request<proto::TargetAuthorityRequest>,
    ) -> Result<Response<proto::TargetAuthorityReply>, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Notification))
        ) {
            return Err(Status::unauthenticated(
                "Verified Notification workload required",
            ));
        }
        let outer = platform::call_deadline::current()
            .ok_or_else(|| Status::unauthenticated("Original callback deadline required"))?;
        let input = request.into_inner();
        let root = input
            .parent
            .as_ref()
            .and_then(|p| p.root_view.as_ref())
            .ok_or_else(unavailable)?;
        // Keep this permit alive through Auth/Friend enrollment, source locks,
        // durable intent, actual PREPARE, and attachment to this very Root.
        let permit = self.registry.lookup_root_read(root)?;
        let scope = permit.scope();
        let (context, candidates, digest) = protocol::decode(&input, &scope)?;
        let deadline = AuthorityRpcDeadline::within_original(
            context.expires_at.timestamp_micros(),
            outer,
            Duration::from_secs(5),
        )?;
        let stop = deadline.stop().min(scope.binding().deadline().stop());
        let result = tokio::time::timeout_at(stop, async {
            let _capacity = self.capacity.acquire().await.map_err(|_| unavailable())?;
            // Even empty/count snapshots must enroll the canonical actor and
            // retain current event/admission/participant policy in the child.
            scope.public_profile_subject(context.actor_id).await?;
            let mut tx = self.pool.begin().await.map_err(|_| unavailable())?;
            native::budget(&mut tx, deadline).await?;
            actor_policy(&mut tx, context).await?;
            let mut decisions = Vec::with_capacity(candidates.len());
            for (wire, candidate) in input.candidates.iter().zip(candidates) {
                native::budget(&mut tx, deadline).await?;
                let mut allowed = true;
                if let Some(source) = candidate.source_actor_id {
                    allowed &= enroll(&scope, source).await?;
                }
                let mut visibility = crate::notification_target_policy::observe_local_target(
                    &mut tx,
                    context,
                    candidate,
                    Utc::now(),
                )
                .await
                .map_err(|_| unavailable())?;
                // Only inspect already-locked target rows. Owner IDs are never
                // taken from notification hints, reconstructed grants or mocks.
                if visibility == Visibility::Visible {
                    for subject in target_subjects(&mut tx, context, candidate).await? {
                        allowed &= enroll(&scope, subject).await?;
                    }
                }
                if !allowed {
                    visibility = Visibility::Hidden;
                }
                decisions.push(proto::TargetAuthorityDecision {
                    candidate: Some(wire.clone()),
                    visibility: match visibility {
                        Visibility::Visible => {
                            proto::TargetAuthorityVisibility::TargetVisible as i32
                        }
                        Visibility::Stale => proto::TargetAuthorityVisibility::TargetStale as i32,
                        Visibility::Hidden => proto::TargetAuthorityVisibility::TargetHidden as i32,
                    },
                });
            }
            native::prepare(self.pool.clone(), tx, &scope, deadline, digest).await?;
            let checked = Utc::now();
            context.check(checked).map_err(|_| unavailable())?;
            let ceiling = scope.disclosure_ceiling()?;
            ceiling.remaining(Duration::from_secs(5), ceiling.expires_micros())?;
            Ok(Response::new(proto::TargetAuthorityReply {
                scope: Some(scope.producer_scope()),
                request_digest: digest.to_vec(),
                checked_at_unix_micros: checked.timestamp_micros(),
                valid_until_unix_micros: context
                    .expires_at
                    .timestamp_micros()
                    .min(ceiling.expires_micros()),
                decisions,
            }))
        })
        .await
        .map_err(|_| Status::deadline_exceeded("Original target callback deadline"))?;
        drop(permit);
        result
    }
}
async fn enroll(scope: &Arc<OwnerGuardScope>, subject: Uuid) -> Result<bool, Status> {
    match scope.public_profile_subject(subject).await {
        Ok(_) => Ok(true),
        Err(error)
            if subject != scope.binding().actor() && error.code() == tonic::Code::NotFound =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}
async fn actor_policy(c: &mut PgConnection, context: Context) -> Result<(), Status> {
    let row = sqlx::query("select phase,revision from concert_events where event_id=$1 for share")
        .bind(context.event_id)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
    revision(row.try_get("revision").map_err(|_| unavailable())?)?;
    if !matches!(
        row.try_get::<String, _>("phase")
            .map_err(|_| unavailable())?
            .as_str(),
        "OPEN" | "LIVE" | "READ_ONLY"
    ) {
        return Err(Status::permission_denied(
            "Notification event is not accessible",
        ));
    }
    let row=sqlx::query("select state,restriction_revision from concert_event_admissions where event_id=$1 and user_id=$2 for share")
        .bind(context.event_id).bind(context.actor_id).fetch_optional(&mut *c).await.map_err(|_|unavailable())?.ok_or_else(unavailable)?;
    revision(
        row.try_get("restriction_revision")
            .map_err(|_| unavailable())?,
    )?;
    if row
        .try_get::<String, _>("state")
        .map_err(|_| unavailable())?
        != "ACTIVE"
    {
        return Err(Status::permission_denied(
            "Notification actor is restricted",
        ));
    }
    let member: Option<i64> = sqlx::query_scalar(
        "select revision from concert_participants where event_id=$1 and user_id=$2 for share",
    )
    .bind(context.event_id)
    .bind(context.actor_id)
    .fetch_optional(c)
    .await
    .map_err(|_| unavailable())?;
    revision(member.ok_or_else(unavailable)?)
}
fn revision(value: i64) -> Result<(), Status> {
    if (1..=concert_domain::MAX_REVISION).contains(&value) {
        Ok(())
    } else {
        Err(unavailable())
    }
}
async fn target_subjects(
    c: &mut PgConnection,
    context: Context,
    candidate: Candidate,
) -> Result<Vec<Uuid>, Status> {
    let target = candidate.target.ok_or_else(unavailable)?;
    let sql=match target.target_type {
        TargetType::Party=>"select owner_id from concert_community_parties where event_id=$1 and party_id=$2",
        TargetType::Application=>"select p.owner_id,a.user_id from concert_community_applications a join concert_community_parties p using(event_id,party_id) where a.event_id=$1 and a.application_id=$2",
        TargetType::Giveaway=>"select owner_id from concert_community_giveaways where event_id=$1 and giveaway_id=$2",
        TargetType::Reservation=>"select g.owner_id,r.recipient_id from concert_community_reservations r join concert_community_giveaways g using(event_id,giveaway_id) where r.event_id=$1 and r.reservation_id=$2",
        TargetType::SocialRequest=>"select requester_id,recipient_id from concert_social_requests where event_id=$1 and request_id=$2",
        TargetType::Notice=>return Ok(vec![]),
        _=>return Err(unavailable()),
    };
    let row = sqlx::query(sql)
        .bind(context.event_id)
        .bind(target.target_id)
        .fetch_optional(c)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
    let mut subjects = (0..row.len())
        .map(|index| row.try_get::<Uuid, _>(index).map_err(|_| unavailable()))
        .collect::<Result<Vec<_>, _>>()?;
    if subjects.iter().any(Uuid::is_nil) {
        return Err(unavailable());
    }
    subjects.sort();
    subjects.dedup();
    Ok(subjects)
}
fn unavailable() -> Status {
    Status::unavailable("Notification target authority unavailable")
}
