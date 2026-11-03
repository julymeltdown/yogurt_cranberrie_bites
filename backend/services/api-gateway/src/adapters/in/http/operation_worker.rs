//! Restart-safe execution of explicitly owner-deduplicated Gateway mutations.

use crate::application::dto::queue_repository_dto::ClaimedQueueOperationDto as ClaimedOperation;
use crate::application::dto::queue_repository_dto::*;
use crate::application::journal_runtime_ports::JournalRuntimePorts;
use crate::application::ports::QueuePayloadPort;
use crate::config::AppState;
use crate::domain::access_token::VerifiedAccessToken;
use anyhow::Context;
use axum::body::{to_bytes, Body};
use axum::http::header::{CONTENT_TYPE, SET_COOKIE, USER_AGENT};
use axum::http::{Method, Request, StatusCode, Uri};
use axum::Router;
use chrono::{Duration as ChronoDuration, Utc};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use tracing::Instrument;
use uuid::Uuid;

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const LEASE_DURATION: Duration = Duration::from_secs(60);
const HANDLER_DEADLINE: Duration = Duration::from_secs(20);
const MAX_ATTEMPTS: u32 = 8;
const MAX_REQUEST_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Run a bounded pool of queue consumers until graceful shutdown.
pub async fn run(
    journal: JournalRuntimePorts,
    cipher: Arc<dyn QueuePayloadPort>,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let router = worker_router(state);
    // The consumers run on the dedicated background pool, so idle polling cannot
    // take the journal connections that request completions depend on.
    tokio::try_join!(
        run_consumer(0, journal.clone(), cipher.clone(), router.clone()),
        run_consumer(1, journal, cipher, router),
    )?;
    Ok(())
}

/// Claim and execute at most one operation. Kept public for deterministic
/// acceptance tests that exercise the same worker path without a background loop.
pub async fn run_once(
    journal: &JournalRuntimePorts,
    cipher: &dyn QueuePayloadPort,
    state: Arc<AppState>,
) -> anyhow::Result<bool> {
    let Some(operation) = journal
        .queue
        .claim_next(QueueClaimQueryDto {
            worker_id: Uuid::new_v4(),
            lease_duration: LEASE_DURATION,
        })
        .await?
    else {
        return Ok(false);
    };
    process_claim(journal, cipher, &worker_router(state), operation).await?;
    Ok(true)
}

fn worker_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(super::post_routes::router(state.clone()))
        .merge(super::friend_routes::router(state.clone()))
        .merge(super::profile_routes::router(state.clone()))
        .merge(super::moderation_routes::router(state))
}

async fn run_consumer(
    index: u8,
    journal: JournalRuntimePorts,
    cipher: Arc<dyn QueuePayloadPort>,
    router: Router,
) -> anyhow::Result<()> {
    platform::shutdown::periodic("gateway-async-operation", POLL_INTERVAL, move || {
        let journal = journal.clone();
        let cipher = cipher.clone();
        let router = router.clone();
        async move {
            let lease_token = Uuid::new_v4();
            let claimed = {
                let mut connection = journal.observed_queue.acquire_claim_connection().await?;
                connection
                    .claim_next(QueueClaimQueryDto {
                        worker_id: lease_token,
                        lease_duration: LEASE_DURATION,
                    })
                    .await?
            };
            let Some(operation) = claimed else {
                return Ok(());
            };
            process_claim(&journal, cipher.as_ref(), &router, operation).await
        }
    })
    .await
    .map_err(|error| anyhow::anyhow!("Gateway async-operation consumer {index} stopped: {error}"))
}

async fn process_claim(
    journal: &JournalRuntimePorts,
    cipher: &dyn QueuePayloadPort,
    router: &Router,
    operation: ClaimedOperation,
) -> anyhow::Result<()> {
    let mut trace_context = operation
        .origin_traceparent
        .as_deref()
        .and_then(platform::trace_context::TraceContext::child_from_traceparent);
    let span = tracing::info_span!(
        parent: None,
        "gateway_async_operation",
        trace_id = tracing::field::Empty,
        span_id = tracing::field::Empty,
        parent_span_id = tracing::field::Empty,
        sampled = tracing::field::Empty,
    );
    if let Some(context) = trace_context.as_mut() {
        context.bind_to_span(&span);
        context.record_on_span(&span);
    }
    let processing = process_claim_inner(journal, cipher, router, operation).instrument(span);
    match trace_context {
        Some(context) => platform::trace_context::scope(context, processing).await,
        None => processing.await,
    }
}

