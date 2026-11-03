//! Durable dispatch intent and bounded, cancellation-independent completion.
//! Completed responses expire after 24 hours. UNKNOWN operations are never
//! redispatched; explicitly opted-in owners can perform a read-only lookup.

mod policy;
use policy::RouteClass;

use super::server::idempotency_owner;
use super::{
    client_ip, ensure_mutation_allowed, header_to_string, matched_route, parse_bearer_token,
    user_agent, verify_bearer_token, GatewayError,
};
use crate::application::dto::journal_write_dto::JournalCompletionWriteDto as CompletionWrite;
pub use crate::application::dto::journal_write_dto::PurgeOutcome;
use crate::application::dto::journal_write_dto::*;
use crate::application::dto::operation_dto::ManualOperationResolutionRequest;
pub use crate::application::dto::operation_dto::ReconciliationOutcome;
use crate::application::dto::queue_repository_dto::*;
use crate::application::journal_runtime_ports::JournalRuntimePorts;
use crate::application::journal_write_error::JournalWriteError;
use crate::application::ports::JournalCompletionConnectionPort;
use crate::application::ports::QueuePayloadPort;
use crate::config::{AppState, ASYNC_OPERATION_ENQUEUE_TIMEOUT_MS};
use crate::domain::operation::{
    supports_restart_safe_async_execution, OperationOutcome, OperationOutcomeDecision,
};
use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::Instrument;
use uuid::Uuid;

/// In-flight budget for ordinary mutations.
const MUTATION_CAPACITY: u32 = 64;

/// Completion attempts made while the caller may still be waiting.
const FAST_COMPLETION_ATTEMPTS: u32 = 3;
const FAST_COMPLETION_RETRY_DELAY: Duration = Duration::from_millis(50);
/// Upper bound for one completion attempt, including pool acquisition.
const COMPLETION_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(2);
/// After the fast attempts fail, the Gateway still knows the outcome it must
/// record, so it keeps retrying in the background with backoff. The horizon
/// ends well before maintenance declares an operation stale, so a transient
/// lock or pool stall cannot turn a known result into UNKNOWN.
const COMPLETION_RETRY_HORIZON: Duration = Duration::from_secs(120);
const COMPLETION_RETRY_INITIAL_BACKOFF: Duration = Duration::from_millis(100);
const COMPLETION_RETRY_MAX_BACKOFF: Duration = Duration::from_secs(2);
/// Age after which maintenance marks an unfinished operation UNKNOWN.
const STALE_OPERATION_AGE_SECONDS: u64 = 300;
const _: () = assert!(COMPLETION_RETRY_HORIZON.as_secs() * 2 <= STALE_OPERATION_AGE_SECONDS);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionOutcome {
    NotRequired,
    Recorded,
    AlreadyResolved,
    /// Fast attempts failed transiently; a detached task keeps retrying.
    RetryingInBackground,
    /// Fast attempts failed permanently; reconciliation must resolve it.
    NeedsReconciliation,
}

impl CompletionOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Recorded => "recorded",
            Self::AlreadyResolved => "already_resolved",
            Self::RetryingInBackground => "retrying_in_background",
            Self::NeedsReconciliation => "needs_reconciliation",
        }
    }
}

/// A completion attempt that ran past [`COMPLETION_ATTEMPT_TIMEOUT`].
#[derive(Debug)]
struct CompletionAttemptTimedOut;

impl std::fmt::Display for CompletionAttemptTimedOut {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("completion attempt exceeded its time budget")
    }
}

impl std::error::Error for CompletionAttemptTimedOut {}

/// Whether a failed completion can succeed later without anyone intervening:
/// lock, statement and attempt timeouts, serialization or deadlock aborts,
/// connection loss and pool exhaustion. Constraint or data errors are not
/// retried; they stay for reconciliation.
fn is_transient_completion_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<CompletionAttemptTimedOut>().is_some()
        || error.downcast_ref::<JournalWriteError>() == Some(&JournalWriteError::Transient)
}

#[derive(Debug)]
struct OperationTiming {
    started: tokio::time::Instant,
    phase: &'static str,
    phase_started: tokio::time::Instant,
    intent_acquire_us: u128,
    intent_write_us: u128,
    handler_us: Option<u128>,
    completion_attempts: Vec<CompletionTiming>,
}

impl Default for OperationTiming {
    fn default() -> Self {
        let now = tokio::time::Instant::now();
        Self {
            started: now,
            phase: "intent_acquire",
            phase_started: now,
            intent_acquire_us: 0,
            intent_write_us: 0,
            handler_us: None,
            completion_attempts: Vec::new(),
        }
    }
}

fn timing_phase(timing: &Arc<Mutex<OperationTiming>>, phase: &'static str) {
    let mut timing = timing.lock().expect("operation timings");
    timing.phase = phase;
    timing.phase_started = tokio::time::Instant::now();
}

#[derive(Debug)]
struct CompletionTiming {
    acquire_us: u128,
    database_us: u128,
    committed: bool,
}

struct CompletionObservation {
    operation: Arc<Mutex<OperationTiming>>,
    started: tokio::time::Instant,
    acquired_at: Option<tokio::time::Instant>,
    committed: bool,
}

#[derive(Clone, Copy)]
struct CompletionContext<'a> {
    timing: &'a Arc<Mutex<OperationTiming>>,
}

impl Drop for CompletionObservation {
    fn drop(&mut self) {
        let now = tokio::time::Instant::now();
        let acquired = self.acquired_at.unwrap_or(now);
        self.operation
            .lock()
            .expect("operation timings")
            .completion_attempts
            .push(CompletionTiming {
                acquire_us: acquired.duration_since(self.started).as_micros(),
                database_us: now.duration_since(acquired).as_micros(),
                committed: self.committed,
            });
        timing_phase(&self.operation, "between_completion_attempts");
    }
}

