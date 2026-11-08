//! Signed profile product dispatch.
//!
//! This is the profile analogue of `concert_product`: the Gateway captures the
//! browser request, proves the actor through the existing Auth/Profile identity
//! composition, and forwards one signed `ProfileRequest` to the profile service.
//! Nothing here decides authorization; the profile producer does.
//!
//! The one difference that matters is the absence of an event. There is no
//! `event_id` on the wire and no event prefix in the target, and a target that
//! names an event path is refused before dispatch so a caller cannot smuggle an
//! event-scoped operation into the profile authority path.
use super::{concert_identity::ConcertIdentityReader, jwt::concert_access::ConcertAccessContext};
use platform::{
    actor_delegation::{request_for, ActorDelegationClient, ActorDelegationSigner},
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("profile.product.v1");
}

/// The concert-hosted counterpart of [`proto`].
///
/// Fifteen frozen profile operations read tables that live in `mcr_concert`
/// (`profile_posts`, `profile_post_attachments`, `profile_media`,
/// `profile_rooms`, `profile_avatars`). `domain-contract.md` §1.1 forbids a
/// cross-database join, so those operations are dispatched to the concert
/// service's own profile transport instead. The message shape is identical on
/// purpose: the authority discipline is the same, only the serving workload and
/// the database differ.
pub mod concert_proto {
    tonic::include_proto!("concert.profile_product.v1");
}

pub use crate::product_http::MAX_BODY;

const BUDGET: Duration = Duration::from_secs(2);
const EDGE: EdgeBudget = EdgeBudget {
    edge: "gateway.profile.product",
    expected_latency: Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};

/// The captured HTTP input for one profile product request.
pub struct ProfileHttpRequest {
    pub request_id: Uuid,
    pub method: String,
    pub target: String,
    pub body: Vec<u8>,
    pub content_type: String,
    pub idempotency_key: String,
}

#[derive(Clone)]
pub struct GrpcProfileProductClient {
    client: proto::profile_product_service_client::ProfileProductServiceClient<
        InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>,
    >,
    delivery: proto::profile_product_delivery_service_client::ProfileProductDeliveryServiceClient<
        InterceptedService<Channel, ServiceIdentityClientInterceptor>,
    >,
}

