//! Loopback gRPC adapter tests with a controlled Auth double; no database claims.
use api_gateway::adapters::out::auth_client::{
    proto, verification::GrpcVerificationClient, GrpcAuthClient,
};
use api_gateway::adapters::out::jwt::MAX_JWKS_DOCUMENT_BYTES;
use api_gateway::application::dto::issuer_dto::{SessionAuthorityRequestDto, SessionStateDto};
use api_gateway::application::dto::verification_dto::{
    ResendVerificationRequest, ResendVerificationResponse,
};
use api_gateway::application::ports::AuthClientPort;
use api_gateway::application::ports::SigningKeySourcePort;
use api_gateway::application::verification::{
    VerificationClientPort, VerificationError, VerificationGatewayService,
};
use api_gateway::domain::high_risk::SessionState;
use platform::grpc_auth::{RequireRegisteredCallerInterceptor, Workload};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tonic::codegen::tokio_stream::Stream;
use tonic::transport::Server;
use tonic::{Code, Request, Response, Status};

const TOKEN: &str = "verification-transport-test-token";
/// The one session the fake issuer reports as live.
const LIVE_SESSION: &str = "5b0f4f58-8a57-4d6b-9d4e-0d4f0e6f1a01";
const PRIVATE_ROLE_SESSION: &str = "de3ad4f7-6ef9-4026-90bf-c694c9dd9cd1";
const ENDED_WITH_ROLES_SESSION: &str = "0e550809-f138-41f8-a261-802d9d27bbf4";

struct Incoming(TcpListener);

impl Stream for Incoming {
    type Item = Result<TcpStream, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0
            .poll_accept(cx)
            .map(|result| Some(result.map(|(stream, _)| stream)))
    }
}

#[derive(Clone, Debug)]
struct Captured {
    email: String,
    deadline: Option<String>,
    authenticated: bool,
}

#[derive(Clone)]
struct FakeAuth {
    status: String,
    error: Option<Code>,
    delay: Duration,
    calls: Arc<Mutex<Vec<Captured>>>,
    signing_keys: String,
    authorization_uri: String,
}

#[tonic::async_trait]
impl proto::auth_service_server::AuthService for FakeAuth {
    async fn read_concert_admission(
        &self,
        _request: Request<proto::ReadConcertAdmissionRequest>,
    ) -> Result<Response<proto::ReadConcertAdmissionResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn resend_verification(
        &self,
        request: Request<proto::ResendVerificationRequest>,
    ) -> Result<Response<proto::ResendVerificationResponse>, Status> {
        let deadline = request
            .metadata()
            .get("grpc-timeout")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let authenticated = request
            .metadata()
            .get("x-service-token")
            .and_then(|value| value.to_str().ok())
            == Some(TOKEN);
        self.calls.lock().await.push(Captured {
            email: request.into_inner().email,
            deadline,
            authenticated,
        });
        tokio::time::sleep(self.delay).await;
        match self.error {
            Some(code) => Err(Status::new(
                code,
                "PRIVATE_DATABASE_DETAIL member@example.invalid",
            )),
            None => Ok(Response::new(proto::ResendVerificationResponse {
                status: self.status.clone(),
            })),
        }
    }

