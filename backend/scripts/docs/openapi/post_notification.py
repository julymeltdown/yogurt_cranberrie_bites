"""Feed, post, and notification operations."""

from __future__ import annotations

from .common import (
    CURSOR,
    IDEMPOTENCY_KEY,
    LIMIT10,
    OperationAdder,
    SEARCH_CURSOR,
    async_acceptance_response,
    json_body,
    p,
    query_parameter,
)


def add_operations(add: OperationAdder) -> None:
    """Add this operation family to the contract path catalog."""
    # Feed, notifications, and posts, ordered by path and HTTP method.
    add("/api/feed", "get", operation_id="getRankedFeed", tag="Feed", summary="Get the ranked hybrid home feed", success_schema="RankedFeedResponse", parameters=[CURSOR, LIMIT10])
    add("/api/feed/events", "post", operation_id="recordFeedRecommendationEvent", tag="Feed", summary="Record a feed recommendation outcome", success_schema="FeedRecommendationEventResponse", request_body=json_body("FeedRecommendationEventRequest"))
    add("/api/notifications", "get", operation_id="listNotifications", tag="Notifications", summary="List notification inbox", success_schema="NotificationFeedResponse", parameters=[CURSOR, LIMIT10])
    add("/api/notifications/read-all", "patch", operation_id="markAllNotificationsRead", tag="Notifications", summary="Mark all notifications read", success_schema="NotificationMarkAllReadResponse")
    add("/api/notifications/{notification_id}/read", "patch", operation_id="markNotificationRead", tag="Notifications", summary="Mark one notification read", success_schema="Notification", parameters=[p("notification_id")])
    add("/api/posts", "post", operation_id="createPost", tag="Posts", summary="Create a post", success_schema="Post", success_headers=("OperationId", "IdempotencyReplayed"), request_body=json_body("PostCreateRequest"), parameters=[IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical command from PostService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
    add("/api/posts/author/{author_id}", "get", operation_id="getAuthorPosts", tag="Posts", summary="Get visible posts by author", success_schema="FeedResponse", parameters=[p("author_id"), CURSOR, LIMIT10])
    add("/api/posts/search", "get", operation_id="searchPosts", tag="Posts", summary="Search visible posts", success_schema="FeedResponse", parameters=[query_parameter("q", "Search text", required=True), SEARCH_CURSOR, LIMIT10])
    add("/api/posts/trending", "get", operation_id="getTrendingPosts", tag="Posts", summary="Get trending visible posts", success_schema="FeedResponse", parameters=[CURSOR, LIMIT10])
    add("/api/posts/{post_id}", "get", operation_id="getPost", tag="Posts", summary="Get a post and comments", success_schema="PostDetailResponse", parameters=[p("post_id")])
    add("/api/posts/{post_id}", "delete", operation_id="deletePost", tag="Posts", summary="Soft-delete a post", success_schema="DeletePostResponse", success_headers=("OperationId", "IdempotencyReplayed"), parameters=[p("post_id"), IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical command from PostService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
    add("/api/posts/{post_id}/comments", "post", operation_id="createComment", tag="Comments", summary="Create a comment", success_schema="Comment", success_headers=("OperationId", "IdempotencyReplayed"), request_body=json_body("CommentCreateRequest"), parameters=[p("post_id"), IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical command from PostService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
    add("/api/posts/{post_id}/likes", "post", operation_id="likePost", tag="Posts", summary="Like a post", success_schema="PostLikeResponse", success_headers=("OperationId", "IdempotencyReplayed"), parameters=[p("post_id"), IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical command from PostService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
    add("/api/posts/{post_id}/likes", "delete", operation_id="unlikePost", tag="Posts", summary="Remove a post like", success_schema="PostLikeResponse", success_headers=("OperationId", "IdempotencyReplayed"), parameters=[p("post_id"), IDEMPOTENCY_KEY], description="Optional Idempotency-Key retries the identical command from PostService's durable owner receipt for 24 hours. When durable async execution is enabled, a timed-out command may return 202 after queue acceptance.", extra_responses={"202": async_acceptance_response()})