impl GrpcProfileProductClient {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Profile,
            EDGE.validate()?,
        )?;
        Ok(Self {
            client: proto::profile_product_service_client::ProfileProductServiceClient::with_interceptor(
                ActorDelegationClient::new(channel.clone(), signer, Workload::Profile),
                ServiceIdentityClientInterceptor::new(Workload::Gateway, token)?,
            )
            .max_decoding_message_size(MAX_BODY + 16 * 1024)
            .max_encoding_message_size(MAX_BODY + 16 * 1024),
            delivery: proto::profile_product_delivery_service_client::ProfileProductDeliveryServiceClient::with_interceptor(
                channel,
                ServiceIdentityClientInterceptor::new(Workload::Gateway, token)?,
            )
            .max_decoding_message_size(4096)
            .max_encoding_message_size(4096),
        })
    }

    pub async fn dispatch(
        &self,
        reader: &ConcertIdentityReader,
        context: &ConcertAccessContext,
        http: ProfileHttpRequest,
    ) -> Result<proto::ProfileResponse, Status> {
        if http.request_id.is_nil()
            || http.body.len() > MAX_BODY
            || http.target.len() > 4096
            || http.target.contains("/events/")
            || !http.target.starts_with("/api/")
        {
            return Err(Status::invalid_argument("Invalid profile product request"));
        }
        context
            .check()
            .map_err(|_| Status::unauthenticated("Invalid profile access"))?;
        let original = context
            .rpc_deadline()
            .map_err(|_| Status::deadline_exceeded("Profile deadline expired"))?;
        let mut stop = original.stop();
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let work = async {
            // The same Auth/Profile identity composition the concert product
            // uses. It proves the actor is live and that the profile is current;
            // it is not a grant for any particular profile operation.
            let identity = Box::pin(reader.read(context, None))
                .await
                .map_err(|_| Status::unavailable("Profile identity unavailable"))?;
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Profile identity expired"))?;
            let actor = identity
                .user_id()
                .map_err(|_| Status::unauthenticated("Invalid profile identity"))?;
            let auth_at = identity
                .admission_observed_at()
                .map_err(|_| Status::unavailable("Profile identity unavailable"))?
                .timestamp_micros();
            let profile_at = identity
                .profile_observed_at()
                .map_err(|_| Status::unavailable("Profile profile unavailable"))?
                .timestamp_micros();
            let now = chrono::Utc::now().timestamp_micros();
            let remaining = context
                .remaining(BUDGET)
                .map_err(|_| Status::deadline_exceeded("Profile deadline expired"))?
                .min(stop.saturating_duration_since(tokio::time::Instant::now()));
            let expires = original
                .expires_micros()
                .min(context.access_expires_micros())
                .min(now.saturating_add(remaining.as_micros() as i64))
                .min(auth_at.saturating_add(BUDGET.as_micros() as i64))
                .min(profile_at.saturating_add(BUDGET.as_micros() as i64));
            if remaining.is_zero() || expires <= now {
                return Err(Status::deadline_exceeded("Profile deadline expired"));
            }
            let message = proto::ProfileRequest {
                user_id: actor.to_string(),
                // The profile id is the actor's own canonical profile. The
                // producer still authorizes every target against the session.
                profile_id: actor.to_string(),
                session_family_id: context
                    .session_family()
                    .map_err(|_| Status::unauthenticated("Invalid profile session"))?
                    .to_string(),
                request_id: http.request_id.to_string(),
                http_method: http.method,
                http_target: http.target,
                http_body: http.body,
                content_type: http.content_type,
                idempotency_key: http.idempotency_key,
                authority_expires_unix_micros: expires,
                auth_observed_at_unix_micros: auth_at,
                profile_observed_at_unix_micros: profile_at,
                access_expires_unix_micros: context.access_expires_micros(),
                trusted_network_bucket: Vec::new(),
            };
            let mut request = request_for(message, &actor.to_string())?;
            request.set_timeout(remaining.min(Duration::from_micros((expires - now) as u64)));
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Profile identity expired"))?;
            let mut client = self.client.clone();
            let response = Box::pin(client.dispatch(request)).await?;
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Profile identity expired"))?;
            if chrono::Utc::now().timestamp_micros() >= expires {
                return Err(Status::deadline_exceeded("Profile deadline expired"));
            }
            if response
                .metadata()
                .get("cache-control")
                .and_then(|value| value.to_str().ok())
                != Some("no-store")
            {
                return Err(Status::internal("Invalid profile response"));
            }
            let response = response.into_inner();
            if response.body.is_empty() || response.body.len() > MAX_BODY {
                return Err(Status::internal("Invalid profile response"));
            }
            Ok(response)
        };
        platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, Box::pin(work))
                .await
                .map_err(|_| Status::deadline_exceeded("Profile deadline expired"))?
        })
        .await
    }

    /// Internal capability only. It never enters browser JSON, headers, caches or
    /// logs. The outermost HTTP body layer invokes it only after body EOF.
    pub fn delivery_token(
        &self,
        capability: proto::ProfileResponseDelivery,
    ) -> Result<platform::actor_delegation::response_scope::ResponseScopeToken, Status> {
        validate_delivery(&capability)?;
        Ok(
            platform::actor_delegation::response_scope::ResponseScopeToken::new(
                std::sync::Arc::new(HttpDelivery {
                    client: self.clone(),
                    capability,
                }),
            ),
        )
    }
}

/// The signed client for the concert-hosted profile transport.
///
/// This is the same dispatch as [`GrpcProfileProductClient`], addressed to the
/// Concert workload and using the concert-side message types. It exists as a
/// separate type rather than a generic parameter so that the two clients cannot
/// be confused at a call site: the routing rule in
/// `adapters::in::http::profile_owner` decides which one is used, and a wrong
/// choice is a type error rather than a runtime surprise.
#[derive(Clone)]
pub struct GrpcConcertProfileProductClient {
    client: concert_proto::profile_product_service_client::ProfileProductServiceClient<
        InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>,
    >,
    delivery:
        concert_proto::profile_product_delivery_service_client::ProfileProductDeliveryServiceClient<
            InterceptedService<Channel, ServiceIdentityClientInterceptor>,
        >,
}