fn log_operation_timing(
    id: Uuid,
    stage: &'static str,
    completion_outcome: &'static str,
    timing: &Arc<Mutex<OperationTiming>>,
) {
    let timing = timing.lock().expect("operation timings");
    let waits: u128 = timing
        .completion_attempts
        .iter()
        .map(|attempt| attempt.acquire_us)
        .sum();
    let writes: u128 = timing
        .completion_attempts
        .iter()
        .map(|attempt| attempt.database_us)
        .sum();
    let commits = timing
        .completion_attempts
        .iter()
        .filter(|attempt| attempt.committed)
        .count();
    macro_rules! event {
        ($level:expr) => { tracing::event!($level, operation_id = %id, stage, completion_outcome, total_elapsed_us = %timing.started.elapsed().as_micros(), phase = timing.phase, phase_elapsed_us = %timing.phase_started.elapsed().as_micros(), intent_acquire_us = %timing.intent_acquire_us,
        intent_write_us = %timing.intent_write_us, handler_us = ?timing.handler_us,
        completion_acquire_us = %waits, completion_database_us = %writes,
        completion_attempts = timing.completion_attempts.len(), completion_commits = commits,
        attempts = ?timing.completion_attempts, code = "JOURNAL_OPERATION_TIMING",
        "journal operation phase timing"); };
    }
    if stage == "completed"
        && matches!(
            completion_outcome,
            "recorded" | "not_required" | "already_resolved"
        )
    {
        event!(tracing::Level::INFO);
    } else {
        event!(tracing::Level::WARN);
    }
}

/// In-flight budget reserved for authentication.
///
/// Authentication shares this middleware but must not compete for the mutation
/// budget. A stalled write path can legitimately occupy every mutation permit,
/// and if login drew from the same pool a dependency outage in one write path
/// would stop users from signing in at all. The two budgets are separate so
/// exhausting one cannot deny the other.
const AUTH_CAPACITY: u32 = 32;
const BODY_LIMIT: usize = 2 * 1024 * 1024;
struct PendingQueueAcceptance {
    cipher: Arc<dyn QueuePayloadPort>,
    actor_id: String,
    request_fingerprint: String,
    request_uri: String,
    content_type: Option<String>,
    body: Vec<u8>,
}

fn capacity(class: RouteClass) -> Arc<Semaphore> {
    static AUTH_STATE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    static MUTATION_STATE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    match class {
        RouteClass::Authentication => AUTH_STATE
            .get_or_init(|| Arc::new(Semaphore::new(AUTH_CAPACITY as usize)))
            .clone(),
        RouteClass::Mutation => MUTATION_STATE
            .get_or_init(|| Arc::new(Semaphore::new(MUTATION_CAPACITY as usize)))
            .clone(),
    }
}

/// Wait for every admission permit to be returned, for both classes.
pub async fn drain() -> anyhow::Result<()> {
    let auth = capacity(RouteClass::Authentication)
        .acquire_many_owned(AUTH_CAPACITY)
        .await?;
    let mutations = capacity(RouteClass::Mutation)
        .acquire_many_owned(MUTATION_CAPACITY)
        .await?;
    drop(auth);
    drop(mutations);
    Ok(())
}

/// Classify a finished response only from evidence stronger than its status.
///
/// A successful response records a completed operation. A non-success response
/// is a proven rejection only when an exact owner-backed route has not crossed
/// its outbound mutation boundary. Once that boundary may have been crossed,
/// only receipt/manual evidence can resolve the outcome.
fn classify_outcome(
    status: StatusCode,
    owner_context: Option<&platform::mutation_context::MutationContext>,
) -> OperationOutcomeDecision {
    // 202 means accepted, not necessarily durably queued. The separately
    // verified queue-acceptance transaction writes PENDING/QUEUE_ACCEPTED.
    let definitely_not_dispatched = owner_context
        .is_some_and(|context| !context.lookup_only && !context.dispatch_may_have_occurred());
    OperationOutcomeDecision::from_response(
        status.is_success(),
        status == StatusCode::ACCEPTED,
        definitely_not_dispatched,
    )
}

/// Validate the Gateway's public idempotency-key syntax and byte bound.
///
/// This adapter-level function is kept directly callable by the isolated parser
/// fuzz target so the fuzz oracle exercises the same production validator used
/// by request admission.
pub fn valid_idempotency_key(key: &str) -> bool {
    policy::valid_key(key)
}

