#[path = "support/mod.rs"]
mod support;

use api_gateway::application::dto::operation_dto::ManualOperationResolutionRequest;
use api_gateway::domain::operation::{ManualResolutionDisposition, ManualResolutionEvidenceKind};
use axum::{body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn journal_ports(
    pool: &sqlx::PgPool,
) -> api_gateway::application::journal_runtime_ports::JournalRuntimePorts {
    api_gateway::config::journal_runtime_ports(
        pool,
        &api_gateway::config::GatewayMetrics::default(),
    )
}

const ADMIN_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const ACTOR_ID: &str = "11111111-1111-1111-1111-111111111111";

fn admin_token() -> String {
    support::bearer_token(ADMIN_ID, &["ROLE_ADMIN"])
}

fn reconciliation_request(operation_id: Uuid, request: Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/admin/operations/{operation_id}/reconciliation"))
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .header("x-request-id", "manual-reconciliation-test")
        .body(Body::from(request.to_string()))
        .expect("request")
}

fn decision(disposition: &str, supersedes: Option<Uuid>) -> Value {
    json!({
        "disposition": disposition,
        "evidenceKind": "DATABASE_AUDIT",
        "evidenceReference": "DB-AUDIT-2026-0917",
        "reason": "Independent transaction audit confirms the operation outcome.",
        "supersedesResolutionId": supersedes,
    })
}

async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(
        &response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes(),
    )
    .expect("json")
}

async fn seed_unknown_operation(pool: &sqlx::PgPool, operation_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        insert into gateway_request_journal (
            id, actor_id, action, resource_path, state, outcome, outcome_evidence
        ) values (
            $1, $2, 'POST /api/moderation/reports', '/api/moderation/reports',
            'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED'
        )
        "#,
    )
    .bind(operation_id)
    .bind(ACTOR_ID)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn private_journal_repository_preserves_actor_scope_and_controlled_failure(
) -> anyhow::Result<()> {
    use api_gateway::adapters::out::repository::operation_journal_repository::PgOperationJournalRepository;
    use api_gateway::application::dto::journal_repository_dto::{
        CallerOperationJournalLookupDto, OperationJournalLookupDto,
    };
    use api_gateway::application::journal_repository_error::JournalRepositoryError;
    use api_gateway::application::ports::OperationJournalRepositoryPort;

    let app = support::start_gateway_app(false).await?;
    let id = Uuid::new_v4();
    seed_unknown_operation(&app.pool, id).await?;
    let repository = PgOperationJournalRepository::new(app.pool.clone());
    let persisted = repository
        .operation_status(OperationJournalLookupDto { operation_id: id })
        .await?
        .expect("private persistence record");
    assert_eq!(persisted.actor_id.as_deref(), Some(ACTOR_ID));
    assert_eq!(persisted.resource_path, "/api/moderation/reports");
    let status = app
        .state
        .operation_journal_service
        .operation_status(id)
        .await?
        .expect("domain status");
    assert_eq!(status.actor_identity(), Some(ACTOR_ID));
    assert_eq!(
        status.owner_mutation_scope(),
        Some("gateway.v1.CreateModerationReport")
    );
    let own = repository
        .caller_operation_status(CallerOperationJournalLookupDto {
            operation_id: id,
            actor_id: ACTOR_ID.into(),
        })
        .await?
        .expect("actor-scoped record");
    assert_eq!(own.operation_id, id);
    assert!(repository
        .caller_operation_status(CallerOperationJournalLookupDto {
            operation_id: id,
            actor_id: ADMIN_ID.into(),
        })
        .await?
        .is_none());
    app.shutdown().await?;
    assert!(matches!(
        repository
            .operation_status(OperationJournalLookupDto { operation_id: id })
            .await,
        Err(JournalRepositoryError::Storage)
    ));
    Ok(())
}

