use crate::runtime::ScriptedRuntime;
use anyhow::Result;
use api_gateway::adapters::out::post_client::classify_post_client_error;
use api_gateway::application::dto::auth_dto::{
    AuthResponse, EmailAvailabilityResponse, EmailCheckRequest, EmailSignupRequest,
    EmailVerifyRequest, LinkedProvidersResponse, LoginRequest, OAuthLinkRequest,
    PasswordResetConfirmRequest, PasswordResetRequest, PasswordResetResponse, SignupResponse,
};
use api_gateway::application::dto::friend_dto::{
    FollowActionResponse, FollowCountsResponse, FollowListResponse, FollowRequestPageResponse,
    FollowRequestResponse, FollowStatusResponse, RelationshipPolicyResponse,
};
use api_gateway::application::dto::notification_dto::{
    NotificationFeedResponse, NotificationMarkAllReadResponse, NotificationResponse,
};
use api_gateway::application::dto::post_dto::{
    CommentCreateRequest, CommentResponse, DeletePostResponse, FeedRecommendationEventRequest,
    FeedRecommendationEventResponse, FeedResponse, PostCreateRequest, PostDetailResponse,
    PostLikeResponse, PostResponse, RankedFeedResponse,
};
use api_gateway::application::dto::profile_dto::{
    AvatarContent, DeleteProfileResponse, ProfileBatchResponse, ProfileResponse,
    ProfileSearchResponse, ProfileUpdateRequest,
};
use api_gateway::application::ports::{
    AggregationRecipeRepositoryPort, AuthClientPort, ClientProfileRepositoryPort, FriendClientPort,
    NotificationClientPort, PolicyRepositoryPort, PostClientPort, ProfileClientPort,
    ResponseCachePort, RolloutRepositoryPort, RouteContractRepositoryPort, TelemetryRecord,
    TelemetryRepositoryPort,
};
use api_gateway::application::search_cursor::{SearchCursorCodec, SearchSort};
use api_gateway::application::service::{
    AggregationService, AuthGatewayService, ClientProfileService, FriendGatewayService,
    NotificationGatewayService, PolicyService, PostGatewayService, ProfileGatewayService,
    RouteContractService, TelemetryService,
};
use api_gateway::domain::aggregation::{AggregationRecipe, DownstreamCall};
use api_gateway::domain::client::ClientProfile;
use api_gateway::domain::contract::{RouteContract, RouteContractVersion};
use api_gateway::domain::policy::{PolicySet, RolloutPolicy};
use api_gateway::domain::telemetry::TelemetrySummary;
use api_gateway::domain::IdentityProvider;
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const POST_SEARCH_POSITION: &str =
    "2026-10-04T12:30:45.123456Z|11111111-1111-1111-1111-111111111111";

fn test_cursor_codec() -> SearchCursorCodec {
    SearchCursorCodec::new(
        Arc::from(b"gateway-search-cursor-test-key-32-bytes-minimum".as_slice()),
        900,
        None,
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    )
    .expect("test cursor codec")
}

fn test_cursor_codec_with_legacy_window() -> SearchCursorCodec {
    SearchCursorCodec::new(
        Arc::from(b"gateway-search-cursor-test-key-32-bytes-minimum".as_slice()),
        900,
        Some(Utc::now() + ChronoDuration::hours(1)),
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    )
    .expect("test cursor codec with a bounded legacy window")
}