pub async fn dispatch(state: Arc<AppState>, request: Request<Body>, next: Next) -> Response {
    match dispatch_inner(state, request, next).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn dispatch_inner(
    state: Arc<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, GatewayError> {
    if platform::shutdown::global().is_requested() {
        return Err(GatewayError::ServiceUnavailable(anyhow::anyhow!(
            "Gateway draining"
        )));
    }
    let method = request.method().to_string();
    let owner = idempotency_owner(request.method(), matched_route(&request));
    let owner_route = owner.is_some();
    let path = request.uri().path().to_owned();
    // Admission happens against the budget for this path's class, so saturating
    // one class cannot deny the other. A login arriving while every mutation
    // permit is held still gets a permit.
    // C03 auth keeps the inherited reserved auth class and never journals bodies,
    // credentials or cookie rotation at its new browser ingress.
    let auth_path = if path.starts_with("/api/auth/") {
        path.strip_prefix("/api").unwrap_or(&path)
    } else {
        &path
    };
    let class = RouteClass::of(auth_path);
    let permit = capacity(class).try_acquire_owned().map_err(|_| {
        GatewayError::too_many_requests(match class {
            RouteClass::Authentication => "Authentication capacity exhausted",
            RouteClass::Mutation => "Mutation capacity exhausted",
        })
    })?;
    let metric_action = format!("{method} {}", matched_route(&request));
    let bearer_token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token)
        .map(str::to_owned);
    let principal = match bearer_token {
        Some(token) => verify_bearer_token(&state, &token).await.ok(),
        None => None,
    };
    let actor = principal.as_ref().map(|claims| claims.subject.clone());
    let key = request
        .headers()
        .get("idempotency-key")
        .map(|value| value.to_str().map(str::to_owned))
        .transpose()
        .map_err(|_| GatewayError::bad_request("Invalid Idempotency-Key"))?;
    if let Some(key) = &key {
        if !valid_idempotency_key(key)
            || actor.is_none()
            || class == RouteClass::Authentication
            || matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS")
        {
            return Err(GatewayError::bad_request("Idempotency-Key requires an authenticated non-auth mutation and 1-128 ASCII key characters"));
        }
        if path.starts_with("/admin")
            && !principal
                .as_ref()
                .is_some_and(|claims| claims.roles.iter().any(|role| role == "ROLE_ADMIN"))
        {
            return Err(GatewayError::forbidden("Admin role required"));
        }
        if let Some(actor) = &actor {
            ensure_mutation_allowed(&state, actor).await?;
        }
    }
    // Authentication does not enter the journal.
    //
    // The journal exists to make a retried command safe, and it does that by
    // holding an intent until it can state an outcome. Auth already has that
    // property through its own durable records: login attempts, lockouts, and
    // session creation are written by the auth service, with its own audit. A
    // journal row on top of that adds a second record of the same fact, and the
    // two disagree exactly when it matters: a failed login is an ordinary auth
    // outcome, but as a journal row it is an operated-on failure, and retrying a
    // login with an `Idempotency-Key` whose outcome was recorded as ambiguous
    // would be refused with "reconcile before retry" on a request that never
    // had a mutation to reconcile.
    //
    // The reserved admission budget above still applies, so exempting auth from
    // the journal does not exempt it from the reason that budget exists: a
    // stalled write path must not be able to deny sign-in.
    if class == RouteClass::Authentication {
        return Ok(next.run(request).await);
    }
    if path.len() > 2048 {
        return Err(GatewayError::bad_request("Path too long"));
    }
    let request_id = bounded_header(&request, "x-request-id", 128)
        .or_else(|| bounded_header(&request, "x-correlation-id", 128));
    let ip = client_ip(request.headers());
    let agent =
        user_agent(request.headers()).map(|value| value.chars().take(512).collect::<String>());
    let context = serde_json::json!([
        actor,
        method,
        request.uri().to_string(),
        header_to_string(request.headers(), header::CONTENT_TYPE.as_str())
    ])
    .to_string();
    let (request, fingerprint_body) = if key.is_some() {
        let (parts, body) = request.into_parts();
        let bytes = to_bytes(body, BODY_LIMIT)
            .await
            .map_err(|_| GatewayError::bad_request("Idempotent request body exceeds 2 MiB"))?;
        (
            Request::from_parts(parts, Body::from(bytes.clone())),
            bytes.to_vec(),
        )
    } else {
        (request, Vec::new())
    };
    let mut id = Uuid::new_v4();
    let action = format!("{method} {path}");
    let trace_privacy = action == "PUT /api/profile/privacy";
    let timing = Arc::new(Mutex::new(OperationTiming::default()));
    let intent_started = tokio::time::Instant::now();
    let mut intent_connection = state
        .journal_dispatch_repository
        .acquire_intent_connection()
        .await
        .map_err(|error| GatewayError::ServiceUnavailable(error.into()))?;
    timing.lock().expect("operation timings").intent_acquire_us =
        intent_started.elapsed().as_micros();
    let intent_write_started = tokio::time::Instant::now();
    timing_phase(&timing, "intent_write");
    let inserted = intent_connection
        .insert_intent(JournalIntentDto {
            id,
            actor: actor.as_deref(),
            action: &action,
            path: &path,
            request_id: request_id.as_deref(),
            ip: ip.as_deref(),
            agent: agent.as_deref(),
            key: key.as_deref(),
            context: &context,
            fingerprint_body: &fingerprint_body,
        })
        .await
        .map_err(|error| GatewayError::ServiceUnavailable(error.into()))?;
    timing.lock().expect("operation timings").intent_write_us =
        intent_write_started.elapsed().as_micros();
    drop(intent_connection);
    timing_phase(&timing, "dispatch_setup");
    let lookup_only = inserted.is_none() && owner_route;
    if lookup_only {
        let existing = owner_replay_id(
            &state.journal,
            actor.as_deref(),
            key.as_deref(),
            &context,
            &fingerprint_body,
        )
        .await?;
        id = existing.0;
        if existing.1 == "QUEUED" {
            return Ok(accepted_response(id, true));
        }
    } else if inserted.is_none() {
        // Synthetic component-test routes only. The production transport rejects
        // idempotency keys outside the explicit owner allowlist before dispatch.
        return replay(
            &state.journal,
            actor.as_deref(),
            key.as_deref(),
            &context,
            &fingerprint_body,
        )
        .await;
    }
    let has_key = key.is_some();
    let owner_context = if owner_route {
        actor
            .as_deref()
            .and_then(|actor| Uuid::parse_str(actor).ok())
            .map(|actor_id| {
                platform::mutation_context::MutationContext::new(
                    id,
                    actor_id,
                    key.clone(),
                    lookup_only,
                )
            })
    } else {
        None
    };
    let outcome_context = owner_context.clone();
    let queue_acceptance =
        if !lookup_only && has_key && supports_restart_safe_async_execution(&action, &path) {
            match (
                state.async_operation_cipher.clone(),
                actor.as_deref(),
                key.as_deref(),
            ) {
                (Some(cipher), Some(actor_id), Some(_)) if Uuid::parse_str(actor_id).is_ok() => {
                    let mut fingerprint = Sha256::new();
                    fingerprint.update(context.as_bytes());
                    fingerprint.update(&fingerprint_body);
                    Some(PendingQueueAcceptance {
                        cipher,
                        actor_id: actor_id.to_owned(),
                        request_fingerprint: format!("{:x}", fingerprint.finalize()),
                        request_uri: request.uri().to_string(),
                        content_type: header_to_string(
                            request.headers(),
                            header::CONTENT_TYPE.as_str(),
                        ),
                        body: fingerprint_body.clone(),
                    })
                }
                _ => None,
            }
        } else {
            None
        };
    let client_budget = state.journal_client_timeout;
    // Route presentation shares the caller's absolute wait budget, not merely
    // the longer outer HTTP timeout. Keep time spent before the handler too.
    let journal_deadline = std::time::Instant::now() + client_budget;
    let deadline = request
        .extensions()
        .get::<super::RequestDeadline>()
        .map_or(journal_deadline, |outer| outer.0.min(journal_deadline));
    let mut request = request;
    request
        .extensions_mut()
        .insert(super::RequestDeadline(deadline));
    let operation_state = state.clone();
    // Tokio tasks do not inherit task-local trace context or the current span.
    let operation_trace_context = platform::trace_context::current();
    let operation_span = tracing::Span::current();
    let operation_timing = timing.clone();
    let caller_timed_out = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let operation_caller_timed_out = caller_timed_out.clone();
    let operation_future = async move {
        // Caller cancellation does not abort this bounded task. A process crash
        // leaves the already-committed intent for explicit reconciliation.
        let permit = permit;
        let handler_started = tokio::time::Instant::now();
        timing_phase(&operation_timing, "handler");
        let mut response = match tokio::time::timeout(
            Duration::from_secs(20),
            platform::mutation_context::scope(owner_context, next.run(request)),
        )
        .await
        {
            Ok(response) => response,
            Err(_) => {
                GatewayError::gateway_timeout("Operation outcome unknown; do not blindly retry")
                    .into_response()
            }
        };
        operation_timing
            .lock()
            .expect("operation timings")
            .handler_us = Some(handler_started.elapsed().as_micros());
        timing_phase(&operation_timing, "completion_setup");
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut saved_body = None;
        if has_key
            && response.status().is_success()
            && response.status() != StatusCode::ACCEPTED
            && !response.headers().contains_key(header::SET_COOKIE)
        {
            let (parts, body) = response.into_parts();
            match tokio::time::timeout(Duration::from_secs(2), to_bytes(body, BODY_LIMIT)).await {
                Ok(Ok(bytes)) => {
                    saved_body = Some(bytes.to_vec());
                    response = Response::from_parts(parts, Body::from(bytes));
                }
                _ => {
                    response = GatewayError::ServiceUnavailable(anyhow::anyhow!(
                        "Operation response not replayable"
                    ))
                    .into_response();
                }
            }
        }
        let status = response.status();
        let classified = classify_outcome(status, outcome_context.as_ref());
        let owner_replay_committed =
            lookup_only && status.is_success() && status != StatusCode::ACCEPTED;
        let should_record = !lookup_only || owner_replay_committed;
        let completion_context = CompletionContext {
            timing: &operation_timing,
        };
        let completion_write = CompletionWriteRequest {
            lookup_only,
            id,
            owner: owner.as_ref().map(|value| value.as_str()),
            classified,
            status,
            content_type: content_type.as_deref(),
            body: saved_body.as_deref(),
        };
        let mut written = None;
        let mut retryable = false;
        if should_record {
            for attempt in 0..FAST_COMPLETION_ATTEMPTS {
                if attempt > 0 {
                    tokio::time::sleep(FAST_COMPLETION_RETRY_DELAY).await;
                }
                match record_completion(
                    &operation_state.journal,
                    completion_context,
                    completion_write,
                )
                .await
                {
                    Ok(write) => {
                        written = Some(write);
                        break;
                    }
                    Err(error) => retryable = is_transient_completion_error(&error),
                }
            }
        }
        let completion = match (should_record, written) {
            (false, _) => CompletionOutcome::NotRequired,
            (true, Some(CompletionWrite::Recorded)) => CompletionOutcome::Recorded,
            (true, Some(CompletionWrite::AlreadyResolved)) => CompletionOutcome::AlreadyResolved,
            (true, None) if retryable => CompletionOutcome::RetryingInBackground,
            (true, None) => CompletionOutcome::NeedsReconciliation,
        };
        match completion {
            CompletionOutcome::Recorded => operation_state
                .metrics
                .inc_admin_audit_event(&metric_action, classified.outcome.audit_result()),
            CompletionOutcome::RetryingInBackground => {
                // The caller gets the owner's actual response now. The outcome
                // it already knows is recorded by a detached task, so a
                // transient lock or pool stall cannot strand it as UNKNOWN.
                tracing::warn!(operation_id = %id, outcome = classified.outcome.as_str(), code = "AUDIT_COMPLETION_PENDING", "durable completion is retrying in the background");
                spawn_background_completion(
                    operation_state.clone(),
                    operation_timing.clone(),
                    metric_action.clone(),
                    OwnedCompletionWrite::from(completion_write),
                );
            }
            CompletionOutcome::NeedsReconciliation => {
                tracing::error!(operation_id = %id, outcome = classified.outcome.as_str(), code = "AUDIT_COMPLETION_PENDING", "durable intent requires reconciliation");
            }
            CompletionOutcome::NotRequired | CompletionOutcome::AlreadyResolved => {}
        }
        drop(permit);
        if owner_replay_committed {
            response
                .headers_mut()
                .insert("idempotency-replayed", HeaderValue::from_static("true"));
        }
        response.headers_mut().insert(
            "x-operation-id",
            HeaderValue::from_str(&id.to_string()).expect("UUID header"),
        );
        // A downstream deadline can finish before the caller's outer budget.
        // Its recorded UNKNOWN response needs the same non-storage policy as
        // the outer operation timeout: it is not a durable rejection/result.
        if classified.outcome == OperationOutcome::Unknown {
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
        if operation_caller_timed_out.load(std::sync::atomic::Ordering::Relaxed) {
            log_operation_timing(
                id,
                "background_finished",
                completion.as_str(),
                &operation_timing,
            );
        } else if trace_privacy
            || !matches!(
                completion,
                CompletionOutcome::Recorded | CompletionOutcome::NotRequired
            )
            || operation_timing
                .lock()
                .expect("operation timings")
                .started
                .elapsed()
                >= Duration::from_millis(100)
        {
            log_operation_timing(id, "completed", completion.as_str(), &operation_timing);
        }
        response
    };
    let mut operation = match operation_trace_context {
        Some(trace_context) => tokio::spawn(platform::trace_context::scope(
            trace_context,
            operation_future.instrument(operation_span),
        )),
        None => tokio::spawn(operation_future.instrument(operation_span)),
    };
    // The caller waits at most as long as the configured request budget, not for
    // the whole internal task. The budget the timeout middleware already applied
    // ends here rather than at the middle of a write, so a caller that gives up is
    // told which operation it gave up on.
    //
    // If enabled for a receipt-backed owner route, durable queue insertion is
    // attempted before the outer deadline. A 202 is returned only after the
    // journal/queue transaction commits; otherwise the caller gets the existing
    // operation-id 504 and can inspect it at the caller-scoped status endpoint.
    match tokio::time::timeout(client_budget, &mut operation).await {
        Ok(Ok(response)) => Ok(response),
        // The task panicked after committing the intent. The intent still exists,
        // so this is an internal error rather than a failed operation, and the row
        // remains for reconciliation.
        Ok(Err(error)) => Err(GatewayError::Internal(error.into())),
        Err(_) => {
            caller_timed_out.store(true, std::sync::atomic::Ordering::Relaxed);
            log_operation_timing(id, "caller_budget_expired", "in_progress", &timing);
            if let Some(queued) = queue_acceptance {
                let enqueue = async {
                    let origin_traceparent =
                        platform::trace_context::current().map(|context| context.traceparent());
                    let payload = queued.cipher.encrypt(QueuePayloadEncryptDto {
                        operation_id: id,
                        actor_id: &queued.actor_id,
                        request_fingerprint: &queued.request_fingerprint,
                        plaintext: &queued.body,
                    })?;
                    state
                        .async_operation_queue
                        .enqueue(QueueEnqueueCommandDto {
                            operation_id: id,
                            actor_id: &queued.actor_id,
                            request_fingerprint: &queued.request_fingerprint,
                            request_uri: &queued.request_uri,
                            content_type: queued.content_type.as_deref(),
                            payload,
                            origin_traceparent,
                        })
                        .await
                };
                match tokio::time::timeout(
                    Duration::from_millis(ASYNC_OPERATION_ENQUEUE_TIMEOUT_MS),
                    enqueue,
                )
                .await
                {
                    Ok(Ok(true)) => {
                        // The queue and journal transition committed atomically.
                        // Stop the best-effort in-process attempt; the owner
                        // receipt makes any in-flight cancellation/recovery safe.
                        operation.abort();
                        return Ok(accepted_response(id, false));
                    }
                    Ok(Ok(false)) => tracing::info!(
                        operation_id = %id,
                        code = "ASYNC_QUEUE_NOT_ACCEPTED",
                        "request completed or changed state before durable acceptance"
                    ),
                    Ok(Err(error)) => tracing::warn!(
                        operation_id = %id,
                        error = %error,
                        code = "ASYNC_QUEUE_ENQUEUE_FAILED",
                        "durable queue acceptance failed; returning the operation timeout"
                    ),
                    Err(_) => tracing::warn!(
                        operation_id = %id,
                        deadline_ms = ASYNC_OPERATION_ENQUEUE_TIMEOUT_MS,
                        code = "ASYNC_QUEUE_ENQUEUE_TIMEOUT",
                        "durable queue acceptance exceeded its deadline; returning the operation timeout"
                    ),
                }
            }
            let mut response = GatewayError::gateway_timeout(
                "Operation is still in progress; use the operation id to check its outcome before retrying",
            )
            .into_response();
            response.headers_mut().insert(
                "x-operation-id",
                HeaderValue::from_str(&id.to_string()).expect("UUID header"),
            );
            // A caller must not cache a timeout as the operation's outcome; the
            // operation is still running and its result is not yet known.
            response.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                HeaderValue::from_static("no-store"),
            );
            Ok(response)
        }
    }
}

async fn owner_replay_id(
    journal: &JournalRuntimePorts,
    actor: Option<&str>,
    key: Option<&str>,
    context: &str,
    body: &[u8],
) -> Result<(Uuid, String), GatewayError> {
    let row = journal
        .dispatch
        .owner_replay_identity(JournalReplayQueryDto {
            actor,
            key,
            context,
            body,
        })
        .await
        .map_err(|error| GatewayError::ServiceUnavailable(error.into()))?
        .ok_or_else(|| {
            GatewayError::conflict("Operation identity unavailable; reconcile before retry")
        })?;
    if !row.same_request {
        return Err(GatewayError::conflict(
            "Idempotency-Key was used for a different request",
        ));
    }
    if !row.unexpired {
        return Err(GatewayError::conflict(
            "Operation replay window expired; reconcile before retry",
        ));
    }
    Ok((row.id, row.state))
}

fn accepted_response(operation_id: Uuid, replayed: bool) -> Response {
    let mut response = (
        StatusCode::ACCEPTED,
        axum::Json(serde_json::json!({
            "operationId": operation_id,
            "status": "ACCEPTED"
        })),
    )
        .into_response();
    let headers = response.headers_mut();
    headers.insert(
        "x-operation-id",
        HeaderValue::from_str(&operation_id.to_string()).expect("UUID header"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("retry-after", HeaderValue::from_static("2"));
    headers.insert(
        header::LOCATION,
        HeaderValue::from_str(&format!("/api/operations/{operation_id}"))
            .expect("operation status location"),
    );
    if replayed {
        headers.insert("idempotency-replayed", HeaderValue::from_static("true"));
    }
    response
}

pub async fn finish_owner_replay(
    journal: &JournalRuntimePorts,
    id: Uuid,
    owner: &str,
    status: StatusCode,
    content_type: Option<&str>,
    body: Option<&[u8]>,
) -> anyhow::Result<()> {
    let mut connection = journal
        .dispatch
        .acquire_completion_connection(JournalCompletionCallerDto::OwnerReplay)
        .await?;
    finish_owner_replay_on(connection.as_mut(), id, owner, status, content_type, body).await?;
    Ok(())
}

async fn finish_owner_replay_on(
    connection: &mut dyn JournalCompletionConnectionPort,
    id: Uuid,
    owner: &str,
    status: StatusCode,
    content_type: Option<&str>,
    body: Option<&[u8]>,
) -> anyhow::Result<CompletionWrite> {
    Ok(connection
        .record_owner_replay(JournalOwnerReplayCommandDto {
            id,
            owner,
            status: status.as_u16(),
            content_type,
            body,
        })
        .await?)
}

async fn replay(
    journal: &JournalRuntimePorts,
    actor: Option<&str>,
    key: Option<&str>,
    context: &str,
    body: &[u8],
) -> Result<Response, GatewayError> {
    let row = journal
        .dispatch
        .response_replay(JournalReplayQueryDto {
            actor,
            key,
            context,
            body,
        })
        .await
        .map_err(|error| GatewayError::ServiceUnavailable(error.into()))?
        .ok_or_else(|| {
            GatewayError::conflict("Idempotency state changed; reconcile before retry")
        })?;
    let same: bool = row.identity.same_request;
    let unexpired: bool = row.identity.unexpired;
    if !same {
        return Err(GatewayError::conflict(
            "Idempotency-Key was used for a different request",
        ));
    }
    let state: String = row.identity.state;
    if !unexpired || state != "COMPLETED" {
        return Err(GatewayError::conflict(
            "Operation is pending, unknown, or expired; reconcile before retry",
        ));
    }
    let saved: Vec<u8> = row.response_body.ok_or_else(|| {
        GatewayError::conflict("Operation is complete but not replayable; reconcile before retry")
    })?;
    let status = row
        .response_status
        .and_then(|value| u16::try_from(value).ok())
        .and_then(|value| StatusCode::from_u16(value).ok())
        .ok_or_else(|| GatewayError::Internal(anyhow::anyhow!("Invalid journal status")))?;
    let mut response = (status, saved).into_response();
    if let Some(content_type) = row.response_content_type {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_str(&content_type)?);
    }
    let id: Uuid = row.identity.id;
    response
        .headers_mut()
        .insert("x-operation-id", HeaderValue::from_str(&id.to_string())?);
    response
        .headers_mut()
        .insert("idempotency-replayed", HeaderValue::from_static("true"));
    Ok(response)
}

/// Everything one completion attempt needs; cheap to copy into each retry.
#[derive(Clone, Copy)]
struct CompletionWriteRequest<'a> {
    lookup_only: bool,
    id: Uuid,
    owner: Option<&'a str>,
    classified: OperationOutcomeDecision,
    status: StatusCode,
    content_type: Option<&'a str>,
    body: Option<&'a [u8]>,
}

/// One bounded completion attempt on the request-path journal pool.
async fn record_completion(
    journal: &JournalRuntimePorts,
    context: CompletionContext<'_>,
    request: CompletionWriteRequest<'_>,
) -> anyhow::Result<CompletionWrite> {
    tokio::time::timeout(COMPLETION_ATTEMPT_TIMEOUT, async {
        if request.lookup_only {
            let owner = request
                .owner
                .ok_or_else(|| anyhow::anyhow!("lookup-only requests have a mutation owner"))?;
            let (mut observation, mut connection) =
                begin_completion(journal, context, JournalCompletionCallerDto::OwnerReplay).await?;
            let written = finish_owner_replay_on(
                connection.as_mut(),
                request.id,
                owner,
                request.status,
                request.content_type,
                request.body,
            )
            .await?;
            observation.committed = true;
            Ok(written)
        } else {
            finish(journal, context, request).await
        }
    })
    .await
    .map_err(|_| anyhow::Error::new(CompletionAttemptTimedOut))?
}

/// Owned copy of a completion request for the detached retry task.
struct OwnedCompletionWrite {
    lookup_only: bool,
    id: Uuid,
    owner: Option<String>,
    classified: OperationOutcomeDecision,
    status: StatusCode,
    content_type: Option<String>,
    body: Option<Vec<u8>>,
}

impl From<CompletionWriteRequest<'_>> for OwnedCompletionWrite {
    fn from(request: CompletionWriteRequest<'_>) -> Self {
        Self {
            lookup_only: request.lookup_only,
            id: request.id,
            owner: request.owner.map(str::to_owned),
            classified: request.classified,
            status: request.status,
            content_type: request.content_type.map(str::to_owned),
            body: request.body.map(<[u8]>::to_vec),
        }
    }
}

