//! Read-only C04 producer inside Root's existing canonical scope and delivery.
//! Construction grants no authority and opens no connection. Root owns settings,
//! scope acquisition, callback registration, recovery and actual HTTP completion.
use super::{
    cursor::CursorCodec,
    live_read::{CommentsQuery, FeedQuery},
    qualified_read::QualifiedReadFeed,
};
use crate::{
    adapters::out::{
        public_profile::PublicProfileReader,
        relationship_authority::guarded::{GuardBinding, OwnerGuardScope},
    },
    product_rpc::{
        proto,
        scoped::{ProductRepresentation, ScopedProductProducer},
        Operation, VerifiedProduct,
    },
};
use concert_application::{feed::Context, Clock};
use concert_domain::feed::FeedError;
use platform::{
    actor_delegation::concert_read::ConcertPostReadSigner,
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use prost::Message;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tonic::Status;

/// Explicit operator inputs only. No environment lookup, default credential,
/// generated cursor key, Debug or serialization of retained secrets.
pub struct FeedProductConfig {
    endpoint: String,
    credential: String,
    cursor_key: [u8; 32],
}
impl FeedProductConfig {
    pub fn new(endpoint: &str, credential: &str, cursor_key: &[u8; 32]) -> anyhow::Result<Self> {
        let uri: axum::http::Uri = endpoint
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid Feed Post endpoint"))?;
        anyhow::ensure!(
            endpoint.len() <= 4096
                && uri.scheme_str() == Some("https")
                && uri.host().is_some_and(|host| !host.is_empty())
                && uri.port_u16().is_some_and(|port| port > 0)
                && uri.authority().is_some_and(|a| !a.as_str().contains('@'))
                && uri.path_and_query().is_none_or(|p| p.as_str() == "/"),
            "Feed Post endpoint requires explicit HTTPS host/port without credentials, path or query"
        );
        anyhow::ensure!(
            credential == credential.trim(),
            "Invalid Feed Concert credential"
        );
        platform::config::require_caller_credential(
            true,
            Some(credential.to_owned()),
            "Feed Concert credential",
        )?;
        ServiceIdentityClientInterceptor::new(Workload::Concert, credential)?;
        anyhow::ensure!(
            *cursor_key != [0; 32],
            "Feed requires a dedicated nonzero cursor key"
        );
        CursorCodec::new(cursor_key).map_err(|_| anyhow::anyhow!("Invalid Feed cursor key"))?;
        Ok(Self {
            endpoint: endpoint.into(),
            credential: credential.into(),
            cursor_key: *cursor_key,
        })
    }
}

/// Pass to the existing ScopedProductOwner, or its closed feature dispatcher.
/// There is deliberately no ProductOwner implementation or scope/delivery factory.
pub struct FeedProductProducer<C> {
    config: FeedProductConfig,
    tls: TlsMaterial,
    signer: ConcertPostReadSigner,
    profiles: Arc<PublicProfileReader>,
    clock: Arc<C>,
}
impl<C: Clock> FeedProductProducer<C> {
    pub fn new(
        config: FeedProductConfig,
        tls: TlsMaterial,
        signer: ConcertPostReadSigner,
        profiles: Arc<PublicProfileReader>,
        clock: C,
    ) -> anyhow::Result<Self> {
        // Validate the fixed Post peer and supplied TLS material without starting
        // a channel, listener or owner transaction. Reachability is not attested.
        tls.endpoint(&config.endpoint, Workload::Post)?;
        Ok(Self {
            config,
            tls,
            signer,
            profiles,
            clock: Arc::new(clock),
        })
    }

    /// Root may use this closed set when composing Intro and Feed producers.
    pub fn handles(operation: Operation) -> bool {
        matches!(
            operation,
            Operation::Feed | Operation::Post(_) | Operation::Comments(_)
        )
    }
}

struct SharedClock<C>(Arc<C>);
impl<C: Clock> Clock for SharedClock<C> {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.0.now()
    }
}

