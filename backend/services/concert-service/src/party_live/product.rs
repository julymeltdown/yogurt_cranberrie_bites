//! Root-scoped Product producer for the bounded Party routes.
use super::{
    PartyLiveError, PartyLiveOwnerGuard, PartyLiveReply, PartyLiveService, ScopePartyLiveGuard,
};
use crate::{
    adapters::out::{
        public_profile::PublicProfileReader, relationship_authority::guarded::OwnerGuardScope,
    },
    product_rpc::{
        proto, scoped::ProductRepresentation, scoped::ScopedProductProducer, VerifiedProduct,
    },
};
use async_trait::async_trait;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

/// Production factory: construct BE37 authority from the exact acquired Root
/// scope for every request. Resources must enforce actual feature/place policy;
/// this type provides no fallback resource grant or second owner acquisition.
pub struct ScopedPartyProductProducer<R> {
    pub pool: sqlx::PgPool,
    pub keys: Arc<concert_application::commands::FingerprintKeys>,
    pub cursors: super::PartyCursorCodec,
    pub profiles: Arc<PublicProfileReader>,
    pub resources: R,
}

#[async_trait]
impl<R> ScopedProductProducer for ScopedPartyProductProducer<R>
where
    R: crate::community::policy::CommunityResources + Clone + 'static,
{
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let authority = crate::community::policy::SharedCommunityAuthority::new(
            crate::relationship::PgRelationshipPolicy::new(
                self.pool.clone(),
                crate::adapters::out::relationship_authority::guarded::GuardedAuthority::new(
                    scope.clone(),
                ),
            ),
            self.resources.clone(),
        );
        let producer = PartyProductProducer {
            pool: self.pool.clone(),
            service: Arc::new(PartyLiveService::new(
                self.pool.clone(),
                Arc::new(authority),
                self.keys.clone(),
                self.cursors.clone(),
                Arc::new(super::UnavailablePartyLiveOwnerFence),
            )),
            profiles: self.profiles.clone(),
        };
        producer.dispatch(context, scope, request).await
    }
}

pub struct PartyProductProducer<A, F> {
    pub pool: sqlx::PgPool,
    pub service: Arc<PartyLiveService<A, F>>,
    pub profiles: Arc<PublicProfileReader>,
}

#[async_trait]
impl<A, F> ScopedProductProducer for PartyProductProducer<A, F>
where
    A: crate::community::CommunityAuthority + 'static,
    F: super::PartyLiveOwnerFence + 'static,
{
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        if !matches!(
            context.operation(),
            crate::product_rpc::Operation::ListParties
                | crate::product_rpc::Operation::GetParty(_)
                | crate::product_rpc::Operation::CreateParty
                | crate::product_rpc::Operation::ApplyParty(_)
                | crate::product_rpc::Operation::DecidePartyApplication(_)
                | crate::product_rpc::Operation::CancelPartyApplication(_)
                | crate::product_rpc::Operation::RemovePartyMember(_, _)
                | crate::product_rpc::Operation::CancelParty(_)
        ) {
            return Err(Status::unimplemented("Party producer route mismatch"));
        }
        let parsed = super::PartyLiveRequest::from_owner_scope(scope.binding(), &request)
            .map_err(party_status)?;
        preflight_pairs(&self.pool, &parsed)
            .await
            .map_err(party_status)?;
        let binding = parsed.binding;
        let guard: Arc<dyn PartyLiveOwnerGuard> = Arc::new(
            ScopePartyLiveGuard::new(scope.clone(), self.profiles.clone(), binding)
                .map_err(party_status)?,
        );
        let reply: PartyLiveReply = match self
            .service
            .dispatch_for_scope(scope, guard, request)
            .await
        {
            Ok(reply) => reply,
            Err(error) => PartyLiveReply {
                status: error.status(),
                body: serde_json::json!({
                    "code": format!("CONCERT_{}", error.status()),
                    "message": "Concert request unavailable",
                    "traceId": context.request().to_string(),
                    "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    "details": {"reason": error.reason()},
                }),
            },
        };
        Ok(ProductRepresentation {
            status: reply.status.into(),
            content_type: "application/json".into(),
            body: serde_json::to_vec(&reply.body)
                .map_err(|_| Status::internal("Invalid Party representation"))?,
        })
    }
}