impl GrpcConcertProfileProductClient {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Concert,
            EDGE.validate()?,
        )?;
        Ok(Self {
            client: concert_proto::profile_product_service_client::ProfileProductServiceClient::with_interceptor(
                ActorDelegationClient::new(channel.clone(), signer, Workload::Concert),
                ServiceIdentityClientInterceptor::new(Workload::Gateway, token)?,
            )
            .max_decoding_message_size(MAX_BODY + 16 * 1024)
            .max_encoding_message_size(MAX_BODY + 16 * 1024),
            delivery: concert_proto::profile_product_delivery_service_client::ProfileProductDeliveryServiceClient::with_interceptor(
                channel,
                ServiceIdentityClientInterceptor::new(Workload::Gateway, token)?,
            )
            .max_decoding_message_size(4096)
            .max_encoding_message_size(4096),
        })
    }

    /// Dispatch one profile operation to the concert service.
    ///
    /// The authority construction is byte-for-byte the profile client's: the
    /// same identity composition, the same observation bounds, the same
    /// deadline arithmetic, the same parent signing. Only the audience workload
    /// and the message type differ.
    pub async fn dispatch(
        &self,
        reader: &ConcertIdentityReader,
        context: &ConcertAccessContext,
        http: ProfileHttpRequest,
    ) -> Result<concert_proto::ProfileResponse, Status> {
        if http.request_id.is_nil()
            || http.body.len() > MAX_BODY
            || http.target.len() > 4096
            || http.target.contains("/events/")
            || !http.target.starts_with("/api/")
        {
            return Err(Status::invalid_argument("Invalid profile product request"));
        }
        context
            .check()
            .map_err(|_| Status::unauthenticated("Invalid profile access"))?;
        let original = context
            .rpc_deadline()
            .map_err(|_| Status::deadline_exceeded("Profile deadline expired"))?;
        let mut stop = original.stop();
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let work = async {
            let identity = Box::pin(reader.read(context, None))
                .await
                .map_err(|_| Status::unavailable("Profile identity unavailable"))?;
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Profile identity expired"))?;
            let actor = identity
                .user_id()
                .map_err(|_| Status::unauthenticated("Invalid profile identity"))?;
            let auth_at = identity
                .admission_observed_at()
                .map_err(|_| Status::unavailable("Profile identity unavailable"))?
                .timestamp_micros();
            let profile_at = identity
                .profile_observed_at()
                .map_err(|_| Status::unavailable("Profile profile unavailable"))?
                .timestamp_micros();
            let now = chrono::Utc::now().timestamp_micros();
            let remaining = context
                .remaining(BUDGET)
                .map_err(|_| Status::deadline_exceeded("Profile deadline expired"))?
                .min(stop.saturating_duration_since(tokio::time::Instant::now()));
            let expires = original
                .expires_micros()
                .min(context.access_expires_micros())
                .min(now.saturating_add(remaining.as_micros() as i64))
                .min(auth_at.saturating_add(BUDGET.as_micros() as i64))
                .min(profile_at.saturating_add(BUDGET.as_micros() as i64));
            if remaining.is_zero() || expires <= now {
                return Err(Status::deadline_exceeded("Profile deadline expired"));
            }
            let message = concert_proto::ProfileRequest {
                user_id: actor.to_string(),
                profile_id: actor.to_string(),
                session_family_id: context
                    .session_family()
                    .map_err(|_| Status::unauthenticated("Invalid profile session"))?
                    .to_string(),
                request_id: http.request_id.to_string(),
                http_method: http.method,
                http_target: http.target,
                http_body: http.body,
                content_type: http.content_type,
                idempotency_key: http.idempotency_key,
                authority_expires_unix_micros: expires,
                auth_observed_at_unix_micros: auth_at,
                profile_observed_at_unix_micros: profile_at,
                access_expires_unix_micros: context.access_expires_micros(),
                trusted_network_bucket: Vec::new(),
            };
            let mut request = request_for(message, &actor.to_string())?;
            request.set_timeout(remaining.min(Duration::from_micros((expires - now) as u64)));
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Profile identity expired"))?;
            let mut client = self.client.clone();
            let response = Box::pin(client.dispatch(request)).await?;
            identity
                .check()
                .map_err(|_| Status::unauthenticated("Profile identity expired"))?;
            if chrono::Utc::now().timestamp_micros() >= expires {
                return Err(Status::deadline_exceeded("Profile deadline expired"));
            }
            if response
                .metadata()
                .get("cache-control")
                .and_then(|value| value.to_str().ok())
                != Some("no-store")
            {
                return Err(Status::internal("Invalid profile response"));
            }
            let response = response.into_inner();
            if response.body.is_empty() || response.body.len() > MAX_BODY {
                return Err(Status::internal("Invalid profile response"));
            }
            Ok(response)
        };
        platform::call_deadline::scope(stop.into_std(), async {
            tokio::time::timeout_at(stop, Box::pin(work))
                .await
                .map_err(|_| Status::deadline_exceeded("Profile deadline expired"))?
        })
        .await
    }

    /// Internal capability only. It never enters browser JSON, headers, caches
    /// or logs. The outermost HTTP body layer invokes it only after body EOF.
    pub fn delivery_token(
        &self,
        capability: concert_proto::ProfileResponseDelivery,
    ) -> Result<platform::actor_delegation::response_scope::ResponseScopeToken, Status> {
        validate_concert_delivery(&capability)?;
        Ok(
            platform::actor_delegation::response_scope::ResponseScopeToken::new(
                std::sync::Arc::new(ConcertHttpDelivery {
                    client: self.clone(),
                    capability,
                }),
            ),
        )
    }
}

