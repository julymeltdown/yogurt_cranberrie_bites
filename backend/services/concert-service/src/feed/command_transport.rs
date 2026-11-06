//! Actual scoped, whole-envelope Concert->Post command transport. Reads inherit
//! the unavailable defaults; no World/legacy client is reused.
use super::owner::{proto, PostTransport};
use crate::adapters::out::relationship_authority::guarded::OwnerGuardScope;
use async_trait::async_trait;
use platform::{
    actor_delegation::{request_for, ActorDelegationClient},
    concert_feed_command::FeedCommandSigner,
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::{sync::Arc, time::Duration};
use tonic::{
    service::interceptor::InterceptedService, transport::Channel, Request, Response, Status,
};

type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
type Client =
    proto::post_concert_feed_command_service_client::PostConcertFeedCommandServiceClient<Transport>;
#[derive(Clone)]
pub struct OriginalIntent {
    pub method: String,
    pub target: String,
    pub body: Vec<u8>,
    pub content_type: String,
    pub key: String,
}
#[derive(Clone)]
pub struct QualifiedCommandTransport {
    client: Client,
    scope: Arc<OwnerGuardScope>,
    intent: OriginalIntent,
}
const EDGE: EdgeBudget = EdgeBudget {
    edge: "concert.feed.canonical_post_command",
    expected_latency: Duration::from_millis(100),
    deadline: Duration::from_secs(5),
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
impl QualifiedCommandTransport {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: &platform::grpc_tls::TlsMaterial,
        signer: FeedCommandSigner,
        scope: Arc<OwnerGuardScope>,
        intent: OriginalIntent,
    ) -> anyhow::Result<Self> {
        scope.binding().check()?;
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            Some(tls),
            endpoint,
            Workload::Post,
            EDGE.validate()?,
        )?;
        let client = proto::post_concert_feed_command_service_client::PostConcertFeedCommandServiceClient::with_interceptor(
            signer.client(channel),
            ServiceIdentityClientInterceptor::new(Workload::Concert, token)?,
        )
        .max_decoding_message_size(1048576)
        .max_encoding_message_size(131072);
        Ok(Self {
            client,
            scope,
            intent,
        })
    }
    fn authority(
        &self,
        context: Option<&proto::Context>,
    ) -> Result<proto::CommandAuthority, Status> {
        let c = context.ok_or_else(|| Status::invalid_argument("feed context required"))?;
        let b = self.scope.binding();
        b.check()?;
        let expiry = chrono::DateTime::parse_from_rfc3339(&c.expires_at)
            .map_err(|_| Status::invalid_argument("feed context invalid"))?
            .with_timezone(&chrono::Utc);
        b.assert_context(
            parse(&c.actor_id)?,
            parse(&c.event_id)?,
            parse(&c.session_family_id)?,
            parse(&c.request_id)?,
            c.original_authority_digest
                .as_slice()
                .try_into()
                .map_err(|_| Status::invalid_argument("feed context invalid"))?,
            expiry,
        )?;
        Ok(proto::CommandAuthority {
            owners: Some(proto::ReadAuthority {
                binding: Some(self.scope.wire_binding()),
                auth: Some(self.scope.auth_view()),
                friend: Some(self.scope.friend_view()),
                root: Some(self.scope.root_read_view()),
            }),
            original_body: self.intent.body.clone(),
            content_type: self.intent.content_type.clone(),
            idempotency_key: self.intent.key.clone(),
            original_http_method: self.intent.method.clone(),
            original_http_target: self.intent.target.clone(),
        })
    }
    fn request<T>(&self, message: T) -> Result<Request<T>, Status> {
        let b = self.scope.binding();
        b.check()?;
        let expiry = self
            .scope
            .auth_view()
            .expires_unix_micros
            .min(self.scope.friend_view().expires_unix_micros);
        let mut request = request_for(message, &b.actor().to_string())?;
        request.set_timeout(b.deadline().remaining(Duration::from_secs(5), expiry)?);
        Ok(request)
    }
    fn reply<T>(&self, r: Response<T>) -> Result<T, Status> {
        self.scope.binding().check()?;
        if r.metadata()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            != Some("no-store")
        {
            return Err(Status::unavailable("Post command unavailable"));
        }
        Ok(r.into_inner())
    }
}
fn parse(value: &str) -> Result<uuid::Uuid, Status> {
    uuid::Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("feed context invalid"))
}
#[async_trait]
impl PostTransport for QualifiedCommandTransport {
    async fn execute(&self, query: proto::ExecuteCommandRequest) -> Result<proto::Receipt, Status> {
        let authority = Some(self.authority(query.context.as_ref())?);
        let r = self
            .client
            .clone()
            .execute_command(self.request(proto::QualifiedExecuteCommandRequest {
                query: Some(query),
                authority,
            })?)
            .await?;
        self.reply(r)
    }
    async fn receipt(
        &self,
        query: proto::GetReceiptRequest,
    ) -> Result<proto::GetReceiptResponse, Status> {
        let authority = Some(self.authority(query.context.as_ref())?);
        let r = self
            .client
            .clone()
            .get_receipt(self.request(proto::QualifiedGetReceiptRequest {
                query: Some(query),
                authority,
            })?)
            .await?;
        self.reply(r)
    }
    async fn preview(
        &self,
        query: proto::ExecuteCommandRequest,
    ) -> Result<proto::PreviewCommandResponse, Status> {
        let authority = Some(self.authority(query.context.as_ref())?);
        let r = self
            .client
            .clone()
            .preview_command(self.request(proto::QualifiedPreviewCommandRequest {
                query: Some(query),
                authority,
            })?)
            .await?;
        self.reply(r)
    }
}
