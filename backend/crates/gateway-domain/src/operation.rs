use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManualResolutionDisposition {
    Committed,
    NotCommitted,
    Unresolved,
}

impl ManualResolutionDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Committed => "COMMITTED",
            Self::NotCommitted => "NOT_COMMITTED",
            Self::Unresolved => "UNRESOLVED",
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "COMMITTED" => Ok(Self::Committed),
            "NOT_COMMITTED" => Ok(Self::NotCommitted),
            "UNRESOLVED" => Ok(Self::Unresolved),
            _ => anyhow::bail!("unknown manual operation resolution disposition"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManualResolutionEvidenceKind {
    DatabaseAudit,
    OwnerConfirmation,
    ServiceLog,
    IncidentRecord,
    Other,
}

impl ManualResolutionEvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DatabaseAudit => "DATABASE_AUDIT",
            Self::OwnerConfirmation => "OWNER_CONFIRMATION",
            Self::ServiceLog => "SERVICE_LOG",
            Self::IncidentRecord => "INCIDENT_RECORD",
            Self::Other => "OTHER",
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "DATABASE_AUDIT" => Ok(Self::DatabaseAudit),
            "OWNER_CONFIRMATION" => Ok(Self::OwnerConfirmation),
            "SERVICE_LOG" => Ok(Self::ServiceLog),
            "INCIDENT_RECORD" => Ok(Self::IncidentRecord),
            "OTHER" => Ok(Self::Other),
            _ => anyhow::bail!("unknown manual operation evidence kind"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManualOperationResolution {
    pub resolution_id: Uuid,
    pub operation_id: Uuid,
    pub revision: i32,
    pub supersedes_resolution_id: Option<Uuid>,
    pub disposition: ManualResolutionDisposition,
    pub evidence_kind: ManualResolutionEvidenceKind,
    pub evidence_reference: String,
    pub reason: String,
    pub reviewer_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OwnerReceiptLookupStatus {
    #[default]
    NotApplicable,
    Found,
    NotFound,
    Unavailable,
    Mismatch,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OwnerReceiptStatus {
    pub lookup_status: OwnerReceiptLookupStatus,
    pub receipt_state: Option<String>,
    pub scope: Option<String>,
    pub committed_at: Option<DateTime<Utc>>,
    pub replay_until: Option<DateTime<Utc>>,
}

/// Best currently supported resolution of a durable Gateway operation.
///
/// This is separate from the journal lifecycle `state`: `COMPLETED` only means
/// the Gateway finished handling a reply, while this outcome is backed by the
/// durable response or explicit reconciliation evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationOutcome {
    Pending,
    Committed,
    RejectedBeforeCommit,
    Unknown,
}

impl OperationOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Committed => "COMMITTED",
            Self::RejectedBeforeCommit => "REJECTED_BEFORE_COMMIT",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "COMMITTED" => Ok(Self::Committed),
            "REJECTED_BEFORE_COMMIT" => Ok(Self::RejectedBeforeCommit),
            "UNKNOWN" => Ok(Self::Unknown),
            _ => anyhow::bail!("unknown operation outcome"),
        }
    }

    pub fn audit_result(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Committed => "SUCCESS",
            Self::RejectedBeforeCommit => "FAILURE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// Fixed evidence labels persisted with the Gateway's outcome projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationOutcomeEvidence {
    IntentRecorded,
    QueueAccepted,
    SuccessResponse,
    PreDispatchRejection,
    ResponseStatusOnly,
    OwnerReplayReceipt,
    ManualResolution,
    StaleOperation,
    LegacyUnresolved,
}

impl OperationOutcomeEvidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IntentRecorded => "INTENT_RECORDED",
            Self::QueueAccepted => "QUEUE_ACCEPTED",
            Self::SuccessResponse => "SUCCESS_RESPONSE",
            Self::PreDispatchRejection => "PRE_DISPATCH_REJECTION",
            Self::ResponseStatusOnly => "RESPONSE_STATUS_ONLY",
            Self::OwnerReplayReceipt => "OWNER_REPLAY_RECEIPT",
            Self::ManualResolution => "MANUAL_RESOLUTION",
            Self::StaleOperation => "STALE_OPERATION",
            Self::LegacyUnresolved => "LEGACY_UNRESOLVED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationOutcomeDecision {
    pub outcome: OperationOutcome,
    pub evidence: OperationOutcomeEvidence,
}