fn validate_concert_delivery(value: &concert_proto::ProfileResponseDelivery) -> Result<(), Status> {
    for text in [&value.instance_id, &value.scope_id] {
        if !Uuid::parse_str(text)
            .ok()
            .is_some_and(|id| !id.is_nil() && id.to_string() == *text)
        {
            return Err(Status::internal("Invalid profile delivery capability"));
        }
    }
    if value.binding_digest.len() != 32
        || value.binding_digest.iter().all(|byte| *byte == 0)
        || value.completion_secret.len() != 32
    {
        return Err(Status::internal("Invalid profile delivery capability"));
    }
    Ok(())
}

struct ConcertHttpDelivery {
    client: GrpcConcertProfileProductClient,
    capability: concert_proto::ProfileResponseDelivery,
}

impl platform::actor_delegation::response_scope::ResponseScope for ConcertHttpDelivery {
    fn finish_response(
        self: std::sync::Arc<Self>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Status>> + Send + 'static>>
    {
        Box::pin(async move {
            tokio::spawn(async move {
                let stop = tokio::time::Instant::now() + BUDGET;
                platform::call_deadline::scope(stop.into_std(), async move {
                    let mut request = tonic::Request::new(self.capability.clone());
                    request.set_timeout(BUDGET);
                    let mut client = self.client.delivery.clone();
                    let result = tokio::time::timeout_at(stop, client.complete_response(request))
                        .await
                        .map_err(|_| {
                            Status::deadline_exceeded("Profile delivery completion unknown")
                        })??;
                    if !result.into_inner().completed {
                        return Err(Status::unavailable("Profile delivery completion unknown"));
                    }
                    Ok(())
                })
                .await
            })
            .await
            .map_err(|_| Status::unavailable("Profile delivery completion interrupted"))?
        })
    }
}

fn validate_delivery(value: &proto::ProfileResponseDelivery) -> Result<(), Status> {
    for text in [&value.instance_id, &value.scope_id] {
        if !Uuid::parse_str(text)
            .ok()
            .is_some_and(|id| !id.is_nil() && id.to_string() == *text)
        {
            return Err(Status::internal("Invalid profile delivery capability"));
        }
    }
    if value.binding_digest.len() != 32
        || value.binding_digest.iter().all(|byte| *byte == 0)
        || value.completion_secret.len() != 32
    {
        return Err(Status::internal("Invalid profile delivery capability"));
    }
    Ok(())
}

struct HttpDelivery {
    client: GrpcProfileProductClient,
    capability: proto::ProfileResponseDelivery,
}

impl platform::actor_delegation::response_scope::ResponseScope for HttpDelivery {
    fn finish_response(
        self: std::sync::Arc<Self>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Status>> + Send + 'static>>
    {
        Box::pin(async move {
            // A separate system operation has a fresh settlement-only budget.
            // Spawning avoids inheriting or renewing an expired content deadline.
            tokio::spawn(async move {
                let stop = tokio::time::Instant::now() + BUDGET;
                platform::call_deadline::scope(stop.into_std(), async move {
                    let mut request = tonic::Request::new(self.capability.clone());
                    request.set_timeout(BUDGET);
                    let mut client = self.client.delivery.clone();
                    let result = tokio::time::timeout_at(stop, client.complete_response(request))
                        .await
                        .map_err(|_| {
                            Status::deadline_exceeded("Profile delivery completion unknown")
                        })??;
                    if !result.into_inner().completed {
                        return Err(Status::unavailable("Profile delivery completion unknown"));
                    }
                    Ok(())
                })
                .await
            })
            .await
            .map_err(|_| Status::unavailable("Profile delivery completion interrupted"))?
        })
    }
}