impl OwnedCompletionWrite {
    fn borrowed(&self) -> CompletionWriteRequest<'_> {
        CompletionWriteRequest {
            lookup_only: self.lookup_only,
            id: self.id,
            owner: self.owner.as_deref(),
            classified: self.classified,
            status: self.status,
            content_type: self.content_type.as_deref(),
            body: self.body.as_deref(),
        }
    }
}

/// Keep recording a known outcome after the fast attempts failed transiently.
///
/// The task is detached from the request so the caller is never held by it. It
/// stops on success, on a permanent error, or at [`COMPLETION_RETRY_HORIZON`],
/// which ends before stale-operation maintenance would mark the row UNKNOWN.
fn spawn_background_completion(
    state: Arc<AppState>,
    timing: Arc<Mutex<OperationTiming>>,
    metric_action: String,
    request: OwnedCompletionWrite,
) {
    let span = tracing::Span::current();
    tokio::spawn(
        async move {
            let context = CompletionContext {
                timing: &timing,
            };
            let write = request.borrowed();
            let deadline = tokio::time::Instant::now() + COMPLETION_RETRY_HORIZON;
            let mut backoff = COMPLETION_RETRY_INITIAL_BACKOFF;
            let mut written = None;
            while tokio::time::Instant::now() + backoff < deadline {
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(COMPLETION_RETRY_MAX_BACKOFF);
                match record_completion(&state.journal, context, write).await {
                    Ok(result) => {
                        written = Some(result);
                        break;
                    }
                    Err(error) if is_transient_completion_error(&error) => {}
                    Err(_) => break,
                }
            }
            let outcome = match written {
                Some(CompletionWrite::Recorded) => {
                    state.metrics.inc_admin_audit_event(
                        &metric_action,
                        write.classified.outcome.audit_result(),
                    );
                    "recorded"
                }
                Some(CompletionWrite::AlreadyResolved) => "already_resolved",
                None => {
                    tracing::error!(operation_id = %write.id, outcome = write.classified.outcome.as_str(), code = "AUDIT_COMPLETION_ABANDONED", "durable intent requires reconciliation");
                    "abandoned"
                }
            };
            log_operation_timing(write.id, "background_completion", outcome, &timing);
        }
        .instrument(span),
    );
}