impl OperationOutcomeDecision {
    /// Status is intentionally reduced to evidence categories here, keeping
    /// transport types outside the domain. `definitely_not_dispatched` is only
    /// supplied for an exact owner-backed route with its tracking context.
    pub fn from_response(
        successful: bool,
        accepted: bool,
        definitely_not_dispatched: bool,
    ) -> Self {
        if successful && !accepted {
            return Self {
                outcome: OperationOutcome::Committed,
                evidence: OperationOutcomeEvidence::SuccessResponse,
            };
        }
        if !accepted && definitely_not_dispatched {
            return Self {
                outcome: OperationOutcome::RejectedBeforeCommit,
                evidence: OperationOutcomeEvidence::PreDispatchRejection,
            };
        }
        Self {
            outcome: OperationOutcome::Unknown,
            evidence: OperationOutcomeEvidence::ResponseStatusOnly,
        }
    }
}

/// What a caller may learn about one durable operation.
///
/// A durable write gives its caller an operation id, and this is the record that
/// id names. The journal already stores the intent, the outcome, and the replay
/// body; without a caller-visible view of them, a timed-out caller holds an
/// identifier it cannot act on and its only remaining paths are retrying blindly
/// or asking an operator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    pub operation_id: Uuid,
    /// One of `IN_PROGRESS`, `QUEUED`, `COMPLETED`, or `UNKNOWN`. A separate manual
    /// disposition may be present while the immutable Gateway state is `UNKNOWN`.
    ///
    /// `state` is the Gateway journal lifecycle. Use `outcome` to distinguish a
    /// successful result, a proven pre-commit rejection, and an unresolved one.
    pub state: String,
    pub outcome: OperationOutcome,
    pub action: String,
    pub resource_path: String,
    /// The recorded downstream status, present only once the operation finished.
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Whether the recorded response is still replayable.
    ///
    /// Replay bodies expire before their rows do, so a completed operation can be
    /// old enough that its outcome is known while its response is gone. Reporting
    /// the distinction keeps a caller from reading an absent body as an absent
    /// outcome.
    pub replayable: bool,
    /// Owner-side receipt evidence. A missing receipt is not proof that the
    /// write did not commit; callers must not use this status to retry blindly.
    #[serde(default)]
    pub owner_receipt: OwnerReceiptStatus,
    /// Latest human decision. The underlying Gateway journal state remains
    /// unchanged; this metadata is an audited disposition, not a replay token.
    #[serde(default)]
    pub manual_resolution: Option<ManualOperationResolution>,
    /// Internal correlation field used only to verify owner receipt identity.
    #[serde(skip)]
    pub(crate) actor_id: Option<String>,
}

/// Privacy-minimal status visible to the authenticated owner of an operation.
///
/// Unlike [`OperationStatus`], this deliberately excludes request paths,
/// owner-receipt details, and operator reconciliation evidence. The response
/// body is never returned by this lookup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CallerOperationStatus {
    pub operation_id: Uuid,
    pub state: String,
    pub outcome: OperationOutcome,
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub replayable: bool,
}

/// Typed persistence projection; private correlation identity is supplied separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationStatusFields {
    pub operation_id: Uuid,
    pub state: String,
    pub outcome: OperationOutcome,
    pub action: String,
    pub resource_path: String,
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub replayable: bool,
    pub owner_receipt: OwnerReceiptStatus,
    pub manual_resolution: Option<ManualOperationResolution>,
}

impl OperationStatus {
    pub fn from_persisted(fields: OperationStatusFields, actor_id: Option<String>) -> Self {
        Self {
            operation_id: fields.operation_id,
            state: fields.state,
            outcome: fields.outcome,
            action: fields.action,
            resource_path: fields.resource_path,
            response_status: fields.response_status,
            created_at: fields.created_at,
            finished_at: fields.finished_at,
            replayable: fields.replayable,
            owner_receipt: fields.owner_receipt,
            manual_resolution: fields.manual_resolution,
            actor_id,
        }
    }

