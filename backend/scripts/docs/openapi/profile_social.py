"""Profile, social-graph, and recommendation operations."""

from __future__ import annotations

from .common import (
    CURSOR,
    IDEMPOTENCY_KEY,
    LIMIT10,
    LIMIT20,
    OperationAdder,
    SEARCH_CURSOR,
    async_acceptance_response,
    json_body,
    obj,
    p,
    path_parameter,
    query_parameter,
    string,
)


def add_operations(add: OperationAdder) -> None:
    """Add this operation family to the contract path catalog."""
    # Social graph, profiles, and recommendations, ordered by path and method.
    add("/api/follows", "post", operation_id="followUser", tag="Social graph", summary="Follow a public account or request a private account", success_schema="FollowActionResponse", success_headers=("OperationId", "IdempotencyReplayed"), request_body=json_body("FollowRequest"), parameters=[IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical follow command from FriendService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
    add("/api/follows/counts", "get", operation_id="getFollowCounts", tag="Social graph", summary="Get follower and following counts", success_schema="FollowCountsResponse", parameters=[query_parameter("user_id", "User UUID; defaults to current user", string(format="uuid"))])
    for route, op_id, summary in [
        ("/api/follows/followers", "listFollowers", "List followers"),
        ("/api/follows/following", "listFollowing", "List following"),
    ]:
        add(route, "get", operation_id=op_id, tag="Social graph", summary=summary, success_schema="FollowListResponse", parameters=[query_parameter("user_id", "User UUID; defaults to the current user", string(format="uuid")), CURSOR, LIMIT20])
    add("/api/follows/requests/incoming", "get", operation_id="listIncomingFollowRequests", tag="Follow requests", summary="List incoming follow requests", success_schema="FollowRequestPageResponse", parameters=[CURSOR, LIMIT20])
    add("/api/follows/requests/outgoing", "get", operation_id="listOutgoingFollowRequests", tag="Follow requests", summary="List outgoing follow requests", success_schema="FollowRequestPageResponse", parameters=[CURSOR, LIMIT20])
    add("/api/follows/requests/{request_id}", "delete", operation_id="cancelFollowRequest", tag="Follow requests", summary="Cancel an outgoing follow request", success_schema="FollowRequestResponse", parameters=[p("request_id")])
    add("/api/follows/requests/{request_id}/accept", "post", operation_id="acceptFollowRequest", tag="Follow requests", summary="Accept a follow request", success_schema="FollowActionResponse", parameters=[p("request_id")])
    add("/api/follows/requests/{request_id}/reject", "post", operation_id="rejectFollowRequest", tag="Follow requests", summary="Reject a follow request", success_schema="FollowRequestResponse", parameters=[p("request_id")])
    add("/api/follows/status", "get", operation_id="getFollowStatus", tag="Social graph", summary="Check whether the current user follows a target", success_schema="FollowStatusResponse", parameters=[query_parameter("target_user_id", "Target user UUID", string(format="uuid"), required=True)])
    add("/api/follows/{target_user_id}", "delete", operation_id="unfollowUser", tag="Social graph", summary="Unfollow a user", success_schema="FollowActionResponse", success_headers=("OperationId", "IdempotencyReplayed"), parameters=[p("target_user_id"), IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical unfollow command from FriendService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
    avatar_content = {
        media_type: {"schema": string(format="binary")}
        for media_type in ["image/png", "image/jpeg", "image/webp", "image/gif"]
    }
    add("/api/profile/avatar", "post", operation_id="uploadAvatar", tag="Profiles", summary="Upload the current user's avatar", success_schema="AvatarUploadResponse", request_body={"required": True, "content": {"multipart/form-data": {"schema": obj({"file": string(format="binary", description="PNG, JPEG, WebP, or GIF; maximum 1 MiB.")}, ["file"])}}}, description="The first multipart file field is stored as the current user's avatar. Supported media types: image/png, image/jpeg, image/webp, image/gif. Maximum size: 1 MiB.")
    add("/api/profile/avatar/{user_id}", "get", operation_id="getAvatar", tag="Profiles", summary="Get a user's avatar bytes", success_schema=None, success_content=avatar_content, success_description="Avatar content", success_headers=("CacheControl", "ContentType"), security="optional", parameters=[p("user_id")])
    add("/api/profile/batch", "post", operation_id="getProfilesBatch", tag="Profiles", summary="Get up to 100 profiles", success_schema="ProfileBatchResponse", request_body=json_body("ProfileBatchRequest"), security="optional")
    add("/api/profile/privacy", "put", operation_id="updateProfilePrivacy", tag="Profiles", summary="Update account visibility", success_schema="Profile", success_headers=("OperationId", "IdempotencyReplayed"), request_body=json_body("ProfilePrivacyUpdateRequest"), parameters=[IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical privacy command from ProfileService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
    add("/api/profile/search", "get", operation_id="searchProfiles", tag="Profiles", summary="Search profiles", success_schema="ProfileSearchResponse", security="optional", parameters=[query_parameter("q", "Search text", required=True), SEARCH_CURSOR, LIMIT10])
    add("/api/profile/username/{username}", "get", operation_id="getProfileByUsername", tag="Profiles", summary="Get a profile by username", success_schema="Profile", security="optional", parameters=[path_parameter("username", "Normalized username", schema=string(minLength=3, maxLength=20, pattern="^[a-zA-Z0-9_]+$"))])
    add("/api/profile/{user_id}", "get", operation_id="getProfile", tag="Profiles", summary="Get a profile", success_schema="Profile", security="optional", parameters=[p("user_id")])
    add("/api/profile/{user_id}", "put", operation_id="updateProfile", tag="Profiles", summary="Update the current user's profile", success_schema="Profile", success_headers=("OperationId", "IdempotencyReplayed"), request_body=json_body("ProfileUpdateRequest"), parameters=[p("user_id"), IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical profile edit from ProfileService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
    add(
        "/api/profile/{user_id}",
        "delete",
        operation_id="deleteProfile",
        tag="Profiles",
        summary="Hide the current user's profile",
        description=(
            "Soft-deletes and hides only the profile for the 30-day retention "
            "window. This operation does not delete or otherwise change the "
            "authentication account and does not invoke the AuthService "
            "account-deletion saga. The response explicitly reports "
            "accountDeleted=false. Use DELETE /auth/account for account erasure."
        ),
        success_schema="DeleteProfileResponse",
        parameters=[p("user_id")],
    )
    add("/api/recommendations/people", "get", operation_id="getPeopleRecommendations", tag="Recommendations", summary="Get ranked people recommendations", success_schema="PeopleRecommendationPageResponse", parameters=[CURSOR, LIMIT20])
    add("/api/recommendations/people/events", "post", operation_id="recordPeopleRecommendationEvent", tag="Recommendations", summary="Record a people-recommendation outcome", success_schema="RecommendationEventResponse", request_body=json_body("PeopleRecommendationEventRequest"))
    for relation, label in [("blocks", "Block"), ("mutes", "Mute")]:
        add(f"/api/relationships/{relation}/{{target_user_id}}", "post", operation_id=f"{label.lower()}User", tag="Relationship safety", summary=f"{label} a user", success_schema="RelationshipPolicyResponse", parameters=[p("target_user_id")])
        add(f"/api/relationships/{relation}/{{target_user_id}}", "delete", operation_id=f"un{label.lower()}User", tag="Relationship safety", summary=f"Remove a {label.lower()}", success_schema="RelationshipPolicyResponse", parameters=[p("target_user_id")])
    add("/api/relationships/{target_user_id}", "get", operation_id="getRelationshipPolicy", tag="Relationship safety", summary="Get relationship policy", success_schema="RelationshipPolicyResponse", parameters=[p("target_user_id")])