/// Acquire a journal connection for a completion and start its timing record.
/// The record is pushed when the returned observation drops, so failed
/// attempts are counted as well.
async fn begin_completion(
    journal: &JournalRuntimePorts,
    context: CompletionContext<'_>,
    caller: JournalCompletionCallerDto,
) -> anyhow::Result<(
    CompletionObservation,
    Box<dyn JournalCompletionConnectionPort>,
)> {
    let mut observation = CompletionObservation {
        operation: context.timing.clone(),
        started: tokio::time::Instant::now(),
        acquired_at: None,
        committed: false,
    };
    timing_phase(context.timing, "completion_acquire");
    let connection = journal
        .observed_dispatch
        .acquire_completion_connection(caller)
        .await?;
    observation.acquired_at = Some(tokio::time::Instant::now());
    timing_phase(context.timing, "completion_database");
    Ok((observation, connection))
}

async fn finish(
    journal: &JournalRuntimePorts,
    context: CompletionContext<'_>,
    request: CompletionWriteRequest<'_>,
) -> anyhow::Result<CompletionWrite> {
    let (mut observation, mut connection) =
        begin_completion(journal, context, JournalCompletionCallerDto::Completion).await?;
    let written = connection
        .record_completion(JournalCompletionCommandDto {
            id: request.id,
            outcome: request.classified.outcome.into(),
            evidence: request.classified.evidence.into(),
            status: request.status.as_u16(),
            content_type: request.content_type,
            body: request.body,
        })
        .await?;
    observation.committed = true;
    Ok(written)
}

