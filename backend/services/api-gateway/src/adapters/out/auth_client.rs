use crate::adapters::out::grpc_call::{delegated, BoundedGrpcCallExt};
use crate::adapters::out::jwt::MAX_JWKS_DOCUMENT_BYTES;
use crate::application::dto::auth_dto::{
    AuthResponse, DeadLetteredUserDeletionListResponse, DeleteAccountResponse,
    EmailAvailabilityResponse, EmailCheckRequest, EmailSignupRequest, EmailVerifyRequest,
    LinkedProvidersResponse, OAuthLinkRequest, PasswordResetConfirmRequest, PasswordResetRequest,
    PasswordResetResponse, RetryUserDeletionRequestDto, SignupResponse,
    UserDeletionSagaCursorResponse, UserDeletionSagaResponse, UserDeletionStepResponse,
};
use crate::application::dto::issuer_dto::{
    SessionAuthorityRequestDto, SessionStateDto as SessionState, SigningKeysDto,
};
use crate::application::ports::{AuthClientPort, SigningKeySourcePort};
use crate::config::GatewayMetrics;
use crate::domain::IdentityProvider;
use async_trait::async_trait;
use platform::actor_delegation::{ActorDelegationClient, ActorDelegationSigner};
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use std::str::FromStr;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

pub mod verification;

pub mod proto {
    tonic::include_proto!("auth.v1");
}

/// Protobuf framing allowed around the key-set document in a signing-key reply:
/// one field tag and a length prefix, with room to spare.
const SIGNING_KEYS_FRAMING_BYTES: usize = 16;

#[derive(Clone)]
pub struct GrpcAuthClient {
    client: proto::auth_service_client::AuthServiceClient<
        InterceptedService<
            GrpcClientTraceService<ActorDelegationClient<Channel>>,
            ServiceIdentityClientInterceptor,
        >,
    >,
    metrics: GatewayMetrics,
}

impl GrpcAuthClient {
    /// Build the client without waiting for the peer to answer.
    ///
    /// The channel connects on first use, so a dependency that is down
    /// does not stop this service from starting. Configuration is still
    /// validated here: an invalid endpoint or TLS material fails now.
    pub async fn connect(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> anyhow::Result<Self> {
        Self::connect_with_metrics(
            endpoint,
            caller,
            service_token,
            tls_material,
            ActorDelegationSigner::local_parity(caller),
            GatewayMetrics::default(),
        )
        .await
    }

    pub async fn connect_with_metrics(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
        metrics: GatewayMetrics,
    ) -> anyhow::Result<Self> {
        // The peer is the auth service; caller is this workload's own identity.
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Auth,
            platform::edge::Edge::GatewayAuth.budget(),
        )?;
        Ok(Self {
            client: proto::auth_service_client::AuthServiceClient::with_interceptor(
                GrpcClientTraceService::new(ActorDelegationClient::new(
                    channel,
                    signer,
                    Workload::Auth,
                )),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            metrics,
        })
    }
}

impl GrpcAuthClient {
    async fn signing_keys_signing_key_source_port_inner(&self) -> anyhow::Result<SigningKeysDto> {
        // Decoding stops at the key-set bound, so an oversized reply is refused
        // by the transport rather than buffered in full and rejected afterwards.
        let mut client = self
            .client
            .clone()
            .max_decoding_message_size(MAX_JWKS_DOCUMENT_BYTES + SIGNING_KEYS_FRAMING_BYTES);
        // The channel enforces the edge deadline locally; sending it as well
        // lets the issuer stop work on a reply this Gateway no longer awaits.
        let mut request = tonic::Request::new(proto::GetSigningKeysRequest {});
        request.set_timeout(platform::edge::Edge::GatewayAuth.budget().deadline);
        let response = client
            .get_signing_keys(request)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(SigningKeysDto {
            jwks_json: response.jwks_json,
        })
    }
}

#[async_trait]
impl SigningKeySourcePort for GrpcAuthClient {
    async fn signing_keys(
        &self,
    ) -> crate::application::client_error::GatewayClientResult<SigningKeysDto> {
        self.signing_keys_signing_key_source_port_inner()
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
}

impl GrpcAuthClient {
    async fn signup_auth_client_port_inner(
        &self,
        request: EmailSignupRequest,
    ) -> anyhow::Result<SignupResponse> {
        let mut client = self.client.clone();
        let response = client
            .signup_email(proto::SignupEmailRequest {
                email: request.email,
                password: request.password,
            })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(SignupResponse {
            user_id: response.user_id.parse()?,
            status: response.status,
        })
    }