#[tokio::test]
async fn manual_reconciliation_is_append_only_admin_scoped_and_never_rewrites_the_journal(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let baseline_backlog =
        api_gateway::adapters::r#in::http::request_journal::reconciliation_backlog(&journal_ports(
            &app.pool,
        ))
        .await?
        .outstanding;
    let operation_id = Uuid::new_v4();
    seed_unknown_operation(&app.pool, operation_id).await?;

    let user = support::bearer_token(ACTOR_ID, &["ROLE_USER"]);
    let forbidden = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            operation_id,
            decision("NOT_COMMITTED", None),
            &user,
        ))
        .await?;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let mut injected_identity = decision("NOT_COMMITTED", None);
    injected_identity["actorId"] = json!(ADMIN_ID);
    let rejected = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            operation_id,
            injected_identity,
            &admin_token(),
        ))
        .await?;
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let created = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            operation_id,
            decision("NOT_COMMITTED", None),
            &admin_token(),
        ))
        .await?;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_json = json_body(created).await;
    let first_resolution_id = created_json["resolutionId"]
        .as_str()
        .expect("resolution id")
        .parse::<Uuid>()?;
    assert_eq!(created_json["operationId"], operation_id.to_string());
    assert_eq!(created_json["disposition"], "NOT_COMMITTED");
    assert_eq!(created_json["reviewerId"], ADMIN_ID);

    // A response lost after commit can be retried with the identical decision.
    // It returns the existing append-only record rather than adding a duplicate.
    let repeated = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            operation_id,
            decision("NOT_COMMITTED", None),
            &admin_token(),
        ))
        .await?;
    assert_eq!(repeated.status(), StatusCode::OK);
    let repeated_json = json_body(repeated).await;
    assert_eq!(repeated_json, created_json);
    assert_eq!(
        repeated_json["resolutionId"],
        first_resolution_id.to_string()
    );

    let state: String =
        sqlx::query_scalar("select state from gateway_request_journal where id = $1")
            .bind(operation_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(
        state, "UNKNOWN",
        "manual review must not rewrite the Gateway journal"
    );
    let outcome: String =
        sqlx::query_scalar("select outcome from gateway_request_journal where id = $1")
            .bind(operation_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(outcome, "REJECTED_BEFORE_COMMIT");

    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events where action = 'OPERATION_MANUALLY_RECONCILED' and resource_id = $1",
    )
    .bind(operation_id.to_string())
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        audit_count, 1,
        "an idempotent repeat must not duplicate audit"
    );

    let backlog = api_gateway::adapters::r#in::http::request_journal::reconciliation_backlog(
        &journal_ports(&app.pool),
    )
    .await?;
    assert_eq!(
        backlog.outstanding, baseline_backlog,
        "a terminal decision must not add to the pre-existing unresolved backlog"
    );

    let status = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/operations/{operation_id}"))
                .header("authorization", format!("Bearer {}", admin_token()))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(status.status(), StatusCode::OK);
    let status_json = json_body(status).await;
    let mut persisted_status =
        api_gateway::adapters::r#in::http::request_journal::operation_status(
            &journal_ports(&app.pool),
            operation_id,
        )
        .await?
        .expect("persisted operation status");
    assert_eq!(persisted_status.actor_identity(), Some(ACTOR_ID));
    // The admin route enriches the journal with an owner lookup. This seeded
    // operation has no moderation owner receipt; absence is not rollback proof.
    let owner_receipt_count: i64 = sqlx::query_scalar(
        "select count(*) from moderation_report_operation_receipts where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(owner_receipt_count, 0);
    persisted_status.owner_receipt.lookup_status =
        api_gateway::domain::operation::OwnerReceiptLookupStatus::NotFound;
    assert_eq!(status_json, serde_json::to_value(&persisted_status)?);
    assert!(status_json.get("actorId").is_none());
    assert_eq!(status_json["manualResolution"], created_json);
    assert_eq!(status_json["state"], "UNKNOWN");
    assert_eq!(status_json["outcome"], "REJECTED_BEFORE_COMMIT");
    assert_eq!(
        status_json["manualResolution"]["resolutionId"],
        first_resolution_id.to_string()
    );
    assert_eq!(
        status_json["manualResolution"]["evidenceReference"],
        "DB-AUDIT-2026-0917"
    );

    // Decisions cannot be edited or deleted in place; a correction must append a
    // new revision that explicitly supersedes this one.
    let update = sqlx::query(
        "update gateway_operation_manual_reconciliations set reason = 'rewritten' where resolution_id = $1",
    )
    .bind(first_resolution_id)
    .execute(&app.pool)
    .await;
    assert!(
        update.is_err(),
        "database trigger must reject in-place edits"
    );

    let correction = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            operation_id,
            decision("UNRESOLVED", Some(first_resolution_id)),
            &admin_token(),
        ))
        .await?;
    assert_eq!(correction.status(), StatusCode::CREATED);
    let correction_json = json_body(correction).await;
    assert_eq!(correction_json["revision"], 2);
    assert_eq!(
        correction_json["supersedesResolutionId"],
        first_resolution_id.to_string()
    );
    let corrected_outcome: String =
        sqlx::query_scalar("select outcome from gateway_request_journal where id = $1")
            .bind(operation_id)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(corrected_outcome, "UNKNOWN");

    let backlog = api_gateway::adapters::r#in::http::request_journal::reconciliation_backlog(
        &journal_ports(&app.pool),
    )
    .await?;
    assert_eq!(
        backlog.outstanding,
        baseline_backlog + 1,
        "an explicitly reopened decision is unresolved again"
    );

    let event_count: i64 = sqlx::query_scalar(
        "select count(*) from gateway_operation_manual_reconciliations where operation_id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(event_count, 2);

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn manual_reconciliation_requires_protected_evidence_and_unknown_state() -> anyhow::Result<()>
{
    let app = support::start_gateway_app(false).await?;
    let unknown = Uuid::new_v4();
    let completed = Uuid::new_v4();
    seed_unknown_operation(&app.pool, unknown).await?;
    sqlx::query(
        "insert into gateway_request_journal (id, action, resource_path, state, outcome, outcome_evidence, response_status) values ($1, 'POST /api/moderation/reports', '/api/moderation/reports', 'COMPLETED', 'COMMITTED', 'SUCCESS_RESPONSE', 201)",
    )
    .bind(completed)
    .execute(&app.pool)
    .await?;

    let invalid_evidence = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            unknown,
            json!({
                "disposition": "COMMITTED",
                "evidenceKind": "SERVICE_LOG",
                "evidenceReference": "../private?token=secret",
                "reason": "The service log supports a commit disposition."
            }),
            &admin_token(),
        ))
        .await?;
    assert_eq!(invalid_evidence.status(), StatusCode::BAD_REQUEST);

    let completed_operation = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            completed,
            decision("COMMITTED", None),
            &admin_token(),
        ))
        .await?;
    assert_eq!(completed_operation.status(), StatusCode::CONFLICT);

    let scoped_manual_rows: i64 = sqlx::query_scalar(
        "select count(*) from gateway_operation_manual_reconciliations where operation_id in ($1, $2)",
    )
    .bind(unknown)
    .bind(completed)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(scoped_manual_rows, 0);

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn verified_owner_receipt_appends_a_correction_to_a_manual_non_commit_decision(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let operation_id = Uuid::new_v4();
    sqlx::query(
        "insert into gateway_request_journal (id, actor_id, action, resource_path, state, outcome, outcome_evidence) values ($1, $2, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED')",
    )
    .bind(operation_id)
    .bind(ACTOR_ID)
    .execute(&app.pool)
    .await?;

    let manual = ManualOperationResolutionRequest {
        disposition: ManualResolutionDisposition::NotCommitted.into(),
        evidence_kind: ManualResolutionEvidenceKind::DatabaseAudit.into(),
        evidence_reference: "DB-AUDIT-2026-0917".into(),
        reason: "Independent transaction audit confirms the operation outcome.".into(),
        supersedes_resolution_id: None,
    };
    let manual = api_gateway::adapters::r#in::http::request_journal::record_manual_resolution(
        &journal_ports(&app.pool),
        operation_id,
        ADMIN_ID,
        &manual,
        None,
        None,
        None,
    )
    .await?;
    let api_gateway::adapters::r#in::http::request_journal::ManualResolutionWriteOutcome::Created(
        first,
    ) = manual
    else {
        panic!("expected first manual resolution");
    };

    api_gateway::adapters::r#in::http::request_journal::finish_owner_replay(
        &journal_ports(&app.pool),
        operation_id,
        "PostService",
        StatusCode::CREATED,
        Some("application/json"),
        Some(br#"{"postId":"33333333-3333-3333-3333-333333333333"}"#),
    )
    .await?;

    let status = api_gateway::adapters::r#in::http::request_journal::operation_status(
        &journal_ports(&app.pool),
        operation_id,
    )
    .await?
    .expect("operation status");
    assert_eq!(status.state, "COMPLETED");
    assert_eq!(
        status.outcome,
        api_gateway::domain::operation::OperationOutcome::Committed
    );
    let latest = status
        .manual_resolution
        .expect("corrected manual resolution");
    assert_eq!(latest.disposition, ManualResolutionDisposition::Committed);
    assert_eq!(
        latest.evidence_kind,
        ManualResolutionEvidenceKind::OwnerConfirmation
    );
    assert_eq!(latest.revision, 2);
    assert_eq!(latest.supersedes_resolution_id, Some(first.resolution_id));
    assert_eq!(latest.reviewer_id, "system:gateway-owner-replay");

    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events where resource_type = 'gateway_request_journal' and resource_id = $1 and action in ('OPERATION_MANUALLY_RECONCILED', 'OPERATION_RECONCILIATION_SUPERSEDED_BY_OWNER')",
    )
    .bind(operation_id.to_string())
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(audit_count, 2);

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn owner_receipt_mismatch_cannot_be_manually_called_committed_or_not_committed(
) -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let found_operation = Uuid::new_v4();
    let mismatch_operation = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into gateway_request_journal (
            id, actor_id, action, resource_path, state, outcome, outcome_evidence
        )
        values
            ($1, $3, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED'),
            ($2, $3, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'LEGACY_UNRESOLVED')
        "#,
    )
    .bind(found_operation)
    .bind(mismatch_operation)
    .bind(ACTOR_ID)
    .execute(&app.pool)
    .await?;

    let committed_at = chrono::Utc::now();
    app.set_owner_receipt_status(
        api_gateway::application::dto::post_dto::MutationReceiptStatusDto {
            operation_id: found_operation,
            actor_id: Uuid::parse_str(ACTOR_ID)?,
            scope: "post.v1.CreatePost".into(),
            committed_at,
            replay_until: committed_at + chrono::Duration::hours(24),
            state: api_gateway::application::dto::post_dto::MutationReceiptStateDto::Replayable,
        },
    );
    app.set_owner_receipt_status(
        api_gateway::application::dto::post_dto::MutationReceiptStatusDto {
            operation_id: mismatch_operation,
            actor_id: Uuid::new_v4(),
            scope: "post.v1.CreatePost".into(),
            committed_at,
            replay_until: committed_at + chrono::Duration::hours(24),
            state: api_gateway::application::dto::post_dto::MutationReceiptStateDto::Replayable,
        },
    );

    let unsupported_claim = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            found_operation,
            decision("COMMITTED", None),
            &admin_token(),
        ))
        .await?;
    assert_eq!(unsupported_claim.status(), StatusCode::CONFLICT);

    let verified_commit_request = json!({
        "disposition": "COMMITTED",
        "evidenceKind": "OWNER_CONFIRMATION",
        "evidenceReference": found_operation.to_string(),
        "reason": "The Gateway-verified owner receipt confirms commit."
    });
    let verified_commit = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            found_operation,
            verified_commit_request.clone(),
            &admin_token(),
        ))
        .await?;
    assert_eq!(verified_commit.status(), StatusCode::CREATED);

    app.set_owner_receipt_lookup_available(false);
    let repeated_verified_commit = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            found_operation,
            verified_commit_request,
            &admin_token(),
        ))
        .await?;
    assert_eq!(repeated_verified_commit.status(), StatusCode::OK);
    app.set_owner_receipt_lookup_available(true);

    let mismatch_commit = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            mismatch_operation,
            decision("NOT_COMMITTED", None),
            &admin_token(),
        ))
        .await?;
    assert_eq!(mismatch_commit.status(), StatusCode::CONFLICT);

    let mismatch_unresolved = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            mismatch_operation,
            decision("UNRESOLVED", None),
            &admin_token(),
        ))
        .await?;
    assert_eq!(mismatch_unresolved.status(), StatusCode::CREATED);
    let resolution = json_body(mismatch_unresolved).await;
    assert_eq!(resolution["disposition"], "UNRESOLVED");

    let mismatch_row_count: i64 = sqlx::query_scalar(
        "select count(*) from gateway_operation_manual_reconciliations where operation_id = $1",
    )
    .bind(mismatch_operation)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(mismatch_row_count, 1);

    app.shutdown().await?;
    Ok(())
}