#[async_trait::async_trait]
impl<C: Clock> ScopedProductProducer for FeedProductProducer<C> {
    async fn dispatch(
        &self,
        verified: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        verified.check()?;
        if !Self::handles(verified.operation()) {
            return Err(Status::permission_denied(
                "Unsupported Feed product operation",
            ));
        }
        let context = bound_context(verified, &scope, &request)?;
        let feed = QualifiedReadFeed::connect(
            &self.config.endpoint,
            &self.config.credential,
            &self.tls,
            self.signer.clone(),
            scope.clone(),
            self.profiles.clone(),
            CursorCodec::new(&self.config.cursor_key).map_err(read_error)?,
            SharedClock(self.clock.clone()),
        )
        .map_err(|_| Status::unavailable("Feed owner unavailable"))?;
        // Root's wrapper runs this entire future under scope.run. Do not acquire
        // a second scope or finish the supplied scope when this method returns.
        let value = match verified.operation() {
            Operation::Feed => {
                let q = verified.feed_query().ok_or_else(missing_query)?;
                feed.page(
                    context,
                    FeedQuery {
                        limit: q.limit,
                        cursor: q.cursor.clone(),
                        filter: q.filter,
                        before: q.before,
                    },
                )
                .await
            }
            Operation::Post(post) => feed.detail(context, post).await,
            Operation::Comments(post) => {
                let q = verified.comments_query().ok_or_else(missing_query)?;
                feed.comments(
                    context,
                    post,
                    CommentsQuery {
                        limit: q.limit,
                        cursor: q.cursor.clone(),
                    },
                )
                .await
            }
            _ => unreachable!("closed Feed operation checked before owner work"),
        }
        .map_err(read_error)?;
        verified.check()?;
        scope.binding().check()?;
        let body = serde_json::to_vec(&value)
            .map_err(|_| Status::unavailable("Feed representation unavailable"))?;
        Ok(ProductRepresentation {
            status: 200,
            content_type: "application/json".into(),
            body,
        })
    }
}

fn bound_context(
    verified: &VerifiedProduct,
    scope: &OwnerGuardScope,
    request: &proto::ProductRequest,
) -> Result<Context, Status> {
    let parent = verified.parent_authority();
    let original = parent
        .message()
        .map_err(|_| Status::permission_denied("Invalid Feed original parent"))?;
    if proto::ProductRequest::decode(original)
        .map_err(|_| Status::permission_denied("Invalid Feed original request"))?
        != *request
    {
        return Err(Status::permission_denied("Feed original request mismatch"));
    }
    let expected = GuardBinding::from_parent(
        parent,
        verified.actor(),
        verified.event(),
        verified.session(),
        verified.request(),
        verified.operation().contract_id(),
        &request.http_method,
        &request.http_target,
        Sha256::digest(&request.http_body).into(),
        verified.deadline(),
    )?;
    let actual = scope.binding();
    actual.check()?;
    if actual.digest() != expected.digest() {
        return Err(Status::permission_denied("Feed original scope mismatch"));
    }
    Ok(Context {
        actor: actual.actor(),
        event: actual.event(),
        session_family: actual.session_family(),
        request: actual.request_id(),
        authority_digest: actual.parent_digest(),
        expires_at: chrono::DateTime::from_timestamp_micros(actual.deadline().expires_micros())
            .ok_or_else(|| Status::permission_denied("Invalid Feed original deadline"))?,
    })
}

fn missing_query() -> Status {
    Status::unavailable("Verified Feed query unavailable")
}
fn read_error(error: FeedError) -> Status {
    match error {
        FeedError::InvalidInput => Status::invalid_argument("Invalid Feed query"),
        FeedError::ResourceUnavailable => Status::not_found("Feed resource unavailable"),
        FeedError::PermissionDenied => Status::permission_denied("Feed read denied"),
        // Mutation-only failures cannot become a successful read or fabricated
        // write response. Use Root's existing fixed dependency-error projection.
        _ => Status::unavailable("Feed owner unavailable"),
    }
}