pub async fn maintenance(journal: JournalRuntimePorts) -> anyhow::Result<()> {
    maintenance_with_retention(journal, RetentionWindows::default(), None).await
}

/// Retention windows for the purge scheduled by [`maintenance`].
#[derive(Debug, Clone, Copy)]
pub struct RetentionWindows {
    pub telemetry_days: i64,
    pub audit_days: i64,
}

impl Default for RetentionWindows {
    fn default() -> Self {
        // Telemetry is high-volume and low-value once it has been aggregated, so
        // it retires first. Audit evidence outlives it, because an access review
        // reaches further back than an operational dashboard.
        Self {
            telemetry_days: 30,
            audit_days: 365,
        }
    }
}

pub async fn maintenance_with_retention(
    journal: JournalRuntimePorts,
    retention: RetentionWindows,
    metrics: Option<Arc<crate::config::GatewayMetrics>>,
) -> anyhow::Result<()> {
    let shutdown = platform::shutdown::global();
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    while shutdown.tick(&mut ticker).await {
        match tokio::time::timeout(Duration::from_secs(5), reconcile_stale(&journal)).await {
            Ok(Ok(outcome)) => {
                if outcome.total() > 0 {
                    tracing::info!(
                        abandoned_operations = outcome.abandoned_operations,
                        journal_bodies_pruned = outcome.journal_bodies_pruned,
                        report_receipt_bodies_pruned = outcome.report_receipt_bodies_pruned,
                        journal_rows_retired = outcome.journal_rows_retired,
                        completed_queue_rows_retired = outcome.completed_queue_rows_retired,
                        retirement_deferred = outcome.retirement_deferred,
                        "reconciled stale operations and pruned expired replay bodies"
                    );
                }
            }
            Ok(Err(error)) => tracing::error!(
                error = ?error,
                code = "JOURNAL_MAINTENANCE_FAILED",
                "request journal maintenance failed"
            ),
            Err(_) => tracing::error!(
                code = "JOURNAL_MAINTENANCE_TIMEOUT",
                "request journal maintenance exceeded its time budget"
            ),
        }
        match tokio::time::timeout(
            Duration::from_secs(5),
            purge_expired_records(&journal, retention.telemetry_days, retention.audit_days),
        )
        .await
        {
            Ok(Ok(outcome)) => {
                // Reported per pass rather than only when it succeeds. A purge
                // that stops making progress is otherwise indistinguishable from
                // one with nothing to do, and the backlog grows silently.
                if outcome.total() > 0 {
                    tracing::info!(
                        telemetry_records = outcome.telemetry_records,
                        audit_events = outcome.audit_events,
                        rate_limit_keys = outcome.rate_limit_keys,
                        "purged expired gateway records"
                    );
                }
            }
            Ok(Err(error)) => {
                if let Some(metrics) = metrics.as_ref() {
                    metrics.inc_retention_purge_failure();
                }
                tracing::error!(
                    error = ?error,
                    code = "RETENTION_PURGE_FAILED",
                    "retention purge failed"
                );
            }
            Err(_) => {
                if let Some(metrics) = metrics.as_ref() {
                    metrics.inc_retention_purge_failure();
                }
                tracing::error!(
                    code = "RETENTION_PURGE_TIMEOUT",
                    "retention purge exceeded its time budget"
                );
            }
        }
        // Reported on every pass alongside reconciliation. The count is what an
        // operator alerts on, and the age is what distinguishes a transient blip
        // from reconciliation that is not happening at all.
        match tokio::time::timeout(Duration::from_secs(5), reconciliation_backlog(&journal)).await {
            Ok(Ok(backlog)) => {
                if let Some(metrics) = metrics.as_ref() {
                    metrics.set_reconciliation_backlog(
                        backlog.outstanding,
                        backlog.oldest_age_seconds,
                    );
                }
                if backlog.outstanding > 0 {
                    tracing::warn!(
                        outstanding = backlog.outstanding,
                        oldest_age_seconds = backlog.oldest_age_seconds,
                        code = "RECONCILIATION_REQUIRED",
                        "operations are awaiting reconciliation"
                    );
                }
            }
            Ok(Err(error)) => tracing::error!(
                error = ?error,
                code = "RECONCILIATION_BACKLOG_FAILED",
                "failed to read the reconciliation backlog"
            ),
            Err(_) => tracing::error!(
                code = "RECONCILIATION_BACKLOG_TIMEOUT",
                "reading the reconciliation backlog exceeded its time budget"
            ),
        }
    }
    Ok(())
}