/// T06.4: a matching owner receipt proves an UNKNOWN operation committed, and
/// the reconciler stores that so the caller's own status shows COMMITTED.
/// Anything short of a matching complete receipt stays UNKNOWN.
#[tokio::test]
async fn owner_receipt_reconciler_records_only_proven_commits() -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::{owner_receipts, request_journal};
    use api_gateway::application::dto::post_dto::{
        MutationReceiptStateDto, MutationReceiptStatusDto,
    };

    let app = support::start_gateway_app(false).await?;
    let [found, mismatch, missing, incomplete, decided] = [(); 5].map(|_| Uuid::new_v4());
    for id in [found, mismatch, missing, incomplete] {
        sqlx::query(
            r#"
            insert into gateway_request_journal (
                id, actor_id, action, resource_path, state, outcome, outcome_evidence
            ) values ($1, $2, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'STALE_OPERATION')
            "#,
        )
        .bind(id)
        .bind(ACTOR_ID)
        .execute(&app.pool)
        .await?;
    }
    // An operator already decided this one; the reconciler must not touch it.
    sqlx::query(
        r#"
        insert into gateway_request_journal (
            id, actor_id, action, resource_path, state, outcome, outcome_evidence
        ) values ($1, $2, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'MANUAL_RESOLUTION')
        "#,
    )
    .bind(decided)
    .bind(ACTOR_ID)
    .execute(&app.pool)
    .await?;
    let committed_at = chrono::Utc::now();
    let receipt = |operation_id, actor_id, state| MutationReceiptStatusDto {
        operation_id,
        actor_id,
        scope: "post.v1.CreatePost".into(),
        committed_at,
        replay_until: committed_at + chrono::Duration::hours(24),
        state,
    };
    let actor = Uuid::parse_str(ACTOR_ID)?;
    app.set_owner_receipt_status(receipt(found, actor, MutationReceiptStateDto::Replayable));
    app.set_owner_receipt_status(receipt(decided, actor, MutationReceiptStateDto::Replayable));
    app.set_owner_receipt_status(receipt(
        mismatch,
        Uuid::new_v4(),
        MutationReceiptStateDto::Replayable,
    ));
    app.set_owner_receipt_status(receipt(
        incomplete,
        actor,
        MutationReceiptStateDto::Incomplete,
    ));

    let row = |id: Uuid| {
        let pool = app.pool.clone();
        async move {
            sqlx::query_as::<_, (String, String, String, bool)>(
                "select state, outcome, outcome_evidence, owner_receipt_checked_at is not null \
                 from gateway_request_journal where id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
        }
    };

    // An unavailable owner only records a retry lease, never a negative outcome.
    app.set_owner_receipt_lookup_available(false);
    let pass =
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?;
    assert!(pass.unavailable >= 4 && pass.committed == 0);
    for id in [found, mismatch, missing, incomplete] {
        assert_eq!(
            row(id).await?,
            (
                "UNKNOWN".into(),
                "UNKNOWN".into(),
                "STALE_OPERATION".into(),
                true
            )
        );
    }

    // Retry only after the persisted lease expires, including unavailable owners.
    sqlx::query("update gateway_request_journal set owner_receipt_checked_at = now() - interval '31 seconds', owner_receipt_next_check_at = now() - interval '1 second' where id = any($1)")
        .bind([found, mismatch, missing, incomplete]).execute(&app.pool).await?;
    app.set_owner_receipt_lookup_available(true);
    let pass =
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?;
    assert_eq!(pass.committed, 1);
    assert!(pass.unresolved >= 3);
    assert_eq!(
        row(found).await?,
        (
            "UNKNOWN".into(),
            "COMMITTED".into(),
            "OWNER_RECEIPT".into(),
            true
        )
    );
    for id in [mismatch, missing, incomplete] {
        assert_eq!(
            row(id).await?,
            (
                "UNKNOWN".into(),
                "UNKNOWN".into(),
                "STALE_OPERATION".into(),
                true
            ),
            "{id}"
        );
    }
    assert_eq!(
        row(decided).await?,
        (
            "UNKNOWN".into(),
            "UNKNOWN".into(),
            "MANUAL_RESOLUTION".into(),
            false
        )
    );
    let audits: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events \
         where action = 'OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id = $1",
    )
    .bind(found.to_string())
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(audits, 1);

    // A present but incomplete receipt is not a verified owner commit, even on
    // the operator route. It cannot authorize an OWNER_CONFIRMATION decision.
    let confirmation = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            incomplete,
            json!({
                "disposition": "COMMITTED", "evidenceKind": "OWNER_CONFIRMATION",
                "evidenceReference": incomplete.to_string(), "reason": "Receipt lookup returned a row."
            }),
            &admin_token(),
        ))
        .await?;
    assert_eq!(confirmation.status(), StatusCode::CONFLICT);
    assert_eq!(row(incomplete).await?.1, "UNKNOWN");
    let resolutions: i64 = sqlx::query_scalar(
        "select count(*) from gateway_operation_manual_reconciliations where operation_id = $1",
    )
    .bind(incomplete)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(resolutions, 0);
    let manual_audits: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events where action = 'OPERATION_MANUALLY_RECONCILED' and resource_id = $1",
    )
    .bind(incomplete.to_string())
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(manual_audits, 0);

    // The caller sees the stored outcome; another caller still sees nothing.
    let mine = request_journal::caller_operation_status(&journal_ports(&app.pool), found, ACTOR_ID)
        .await?
        .expect("caller status");
    assert_eq!(
        mine.outcome,
        api_gateway::domain::operation::OperationOutcome::Committed
    );
    assert!(
        request_journal::caller_operation_status(&journal_ports(&app.pool), found, ADMIN_ID)
            .await?
            .is_none()
    );

    // The lease suppresses immediate polling; completed outcomes stay terminal.
    let pass =
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?;
    assert_eq!(pass, Default::default());
    let audits_after: i64 = sqlx::query_scalar(
        "select count(*) from admin_audit_events \
         where action = 'OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id = $1",
    )
    .bind(found.to_string())
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(audits_after, 1);

    // An operator can still confirm the receipt-backed commit explicitly.
    let confirmation = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            found,
            json!({
                "disposition": "COMMITTED",
                "evidenceKind": "OWNER_CONFIRMATION",
                "evidenceReference": found.to_string(),
                "reason": "The Gateway-verified owner receipt confirms commit."
            }),
            &admin_token(),
        ))
        .await?;
    assert_eq!(confirmation.status(), StatusCode::CREATED);

    // Missing/incomplete receipts are not permanent negative evidence. Late
    // commits, including an expired-body tombstone, must still resolve safely.
    app.set_owner_receipt_status(receipt(missing, actor, MutationReceiptStateDto::Expired));
    app.set_owner_receipt_status(receipt(
        incomplete,
        actor,
        MutationReceiptStateDto::Replayable,
    ));
    sqlx::query("update gateway_request_journal set owner_receipt_checked_at = now() - interval '31 seconds', owner_receipt_next_check_at = now() - interval '1 second' where id = any($1)")
        .bind([missing, incomplete]).execute(&app.pool).await?;
    let pass =
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?;
    assert_eq!(pass.committed, 2);
    for id in [missing, incomplete] {
        let mine =
            request_journal::caller_operation_status(&journal_ports(&app.pool), id, ACTOR_ID)
                .await?
                .expect("late owner commit remains caller scoped");
        assert_eq!(
            mine.outcome,
            api_gateway::domain::operation::OperationOutcome::Committed
        );
        assert_eq!(mine.state, "UNKNOWN");
        assert!(!mine.replayable);
        assert!(
            mine.response_status.is_none(),
            "do not invent a lost response"
        );
    }

    app.shutdown().await?;
    Ok(())
}

