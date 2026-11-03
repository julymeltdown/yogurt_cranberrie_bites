//! One-hop interactive authority. Missing admission never becomes system access.
use crate::adapters::r#in::grpc::resource_authority::proto;
use platform::actor_delegation::{exchange::VerifiedParentAuthority, HEADER};
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use post_application::query_witness::PublishedWitnessStageDto;
use prost::Message;
use std::time::Duration;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;
use tonic::{Request, Status};

#[derive(Clone)]
pub struct GrpcAuthorizationExchangeClient {
    client: proto::authorization_exchange_service_client::AuthorizationExchangeServiceClient<
        InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>,
    >,
    budget: Duration,
    deadlines: platform::edge_metrics::EdgeDeadlineMetrics,
}
impl GrpcAuthorizationExchangeClient {
    /// Production injects the Gateway to Post budget values; see
    /// config::authority_exchange_budget. No separate SLO is defined here.
    pub fn connect(
        endpoint: &str,
        credential: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        budget: platform::edge::EdgeBudget,
    ) -> anyhow::Result<Self> {
        let budget = budget.validate()?;
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Gateway,
            budget,
        )?;
        Ok(Self {
            client:
                proto::authorization_exchange_service_client::AuthorizationExchangeServiceClient::with_interceptor(
                    GrpcClientTraceService::new(channel),
                    ServiceIdentityClientInterceptor::new(Workload::Post, credential)?,
                ),
            budget: budget.deadline,
            deadlines: platform::edge_metrics::EdgeDeadlineMetrics::default(),
        })
    }
    /// Count local or peer deadline expiry on the Post to Gateway edge.
    pub fn with_deadline_metrics(
        mut self,
        deadlines: platform::edge_metrics::EdgeDeadlineMetrics,
    ) -> Self {
        self.deadlines = deadlines;
        self
    }
    pub async fn authorize<T: Message>(
        &self,
        method: &str,
        audience: Workload,
        message: T,
        stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<Request<T>, Status> {
        let parent = VerifiedParentAuthority::current()
            .ok_or_else(|| Status::permission_denied("Admitted interactive authority required"))?;
        let now = chrono::Utc::now();
        if now.timestamp() >= parent.expires_at() {
            return Err(Status::permission_denied("Interactive authority expired"));
        }
        if stage.is_some_and(|stage| {
            stage.parent_digest() != &parent.parent_digest()
                || stage.actor_id() != parent.actor()
                || stage.expires_at().timestamp() != parent.expires_at()
                || stage.expires_at() <= now
        }) {
            return Err(Status::permission_denied("Query witness binding denied"));
        }
        let body = message.encode_to_vec();
        let length = u32::try_from(body.len())
            .map_err(|_| Status::resource_exhausted("Child request too large"))?;
        let framed = [vec![0], length.to_be_bytes().to_vec(), body].concat();
        let root = parent.request();
        let mut exchange = Request::new(proto::ExchangeActorRequest {
            parent: Some(proto::ParentAuthority {
                token: root.token.to_owned(),
                method: root.method.to_owned(),
                framed_body: root.body.to_vec(),
                operation_id: root.operation.map(|id| id.to_string()),
            }),
            audience: audience.as_str().to_owned(),
            method: method.to_owned(),
            framed_body: framed,
            operation_id: None,
            witness_id: stage.map(|stage| stage.stage_id().to_string()),
        });
        let remaining = parent.expires_at().saturating_sub(now.timestamp()) as u64;
        let budget = self.budget.min(Duration::from_secs(remaining));
        exchange.set_timeout(budget);
        let issued = platform::grpc_deadline::with_deadline(
            budget,
            platform::edge::Edge::PostGateway,
            self.deadlines.clone(),
            self.client.clone().exchange_actor(exchange),
        )
        .await
        .map_err(|status| match status.code() {
            tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => {
                Status::permission_denied("Authority exchange denied")
            }
            _ => Status::unavailable("Authority exchange unavailable"),
        })?
        .into_inner();
        if issued.token.len() > 4096
            || issued.token.is_empty()
            || issued.expires_at > parent.expires_at()
            || issued.expires_at <= chrono::Utc::now().timestamp()
        {
            return Err(Status::permission_denied("Issued authority binding denied"));
        }
        let mut request = Request::new(message);
        request.metadata_mut().insert(
            HEADER,
            issued
                .token
                .parse()
                .map_err(|_| Status::permission_denied("Issued authority malformed"))?,
        );
        Ok(request)
    }
}
