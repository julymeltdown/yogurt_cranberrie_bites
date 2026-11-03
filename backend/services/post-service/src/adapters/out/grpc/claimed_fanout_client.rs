//! Required claimed-page reader; no actor proof or general Friend fallback.
use crate::adapters::r#in::grpc::{
    fanout_authority::{frame, message, scope},
    resource_authority::proto,
};
use platform::actor_delegation::fanout::{self, FanoutVerifier};
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use post_application::fanout_authority::{
    ClaimedFanoutPageDto, ClaimedFanoutReadPort, FanoutPageLookupDto,
};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request};
use uuid::Uuid;
type Transport =
    InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>;
#[derive(Clone)]
pub struct GrpcClaimedFanoutClient {
    issuer: proto::fanout_grant_service_client::FanoutGrantServiceClient<Transport>,
    friend: proto::fanout_projection_service_client::FanoutProjectionServiceClient<Transport>,
    verifier: FanoutVerifier,
    issue_budget: Duration,
    projection_budget: Duration,
}
impl GrpcClaimedFanoutClient {
    /// All credentials, trust keys, budgets and selection are explicitly
    /// injected by composition. This constructor does not activate a worker.
    pub fn connect(
        gateway: &str,
        friend: &str,
        credential: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        budgets: [platform::edge::EdgeBudget; 2],
        verifier: FanoutVerifier,
    ) -> anyhow::Result<Self> {
        let issue = budgets[0].validate()?;
        let projection = budgets[1].validate()?;
        let transport = |endpoint, audience, budget| -> anyhow::Result<Transport> {
            Ok(InterceptedService::new(
                GrpcClientTraceService::new(
                    platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
                        tls, endpoint, audience, budget,
                    )?,
                ),
                ServiceIdentityClientInterceptor::new(Workload::Post, credential)?,
            ))
        };
        Ok(Self {
            issuer: proto::fanout_grant_service_client::FanoutGrantServiceClient::new(transport(
                gateway,
                Workload::Gateway,
                issue,
            )?),
            friend: proto::fanout_projection_service_client::FanoutProjectionServiceClient::new(
                transport(friend, Workload::Friend, projection)?,
            ),
            verifier,
            issue_budget: issue.deadline,
            projection_budget: projection.deadline,
        })
    }
}
#[async_trait::async_trait]
impl ClaimedFanoutReadPort for GrpcClaimedFanoutClient {
    async fn fetch_claimed_page(
        &self,
        lookup: &FanoutPageLookupDto,
    ) -> anyhow::Result<ClaimedFanoutPageDto> {
        anyhow::ensure!(
            lookup.claim().job.post_created_at.timestamp_subsec_nanos() % 1000 == 0
                && lookup.claim().lease_expires_at.timestamp_subsec_nanos() % 1000 == 0,
            "Invalid fanout timestamp precision"
        );
        let message = message(lookup);
        let body = frame(&message);
        let original = lookup.deadline();
        let micros = original
            .expires_at()
            .timestamp_micros()
            .saturating_sub(chrono::Utc::now().timestamp_micros());
        anyhow::ensure!(micros > 0, "Authority expired");
        let deadline =
            platform::actor_delegation::authority_deadline::AuthorityRpcDeadline::within_original(
                original.expires_at().timestamp_micros(),
                original.monotonic_stop(),
                Duration::from_micros(micros as u64),
            )?;
        let budget = deadline.remaining(self.issue_budget, deadline.expires_micros())?;
        let mut request = Request::new(proto::IssueFanoutGrantRequest {
            framed_body: body.clone(),
            deadline_micros: deadline.expires_micros(),
        });
        request.set_timeout(budget);
        let issued = tokio::time::timeout(budget, self.issuer.clone().issue_fanout_grant(request))
            .await
            .map_err(|_| anyhow::anyhow!("Fanout issuance unavailable"))?
            .map_err(|_| anyhow::anyhow!("Fanout issuance denied or unavailable"))?
            .into_inner();
        let proof = self
            .verifier
            .verify(
                &issued.token,
                Workload::Post,
                fanout::METHOD,
                &body,
                chrono::Utc::now().timestamp_micros(),
            )
            .map_err(|_| anyhow::anyhow!("Invalid issued fanout grant"))?;
        anyhow::ensure!(
            proof.scope() == &scope(lookup)? && proof.expires_micros() == issued.expires_micros,
            "Issued fanout scope mismatch"
        );
        anyhow::ensure!(
            issued.expires_micros <= deadline.expires_micros(),
            "Extended authority deadline"
        );
        let budget = deadline.remaining(self.projection_budget, issued.expires_micros)?;
        let mut request = Request::new(message);
        request.set_timeout(budget);
        request.metadata_mut().insert(
            fanout::HEADER,
            issued
                .token
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid issued fanout grant"))?,
        );
        let page = tokio::time::timeout(budget, self.friend.clone().list_fanout_followers(request))
            .await
            .map_err(|_| anyhow::anyhow!("Fanout projection unavailable"))?
            .map_err(|_| anyhow::anyhow!("Fanout projection denied or unavailable"))?
            .into_inner();
        anyhow::ensure!(
            page.expires_micros <= issued.expires_micros,
            "Extended projection authority"
        );
        deadline.remaining(self.projection_budget, page.expires_micros)?;
        let expiry = chrono::DateTime::from_timestamp_micros(page.expires_micros)
            .ok_or_else(|| anyhow::anyhow!("Invalid projection authority"))?;
        let invalid = || anyhow::anyhow!("Invalid fanout projection");
        let author = Uuid::parse_str(&page.author_id).map_err(|_| invalid())?;
        let followers = page
            .follower_ids
            .iter()
            .map(|id| Uuid::parse_str(id).map_err(|_| invalid()))
            .collect::<anyhow::Result<_>>()?;
        ClaimedFanoutPageDto::new(lookup, author, followers, page.next_cursor, expiry)
    }
}
