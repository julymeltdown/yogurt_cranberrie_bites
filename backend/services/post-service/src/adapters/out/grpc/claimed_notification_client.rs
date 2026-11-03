//! Explicit Post presenter for owning notification issuance/page/write calls.
use crate::adapters::{
    out::{
        grpc::notification_client::proto as np,
        persistence::notification_authority_repository::notification_reference,
    },
    r#in::grpc::{notification_authority::page_message, resource_authority::proto},
};
use platform::{
    actor_delegation::notification::{self as capability, *},
    edge::EdgeBudget,
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_client_trace::GrpcClientTraceService,
    grpc_tls::TlsMaterial,
};
use post_application::{
    claimed_notification::*,
    delivery_authority::OwnedDeliveryClaimDto,
    delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure},
    notification_authority::NotificationScopeDto,
};
use prost::Message;
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request};
use uuid::Uuid;
type Transport =
    InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>;
#[derive(Clone)]
pub struct GrpcClaimedNotificationClient {
    issuer: proto::notification_grant_service_client::NotificationGrantServiceClient<Transport>,
    friend: proto::notification_projection_service_client::NotificationProjectionServiceClient<
        Transport,
    >,
    notification: np::notification_service_client::NotificationServiceClient<Transport>,
    verifier: NotificationVerifier,
    membership: NotificationMembershipVerifier,
    budgets: [Duration; 3],
}
impl GrpcClaimedNotificationClient {
    pub fn connect(
        gateway: &str,
        friend: &str,
        notification: &str,
        credential: &str,
        tls: &TlsMaterial,
        budgets: [EdgeBudget; 3],
        verifier: NotificationVerifier,
        membership: NotificationMembershipVerifier,
    ) -> anyhow::Result<Self> {
        let budgets = [
            budgets[0].validate()?,
            budgets[1].validate()?,
            budgets[2].validate()?,
        ];
        let transport = |url, peer, budget| -> anyhow::Result<Transport> {
            Ok(InterceptedService::new(
                GrpcClientTraceService::new(
                    platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
                        Some(tls),
                        url,
                        peer,
                        budget,
                    )?,
                ),
                ServiceIdentityClientInterceptor::new(Workload::Post, credential)?,
            ))
        };
        Ok(Self {
            issuer:proto::notification_grant_service_client::NotificationGrantServiceClient::new(transport(gateway,Workload::Gateway,budgets[0])?),
            friend:proto::notification_projection_service_client::NotificationProjectionServiceClient::new(transport(friend,Workload::Friend,budgets[1])?),
            notification:np::notification_service_client::NotificationServiceClient::new(transport(notification,Workload::Notification,budgets[2])?),
            verifier,membership,budgets:budgets.map(|b|b.deadline),
        })
    }
    fn deadline(&self, d: DeliveryEffectDeadlineDto) -> anyhow::Result<NotificationRpcDeadline> {
        // The overall bound is supplied by the original fence. Per-edge budgets
        // below only shorten it; prior local owner/pagination waits cannot reset it.
        let micros = d.expires_at().timestamp_micros().saturating_sub(now());
        anyhow::ensure!(micros > 0, "Notification effect expired");
        Ok(NotificationRpcDeadline::within_original(
            d.expires_at().timestamp_micros(),
            d.monotonic_stop(),
            Duration::from_micros(micros as u64),
        )?)
    }
    async fn issue(
        &self,
        binding: &NotificationBinding,
        deadline: NotificationRpcDeadline,
    ) -> anyhow::Result<Option<(String, i64)>> {
        let budget = deadline.remaining(self.budgets[0], binding.claim().lease_expires_micros)?;
        let mut request = Request::new(proto::IssueNotificationGrantRequest {
            binding: binding.encode().map_err(capability::status)?,
            framed_body: binding.frame().map_err(capability::status)?,
            deadline_micros: deadline.expires_micros(),
        });
        request.set_timeout(budget);
        let result = tokio::time::timeout(
            budget,
            self.issuer.clone().issue_notification_grant(request),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Notification issuance unavailable"))?
        .map_err(|_| anyhow::anyhow!("Notification issuance denied or unavailable"))?
        .into_inner()
        .result
        .ok_or_else(|| anyhow::anyhow!("Missing notification issuance outcome"))?;
        use proto::issue_notification_grant_response::Result as Wire;
        match result {
            Wire::Suppressed(s) => {
                anyhow::ensure!(
                    s.expires_micros <= deadline.expires_micros(),
                    "Extended notification suppression"
                );
                deadline.remaining(self.budgets[0], s.expires_micros)?;
                Ok(None)
            }
            Wire::Granted(g) => {
                let proof = self
                    .verifier
                    .verify(
                        &g.token,
                        Workload::Post,
                        binding.method(),
                        &binding.frame().map_err(capability::status)?,
                        now(),
                    )
                    .map_err(capability::status)?;
                anyhow::ensure!(
                    proof.binding() == binding
                        && g.expires_micros == proof.expires_micros()
                        && g.expires_micros <= deadline.expires_micros(),
                    "Notification grant mismatch"
                );
                deadline.remaining(self.budgets[0], g.expires_micros)?;
                Ok(Some((g.token, g.expires_micros)))
            }
        }
    }
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_micros()
}
fn claim(
    owner: &OwnedDeliveryClaimDto,
    scope: NotificationScopeDto,
) -> anyhow::Result<NotificationClaim> {
    let r = notification_reference(owner, scope)?;
    Ok(NotificationClaim {
        event_id: r.event_id,
        aggregate_id: r.aggregate_id,
        event_digest: NotificationClaim::encode_digest(&r.event_digest),
        attempt_digest: NotificationClaim::encode_digest(&r.attempt_digest),
        lease_digest: NotificationClaim::digest(r.lease_token.as_bytes()),
        attempt_id: r.attempt_id,
        attempt: r.attempt_number,
        lease_expires_micros: r.lease_expires_at.timestamp_micros(),
    })
}
#[async_trait::async_trait]
impl ClaimedNotificationTransportPort for GrpcClaimedNotificationClient {
    async fn select_page(
        &self,
        r: &NotificationPageRequestDto,
    ) -> anyhow::Result<NotificationPageResultDto> {
        async {
            let deadline = self.deadline(r.deadline())?;
            let page = NotificationPage {
                claim: claim(
                    r.owner(),
                    NotificationScopeDto::FollowerPage {
                        author_id: r.author(),
                        cursor: r.cursor().map(str::to_owned),
                        limit: r.limit(),
                    },
                )?,
                author_id: r.author(),
                cursor: r.cursor().map(str::to_owned),
                limit: r.limit(),
            };
            let binding = NotificationBinding::FollowerPage(page.clone());
            let Some((token, expiry)) = self.issue(&binding, deadline).await? else {
                return Ok(NotificationPageResultDto::Suppressed);
            };
            let budget = deadline.remaining(self.budgets[1], expiry)?;
            let mut request = Request::new(page_message(&page).map_err(capability::status)?);
            request
                .metadata_mut()
                .insert(capability::HEADER, token.parse()?);
            request.set_timeout(budget);
            let response = tokio::time::timeout(
                budget,
                self.friend.clone().list_notification_followers(request),
            )
            .await
            .map_err(|_| anyhow::anyhow!("Notification projection unavailable"))?
            .map_err(|_| anyhow::anyhow!("Notification projection denied or unavailable"))?
            .into_inner()
            .result
            .ok_or_else(|| anyhow::anyhow!("Missing notification page outcome"))?;
            use proto::list_notification_followers_response::Result as Wire;
            match response {
                Wire::Suppressed(s) => {
                    anyhow::ensure!(s.expires_micros <= expiry, "Extended page suppression");
                    deadline.remaining(self.budgets[1], s.expires_micros)?;
                    Ok(NotificationPageResultDto::Suppressed)
                }
                Wire::Page(p) => {
                    let author = Uuid::parse_str(&p.author_id)?;
                    let recipients = p
                        .follower_ids
                        .iter()
                        .map(|id| Uuid::parse_str(id))
                        .collect::<Result<Vec<_>, _>>()?;
                    let exp = self
                        .membership
                        .verify_page_expiry(
                            &page,
                            &recipients,
                            p.next_cursor.as_deref(),
                            &p.membership,
                            now(),
                        )
                        .map_err(capability::status)?;
                    anyhow::ensure!(
                        author == page.author_id && exp == p.expires_micros && exp <= expiry,
                        "Notification membership mismatch"
                    );
                    deadline.remaining(self.budgets[1], exp)?;
                    Ok(NotificationPageResultDto::Selected(
                        SelectedNotificationPageDto::new(
                            r,
                            author,
                            recipients,
                            p.next_cursor,
                            p.membership,
                            chrono::DateTime::from_timestamp_micros(exp)
                                .ok_or_else(|| anyhow::anyhow!("Invalid membership expiry"))?,
                        )?,
                    ))
                }
            }
        }
        .await
        .map_err(DeliveryEffectFailure::confirmed)
    }
    async fn write(
        &self,
        r: &ClaimedNotificationWriteDto,
    ) -> anyhow::Result<ClaimedNotificationAckDto> {
        let prepared = async {
            let deadline = self.deadline(r.deadline())?;
            let original = r.request();
            let post: serde_json::Value = serde_json::from_str(&original.payload_json)?;
            let post_id = post
                .get("postId")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("Missing notification post"))?
                .parse()?;
            let kind = match original.notification_type.as_str() {
                "POST_CREATED" => NotificationKind::Created,
                "POST_LIKED" => NotificationKind::Liked,
                "POST_COMMENTED" => NotificationKind::Commented,
                "COMMENT" => NotificationKind::LegacyComment,
                _ => anyhow::bail!("Unsupported notification intent"),
            };
            let d = NotificationDelivery {
                claim: claim(
                    r.owner(),
                    NotificationScopeDto::RecipientPolicy {
                        recipient_id: original.user_id,
                    },
                )?,
                recipient_id: original.user_id,
                actor_id: original
                    .actor_id
                    .ok_or_else(|| anyhow::anyhow!("Missing notification actor"))?,
                post_id,
                kind,
                occurred_at: original.occurred_at.clone(),
                membership: r.membership().map(str::to_owned),
            };
            let binding = NotificationBinding::Delivery(d);
            let Some((token, expiry)) = self.issue(&binding, deadline).await? else {
                return Ok(None);
            };
            let frame = binding.frame().map_err(capability::status)?;
            let message = np::CreateNotificationRequest::decode(&frame[5..])?;
            let budget = deadline.remaining(self.budgets[2], expiry)?;
            let mut request = Request::new(message);
            request
                .metadata_mut()
                .insert(capability::HEADER, token.parse()?);
            request.set_timeout(budget);
            Ok(Some((request, budget, deadline, expiry)))
        }
        .await
        .map_err(DeliveryEffectFailure::confirmed)?;
        let Some((request, budget, deadline, expiry)) = prepared else {
            return Ok(ClaimedNotificationAckDto::Suppressed);
        };
        // Once the original write RPC starts, cancellation, lost acknowledgement
        // and malformed success cannot prove absence of a committed effect.
        async {
            let response = tokio::time::timeout(
                budget,
                self.notification.clone().create_notification(request),
            )
            .await
            .map_err(|_| anyhow::anyhow!("Notification acknowledgement unavailable"))?
            .map_err(|_| anyhow::anyhow!("Notification write denied or unavailable"))?
            .into_inner();
            deadline.remaining(self.budgets[2], expiry)?;
            if response.suppressed {
                anyhow::ensure!(
                    response.notification.is_none(),
                    "Invalid suppression acknowledgement"
                );
                return Ok(ClaimedNotificationAckDto::Suppressed);
            }
            let n = response
                .notification
                .ok_or_else(|| anyhow::anyhow!("Missing notification acknowledgement"))?;
            let original = r.request();
            anyhow::ensure!(
                Uuid::parse_str(&n.id).is_ok_and(|id| !id.is_nil())
                    && n.user_id == original.user_id.to_string()
                    && n.event_id == original.event_id.to_string()
                    && n.r#type == original.notification_type
                    && n.actor_id
                        == original
                            .actor_id
                            .map(|id| id.to_string())
                            .unwrap_or_default()
                    && n.payload_json == original.payload_json
                    && chrono::DateTime::parse_from_rfc3339(&n.occurred_at)?
                        == chrono::DateTime::parse_from_rfc3339(&original.occurred_at)?,
                "Notification acknowledgement mismatch"
            );
            Ok(ClaimedNotificationAckDto::Delivered)
        }
        .await
        .map_err(DeliveryEffectFailure::uncertain)
    }
}