pub async fn reconcile_stale(
    journal: &JournalRuntimePorts,
) -> anyhow::Result<ReconciliationOutcome> {
    Ok(journal
        .maintenance
        .reconcile_stale(JournalMaintenanceCommandDto {
            stale_operation_age_seconds: STALE_OPERATION_AGE_SECONDS,
        })
        .await?)
}

fn bounded_header(request: &Request<Body>, name: &str, limit: usize) -> Option<String> {
    header_to_string(request.headers(), name).map(|value| value.chars().take(limit).collect())
}

/// Delete telemetry, audit, and expired rate-limit rows past their retention.
///
/// Two of the three tables have a retention rule that is not simply "older than
/// N days":
///
/// * `admin_audit_events` rows that are still unresolved are the ones an operator
///   is expected to act on. Removing them by age would delete the only evidence
///   that a reconciliation is outstanding, so those rows are excluded and the
///   purge reports only what it actually removed.
/// * `gateway_request_journal` rows already carry their own 24-hour window for
///   completed replays, and unresolved intents are retained deliberately, so the
///   journal is left to `reconcile_stale` rather than duplicated here.
///
/// Returns the number of rows removed per table so the caller can report progress.
/// A purge that silently removed nothing for weeks would look identical to one
/// that was working, which is why the counts are returned and exported.
pub async fn purge_expired_records(
    journal: &JournalRuntimePorts,
    telemetry_retention_days: i64,
    audit_retention_days: i64,
) -> anyhow::Result<PurgeOutcome> {
    Ok(journal
        .retention
        .purge(JournalPurgeCommandDto {
            telemetry_retention_days,
            audit_retention_days,
        })
        .await?)
}

pub use crate::application::dto::journal_repository_dto::ReconciliationBacklog;
pub use crate::application::service::operation_journal_service::ManualResolutionWriteOutcome;