/// Commits the pair rows a command will need before its own transaction
/// starts. A public Profile child prepares a separate transaction that takes
/// SHARE on the same pair; if the command instead inserts that row first, the
/// child's `ON CONFLICT` insert waits on the uncommitted row and the command
/// can never commit. Preflight is metadata only and grants no access.
async fn preflight_pairs(
    pool: &sqlx::PgPool,
    parsed: &super::PartyLiveRequest,
) -> Result<(), PartyLiveError> {
    let binding = &parsed.binding;
    let actor = binding.actor();
    let event = binding.event();
    let mut subjects = vec![actor];
    let owner = |party: Uuid| async move {
        sqlx::query_scalar::<_, Uuid>(
            "select owner_id from public.concert_community_parties where event_id=$1 and party_id=$2",
        )
        .bind(event)
        .bind(party)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    };
    match &parsed.action {
        super::PartyLiveAction::List(_) => {
            if let Ok(owners) = sqlx::query_scalar::<_, Uuid>(
                "select distinct owner_id from public.concert_community_parties where event_id=$1",
            )
            .bind(event)
            .fetch_all(pool)
            .await
            {
                subjects.extend(owners);
            }
        }
        super::PartyLiveAction::Detail(party)
        | super::PartyLiveAction::Apply { party, .. }
        | super::PartyLiveAction::Remove { party, .. }
        | super::PartyLiveAction::CancelParty { party, .. } => {
            if let Some(owner) = owner(*party).await {
                subjects.push(owner);
            }
        }
        super::PartyLiveAction::Decide { application, .. }
        | super::PartyLiveAction::CancelApplication { application, .. } => {
            if let Ok(Some((applicant, owner))) = sqlx::query_as::<_, (Uuid, Uuid)>(
                "select a.user_id,p.owner_id from public.concert_community_applications a join public.concert_community_parties p on p.event_id=a.event_id and p.party_id=a.party_id where a.event_id=$1 and a.application_id=$2",
            )
            .bind(event)
            .bind(application)
            .fetch_optional(pool)
            .await
            {
                subjects.push(applicant);
                subjects.push(owner);
            }
        }
        super::PartyLiveAction::Create(_) => {}
    }
    if let super::PartyLiveAction::Remove { user, .. } = &parsed.action {
        subjects.push(*user);
    }
    subjects.sort_unstable();
    subjects.dedup();
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| PartyLiveError::DependencyUnavailable)?;
    let until =
        chrono::DateTime::<chrono::Utc>::from_timestamp_micros(binding.deadline().expires_micros())
            .ok_or(PartyLiveError::DependencyUnavailable)?;
    crate::relationship::pin_deadline(&mut tx, until)
        .await
        .map_err(|_| PartyLiveError::DependencyUnavailable)?;
    for subject in subjects {
        let (low, high) = if actor <= subject {
            (actor, subject)
        } else {
            (subject, actor)
        };
        sqlx::query(
            "insert into public.concert_relationship_pairs(event_id,low_user,high_user) values($1,$2,$3) on conflict do nothing",
        )
        .bind(event)
        .bind(low)
        .bind(high)
        .execute(&mut *tx)
        .await
        .map_err(|_| PartyLiveError::DependencyUnavailable)?;
    }
    tx.commit()
        .await
        .map_err(|_| PartyLiveError::DependencyUnavailable)
}

fn party_status(error: PartyLiveError) -> Status {
    match error {
        PartyLiveError::InvalidInput => Status::invalid_argument("Invalid Party request"),
        PartyLiveError::Denied => Status::permission_denied("Party authorization denied"),
        PartyLiveError::ResourceUnavailable => Status::not_found("Party unavailable"),
        PartyLiveError::VersionConflict => Status::aborted("Party revision conflict"),
        PartyLiveError::InvalidTransition | PartyLiveError::EventReadOnly => {
            Status::failed_precondition("Party transition unavailable")
        }
        PartyLiveError::PartyFull | PartyLiveError::DuplicateApplication => {
            Status::already_exists("Party command conflict")
        }
        PartyLiveError::DependencyUnavailable | PartyLiveError::Internal => {
            Status::unavailable("Party dependency unavailable")
        }
    }
}
