//! Read-only decision recovery. The caller cannot choose an outcome, create a
//! registry row, extend the original mutation, or release an owner exclusion.
use crate::participant::{context::BUDGET, proto};
use chrono::{DateTime, Utc};
use platform::{
    actor_delegation::{read_authority::VerifiedReadAuthority, VerifiedActor},
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use sqlx::{PgPool, Row};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub const DECISION_POLICY: MethodPolicy = MethodPolicy {
    service: "concert.participant.v1.ConcertParticipantDecisionService",
    workload: Some(Workload::Concert),
    methods: &[("ResolveDecision", &[Workload::Profile])],
    actor_fields: &[("ResolveDecision", &[&[1]])],
};
#[derive(Clone)]
pub struct ParticipantDecisionService {
    pool: PgPool,
}
impl ParticipantDecisionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub fn server(self) -> proto::concert_participant_decision_service_server::ConcertParticipantDecisionServiceServer<Self>{
        proto::concert_participant_decision_service_server::ConcertParticipantDecisionServiceServer::new(self)
            .max_decoding_message_size(4096).max_encoding_message_size(4096)
    }
    async fn resolve(
        &self,
        query: &proto::ParticipantDecisionRequest,
    ) -> Result<proto::ParticipantDecisionResponse, Status> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let row = sqlx::query("select decision,committed_revision,expected_revision,concert_system_identifier,concert_database_oid,concert_full_xid,concert_backend_pid,concert_backend_started from public.concert_participant_profile_operations where operation_id=$1 and actor_id=$2 and event_id=$3 and session_family_id=$4 and parent_digest=$5 and body_digest=$6 and guard_binding_digest=$7 and profile_prepared_gid=$8 and profile_effect_digest=$9")
            .bind(canonical(&query.operation_id)?).bind(canonical(&query.actor_id)?).bind(canonical(&query.event_id)?).bind(canonical(&query.session_family_id)?)
            .bind(&query.parent_digest).bind(&query.body_digest).bind(&query.guard_binding_digest).bind(&query.profile_prepared_gid).bind(&query.profile_effect_digest)
            .fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let mut response = proto::ParticipantDecisionResponse {
            query: Some(query.clone()),
            decision: proto::ParticipantDecision::Unresolved as i32,
            concert_transaction: None,
            committed_revision: 0,
        };
        if let Some(row) = row {
            let system: String = row
                .try_get("concert_system_identifier")
                .map_err(unavailable)?;
            let oid: i64 = row.try_get("concert_database_oid").map_err(unavailable)?;
            let actual: (String,i64) = sqlx::query_as("select (select system_identifier::text from pg_control_system()),(select oid::bigint from pg_database where datname=current_database())")
                .fetch_one(&mut *tx).await.map_err(unavailable)?;
            if actual != (system.clone(), oid) {
                return Err(Status::unavailable("Decision store identity mismatch"));
            }
            let xid: String = row.try_get("concert_full_xid").map_err(unavailable)?;
            // Resolve only this store's actual retained full XID. Errors such as
            // aged-out status produce no decision and never mean ABORT.
            let outcome: Option<String> =
                sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
                    .bind(&xid)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(unavailable)?;
            let registered_decision: String = row.try_get("decision").map_err(unavailable)?;
            let committed_revision: Option<i64> =
                row.try_get("committed_revision").map_err(unavailable)?;
            let expected_revision: i64 = row.try_get("expected_revision").map_err(unavailable)?;
            let started: DateTime<Utc> = row
                .try_get("concert_backend_started")
                .map_err(unavailable)?;
            response.concert_transaction = Some(proto::ParticipantDecisionTransaction {
                system_identifier: system,
                database_oid: u32::try_from(oid).map_err(unavailable)?,
                full_xid: xid,
                backend_pid: row.try_get("concert_backend_pid").map_err(unavailable)?,
                backend_started_unix_micros: started.timestamp_micros(),
            });
            match (
                outcome.as_deref(),
                registered_decision.as_str(),
                committed_revision,
            ) {
                (Some("committed"), "COMMIT", Some(revision))
                    if revision == expected_revision.saturating_add(1) && revision > 1 =>
                {
                    response.decision = proto::ParticipantDecision::Commit as i32;
                    response.committed_revision = revision;
                }
                (Some("aborted"), "PENDING", None) => {
                    response.decision = proto::ParticipantDecision::Abort as i32
                }
                _ => {}
            }
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(response)
    }
}
fn canonical(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("Invalid participant decision binding"))
}
fn unavailable(_: impl std::fmt::Display) -> Status {
    Status::unavailable("Participant decision unavailable")
}
#[tonic::async_trait]
impl proto::concert_participant_decision_service_server::ConcertParticipantDecisionService
    for ParticipantDecisionService
{
    async fn resolve_decision(
        &self,
        request: Request<proto::ParticipantDecisionRequest>,
    ) -> Result<Response<proto::ParticipantDecisionResponse>, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Profile))
        ) {
            return Err(Status::unauthenticated("Verified Profile required"));
        }
        let actor = request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified decision actor required"))?
            .0;
        let proof = request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .ok_or_else(|| Status::unauthenticated("Bounded decision query required"))?;
        let query = request.get_ref();
        if canonical(&query.actor_id)? != actor || proof.actor() != actor {
            return Err(Status::permission_denied("Decision actor mismatch"));
        }
        canonical(&query.event_id)?;
        canonical(&query.session_family_id)?;
        let operation = canonical(&query.operation_id)?;
        if query.profile_prepared_gid != format!("mcr_sp_{}", operation.simple())
            || [
                &query.parent_digest,
                &query.body_digest,
                &query.guard_binding_digest,
                &query.profile_effect_digest,
            ]
            .iter()
            .any(|bytes| bytes.len() != 32 || bytes.iter().all(|byte| *byte == 0))
        {
            return Err(Status::invalid_argument("Invalid decision effect binding"));
        }
        let original = proof.deadline(BUDGET)?;
        if query.authority_expires_unix_micros > original.expires_micros() {
            return Err(Status::permission_denied(
                "Decision query exceeds original expiry",
            ));
        }
        let deadline =
            platform::actor_delegation::authority_deadline::AuthorityRpcDeadline::within_original(
                query.authority_expires_unix_micros,
                original.stop().into_std(),
                BUDGET,
            )?;
        let result = platform::call_deadline::scope(
            deadline.stop().into_std(),
            tokio::time::timeout_at(deadline.stop(), self.resolve(query)),
        )
        .await;
        deadline.remaining(BUDGET, query.authority_expires_unix_micros)?;
        let value =
            result.map_err(|_| Status::deadline_exceeded("Decision query deadline elapsed"))??;
        let mut response = Response::new(value);
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(response)
    }
}