    async fn signup_email(
        &self,
        _request: Request<proto::SignupEmailRequest>,
    ) -> Result<Response<proto::SignupResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn verify_email(
        &self,
        _request: Request<proto::VerifyEmailRequest>,
    ) -> Result<Response<proto::SignupResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn check_email(
        &self,
        _request: Request<proto::CheckEmailRequest>,
    ) -> Result<Response<proto::EmailAvailabilityResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn login_email(
        &self,
        _request: Request<proto::LoginEmailRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn refresh_tokens(
        &self,
        _request: Request<proto::RefreshTokensRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn logout(
        &self,
        _request: Request<proto::LogoutRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn request_password_reset(
        &self,
        _request: Request<proto::RequestPasswordResetRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn confirm_password_reset(
        &self,
        _request: Request<proto::ConfirmPasswordResetRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn authorize_o_auth(
        &self,
        request: Request<proto::AuthorizeOAuthRequest>,
    ) -> Result<Response<proto::OAuthAuthorizeResponse>, Status> {
        let deadline = request
            .metadata()
            .get("grpc-timeout")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let authenticated = request
            .metadata()
            .get("x-service-token")
            .and_then(|v| v.to_str().ok())
            == Some(TOKEN);
        self.calls.lock().await.push(Captured {
            email: format!("oauth:{}", request.into_inner().provider),
            deadline,
            authenticated,
        });
        Ok(Response::new(proto::OAuthAuthorizeResponse {
            authorization_uri: self.authorization_uri.clone(),
        }))
    }

    async fn o_auth_callback(
        &self,
        _request: Request<proto::OAuthCallbackRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn link_o_auth_provider(
        &self,
        _request: Request<proto::LinkOAuthProviderRequest>,
    ) -> Result<Response<proto::LinkedProvidersResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn delete_user(
        &self,
        _request: Request<proto::DeleteUserRequest>,
    ) -> Result<Response<proto::DeleteUserResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn get_user_deletion(
        &self,
        _request: Request<proto::GetUserDeletionRequest>,
    ) -> Result<Response<proto::GetUserDeletionResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn get_user_deletion_by_saga_id(
        &self,
        request: Request<proto::GetUserDeletionBySagaIdRequest>,
    ) -> Result<Response<proto::GetUserDeletionBySagaIdResponse>, Status> {
        let deadline = request
            .metadata()
            .get("grpc-timeout")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let authenticated = request
            .metadata()
            .get("x-service-token")
            .and_then(|value| value.to_str().ok())
            == Some(TOKEN);
        let saga_id = request.into_inner().saga_id;
        self.calls.lock().await.push(Captured {
            email: saga_id.clone(),
            deadline,
            authenticated,
        });
        Ok(Response::new(proto::GetUserDeletionBySagaIdResponse {
            found: true,
            saga: Some(deletion_saga_proto(&saga_id)),
        }))
    }

    async fn list_dead_lettered_user_deletions(
        &self,
        request: Request<proto::ListDeadLetteredUserDeletionsRequest>,
    ) -> Result<Response<proto::ListDeadLetteredUserDeletionsResponse>, Status> {
        let deadline = request
            .metadata()
            .get("grpc-timeout")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let authenticated = request
            .metadata()
            .get("x-service-token")
            .and_then(|value| value.to_str().ok())
            == Some(TOKEN);
        let request = request.into_inner();
        self.calls.lock().await.push(Captured {
            email: format!(
                "{}|{}|{}",
                request.limit, request.after_updated_at, request.after_saga_id
            ),
            deadline,
            authenticated,
        });
        let updated_at = "2026-09-01T12:00:00+00:00";
        Ok(Response::new(
            proto::ListDeadLetteredUserDeletionsResponse {
                sagas: vec![deletion_saga_proto("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")],
                has_more: true,
                next_updated_at: updated_at.into(),
                next_saga_id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".into(),
            },
        ))
    }

    async fn retry_user_deletion(
        &self,
        _request: Request<proto::RetryUserDeletionRequest>,
    ) -> Result<Response<proto::RetryUserDeletionResponse>, Status> {
        Err(Status::unimplemented(
            "not part of verification transport test",
        ))
    }

    async fn get_signing_keys(
        &self,
        request: Request<proto::GetSigningKeysRequest>,
    ) -> Result<Response<proto::GetSigningKeysResponse>, Status> {
        let deadline = request
            .metadata()
            .get("grpc-timeout")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let authenticated = request
            .metadata()
            .get("x-service-token")
            .and_then(|value| value.to_str().ok())
            == Some(TOKEN);
        self.calls.lock().await.push(Captured {
            email: String::new(),
            deadline,
            authenticated,
        });
        Ok(Response::new(proto::GetSigningKeysResponse {
            jwks_json: self.signing_keys.clone(),
        }))
    }

    async fn check_session(
        &self,
        request: Request<proto::CheckSessionRequest>,
    ) -> Result<Response<proto::CheckSessionResponse>, Status> {
        let deadline = request
            .metadata()
            .get("grpc-timeout")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let authenticated = request
            .metadata()
            .get("x-service-token")
            .and_then(|value| value.to_str().ok())
            == Some(TOKEN);
        let request = request.into_inner();
        self.calls.lock().await.push(Captured {
            email: format!("{}/{}", request.user_id, request.session_id),
            deadline,
            authenticated,
        });
        let active =
            request.session_id == LIVE_SESSION || request.session_id == PRIVATE_ROLE_SESSION;
        Ok(Response::new(proto::CheckSessionResponse {
            active,
            roles: if request.session_id == PRIVATE_ROLE_SESSION {
                vec![
                    "private-role-사용자".into(),
                    "ROLE_ADMIN".into(),
                    "ROLE_ADMIN".into(),
                    " role_user ".into(),
                ]
            } else if request.session_id == ENDED_WITH_ROLES_SESSION {
                vec!["ROLE_ADMIN".into(), "private-ended-role".into()]
            } else if active {
                vec!["ROLE_USER".into(), "ROLE_ADMIN".into()]
            } else {
                Vec::new()
            },
        }))
    }
}

fn deletion_saga_proto(saga_id: &str) -> proto::UserDeletionSaga {
    proto::UserDeletionSaga {
        id: saga_id.into(),
        user_id: "11111111-1111-1111-1111-111111111111".into(),
        status: "DEAD_LETTERED".into(),
        retention_until: "2026-10-01T12:00:00+00:00".into(),
        created_at: "2026-08-01T12:00:00+00:00".into(),
        updated_at: "2026-09-01T12:00:00+00:00".into(),
        completed_at: String::new(),
        steps: vec![proto::UserDeletionStep {
            target: "PROFILE".into(),
            status: "DEAD_LETTERED".into(),
            attempt_count: 3,
            next_attempt_at: "2026-09-01T12:00:00+00:00".into(),
            last_error: "provider unavailable".into(),
            completed_at: String::new(),
        }],
    }
}

struct Harness {
    client: Option<GrpcVerificationClient>,
    auth: Option<GrpcAuthClient>,
    endpoint: String,
    calls: Arc<Mutex<Vec<Captured>>>,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
}

impl Harness {
    async fn start(
        status: &str,
        error: Option<Code>,
        delay: Duration,
        deadline: Duration,
        token: &str,
    ) -> anyhow::Result<Self> {
        let fake = FakeAuth {
            status: status.into(),
            error,
            delay,
            calls: Arc::new(Mutex::new(Vec::new())),
            signing_keys: String::new(),
            authorization_uri: String::new(),
        };
        Self::start_with(fake, deadline, token).await
    }

    /// An Auth double that answers signing-key requests with `document`.
    async fn serving_signing_keys(document: String) -> anyhow::Result<Self> {
        let fake = FakeAuth {
            status: "ACCEPTED".into(),
            error: None,
            delay: Duration::ZERO,
            calls: Arc::new(Mutex::new(Vec::new())),
            signing_keys: document,
            authorization_uri: String::new(),
        };
        Self::start_with(fake, Duration::from_secs(1), TOKEN).await
    }

    async fn start_with(fake: FakeAuth, deadline: Duration, token: &str) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let calls = fake.calls.clone();
        let service = proto::auth_service_server::AuthServiceServer::with_interceptor(
            fake,
            RequireRegisteredCallerInterceptor::new(
                platform::grpc_auth::CallerRegistry::new().register(Workload::Gateway, TOKEN)?,
            )?,
        );
        let (shutdown, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(Incoming(listener), async move {
                    let _ = stopped.await;
                })
                .await
        });
        let mut harness = Self {
            client: None,
            auth: None,
            endpoint,
            calls,
            shutdown: Some(shutdown),
            task: Some(task),
        };
        let auth = tokio::time::timeout(
            Duration::from_secs(5),
            GrpcAuthClient::connect(
                harness.endpoint.clone(),
                platform::grpc_auth::Workload::Gateway,
                token,
                None,
            ),
        )
        .await??;
        harness.client = Some(GrpcVerificationClient::new(auth.clone(), deadline)?);
        harness.auth = Some(auth);
        Ok(harness)
    }

    fn client(&self) -> &GrpcVerificationClient {
        self.client.as_ref().expect("connected verification client")
    }

    fn auth(&self) -> &GrpcAuthClient {
        self.auth.as_ref().expect("connected auth client")
    }

    async fn close(mut self) -> anyhow::Result<()> {
        drop(self.client.take());
        drop(self.auth.take());
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
                Ok(result) => {
                    result??;
                }
                Err(error) => {
                    task.abort();
                    let _ = task.await;
                    return Err(error.into());
                }
            }
        }
        Ok(())
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

fn request(email: &str) -> ResendVerificationRequest {
    ResendVerificationRequest {
        email: email.into(),
    }
}

#[tokio::test]
async fn signing_keys_arrive_over_the_authenticated_channel_with_a_deadline() -> anyhow::Result<()>
{
    // Given: an issuer publishing a key set.
    let document = r#"{"keys":[{"kty":"RSA","kid":"issuer-key","n":"AQAB","e":"AQAB"}]}"#;
    let harness = Harness::serving_signing_keys(document.into()).await?;

    // When: the Gateway fetches the issuer's signing keys.
    let fetched = harness.auth().signing_keys().await?.jwks_json;

    // Then: the document arrives unchanged, on a call that carried the service
    // credential and the edge deadline like every other internal call.
    assert_eq!(fetched, document);
    let calls = harness.calls.lock().await;
    assert_eq!(calls.len(), 1);
    assert!(calls[0].authenticated);
    assert!(calls[0]
        .deadline
        .as_ref()
        .is_some_and(|value| !value.is_empty()));
    drop(calls);
    harness.close().await
}

#[tokio::test]
async fn session_checks_ask_the_issuer_over_the_authenticated_channel() -> anyhow::Result<()> {
    // Given: an issuer that knows one live session.
    let harness = Harness::serving_signing_keys(String::new()).await?;

    // When: the Gateway asks about the live session and about an ended one.
    let live = SessionState::from(
        harness
            .auth()
            .session_state(&SessionAuthorityRequestDto {
                user_id: "user-1".into(),
                session_id: LIVE_SESSION.into(),
            })
            .await?,
    );
    let ended = SessionState::from(
        harness
            .auth()
            .session_state(&SessionAuthorityRequestDto {
                user_id: "user-1".into(),
                session_id: "0b7c1d8e-3f7a-4c55-8d0e-5a9c2e4b7f10".into(),
            })
            .await?,
    );

    // Then: the live session reports its current roles, the ended one reports
    // nothing, and both calls carried the service credential and a deadline.
    assert_eq!(
        live,
        SessionState::Active {
            roles: vec!["ROLE_USER".into(), "ROLE_ADMIN".into()]
        }
    );
    assert_eq!(ended, SessionState::Ended);
    let calls = harness.calls.lock().await;
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].email, format!("user-1/{LIVE_SESSION}"));
    assert!(calls.iter().all(|call| call.authenticated));
    assert!(calls.iter().all(|call| call
        .deadline
        .as_ref()
        .is_some_and(|value| !value.is_empty())));
    drop(calls);
    harness.close().await
}

#[tokio::test]
async fn an_oversized_signing_key_reply_is_refused_by_the_transport() -> anyhow::Result<()> {
    // Given: an issuer whose reply is far over the key-set bound.
    let harness = Harness::serving_signing_keys("x".repeat(MAX_JWKS_DOCUMENT_BYTES * 2)).await?;

    // When/Then: decoding stops at the bound, so the reply is refused before it
    // is buffered and parsed.
    let error = harness
        .auth()
        .signing_keys()
        .await
        .expect_err("an oversized reply must be refused");
    assert_eq!(
        error,
        api_gateway::application::client_error::GatewayClientError::Unexpected
    );
    assert!(std::error::Error::source(&error).is_none());
    harness.close().await?;

    // Control: a reply at the bound is accepted, so the refusal is the size.
    let at_bound = "x".repeat(MAX_JWKS_DOCUMENT_BYTES);
    let harness = Harness::serving_signing_keys(at_bound.clone()).await?;
    assert_eq!(harness.auth().signing_keys().await?.jwks_json, at_bound);
    harness.close().await
}

#[tokio::test]
async fn issuer_transfers_preserve_opaque_ids_roles_and_raw_keys_without_exposing_diagnostics(
) -> anyhow::Result<()> {
    let raw = " { \"keys\": [{\"kid\":\"private-key-사용자\",\"d\":\"private-exponent\"}] }\n";
    let harness = Harness::serving_signing_keys(raw.into()).await?;
    let request = SessionAuthorityRequestDto {
        user_id: " private-user-使用者 ".into(),
        session_id: PRIVATE_ROLE_SESSION.into(),
    };
    let active = harness.auth().session_state(&request).await?;
    assert_eq!(
        active,
        SessionStateDto::Active {
            roles: vec![
                "private-role-사용자".into(),
                "ROLE_ADMIN".into(),
                "ROLE_ADMIN".into(),
                " role_user ".into()
            ],
        }
    );
    let ended = harness
        .auth()
        .session_state(&SessionAuthorityRequestDto {
            user_id: "private-other-user".into(),
            session_id: ENDED_WITH_ROLES_SESSION.into(),
        })
        .await?;
    assert_eq!(ended, SessionStateDto::Ended);
    let document = harness.auth().signing_keys().await?;
    assert_eq!(document.jwks_json.as_bytes(), raw.as_bytes());
    let diagnostic = format!("{request:?} {active:?} {ended:?} {document:?}");
    for private in [
        "private-user",
        "private-role",
        "private-key",
        "private-exponent",
        PRIVATE_ROLE_SESSION,
    ] {
        assert!(
            !diagnostic.contains(private),
            "private transfer content in diagnostics"
        );
    }
    let calls = harness.calls.lock().await;
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[0].email,
        format!(" private-user-使用者 /{PRIVATE_ROLE_SESSION}")
    );
    assert_eq!(
        calls[1].email,
        format!("private-other-user/{ENDED_WITH_ROLES_SESSION}")
    );
    assert!(calls.iter().all(|call| call.authenticated));
    assert!(calls.iter().all(|call| call
        .deadline
        .as_ref()
        .is_some_and(|value| !value.is_empty())));
    drop(calls);
    harness.close().await
}

#[tokio::test]
async fn carries_service_auth_and_deadline_and_preserves_the_accepted_contract(
) -> anyhow::Result<()> {
    let harness = Harness::start(
        "ACCEPTED",
        None,
        Duration::ZERO,
        Duration::from_secs(1),
        TOKEN,
    )
    .await?;
    let service = VerificationGatewayService::new(Arc::new(harness.client().clone()));
    let response = service
        .resend_verification(request("  MEMBER@EXAMPLE.INVALID  "))
        .await?;
    assert_eq!(response, ResendVerificationResponse::accepted());
    assert_eq!(
        serde_json::to_value(response)?,
        serde_json::json!({"status": "ACCEPTED"})
    );
    let calls = harness.calls.lock().await;
    assert_eq!(calls.len(), 1);
    // The service trims and lowercases before transport, matching
    // `VerificationGatewayService::normalized_email` and the auth-service
    // `User::normalized_email` contract, so the RPC carries the normalized
    // address rather than the caller's original casing and padding.
    assert_eq!(calls[0].email, "member@example.invalid");
    assert!(calls[0].authenticated);
    assert!(calls[0]
        .deadline
        .as_ref()
        .is_some_and(|value| !value.is_empty()));
    drop(calls);
    drop(service);
    harness.close().await
}

#[tokio::test]
async fn deletion_admin_queries_use_authenticated_grpc_and_validate_page_cursor(
) -> anyhow::Result<()> {
    let harness = Harness::start(
        "ACCEPTED",
        None,
        Duration::ZERO,
        Duration::from_secs(1),
        TOKEN,
    )
    .await?;
    let after_updated_at =
        chrono::DateTime::parse_from_rfc3339("2026-08-01T00:00:00Z")?.with_timezone(&chrono::Utc);
    let after_saga_id = uuid::Uuid::parse_str("99999999-9999-9999-9999-999999999999")?;
    let page = harness
        .auth()
        .list_dead_lettered_user_deletions(7, Some(after_updated_at), Some(after_saga_id))
        .await?;
    assert_eq!(page.sagas.len(), 1);
    assert_eq!(page.sagas[0].status, "DEAD_LETTERED");
    let cursor = page.next_cursor.expect("has-more cursor");
    assert_eq!(cursor.saga_id, "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    assert_eq!(cursor.updated_at.to_rfc3339(), "2026-09-01T12:00:00+00:00");

    let detail = harness
        .auth()
        .get_user_deletion_by_saga_id(cursor.saga_id.clone())
        .await?
        .expect("saga detail");
    assert_eq!(detail.id, cursor.saga_id);
    assert_eq!(
        detail.steps[0].last_error.as_deref(),
        Some("provider unavailable")
    );

    let calls = harness.calls.lock().await;
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].email,
        "7|2026-08-01T00:00:00+00:00|99999999-9999-9999-9999-999999999999"
    );
    assert_eq!(calls[1].email, cursor.saga_id);
    assert!(calls.iter().all(|call| call.authenticated));
    assert!(calls.iter().all(|call| call
        .deadline
        .as_ref()
        .is_some_and(|value| !value.is_empty())));
    drop(calls);
    harness.close().await
}