    /// Internal receipt correlation only; this is never part of serialized status.
    pub fn actor_identity(&self) -> Option<&str> {
        self.actor_id.as_deref()
    }

    /// Return the expected versioned owner RPC scope for the explicitly
    /// supported PostService and FriendService idempotency routes.
    pub fn owner_mutation_scope(&self) -> Option<&'static str> {
        owner_mutation_scope(&self.action, &self.resource_path)
    }
}

/// Return the versioned owner scope for an exact, idempotency-enabled route.
pub fn owner_mutation_scope(action: &str, resource_path: &str) -> Option<&'static str> {
    let (method, action_path) = action.split_once(' ')?;
    if action_path != resource_path {
        return None;
    }
    match (method, resource_path) {
        ("POST", "/api/posts") => Some("post.v1.CreatePost"),
        ("DELETE", path) if uuid_resource(path, "/api/posts/", "") => Some("post.v1.DeletePost"),
        ("POST", path) if uuid_resource(path, "/api/posts/", "/comments") => {
            Some("post.v1.CreateComment")
        }
        ("POST", path) if uuid_resource(path, "/api/posts/", "/likes") => Some("post.v1.LikePost"),
        ("DELETE", path) if uuid_resource(path, "/api/posts/", "/likes") => {
            Some("post.v1.UnlikePost")
        }
        ("POST", "/api/follows") => Some("friend.v1.FollowUser"),
        ("DELETE", path) if uuid_resource(path, "/api/follows/", "") => {
            Some("friend.v1.UnfollowUser")
        }
        ("PUT", "/api/profile/privacy") => Some("profile.v1.UpdateProfilePrivacy"),
        ("PUT", path) if uuid_resource(path, "/api/profile/", "") => {
            Some("profile.v1.UpdateProfile")
        }
        ("POST", "/api/moderation/reports") => Some("gateway.v1.CreateModerationReport"),
        _ => None,
    }
}

/// Only mutations whose effect and deduplication receipt commit atomically and
/// whose execution can return that receipt on replay may be driven by the
/// restart-safe at-least-once worker. Gateway owns moderation-report writes.
pub fn supports_restart_safe_async_execution(action: &str, resource_path: &str) -> bool {
    matches!(
        owner_mutation_scope(action, resource_path),
        Some(
            "post.v1.CreatePost"
                | "post.v1.DeletePost"
                | "post.v1.CreateComment"
                | "post.v1.LikePost"
                | "post.v1.UnlikePost"
                | "friend.v1.FollowUser"
                | "friend.v1.UnfollowUser"
                | "profile.v1.UpdateProfilePrivacy"
                | "profile.v1.UpdateProfile"
                | "gateway.v1.CreateModerationReport"
        )
    )
}