async fn process_claim_inner(
    journal: &JournalRuntimePorts,
    cipher: &dyn QueuePayloadPort,
    router: &Router,
    operation: ClaimedOperation,
) -> anyhow::Result<()> {
    let body = match operation.decrypt_body(cipher) {
        Ok(body) => body,
        Err(_) => {
            journal
                .queue
                .dead_letter_claim(QueueDeadClaimCommandDto {
                    lease: operation.lease_identity(),
                    error_code: "PAYLOAD_DECRYPTION_FAILED",
                    response_status: 500,
                })
                .await?;
            return Ok(());
        }
    };
    let request = match build_internal_request(&operation, body) {
        Ok(request) => request,
        Err(_) => {
            journal
                .queue
                .dead_letter_claim(QueueDeadClaimCommandDto {
                    lease: operation.lease_identity(),
                    error_code: "COMMAND_NOT_ALLOWLISTED",
                    response_status: 500,
                })
                .await?;
            return Ok(());
        }
    };
    let actor_id = match Uuid::parse_str(&operation.actor_id) {
        Ok(actor_id) if !actor_id.is_nil() => actor_id,
        _ => {
            journal
                .queue
                .dead_letter_claim(QueueDeadClaimCommandDto {
                    lease: operation.lease_identity(),
                    error_code: "INVALID_ACTOR_ID",
                    response_status: 500,
                })
                .await?;
            return Ok(());
        }
    };
    let context = platform::mutation_context::MutationContext::new(
        operation.operation_id,
        actor_id,
        Some(operation.idempotency_key.clone()),
        false,
    );
    let response = match tokio::time::timeout(
        HANDLER_DEADLINE,
        platform::mutation_context::scope(Some(context), router.clone().oneshot(request)),
    )
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(never)) => match never {},
        Err(_) => {
            retry_or_dead_letter(journal, &operation, "WORKER_HANDLER_TIMEOUT", 504).await?;
            return Ok(());
        }
    };
    let status = response.status();
    if status.is_server_error() {
        retry_or_dead_letter(journal, &operation, "OWNER_OR_GATEWAY_5XX", status.as_u16()).await?;
        return Ok(());
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let response_body = if status.is_success()
        && status != StatusCode::ACCEPTED
        && !response.headers().contains_key(SET_COOKIE)
    {
        to_bytes(response.into_body(), MAX_REQUEST_BODY_BYTES)
            .await
            .ok()
            .map(|bytes| bytes.to_vec())
    } else {
        None
    };
    // A queued command may follow an ambiguous synchronous attempt, and later
    // worker attempts may follow earlier dispatches. Without a durable per-attempt
    // no-dispatch record, worker rejections stay conservative even if this attempt
    // itself did not reach the owner.
    let outcome = crate::domain::operation::OperationOutcomeDecision::from_response(
        status.is_success(),
        status == StatusCode::ACCEPTED,
        false,
    );
    journal
        .queue
        .complete_claim(QueueCompleteCommandDto {
            lease: operation.lease_identity(),
            response_status: status.as_u16(),
            outcome: outcome.outcome.into(),
            evidence: outcome.evidence.into(),
            content_type: content_type.as_deref(),
            response_body: response_body.as_deref(),
        })
        .await?;
    Ok(())
}

async fn retry_or_dead_letter(
    journal: &JournalRuntimePorts,
    operation: &ClaimedOperation,
    error_code: &str,
    response_status: u16,
) -> anyhow::Result<()> {
    if operation.attempt_count >= MAX_ATTEMPTS {
        journal
            .queue
            .dead_letter_claim(QueueDeadClaimCommandDto {
                lease: operation.lease_identity(),
                error_code,
                response_status,
            })
            .await?;
        return Ok(());
    }
    let backoff_seconds = 2_i64
        .saturating_pow(operation.attempt_count.saturating_sub(1).min(6))
        .min(60);
    journal
        .queue
        .retry_later(QueueRetryCommandDto {
            lease: operation.lease_identity(),
            available_at: Utc::now() + ChronoDuration::seconds(backoff_seconds),
            error_code,
        })
        .await?;
    Ok(())
}

fn build_internal_request(
    operation: &ClaimedOperation,
    body: Vec<u8>,
) -> anyhow::Result<Request<Body>> {
    anyhow::ensure!(
        body.len() <= MAX_REQUEST_BODY_BYTES,
        "request body exceeds queue limit"
    );
    anyhow::ensure!(
        crate::domain::operation::supports_restart_safe_async_execution(
            &operation.action,
            &operation.resource_path,
        ),
        "command route is not restart-safe"
    );
    let (method, action_path) = operation
        .action
        .split_once(' ')
        .context("journal action is malformed")?;
    anyhow::ensure!(
        action_path == operation.resource_path,
        "journal route mismatch"
    );
    let method = Method::from_bytes(method.as_bytes())?;
    let uri: Uri = operation.request_uri.parse()?;
    anyhow::ensure!(
        uri.scheme().is_none() && uri.authority().is_none(),
        "absolute request URI refused"
    );
    anyhow::ensure!(
        uri.path() == operation.resource_path,
        "request path mismatch"
    );

    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("idempotency-key", &operation.idempotency_key);
    if let Some(content_type) = operation.content_type.as_deref() {
        builder = builder.header(CONTENT_TYPE, content_type);
    }
    if let Some(request_id) = operation.request_id.as_deref() {
        builder = builder.header("x-request-id", request_id);
    }
    if let Some(ip_address) = operation.ip_address.as_deref() {
        builder = builder.header("x-forwarded-for", ip_address);
    }
    if let Some(user_agent) = operation.user_agent.as_deref() {
        builder = builder.header(USER_AGENT, user_agent);
    }
    let mut request = builder.body(Body::from(body))?;
    request.extensions_mut().insert(VerifiedAccessToken {
        subject: operation.actor_id.clone(),
        roles: Vec::new(),
        session_id: None,
    });
    Ok(request)
}