    async fn verify_auth_client_port_inner(
        &self,
        request: EmailVerifyRequest,
    ) -> anyhow::Result<SignupResponse> {
        let mut client = self.client.clone();
        let response = client
            .verify_email(proto::VerifyEmailRequest {
                email: request.email,
                code: request.code,
            })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(SignupResponse {
            user_id: response.user_id.parse()?,
            status: response.status,
        })
    }

    async fn check_email_auth_client_port_inner(
        &self,
        request: EmailCheckRequest,
    ) -> anyhow::Result<EmailAvailabilityResponse> {
        let mut client = self.client.clone();
        let response = client
            .check_email(proto::CheckEmailRequest {
                email: request.email,
            })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(EmailAvailabilityResponse {
            email: response.email,
            available: response.available,
            status: response.status,
        })
    }

    async fn request_password_reset_auth_client_port_inner(
        &self,
        request: PasswordResetRequest,
    ) -> anyhow::Result<PasswordResetResponse> {
        let mut client = self.client.clone();
        let response = client
            .request_password_reset(proto::RequestPasswordResetRequest {
                email: request.email,
            })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(PasswordResetResponse {
            status: response.status,
        })
    }

    async fn confirm_password_reset_auth_client_port_inner(
        &self,
        request: PasswordResetConfirmRequest,
    ) -> anyhow::Result<PasswordResetResponse> {
        let mut client = self.client.clone();
        let response = client
            .confirm_password_reset(proto::ConfirmPasswordResetRequest {
                token: request.token,
                new_password: request.new_password,
            })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(PasswordResetResponse {
            status: response.status,
        })
    }

    async fn login_auth_client_port_inner(
        &self,
        request: crate::application::dto::auth_dto::LoginRequest,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<AuthResponse> {
        let mut client = self.client.clone();
        let response = client
            .login_email(proto::LoginEmailRequest {
                email: request.email,
                password: request.password,
                ip_address: ip_address.unwrap_or_default(),
                user_agent: user_agent.unwrap_or_default(),
            })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        map_auth_response(response)
    }

    async fn refresh_auth_client_port_inner(
        &self,
        refresh_token: String,
    ) -> anyhow::Result<AuthResponse> {
        let mut client = self.client.clone();
        let response = client
            .refresh_tokens(proto::RefreshTokensRequest { refresh_token })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        map_auth_response(response)
    }

    async fn logout_auth_client_port_inner(&self, refresh_token: String) -> anyhow::Result<()> {
        let mut client = self.client.clone();
        client
            .logout(proto::LogoutRequest { refresh_token })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?;
        Ok(())
    }

    async fn authorize_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
    ) -> anyhow::Result<crate::application::dto::auth_dto::OAuthAuthorizationDto> {
        let mut client = self.client.clone();
        let mut request = tonic::Request::new(proto::AuthorizeOAuthRequest {
            provider: to_proto_provider(provider),
        });
        request.set_timeout(platform::edge::Edge::GatewayAuth.budget().deadline);
        let response = client
            .authorize_o_auth(request)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(crate::application::dto::auth_dto::OAuthAuthorizationDto {
            authorization_uri: http::Uri::from_str(&response.authorization_uri)?.to_string(),
        })
    }

    async fn oauth_callback_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
        code: String,
        state: String,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<AuthResponse> {
        let mut client = self.client.clone();
        let response = client
            .o_auth_callback(proto::OAuthCallbackRequest {
                provider: to_proto_provider(provider),
                code,
                state,
                ip_address: ip_address.unwrap_or_default(),
                user_agent: user_agent.unwrap_or_default(),
            })
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        map_auth_response(response)
    }