async fn seed_receipt_reconciliation(app: &support::TestApp) -> anyhow::Result<Uuid> {
    use api_gateway::application::dto::post_dto::{
        MutationReceiptStateDto, MutationReceiptStatusDto,
    };
    let id = Uuid::new_v4();
    sqlx::query("insert into gateway_request_journal (id, actor_id, action, resource_path, state, outcome, outcome_evidence) values ($1, $2, 'POST /api/posts', '/api/posts', 'UNKNOWN', 'UNKNOWN', 'STALE_OPERATION')")
        .bind(id).bind(ACTOR_ID).execute(&app.pool).await?;
    let committed_at = chrono::Utc::now();
    app.set_owner_receipt_status(MutationReceiptStatusDto {
        operation_id: id,
        actor_id: Uuid::parse_str(ACTOR_ID)?,
        scope: "post.v1.CreatePost".into(),
        committed_at,
        replay_until: committed_at + chrono::Duration::hours(24),
        state: MutationReceiptStateDto::Replayable,
    });
    Ok(id)
}

#[tokio::test]
async fn owner_receipt_committed_outcome_survives_owner_outage_and_receipt_pruning(
) -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::{owner_receipts, request_journal};

    let app = support::start_gateway_app(false).await?;
    let unconfirmed = seed_receipt_reconciliation(&app).await?;
    let confirmed = seed_receipt_reconciliation(&app).await?;
    let corrected = seed_receipt_reconciliation(&app).await?;
    // A previous operator decision can be corrected once a real owner receipt
    // appears. That path has MANUAL_RESOLUTION projection evidence, but its
    // immutable OWNER_CONFIRMATION decision is just as authoritative.
    let previous: ManualOperationResolutionRequest =
        serde_json::from_value(decision("NOT_COMMITTED", None))?;
    let previous = request_journal::record_manual_resolution(
        &journal_ports(&app.pool),
        corrected,
        ADMIN_ID,
        &previous,
        None,
        None,
        None,
    )
    .await?;
    let request_journal::ManualResolutionWriteOutcome::Created(previous) = previous else {
        anyhow::bail!("expected initial operator decision");
    };
    let pass =
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?;
    assert_eq!(pass.committed, 2);

    let mut confirmation_request = json!({
        "disposition": "COMMITTED", "evidenceKind": "OWNER_CONFIRMATION",
        "evidenceReference": confirmed.to_string(), "reason": "Verified matching owner receipt."
    });
    let response = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            confirmed,
            confirmation_request.clone(),
            &admin_token(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let first_confirmation_id = json_body(response).await["resolutionId"]
        .as_str()
        .expect("confirmation id")
        .parse::<Uuid>()?;
    // A superseding positive confirmation must also retain the owner proof.
    confirmation_request["supersedesResolutionId"] = json!(first_confirmation_id);
    confirmation_request["reason"] = json!("The same receipt confirms a revised operator note.");
    let response = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            confirmed,
            confirmation_request.clone(),
            &admin_token(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let response = json_body(response).await;
    assert_eq!(response["revision"], 2);
    let confirmation_id = response["resolutionId"]
        .as_str()
        .expect("superseding confirmation id")
        .parse::<Uuid>()?;

    let correction_request = json!({
        "disposition": "COMMITTED", "evidenceKind": "OWNER_CONFIRMATION",
        "evidenceReference": corrected.to_string(), "reason": "The matching owner receipt corrects the previous decision.",
        "supersedesResolutionId": previous.resolution_id
    });
    let response = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            corrected,
            correction_request.clone(),
            &admin_token(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let response = json_body(response).await;
    assert_eq!(response["revision"], 2);
    let correction_id = response["resolutionId"]
        .as_str()
        .expect("owner correction id")
        .parse::<Uuid>()?;

    for pruned in [false, true] {
        app.set_owner_receipt_lookup_available(pruned);
        if pruned {
            app.remove_owner_receipt(unconfirmed);
            app.remove_owner_receipt(confirmed);
            app.remove_owner_receipt(corrected);
        }
        for (id, supersedes, expected_evidence, expected_resolutions) in [
            (unconfirmed, None, "OWNER_RECEIPT", 0),
            (confirmed, Some(confirmation_id), "OWNER_RECEIPT", 2),
            (corrected, Some(correction_id), "MANUAL_RESOLUTION", 2),
        ] {
            for disposition in ["NOT_COMMITTED", "UNRESOLVED", "COMMITTED"] {
                let request = decision(disposition, supersedes);
                let response = app
                    .router
                    .clone()
                    .oneshot(reconciliation_request(id, request.clone(), &admin_token()))
                    .await?;
                assert_eq!(response.status(), StatusCode::CONFLICT, "pruned={pruned}");

                // Test the persistence boundary directly too, independently of
                // the route's best-effort owner lookup.
                let request: ManualOperationResolutionRequest = serde_json::from_value(request)?;
                let outcome = request_journal::record_manual_resolution(
                    &journal_ports(&app.pool),
                    id,
                    ADMIN_ID,
                    &request,
                    None,
                    None,
                    None,
                )
                .await?;
                assert_eq!(
                    outcome,
                    request_journal::ManualResolutionWriteOutcome::Conflict
                );
            }
            let durable: (String, String, String, Option<i32>, Option<Vec<u8>>) = sqlx::query_as(
                "select state, outcome, outcome_evidence, response_status, response_body \
                 from gateway_request_journal where id = $1",
            )
            .bind(id)
            .fetch_one(&app.pool)
            .await?;
            assert_eq!(
                durable,
                (
                    "UNKNOWN".into(),
                    "COMMITTED".into(),
                    expected_evidence.into(),
                    None,
                    None
                )
            );
            let resolutions: i64 = sqlx::query_scalar(
                "select count(*) from gateway_operation_manual_reconciliations where operation_id = $1",
            )
            .bind(id)
            .fetch_one(&app.pool)
            .await?;
            assert_eq!(resolutions, expected_resolutions);
            let manual_audits: i64 = sqlx::query_scalar(
                "select count(*) from admin_audit_events where action = 'OPERATION_MANUALLY_RECONCILED' and resource_id = $1",
            )
            .bind(id.to_string())
            .fetch_one(&app.pool)
            .await?;
            assert_eq!(
                manual_audits, resolutions,
                "rejected decisions leave no audit or revision"
            );
        }
        for (id, request, resolution_id) in [
            (confirmed, &confirmation_request, confirmation_id),
            (corrected, &correction_request, correction_id),
        ] {
            let retry = app
                .router
                .clone()
                .oneshot(reconciliation_request(id, request.clone(), &admin_token()))
                .await?;
            assert_eq!(retry.status(), StatusCode::OK);
            assert_eq!(
                json_body(retry).await["resolutionId"],
                resolution_id.to_string()
            );
        }
    }
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn owner_receipt_commit_between_route_lookup_and_manual_write_is_protected(
) -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::owner_receipts;

    let app = support::start_gateway_app(false).await?;
    let id = seed_receipt_reconciliation(&app).await?;
    let gate = app.post_client.hold_owner_receipt_lookup();
    let router = app.router.clone();
    let request = reconciliation_request(id, decision("NOT_COMMITTED", None), &admin_token());
    let manual = tokio::spawn(async move { router.oneshot(request).await });
    tokio::time::timeout(std::time::Duration::from_secs(3), gate.entered.acquire())
        .await??
        .forget();
    app.post_client.clear_owner_receipt_lookup_gate();

    // The route has read UNKNOWN, but its owner RPC has not answered. The
    // reconciler now persists the commit before that request can write.
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?
            .committed,
        1
    );
    app.set_owner_receipt_lookup_available(false);
    gate.released.add_permits(1);
    let response = manual.await??;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let durable: (String, String) = sqlx::query_as(
        "select outcome, outcome_evidence from gateway_request_journal where id = $1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(durable, ("COMMITTED".into(), "OWNER_RECEIPT".into()));
    let resolutions: i64 = sqlx::query_scalar(
        "select count(*) from gateway_operation_manual_reconciliations where operation_id = $1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(resolutions, 0);
    let audits: (i64, i64) = sqlx::query_as(
        "select count(*) filter (where action = 'OPERATION_RECONCILED_FROM_OWNER_RECEIPT'), \
                count(*) filter (where action = 'OPERATION_MANUALLY_RECONCILED') \
         from admin_audit_events where resource_id = $1",
    )
    .bind(id.to_string())
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(audits, (1, 0));
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn owner_receipt_reconciliation_rolls_back_with_audit_and_commits_once() -> anyhow::Result<()>
{
    use api_gateway::adapters::r#in::http::owner_receipts;
    let app = support::start_gateway_app(false).await?;
    let id = seed_receipt_reconciliation(&app).await?;
    sqlx::raw_sql(
        r#"
        create function e2e_reject_receipt_audit() returns trigger language plpgsql as $$
        begin
            if new.action = 'OPERATION_RECONCILED_FROM_OWNER_RECEIPT' then
                raise exception 'injected receipt audit rejection' using errcode = '23514';
            end if;
            return new;
        end $$;
        create trigger e2e_reject_receipt_audit before insert on admin_audit_events
        for each row execute function e2e_reject_receipt_audit();
    "#,
    )
    .execute(&app.pool)
    .await?;
    assert!(owner_receipts::reconcile_from_owner_receipts(
        &app.state,
        &journal_ports(&app.pool),
        50
    )
    .await
    .is_err());
    let outcome: (String, String) = sqlx::query_as(
        "select outcome, outcome_evidence from gateway_request_journal where id = $1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(outcome, ("UNKNOWN".into(), "STALE_OPERATION".into()));
    let audit_count: i64 = sqlx::query_scalar("select count(*) from admin_audit_events where action = 'OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id = $1")
        .bind(id.to_string()).fetch_one(&app.pool).await?;
    assert_eq!(
        audit_count, 0,
        "outcome and mandatory audit roll back together"
    );
    sqlx::raw_sql("drop trigger e2e_reject_receipt_audit on admin_audit_events; drop function e2e_reject_receipt_audit();")
        .execute(&app.pool).await?;
    sqlx::query("update gateway_request_journal set owner_receipt_checked_at = now() - interval '31 seconds', owner_receipt_next_check_at = now() - interval '1 second' where id = $1")
        .bind(id).execute(&app.pool).await?;
    let gate = app.post_client.hold_owner_receipt_lookup();
    let state = app.state.clone();
    let pool = app.pool.clone();
    let first = tokio::spawn(async move {
        owner_receipts::reconcile_from_owner_receipts(&state, &journal_ports(&pool), 50).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), gate.entered.acquire())
        .await??
        .forget();
    app.post_client.clear_owner_receipt_lookup_gate();
    // The first replica has claimed this row and is still in its owner RPC.
    // A second replica must finish without claiming or committing it again.
    let second =
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?;
    assert_eq!(second.committed, 0);
    gate.released.add_permits(1);
    assert_eq!(first.await??.committed, 1);
    let audit_count: i64 = sqlx::query_scalar("select count(*) from admin_audit_events where action = 'OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id = $1")
        .bind(id.to_string()).fetch_one(&app.pool).await?;
    assert_eq!(audit_count, 1);

    // Verify the actual caller API and its privacy boundary after recovery.
    for (actor, expected) in [
        (ACTOR_ID, StatusCode::OK),
        (ADMIN_ID, StatusCode::NOT_FOUND),
    ] {
        let token = support::bearer_token(actor, &["ROLE_USER"]);
        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/operations/{id}"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), expected);
        if expected == StatusCode::OK {
            assert_eq!(response.headers()["cache-control"], "no-store");
            let value = json_body(response).await;
            assert_eq!(value["outcome"], "COMMITTED");
            assert_eq!(value["state"], "UNKNOWN");
            assert_eq!(value["responseStatus"], Value::Null);
            assert_eq!(value["replayable"], false);
            assert_eq!(value.as_object().expect("status object").len(), 7);
            let actual_fields: std::collections::BTreeSet<_> = value
                .as_object()
                .expect("status object")
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(
                actual_fields,
                std::collections::BTreeSet::from([
                    "operationId",
                    "state",
                    "outcome",
                    "responseStatus",
                    "createdAt",
                    "finishedAt",
                    "replayable",
                ])
            );
        }
    }
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn owner_receipt_reconciliation_preserves_an_inflight_operator_decision() -> anyhow::Result<()>
{
    use api_gateway::adapters::r#in::http::owner_receipts;
    let app = support::start_gateway_app(false).await?;
    let id = seed_receipt_reconciliation(&app).await?;
    let gate = app.post_client.hold_owner_receipt_lookup();
    let state = app.state.clone();
    let pool = app.pool.clone();
    let reconciler = tokio::spawn(async move {
        owner_receipts::reconcile_from_owner_receipts(&state, &journal_ports(&pool), 50).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), gate.entered.acquire())
        .await??
        .forget();
    app.post_client.clear_owner_receipt_lookup_gate();
    // The owner lookup has started and holds no journal row lock or connection.
    let confirmation = app
        .router
        .clone()
        .oneshot(reconciliation_request(
            id,
            json!({
                "disposition": "COMMITTED", "evidenceKind": "OWNER_CONFIRMATION",
                "evidenceReference": id.to_string(), "reason": "Verified matching owner receipt."
            }),
            &admin_token(),
        ))
        .await?;
    assert_eq!(confirmation.status(), StatusCode::CREATED);
    let resolution_id = json_body(confirmation).await["resolutionId"].clone();
    gate.released.add_permits(1);
    assert_eq!(reconciler.await??.committed, 0);
    let outcome: (String, String) = sqlx::query_as(
        "select outcome, outcome_evidence from gateway_request_journal where id = $1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(outcome, ("COMMITTED".into(), "MANUAL_RESOLUTION".into()));
    let recorded: Uuid = sqlx::query_scalar("select resolution_id from gateway_operation_manual_reconciliations where operation_id = $1")
        .bind(id).fetch_one(&app.pool).await?;
    assert_eq!(resolution_id, recorded.to_string());
    let audit_count: i64 = sqlx::query_scalar("select count(*) from admin_audit_events where action = 'OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id = $1")
        .bind(id.to_string()).fetch_one(&app.pool).await?;
    assert_eq!(
        audit_count, 0,
        "a concurrent operator decision is never overwritten"
    );
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn owner_receipt_reconciliation_batch_is_bounded_and_fair() -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::owner_receipts;
    let app = support::start_gateway_app(false).await?;
    for invalid in [-1, 0, 51] {
        assert!(owner_receipts::reconcile_from_owner_receipts(
            &app.state,
            &journal_ports(&app.pool),
            invalid
        )
        .await
        .is_err());
    }
    // Old unavailable work cannot monopolize every later pass.
    let unavailable = seed_receipt_reconciliation(&app).await?;
    sqlx::query(
        "update gateway_request_journal set created_at = now() - interval '1 hour' where id = $1",
    )
    .bind(unavailable)
    .execute(&app.pool)
    .await?;
    app.set_owner_receipt_lookup_available(false);
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 1)
            .await?
            .unavailable,
        1
    );
    let newer = seed_receipt_reconciliation(&app).await?;
    app.set_owner_receipt_lookup_available(true);
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 1)
            .await?
            .committed,
        1
    );
    let outcome: String =
        sqlx::query_scalar("select outcome from gateway_request_journal where id = $1")
            .bind(newer)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(outcome, "COMMITTED");
    let older: String =
        sqlx::query_scalar("select outcome from gateway_request_journal where id = $1")
            .bind(unavailable)
            .fetch_one(&app.pool)
            .await?;
    assert_eq!(older, "UNKNOWN");
    app.shutdown().await?;
    Ok(())
}

async fn force_receipt_due(app: &support::TestApp, id: Uuid) -> anyhow::Result<()> {
    sqlx::query("update gateway_request_journal set owner_receipt_next_check_at=now()-interval '1 second' where id=$1")
        .bind(id).execute(&app.pool).await?;
    Ok(())
}

async fn receipt_schedule(app: &support::TestApp, id: Uuid) -> anyhow::Result<Value> {
    Ok(sqlx::query_scalar("select jsonb_build_object('state',state,'outcome',outcome,'evidence',outcome_evidence,'attempt',owner_receipt_attempt_count,'next',owner_receipt_next_check_at,'claim',owner_receipt_claim_id,'suspended',owner_receipt_suspended_reason) from gateway_request_journal where id=$1")
        .bind(id).fetch_one(&app.pool).await?)
}

#[tokio::test]
async fn receipt_schedule_suspends_inapplicable_identity_without_lookup_and_resumes_repairs(
) -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::owner_receipts;
    let app = support::start_gateway_app(false).await?;
    let mut ids = Vec::new();
    for (actor, action, path, reason) in [
        (
            Some(ACTOR_ID),
            "POST /api/unimplemented",
            "/api/unimplemented",
            "NOT_APPLICABLE",
        ),
        (
            Some(ACTOR_ID),
            "POST /api/posts",
            "/different-path",
            "NOT_APPLICABLE",
        ),
        (None, "POST /api/posts", "/api/posts", "INVALID_ACTOR"),
        (
            Some("malformed"),
            "POST /api/posts",
            "/api/posts",
            "INVALID_ACTOR",
        ),
        (
            Some("00000000-0000-0000-0000-000000000000"),
            "POST /api/posts",
            "/api/posts",
            "INVALID_ACTOR",
        ),
    ] {
        let id = Uuid::new_v4();
        sqlx::query("insert into gateway_request_journal(id,actor_id,action,resource_path,state,outcome,outcome_evidence) values($1,$2,$3,$4,'UNKNOWN','UNKNOWN','STALE_OPERATION')")
            .bind(id).bind(actor).bind(action).bind(path).execute(&app.pool).await?;
        ids.push((id, reason));
    }
    let pass =
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?;
    assert_eq!(pass.suspended, 5);
    assert_eq!(app.post_client.owner_receipt_lookup_calls(), 0);
    for (id, reason) in &ids {
        let row = receipt_schedule(&app, *id).await?;
        assert_eq!(row["outcome"], "UNKNOWN");
        assert_eq!(row["evidence"], "STALE_OPERATION");
        assert_eq!(row["suspended"], *reason);
        assert_eq!(row["next"], Value::Null);
        assert_eq!(row["claim"], Value::Null);
    }
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?
            .suspended,
        0
    );
    assert_eq!(app.post_client.owner_receipt_lookup_calls(), 0);
    for (id, _) in ids {
        sqlx::query("update gateway_request_journal set actor_id=$2,action='POST /api/posts',resource_path='/api/posts' where id=$1")
            .bind(id).bind(ACTOR_ID).execute(&app.pool).await?;
        let reset = receipt_schedule(&app, id).await?;
        assert_eq!(reset["attempt"], 0);
        assert_eq!(reset["suspended"], Value::Null);
        app.set_owner_receipt_status(
            api_gateway::application::dto::post_dto::MutationReceiptStatusDto {
                operation_id: id,
                actor_id: Uuid::parse_str(ACTOR_ID)?,
                scope: "post.v1.CreatePost".into(),
                committed_at: chrono::Utc::now(),
                replay_until: chrono::Utc::now() + chrono::Duration::hours(24),
                state: api_gateway::application::dto::post_dto::MutationReceiptStateDto::Expired,
            },
        );
    }
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?
            .committed,
        5
    );
    assert_eq!(app.post_client.owner_receipt_lookup_calls(), 5);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn missing_receipt_backoff_caps_without_retiring_unknown_or_losing_late_commit(
) -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::owner_receipts;
    let app = support::start_gateway_app(false).await?;
    let id = seed_receipt_reconciliation(&app).await?;
    app.remove_owner_receipt(id);
    for (attempt, delay) in [
        (1, 30.0),
        (2, 60.0),
        (3, 120.0),
        (4, 240.0),
        (5, 300.0),
        (6, 300.0),
    ] {
        if attempt > 1 {
            force_receipt_due(&app, id).await?;
        }
        assert_eq!(
            owner_receipts::reconcile_from_owner_receipts(
                &app.state,
                &journal_ports(&app.pool),
                50
            )
            .await?
            .unresolved,
            1
        );
        let row = receipt_schedule(&app, id).await?;
        assert_eq!(row["outcome"], "UNKNOWN");
        assert_eq!(row["attempt"], attempt);
        assert_eq!(row["claim"], Value::Null);
        assert_eq!(row["suspended"], Value::Null);
        let remaining:f64=sqlx::query_scalar("select extract(epoch from owner_receipt_next_check_at-now())::double precision from gateway_request_journal where id=$1")
            .bind(id).fetch_one(&app.pool).await?;
        assert!(
            (delay - 3.0..=delay).contains(&remaining),
            "attempt{attempt}: {remaining}"
        );
        let calls = app.post_client.owner_receipt_lookup_calls();
        assert_eq!(
            owner_receipts::reconcile_from_owner_receipts(
                &app.state,
                &journal_ports(&app.pool),
                50
            )
            .await?
            .unresolved,
            0
        );
        assert_eq!(app.post_client.owner_receipt_lookup_calls(), calls);
    }
    sqlx::query("update gateway_request_journal set owner_receipt_attempt_count=2147483647,owner_receipt_next_check_at=now()-interval '1 second' where id=$1")
        .bind(id).execute(&app.pool).await?;
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?
            .unresolved,
        1
    );
    assert_eq!(receipt_schedule(&app, id).await?["attempt"], 2147483647);
    let remaining:f64=sqlx::query_scalar("select extract(epoch from owner_receipt_next_check_at-now())::double precision from gateway_request_journal where id=$1")
        .bind(id).fetch_one(&app.pool).await?;
    assert!((297.0..=300.0).contains(&remaining));
    app.set_owner_receipt_status(
        api_gateway::application::dto::post_dto::MutationReceiptStatusDto {
            operation_id: id,
            actor_id: Uuid::parse_str(ACTOR_ID)?,
            scope: "post.v1.CreatePost".into(),
            committed_at: chrono::Utc::now(),
            replay_until: chrono::Utc::now() - chrono::Duration::hours(1),
            state: api_gateway::application::dto::post_dto::MutationReceiptStateDto::Expired,
        },
    );
    force_receipt_due(&app, id).await?;
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?
            .committed,
        1
    );
    let row = receipt_schedule(&app, id).await?;
    assert_eq!(row["outcome"], "COMMITTED");
    assert_eq!(row["evidence"], "OWNER_RECEIPT");
    assert_eq!(row["next"], Value::Null);
    assert_eq!(row["claim"], Value::Null);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn stale_receipt_claim_cannot_commit_or_rewrite_a_newer_retry_schedule() -> anyhow::Result<()>
{
    use api_gateway::adapters::r#in::http::owner_receipts;
    let app = support::start_gateway_app(false).await?;
    for old_answer_available in [true, false] {
        let id = seed_receipt_reconciliation(&app).await?;
        let gate = app.post_client.hold_owner_receipt_lookup();
        let state = app.state.clone();
        let pool = app.pool.clone();
        let old = tokio::spawn(async move {
            owner_receipts::reconcile_from_owner_receipts(&state, &journal_ports(&pool), 1).await
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), gate.entered.acquire())
            .await??
            .forget();
        let old_claim = receipt_schedule(&app, id).await?["claim"].clone();
        assert_ne!(old_claim, Value::Null);
        app.post_client.clear_owner_receipt_lookup_gate();
        force_receipt_due(&app, id).await?;
        app.set_owner_receipt_lookup_available(false);
        assert_eq!(
            owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 1)
                .await?
                .unavailable,
            1
        );
        let new = receipt_schedule(&app, id).await?;
        assert_eq!(new["attempt"], 2);
        app.set_owner_receipt_lookup_available(old_answer_available);
        gate.released.add_permits(1);
        assert_eq!(old.await??.committed, 0);
        assert_eq!(
            receipt_schedule(&app, id).await?,
            new,
            "old answer rewrote newer claim"
        );
        app.set_owner_receipt_lookup_available(true);
        force_receipt_due(&app, id).await?;
        assert_eq!(
            owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 1)
                .await?
                .committed,
            1
        );
        let audits:i64=sqlx::query_scalar("select count(*) from admin_audit_events where action='OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id=$1")
            .bind(id.to_string()).fetch_one(&app.pool).await?;
        assert_eq!(audits, 1);
    }
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn lookup_identity_change_fences_an_inflight_receipt_before_new_suspension(
) -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::owner_receipts;
    let app = support::start_gateway_app(false).await?;
    let id = seed_receipt_reconciliation(&app).await?;
    let gate = app.post_client.hold_owner_receipt_lookup();
    let state = app.state.clone();
    let pool = app.pool.clone();
    let old = tokio::spawn(async move {
        owner_receipts::reconcile_from_owner_receipts(&state, &journal_ports(&pool), 1).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), gate.entered.acquire())
        .await??
        .forget();
    app.post_client.clear_owner_receipt_lookup_gate();
    sqlx::query("update gateway_request_journal set action='POST /unsupported',resource_path='/unsupported' where id=$1")
        .bind(id).execute(&app.pool).await?;
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 1)
            .await?
            .suspended,
        1
    );
    assert_eq!(
        receipt_schedule(&app, id).await?["suspended"],
        "NOT_APPLICABLE"
    );
    // Restore the original tuple (ABA) while its old receipt answer is held.
    // The first token must stay invalid even though actor/scope match again.
    sqlx::query("update gateway_request_journal set action='POST /api/posts',resource_path='/api/posts' where id=$1")
        .bind(id).execute(&app.pool).await?;
    app.set_owner_receipt_lookup_available(false);
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 1)
            .await?
            .unavailable,
        1
    );
    app.set_owner_receipt_lookup_available(true);
    let current = receipt_schedule(&app, id).await?;
    gate.released.add_permits(1);
    assert_eq!(old.await??.committed, 0);
    assert_eq!(receipt_schedule(&app, id).await?, current);
    let audits:i64=sqlx::query_scalar("select count(*) from admin_audit_events where action='OPERATION_RECONCILED_FROM_OWNER_RECEIPT' and resource_id=$1")
        .bind(id.to_string()).fetch_one(&app.pool).await?;
    assert_eq!(audits, 0);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn incomplete_mismatched_and_missing_receipts_back_off_then_accept_later_proof(
) -> anyhow::Result<()> {
    use api_gateway::adapters::r#in::http::owner_receipts;
    use api_gateway::application::dto::post_dto::{
        MutationReceiptStateDto, MutationReceiptStatusDto,
    };
    let app = support::start_gateway_app(false).await?;
    let incomplete = seed_receipt_reconciliation(&app).await?;
    let mismatch = seed_receipt_reconciliation(&app).await?;
    let missing = seed_receipt_reconciliation(&app).await?;
    let receipt = |operation_id, actor_id, state| MutationReceiptStatusDto {
        operation_id,
        actor_id,
        scope: "post.v1.CreatePost".into(),
        committed_at: chrono::Utc::now(),
        replay_until: chrono::Utc::now() + chrono::Duration::hours(24),
        state,
    };
    let actor = Uuid::parse_str(ACTOR_ID)?;
    app.set_owner_receipt_status(receipt(
        incomplete,
        actor,
        MutationReceiptStateDto::Incomplete,
    ));
    app.set_owner_receipt_status(receipt(
        mismatch,
        Uuid::new_v4(),
        MutationReceiptStateDto::Replayable,
    ));
    app.remove_owner_receipt(missing);
    app.set_owner_receipt_lookup_available(false);
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?
            .unavailable,
        3
    );
    app.set_owner_receipt_lookup_available(true);
    for id in [incomplete, mismatch, missing] {
        force_receipt_due(&app, id).await?;
    }
    let pass =
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?;
    assert_eq!(pass.unresolved, 3);
    assert_eq!(pass.suspended, 0);
    assert_eq!(pass.committed, 0);
    for id in [incomplete, mismatch, missing] {
        let row = receipt_schedule(&app, id).await?;
        assert_eq!(row["outcome"], "UNKNOWN");
        assert_eq!(row["attempt"], 2);
        assert_eq!(row["suspended"], Value::Null);
        let remaining:f64=sqlx::query_scalar("select extract(epoch from owner_receipt_next_check_at-now())::double precision from gateway_request_journal where id=$1")
            .bind(id).fetch_one(&app.pool).await?;
        assert!((57.0..=60.0).contains(&remaining));
        app.set_owner_receipt_status(receipt(id, actor, MutationReceiptStateDto::Expired));
        force_receipt_due(&app, id).await?;
    }
    assert_eq!(
        owner_receipts::reconcile_from_owner_receipts(&app.state, &journal_ports(&app.pool), 50)
            .await?
            .committed,
        3
    );
    for id in [incomplete, mismatch, missing] {
        assert_eq!(receipt_schedule(&app, id).await?["outcome"], "COMMITTED");
    }
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn typed_journal_sessions_preserve_metrics_fingerprints_and_transaction_retry(
) -> anyhow::Result<()> {
    use api_gateway::application::dto::journal_write_dto::*;
    use api_gateway::application::dto::operation_view_dto::OperationOutcomeDto;
    use api_gateway::application::journal_write_error::JournalWriteError;
    let app = support::start_gateway_app(false).await?;
    let metrics = api_gateway::config::GatewayMetrics::default();
    let repository =
        api_gateway::config::journal_dispatch_repository(&app.pool, Some(metrics.clone()));
    let id = Uuid::new_v4();
    let intent = || JournalIntentDto {
        id,
        actor: Some(ACTOR_ID),
        action: "POST /api/moderation/reports",
        path: "/api/moderation/reports",
        request_id: None,
        ip: None,
        agent: None,
        key: Some("typed-journal-session"),
        context: "private-context",
        fingerprint_body: b"private-body",
    };
    let mut held = repository.acquire_intent_connection().await?;
    assert!(metrics
        .render_prometheus()
        .contains("gateway_db_pool_observed_holders{pool=\"journal\",caller=\"intent\"} 1"));
    assert_eq!(held.insert_intent(intent()).await?, Some(id));
    assert_eq!(held.insert_intent(intent()).await?, None);
    drop(held);
    assert!(metrics
        .render_prometheus()
        .contains("gateway_db_pool_observed_holders{pool=\"journal\",caller=\"intent\"} 0"));
    let query = |actor, body| JournalReplayQueryDto {
        actor,
        key: Some("typed-journal-session"),
        context: "private-context",
        body,
    };
    let replay = repository
        .owner_replay_identity(query(Some(ACTOR_ID), b"private-body"))
        .await?
        .expect("identity");
    assert_eq!(replay.id, id);
    assert!(replay.same_request && replay.unexpired);
    assert!(
        !repository
            .owner_replay_identity(query(Some(ACTOR_ID), b"different-body"))
            .await?
            .expect("identity")
            .same_request
    );
    assert!(repository
        .owner_replay_identity(query(Some(ADMIN_ID), b"private-body"))
        .await?
        .is_none());
    let command = || JournalCompletionCommandDto {
        id,
        outcome: OperationOutcomeDto::Committed,
        evidence: JournalOutcomeEvidenceDto::SuccessResponse,
        status: 201,
        content_type: Some("application/json"),
        body: Some(b"{\"created\":true}"),
    };
    let mut lock = app.pool.begin().await?;
    sqlx::query("select id from gateway_request_journal where id=$1 for update")
        .bind(id)
        .fetch_one(&mut *lock)
        .await?;
    let mut completion = repository
        .acquire_completion_connection(JournalCompletionCallerDto::Completion)
        .await?;
    assert_eq!(
        completion.record_completion(command()).await,
        Err(JournalWriteError::Transient)
    );
    assert!(metrics
        .render_prometheus()
        .contains("gateway_db_pool_observed_holders{pool=\"journal\",caller=\"completion\"} 1"));
    let audit_count = || {
        sqlx::query_scalar::<_, i64>("select count(*) from admin_audit_events where id=$1").bind(id)
    };
    assert_eq!(audit_count().fetch_one(&app.pool).await?, 0);
    lock.rollback().await?;
    assert_eq!(
        completion.record_completion(command()).await?,
        JournalCompletionWriteDto::Recorded
    );
    assert_eq!(
        completion.record_completion(command()).await?,
        JournalCompletionWriteDto::AlreadyResolved
    );
    assert_eq!(audit_count().fetch_one(&app.pool).await?, 1);
    drop(completion);
    assert!(metrics
        .render_prometheus()
        .contains("gateway_db_pool_observed_holders{pool=\"journal\",caller=\"completion\"} 0"));
    let response = repository
        .response_replay(query(Some(ACTOR_ID), b"private-body"))
        .await?
        .expect("response");
    assert_eq!(response.response_status, Some(201));
    assert_eq!(
        response.response_body.as_deref(),
        Some(b"{\"created\":true}".as_slice())
    );
    let invalid = Uuid::new_v4();
    seed_unknown_operation(&app.pool, invalid).await?;
    sqlx::query("update gateway_request_journal set state='IN_PROGRESS',outcome='PENDING',outcome_evidence='INTENT_RECORDED' where id=$1").bind(invalid).execute(&app.pool).await?;
    let mut completion = repository
        .acquire_completion_connection(JournalCompletionCallerDto::Completion)
        .await?;
    assert_eq!(
        completion
            .record_completion(JournalCompletionCommandDto {
                id: invalid,
                outcome: OperationOutcomeDto::Committed,
                evidence: JournalOutcomeEvidenceDto::PreDispatchRejection,
                status: 201,
                content_type: None,
                body: None
            })
            .await,
        Err(JournalWriteError::Permanent)
    );
    let state: String = sqlx::query_scalar("select state from gateway_request_journal where id=$1")
        .bind(invalid)
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(
        state, "IN_PROGRESS",
        "constraint failure must roll back the outcome and audit together"
    );
    drop(completion);
    app.shutdown().await?;
    assert!(matches!(
        repository.acquire_intent_connection().await,
        Err(JournalWriteError::Permanent)
    ));
    Ok(())
}