/// Append a reviewer decision for an UNKNOWN Gateway operation.
///
/// This path never invokes an owner or replays the mutation. The lifecycle and
/// original response remain unchanged; the outcome projection is updated in
/// the same transaction as the immutable decision and audit event. A correction
/// must name the current resolution id and is stored as the next revision.
pub async fn record_manual_resolution(
    journal: &JournalRuntimePorts,
    operation_id: Uuid,
    reviewer_id: &str,
    request: &ManualOperationResolutionRequest,
    request_id: Option<&str>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> anyhow::Result<ManualResolutionWriteOutcome> {
    record_manual_resolution_with_service(
        &journal.operations,
        operation_id,
        reviewer_id,
        request,
        request_id,
        ip_address,
        user_agent,
    )
    .await
}

pub async fn record_manual_resolution_with_service(
    service: &crate::application::service::OperationJournalService,
    operation_id: Uuid,
    reviewer_id: &str,
    request: &ManualOperationResolutionRequest,
    request_id: Option<&str>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> anyhow::Result<ManualResolutionWriteOutcome> {
    service
        .record_manual_resolution(
            crate::application::dto::journal_repository_dto::ManualResolutionJournalCommandDto {
                operation_id,
                reviewer_id: reviewer_id.to_owned(),
                disposition: request.disposition,
                evidence_kind: request.evidence_kind,
                evidence_reference: request.evidence_reference.clone(),
                reason: request.reason.clone(),
                supersedes_resolution_id: request.supersedes_resolution_id,
                request_id: request_id.map(str::to_owned),
                ip_address: ip_address.map(str::to_owned),
                user_agent: user_agent.map(str::to_owned),
            },
        )
        .await
        .map_err(anyhow::Error::new)
}

/// Count operations whose outcome is still unresolved.
///
/// An UNKNOWN row is a request that may have taken effect without its reply
/// arriving, so it is deliberately not retried automatically: doing so could
/// repeat a write that already succeeded. What is missing is not automation but
/// visibility, because a row an operator never sees is one that is never
/// reconciled. Age is reported alongside the count because a small, stale backlog
/// is a more urgent signal than a large, fresh one -- a fresh backlog is usually a
/// dependency blip, while an aged one means reconciliation is not happening at all.
pub async fn reconciliation_backlog(
    journal: &JournalRuntimePorts,
) -> anyhow::Result<ReconciliationBacklog> {
    journal
        .operations
        .reconciliation_backlog()
        .await
        .map_err(anyhow::Error::new)
}

/// Look up one durable operation by id.
///
/// The journal already records the intent, the outcome, and the replay body, but
/// none of it was reachable by the identifier the caller is given. Without this, a
/// timed-out caller holds an operation id that names nothing it can act on, and its
/// only remaining paths are retrying or asking an operator.
///
/// Returns `None` when no such operation exists, which the route reports as a 404
/// rather than an empty result: an unknown id and a known-but-unresolved one are
/// different answers and call for different actions.
pub async fn operation_status(
    journal: &JournalRuntimePorts,
    operation_id: Uuid,
) -> anyhow::Result<Option<crate::domain::operation::OperationStatus>> {
    journal
        .operations
        .operation_status(operation_id)
        .await
        .map_err(anyhow::Error::new)
}

/// Look up an operation only when it belongs to the authenticated caller.
///
/// The actor predicate is part of the query, so a caller cannot distinguish an
/// operation owned by someone else from an id that was never issued. No
/// response body, request path, owner receipt, or operator evidence is exposed.
pub async fn caller_operation_status(
    journal: &JournalRuntimePorts,
    operation_id: Uuid,
    actor_id: &str,
) -> anyhow::Result<Option<crate::domain::operation::CallerOperationStatus>> {
    journal
        .operations
        .caller_operation_status(operation_id, actor_id)
        .await
        .map_err(anyhow::Error::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn idempotency_keys_have_strict_bounds() {
        assert!(valid_idempotency_key("operation:123-abc_DEF.4"));
        assert!(!valid_idempotency_key(""));
        assert!(!valid_idempotency_key("contains whitespace"));
        assert!(!valid_idempotency_key(&"a".repeat(129)));
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
            ..ProptestConfig::default()
        })]

        #[test]
        fn arbitrary_idempotency_keys_are_ascii_bounded_or_rejected(input in prop::collection::vec(any::<u8>(), 0..=512)) {
            let candidate = String::from_utf8_lossy(&input);
            let accepted = valid_idempotency_key(&candidate);
            if candidate.len() > 128 {
                prop_assert!(!accepted);
            }
            if accepted {
                prop_assert!(!candidate.is_empty());
                prop_assert!(candidate.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')));
            }
        }
    }

    #[test]
    fn response_outcomes_require_owner_dispatch_evidence() {
        let context = platform::mutation_context::MutationContext::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("key-1".into()),
            false,
        );
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::CONFLICT,
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::GATEWAY_TIMEOUT,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            let rejected = classify_outcome(status, Some(&context));
            assert_eq!(
                rejected.outcome,
                OperationOutcome::RejectedBeforeCommit,
                "{status} before dispatch"
            );
            assert_eq!(rejected.evidence.as_str(), "PRE_DISPATCH_REJECTION");
        }

        context.mark_dispatch_may_have_occurred();
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::CONFLICT,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::GATEWAY_TIMEOUT,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            let unresolved = classify_outcome(status, Some(&context));
            assert_eq!(unresolved.outcome, OperationOutcome::Unknown, "{status}");
            assert_eq!(unresolved.evidence.as_str(), "RESPONSE_STATUS_ONLY");
        }

        assert_eq!(
            classify_outcome(StatusCode::BAD_REQUEST, None).outcome,
            OperationOutcome::Unknown
        );
        assert_eq!(
            classify_outcome(StatusCode::ACCEPTED, Some(&context)).outcome,
            OperationOutcome::Unknown,
            "202 acceptance is not proof of durable completion"
        );
        for status in [StatusCode::OK, StatusCode::CREATED, StatusCode::NO_CONTENT] {
            assert_eq!(
                classify_outcome(status, Some(&context)).outcome,
                OperationOutcome::Committed,
                "{status}"
            );
        }
    }

    #[test]
    fn authentication_paths_are_classified_separately_from_mutations() {
        for path in [
            "/auth/login",
            "/auth/email/signup",
            "/auth/password/reset/request",
            "/auth/refresh",
            "/oauth/google/callback",
        ] {
            assert_eq!(
                RouteClass::of(path),
                RouteClass::Authentication,
                "{path} must draw from the authentication budget"
            );
        }
        for path in ["/api/posts", "/api/profile/user-1", "/api/friends/requests"] {
            assert_eq!(RouteClass::of(path), RouteClass::Mutation, "{path}");
        }
    }

    #[test]
    fn the_two_classes_do_not_share_a_budget() {
        // The regression this guards: one pool for everything, where a stalled
        // write path could hold every permit and a login would then be refused.
        // The budgets must be distinct objects, not two names for one semaphore.
        let auth = capacity(RouteClass::Authentication);
        let mutations = capacity(RouteClass::Mutation);
        assert!(
            !Arc::ptr_eq(&auth, &mutations),
            "authentication and mutations must not share one capacity pool"
        );
        // Exhaust the mutation budget and confirm authentication is unaffected.
        let held: Vec<_> = (0..MUTATION_CAPACITY)
            .map(|_| {
                mutations
                    .clone()
                    .try_acquire_owned()
                    .expect("mutation permit")
            })
            .collect();
        assert!(
            mutations.clone().try_acquire_owned().is_err(),
            "budget must be exhausted"
        );
        assert!(
            auth.clone().try_acquire_owned().is_ok(),
            "a login must still be admitted while every mutation permit is held"
        );
        drop(held);
    }
}
