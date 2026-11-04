//! Real mTLS callback to the original canonical Concert root. A response is
//! usable only while Root retains its actual native target exclusion child.
use super::{authority::RootTargetSnapshotOwner, transport::proto, AuthorityFence, Authorization};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::notification::{Candidate, Context};
use concert_domain::notification::{NotificationError, Result, TargetType, Visibility};
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use prost::Message;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request};
use uuid::Uuid;

const BUDGET: Duration = Duration::from_secs(5);
const MAX_FRAME: usize = 1024 * 1024;
type Transport = InterceptedService<Channel, ServiceIdentityClientInterceptor>;

#[derive(Clone)]
pub struct RootTargetClient {
    client: proto::concert_notification_target_authority_client::ConcertNotificationTargetAuthorityClient<Transport>,
}
impl RootTargetClient {
    /// Live-only: TLS is mandatory and the remote workload is exactly Concert.
    pub fn connect(endpoint: &str, token: &str, tls: &TlsMaterial) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            Some(tls),
            endpoint,
            Workload::Concert,
            platform::edge::EdgeBudget {
                edge: "notification->concert-target-authority",
                expected_latency: Duration::from_millis(100),
                deadline: BUDGET,
                connect_deadline: Duration::from_millis(500),
                retry: platform::edge::RetryClass::Never,
                failure_policy: platform::edge::FailurePolicy::FailRequest,
            },
        )?;
        let identity = ServiceIdentityClientInterceptor::new(Workload::Notification, token)?;
        Ok(Self {
            client: proto::concert_notification_target_authority_client::ConcertNotificationTargetAuthorityClient::new(
                InterceptedService::new(channel, identity),
            ).max_decoding_message_size(MAX_FRAME).max_encoding_message_size(MAX_FRAME),
        })
    }
}
fn unavailable(_: impl std::fmt::Display) -> NotificationError {
    NotificationError::Unavailable
}
fn candidate(c: &Candidate) -> proto::TargetAuthorityCandidate {
    proto::TargetAuthorityCandidate {
        notification_id: c.notification_id.to_string(),
        source_actor_id: c.source_actor_id.map(|id| id.to_string()),
        target: c.target.map(|t| proto::Target {
            target_type: match t.target_type {
                TargetType::Party => proto::TargetType::TargetParty,
                TargetType::Application => proto::TargetType::Application,
                TargetType::Giveaway => proto::TargetType::TargetGiveaway,
                TargetType::Reservation => proto::TargetType::Reservation,
                TargetType::SocialRequest => proto::TargetType::SocialRequest,
                TargetType::Post => proto::TargetType::Post,
                TargetType::Notice => proto::TargetType::TargetNotice,
                TargetType::Report => proto::TargetType::Report,
            } as i32,
            target_id: t.target_id.to_string(),
        }),
    }
}
struct RetainedTargetFence {
    context: Context,
    valid_until: DateTime<Utc>,
    stop: Instant,
}
impl AuthorityFence for RetainedTargetFence {
    fn check(&self, now: DateTime<Utc>) -> Result<()> {
        self.context.check(now)?;
        if now >= self.valid_until || Utc::now() >= self.valid_until || Instant::now() >= self.stop
        {
            return Err(NotificationError::Unavailable);
        }
        Ok(())
    }
}
#[async_trait]
impl RootTargetSnapshotOwner for RootTargetClient {
    async fn authorize(
        &self,
        parent: &proto::ParentScope,
        context: Context,
        candidates: &[Candidate],
        now: DateTime<Utc>,
    ) -> Result<Authorization> {
        context.check(now)?;
        context.check(Utc::now())?;
        let outer = platform::call_deadline::current().ok_or(NotificationError::Unavailable)?;
        let deadline = AuthorityRpcDeadline::within_original(
            context.expires_at.timestamp_micros(),
            outer,
            BUDGET,
        )
        .map_err(unavailable)?;
        let message = proto::TargetAuthorityRequest {
            parent: Some(parent.clone()),
            context: Some(proto::TargetAuthorityContext {
                actor_id: context.actor_id.to_string(),
                event_id: context.event_id.to_string(),
                session_family_id: context.session_family_id.to_string(),
                request_id: context.request_id.to_string(),
                authority_digest: context.authority_digest.to_vec(),
                observed_at_unix_micros: context.observed_at.timestamp_micros(),
                expires_at_unix_micros: context.expires_at.timestamp_micros(),
            }),
            candidates: candidates.iter().map(candidate).collect(),
            call_id: Uuid::new_v4().to_string(),
        };
        if message.encoded_len() > MAX_FRAME {
            return Err(NotificationError::Unavailable);
        }
        let expected_digest: [u8; 32] = Sha256::digest(message.encode_to_vec()).into();
        let original_candidates = message.candidates.clone();
        let original_scope = parent
            .root_view
            .as_ref()
            .and_then(|root| root.scope.clone())
            .ok_or(NotificationError::PermissionDenied)?;
        let remaining = deadline
            .remaining(BUDGET, deadline.expires_micros())
            .map_err(unavailable)?;
        let mut request = Request::new(message);
        request.set_timeout(remaining);
        request.metadata_mut().insert(
            platform::call_deadline::REMAINING_HEADER,
            platform::call_deadline::encode_grpc_timeout(remaining)
                .parse()
                .map_err(unavailable)?,
        );
        let mut client = self.client.clone();
        let reply = tokio::time::timeout_at(deadline.stop(), client.authorize(request))
            .await
            .map_err(unavailable)?
            .map_err(unavailable)?
            .into_inner();
        let checked_at = DateTime::from_timestamp_micros(reply.checked_at_unix_micros)
            .ok_or(NotificationError::Unavailable)?;
        let until = DateTime::from_timestamp_micros(reply.valid_until_unix_micros)
            .ok_or(NotificationError::Unavailable)?;
        if reply.scope != Some(original_scope)
            || reply.request_digest != expected_digest
            || checked_at < context.observed_at
            || checked_at > Utc::now()
            || until > context.expires_at
            || until <= Utc::now()
            || reply.decisions.len() != candidates.len()
        {
            return Err(NotificationError::Unavailable);
        }
        let decisions = reply
            .decisions
            .into_iter()
            .zip(original_candidates)
            .zip(candidates)
            .map(|((decision, wire), original)| {
                if decision.candidate != Some(wire) {
                    return Err(NotificationError::Unavailable);
                }
                let visibility =
                    match proto::TargetAuthorityVisibility::try_from(decision.visibility) {
                        Ok(proto::TargetAuthorityVisibility::TargetVisible) => Visibility::Visible,
                        Ok(proto::TargetAuthorityVisibility::TargetStale) => Visibility::Stale,
                        Ok(proto::TargetAuthorityVisibility::TargetHidden) => Visibility::Hidden,
                        _ => return Err(NotificationError::Unavailable),
                    };
                Ok((*original, visibility))
            })
            .collect::<Result<Vec<_>>>()?;
        let fence = RetainedTargetFence {
            context,
            valid_until: until,
            stop: deadline
                .stop()
                .into_std()
                .min(Instant::now() + (until - Utc::now()).to_std().map_err(unavailable)?),
        };
        fence.check(Utc::now())?;
        Ok(Authorization {
            context,
            checked_at,
            valid_until: until,
            decisions,
            fence: Box::new(fence),
        })
    }
}