    async fn link_provider_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
        request: OAuthLinkRequest,
        user_id: String,
    ) -> anyhow::Result<LinkedProvidersResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let response = client
            .link_o_auth_provider(delegated(
                proto::LinkOAuthProviderRequest {
                    provider: to_proto_provider(provider),
                    user_id,
                    code: request.code,
                    state: request.state,
                },
                &actor,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(LinkedProvidersResponse {
            user_id: response.user_id.parse()?,
            providers: response
                .providers
                .into_iter()
                .filter_map(from_proto_provider)
                .collect(),
        })
    }

    async fn delete_account_auth_client_port_inner(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<DeleteAccountResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let response = client
            .delete_user(delegated(
                proto::DeleteUserRequest {
                    actor_id: user_id.clone(),
                    user_id,
                    request_id: request_id.unwrap_or_default(),
                    ip_address: ip_address.unwrap_or_default(),
                    user_agent: user_agent.unwrap_or_default(),
                },
                &actor,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        let saga = response
            .saga
            .ok_or_else(|| anyhow::anyhow!("Auth service omitted deletion saga"))?;
        Ok(DeleteAccountResponse {
            user_id: response.user_id,
            deleted: response.deleted,
            saga_id: response.saga_id,
            saga: map_deletion_saga(saga),
        })
    }

    async fn get_user_deletion_auth_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<Option<UserDeletionSagaResponse>> {
        let mut client = self.client.clone();
        let response = client
            .get_user_deletion(delegated(
                proto::GetUserDeletionRequest {
                    user_id: user_id.clone(),
                },
                &user_id,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        if !response.found {
            return Ok(None);
        }
        Ok(Some(map_deletion_saga(response.saga.ok_or_else(|| {
            anyhow::anyhow!("Auth service reported deletion saga without payload")
        })?)))
    }

    async fn get_user_deletion_by_saga_id_auth_client_port_inner(
        &self,
        saga_id: String,
    ) -> anyhow::Result<Option<UserDeletionSagaResponse>> {
        let mut client = self.client.clone();
        let mut request = tonic::Request::new(proto::GetUserDeletionBySagaIdRequest { saga_id });
        request.set_timeout(platform::edge::Edge::GatewayAuth.budget().deadline);
        let response = client
            .get_user_deletion_by_saga_id(request)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        if !response.found {
            return Ok(None);
        }
        Ok(Some(map_deletion_saga(response.saga.ok_or_else(|| {
            anyhow::anyhow!("Auth service reported deletion saga without payload")
        })?)))
    }

    async fn list_dead_lettered_user_deletions_auth_client_port_inner(
        &self,
        limit: i32,
        after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        after_saga_id: Option<uuid::Uuid>,
    ) -> anyhow::Result<DeadLetteredUserDeletionListResponse> {
        anyhow::ensure!(
            after_updated_at.is_some() == after_saga_id.is_some(),
            "deletion saga cursor fields must be paired"
        );
        let mut client = self.client.clone();
        let mut request = tonic::Request::new(proto::ListDeadLetteredUserDeletionsRequest {
            limit,
            after_updated_at: after_updated_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_default(),
            after_saga_id: after_saga_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
        });
        request.set_timeout(platform::edge::Edge::GatewayAuth.budget().deadline);
        let response = client
            .list_dead_lettered_user_deletions(request)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        let sagas = response
            .sagas
            .into_iter()
            .map(map_deletion_saga)
            .collect::<Vec<_>>();
        let cursor_is_present = !response.next_updated_at.is_empty();
        anyhow::ensure!(
            cursor_is_present != response.next_saga_id.is_empty()
                && cursor_is_present == response.has_more,
            "Auth service returned an inconsistent deletion saga page cursor"
        );
        let next_cursor = if cursor_is_present {
            let last = sagas.last().ok_or_else(|| {
                anyhow::anyhow!("Auth service returned a cursor for an empty deletion saga page")
            })?;
            let updated_at = chrono::DateTime::parse_from_rfc3339(&response.next_updated_at)?
                .with_timezone(&chrono::Utc);
            anyhow::ensure!(
                last.id == response.next_saga_id
                    && chrono::DateTime::parse_from_rfc3339(&last.updated_at)?
                        .with_timezone(&chrono::Utc)
                        == updated_at,
                "Auth service returned a deletion saga cursor that does not match the last item"
            );
            Some(UserDeletionSagaCursorResponse {
                updated_at,
                saga_id: response.next_saga_id,
            })
        } else {
            None
        };
        Ok(DeadLetteredUserDeletionListResponse { sagas, next_cursor })
    }

    async fn retry_user_deletion_auth_client_port_inner(
        &self,
        request: RetryUserDeletionRequestDto,
    ) -> anyhow::Result<UserDeletionSagaResponse> {
        let mut client = self.client.clone();
        let expected_saga_id = request.saga_id.clone();
        let expected_resolution_id = request.resolution_id.clone();
        let actor = request.actor_id.clone();
        let response = client
            .retry_user_deletion(delegated(
                proto::RetryUserDeletionRequest {
                    saga_id: request.saga_id,
                    actor_id: request.actor_id,
                    resolution_id: request.resolution_id,
                    reason: request.reason,
                    evidence_reference: request.evidence_reference,
                },
                &actor,
            )?)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        anyhow::ensure!(
            response.accepted && response.resolution_id == expected_resolution_id,
            "Auth service returned a mismatched deletion retry resolution"
        );
        let saga = map_deletion_saga(response.saga.ok_or_else(|| {
            anyhow::anyhow!("Auth service accepted deletion retry without a saga")
        })?);
        anyhow::ensure!(
            saga.id == expected_saga_id,
            "Auth service returned a mismatched deletion retry saga"
        );
        Ok(saga)
    }

    async fn session_state_auth_client_port_inner(
        &self,
        request: &SessionAuthorityRequestDto,
    ) -> anyhow::Result<SessionState> {
        // The caller is waiting on a high-risk request, so the check gets the
        // same edge deadline as other auth calls; a late answer is a refusal.
        let mut client = self.client.clone();
        let mut request = tonic::Request::new(proto::CheckSessionRequest {
            user_id: request.user_id.clone(),
            session_id: request.session_id.clone(),
        });
        request.set_timeout(platform::edge::Edge::GatewayAuth.budget().deadline);
        let response = client
            .check_session(request)
            .with_edge_deadline(platform::edge::Edge::GatewayAuth, self.metrics.clone())
            .await?
            .into_inner();
        Ok(if response.active {
            SessionState::Active {
                roles: response.roles,
            }
        } else {
            SessionState::Ended
        })
    }
}

#[async_trait]
impl AuthClientPort for GrpcAuthClient {
    async fn signup(
        &self,
        request: EmailSignupRequest,
    ) -> crate::application::client_error::GatewayClientResult<SignupResponse> {
        self.signup_auth_client_port_inner(request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn verify(
        &self,
        request: EmailVerifyRequest,
    ) -> crate::application::client_error::GatewayClientResult<SignupResponse> {
        self.verify_auth_client_port_inner(request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn check_email(
        &self,
        request: EmailCheckRequest,
    ) -> crate::application::client_error::GatewayClientResult<EmailAvailabilityResponse> {
        self.check_email_auth_client_port_inner(request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn request_password_reset(
        &self,
        request: PasswordResetRequest,
    ) -> crate::application::client_error::GatewayClientResult<PasswordResetResponse> {
        self.request_password_reset_auth_client_port_inner(request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn confirm_password_reset(
        &self,
        request: PasswordResetConfirmRequest,
    ) -> crate::application::client_error::GatewayClientResult<PasswordResetResponse> {
        self.confirm_password_reset_auth_client_port_inner(request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn login(
        &self,
        request: crate::application::dto::auth_dto::LoginRequest,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::application::client_error::GatewayClientResult<AuthResponse> {
        self.login_auth_client_port_inner(request, ip_address, user_agent)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn refresh(
        &self,
        refresh_token: String,
    ) -> crate::application::client_error::GatewayClientResult<AuthResponse> {
        self.refresh_auth_client_port_inner(refresh_token)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn logout(
        &self,
        refresh_token: String,
    ) -> crate::application::client_error::GatewayClientResult<()> {
        self.logout_auth_client_port_inner(refresh_token)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn authorize(
        &self,
        provider: crate::application::dto::facade_dto::IdentityProviderDto,
    ) -> crate::application::client_error::GatewayClientResult<
        crate::application::dto::auth_dto::OAuthAuthorizationDto,
    > {
        self.authorize_auth_client_port_inner(provider.into())
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn oauth_callback(
        &self,
        provider: crate::application::dto::facade_dto::IdentityProviderDto,
        code: String,
        state: String,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::application::client_error::GatewayClientResult<AuthResponse> {
        self.oauth_callback_auth_client_port_inner(
            provider.into(),
            code,
            state,
            ip_address,
            user_agent,
        )
        .await
        .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn link_provider(
        &self,
        provider: crate::application::dto::facade_dto::IdentityProviderDto,
        request: OAuthLinkRequest,
        user_id: String,
    ) -> crate::application::client_error::GatewayClientResult<LinkedProvidersResponse> {
        self.link_provider_auth_client_port_inner(provider.into(), request, user_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn delete_account(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::application::client_error::GatewayClientResult<DeleteAccountResponse> {
        self.delete_account_auth_client_port_inner(user_id, request_id, ip_address, user_agent)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_user_deletion(
        &self,
        user_id: String,
    ) -> crate::application::client_error::GatewayClientResult<Option<UserDeletionSagaResponse>>
    {
        self.get_user_deletion_auth_client_port_inner(user_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn get_user_deletion_by_saga_id(
        &self,
        saga_id: String,
    ) -> crate::application::client_error::GatewayClientResult<Option<UserDeletionSagaResponse>>
    {
        self.get_user_deletion_by_saga_id_auth_client_port_inner(saga_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn list_dead_lettered_user_deletions(
        &self,
        limit: i32,
        after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        after_saga_id: Option<uuid::Uuid>,
    ) -> crate::application::client_error::GatewayClientResult<DeadLetteredUserDeletionListResponse>
    {
        self.list_dead_lettered_user_deletions_auth_client_port_inner(
            limit,
            after_updated_at,
            after_saga_id,
        )
        .await
        .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn retry_user_deletion(
        &self,
        request: RetryUserDeletionRequestDto,
    ) -> crate::application::client_error::GatewayClientResult<UserDeletionSagaResponse> {
        self.retry_user_deletion_auth_client_port_inner(request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn session_state(
        &self,
        request: &SessionAuthorityRequestDto,
    ) -> crate::application::client_error::GatewayClientResult<SessionState> {
        self.session_state_auth_client_port_inner(request)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
}

fn map_deletion_saga(value: proto::UserDeletionSaga) -> UserDeletionSagaResponse {
    UserDeletionSagaResponse {
        id: value.id,
        user_id: value.user_id,
        status: value.status,
        retention_until: value.retention_until,
        created_at: value.created_at,
        updated_at: value.updated_at,
        completed_at: non_empty(value.completed_at),
        steps: value
            .steps
            .into_iter()
            .map(|step| UserDeletionStepResponse {
                target: step.target,
                status: step.status,
                attempt_count: step.attempt_count,
                next_attempt_at: step.next_attempt_at,
                last_error: non_empty(step.last_error),
                completed_at: non_empty(step.completed_at),
            })
            .collect(),
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn map_auth_response(response: proto::AuthResponse) -> anyhow::Result<AuthResponse> {
    Ok(AuthResponse {
        user_id: response.user_id.parse()?,
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        token_type: response.token_type,
        expires_in: response.expires_in_seconds,
        linked_providers: response
            .linked_providers
            .into_iter()
            .filter_map(from_proto_provider)
            .collect(),
    })
}

fn to_proto_provider(provider: IdentityProvider) -> i32 {
    match provider {
        IdentityProvider::Email => proto::IdentityProvider::Email as i32,
        IdentityProvider::Google => proto::IdentityProvider::Google as i32,
        IdentityProvider::Apple => proto::IdentityProvider::Apple as i32,
        IdentityProvider::X => proto::IdentityProvider::X as i32,
    }
}

fn from_proto_provider(provider: i32) -> Option<IdentityProvider> {
    match proto::IdentityProvider::try_from(provider).ok()? {
        proto::IdentityProvider::Email => Some(IdentityProvider::Email),
        proto::IdentityProvider::Google => Some(IdentityProvider::Google),
        proto::IdentityProvider::Apple => Some(IdentityProvider::Apple),
        proto::IdentityProvider::X => Some(IdentityProvider::X),
        _ => None,
    }
}