#[tokio::test]
async fn downstream_failures_are_sanitized_and_not_retried() -> anyhow::Result<()> {
    for (code, expected) in [
        (Code::InvalidArgument, VerificationError::InvalidEmail),
        (Code::Unavailable, VerificationError::Unavailable),
        (Code::Unimplemented, VerificationError::Unavailable),
        (Code::ResourceExhausted, VerificationError::Unavailable),
        (Code::DeadlineExceeded, VerificationError::Timeout),
        (Code::Internal, VerificationError::Internal),
    ] {
        let harness = Harness::start(
            "",
            Some(code),
            Duration::ZERO,
            Duration::from_secs(1),
            TOKEN,
        )
        .await?;
        let error = harness
            .client()
            .resend_verification(request("member@example.invalid"))
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("PRIVATE_DATABASE_DETAIL"));
        assert!(!error.to_string().contains("member@example.invalid"));
        assert_eq!(harness.calls.lock().await.len(), 1);
        harness.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn non_accepted_success_payloads_are_protocol_failures() -> anyhow::Result<()> {
    for status in ["", "SENT", "DELIVERED", "ACCEPTED "] {
        let harness =
            Harness::start(status, None, Duration::ZERO, Duration::from_secs(1), TOKEN).await?;
        assert_eq!(
            harness
                .client()
                .resend_verification(request("member@example.invalid"))
                .await,
            Err(VerificationError::InvalidResponse)
        );
        assert_eq!(harness.calls.lock().await.len(), 1);
        harness.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn wrong_or_missing_service_token_never_reaches_the_handler() -> anyhow::Result<()> {
    let harness = Harness::start(
        "ACCEPTED",
        None,
        Duration::ZERO,
        Duration::from_secs(1),
        "wrong-token",
    )
    .await?;
    assert_eq!(
        harness
            .client()
            .resend_verification(request("member@example.invalid"))
            .await,
        Err(VerificationError::Internal)
    );
    let mut unauthenticated = tokio::time::timeout(
        Duration::from_secs(5),
        proto::auth_service_client::AuthServiceClient::connect(harness.endpoint.clone()),
    )
    .await??;
    let mut request = Request::new(proto::ResendVerificationRequest {
        email: "member@example.invalid".into(),
    });
    request.set_timeout(Duration::from_secs(1));
    let error = tokio::time::timeout(
        Duration::from_secs(2),
        unauthenticated.resend_verification(request),
    )
    .await?
    .unwrap_err();
    assert_eq!(error.code(), Code::Unauthenticated);
    assert_eq!(harness.calls.lock().await.len(), 0);
    drop(unauthenticated);
    harness.close().await
}

#[tokio::test]
async fn local_deadline_bounds_a_slow_call_without_a_retry() -> anyhow::Result<()> {
    let harness = Harness::start(
        "ACCEPTED",
        None,
        Duration::from_secs(2),
        Duration::from_millis(100),
        TOKEN,
    )
    .await?;
    let response = tokio::time::timeout(
        Duration::from_secs(3),
        harness
            .client()
            .resend_verification(request("member@example.invalid")),
    )
    .await?;
    // Tonic may return CANCELLED for a server-side deadline before the local
    // timer wins. Both are bounded failures; neither may become ACCEPTED.
    assert!(matches!(
        response,
        Err(VerificationError::Timeout | VerificationError::Unavailable)
    ));
    assert!(harness.calls.lock().await.len() <= 1);
    harness.close().await
}

#[tokio::test]
async fn empty_and_oversized_requests_are_rejected_before_rpc() -> anyhow::Result<()> {
    let harness = Harness::start(
        "ACCEPTED",
        None,
        Duration::ZERO,
        Duration::from_secs(1),
        TOKEN,
    )
    .await?;
    for email in [
        String::new(),
        " ".into(),
        format!("{}@example.invalid", "x".repeat(255)),
    ] {
        assert_eq!(
            harness.client().resend_verification(request(&email)).await,
            Err(VerificationError::InvalidEmail)
        );
    }
    assert_eq!(harness.calls.lock().await.len(), 0);
    harness.close().await
}

#[tokio::test]
async fn oauth_authorization_neutral_dto_preserves_valid_uri_auth_and_deadline(
) -> anyhow::Result<()> {
    let uri = "https://accounts.example.test/oauth?state=opaque-state&scope=openid%20email";
    let harness = Harness::start_with(
        FakeAuth {
            status: "ACCEPTED".into(),
            error: None,
            delay: Duration::ZERO,
            calls: Arc::new(Mutex::new(Vec::new())),
            signing_keys: String::new(),
            authorization_uri: uri.into(),
        },
        Duration::from_secs(1),
        TOKEN,
    )
    .await?;
    let authorization = harness
        .auth()
        .authorize(api_gateway::domain::IdentityProvider::Google.into())
        .await?;
    assert_eq!(
        authorization.authorization_uri,
        uri.parse::<http::Uri>()?.to_string()
    );
    assert!(!format!("{authorization:?}").contains("opaque-state"));
    let calls = harness.calls.lock().await;
    assert_eq!(calls.len(), 1);
    assert!(calls[0].authenticated);
    assert!(calls[0].deadline.as_ref().is_some_and(|v| !v.is_empty()));
    drop(calls);
    harness.close().await
}

#[tokio::test]
async fn malformed_oauth_authorization_uri_is_refused_at_rpc_adapter() -> anyhow::Result<()> {
    let harness = Harness::start_with(
        FakeAuth {
            status: "ACCEPTED".into(),
            error: None,
            delay: Duration::ZERO,
            calls: Arc::new(Mutex::new(Vec::new())),
            signing_keys: String::new(),
            authorization_uri: "https://bad host/oauth".into(),
        },
        Duration::from_secs(1),
        TOKEN,
    )
    .await?;
    let error = harness
        .auth()
        .authorize(api_gateway::domain::IdentityProvider::Google.into())
        .await
        .expect_err("invalid URI");
    assert_eq!(
        error,
        api_gateway::application::client_error::GatewayClientError::Unexpected
    );
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(error.to_string(), "Internal error");
    assert_eq!(harness.calls.lock().await.len(), 1);
    harness.close().await
}