#[tokio::test]
async fn aggregation_cache_hit_returns_cached_payload_without_repository_lookup() {
    let repository = Arc::new(FakeAggregationRepository::default());
    let cache = Arc::new(FakeResponseCache::with_cached(json!({"cached": true})));
    let service =
        AggregationService::new(repository.clone(), cache.clone(), Duration::from_secs(60));

    let response = service
        .aggregate("GET /profile".to_owned())
        .await
        .expect("aggregate from cache");

    assert!(response.cached);
    assert_eq!(response.payload, json!({"cached": true}));
    assert!(repository
        .find_by_route_key_calls
        .lock()
        .unwrap()
        .is_empty());
    assert_eq!(
        *cache.get_calls.lock().unwrap(),
        vec!["aggregation:GET /profile".to_owned()]
    );
    assert!(cache.put_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn aggregation_cache_miss_builds_payload_from_recipe_and_stores_it() {
    let repository = Arc::new(FakeAggregationRepository::with_recipe(AggregationRecipe {
        id: "recipe-1".to_owned(),
        route_key: "GET /feed".to_owned(),
        downstream_calls: vec![DownstreamCall {
            service: "post-service".to_owned(),
            path: "/posts/feed".to_owned(),
            fields: vec!["posts".to_owned()],
        }],
        join_strategy: "merge".to_owned(),
        timeout_ms: Some(250),
        cache_policy: Some("short".to_owned()),
    }));
    let cache = Arc::new(FakeResponseCache::default());
    let service =
        AggregationService::new(repository.clone(), cache.clone(), Duration::from_secs(30));

    let response = service
        .aggregate("GET /feed".to_owned())
        .await
        .expect("aggregate from repository");

    assert!(!response.cached);
    assert_eq!(response.payload["routeKey"], "GET /feed");
    assert_eq!(response.payload["joinStrategy"], "merge");
    assert_eq!(
        response.payload["downstreams"][0]["service"],
        "post-service"
    );
    assert_eq!(
        *repository.find_by_route_key_calls.lock().unwrap(),
        vec!["GET /feed".to_owned()]
    );
    let puts = cache.put_calls.lock().unwrap();
    assert_eq!(puts.len(), 1);
    assert_eq!(puts[0].cache_key, "aggregation:GET /feed");
    assert_eq!(puts[0].ttl, Duration::from_secs(30));
}

#[tokio::test]
async fn telemetry_records_request_fields_and_returns_summary() {
    let repository = Arc::new(FakeTelemetryRepository::default());
    *repository.summary.lock().unwrap() = TelemetrySummary {
        total_requests: 10,
        error_rate: 0.2,
        p95_latency_ms: 150,
    };
    let runtime = ScriptedRuntime::new();
    let service = TelemetryService::new(repository.clone(), runtime.clone());
    let downstream = HashMap::from([("auth".to_owned(), 17)]);

    service
        .record_request(
            "corr-1".to_owned(),
            "POST /login".to_owned(),
            Some("mobile".to_owned()),
            "ERROR".to_owned(),
            42,
            Some("AUTH_DOWN".to_owned()),
            downstream.clone(),
        )
        .await
        .expect("record telemetry");
    let summary = service.summary().await.expect("summary");

    assert_eq!(summary.total_requests, 10);
    let records = repository.records.lock().unwrap();
    assert_eq!(records.len(), 1);
    assert!(!records[0].id.is_empty());
    assert_eq!(records[0].id, runtime.id.to_string());
    assert_eq!(records[0].recorded_at, *runtime.wall.lock().unwrap());
    assert_eq!(runtime.id_reads.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.utc_reads.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.monotonic_reads.load(Ordering::SeqCst), 0);
    assert_eq!(records[0].correlation_id, "corr-1");
    assert_eq!(records[0].route_key, "POST /login");
    assert_eq!(records[0].client_profile_id.as_deref(), Some("mobile"));
    assert_eq!(records[0].downstream_latencies, downstream);
}

#[tokio::test]
async fn profile_gateway_merges_batch_counts_and_preserves_missing_count_values() {
    let profile_client = Arc::new(FakeProfileClient::with_profiles(vec![
        profile_response("user-1", 1, 2),
        profile_response("user-2", 3, 4),
    ]));
    let friend_client = Arc::new(FakeFriendClient::default());
    friend_client.counts_batch.lock().unwrap().insert(
        "user-1".to_owned(),
        FollowCountsResponse {
            follower_count: 10,
            following_count: 20,
        },
    );
    let service = ProfileGatewayService::new(
        profile_client.clone(),
        friend_client.clone(),
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    let response = service
        .get_profiles(vec!["user-1".to_owned(), "user-2".to_owned()])
        .await
        .expect("profiles");

    assert_eq!(response.profiles[0].follower_count, 10);
    assert_eq!(response.profiles[0].following_count, 20);
    assert_eq!(response.profiles[1].follower_count, 3);
    assert_eq!(response.profiles[1].following_count, 4);
    assert_eq!(
        *friend_client.get_counts_batch_calls.lock().unwrap(),
        vec![vec!["user-1".to_owned(), "user-2".to_owned()]]
    );
}

#[tokio::test]
async fn profile_gateway_hides_blocked_profiles_and_muted_search_results() {
    let profile_client = Arc::new(FakeProfileClient::with_profiles(vec![
        profile_response("user-1", 1, 2),
        profile_response("user-2", 3, 4),
    ]));
    let friend_client = Arc::new(FakeFriendClient::default());
    friend_client.relationship_policies.lock().unwrap().insert(
        ("viewer".into(), "user-1".into()),
        RelationshipPolicyResponse {
            blocked_viewer: true,
            ..RelationshipPolicyResponse::default()
        },
    );
    friend_client.relationship_policies.lock().unwrap().insert(
        ("viewer".into(), "user-2".into()),
        RelationshipPolicyResponse {
            muted_by_viewer: true,
            ..RelationshipPolicyResponse::default()
        },
    );
    let service = ProfileGatewayService::new(
        profile_client,
        friend_client,
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    let direct = service
        .get_profile_for_viewer(Some("viewer".into()), "user-1".into())
        .await
        .expect_err("blocked profile must be hidden");
    let search = service
        .search_profiles_for_viewer(Some("viewer".into()), "user".into(), None, 20)
        .await
        .expect("search profiles");

    assert_eq!(direct.to_string(), "Profile not found");
    assert!(search.profiles.is_empty());
}

#[tokio::test]
async fn profile_gateway_enforces_private_visibility_and_block_precedence() {
    let mut private_profile = profile_response("private-user", 0, 0);
    private_profile.account_visibility = "PRIVATE".into();
    let profile_client = Arc::new(FakeProfileClient::with_profiles(vec![private_profile]));
    let friend_client = Arc::new(FakeFriendClient::default());
    friend_client.relationship_policies.lock().unwrap().insert(
        ("viewer".into(), "private-user".into()),
        RelationshipPolicyResponse::default(),
    );
    let service = ProfileGatewayService::new(
        profile_client,
        friend_client.clone(),
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    assert_eq!(
        service
            .get_profile("private-user".into())
            .await
            .expect_err("anonymous viewers cannot read private profiles")
            .to_string(),
        "Profile not found"
    );
    assert_eq!(
        service
            .get_profile_for_viewer(Some("viewer".into()), "private-user".into())
            .await
            .expect_err("a pending or absent follow must not grant profile access")
            .to_string(),
        "Profile not found"
    );
    assert!(
        service
            .get_profile_for_viewer(Some("private-user".into()), "private-user".into())
            .await
            .is_ok(),
        "the profile owner retains access"
    );
    assert!(service
        .get_profiles_for_viewer(Some("viewer".into()), vec!["private-user".into()])
        .await
        .expect("profile batch")
        .profiles
        .is_empty());
    assert!(service
        .search_profiles_for_viewer(Some("viewer".into()), "private".into(), None, 10)
        .await
        .expect("profile search")
        .profiles
        .is_empty());

    friend_client.relationship_policies.lock().unwrap().insert(
        ("viewer".into(), "private-user".into()),
        RelationshipPolicyResponse {
            following: true,
            ..RelationshipPolicyResponse::default()
        },
    );
    assert!(
        service
            .get_profile_for_viewer(Some("viewer".into()), "private-user".into())
            .await
            .is_ok(),
        "an accepted follower can read the private profile"
    );
    assert_eq!(
        service
            .get_profiles_for_viewer(Some("viewer".into()), vec!["private-user".into()])
            .await
            .expect("profile batch")
            .profiles
            .len(),
        1
    );
    assert_eq!(
        service
            .search_profiles_for_viewer(Some("viewer".into()), "private".into(), None, 10)
            .await
            .expect("profile search")
            .profiles
            .len(),
        1
    );

    friend_client.relationship_policies.lock().unwrap().insert(
        ("viewer".into(), "private-user".into()),
        RelationshipPolicyResponse {
            following: true,
            blocked_by_viewer: true,
            ..RelationshipPolicyResponse::default()
        },
    );
    assert_eq!(
        service
            .get_profile_for_viewer(Some("viewer".into()), "private-user".into())
            .await
            .expect_err("a block overrides an accepted follow")
            .to_string(),
        "Profile not found"
    );
}

#[tokio::test]
async fn profile_gateway_fails_closed_on_incomplete_relationships_and_invalid_profile_state() {
    let mut public_profile = profile_response("public-user", 0, 0);
    let mut private_profile = profile_response("private-user", 0, 0);
    private_profile.account_visibility = "PRIVATE".into();
    let mut unknown_visibility = profile_response("unknown-visibility", 0, 0);
    unknown_visibility.account_visibility = "UNRECOGNIZED".into();
    let mut inactive_profile = profile_response("inactive-user", 0, 0);
    inactive_profile.lifecycle_status = "DELETED".into();
    public_profile.lifecycle_status = "ACTIVE".into();
    let profile_client = Arc::new(FakeProfileClient::with_profiles(vec![
        public_profile,
        private_profile,
        unknown_visibility,
        inactive_profile,
    ]));
    let friend_client = Arc::new(FakeFriendClient::default());
    friend_client
        .missing_relationship_batch
        .lock()
        .unwrap()
        .insert(("viewer".into(), "private-user".into()));
    let service = ProfileGatewayService::new(
        profile_client,
        friend_client,
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    assert!(service
        .get_profiles_for_viewer(
            Some("viewer".into()),
            vec!["public-user".into(), "private-user".into()]
        )
        .await
        .expect_err("a missing policy entry could conceal a block or private relationship")
        .to_string()
        .contains("Incomplete profile relationship policy batch"));

    let anonymous = service
        .get_profiles(vec![
            "public-user".into(),
            "private-user".into(),
            "unknown-visibility".into(),
            "inactive-user".into(),
        ])
        .await
        .expect("anonymous profile batch");
    assert_eq!(
        anonymous
            .profiles
            .iter()
            .map(|profile| profile.user_id.as_str())
            .collect::<Vec<_>>(),
        vec!["public-user"]
    );
    assert_eq!(
        service
            .get_profile("unknown-visibility".into())
            .await
            .expect_err("unknown visibility must not be treated as public")
            .to_string(),
        "Profile not found"
    );
    assert_eq!(
        service
            .get_profile("inactive-user".into())
            .await
            .expect_err("inactive lifecycle state must not be readable")
            .to_string(),
        "Profile not found"
    );
}

#[tokio::test]
async fn profile_search_keeps_scanning_past_blocked_profiles_to_fill_the_page() {
    // Visibility is decided in the gateway, because a block is a relationship the
    // profile service does not own. That makes a database page different from a
    // response page: the first batch can be entirely blocked while matching
    // profiles remain behind it, and answering from that one batch returns a short
    // page with a cursor -- which reads exactly like the end of the results.
    let stranger = "user-stranger";
    let blocked = profile_response("user-blocked", 0, 0);
    let early = profile_response("user-early", 0, 0);
    let late = profile_response("user-late", 0, 0);
    let profile_client = Arc::new(FakeProfileClient::with_profiles(Vec::new()));
    // The first batch is entirely blocked and still has rows behind it, so a
    // single-batch implementation would answer with an empty page.
    profile_client.search_pages.lock().unwrap().extend([
        ProfileSearchResponse {
            profiles: vec![blocked.clone()],
            next_cursor: Some("user_blocked".to_owned()),
        },
        ProfileSearchResponse {
            profiles: vec![early.clone(), late.clone()],
            next_cursor: None,
        },
    ]);
    let friend_client = Arc::new(FakeFriendClient::default());
    friend_client.relationship_policies.lock().unwrap().insert(
        (stranger.into(), "user-blocked".into()),
        RelationshipPolicyResponse {
            blocked_by_viewer: true,
            ..RelationshipPolicyResponse::default()
        },
    );
    let service = ProfileGatewayService::new(
        profile_client.clone(),
        friend_client,
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    // When: a page of two is requested while an unreadable row sits ahead of the
    // readable ones.
    let page = service
        .search_profiles_for_viewer(Some(stranger.into()), "user".into(), None, 2)
        .await
        .expect("search profiles");

    // Then: the scan continued past the blocked batch and filled the page.
    let ids: Vec<&str> = page
        .profiles
        .iter()
        .map(|profile| profile.user_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![early.user_id.as_str(), late.user_id.as_str()],
        "the scan must continue past the blocked batch and return the readable profiles"
    );
    assert_eq!(
        page.next_cursor, None,
        "the matching set ended, so there is no continuation to offer"
    );
    // Both batches were requested, which is the behaviour under test.
    assert_eq!(profile_client.search_calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn profile_search_continues_from_the_last_visible_profile_when_a_batch_overflows() {
    let mut profiles: Vec<_> = (1..=3)
        .map(|index| profile_response(&format!("user-{index}"), 0, 0))
        .collect();
    for (index, profile) in profiles.iter_mut().enumerate() {
        profile.username = format!("user_{}", index + 1);
    }
    let profile_client = Arc::new(FakeProfileClient::with_profiles(profiles));
    let service = ProfileGatewayService::new(
        profile_client,
        Arc::new(FakeFriendClient::default()),
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    let page = service
        .search_profiles("user".into(), None, 2)
        .await
        .unwrap();

    assert_eq!(page.profiles.len(), 2);
    assert_eq!(page.profiles[1].user_id, "user-2");
    // The upstream batch is exhausted, but user-3 must still be reachable.
    assert_eq!(
        test_cursor_codec()
            .decode(
                page.next_cursor.as_deref(),
                "user",
                None,
                SearchSort::ProfilesUsernameAscV1,
            )
            .expect("decode public search cursor")
            .as_deref(),
        Some(page.profiles[1].username.as_str())
    );
}

#[tokio::test]
async fn profile_search_bounds_empty_advancing_pages_and_preserves_continuation() {
    let profile_client = Arc::new(FakeProfileClient::with_profiles(Vec::new()));
    profile_client
        .search_pages
        .lock()
        .unwrap()
        .extend((1..=30).map(|index| ProfileSearchResponse {
            profiles: Vec::new(),
            next_cursor: Some(format!("cursor_{index}")),
        }));
    let service = ProfileGatewayService::new(
        profile_client.clone(),
        Arc::new(FakeFriendClient::default()),
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    let page = service
        .search_profiles("user".into(), None, 2)
        .await
        .unwrap();

    assert!(page.profiles.is_empty());
    assert_eq!(
        test_cursor_codec()
            .decode(
                page.next_cursor.as_deref(),
                "user",
                None,
                SearchSort::ProfilesUsernameAscV1,
            )
            .expect("decode public search cursor")
            .as_deref(),
        Some("cursor_20")
    );
    let calls = profile_client.search_calls.lock().unwrap();
    assert_eq!(calls.len(), 20);
    assert_eq!(calls.iter().map(|(_, _, limit)| limit).sum::<i32>(), 500);
    assert_eq!(calls[0].1, None);
    assert_eq!(calls[1].1.as_deref(), Some("cursor_1"));
}

#[tokio::test]
async fn profile_search_caps_the_last_batch_at_the_remaining_budget() {
    let profile_client = Arc::new(FakeProfileClient::with_profiles(Vec::new()));
    profile_client
        .search_pages
        .lock()
        .unwrap()
        .push_back(ProfileSearchResponse {
            profiles: vec![profile_response("user-1", 0, 0)],
            next_cursor: Some("cursor_1".into()),
        });
    profile_client
        .search_pages
        .lock()
        .unwrap()
        .extend((2..=10).map(|index| ProfileSearchResponse {
            profiles: Vec::new(),
            next_cursor: Some(format!("cursor_{index}")),
        }));
    let service = ProfileGatewayService::new(
        profile_client.clone(),
        Arc::new(FakeFriendClient::default()),
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    let page = service
        .search_profiles("user".into(), None, 100)
        .await
        .unwrap();

    assert_eq!(page.profiles.len(), 1);
    assert_eq!(
        test_cursor_codec()
            .decode(
                page.next_cursor.as_deref(),
                "user",
                None,
                SearchSort::ProfilesUsernameAscV1,
            )
            .expect("decode public search cursor")
            .as_deref(),
        Some("cursor_6")
    );
    let limits: Vec<_> = profile_client
        .search_calls
        .lock()
        .unwrap()
        .iter()
        .map(|(_, _, limit)| *limit)
        .collect();
    assert_eq!(limits, vec![100, 99, 99, 99, 99, 4]);
}

#[tokio::test]
async fn profile_search_stops_when_the_upstream_cursor_repeats() {
    let profile_client = Arc::new(FakeProfileClient::with_profiles(Vec::new()));
    let page = ProfileSearchResponse {
        profiles: vec![profile_response("user-1", 0, 0)],
        next_cursor: Some("user_1".into()),
    };
    profile_client
        .search_pages
        .lock()
        .unwrap()
        .extend([page.clone(), page]);
    let service = ProfileGatewayService::new(
        profile_client.clone(),
        Arc::new(FakeFriendClient::default()),
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    let page = service
        .search_profiles("user".into(), None, 2)
        .await
        .unwrap();

    assert_eq!(page.profiles.len(), 1);
    assert!(page.next_cursor.is_none());
    assert_eq!(profile_client.search_calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn profile_search_rejects_a_cursor_from_another_query_or_caller_before_rpc() {
    let profile_client = Arc::new(FakeProfileClient::with_profiles(Vec::new()));
    let service = ProfileGatewayService::new(
        profile_client.clone(),
        Arc::new(FakeFriendClient::default()),
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );
    let query_mismatch = test_cursor_codec()
        .encode(
            Some("user_1"),
            "other-query",
            Some("viewer-1"),
            SearchSort::ProfilesUsernameAscV1,
        )
        .expect("encode cursor")
        .expect("query-mismatch continuation");
    let caller_mismatch = test_cursor_codec()
        .encode(
            Some("user_1"),
            "user",
            Some("viewer-1"),
            SearchSort::ProfilesUsernameAscV1,
        )
        .expect("encode cursor")
        .expect("caller-mismatch continuation");

    for (viewer_id, query, cursor) in [
        ("viewer-1", "user", query_mismatch),
        ("viewer-2", "user", caller_mismatch),
    ] {
        let error = service
            .search_profiles_for_viewer(
                Some(viewer_id.to_owned()),
                query.to_owned(),
                Some(cursor),
                10,
            )
            .await
            .expect_err("query or caller mismatch must be rejected");

        assert!(matches!(
            error,
            api_gateway::application::service_error::GatewayServiceError::SearchCursor(_)
        ));
    }
    assert!(profile_client.search_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn profile_search_reissues_accepted_legacy_positions_as_signed_cursors() {
    let mut profiles = vec![
        profile_response("user-1", 0, 0),
        profile_response("user-2", 0, 0),
    ];
    profiles[0].username = "hana_ani".to_owned();
    profiles[1].username = "hana_mai".to_owned();
    let profile_client = Arc::new(FakeProfileClient::with_profiles(Vec::new()));
    profile_client
        .search_pages
        .lock()
        .unwrap()
        .push_back(ProfileSearchResponse {
            profiles,
            next_cursor: Some("hana_next".to_owned()),
        });
    let codec = test_cursor_codec_with_legacy_window();
    let service = ProfileGatewayService::new(
        profile_client.clone(),
        Arc::new(FakeFriendClient::default()),
        Arc::new(FakePostClient::default()),
        codec.clone(),
    );

    let page = service
        .search_profiles_for_viewer(
            Some("viewer-1".to_owned()),
            "user".to_owned(),
            Some("hana_old".to_owned()),
            1,
        )
        .await
        .expect("legacy continuation is accepted during migration");

    assert_eq!(
        profile_client.search_calls.lock().unwrap()[0].1.as_deref(),
        Some("hana_old")
    );
    let signed_cursor = page.next_cursor.expect("signed continuation");
    assert!(signed_cursor.starts_with("v1."));
    assert_eq!(
        codec
            .decode(
                Some(&signed_cursor),
                "user",
                Some("viewer-1"),
                SearchSort::ProfilesUsernameAscV1,
            )
            .expect("decode signed continuation")
            .as_deref(),
        Some("hana_ani"),
        "the signed continuation must preserve the page boundary"
    );
}

#[tokio::test]
async fn profile_gateway_reports_the_authoritative_post_count() {
    // The stored profile column is a permanent zero because nothing maintains
    // it, so a profile served with that value claims the author has no posts.
    // The count must come from the service that owns the posts instead.
    let profile_client = Arc::new(FakeProfileClient::with_profiles(vec![
        // Stored value is a stale zero; the real count is 7.
        profile_response_with_post_count("user-1", 1, 2, 0),
    ]));
    let post_client = Arc::new(FakePostClient::default());
    post_client
        .post_counts
        .lock()
        .unwrap()
        .insert("user-1".to_owned(), 7);
    let service = ProfileGatewayService::new(
        profile_client,
        Arc::new(FakeFriendClient::default()),
        post_client.clone(),
        test_cursor_codec(),
    );

    let profile = service
        .get_profile("user-1".to_owned())
        .await
        .expect("profile");

    assert_eq!(
        profile.post_count, 7,
        "the stored stale zero must be replaced"
    );
    assert_eq!(
        *post_client.post_count_calls.lock().unwrap(),
        vec![vec!["user-1".to_owned()]],
        "the count must be requested from the service that owns the posts"
    );
}

#[tokio::test]
async fn profile_gateway_does_not_turn_a_missing_count_into_zero() {
    // A missing observation is not a count of zero. Reporting one would state
    // that the author has no posts, which is a different claim from "the count
    // could not be read".
    let profile_client = Arc::new(FakeProfileClient::with_profiles(vec![
        profile_response_with_post_count("user-1", 1, 2, 5),
    ]));
    // No entry for user-1: the lookup produced no observation.
    let post_client = Arc::new(FakePostClient::default());
    let service = ProfileGatewayService::new(
        profile_client,
        Arc::new(FakeFriendClient::default()),
        post_client,
        test_cursor_codec(),
    );

    let profile = service
        .get_profile("user-1".to_owned())
        .await
        .expect("profile");

    assert_eq!(
        profile.post_count, 5,
        "a missing observation must not be replaced by a fabricated zero"
    );
}

#[tokio::test]
async fn profile_gateway_batches_post_counts_for_a_profile_list() {
    let profile_client = Arc::new(FakeProfileClient::with_profiles(vec![
        profile_response_with_post_count("user-1", 1, 2, 0),
        profile_response_with_post_count("user-2", 3, 4, 0),
    ]));
    let post_client = Arc::new(FakePostClient::default());
    {
        let mut counts = post_client.post_counts.lock().unwrap();
        counts.insert("user-1".to_owned(), 3);
        counts.insert("user-2".to_owned(), 9);
    }
    let service = ProfileGatewayService::new(
        profile_client,
        Arc::new(FakeFriendClient::default()),
        post_client.clone(),
        test_cursor_codec(),
    );

    let response = service
        .get_profiles(vec!["user-1".to_owned(), "user-2".to_owned()])
        .await
        .expect("profiles");

    assert_eq!(response.profiles[0].post_count, 3);
    assert_eq!(response.profiles[1].post_count, 9);
    // One batched request rather than one per profile.
    assert_eq!(post_client.post_count_calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn profile_gateway_forwards_remaining_profile_methods_and_merges_single_counts() {
    let profile_client = Arc::new(FakeProfileClient::with_profiles(vec![profile_response(
        "user-1", 1, 2,
    )]));
    let friend_client = Arc::new(FakeFriendClient::default());
    friend_client.counts_batch.lock().unwrap().insert(
        "user-1".to_owned(),
        FollowCountsResponse {
            follower_count: 30,
            following_count: 40,
        },
    );
    let service = ProfileGatewayService::new(
        profile_client.clone(),
        friend_client,
        Arc::new(FakePostClient::default()),
        test_cursor_codec(),
    );

    let profile = service
        .get_profile("user-1".to_owned())
        .await
        .expect("get profile");
    let by_username = service
        .get_profile_by_username("user-1".to_owned())
        .await
        .expect("get profile by username");
    let cursor = test_cursor_codec()
        .encode(
            Some("cursor"),
            "user",
            None,
            SearchSort::ProfilesUsernameAscV1,
        )
        .expect("encode public search cursor");
    let search = service
        .search_profiles("user".to_owned(), cursor, 10)
        .await
        .expect("search profiles");
    assert_eq!(
        profile_client.search_calls.lock().unwrap()[0].1.as_deref(),
        Some("cursor")
    );
    let updated = service
        .update_profile(
            "user-1".to_owned(),
            ProfileUpdateRequest {
                username: "user-1".to_owned(),
                nickname: "User".to_owned(),
                avatar_url: "/avatar.png".to_owned(),
                bio: "bio".to_owned(),
            },
        )
        .await
        .expect("update profile");
    let avatar_url = service
        .upload_avatar(
            "user-1".to_owned(),
            b"png".to_vec(),
            "image/png".to_owned(),
            "avatar.png".to_owned(),
        )
        .await
        .expect("upload avatar");
    let avatar = service
        .get_avatar("user-1".to_owned())
        .await
        .expect("get avatar");
    let deleted = service
        .delete_profile(
            "user-1".to_owned(),
            Some("req-1".to_owned()),
            Some("127.0.0.1".to_owned()),
            Some("unit-test".to_owned()),
        )
        .await
        .expect("delete profile");

    assert_eq!(profile.follower_count, 30);
    assert_eq!(profile.following_count, 40);
    assert_eq!(by_username.follower_count, 30);
    assert_eq!(search.profiles[0].follower_count, 30);
    assert_eq!(updated.following_count, 40);
    assert_eq!(avatar_url, "/avatar.png");
    assert_eq!(avatar.content_type, "image/png");
    assert!(deleted.deleted);
}

#[tokio::test]
async fn post_gateway_forwards_delete_metadata_and_returns_client_response() {
    let client = Arc::new(FakePostClient::default());
    let service = PostGatewayService::new(client.clone(), test_cursor_codec());

    let response = service
        .delete_post(
            "user-1".to_owned(),
            "post-1".to_owned(),
            Some("req-1".to_owned()),
            Some("127.0.0.1".to_owned()),
            Some("unit-test".to_owned()),
        )
        .await
        .expect("delete post");

    assert!(response.deleted);
    assert_eq!(
        *client.delete_post_calls.lock().unwrap(),
        vec![DeletePostCall {
            user_id: "user-1".to_owned(),
            post_id: "post-1".to_owned(),
            request_id: Some("req-1".to_owned()),
            ip_address: Some("127.0.0.1".to_owned()),
            user_agent: Some("unit-test".to_owned()),
        }]
    );
}

#[tokio::test]
async fn post_search_cursor_binds_query_and_returns_only_internal_position_to_post_service() {
    let client = Arc::new(FakePostClient::default());
    *client.search_next_cursor.lock().unwrap() = Some(POST_SEARCH_POSITION.to_owned());
    let service = PostGatewayService::new(client.clone(), test_cursor_codec());

    let first = service
        .search_posts("viewer-1".into(), "  Hello  ".into(), None, 10)
        .await
        .expect("first search page");
    let public_cursor = first.next_cursor.expect("public continuation");
    assert_eq!(
        test_cursor_codec()
            .decode(
                Some(&public_cursor),
                "hello",
                Some("viewer-1"),
                SearchSort::PostsCreatedAtDescIdDescV1,
            )
            .expect("decode continuation")
            .as_deref(),
        Some(POST_SEARCH_POSITION)
    );

    *client.search_next_cursor.lock().unwrap() = None;
    service
        .search_posts("viewer-1".into(), "HELLO".into(), Some(public_cursor), 5)
        .await
        .expect("second search page");

    let calls = client.search_calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].query, "  Hello  ");
    assert_eq!(calls[0].cursor, None);
    assert_eq!(calls[0].limit, 10);
    assert_eq!(calls[1].query, "HELLO");
    assert_eq!(calls[1].cursor.as_deref(), Some(POST_SEARCH_POSITION));
    assert_eq!(calls[1].limit, 5);
}

#[tokio::test]
async fn post_search_rejects_a_cursor_reused_for_another_query_before_rpc() {
    let client = Arc::new(FakePostClient::default());
    let service = PostGatewayService::new(client.clone(), test_cursor_codec());
    let cursor = test_cursor_codec()
        .encode(
            Some(POST_SEARCH_POSITION),
            "original",
            Some("viewer-1"),
            SearchSort::PostsCreatedAtDescIdDescV1,
        )
        .expect("encode cursor");

    let error = service
        .search_posts("viewer-1".into(), "different".into(), cursor, 10)
        .await
        .expect_err("query mismatch must be rejected");

    assert!(matches!(
        error,
        api_gateway::application::service_error::GatewayServiceError::SearchCursor(_)
    ));
    assert!(client.search_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn post_gateway_forwards_ranked_feed_context_and_feedback() {
    let client = Arc::new(FakePostClient::default());
    let service = PostGatewayService::new(client.clone(), test_cursor_codec());

    let feed = service
        .get_ranked_feed(
            "user-1".to_owned(),
            Some("cursor".to_owned()),
            20,
            "request-1".to_owned(),
            None,
        )
        .await
        .expect("ranked feed");
    let accepted = service
        .record_feed_recommendation_event(
            "user-1".to_owned(),
            "request-1".to_owned(),
            FeedRecommendationEventRequest {
                post_id: "post-1".to_owned(),
                impression_id: "11111111-1111-1111-1111-111111111111".to_owned(),
                action: "LIKE".to_owned(),
                source: "MATERIALIZED".to_owned(),
                model_version: "feed-deterministic-v1".to_owned(),
                position: Some(0),
                propensity_micros: Some(1_000_000),
                dwell_ms: None,
                metadata: json!({"surface": "home"}),
            },
        )
        .await
        .expect("record feed event");

    assert_eq!(feed.request_id.as_deref(), Some("request-1"));
    assert!(accepted.accepted);
    assert_eq!(client.ranked_feed_calls.lock().unwrap().len(), 1);
    assert_eq!(client.feed_event_calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn post_gateway_forwards_read_and_mutation_methods() {
    let client = Arc::new(FakePostClient::default());
    let service = PostGatewayService::new(client, test_cursor_codec());

    let feed = service
        .get_feed("user-1".to_owned(), Some("cursor".to_owned()), 20)
        .await
        .expect("feed");
    let author_posts = service
        .get_author_posts("author-1".to_owned(), "viewer-1".to_owned(), None, 10)
        .await
        .expect("author posts");
    let search = service
        .search_posts("viewer-1".to_owned(), "hello".to_owned(), None, 10)
        .await
        .expect("search posts");
    let trending = service
        .get_trending("viewer-1".to_owned(), None, 10)
        .await
        .expect("trending");
    let detail = service
        .get_post("viewer-1".to_owned(), "post-1".to_owned())
        .await
        .expect("post detail");
    let created = service
        .create_post(
            "user-1".to_owned(),
            PostCreateRequest {
                content: "hello".to_owned(),
                visibility: "PUBLIC".to_owned(),
            },
        )
        .await
        .expect("create post");
    let comment = service
        .create_comment(
            "user-1".to_owned(),
            "post-1".to_owned(),
            CommentCreateRequest {
                content: "comment".to_owned(),
            },
        )
        .await
        .expect("create comment");
    let liked = service
        .like_post("user-1".to_owned(), "post-1".to_owned())
        .await
        .expect("like post");
    let unliked = service
        .unlike_post("user-1".to_owned(), "post-1".to_owned())
        .await
        .expect("unlike post");

    assert!(feed.posts.is_empty());
    assert!(author_posts.posts.is_empty());
    assert!(search.posts.is_empty());
    assert!(trending.posts.is_empty());
    assert_eq!(detail.post.id, "post-1");
    assert_eq!(created.content, "hello");
    assert_eq!(comment.content, "comment");
    assert!(liked.liked);
    assert!(!unliked.liked);
}

#[tokio::test]
async fn auth_gateway_forwards_oauth_callback_context() {
    let client = Arc::new(FakeAuthClient::default());
    let service = AuthGatewayService::new(client.clone());

    let response = service
        .oauth_callback(
            IdentityProvider::Google.into(),
            "code-1".to_owned(),
            "state-1".to_owned(),
            Some("10.0.0.1".to_owned()),
            Some("browser".to_owned()),
        )
        .await
        .expect("oauth callback");

    assert_eq!(response.access_token, "access");
    assert_eq!(
        *client.oauth_callback_calls.lock().unwrap(),
        vec![OAuthCallbackCall {
            provider: IdentityProvider::Google,
            code: "code-1".to_owned(),
            state: "state-1".to_owned(),
            ip_address: Some("10.0.0.1".to_owned()),
            user_agent: Some("browser".to_owned()),
        }]
    );
}

#[tokio::test]
async fn auth_gateway_forwards_core_auth_methods() {
    let client = Arc::new(FakeAuthClient::default());
    let service = AuthGatewayService::new(client);

    let signup = service
        .signup(EmailSignupRequest {
            email: "user@example.com".to_owned(),
            password: "password".to_owned(),
        })
        .await
        .expect("signup");
    let verify = service
        .verify(EmailVerifyRequest {
            email: "user@example.com".to_owned(),
            code: "123456".to_owned(),
        })
        .await
        .expect("verify");
    let email = service
        .check_email(EmailCheckRequest {
            email: "user@example.com".to_owned(),
        })
        .await
        .expect("check email");
    let reset = service
        .request_password_reset(PasswordResetRequest {
            email: "user@example.com".to_owned(),
        })
        .await
        .expect("request reset");
    let confirmed = service
        .confirm_password_reset(PasswordResetConfirmRequest {
            token: "token".to_owned(),
            new_password: "password".to_owned(),
        })
        .await
        .expect("confirm reset");
    let login = service
        .login(
            LoginRequest {
                email: "user@example.com".to_owned(),
                password: "password".to_owned(),
            },
            Some("127.0.0.1".to_owned()),
            Some("unit-test".to_owned()),
        )
        .await
        .expect("login");
    let refreshed = service
        .refresh("refresh".to_owned())
        .await
        .expect("refresh");
    service.logout("refresh".to_owned()).await.expect("logout");
    let authorize = service
        .authorize(IdentityProvider::Google.into())
        .await
        .expect("authorize");
    let linked = service
        .link_provider(
            IdentityProvider::Google.into(),
            OAuthLinkRequest {
                code: "code".to_owned(),
                state: "state".to_owned(),
            },
            "user-1".to_owned(),
        )
        .await
        .expect("link provider");

    assert_eq!(signup.status, "OK");
    assert_eq!(verify.status, "VERIFIED");
    assert!(email.available);
    assert_eq!(reset.status, "SENT");
    assert_eq!(confirmed.status, "RESET");
    assert_eq!(login.access_token, "access");
    assert_eq!(refreshed.refresh_token, "refresh");
    assert_eq!(authorize.authorization_uri, "https://auth.example/oauth");
    assert_eq!(linked.providers, vec![IdentityProvider::Google]);
}

#[tokio::test]
async fn friend_and_notification_gateways_forward_requests() {
    let friend_client = Arc::new(FakeFriendClient::default());
    let friend_service = FriendGatewayService::new(friend_client.clone());
    let notification_client = Arc::new(FakeNotificationClient::default());
    let notification_service = NotificationGatewayService::new(
        notification_client.clone(),
        Arc::new(FakePostClient::default()),
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    );

    let follow = friend_service
        .follow("user-1".to_owned(), "user-2".to_owned())
        .await
        .expect("follow");
    let notifications = notification_service
        .list_notifications("user-1".to_owned(), Some("cursor".to_owned()), 25)
        .await
        .expect("notifications");

    assert!(follow.following);
    assert!(notifications.items.is_empty());
    assert_eq!(
        *friend_client.follow_calls.lock().unwrap(),
        vec![("user-1".to_owned(), "user-2".to_owned())]
    );
    assert_eq!(
        *notification_client.list_calls.lock().unwrap(),
        vec![("user-1".to_owned(), Some("cursor".to_owned()), 25)]
    );
}

#[tokio::test]
async fn friend_gateway_forwards_follow_request_and_relationship_operations() {
    let client = Arc::new(FakeFriendClient::default());
    let service = FriendGatewayService::new(client.clone());

    let accepted = service
        .accept_follow_request("request-1".into(), "user-2".into())
        .await
        .expect("accept request");
    let rejected = service
        .reject_follow_request("request-2".into(), "user-2".into())
        .await
        .expect("reject request");
    let cancelled = service
        .cancel_follow_request("request-3".into(), "user-1".into())
        .await
        .expect("cancel request");
    let incoming = service
        .list_incoming_follow_requests("user-2".into(), Some("cursor".into()), 25)
        .await
        .expect("incoming requests");
    let outgoing = service
        .list_outgoing_follow_requests("user-1".into(), None, 10)
        .await
        .expect("outgoing requests");
    let blocked = service
        .block("user-1".into(), "user-2".into())
        .await
        .expect("block user");
    let unblocked = service
        .unblock("user-1".into(), "user-2".into())
        .await
        .expect("unblock user");
    let muted = service
        .mute("user-1".into(), "user-2".into())
        .await
        .expect("mute user");
    let unmuted = service
        .unmute("user-1".into(), "user-2".into())
        .await
        .expect("unmute user");
    let relationship = service
        .relationship("user-1".into(), "user-2".into())
        .await
        .expect("relationship");

    assert!(accepted.following);
    assert_eq!(rejected.status, "REJECTED");
    assert_eq!(cancelled.status, "CANCELLED");
    assert_eq!(incoming.requests.len(), 1);
    assert_eq!(outgoing.requests.len(), 1);
    assert!(blocked.blocked_by_viewer);
    assert!(!unblocked.blocked_by_viewer);
    assert!(muted.muted_by_viewer);
    assert!(!unmuted.muted_by_viewer);
    assert!(relationship.following);
    assert_eq!(
        *client.follow_request_calls.lock().unwrap(),
        vec![
            ("accept".into(), "request-1".into(), "user-2".into()),
            ("reject".into(), "request-2".into(), "user-2".into()),
            ("cancel".into(), "request-3".into(), "user-1".into()),
        ]
    );
    assert_eq!(
        *client.follow_request_list_calls.lock().unwrap(),
        vec![
            (
                "incoming".into(),
                "user-2".into(),
                Some("cursor".into()),
                25,
            ),
            ("outgoing".into(), "user-1".into(), None, 10),
        ]
    );
    assert_eq!(client.relationship_calls.lock().unwrap().len(), 5);
}

#[tokio::test]
async fn friend_and_notification_gateways_forward_remaining_methods() {
    let friend_client = Arc::new(FakeFriendClient::default());
    let friend_service = FriendGatewayService::new(friend_client);
    let notification_client = Arc::new(FakeNotificationClient::default());
    let notification_service = NotificationGatewayService::new(
        notification_client,
        Arc::new(FakePostClient::default()),
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    );

    let unfollow = friend_service
        .unfollow("user-1".to_owned(), "user-2".to_owned())
        .await
        .expect("unfollow");
    let followers = friend_service
        .list_followers("user-1".to_owned(), None, Some(20))
        .await
        .expect("followers");
    let following = friend_service
        .list_following("user-1".to_owned(), None, Some(20))
        .await
        .expect("following");
    let counts = friend_service
        .counts("user-1".to_owned())
        .await
        .expect("counts");
    let status = friend_service
        .status("user-1".to_owned(), "user-2".to_owned())
        .await
        .expect("status");
    let marked = notification_service
        .mark_read("user-1".to_owned(), "notification-1".to_owned())
        .await
        .expect("mark read");
    let mark_all = notification_service
        .mark_all_read("user-1".to_owned())
        .await
        .expect("mark all read");

    assert!(!unfollow.following);
    assert!(followers.edges.is_empty());
    assert!(following.edges.is_empty());
    assert_eq!(counts.follower_count, 0);
    assert!(status.following);
    assert_eq!(marked.id, "notification-1");
    assert_eq!(mark_all.updated_count, 1);
}

#[tokio::test]
async fn governance_services_save_and_return_missing_errors() {
    let route_repo = Arc::new(FakeRouteContractRepository::default());
    let route_service = RouteContractService::new(route_repo.clone());
    let policy_repo = Arc::new(FakePolicyRepository::default());
    let rollout_repo = Arc::new(FakeRolloutRepository::default());
    let runtime = ScriptedRuntime::new();
    let policy_service =
        PolicyService::new(policy_repo.clone(), rollout_repo.clone(), runtime.clone());
    let profile_repo = Arc::new(FakeClientProfileRepository::default());
    let client_profile_service = ClientProfileService::new(profile_repo.clone());

    let saved_contract = route_service
        .create_contract(route_contract("contract-1").into())
        .await
        .expect("save contract");
    let missing_contract = route_service
        .get_contract("missing")
        .await
        .expect_err("missing route contract");
    let saved_policy = policy_service
        .create_policy(policy("policy-1").into())
        .await
        .expect("save policy");
    let saved_rollout = policy_service
        .create_rollout(
            RolloutPolicy {
                id: String::new(),
                contract_id: "contract-1".to_owned(),
                version: "v1".to_owned(),
                strategy: "percentage".to_owned(),
                status: "ACTIVE".to_owned(),
                percent: Some(10),
            }
            .into(),
        )
        .await
        .expect("save rollout");
    let saved_profile = client_profile_service
        .create_profile(client_profile("client-1").into())
        .await
        .expect("save client profile");

    assert_eq!(saved_contract.id, "contract-1");
    assert_eq!(missing_contract.to_string(), "Route contract not found");
    assert_eq!(saved_policy.id, "policy-1");
    assert!(!saved_rollout.id.is_empty());
    assert_eq!(saved_rollout.id, runtime.id.to_string());
    assert_eq!(runtime.id_reads.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.utc_reads.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.monotonic_reads.load(Ordering::SeqCst), 0);
    assert_eq!(saved_profile.id, "client-1");
}

#[tokio::test]
async fn governance_services_read_existing_versions_rollouts_and_profiles() {
    let route_repo = Arc::new(FakeRouteContractRepository::default());
    let route_service = RouteContractService::new(route_repo.clone());
    let policy_repo = Arc::new(FakePolicyRepository::default());
    let rollout_repo = Arc::new(FakeRolloutRepository::default());
    let runtime = ScriptedRuntime::new();
    let policy_service =
        PolicyService::new(policy_repo.clone(), rollout_repo.clone(), runtime.clone());
    let profile_repo = Arc::new(FakeClientProfileRepository::default());
    let client_profile_service = ClientProfileService::new(profile_repo.clone());

    route_service
        .create_contract(route_contract("contract-2").into())
        .await
        .expect("save contract");
    route_service
        .create_version(
            "contract-2",
            RouteContractVersion {
                version: "v2".to_owned(),
                status: "ACTIVE".to_owned(),
                deprecation_date: None,
                response_schema_ref: "schema://v2".to_owned(),
            }
            .into(),
        )
        .await
        .expect("save version");
    policy_service
        .create_policy(policy("policy-2").into())
        .await
        .expect("save policy");
    let rollout = policy_service
        .create_rollout(
            RolloutPolicy {
                id: "rollout-1".to_owned(),
                contract_id: "contract-2".to_owned(),
                version: "v2".to_owned(),
                strategy: "blue_green".to_owned(),
                status: "PAUSED".to_owned(),
                percent: None,
            }
            .into(),
        )
        .await
        .expect("save rollout");
    client_profile_service
        .create_profile(client_profile("client-2").into())
        .await
        .expect("save client");

    let contracts = route_service.list_contracts().await.expect("contracts");
    let contract = route_service
        .get_contract("contract-2")
        .await
        .expect("contract");
    let version = route_service
        .get_version("contract-2", "v2")
        .await
        .expect("version");
    let policies = policy_service.list_policies().await.expect("policies");
    let policy = policy_service.get_policy("policy-2").await.expect("policy");
    let default_policy = policy_service
        .default_policy()
        .await
        .expect("default policy")
        .expect("policy exists");
    let stored_rollout = policy_service
        .get_rollout(&rollout.id)
        .await
        .expect("rollout");
    let client_profiles = client_profile_service
        .list_profiles()
        .await
        .expect("client profiles");
    let client_profile = client_profile_service
        .get_profile("client-2")
        .await
        .expect("client profile");

    assert_eq!(contracts.len(), 1);
    assert_eq!(contract.id, "contract-2");
    assert_eq!(version.version, "v2");
    assert_eq!(policies.len(), 1);
    assert_eq!(policy.id, "policy-2");
    assert_eq!(default_policy.id, "policy-2");
    assert_eq!(stored_rollout.id, "rollout-1");
    assert_eq!(runtime.id_reads.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.utc_reads.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.monotonic_reads.load(Ordering::SeqCst), 0);
    assert_eq!(client_profiles.len(), 1);
    assert_eq!(client_profile.id, "client-2");
}

#[derive(Default)]
struct FakeAggregationRepository {
    recipe: Mutex<Option<AggregationRecipe>>,
    find_by_route_key_calls: Mutex<Vec<String>>,
}

impl FakeAggregationRepository {
    fn with_recipe(recipe: AggregationRecipe) -> Self {
        Self {
            recipe: Mutex::new(Some(recipe)),
            ..Self::default()
        }
    }
}

impl FakeAggregationRepository {
    async fn find_all_infrastructure_inner(&self) -> Result<Vec<AggregationRecipe>> {
        Ok(self.recipe.lock().unwrap().clone().into_iter().collect())
    }

    async fn find_by_route_key_infrastructure_inner(
        &self,
        route_key: &str,
    ) -> Result<Option<AggregationRecipe>> {
        self.find_by_route_key_calls
            .lock()
            .unwrap()
            .push(route_key.to_owned());
        Ok(self.recipe.lock().unwrap().clone())
    }
}

#[async_trait]
impl AggregationRecipeRepositoryPort for FakeAggregationRepository {
    async fn find_all(
        &self,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Vec<api_gateway::application::dto::infrastructure_dto::AggregationRecipeDto>,
    > {
        self.find_all_infrastructure_inner()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn find_by_route_key(
        &self,
        route_key: &str,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Option<api_gateway::application::dto::infrastructure_dto::AggregationRecipeDto>,
    > {
        self.find_by_route_key_infrastructure_inner(route_key)
            .await
            .map(|value| value.map(Into::into))
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

#[derive(Default)]
struct FakeResponseCache {
    cached: Mutex<Option<Value>>,
    get_calls: Mutex<Vec<String>>,
    put_calls: Mutex<Vec<PutCacheCall>>,
}

impl FakeResponseCache {
    fn with_cached(payload: Value) -> Self {
        Self {
            cached: Mutex::new(Some(payload)),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PutCacheCall {
    cache_key: String,
    payload: Value,
    ttl: Duration,
}

impl FakeResponseCache {
    async fn get_infrastructure_inner(&self, cache_key: &str) -> Result<Option<Value>> {
        self.get_calls.lock().unwrap().push(cache_key.to_owned());
        Ok(self.cached.lock().unwrap().clone())
    }

    async fn put_infrastructure_inner(
        &self,
        cache_key: &str,
        payload: Value,
        ttl: Duration,
    ) -> Result<()> {
        self.put_calls.lock().unwrap().push(PutCacheCall {
            cache_key: cache_key.to_owned(),
            payload,
            ttl,
        });
        Ok(())
    }
}

#[async_trait]
impl ResponseCachePort for FakeResponseCache {
    async fn get(
        &self,
        cache_key: &str,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Option<api_gateway::application::dto::cache_dto::CachePayloadDto>,
    > {
        self.get_infrastructure_inner(cache_key)
            .await
            .map(|value| value.map(Into::into))
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn put(
        &self,
        cache_key: &str,
        payload: api_gateway::application::dto::cache_dto::CachePayloadDto,
        ttl: Duration,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<()> {
        self.put_infrastructure_inner(cache_key, payload.into(), ttl)
            .await
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

struct FakeTelemetryRepository {
    records: Mutex<Vec<TelemetryRecord>>,
    summary: Mutex<TelemetrySummary>,
}

impl Default for FakeTelemetryRepository {
    fn default() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            summary: Mutex::new(TelemetrySummary {
                total_requests: 0,
                error_rate: 0.0,
                p95_latency_ms: 0,
            }),
        }
    }
}

impl FakeTelemetryRepository {
    async fn record_infrastructure_inner(&self, record: TelemetryRecord) -> Result<()> {
        self.records.lock().unwrap().push(record);
        Ok(())
    }

    async fn summary_infrastructure_inner(&self) -> Result<TelemetrySummary> {
        Ok(self.summary.lock().unwrap().clone())
    }
}

#[async_trait]
impl TelemetryRepositoryPort for FakeTelemetryRepository {
    async fn record(
        &self,
        record: TelemetryRecord,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<()> {
        self.record_infrastructure_inner(record).await.map_err(
            api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
        )
    }
    async fn summary(
        &self,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::TelemetrySummaryDto,
    > {
        self.summary_infrastructure_inner()
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

struct FakeProfileClient {
    profiles: Mutex<Vec<ProfileResponse>>,
    /// Scripted search pages returned in order, one per call.
    ///
    /// `profiles` answers every search with the same list, which cannot express a
    /// scan that advances. A queue can, so the multi-batch path is testable, along
    /// with the requests the scan actually issued.
    search_pages: Mutex<std::collections::VecDeque<ProfileSearchResponse>>,
    search_calls: Mutex<Vec<(String, Option<String>, i32)>>,
}

impl FakeProfileClient {
    fn with_profiles(profiles: Vec<ProfileResponse>) -> Self {
        Self {
            profiles: Mutex::new(profiles),
            search_pages: Mutex::new(std::collections::VecDeque::new()),
            search_calls: Mutex::new(Vec::new()),
        }
    }
}

impl FakeProfileClient {
    async fn get_profile_profile_client_port_inner(
        &self,
        user_id: String,
    ) -> Result<ProfileResponse> {
        // Return the configured fixture when one exists. Synthesizing a blank
        // profile instead would make a single-profile test measure the fake
        // rather than the merge under test.
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.user_id == user_id)
            .cloned()
            .unwrap_or_else(|| profile_response(&user_id, 0, 0)))
    }

    async fn get_profile_by_username_profile_client_port_inner(
        &self,
        username: String,
    ) -> Result<ProfileResponse> {
        Ok(profile_response(&username, 0, 0))
    }

    async fn get_profiles_profile_client_port_inner(
        &self,
        user_ids: Vec<String>,
    ) -> Result<ProfileBatchResponse> {
        Ok(ProfileBatchResponse {
            profiles: self
                .profiles
                .lock()
                .unwrap()
                .iter()
                .filter(|profile| user_ids.contains(&profile.user_id))
                .cloned()
                .collect(),
        })
    }

    async fn search_profiles_profile_client_port_inner(
        &self,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> Result<ProfileSearchResponse> {
        self.search_calls
            .lock()
            .unwrap()
            .push((query, cursor, limit));
        // A scripted queue takes precedence: it is the only fixture that can show
        // a scan advancing, because one stored list answers every request.
        if let Some(page) = self.search_pages.lock().unwrap().pop_front() {
            return Ok(page);
        }
        Ok(ProfileSearchResponse {
            profiles: self.profiles.lock().unwrap().clone(),
            next_cursor: None,
        })
    }

    async fn upload_avatar_profile_client_port_inner(
        &self,
        _user_id: String,
        _content: Vec<u8>,
        _content_type: String,
        _filename: String,
    ) -> Result<String> {
        Ok("/avatar.png".to_owned())
    }

    async fn get_avatar_profile_client_port_inner(
        &self,
        _user_id: String,
    ) -> Result<AvatarContent> {
        Ok(AvatarContent {
            content: Vec::new(),
            content_type: "image/png".to_owned(),
        })
    }

    async fn update_profile_profile_client_port_inner(
        &self,
        user_id: String,
        _request: ProfileUpdateRequest,
    ) -> Result<ProfileResponse> {
        Ok(profile_response(&user_id, 0, 0))
    }

    async fn delete_profile_profile_client_port_inner(
        &self,
        user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> Result<DeleteProfileResponse> {
        Ok(DeleteProfileResponse {
            user_id,
            deleted: true,
            account_deleted: false,
        })
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn get_mutation_receipt_status_profile_client_port_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Profile mutation status lookup is unavailable")
    }

    async fn update_profile_privacy_profile_client_port_inner(
        &self,
        _user_id: String,
        _request: api_gateway::application::dto::profile_dto::ProfilePrivacyUpdateRequest,
    ) -> anyhow::Result<api_gateway::application::dto::profile_dto::ProfileResponse> {
        anyhow::bail!("Profile privacy operation is not implemented")
    }
}

#[async_trait]
impl ProfileClientPort for FakeProfileClient {
    async fn get_profile(
        &self,
        user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.get_profile_profile_client_port_inner(user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_profile_by_username(
        &self,
        username: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.get_profile_by_username_profile_client_port_inner(username)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_profiles(
        &self,
        user_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileBatchResponse> {
        self.get_profiles_profile_client_port_inner(user_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn search_profiles(
        &self,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileSearchResponse> {
        self.search_profiles_profile_client_port_inner(query, cursor, limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn upload_avatar(
        &self,
        _user_id: String,
        _content: Vec<u8>,
        _content_type: String,
        _filename: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<String> {
        self.upload_avatar_profile_client_port_inner(_user_id, _content, _content_type, _filename)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_avatar(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<AvatarContent> {
        self.get_avatar_profile_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn update_profile(
        &self,
        user_id: String,
        _request: ProfileUpdateRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<ProfileResponse> {
        self.update_profile_profile_client_port_inner(user_id, _request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn delete_profile(
        &self,
        user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<DeleteProfileResponse> {
        self.delete_profile_profile_client_port_inner(
            user_id,
            _request_id,
            _ip_address,
            _user_agent,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_profile_client_port_inner(_operation_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn update_profile_privacy(
        &self,
        _user_id: String,
        _request: api_gateway::application::dto::profile_dto::ProfilePrivacyUpdateRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::profile_dto::ProfileResponse,
    > {
        self.update_profile_privacy_profile_client_port_inner(_user_id, _request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

type FollowRequestListCall = (String, String, Option<String>, i32);

#[derive(Default)]
struct FakeFriendClient {
    counts_batch: Mutex<HashMap<String, FollowCountsResponse>>,
    get_counts_batch_calls: Mutex<Vec<Vec<String>>>,
    follow_calls: Mutex<Vec<(String, String)>>,
    follow_request_calls: Mutex<Vec<(String, String, String)>>,
    follow_request_list_calls: Mutex<Vec<FollowRequestListCall>>,
    relationship_calls: Mutex<Vec<(String, String, String)>>,
    relationship_policies: Mutex<HashMap<(String, String), RelationshipPolicyResponse>>,
    missing_relationship_batch: Mutex<HashSet<(String, String)>>,
}

impl FakeFriendClient {
    async fn follow_friend_client_port_inner(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> Result<FollowActionResponse> {
        self.follow_calls
            .lock()
            .unwrap()
            .push((follower_id, followee_id));
        Ok(FollowActionResponse {
            following: true,
            followed_at: Some("now".to_owned()),
            status: "ACTIVE".into(),
            request_id: None,
        })
    }

    async fn unfollow_friend_client_port_inner(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> Result<FollowActionResponse> {
        Ok(FollowActionResponse {
            following: false,
            followed_at: None,
            status: "INACTIVE".into(),
            request_id: None,
        })
    }

    async fn list_followers_friend_client_port_inner(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: Option<i32>,
    ) -> Result<FollowListResponse> {
        Ok(FollowListResponse {
            edges: Vec::new(),
            next_cursor: None,
            total_count: 0,
        })
    }

    async fn list_following_friend_client_port_inner(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: Option<i32>,
    ) -> Result<FollowListResponse> {
        Ok(FollowListResponse {
            edges: Vec::new(),
            next_cursor: None,
            total_count: 0,
        })
    }

    async fn get_counts_friend_client_port_inner(
        &self,
        user_id: String,
    ) -> Result<FollowCountsResponse> {
        Ok(self
            .counts_batch
            .lock()
            .unwrap()
            .get(&user_id)
            .copied()
            .unwrap_or(FollowCountsResponse {
                follower_count: 0,
                following_count: 0,
            }))
    }

    async fn get_counts_batch_friend_client_port_inner(
        &self,
        user_ids: Vec<String>,
    ) -> Result<HashMap<String, FollowCountsResponse>> {
        self.get_counts_batch_calls.lock().unwrap().push(user_ids);
        Ok(self.counts_batch.lock().unwrap().clone())
    }

    async fn check_following_friend_client_port_inner(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> Result<FollowStatusResponse> {
        Ok(FollowStatusResponse { following: true })
    }

    async fn accept_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        actor_id: String,
    ) -> Result<FollowActionResponse> {
        self.follow_request_calls
            .lock()
            .unwrap()
            .push(("accept".into(), request_id, actor_id));
        Ok(FollowActionResponse {
            following: true,
            followed_at: Some("now".into()),
            status: "ACTIVE".into(),
            request_id: None,
        })
    }

    async fn reject_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        actor_id: String,
    ) -> Result<FollowRequestResponse> {
        self.follow_request_calls.lock().unwrap().push((
            "reject".into(),
            request_id.clone(),
            actor_id,
        ));
        Ok(follow_request_response(&request_id, "REJECTED"))
    }

    async fn cancel_follow_request_friend_client_port_inner(
        &self,
        request_id: String,
        actor_id: String,
    ) -> Result<FollowRequestResponse> {
        self.follow_request_calls.lock().unwrap().push((
            "cancel".into(),
            request_id.clone(),
            actor_id,
        ));
        Ok(follow_request_response(&request_id, "CANCELLED"))
    }

    async fn list_incoming_follow_requests_friend_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> Result<FollowRequestPageResponse> {
        self.follow_request_list_calls.lock().unwrap().push((
            "incoming".into(),
            user_id,
            cursor.clone(),
            limit,
        ));
        Ok(FollowRequestPageResponse {
            requests: vec![follow_request_response("request-in", "PENDING")],
            next_cursor: cursor,
        })
    }

    async fn list_outgoing_follow_requests_friend_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> Result<FollowRequestPageResponse> {
        self.follow_request_list_calls.lock().unwrap().push((
            "outgoing".into(),
            user_id,
            cursor.clone(),
            limit,
        ));
        Ok(FollowRequestPageResponse {
            requests: vec![follow_request_response("request-out", "PENDING")],
            next_cursor: cursor,
        })
    }

    async fn block_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(("block".into(), viewer_id, target_id));
        Ok(RelationshipPolicyResponse {
            blocked_by_viewer: true,
            ..RelationshipPolicyResponse::default()
        })
    }

    async fn unblock_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(("unblock".into(), viewer_id, target_id));
        Ok(RelationshipPolicyResponse::default())
    }

    async fn mute_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(("mute".into(), viewer_id, target_id));
        Ok(RelationshipPolicyResponse {
            muted_by_viewer: true,
            ..RelationshipPolicyResponse::default()
        })
    }

    async fn unmute_user_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> Result<RelationshipPolicyResponse> {
        self.relationship_calls
            .lock()
            .unwrap()
            .push(("unmute".into(), viewer_id, target_id));
        Ok(RelationshipPolicyResponse::default())
    }

    async fn get_relationship_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> Result<RelationshipPolicyResponse> {
        self.relationship_calls.lock().unwrap().push((
            "get".into(),
            viewer_id.clone(),
            target_id.clone(),
        ));
        Ok(self
            .relationship_policies
            .lock()
            .unwrap()
            .get(&(viewer_id, target_id))
            .copied()
            .unwrap_or(RelationshipPolicyResponse {
                following: true,
                ..RelationshipPolicyResponse::default()
            }))
    }

    async fn get_relationships_batch_friend_client_port_inner(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> Result<HashMap<String, RelationshipPolicyResponse>> {
        let policies = self.relationship_policies.lock().unwrap();
        let missing = self.missing_relationship_batch.lock().unwrap();
        Ok(target_ids
            .into_iter()
            .filter(|target_id| !missing.contains(&(viewer_id.clone(), target_id.clone())))
            .map(|target_id| {
                let policy = policies
                    .get(&(viewer_id.clone(), target_id.clone()))
                    .copied()
                    .unwrap_or_default();
                (target_id, policy)
            })
            .collect())
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn get_mutation_receipt_status_friend_client_port_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Friend mutation status lookup is unavailable")
    }

    async fn get_people_recommendations_friend_client_port_inner(
        &self,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
        _request_id: String,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::PeopleRecommendationPageResponse>
    {
        anyhow::bail!("People recommendations are not implemented")
    }

    async fn record_people_recommendation_event_friend_client_port_inner(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: api_gateway::application::dto::friend_dto::PeopleRecommendationEventRequest,
    ) -> anyhow::Result<api_gateway::application::dto::friend_dto::RecommendationEventResponse>
    {
        anyhow::bail!("People recommendation events are not implemented")
    }
}

#[async_trait]
impl FriendClientPort for FakeFriendClient {
    async fn follow(
        &self,
        follower_id: String,
        followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.follow_friend_client_port_inner(follower_id, followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unfollow(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.unfollow_friend_client_port_inner(_follower_id, _followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_followers(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: Option<i32>,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowListResponse> {
        self.list_followers_friend_client_port_inner(_user_id, _cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_following(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: Option<i32>,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowListResponse> {
        self.list_following_friend_client_port_inner(_user_id, _cursor, _limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_counts(
        &self,
        user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowCountsResponse> {
        self.get_counts_friend_client_port_inner(user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_counts_batch(
        &self,
        user_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        HashMap<String, FollowCountsResponse>,
    > {
        self.get_counts_batch_friend_client_port_inner(user_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn check_following(
        &self,
        _follower_id: String,
        _followee_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowStatusResponse> {
        self.check_following_friend_client_port_inner(_follower_id, _followee_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn accept_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowActionResponse> {
        self.accept_follow_request_friend_client_port_inner(request_id, actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn reject_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowRequestResponse> {
        self.reject_follow_request_friend_client_port_inner(request_id, actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn cancel_follow_request(
        &self,
        request_id: String,
        actor_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowRequestResponse> {
        self.cancel_follow_request_friend_client_port_inner(request_id, actor_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_incoming_follow_requests(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowRequestPageResponse>
    {
        self.list_incoming_follow_requests_friend_client_port_inner(user_id, cursor, limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_outgoing_follow_requests(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<FollowRequestPageResponse>
    {
        self.list_outgoing_follow_requests_friend_client_port_inner(user_id, cursor, limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn block_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.block_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unblock_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.unblock_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mute_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.mute_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn unmute_user(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.unmute_user_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_relationship(
        &self,
        viewer_id: String,
        target_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<RelationshipPolicyResponse>
    {
        self.get_relationship_friend_client_port_inner(viewer_id, target_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_relationships_batch(
        &self,
        viewer_id: String,
        target_ids: Vec<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        HashMap<String, RelationshipPolicyResponse>,
    > {
        self.get_relationships_batch_friend_client_port_inner(viewer_id, target_ids)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_friend_client_port_inner(_operation_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_people_recommendations(
        &self,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
        _request_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::PeopleRecommendationPageResponse,
    > {
        self.get_people_recommendations_friend_client_port_inner(
            _viewer_id,
            _cursor,
            _limit,
            _request_id,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn record_people_recommendation_event(
        &self,
        _viewer_id: String,
        _request_id: String,
        _event: api_gateway::application::dto::friend_dto::PeopleRecommendationEventRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::friend_dto::RecommendationEventResponse,
    > {
        self.record_people_recommendation_event_friend_client_port_inner(
            _viewer_id,
            _request_id,
            _event,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

fn follow_request_response(id: &str, status: &str) -> FollowRequestResponse {
    FollowRequestResponse {
        id: id.into(),
        follower_id: "user-1".into(),
        followee_id: "user-2".into(),
        status: status.into(),
        created_at: "2026-08-03T00:00:00Z".into(),
        decided_at: (status != "PENDING").then(|| "2026-08-03T00:01:00Z".into()),
    }
}

#[derive(Default)]
struct FakePostClient {
    delete_post_calls: Mutex<Vec<DeletePostCall>>,
    ranked_feed_calls: Mutex<Vec<RankedFeedCall>>,
    feed_event_calls: Mutex<Vec<FeedEventCall>>,
    search_calls: Mutex<Vec<SearchPostsCall>>,
    search_next_cursor: Mutex<Option<String>>,
    /// Authoritative post counts keyed by author. An author absent from the map
    /// stands for a lookup that produced no observation, so a test can assert
    /// that a missing count is not reported as zero.
    post_counts: Mutex<HashMap<String, i64>>,
    post_count_calls: Mutex<Vec<Vec<String>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RankedFeedCall {
    user_id: String,
    cursor: Option<String>,
    limit: i32,
    request_id: String,
    session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct FeedEventCall {
    viewer_id: String,
    request_id: String,
    event: FeedRecommendationEventRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeletePostCall {
    user_id: String,
    post_id: String,
    request_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchPostsCall {
    viewer_id: String,
    query: String,
    cursor: Option<String>,
    limit: i32,
}

impl FakePostClient {
    async fn get_author_post_counts_inner(
        &self,
        author_ids: Vec<String>,
    ) -> Result<HashMap<String, i64>> {
        self.post_count_calls
            .lock()
            .unwrap()
            .push(author_ids.clone());
        let counts = self.post_counts.lock().unwrap();
        Ok(author_ids
            .into_iter()
            .filter_map(|author_id| counts.get(&author_id).map(|count| (author_id, *count)))
            .collect())
    }

    async fn get_feed_inner(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> Result<FeedResponse> {
        Ok(FeedResponse {
            posts: Vec::new(),
            next_cursor: None,
        })
    }

    async fn get_ranked_feed_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        session_id: Option<String>,
    ) -> Result<RankedFeedResponse> {
        self.ranked_feed_calls.lock().unwrap().push(RankedFeedCall {
            user_id,
            cursor,
            limit,
            request_id: request_id.clone(),
            session_id,
        });
        Ok(RankedFeedResponse {
            posts: Vec::new(),
            next_cursor: None,
            snapshot_id: Some("snapshot-1".into()),
            rank_version: Some("feed-deterministic-v1".into()),
            request_id: Some(request_id),
            fallback_used: false,
            recommendation_metadata: Vec::new(),
            failed_sources: Vec::new(),
        })
    }

    async fn record_feed_recommendation_event_inner(
        &self,
        viewer_id: String,
        request_id: String,
        event: FeedRecommendationEventRequest,
    ) -> Result<FeedRecommendationEventResponse> {
        self.feed_event_calls.lock().unwrap().push(FeedEventCall {
            viewer_id,
            request_id,
            event,
        });
        Ok(FeedRecommendationEventResponse { accepted: true })
    }

    async fn get_author_posts_inner(
        &self,
        _author_id: String,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> Result<FeedResponse> {
        Ok(FeedResponse {
            posts: Vec::new(),
            next_cursor: None,
        })
    }

    async fn search_posts_inner(
        &self,
        viewer_id: String,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> Result<FeedResponse> {
        self.search_calls.lock().unwrap().push(SearchPostsCall {
            viewer_id,
            query,
            cursor,
            limit,
        });
        Ok(FeedResponse {
            posts: Vec::new(),
            next_cursor: self.search_next_cursor.lock().unwrap().clone(),
        })
    }

    async fn get_trending_inner(
        &self,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> Result<FeedResponse> {
        Ok(FeedResponse {
            posts: Vec::new(),
            next_cursor: None,
        })
    }

    async fn get_post_inner(
        &self,
        _viewer_id: String,
        _post_id: String,
    ) -> Result<PostDetailResponse> {
        Ok(PostDetailResponse {
            post: post_response("post-1"),
            comments: Vec::new(),
        })
    }

    async fn create_post_inner(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> Result<PostResponse> {
        Ok(PostResponse {
            author_id: user_id,
            content: request.content,
            visibility: request.visibility,
            ..post_response("post-1")
        })
    }

    async fn delete_post_inner(
        &self,
        user_id: String,
        post_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<DeletePostResponse> {
        self.delete_post_calls.lock().unwrap().push(DeletePostCall {
            user_id,
            post_id: post_id.clone(),
            request_id,
            ip_address,
            user_agent,
        });
        Ok(DeletePostResponse {
            post_id,
            deleted: true,
        })
    }

    async fn create_comment_inner(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> Result<CommentResponse> {
        Ok(CommentResponse {
            id: "comment-1".to_owned(),
            post_id,
            author_id: user_id,
            content: request.content,
            created_at: "now".to_owned(),
        })
    }

    async fn like_post_inner(&self, _user_id: String, post_id: String) -> Result<PostLikeResponse> {
        Ok(PostLikeResponse {
            post_id,
            like_count: 1,
            liked: true,
        })
    }

    async fn unlike_post_inner(
        &self,
        _user_id: String,
        post_id: String,
    ) -> Result<PostLikeResponse> {
        Ok(PostLikeResponse {
            post_id,
            like_count: 0,
            liked: false,
        })
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn notification_posts_visible_inner(
        &self,
        _viewer_id: String,
        _post_ids: Vec<String>,
        _max_duration: std::time::Duration,
    ) -> anyhow::Result<std::collections::HashMap<String, bool>> {
        anyhow::bail!("Notification batch authorization is unavailable")
    }

    async fn get_mutation_receipt_status_inner(
        &self,
        _operation_id: uuid::Uuid,
    ) -> anyhow::Result<Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>>
    {
        anyhow::bail!("Owner mutation status lookup is unavailable")
    }

    async fn apply_content_moderation_inner(
        &self,
        _report_id: String,
        _target_type: String,
        _target_id: String,
        _actor_id: String,
        _action: String,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("Content moderation is not implemented")
    }

    async fn resolve_dead_letter_replay_inner(
        &self,
        _request: api_gateway::application::dto::post_dto::ResolveDeadLetterReplayRequestDto,
    ) -> anyhow::Result<api_gateway::application::dto::post_dto::ResolveDeadLetterReplayResponseDto>
    {
        anyhow::bail!("Audited dead-letter replay is not implemented")
    }
}

#[async_trait]
impl PostClientPort for FakePostClient {
    async fn get_author_post_counts(
        &self,
        author_ids: Vec<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<HashMap<String, i64>> {
        self.get_author_post_counts_inner(author_ids)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_feed(
        &self,
        _user_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_feed_inner(_user_id, _cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_ranked_feed(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
        request_id: String,
        session_id: Option<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<RankedFeedResponse> {
        self.get_ranked_feed_inner(user_id, cursor, limit, request_id, session_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn record_feed_recommendation_event(
        &self,
        viewer_id: String,
        request_id: String,
        event: FeedRecommendationEventRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        FeedRecommendationEventResponse,
    > {
        self.record_feed_recommendation_event_inner(viewer_id, request_id, event)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_author_posts(
        &self,
        _author_id: String,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_author_posts_inner(_author_id, _viewer_id, _cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn search_posts(
        &self,
        viewer_id: String,
        query: String,
        cursor: Option<String>,
        limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.search_posts_inner(viewer_id, query, cursor, limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_trending(
        &self,
        _viewer_id: String,
        _cursor: Option<String>,
        _limit: i32,
    ) -> api_gateway::application::post_client_error::PostClientResult<FeedResponse> {
        self.get_trending_inner(_viewer_id, _cursor, _limit)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_post(
        &self,
        _viewer_id: String,
        _post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostDetailResponse> {
        self.get_post_inner(_viewer_id, _post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_post(
        &self,
        user_id: String,
        request: PostCreateRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostResponse> {
        self.create_post_inner(user_id, request)
            .await
            .map_err(classify_post_client_error)
    }
    async fn delete_post(
        &self,
        user_id: String,
        post_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> api_gateway::application::post_client_error::PostClientResult<DeletePostResponse> {
        self.delete_post_inner(user_id, post_id, request_id, ip_address, user_agent)
            .await
            .map_err(classify_post_client_error)
    }
    async fn create_comment(
        &self,
        user_id: String,
        post_id: String,
        request: CommentCreateRequest,
    ) -> api_gateway::application::post_client_error::PostClientResult<CommentResponse> {
        self.create_comment_inner(user_id, post_id, request)
            .await
            .map_err(classify_post_client_error)
    }
    async fn like_post(
        &self,
        _user_id: String,
        post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.like_post_inner(_user_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn unlike_post(
        &self,
        _user_id: String,
        post_id: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<PostLikeResponse> {
        self.unlike_post_inner(_user_id, post_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn notification_posts_visible(
        &self,
        _viewer_id: String,
        _post_ids: Vec<String>,
        _max_duration: std::time::Duration,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        std::collections::HashMap<String, bool>,
    > {
        self.notification_posts_visible_inner(_viewer_id, _post_ids, _max_duration)
            .await
            .map_err(classify_post_client_error)
    }
    async fn get_mutation_receipt_status(
        &self,
        _operation_id: uuid::Uuid,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        Option<api_gateway::application::dto::post_dto::MutationReceiptStatusDto>,
    > {
        self.get_mutation_receipt_status_inner(_operation_id)
            .await
            .map_err(classify_post_client_error)
    }
    async fn apply_content_moderation(
        &self,
        _report_id: String,
        _target_type: String,
        _target_id: String,
        _actor_id: String,
        _action: String,
    ) -> api_gateway::application::post_client_error::PostClientResult<bool> {
        self.apply_content_moderation_inner(
            _report_id,
            _target_type,
            _target_id,
            _actor_id,
            _action,
        )
        .await
        .map_err(classify_post_client_error)
    }
    async fn resolve_dead_letter_replay(
        &self,
        _request: api_gateway::application::dto::post_dto::ResolveDeadLetterReplayRequestDto,
    ) -> api_gateway::application::post_client_error::PostClientResult<
        api_gateway::application::dto::post_dto::ResolveDeadLetterReplayResponseDto,
    > {
        self.resolve_dead_letter_replay_inner(_request)
            .await
            .map_err(classify_post_client_error)
    }
}

#[derive(Default)]
struct FakeAuthClient {
    oauth_callback_calls: Mutex<Vec<OAuthCallbackCall>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OAuthCallbackCall {
    provider: IdentityProvider,
    code: String,
    state: String,
    ip_address: Option<String>,
    user_agent: Option<String>,
}

impl FakeAuthClient {
    async fn signup_auth_client_port_inner(
        &self,
        _request: EmailSignupRequest,
    ) -> Result<SignupResponse> {
        Ok(SignupResponse {
            user_id: user_uuid(),
            status: "OK".to_owned(),
        })
    }

    async fn verify_auth_client_port_inner(
        &self,
        _request: EmailVerifyRequest,
    ) -> Result<SignupResponse> {
        Ok(SignupResponse {
            user_id: user_uuid(),
            status: "VERIFIED".to_owned(),
        })
    }

    async fn check_email_auth_client_port_inner(
        &self,
        request: EmailCheckRequest,
    ) -> Result<EmailAvailabilityResponse> {
        Ok(EmailAvailabilityResponse {
            email: request.email,
            available: true,
            status: "AVAILABLE".to_owned(),
        })
    }

    async fn request_password_reset_auth_client_port_inner(
        &self,
        _request: PasswordResetRequest,
    ) -> Result<PasswordResetResponse> {
        Ok(PasswordResetResponse {
            status: "SENT".to_owned(),
        })
    }

    async fn confirm_password_reset_auth_client_port_inner(
        &self,
        _request: PasswordResetConfirmRequest,
    ) -> Result<PasswordResetResponse> {
        Ok(PasswordResetResponse {
            status: "RESET".to_owned(),
        })
    }

    async fn login_auth_client_port_inner(
        &self,
        _request: LoginRequest,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> Result<AuthResponse> {
        Ok(auth_response())
    }

    async fn refresh_auth_client_port_inner(&self, _refresh_token: String) -> Result<AuthResponse> {
        Ok(auth_response())
    }

    async fn logout_auth_client_port_inner(&self, _refresh_token: String) -> Result<()> {
        Ok(())
    }

    async fn authorize_auth_client_port_inner(
        &self,
        _provider: IdentityProvider,
    ) -> Result<api_gateway::application::dto::auth_dto::OAuthAuthorizationDto> {
        Ok(
            api_gateway::application::dto::auth_dto::OAuthAuthorizationDto {
                authorization_uri: "https://auth.example/oauth"
                    .parse::<http::Uri>()?
                    .to_string(),
            },
        )
    }

    async fn oauth_callback_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
        code: String,
        state: String,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthResponse> {
        self.oauth_callback_calls
            .lock()
            .unwrap()
            .push(OAuthCallbackCall {
                provider,
                code,
                state,
                ip_address,
                user_agent,
            });
        Ok(auth_response())
    }

    async fn link_provider_auth_client_port_inner(
        &self,
        _provider: IdentityProvider,
        _request: OAuthLinkRequest,
        _user_id: String,
    ) -> Result<LinkedProvidersResponse> {
        Ok(LinkedProvidersResponse {
            user_id: user_uuid(),
            providers: vec![IdentityProvider::Google],
        })
    }

    // Explicit recording behavior formerly inherited from the port.
    async fn delete_account_auth_client_port_inner(
        &self,
        _user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> anyhow::Result<api_gateway::application::dto::auth_dto::DeleteAccountResponse> {
        anyhow::bail!("Account deletion is not implemented")
    }

    async fn get_user_deletion_auth_client_port_inner(
        &self,
        _user_id: String,
    ) -> anyhow::Result<Option<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse>>
    {
        anyhow::bail!("Account deletion status is not implemented")
    }

    async fn get_user_deletion_by_saga_id_auth_client_port_inner(
        &self,
        _saga_id: String,
    ) -> anyhow::Result<Option<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse>>
    {
        anyhow::bail!("Account deletion saga lookup is not implemented")
    }

    async fn list_dead_lettered_user_deletions_auth_client_port_inner(
        &self,
        _limit: i32,
        _after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        _after_saga_id: Option<uuid::Uuid>,
    ) -> anyhow::Result<api_gateway::application::dto::auth_dto::DeadLetteredUserDeletionListResponse>
    {
        anyhow::bail!("Dead-lettered account deletion listing is not implemented")
    }

    async fn retry_user_deletion_auth_client_port_inner(
        &self,
        _request: api_gateway::application::dto::auth_dto::RetryUserDeletionRequestDto,
    ) -> anyhow::Result<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse> {
        anyhow::bail!("Account deletion recovery is not implemented")
    }

    async fn session_state_auth_client_port_inner(
        &self,
        _request: &api_gateway::application::dto::issuer_dto::SessionAuthorityRequestDto,
    ) -> anyhow::Result<api_gateway::application::dto::issuer_dto::SessionStateDto> {
        anyhow::bail!("Session checks are not implemented")
    }
}

#[async_trait]
impl AuthClientPort for FakeAuthClient {
    async fn signup(
        &self,
        _request: EmailSignupRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<SignupResponse> {
        self.signup_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn verify(
        &self,
        _request: EmailVerifyRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<SignupResponse> {
        self.verify_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn check_email(
        &self,
        request: EmailCheckRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<EmailAvailabilityResponse>
    {
        self.check_email_auth_client_port_inner(request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn request_password_reset(
        &self,
        _request: PasswordResetRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<PasswordResetResponse> {
        self.request_password_reset_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn confirm_password_reset(
        &self,
        _request: PasswordResetConfirmRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<PasswordResetResponse> {
        self.confirm_password_reset_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn login(
        &self,
        _request: LoginRequest,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.login_auth_client_port_inner(_request, _ip_address, _user_agent)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn refresh(
        &self,
        _refresh_token: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.refresh_auth_client_port_inner(_refresh_token)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn logout(
        &self,
        _refresh_token: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<()> {
        self.logout_auth_client_port_inner(_refresh_token)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn authorize(
        &self,
        _provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::OAuthAuthorizationDto,
    > {
        self.authorize_auth_client_port_inner(_provider.into())
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn oauth_callback(
        &self,
        provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
        code: String,
        state: String,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.oauth_callback_auth_client_port_inner(
            provider.into(),
            code,
            state,
            ip_address,
            user_agent,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn link_provider(
        &self,
        _provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
        _request: OAuthLinkRequest,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<LinkedProvidersResponse> {
        self.link_provider_auth_client_port_inner(_provider.into(), _request, _user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn delete_account(
        &self,
        _user_id: String,
        _request_id: Option<String>,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::DeleteAccountResponse,
    > {
        self.delete_account_auth_client_port_inner(_user_id, _request_id, _ip_address, _user_agent)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_user_deletion(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse>,
    > {
        self.get_user_deletion_auth_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_user_deletion_by_saga_id(
        &self,
        _saga_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        Option<api_gateway::application::dto::auth_dto::UserDeletionSagaResponse>,
    > {
        self.get_user_deletion_by_saga_id_auth_client_port_inner(_saga_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_dead_lettered_user_deletions(
        &self,
        _limit: i32,
        _after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        _after_saga_id: Option<uuid::Uuid>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::DeadLetteredUserDeletionListResponse,
    > {
        self.list_dead_lettered_user_deletions_auth_client_port_inner(
            _limit,
            _after_updated_at,
            _after_saga_id,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn retry_user_deletion(
        &self,
        _request: api_gateway::application::dto::auth_dto::RetryUserDeletionRequestDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::UserDeletionSagaResponse,
    > {
        self.retry_user_deletion_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn session_state(
        &self,
        _request: &api_gateway::application::dto::issuer_dto::SessionAuthorityRequestDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::issuer_dto::SessionStateDto,
    > {
        self.session_state_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

#[derive(Default)]
struct FakeNotificationClient {
    list_calls: Mutex<Vec<(String, Option<String>, i32)>>,
}

impl FakeNotificationClient {
    async fn list_notifications_notification_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> Result<NotificationFeedResponse> {
        self.list_calls
            .lock()
            .unwrap()
            .push((user_id, cursor, limit));
        Ok(NotificationFeedResponse {
            items: Vec::new(),
            next_cursor: None,
        })
    }

    async fn mark_read_notification_client_port_inner(
        &self,
        user_id: String,
        notification_id: String,
    ) -> Result<NotificationResponse> {
        Ok(NotificationResponse {
            id: notification_id,
            user_id,
            notification_type: "SYSTEM".to_owned(),
            payload_json: "{}".to_owned(),
            created_at: "now".to_owned(),
            read_at: "now".to_owned(),
        })
    }

    async fn mark_all_read_notification_client_port_inner(
        &self,
        _user_id: String,
    ) -> Result<NotificationMarkAllReadResponse> {
        Ok(NotificationMarkAllReadResponse { updated_count: 1 })
    }
}

#[async_trait]
impl NotificationClientPort for FakeNotificationClient {
    async fn list_notifications(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationFeedResponse> {
        self.list_notifications_notification_client_port_inner(user_id, cursor, limit)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mark_read(
        &self,
        user_id: String,
        notification_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationResponse> {
        self.mark_read_notification_client_port_inner(user_id, notification_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn mark_all_read(
        &self,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<NotificationMarkAllReadResponse>
    {
        self.mark_all_read_notification_client_port_inner(_user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

#[derive(Default)]
struct FakeRouteContractRepository {
    contracts: Mutex<HashMap<String, RouteContract>>,
}

impl FakeRouteContractRepository {
    async fn find_all_infrastructure_inner(&self) -> Result<Vec<RouteContract>> {
        Ok(self.contracts.lock().unwrap().values().cloned().collect())
    }

    async fn find_by_id_infrastructure_inner(
        &self,
        contract_id: &str,
    ) -> Result<Option<RouteContract>> {
        Ok(self.contracts.lock().unwrap().get(contract_id).cloned())
    }

    async fn find_version_infrastructure_inner(
        &self,
        contract_id: &str,
        version: &str,
    ) -> Result<Option<RouteContractVersion>> {
        Ok(self
            .contracts
            .lock()
            .unwrap()
            .get(contract_id)
            .and_then(|contract| {
                contract
                    .versions
                    .iter()
                    .find(|candidate| candidate.version == version)
                    .cloned()
            }))
    }

    async fn save_infrastructure_inner(&self, contract: RouteContract) -> Result<RouteContract> {
        self.contracts
            .lock()
            .unwrap()
            .insert(contract.id.clone(), contract.clone());
        Ok(contract)
    }

    async fn save_version_infrastructure_inner(
        &self,
        contract_id: &str,
        version: RouteContractVersion,
    ) -> Result<RouteContractVersion> {
        if let Some(contract) = self.contracts.lock().unwrap().get_mut(contract_id) {
            contract.versions.push(version.clone());
        }
        Ok(version)
    }
}

#[async_trait]
impl RouteContractRepositoryPort for FakeRouteContractRepository {
    async fn find_all(
        &self,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Vec<api_gateway::application::dto::infrastructure_dto::RouteContractDto>,
    > {
        self.find_all_infrastructure_inner()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn find_by_id(
        &self,
        contract_id: &str,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Option<api_gateway::application::dto::infrastructure_dto::RouteContractDto>,
    > {
        self.find_by_id_infrastructure_inner(contract_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn find_version(
        &self,
        contract_id: &str,
        version: &str,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Option<api_gateway::application::dto::infrastructure_dto::RouteContractVersionDto>,
    > {
        self.find_version_infrastructure_inner(contract_id, version)
            .await
            .map(|value| value.map(Into::into))
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn save(
        &self,
        contract: api_gateway::application::dto::infrastructure_dto::RouteContractDto,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::RouteContractDto,
    > {
        self.save_infrastructure_inner(contract.into())
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn save_version(
        &self,
        contract_id: &str,
        version: api_gateway::application::dto::infrastructure_dto::RouteContractVersionDto,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::RouteContractVersionDto,
    > {
        self.save_version_infrastructure_inner(contract_id, version.into())
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

#[derive(Default)]
struct FakePolicyRepository {
    policies: Mutex<HashMap<String, PolicySet>>,
}

impl FakePolicyRepository {
    async fn find_all_infrastructure_inner(&self) -> Result<Vec<PolicySet>> {
        Ok(self.policies.lock().unwrap().values().cloned().collect())
    }

    async fn find_by_id_infrastructure_inner(&self, policy_id: &str) -> Result<Option<PolicySet>> {
        Ok(self.policies.lock().unwrap().get(policy_id).cloned())
    }

    async fn save_infrastructure_inner(&self, policy: PolicySet) -> Result<PolicySet> {
        self.policies
            .lock()
            .unwrap()
            .insert(policy.id.clone(), policy.clone());
        Ok(policy)
    }
}

#[async_trait]
impl PolicyRepositoryPort for FakePolicyRepository {
    async fn find_all(
        &self,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Vec<api_gateway::application::dto::infrastructure_dto::PolicySetDto>,
    > {
        self.find_all_infrastructure_inner()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn find_by_id(
        &self,
        policy_id: &str,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Option<api_gateway::application::dto::infrastructure_dto::PolicySetDto>,
    > {
        self.find_by_id_infrastructure_inner(policy_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn save(
        &self,
        policy: api_gateway::application::dto::infrastructure_dto::PolicySetDto,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::PolicySetDto,
    > {
        self.save_infrastructure_inner(policy.into())
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

#[derive(Default)]
struct FakeRolloutRepository {
    rollouts: Mutex<HashMap<String, RolloutPolicy>>,
}

impl FakeRolloutRepository {
    async fn find_by_id_infrastructure_inner(
        &self,
        rollout_id: &str,
    ) -> Result<Option<RolloutPolicy>> {
        Ok(self.rollouts.lock().unwrap().get(rollout_id).cloned())
    }

    async fn save_infrastructure_inner(&self, rollout: RolloutPolicy) -> Result<RolloutPolicy> {
        self.rollouts
            .lock()
            .unwrap()
            .insert(rollout.id.clone(), rollout.clone());
        Ok(rollout)
    }
}

#[async_trait]
impl RolloutRepositoryPort for FakeRolloutRepository {
    async fn find_by_id(
        &self,
        rollout_id: &str,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Option<api_gateway::application::dto::infrastructure_dto::RolloutPolicyDto>,
    > {
        self.find_by_id_infrastructure_inner(rollout_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn save(
        &self,
        rollout: api_gateway::application::dto::infrastructure_dto::RolloutPolicyDto,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::RolloutPolicyDto,
    > {
        self.save_infrastructure_inner(rollout.into())
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

#[derive(Default)]
struct FakeClientProfileRepository {
    profiles: Mutex<HashMap<String, ClientProfile>>,
}

impl FakeClientProfileRepository {
    async fn find_all_infrastructure_inner(&self) -> Result<Vec<ClientProfile>> {
        Ok(self.profiles.lock().unwrap().values().cloned().collect())
    }

    async fn find_by_id_infrastructure_inner(
        &self,
        client_id: &str,
    ) -> Result<Option<ClientProfile>> {
        Ok(self.profiles.lock().unwrap().get(client_id).cloned())
    }

    async fn save_infrastructure_inner(&self, profile: ClientProfile) -> Result<ClientProfile> {
        self.profiles
            .lock()
            .unwrap()
            .insert(profile.id.clone(), profile.clone());
        Ok(profile)
    }
}

#[async_trait]
impl ClientProfileRepositoryPort for FakeClientProfileRepository {
    async fn find_all(
        &self,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Vec<api_gateway::application::dto::infrastructure_dto::ClientProfileDto>,
    > {
        self.find_all_infrastructure_inner()
            .await
            .map(|values| values.into_iter().map(Into::into).collect())
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn find_by_id(
        &self,
        client_id: &str,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        Option<api_gateway::application::dto::infrastructure_dto::ClientProfileDto>,
    > {
        self.find_by_id_infrastructure_inner(client_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
    async fn save(
        &self,
        profile: api_gateway::application::dto::infrastructure_dto::ClientProfileDto,
    ) -> api_gateway::application::infrastructure_error::GatewayInfrastructureResult<
        api_gateway::application::dto::infrastructure_dto::ClientProfileDto,
    > {
        self.save_infrastructure_inner(profile.into())
            .await
            .map(Into::into)
            .map_err(
                api_gateway::adapters::out::infrastructure_error::classify_infrastructure_error,
            )
    }
}

fn profile_response(user_id: &str, follower_count: i64, following_count: i64) -> ProfileResponse {
    profile_response_with_post_count(user_id, follower_count, following_count, 0)
}

fn profile_response_with_post_count(
    user_id: &str,
    follower_count: i64,
    following_count: i64,
    post_count: i64,
) -> ProfileResponse {
    ProfileResponse {
        user_id: user_id.to_owned(),
        username: user_id.to_owned(),
        nickname: user_id.to_owned(),
        avatar_url: "/avatar.png".to_owned(),
        bio: "bio".to_owned(),
        follower_count,
        following_count,
        post_count,
        account_visibility: "PUBLIC".into(),
        lifecycle_status: "ACTIVE".into(),
    }
}

fn post_response(id: &str) -> PostResponse {
    PostResponse {
        id: id.to_owned(),
        author_id: "user-1".to_owned(),
        content: "content".to_owned(),
        visibility: "PUBLIC".to_owned(),
        created_at: "now".to_owned(),
        comment_count: 0,
        like_count: 0,
        liked_by_me: false,
    }
}

fn auth_response() -> AuthResponse {
    AuthResponse {
        user_id: user_uuid(),
        access_token: "access".to_owned(),
        refresh_token: "refresh".to_owned(),
        token_type: "Bearer".to_owned(),
        expires_in: 3600,
        linked_providers: vec![IdentityProvider::Google],
    }
}

fn route_contract(id: &str) -> RouteContract {
    RouteContract {
        id: id.to_owned(),
        route_key: "GET /profiles".to_owned(),
        owner: "platform".to_owned(),
        sla_target: Some("p95<200ms".to_owned()),
        status: "ACTIVE".to_owned(),
        description: None,
        versions: vec![RouteContractVersion {
            version: "v1".to_owned(),
            status: "ACTIVE".to_owned(),
            deprecation_date: None,
            response_schema_ref: "schema://profile".to_owned(),
        }],
    }
}

fn policy(id: &str) -> PolicySet {
    PolicySet {
        id: id.to_owned(),
        name: "default".to_owned(),
        rate_limit: Some("100/m".to_owned()),
        quota: None,
        timeout_ms: Some(500),
        retry_policy: None,
        circuit_breaker_policy: None,
        cache_policy: None,
    }
}

fn client_profile(id: &str) -> ClientProfile {
    ClientProfile {
        id: id.to_owned(),
        client_type: "mobile".to_owned(),
        allowed_contracts: vec!["contract-1".to_owned()],
        policy_set_id: Some("policy-1".to_owned()),
    }
}

fn user_uuid() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000701").expect("valid uuid")
}