#[tokio::test]
async fn typed_owner_receipt_repository_preserves_lease_fences_and_atomic_audit(
) -> anyhow::Result<()> {
    use api_gateway::application::dto::journal_write_dto::*;
    use api_gateway::application::journal_write_error::JournalWriteError;
    let app = support::start_gateway_app(false).await?;
    let id = Uuid::new_v4();
    seed_unknown_operation(&app.pool, id).await?;
    let repository = api_gateway::config::owner_receipt_journal_repository(&app.pool);
    let claim = repository
        .claim_next(OwnerReceiptClaimRequestDto {
            lease_seconds: 30.0,
        })
        .await?
        .expect("claim");
    assert_eq!(claim.operation_id, id);
    assert_eq!(claim.attempt, 1);
    assert!(repository
        .claim_next(OwnerReceiptClaimRequestDto {
            lease_seconds: 30.0
        })
        .await?
        .is_none());
    let identity = || OwnerReceiptIdentityDto {
        actor: Some(ACTOR_ID),
        action: "POST /api/moderation/reports",
        resource_path: "/api/moderation/reports",
    };
    assert_eq!(
        repository
            .finish_lookup(OwnerReceiptFinishLookupDto {
                claim: &claim,
                identity: identity(),
                suspended_reason: Some("INVALID_REASON"),
                retry_seconds: 30.0
            })
            .await,
        Err(JournalWriteError::Permanent)
    );
    let still_claimed: Option<Uuid> = sqlx::query_scalar(
        "select owner_receipt_claim_id from gateway_request_journal where id=$1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(still_claimed, Some(claim.token));
    assert!(
        repository
            .finish_lookup(OwnerReceiptFinishLookupDto {
                claim: &claim,
                identity: identity(),
                suspended_reason: None,
                retry_seconds: 30.0
            })
            .await?
    );
    assert!(repository
        .claim_next(OwnerReceiptClaimRequestDto {
            lease_seconds: 30.0
        })
        .await?
        .is_none());
    sqlx::query("update gateway_request_journal set owner_receipt_next_check_at=now()-interval '1 second' where id=$1").bind(id).execute(&app.pool).await?;
    let fresh = repository
        .claim_next(OwnerReceiptClaimRequestDto {
            lease_seconds: 30.0,
        })
        .await?
        .expect("fresh claim");
    assert_ne!(claim.token, fresh.token);
    assert_eq!(fresh.attempt, 2);
    let commit = |claim| OwnerReceiptCommitDto {
        claim,
        identity: identity(),
        scope: Some("moderation.v1.CreateReport"),
        receipt_state: Some("REPLAYABLE"),
        committed_at: Some(chrono::Utc::now()),
    };
    assert!(
        !repository.record_commit(commit(&claim)).await?,
        "an expired claim cannot publish owner evidence"
    );
    assert!(repository.record_commit(commit(&fresh)).await?);
    assert!(!repository.record_commit(commit(&fresh)).await?);
    let audits: i64 = sqlx::query_scalar("select count(*) from admin_audit_events where resource_id=$1 and action='OPERATION_RECONCILED_FROM_OWNER_RECEIPT'").bind(id.to_string()).fetch_one(&app.pool).await?;
    assert_eq!(audits, 1);
    app.shutdown().await?;
    assert!(matches!(
        repository
            .claim_next(OwnerReceiptClaimRequestDto {
                lease_seconds: 30.0
            })
            .await,
        Err(JournalWriteError::Permanent)
    ));
    Ok(())
}
