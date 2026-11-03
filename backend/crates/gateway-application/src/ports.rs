use crate::dto::auth_dto::{
    AuthResponse, DeadLetteredUserDeletionListResponse, DeleteAccountResponse,
    EmailAvailabilityResponse, EmailCheckRequest, EmailSignupRequest, EmailVerifyRequest,
    LinkedProvidersResponse, LoginRequest, OAuthLinkRequest, PasswordResetConfirmRequest,
    PasswordResetRequest, PasswordResetResponse, RetryUserDeletionRequestDto, SignupResponse,
    UserDeletionSagaResponse,
};
use crate::dto::facade_dto::IdentityProviderDto;
use crate::dto::friend_dto::{
    FollowActionResponse, FollowCountsResponse, FollowListResponse, FollowRequestPageResponse,
    FollowRequestResponse, FollowStatusResponse, PeopleRecommendationEventRequest,
    PeopleRecommendationPageResponse, RecommendationEventResponse, RelationshipPolicyResponse,
};
use crate::dto::issuer_dto::{SessionAuthorityRequestDto, SessionStateDto, SigningKeysDto};
use crate::dto::moderation_repository_dto::{
    ContentModerationDecisionBacklogDto, ContentModerationDecisionCursorDto,
    ContentModerationDecisionOutboxEntryDto, ContentModerationDecisionStageDto,
    ModerationDecisionDto, ModerationDecisionTypeDto, ModerationReportDto,
    PendingContentModerationDecisionDto,
};
use crate::dto::notification_dto::{
    NotificationFeedResponse, NotificationMarkAllReadResponse, NotificationResponse,
};
use crate::dto::post_dto::{
    CommentCreateRequest, CommentResponse, DeletePostResponse, FeedRecommendationEventRequest,
    FeedRecommendationEventResponse, FeedResponse, MutationReceiptStatusDto, PostCreateRequest,
    PostDetailResponse, PostLikeResponse, PostResponse, RankedFeedResponse,
    ResolveDeadLetterReplayRequestDto, ResolveDeadLetterReplayResponseDto,
};
use crate::dto::profile_dto::{
    AvatarContent, DeleteProfileResponse, ProfileBatchResponse, ProfilePrivacyUpdateRequest,
    ProfileResponse, ProfileSearchResponse, ProfileUpdateRequest,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Runtime values supplied by the composition adapter, never by application defaults.
/// UTC is for persisted timestamps and cursor expiry. Monotonic time is for
/// elapsed budgets and session freshness; implementations must not substitute
/// wall time for it. Production identifiers retain UUID v4 generation.
pub trait GatewayRuntimePort: Send + Sync {
    fn utc_now(&self) -> DateTime<Utc>;
    fn monotonic_now(&self) -> Instant;
    fn new_id(&self) -> uuid::Uuid;
}

/// Best-effort router audit persistence; durable admission uses separate ports.
#[async_trait]
pub trait LegacyAuditRepositoryPort: Send + Sync {
    async fn record(
        &self,
        command: crate::dto::legacy_audit_dto::LegacyAuditCommandDto,
    ) -> Result<(), crate::legacy_audit_error::LegacyAuditStorageError>;
}

/// Private journal records cross this boundary; public HTTP views do not.
#[async_trait]
pub trait OperationJournalRepositoryPort: Send + Sync {
    async fn operation_status(
        &self,
        query: crate::dto::journal_repository_dto::OperationJournalLookupDto,
    ) -> crate::journal_repository_error::JournalRepositoryResult<
        Option<crate::dto::journal_repository_dto::OperationJournalRecordDto>,
    >;
    async fn caller_operation_status(
        &self,
        query: crate::dto::journal_repository_dto::CallerOperationJournalLookupDto,
    ) -> crate::journal_repository_error::JournalRepositoryResult<
        Option<crate::dto::journal_repository_dto::CallerOperationJournalRecordDto>,
    >;
    async fn reconciliation_backlog(
        &self,
    ) -> crate::journal_repository_error::JournalRepositoryResult<
        crate::dto::journal_repository_dto::ReconciliationBacklog,
    >;
    async fn record_manual_resolution(
        &self,
        command: crate::dto::journal_repository_dto::ManualResolutionJournalCommandDto,
    ) -> crate::journal_repository_error::JournalRepositoryResult<
        crate::dto::journal_repository_dto::ManualResolutionJournalWriteDto,
    >;
}

#[derive(Debug, Clone)]
pub struct TelemetryRecord {
    pub id: String,
    pub correlation_id: String,
    pub route_key: String,
    pub client_profile_id: Option<String>,
    pub outcome: String,
    pub latency_ms: i64,
    pub error_code: Option<String>,
    pub downstream_latencies: HashMap<String, i64>,
    pub recorded_at: DateTime<Utc>,
}

/// The issuer's published verification keys.
///
/// Implemented over the authenticated auth channel, so the keys this Gateway
/// trusts arrive the same way as every other internal call: from a peer whose
/// identity was verified, never from a location named by a token.
#[async_trait]
pub trait SigningKeySourcePort: Send + Sync {
    /// The issuer's current JWK Set document.
    async fn signing_keys(&self) -> crate::client_error::GatewayClientResult<SigningKeysDto>;
}

/// The issuer's current view of a login session.
///
/// Consulted before high-risk actions, because an access token cannot show that
/// its session was logged out or its roles removed after it was issued.
#[async_trait]
pub trait SessionAuthorityPort: Send + Sync {
    async fn session_state(
        &self,
        request: &SessionAuthorityRequestDto,
    ) -> crate::client_error::GatewayClientResult<SessionStateDto>;
}

#[async_trait]
pub trait AuthClientPort: Send + Sync {
    async fn signup(
        &self,
        request: EmailSignupRequest,
    ) -> crate::client_error::GatewayClientResult<SignupResponse>;
    async fn verify(
        &self,
        request: EmailVerifyRequest,
    ) -> crate::client_error::GatewayClientResult<SignupResponse>;
    async fn check_email(
        &self,
        request: EmailCheckRequest,
    ) -> crate::client_error::GatewayClientResult<EmailAvailabilityResponse>;
    async fn request_password_reset(
        &self,
        request: PasswordResetRequest,
    ) -> crate::client_error::GatewayClientResult<PasswordResetResponse>;
    async fn confirm_password_reset(
        &self,
        request: PasswordResetConfirmRequest,
    ) -> crate::client_error::GatewayClientResult<PasswordResetResponse>;
    async fn login(
        &self,
        request: LoginRequest,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::client_error::GatewayClientResult<AuthResponse>;
    async fn refresh(
        &self,
        refresh_token: String,
    ) -> crate::client_error::GatewayClientResult<AuthResponse>;
    async fn logout(&self, refresh_token: String) -> crate::client_error::GatewayClientResult<()>;
    async fn authorize(
        &self,
        provider: IdentityProviderDto,
    ) -> crate::client_error::GatewayClientResult<crate::dto::auth_dto::OAuthAuthorizationDto>;
    async fn oauth_callback(
        &self,
        provider: IdentityProviderDto,
        code: String,
        state: String,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::client_error::GatewayClientResult<AuthResponse>;
    async fn link_provider(
        &self,
        provider: IdentityProviderDto,
        request: OAuthLinkRequest,
        user_id: String,
    ) -> crate::client_error::GatewayClientResult<LinkedProvidersResponse>;
    async fn delete_account(
        &self,
        _user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> crate::client_error::GatewayClientResult<DeleteAccountResponse>;
    async fn get_user_deletion(
        &self,
        _user_id: String,
    ) -> crate::client_error::GatewayClientResult<Option<UserDeletionSagaResponse>>;
    async fn get_user_deletion_by_saga_id(
        &self,
        _saga_id: String,
    ) -> crate::client_error::GatewayClientResult<Option<UserDeletionSagaResponse>>;
    async fn list_dead_lettered_user_deletions(
        &self,
        _limit: i32,
        _after_updated_at: Option<DateTime<Utc>>,
        _after_saga_id: Option<uuid::Uuid>,
    ) -> crate::client_error::GatewayClientResult<DeadLetteredUserDeletionListResponse>;
    async fn retry_user_deletion(
        &self,
        _request: RetryUserDeletionRequestDto,
    ) -> crate::client_error::GatewayClientResult<UserDeletionSagaResponse>;
    /// The issuer's current view of one session. A client that cannot answer
    /// reports an error, which refuses the high-risk action that asked.
    async fn session_state(
        &self,
        request: &SessionAuthorityRequestDto,
    ) -> crate::client_error::GatewayClientResult<SessionStateDto>;
}

#[async_trait]
pub trait ProfileClientPort: Send + Sync {
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> crate::client_error::GatewayClientResult<Option<MutationReceiptStatusDto>>;
    async fn get_profile(
        &self,
        user_id: String,
    ) -> crate::client_error::GatewayClientResult<ProfileResponse>;
    async fn get_profile_by_username(
        &self,
        username: String,
    ) -> crate::client_error::GatewayClientResult<ProfileResponse>;
    async fn get_profiles(
        &self,
        user_ids: Vec<String>,
    ) -> crate::client_error::GatewayClientResult<ProfileBatchResponse>;
    async fn search_profiles(
        &self,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::client_error::GatewayClientResult<ProfileSearchResponse>;
    async fn upload_avatar(
        &self,
        user_id: String,
        content: Vec<u8>,
        content_type: String,
        filename: String,
    ) -> crate::client_error::GatewayClientResult<String>;
    async fn get_avatar(
        &self,
        user_id: String,
    ) -> crate::client_error::GatewayClientResult<AvatarContent>;
    async fn update_profile(
        &self,
        user_id: String,
        request: ProfileUpdateRequest,
    ) -> crate::client_error::GatewayClientResult<ProfileResponse>;
    async fn update_profile_privacy(
        &self,
        _user_id: String,
        _request: ProfilePrivacyUpdateRequest,
    ) -> crate::client_error::GatewayClientResult<ProfileResponse>;
    async fn delete_profile(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::client_error::GatewayClientResult<DeleteProfileResponse>;
}

#[async_trait]
pub trait FriendClientPort: Send + Sync {
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> crate::client_error::GatewayClientResult<Option<MutationReceiptStatusDto>>;
    async fn follow(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::client_error::GatewayClientResult<FollowActionResponse>;
    async fn unfollow(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::client_error::GatewayClientResult<FollowActionResponse>;
    async fn list_followers(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> crate::client_error::GatewayClientResult<FollowListResponse>;
    async fn list_following(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: Option<i32>,
    ) -> crate::client_error::GatewayClientResult<FollowListResponse>;
    async fn get_counts(
        &self,
        user_id: String,
    ) -> crate::client_error::GatewayClientResult<FollowCountsResponse>;
    async fn get_counts_batch(
        &self,
        user_ids: Vec<String>,
    ) -> crate::client_error::GatewayClientResult<HashMap<String, FollowCountsResponse>>;
    async fn check_following(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> crate::client_error::GatewayClientResult<FollowStatusResponse>;
    async fn accept_follow_request(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> crate::client_error::GatewayClientResult<FollowActionResponse>;
    async fn reject_follow_request(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> crate::client_error::GatewayClientResult<FollowRequestResponse>;
    async fn cancel_follow_request(
        &self,
        _request_id: String,
        _actor_id: String,
    ) -> crate::client_error::GatewayClientResult<FollowRequestResponse>;
    async fn list_incoming_follow_requests(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> crate::client_error::GatewayClientResult<FollowRequestPageResponse>;
    async fn list_outgoing_follow_requests(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> crate::client_error::GatewayClientResult<FollowRequestPageResponse>;
    async fn block_user(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> crate::client_error::GatewayClientResult<RelationshipPolicyResponse>;
    async fn unblock_user(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> crate::client_error::GatewayClientResult<RelationshipPolicyResponse>;
    async fn mute_user(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> crate::client_error::GatewayClientResult<RelationshipPolicyResponse>;
    async fn unmute_user(
        &self,
        _viewer_id: String,
        _target_id: String,
    ) -> crate::client_error::GatewayClientResult<RelationshipPolicyResponse>;
    async fn get_relationship(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> crate::client_error::GatewayClientResult<RelationshipPolicyResponse>;
    async fn get_people_recommendations(
        &self,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
        _request_id: String,
    ) -> crate::client_error::GatewayClientResult<PeopleRecommendationPageResponse>;
    async fn record_people_recommendation_event(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: PeopleRecommendationEventRequest,
    ) -> crate::client_error::GatewayClientResult<RecommendationEventResponse>;
    async fn get_relationships_batch(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> crate::client_error::GatewayClientResult<HashMap<String, RelationshipPolicyResponse>>;
}

#[async_trait]
pub trait NotificationClientPort: Send + Sync {
    async fn list_notifications(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::client_error::GatewayClientResult<NotificationFeedResponse>;
    async fn mark_read(
        &self,
        user_id: String,
        notification_id: String,
    ) -> crate::client_error::GatewayClientResult<NotificationResponse>;
    async fn mark_all_read(
        &self,
        user_id: String,
    ) -> crate::client_error::GatewayClientResult<NotificationMarkAllReadResponse>;
}

#[async_trait]
pub trait PostClientPort: Send + Sync {
    /// Re-authorize a bounded notification page in one owner call. The returned
    /// map must contain an explicit decision for every requested ID. The adapter
    /// must abandon lookup within max_duration; absence is never permission.
    async fn notification_posts_visible(
        &self,
        _viewer_id: String,
        _post_ids: Vec<String>,
        _max_duration: std::time::Duration,
    ) -> crate::post_client_error::PostClientResult<HashMap<String, bool>>;

    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> crate::post_client_error::PostClientResult<Option<MutationReceiptStatusDto>>;
    async fn get_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::post_client_error::PostClientResult<FeedResponse>;
    async fn get_ranked_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        _session_id: Option<String>,
    ) -> crate::post_client_error::PostClientResult<RankedFeedResponse>;
    async fn record_feed_recommendation_event(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: FeedRecommendationEventRequest,
    ) -> crate::post_client_error::PostClientResult<FeedRecommendationEventResponse>;
    async fn get_author_posts(
        &self,
        author_id: String,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::post_client_error::PostClientResult<FeedResponse>;

    /// Post counts for a set of authors.
    ///
    /// Returns a map keyed by author. An author absent from the map was not
    /// counted: the caller must surface that as unavailable rather than as zero,
    /// because a zero is a real observation and a missing entry is not.
    async fn get_author_post_counts(
        &self,
        author_ids: Vec<String>,
    ) -> crate::post_client_error::PostClientResult<HashMap<String, i64>>;
    async fn search_posts(
        &self,
        viewer_id: String,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::post_client_error::PostClientResult<FeedResponse>;
    async fn get_trending(
        &self,
        viewer_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::post_client_error::PostClientResult<FeedResponse>;
    async fn get_post(
        &self,
        viewer_id: String,
        post_id: String,
    ) -> crate::post_client_error::PostClientResult<PostDetailResponse>;
    async fn create_post(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> crate::post_client_error::PostClientResult<PostResponse>;
    async fn delete_post(
        &self,
        user_id: String,
        post_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::post_client_error::PostClientResult<DeletePostResponse>;
    async fn create_comment(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> crate::post_client_error::PostClientResult<CommentResponse>;
    async fn like_post(
        &self,
        user_id: String,
        post_id: String,
    ) -> crate::post_client_error::PostClientResult<PostLikeResponse>;
    async fn unlike_post(
        &self,
        user_id: String,
        post_id: String,
    ) -> crate::post_client_error::PostClientResult<PostLikeResponse>;
    async fn apply_content_moderation(
        &self,
        _report_id: String,
        _target_type: String,
        _target_id: String,
        _actor_id: String,
        _action: String,
    ) -> crate::post_client_error::PostClientResult<bool>;
    async fn resolve_dead_letter_replay(
        &self,
        _request: ResolveDeadLetterReplayRequestDto,
    ) -> crate::post_client_error::PostClientResult<ResolveDeadLetterReplayResponseDto>;
}

/// Time and identity values sampled by moderation application decisions.
/// Persistence lease clocks remain the repository adapter's responsibility.
pub trait ModerationRuntimePort: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn new_id(&self) -> uuid::Uuid;
}

#[async_trait]
pub trait ModerationRepositoryPort: Send + Sync {
    async fn create_report(
        &self,
        report: ModerationReportDto,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<ModerationReportDto>;
    async fn create_report_with_mutation(
        &self,
        report: ModerationReportDto,
        context: Option<crate::dto::moderation_dto::ModerationMutationContextDto>,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<ModerationReportDto>;
    async fn find_report(
        &self,
        report_id: uuid::Uuid,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<Option<ModerationReportDto>>;
    async fn list_by_reporter(
        &self,
        reporter_id: uuid::Uuid,
        limit: i32,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<Vec<ModerationReportDto>>;
    async fn list_open(
        &self,
        limit: i32,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<Vec<ModerationReportDto>>;
    async fn content_decision_backlog(
        &self,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<
        ContentModerationDecisionBacklogDto,
    >;
    async fn list_content_decisions(
        &self,
        _limit: i32,
        _after: Option<ContentModerationDecisionCursorDto>,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<
        Vec<ContentModerationDecisionOutboxEntryDto>,
    >;
    async fn record_decision(
        &self,
        report_id: uuid::Uuid,
        actor_id: uuid::Uuid,
        decision_type: ModerationDecisionTypeDto,
        note: &str,
        restriction_expires_at: Option<DateTime<Utc>>,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<(
        ModerationReportDto,
        ModerationDecisionDto,
    )>;
    async fn begin_content_decision(
        &self,
        _report_id: uuid::Uuid,
        _actor_id: uuid::Uuid,
        _decision_type: ModerationDecisionTypeDto,
        _note: &str,
        _lease_owner: uuid::Uuid,
        _lease_for: Duration,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<
        ContentModerationDecisionStageDto,
    >;
    async fn claim_next_content_decision(
        &self,
        _lease_owner: uuid::Uuid,
        _lease_for: Duration,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<
        Option<PendingContentModerationDecisionDto>,
    >;
    async fn complete_content_decision(
        &self,
        _pending: &PendingContentModerationDecisionDto,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<(
        ModerationReportDto,
        ModerationDecisionDto,
    )>;
    async fn retry_content_decision(
        &self,
        _pending: &PendingContentModerationDecisionDto,
        _retry_after: Duration,
        _error_code: &str,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<bool>;
    async fn dead_letter_content_decision(
        &self,
        _pending: &PendingContentModerationDecisionDto,
        _error_code: &str,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<bool>;
    async fn replay_dead_lettered_content_decision(
        &self,
        _report_id: uuid::Uuid,
        _resolution_id: uuid::Uuid,
        _actor_id: uuid::Uuid,
        _reason: &str,
        _evidence_reference: &str,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<bool>;
    async fn mark_content_target_unavailable(
        &self,
        _pending: &PendingContentModerationDecisionDto,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<bool>;
    async fn is_user_restricted(
        &self,
        user_id: uuid::Uuid,
        now: DateTime<Utc>,
    ) -> crate::moderation_repository_error::ModerationRepositoryResult<bool>;
}

#[async_trait]
pub trait RouteContractRepositoryPort: Send + Sync {
    async fn find_all(
        &self,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Vec<crate::dto::infrastructure_dto::RouteContractDto>,
    >;
    async fn find_by_id(
        &self,
        contract_id: &str,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::dto::infrastructure_dto::RouteContractDto>,
    >;
    async fn find_version(
        &self,
        contract_id: &str,
        version: &str,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::dto::infrastructure_dto::RouteContractVersionDto>,
    >;
    async fn save(
        &self,
        contract: crate::dto::infrastructure_dto::RouteContractDto,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        crate::dto::infrastructure_dto::RouteContractDto,
    >;
    async fn save_version(
        &self,
        contract_id: &str,
        version: crate::dto::infrastructure_dto::RouteContractVersionDto,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        crate::dto::infrastructure_dto::RouteContractVersionDto,
    >;
}

#[async_trait]
pub trait PolicyRepositoryPort: Send + Sync {
    async fn find_all(
        &self,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Vec<crate::dto::infrastructure_dto::PolicySetDto>,
    >;
    async fn find_by_id(
        &self,
        policy_id: &str,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::dto::infrastructure_dto::PolicySetDto>,
    >;
    async fn save(
        &self,
        policy: crate::dto::infrastructure_dto::PolicySetDto,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        crate::dto::infrastructure_dto::PolicySetDto,
    >;
}

#[async_trait]
pub trait RolloutRepositoryPort: Send + Sync {
    async fn find_by_id(
        &self,
        rollout_id: &str,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::dto::infrastructure_dto::RolloutPolicyDto>,
    >;
    async fn save(
        &self,
        rollout: crate::dto::infrastructure_dto::RolloutPolicyDto,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        crate::dto::infrastructure_dto::RolloutPolicyDto,
    >;
}

#[async_trait]
pub trait ClientProfileRepositoryPort: Send + Sync {
    async fn find_all(
        &self,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Vec<crate::dto::infrastructure_dto::ClientProfileDto>,
    >;
    async fn find_by_id(
        &self,
        client_id: &str,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::dto::infrastructure_dto::ClientProfileDto>,
    >;
    async fn save(
        &self,
        profile: crate::dto::infrastructure_dto::ClientProfileDto,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        crate::dto::infrastructure_dto::ClientProfileDto,
    >;
}

#[async_trait]
pub trait AggregationRecipeRepositoryPort: Send + Sync {
    async fn find_all(
        &self,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Vec<crate::dto::infrastructure_dto::AggregationRecipeDto>,
    >;
    async fn find_by_route_key(
        &self,
        route_key: &str,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::dto::infrastructure_dto::AggregationRecipeDto>,
    >;
}

#[async_trait]
pub trait ResponseCachePort: Send + Sync {
    async fn get(
        &self,
        cache_key: &str,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        Option<crate::dto::cache_dto::CachePayloadDto>,
    >;
    async fn put(
        &self,
        cache_key: &str,
        payload: crate::dto::cache_dto::CachePayloadDto,
        ttl: Duration,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<()>;
}

#[async_trait]
pub trait TelemetryRepositoryPort: Send + Sync {
    async fn record(
        &self,
        record: TelemetryRecord,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<()>;
    async fn summary(
        &self,
    ) -> crate::infrastructure_error::GatewayInfrastructureResult<
        crate::dto::infrastructure_dto::TelemetrySummaryDto,
    >;
}
pub use crate::journal_write_ports::*;
pub use crate::queue_operation_ports::*;
