"""Reusable component schemas for the Gateway contract."""

from __future__ import annotations

from typing import Any

from .common import array, boolean, integer, nullable, number, obj, ref, string




SCHEMAS: dict[str, Any] = {
    "OpenApiDocument": obj(additionalProperties=True),
    "ErrorResponse": obj(
        {
            "code": string(example="GATEWAY_400"),
            "message": string(example="Invalid request"),
            "traceId": nullable(string(format="uuid")),
            "timestamp": nullable(string(format="date-time")),
            "details": nullable(obj(additionalProperties=True)),
        },
        ["code", "message"],
    ),
    "EmailSignupRequest": obj(
        {
            "email": string(format="email", example="user@example.com"),
            "password": string(format="password", minLength=8, maxLength=128, example="correct-horse-battery-staple"),
        },
        ["email", "password"],
    ),
    "EmailVerifyRequest": obj(
        {
            "email": string(format="email", example="user@example.com"),
            "code": string(pattern="^[0-9]{6}$", example="123456"),
        },
        ["email", "code"],
    ),
    "EmailCheckRequest": obj(
        {"email": string(format="email", example="user@example.com")}, ["email"]
    ),
    "LoginRequest": obj(
        {
            "email": string(format="email", example="user@example.com"),
            "password": string(format="password", minLength=1, maxLength=128),
        },
        ["email", "password"],
    ),
    "SignupResponse": obj(
        {
            "userId": string(format="uuid"),
            "status": string(example="PENDING_VERIFICATION"),
        },
        ["userId", "status"],
    ),
    "EmailAvailabilityResponse": obj(
        {
            "email": string(format="email"),
            "available": boolean(),
            "status": string(enum=["AVAILABLE", "TAKEN"]),
        },
        ["email", "available", "status"],
    ),
    "PasswordResetRequest": obj(
        {"email": string(format="email", example="user@example.com")}, ["email"]
    ),
    "PasswordResetConfirmRequest": obj(
        {
            "token": string(minLength=1),
            "newPassword": string(format="password", minLength=8, maxLength=128),
        },
        ["token", "newPassword"],
    ),
    "PasswordResetResponse": obj(
        {"status": string(example="ACCEPTED")}, ["status"]
    ),
    "AuthResponse": obj(
        {
            "userId": string(format="uuid"),
            "accessToken": string(description="RS256 bearer access token."),
            "refreshToken": string(
                maxLength=0,
                example="",
                readOnly=True,
                description="Always an empty string at the public REST edge. The actual refresh token is sent only as an HttpOnly cookie.",
            ),
            "tokenType": string(example="Bearer"),
            "expiresIn": integer(format="int64", minimum=1, description="Access-token lifetime in seconds."),
            "linkedProviders": array(string(enum=["EMAIL", "GOOGLE", "APPLE", "X"])),
        },
        ["userId", "accessToken", "refreshToken", "tokenType", "expiresIn", "linkedProviders"],
    ),
    "OAuthLinkRequest": obj(
        {
            "code": string(minLength=1, description="Provider authorization code."),
            "state": string(minLength=1, description="One-time state matching the OAuth state cookie."),
        },
        ["code", "state"],
    ),
    "LinkedProvidersResponse": obj(
        {
            "userId": string(format="uuid"),
            "providers": array(string(enum=["EMAIL", "GOOGLE", "APPLE", "X"])),
        },
        ["userId", "providers"],
    ),
    "ShadowComparisonResponse": obj(
        {
            "scenarioId": string(),
            "diffCategory": string(enum=["none", "critical"]),
            "sourceStatus": integer(minimum=100, maximum=599),
            "targetStatus": integer(minimum=100, maximum=599),
            "databaseStateMatch": boolean(),
        },
        ["scenarioId", "diffCategory", "sourceStatus", "targetStatus", "databaseStateMatch"],
    ),
    "UserDeletionStep": obj(
        {
            "target": string(enum=["PROFILE", "FRIEND", "POST", "NOTIFICATION"]),
            "status": string(enum=["PENDING", "IN_PROGRESS", "RETRY", "SUCCEEDED", "DEAD_LETTERED"]),
            "attemptCount": integer(minimum=0),
            "nextAttemptAt": string(format="date-time"),
            "lastError": nullable(string()),
            "completedAt": nullable(string(format="date-time")),
        },
        ["target", "status", "attemptCount", "nextAttemptAt", "lastError", "completedAt"],
    ),
    "UserDeletionSaga": obj(
        {
            "id": string(format="uuid"),
            "userId": string(format="uuid"),
            "status": string(enum=["IN_PROGRESS", "COMPLETED", "DEAD_LETTERED"]),
            "retentionUntil": string(format="date-time"),
            "createdAt": string(format="date-time"),
            "updatedAt": string(format="date-time"),
            "completedAt": nullable(string(format="date-time")),
            "steps": array(ref("UserDeletionStep")),
        },
        ["id", "userId", "status", "retentionUntil", "createdAt", "updatedAt", "completedAt", "steps"],
    ),
    "DeadLetteredUserDeletionCursor": obj(
        {
            "updatedAt": string(format="date-time"),
            "sagaId": string(format="uuid"),
        },
        ["updatedAt", "sagaId"],
    ),
    "DeadLetteredUserDeletionListResponse": obj(
        {
            "sagas": array(ref("UserDeletionSaga")),
            "nextCursor": nullable(ref("DeadLetteredUserDeletionCursor")),
        },
        ["sagas", "nextCursor"],
    ),
    "DeleteAccountResponse": obj(
        {
            "userId": string(format="uuid"),
            "deleted": boolean(),
            "sagaId": string(format="uuid"),
            "saga": ref("UserDeletionSaga"),
        },
        ["userId", "deleted", "sagaId", "saga"],
    ),
    "RetryUserDeletionRequest": obj(
        {
            "resolutionId": string(
                format="uuid",
                description="Client-generated idempotency ID. Reuse this exact ID and request values after an uncertain response.",
            ),
            "reason": string(minLength=20, maxLength=1024),
            "evidenceReference": string(
                minLength=3,
                maxLength=256,
                pattern="^[A-Za-z0-9][A-Za-z0-9._:/#-]{2,255}$",
                description="Identifier for a protected incident or change record. The API validates syntax only; do not include payloads or secrets.",
            ),
        },
        ["resolutionId", "reason", "evidenceReference"],
        additionalProperties=False,
    ),
    "UserDeletionRetryAcceptedResponse": obj(
        {
            "resolutionId": string(format="uuid"),
            "accepted": boolean(),
            "saga": ref("UserDeletionSaga"),
        },
        ["resolutionId", "accepted", "saga"],
        additionalProperties=False,
    ),
    "ProfileUpdateRequest": obj(
        {
            "username": string(minLength=3, maxLength=20, pattern="^[a-zA-Z0-9_]+$", example="port_user"),
            "nickname": string(minLength=1, example="Port User"),
            "avatarUrl": string(minLength=1, example="/api/profile/avatar/11111111-1111-1111-1111-111111111111"),
            "bio": string(minLength=1, example="Building a Rust social backend."),
        },
        ["username", "nickname", "avatarUrl", "bio"],
    ),
    "ProfilePrivacyUpdateRequest": obj(
        {"accountVisibility": string(enum=["PUBLIC", "PRIVATE"])}, ["accountVisibility"]
    ),
    "ProfileBatchRequest": obj(
        {"userIds": array(string(format="uuid"), maxItems=100, uniqueItems=True)}, ["userIds"]
    ),
    "Profile": obj(
        {
            "userId": string(format="uuid"),
            "username": string(),
            "nickname": string(),
            "avatarUrl": string(),
            "bio": string(),
            "followerCount": integer(format="int64", minimum=0),
            "followingCount": integer(format="int64", minimum=0),
            "postCount": integer(format="int64", minimum=0),
            "accountVisibility": string(enum=["PUBLIC", "PRIVATE"]),
            "lifecycleStatus": string(example="ACTIVE"),
        },
        ["userId", "username", "nickname", "avatarUrl", "bio", "followerCount", "followingCount", "postCount", "accountVisibility", "lifecycleStatus"],
    ),
    "ProfileBatchResponse": obj({"profiles": array(ref("Profile"))}, ["profiles"]),
    "ProfileSearchResponse": obj(
        {"profiles": array(ref("Profile")), "nextCursor": nullable(string())}, ["profiles", "nextCursor"]
    ),
    "AvatarUploadResponse": obj({"avatarUrl": string()}, ["avatarUrl"]),
    "DeleteProfileResponse": obj(
        {
            "userId": string(format="uuid"),
            "deleted": boolean(description="Whether the profile was hidden."),
            "accountDeleted": boolean(enum=[False], description="Always false; account deletion is a separate AuthService saga."),
        },
        ["userId", "deleted", "accountDeleted"],
    ),
    "FollowRequest": obj({"targetUserId": string(format="uuid")}, ["targetUserId"]),
    "FollowActionResponse": obj(
        {
            "following": boolean(),
            "followedAt": nullable(string(format="date-time")),
            "status": string(enum=["ACTIVE", "PENDING", "INACTIVE"]),
            "requestId": nullable(string(format="uuid")),
        },
        ["following", "followedAt", "status", "requestId"],
    ),
    "FollowEdge": obj(
        {"userId": string(format="uuid"), "followedAt": string(format="date-time")},
        ["userId", "followedAt"],
    ),
    "FollowListResponse": obj(
        {
            "edges": array(ref("FollowEdge")),
            "nextCursor": nullable(string()),
            "totalCount": integer(format="int64", minimum=0),
        },
        ["edges", "nextCursor", "totalCount"],
    ),
    "FollowCountsResponse": obj(
        {"followerCount": integer(format="int64", minimum=0), "followingCount": integer(format="int64", minimum=0)},
        ["followerCount", "followingCount"],
    ),
    "FollowStatusResponse": obj({"following": boolean()}, ["following"]),
    "FollowRequestResponse": obj(
        {
            "id": string(format="uuid"),
            "followerId": string(format="uuid"),
            "followeeId": string(format="uuid"),
            "status": string(enum=["PENDING", "ACCEPTED", "REJECTED", "CANCELLED"]),
            "createdAt": string(format="date-time"),
            "decidedAt": nullable(string(format="date-time")),
        },
        ["id", "followerId", "followeeId", "status", "createdAt", "decidedAt"],
    ),
    "FollowRequestPageResponse": obj(
        {"requests": array(ref("FollowRequestResponse")), "nextCursor": nullable(string())},
        ["requests", "nextCursor"],
    ),
    "RelationshipPolicyResponse": obj(
        {
            "blockedByViewer": boolean(),
            "blockedViewer": boolean(),
            "mutedByViewer": boolean(),
            "following": boolean(),
        },
        ["blockedByViewer", "blockedViewer", "mutedByViewer", "following"],
    ),
    "PeopleRecommendation": obj(
        {
            "userId": string(format="uuid"),
            "username": string(),
            "nickname": string(),
            "bio": string(),
            "avatarUrl": string(),
            "source": string(),
            "reasonCode": string(),
            "scoreMicros": integer(format="int64"),
            "rankVersion": string(),
            "impressionId": string(format="uuid"),
        },
        ["userId", "username", "nickname", "bio", "avatarUrl", "source", "reasonCode", "scoreMicros", "rankVersion", "impressionId"],
    ),
    "PeopleRecommendationEventRequest": obj(
        {
            "candidateUserId": string(format="uuid"),
            "impressionId": string(format="uuid"),
            "action": string(
                enum=[
                    "CLICK", "DWELL", "LIKE", "COMMENT", "FOLLOW", "ACCEPT_FOLLOW",
                    "HIDE", "NOT_INTERESTED", "BLOCK", "MUTE", "REPORT", "SAVE",
                    "SHARE", "FAST_SCROLL",
                ],
                description="Outcome action linked to an existing impression. Impression events are emitted by the recommendation read path and must not be submitted here.",
            ),
            "source": string(minLength=1),
            "modelVersion": string(minLength=1),
            "position": nullable(integer(minimum=0)),
            "propensityMicros": nullable(integer(minimum=0, maximum=1_000_000)),
            "dwellMs": nullable(integer(minimum=0)),
            "metadata": obj(additionalProperties=True),
        },
        ["candidateUserId", "impressionId", "action", "source", "modelVersion"],
    ),
    "RecommendationEventResponse": obj({"accepted": boolean()}, ["accepted"]),
    "PeopleRecommendationPageResponse": obj(
        {
            "items": array(ref("PeopleRecommendation")),
            "nextCursor": nullable(string()),
            "snapshotId": string(format="uuid"),
            "rankVersion": string(),
            "requestId": string(format="uuid"),
            "fallbackUsed": boolean(),
        },
        ["items", "nextCursor", "snapshotId", "rankVersion", "requestId", "fallbackUsed"],
    ),
    "PostCreateRequest": obj(
        {
            "content": string(minLength=1, example="Hello from Port Rust."),
            "visibility": string(enum=["PUBLIC", "FRIENDS", "PRIVATE"]),
        },
        ["content", "visibility"],
    ),
    "CommentCreateRequest": obj({"content": string(minLength=1)}, ["content"]),
    "Post": obj(
        {
            "id": string(format="uuid"),
            "authorId": string(format="uuid"),
            "content": string(),
            "visibility": string(enum=["PUBLIC", "FRIENDS", "PRIVATE"]),
            "createdAt": string(format="date-time"),
            "commentCount": integer(format="int64", minimum=0),
            "likeCount": integer(format="int64", minimum=0),
            "likedByMe": boolean(),
        },
        ["id", "authorId", "content", "visibility", "createdAt", "commentCount", "likeCount", "likedByMe"],
    ),
    "Comment": obj(
        {
            "id": string(format="uuid"),
            "postId": string(format="uuid"),
            "authorId": string(format="uuid"),
            "content": string(),
            "createdAt": string(format="date-time"),
        },
        ["id", "postId", "authorId", "content", "createdAt"],
    ),
    "FeedRecommendationMetadata": obj(
        {
            "postId": string(format="uuid"),
            "source": string(),
            "reasonCode": string(),
            "scoreMicros": integer(format="int64"),
            "rankVersion": string(),
            "impressionId": string(format="uuid"),
            "position": integer(minimum=0),
            "propensityMicros": integer(minimum=0, maximum=1_000_000),
            "exploration": boolean(),
        },
        ["postId", "source", "reasonCode", "scoreMicros", "rankVersion", "impressionId", "position", "propensityMicros", "exploration"],
    ),
    "FeedResponse": obj(
        {"posts": array(ref("Post")), "nextCursor": nullable(string())},
        ["posts", "nextCursor"],
    ),
    "RankedFeedResponse": obj(
        {
            "posts": array(ref("Post")),
            "nextCursor": nullable(string()),
            "snapshotId": nullable(string(format="uuid")),
            "rankVersion": nullable(string()),
            "requestId": nullable(string(format="uuid")),
            "fallbackUsed": boolean(),
            "recommendationMetadata": array(ref("FeedRecommendationMetadata")),
            "failedSources": array(string()),
        },
        ["posts", "nextCursor", "snapshotId", "rankVersion", "requestId", "fallbackUsed", "recommendationMetadata", "failedSources"],
    ),
    "FeedRecommendationEventRequest": obj(
        {
            "postId": string(format="uuid"),
            "impressionId": string(format="uuid"),
            "action": string(
                enum=[
                    "CLICK", "DWELL", "LIKE", "COMMENT", "FOLLOW", "ACCEPT_FOLLOW",
                    "HIDE", "NOT_INTERESTED", "BLOCK", "MUTE", "REPORT", "SAVE",
                    "SHARE", "FAST_SCROLL",
                ],
                description="Outcome action linked to an existing feed impression. IMPRESSION is recorded by GET /api/feed and is rejected here.",
            ),
            "source": string(minLength=1),
            "modelVersion": string(minLength=1),
            "position": nullable(integer(minimum=0)),
            "propensityMicros": nullable(integer(minimum=0, maximum=1_000_000)),
            "dwellMs": nullable(integer(minimum=0)),
            "metadata": obj(additionalProperties=True),
        },
        ["postId", "impressionId", "action", "source", "modelVersion"],
    ),
    "FeedRecommendationEventResponse": obj({"accepted": boolean()}, ["accepted"]),
    "PostDetailResponse": obj({"post": ref("Post"), "comments": array(ref("Comment"))}, ["post", "comments"]),
    "DeletePostResponse": obj({"postId": string(format="uuid"), "deleted": boolean()}, ["postId", "deleted"]),
    "PostLikeResponse": obj(
        {"postId": string(format="uuid"), "likeCount": integer(format="int64", minimum=0), "liked": boolean()},
        ["postId", "likeCount", "liked"],
    ),
    "Notification": obj(
        {
            "id": string(format="uuid"),
            "userId": string(format="uuid"),
            "type": string(enum=["POST_CREATED", "POST_LIKED", "POST_COMMENTED", "FRIEND_REQUEST", "FRIEND_ACCEPTED"]),
            "payloadJson": string(description="Serialized notification payload JSON."),
            "createdAt": string(format="date-time"),
            "readAt": string(description="Read timestamp, or an empty string while unread."),
        },
        ["id", "userId", "type", "payloadJson", "createdAt", "readAt"],
    ),
    "NotificationFeedResponse": obj(
        {"items": array(ref("Notification")), "nextCursor": nullable(string())}, ["items", "nextCursor"]
    ),
    "NotificationMarkAllReadResponse": obj({"updatedCount": integer(format="int64", minimum=0)}, ["updatedCount"]),
    "CreateReportRequest": obj(
        {
            "targetType": string(enum=["USER", "POST", "COMMENT"]),
            "targetId": string(format="uuid"),
            "category": string(enum=["SPAM", "HARASSMENT", "HATE", "VIOLENCE", "OTHER"]),
            "reason": string(minLength=3, maxLength=1000),
        },
        ["targetType", "targetId", "category", "reason"],
    ),
    "CreateModerationDecisionRequest": obj(
        {
            "decisionType": string(enum=["DISMISS", "HIDE_CONTENT", "RESTORE_CONTENT", "RESTRICT_USER", "LIFT_RESTRICTION"]),
            "note": string(maxLength=2000),
            "restrictionExpiresAt": nullable(string(format="date-time")),
        },
        ["decisionType"],
    ),
    "ResolveDeadLetterReplayRequest": obj(
        {
            "resolutionId": string(format="uuid", description="Client-generated idempotency ID. Reuse the exact ID and request values after an uncertain response."),
            "reason": string(minLength=20, maxLength=1024),
            "evidenceReference": string(minLength=3, maxLength=256, pattern="^[A-Za-z0-9][A-Za-z0-9._:/#-]{2,255}$", description="Identifier for a protected incident or change record. The API validates syntax only; the operator must verify the protected record. Do not include payloads or secrets."),
        },
        ["resolutionId", "reason", "evidenceReference"],
        additionalProperties=False,
    ),
    "DeadLetterReplayAcceptedResponse": obj(
        {
            "eventId": string(format="uuid"),
            "resolutionId": string(format="uuid"),
            "accepted": boolean(),
        },
        ["eventId", "resolutionId", "accepted"],
        additionalProperties=False,
    ),
    "ReplayContentDecisionRequest": obj(
        {
            "resolutionId": string(format="uuid", description="Client-generated idempotency ID. Reuse the exact ID and request values after an uncertain response."),
            "reason": string(minLength=20, maxLength=1024),
            "evidenceReference": string(minLength=3, maxLength=256, pattern="^[A-Za-z0-9][A-Za-z0-9._:/#-]{2,255}$", description="Identifier for a protected incident or change record; do not include payloads or secrets."),
        },
        ["resolutionId", "reason", "evidenceReference"],
        additionalProperties=False,
    ),
    "ContentDecisionReplayAcceptedResponse": obj(
        {
            "reportId": string(format="uuid"),
            "resolutionId": string(format="uuid"),
            "accepted": boolean(),
        },
        ["reportId", "resolutionId", "accepted"],
        additionalProperties=False,
    ),
    "ModerationReport": obj(
        {
            "id": string(format="uuid"),
            "reporterId": string(format="uuid"),
            "targetType": string(enum=["USER", "POST", "COMMENT"]),
            "targetId": string(format="uuid"),
            "category": string(enum=["SPAM", "HARASSMENT", "HATE", "VIOLENCE", "OTHER"]),
            "reason": string(),
            "status": string(enum=["OPEN", "RESOLVED", "DISMISSED"]),
            "createdAt": string(format="date-time"),
            "resolvedAt": nullable(string(format="date-time")),
        },
        ["id", "reporterId", "targetType", "targetId", "category", "reason", "status", "createdAt", "resolvedAt"],
    ),
    "ModerationDecisionRecord": obj(
        {
            "id": string(format="uuid"),
            "reportId": string(format="uuid"),
            "actorId": string(format="uuid"),
            "decisionType": string(enum=["DISMISS", "HIDE_CONTENT", "RESTORE_CONTENT", "RESTRICT_USER", "LIFT_RESTRICTION"]),
            "note": string(),
            "createdAt": string(format="date-time"),
        },
        ["id", "reportId", "actorId", "decisionType", "note", "createdAt"],
    ),
    "ModerationDecisionResponse": obj(
        {"report": ref("ModerationReport"), "decision": ref("ModerationDecisionRecord")},
        ["report", "decision"],
    ),
    "ModerationReportListResponse": obj({"reports": array(ref("ModerationReport"))}, ["reports"]),
    "ModerationContentDecisionStatus": obj(
        {
            "reportId": string(format="uuid"),
            "targetType": string(enum=["POST", "COMMENT"]),
            "targetId": string(format="uuid"),
            "decisionType": string(enum=["HIDE_CONTENT", "RESTORE_CONTENT"]),
            "state": string(enum=["PENDING", "TARGET_UNAVAILABLE", "RETRY_EXHAUSTED"]),
            "attemptCount": integer(minimum=0),
            "replayCount": integer(minimum=0),
            "lastErrorCode": nullable(string()),
            "createdAt": string(format="date-time"),
            "updatedAt": string(format="date-time"),
            "nextAttemptAt": string(format="date-time"),
        },
        ["reportId", "targetType", "targetId", "decisionType", "state", "attemptCount", "replayCount", "lastErrorCode", "createdAt", "updatedAt", "nextAttemptAt"],
    ),
    "ModerationContentDecisionCursor": obj(
        {"createdAt": string(format="date-time"), "reportId": string(format="uuid")},
        ["createdAt", "reportId"],
    ),
    "ModerationContentDecisionListResponse": obj(
        {
            "decisions": array(ref("ModerationContentDecisionStatus")),
            "nextCursor": nullable(ref("ModerationContentDecisionCursor")),
        },
        ["decisions", "nextCursor"],
    ),
    "RouteContractVersion": obj(
        {
            "version": string(),
            "status": string(),
            "deprecationDate": nullable(string(format="date-time")),
            "responseSchemaRef": string(),
        },
        ["version", "status", "deprecationDate", "responseSchemaRef"],
    ),
    "RouteContract": obj(
        {
            "id": string(format="uuid"),
            "routeKey": string(),
            "owner": string(),
            "slaTarget": nullable(string()),
            "status": string(),
            "description": nullable(string()),
            "versions": array(ref("RouteContractVersion")),
        },
        ["id", "routeKey", "owner", "slaTarget", "status", "description", "versions"],
    ),
    "RouteContractCreateRequest": obj(
        {
            "routeKey": string(minLength=1),
            "owner": string(minLength=1),
            "slaTarget": nullable(string()),
            "status": nullable(string()),
            "description": nullable(string()),
        },
        ["routeKey", "owner"],
    ),
    "RouteContractVersionCreateRequest": obj(
        {
            "version": string(minLength=1),
            "responseSchemaRef": string(minLength=1),
            "status": nullable(string()),
            "deprecationDate": nullable(string(format="date-time")),
        },
        ["version", "responseSchemaRef"],
    ),
    "RouteContractVersionUpdateRequest": obj(
        {"status": nullable(string()), "deprecationDate": nullable(string(format="date-time"))}
    ),
    "PolicySet": obj(
        {
            "id": string(format="uuid"),
            "name": string(),
            "rateLimit": nullable(string()),
            "quota": nullable(string()),
            "timeoutMs": nullable(integer()),
            "retryPolicy": nullable(string()),
            "circuitBreakerPolicy": nullable(string()),
            "cachePolicy": nullable(string()),
        },
        ["id", "name", "rateLimit", "quota", "timeoutMs", "retryPolicy", "circuitBreakerPolicy", "cachePolicy"],
    ),
    "PolicySetRequest": obj(
        {
            "name": string(minLength=1),
            "rateLimit": nullable(string()),
            "quota": nullable(string()),
            "timeoutMs": nullable(integer(minimum=1)),
            "retryPolicy": nullable(string()),
            "circuitBreakerPolicy": nullable(string()),
            "cachePolicy": nullable(string()),
        },
        ["name"],
    ),
    "ClientProfile": obj(
        {
            "id": string(format="uuid"),
            "clientType": string(),
            "allowedContracts": array(string()),
            "policySetId": nullable(string(format="uuid")),
        },
        ["id", "clientType", "allowedContracts", "policySetId"],
    ),
    "ClientProfileRequest": obj(
        {
            "clientType": string(minLength=1),
            "allowedContracts": array(string(), uniqueItems=True),
            "policySetId": nullable(string(format="uuid")),
        },
        ["clientType", "allowedContracts"],
    ),
    "RolloutPolicy": obj(
        {
            "id": string(format="uuid"),
            "contractId": string(format="uuid"),
            "version": string(),
            "strategy": string(),
            "status": string(),
            "percent": nullable(integer(minimum=0, maximum=100)),
        },
        ["id", "contractId", "version", "strategy", "status", "percent"],
    ),
    "RolloutCreateRequest": obj(
        {
            "contractId": string(format="uuid"),
            "version": string(minLength=1),
            "strategy": string(minLength=1),
            "status": nullable(string()),
            "percent": nullable(integer(minimum=0, maximum=100)),
        },
        ["contractId", "version", "strategy"],
    ),
    "TelemetrySummary": obj(
        {
            "totalRequests": integer(format="int64", minimum=0),
            "errorRate": number(format="double", minimum=0, maximum=1),
            "p95LatencyMs": integer(format="int64", minimum=0),
        },
        ["totalRequests", "errorRate", "p95LatencyMs"],
    ),
    "OperationStatus": obj(
        {
            "operationId": string(format="uuid"),
            "state": string(),
            "outcome": string(
                enum=["PENDING", "COMMITTED", "REJECTED_BEFORE_COMMIT", "UNKNOWN"],
                description="Best durable operation outcome. HTTP status alone never proves rejection; UNKNOWN does not authorize a new command.",
            ),
            "action": string(),
            "resourcePath": string(),
            "responseStatus": nullable(integer()),
            "createdAt": string(format="date-time"),
            "finishedAt": nullable(string(format="date-time")),
            "replayable": boolean(),
            "ownerReceipt": ref("OwnerReceiptStatus"),
            "manualResolution": nullable(ref("ManualOperationResolution")),
        },
        [
            "operationId",
            "state",
            "outcome",
            "action",
            "resourcePath",
            "responseStatus",
            "createdAt",
            "finishedAt",
            "replayable",
            "ownerReceipt",
            "manualResolution",
        ],
    ),
    "CallerOperationStatus": obj(
        {
            "operationId": string(format="uuid"),
            "state": string(enum=["IN_PROGRESS", "QUEUED", "COMPLETED", "UNKNOWN"]),
            "outcome": string(
                enum=["PENDING", "COMMITTED", "REJECTED_BEFORE_COMMIT", "UNKNOWN"],
                description="Best durable operation outcome. UNKNOWN does not authorize a new command.",
            ),
            "responseStatus": nullable(integer()),
            "createdAt": string(format="date-time"),
            "finishedAt": nullable(string(format="date-time")),
            "replayable": boolean(),
        },
        ["operationId", "state", "outcome", "responseStatus", "createdAt", "finishedAt", "replayable"],
        additionalProperties=False,
    ),
    "AsyncOperationAccepted": obj(
        {
            "operationId": string(format="uuid"),
            "status": string(enum=["ACCEPTED"]),
        },
        ["operationId", "status"],
        additionalProperties=False,
    ),
    "ManualOperationResolutionRequest": obj(
        {
            "disposition": string(enum=["COMMITTED", "NOT_COMMITTED", "UNRESOLVED"]),
            "evidenceKind": string(enum=["DATABASE_AUDIT", "OWNER_CONFIRMATION", "SERVICE_LOG", "INCIDENT_RECORD", "OTHER"]),
            "evidenceReference": string(minLength=3, maxLength=256, pattern="^[A-Za-z0-9][A-Za-z0-9._:/#-]*$", description="Identifier for a protected evidence record; do not include raw secrets or evidence payloads."),
            "reason": string(minLength=20, maxLength=1024),
            "supersedesResolutionId": nullable(string(format="uuid")),
        },
        ["disposition", "evidenceKind", "evidenceReference", "reason"],
        additionalProperties=False,
    ),
    "ManualOperationResolution": obj(
        {
            "resolutionId": string(format="uuid"),
            "operationId": string(format="uuid"),
            "revision": integer(minimum=1),
            "supersedesResolutionId": nullable(string(format="uuid")),
            "disposition": string(enum=["COMMITTED", "NOT_COMMITTED", "UNRESOLVED"]),
            "evidenceKind": string(enum=["DATABASE_AUDIT", "OWNER_CONFIRMATION", "SERVICE_LOG", "INCIDENT_RECORD", "OTHER"]),
            "evidenceReference": string(),
            "reason": string(),
            "reviewerId": string(),
            "createdAt": string(format="date-time"),
        },
        ["resolutionId", "operationId", "revision", "supersedesResolutionId", "disposition", "evidenceKind", "evidenceReference", "reason", "reviewerId", "createdAt"],
    ),
    "OwnerReceiptStatus": obj(
        {
            "lookupStatus": string(
                enum=["NOT_APPLICABLE", "FOUND", "NOT_FOUND", "UNAVAILABLE", "MISMATCH"],
                description="NOT_FOUND is not proof that the mutation did not commit. UNAVAILABLE and MISMATCH require investigation; none of these states authorizes a retry.",
            ),
            "receiptState": nullable(
                string(enum=["REPLAYABLE", "EXPIRED", "INCOMPLETE"])
            ),
            "scope": nullable(string()),
            "committedAt": nullable(string(format="date-time")),
            "replayUntil": nullable(string(format="date-time")),
        },
        ["lookupStatus", "receiptState", "scope", "committedAt", "replayUntil"],
    ),
    "DownstreamCall": obj(
        {"service": string(), "path": string(), "fields": array(string())},
        ["service", "path", "fields"],
    ),
    "AggregationPayload": obj(
        {
            "routeKey": string(),
            "joinStrategy": string(),
            "downstreams": array(ref("DownstreamCall")),
        },
        ["routeKey", "joinStrategy", "downstreams"],
    ),
    "AggregationResponse": obj(
        {"routeKey": string(), "cached": boolean(), "payload": ref("AggregationPayload")},
        ["routeKey", "cached", "payload"],
    ),
}