fn uuid_resource(path: &str, prefix: &str, suffix: &str) -> bool {
    path.strip_prefix(prefix)
        .and_then(|resource| resource.strip_suffix(suffix))
        .is_some_and(|resource| Uuid::parse_str(resource).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_projection_preserves_status_and_private_actor_identity() {
        let original = operation("POST", "/api/follows");
        let actor = original.actor_id.clone();
        let fields = OperationStatusFields {
            operation_id: original.operation_id,
            state: original.state.clone(),
            outcome: original.outcome,
            action: original.action.clone(),
            resource_path: original.resource_path.clone(),
            response_status: original.response_status,
            created_at: original.created_at,
            finished_at: original.finished_at,
            replayable: original.replayable,
            owner_receipt: original.owner_receipt.clone(),
            manual_resolution: original.manual_resolution.clone(),
        };
        let reconstructed = OperationStatus::from_persisted(fields, actor.clone());
        assert_eq!(original, reconstructed);
        assert_eq!(reconstructed.actor_identity(), actor.as_deref());
        let payload = serde_json::to_value(reconstructed).expect("status JSON");
        assert!(payload.get("actorId").is_none());
        assert!(payload.get("actor_id").is_none());
        assert!(!payload
            .to_string()
            .contains(actor.as_deref().expect("actor")));
    }

    fn operation(method: &str, path: &str) -> OperationStatus {
        OperationStatus {
            operation_id: Uuid::new_v4(),
            state: "UNKNOWN".into(),
            outcome: OperationOutcome::Unknown,
            action: format!("{method} {path}"),
            resource_path: path.into(),
            response_status: None,
            created_at: Utc::now(),
            finished_at: None,
            replayable: false,
            owner_receipt: OwnerReceiptStatus::default(),
            manual_resolution: None,
            actor_id: Some(Uuid::new_v4().to_string()),
        }
    }

    #[test]
    fn owner_scope_matches_only_the_allowlisted_exact_mutation_routes() {
        let post_id = Uuid::new_v4();
        for (method, path, scope) in [
            ("POST", "/api/posts".to_owned(), "post.v1.CreatePost"),
            (
                "DELETE",
                format!("/api/posts/{post_id}"),
                "post.v1.DeletePost",
            ),
            (
                "POST",
                format!("/api/posts/{post_id}/comments"),
                "post.v1.CreateComment",
            ),
            (
                "POST",
                format!("/api/posts/{post_id}/likes"),
                "post.v1.LikePost",
            ),
            (
                "DELETE",
                format!("/api/posts/{post_id}/likes"),
                "post.v1.UnlikePost",
            ),
            ("POST", "/api/follows".to_owned(), "friend.v1.FollowUser"),
            (
                "DELETE",
                format!("/api/follows/{post_id}"),
                "friend.v1.UnfollowUser",
            ),
            (
                "PUT",
                "/api/profile/privacy".to_owned(),
                "profile.v1.UpdateProfilePrivacy",
            ),
            (
                "PUT",
                format!("/api/profile/{post_id}"),
                "profile.v1.UpdateProfile",
            ),
            (
                "POST",
                "/api/moderation/reports".to_owned(),
                "gateway.v1.CreateModerationReport",
            ),
        ] {
            assert_eq!(operation(method, &path).owner_mutation_scope(), Some(scope));
        }
    }

    #[test]
    fn owner_scope_rejects_unrelated_or_malformed_paths() {
        for (method, path) in [
            ("GET", "/api/posts".to_owned()),
            ("POST", "/api/posts/not-a-uuid/likes".to_owned()),
            ("POST", format!("/api/posts/{}/likes/extra", Uuid::new_v4())),
            ("POST", format!("/api/posts/{}/commentary", Uuid::new_v4())),
            ("POST", "/api/follows/extra".to_owned()),
            ("DELETE", "/api/follows/not-a-uuid".to_owned()),
            ("POST", "/api/profile/privacy".to_owned()),
            ("PUT", "/api/profile/batch".to_owned()),
            ("PUT", "/api/profile/not-a-uuid".to_owned()),
            ("GET", "/api/moderation/reports".to_owned()),
            ("POST", "/admin/moderation/reports".to_owned()),
        ] {
            assert_eq!(operation(method, &path).owner_mutation_scope(), None);
        }

        let mut mismatched = operation("POST", "/api/posts");
        mismatched.resource_path = "/api/posts/another-resource".into();
        assert_eq!(mismatched.owner_mutation_scope(), None);
    }

    #[test]
    fn restart_safe_async_allowlist_includes_only_replayable_receipt_backed_routes() {
        let post_id = Uuid::new_v4();
        for (method, path) in [
            ("POST", "/api/posts".to_owned()),
            ("DELETE", format!("/api/posts/{post_id}")),
            ("POST", format!("/api/posts/{post_id}/comments")),
            ("POST", format!("/api/posts/{post_id}/likes")),
            ("DELETE", format!("/api/posts/{post_id}/likes")),
            ("POST", "/api/follows".to_owned()),
            ("DELETE", format!("/api/follows/{post_id}")),
            ("PUT", "/api/profile/privacy".to_owned()),
            ("PUT", format!("/api/profile/{post_id}")),
            ("POST", "/api/moderation/reports".to_owned()),
        ] {
            let action = format!("{method} {path}");
            assert!(supports_restart_safe_async_execution(&action, &path));
        }

        assert!(!supports_restart_safe_async_execution(
            "DELETE /api/profile",
            "/api/profile"
        ));
    }
}
