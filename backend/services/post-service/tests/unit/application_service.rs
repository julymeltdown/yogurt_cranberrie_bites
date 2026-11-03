use super::support::UnforwardedQueryFixture;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use post_service::application::dto::post_dto::{
    CreatePostDto, DeletePostDto, PurgeExpiredPostsDto, ReplayRequestDto,
};
use post_service::application::ports::{
    EventOutboxPort, ExternalEventPublisherPort, FeedCachePort, FriendClientPort,
    InternalEventDispatcherPort, PostCachePort, PostLikeRepositoryPort, PostRepositoryPort,
    ProfileClientPort,
};
use post_service::application::query_witness::PublishedWitnessStageDto;
use post_service::application::use_cases::PostApplicationService;
use post_service::domain::comment::Comment;
use post_service::domain::feed::{FeedCachePage, FeedPage};
use post_service::domain::post::{
    DeliveryAttempt, EventReplayFilter, OutboxMetricsSnapshot, Post, PostAccessSnapshot,
    PostAuditEvent, PostDetail, RelationshipPolicySnapshot, ReplayTarget, RetentionPurgeResult,
    StoredEvent,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, Semaphore};
use uuid::Uuid;

#[tokio::test]
async fn create_post_fetches_profile_and_saves_post_with_created_event() {
    let fixture = Fixture::new();
    let author_id = uuid("00000000-0000-0000-0000-000000000601");

    let response = fixture
        .service
        .create_post(CreatePostDto {
            author_id: format!(" {author_id} "),
            content: "  hello world  ".to_owned(),
            visibility: "friends".to_owned(),
        })
        .await
        .expect("create post");

    assert_eq!(response.author_id, author_id.to_string());
    assert_eq!(response.content, "hello world");
    assert_eq!(response.visibility, "FRIENDS");
    assert_eq!(
        *fixture.profile.fetch_calls.lock().unwrap(),
        vec![author_id]
    );
    let calls = fixture.posts.save_post_with_event_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].post.author_id, author_id);
    assert_eq!(calls[0].post.content, "hello world");
    assert_eq!(calls[0].event.event_type, "POST_CREATED");
    assert_eq!(calls[0].event.actor_id, Some(author_id));
    assert_eq!(calls[0].event.attributes, vec!["FRIENDS".to_owned()]);
}

#[tokio::test]
async fn create_post_rejects_blank_content_before_profile_lookup() {
    let fixture = Fixture::new();
    let author_id = uuid("00000000-0000-0000-0000-000000000602");

    let error = fixture
        .service
        .create_post(CreatePostDto {
            author_id: author_id.to_string(),
            content: "  ".to_owned(),
            visibility: "PUBLIC".to_owned(),
        })
        .await
        .expect_err("blank content");

    assert_eq!(error.to_string(), "Content is required");
    assert!(fixture.profile.fetch_calls.lock().unwrap().is_empty());
    assert!(fixture
        .posts
        .save_post_with_event_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn create_post_refuses_an_owner_that_confirms_a_different_author() {
    // The author in the command is the one whose profile must be confirmed. An
    // owner that answers with a different user must not be able to turn that
    // into a post, so a create cannot be redirected onto another identity.
    let fixture = Fixture::new();
    let author_id = uuid("00000000-0000-0000-0000-000000000603");
    let answered_with = uuid("00000000-0000-0000-0000-000000000604");
    let profile = Arc::new(ForeignProfileClient {
        fetch_calls: Mutex::new(Vec::new()),
        answers_with: answered_with,
    });
    let service = fixture.application_with_profile_client(profile.clone());

    let error = service
        .create_post(CreatePostDto {
            author_id: author_id.to_string(),
            content: "must not be written".to_owned(),
            visibility: "PUBLIC".to_owned(),
        })
        .await
        .expect_err("a foreign author confirmation is not a create");

    assert_eq!(error.to_string(), "Profile author binding denied");
    // The lookup did happen, and it asked about the command's author.
    assert_eq!(*profile.fetch_calls.lock().unwrap(), vec![author_id]);
    // No post and no event were written for either identity.
    assert!(fixture
        .posts
        .save_post_with_event_calls
        .lock()
        .unwrap()
        .is_empty());
    assert!(fixture.outbox.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn feed_cache_hit_resolves_cached_and_missing_posts_in_order() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000603");
    let author_id = uuid("00000000-0000-0000-0000-000000000604");
    let cached = post(
        uuid("00000000-0000-0000-0000-000000000605"),
        author_id,
        "cached",
    );
    *fixture.friend.following_ids.lock().unwrap() = vec![author_id];
    let missing = post(
        uuid("00000000-0000-0000-0000-000000000606"),
        author_id,
        "missing",
    );
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![cached.id, missing.id],
        next_cursor: Some("next".to_owned()),
    });
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(cached.id, cached.clone());
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(cached.id, cached.clone());
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(missing.id, missing.clone());
    fixture
        .likes
        .like_counts
        .lock()
        .unwrap()
        .insert(missing.id, 42);
    fixture.likes.liked_ids.lock().unwrap().insert(missing.id);

    let response = fixture
        .service
        .get_feed(
            viewer_id,
            Some("2026-01-20T09:10:00+00:00|00000000-0000-0000-0000-000000000601"),
            0,
        )
        .await
        .expect("cached feed");

    assert_eq!(response.posts.len(), 2);
    assert_eq!(response.posts[0].id, cached.id.to_string());
    assert_eq!(response.posts[1].id, missing.id.to_string());
    assert_eq!(response.posts[1].like_count, 42);
    assert!(response.posts[1].liked_by_me);
    assert_eq!(response.next_cursor.as_deref(), Some("next"));
    assert!(fixture
        .friend
        .fetch_following_calls
        .lock()
        .unwrap()
        .is_empty());
    assert!(fixture.posts.fetch_feed_calls.lock().unwrap().is_empty());
    assert_eq!(
        *fixture.posts.fetch_posts_by_ids_calls.lock().unwrap(),
        vec![vec![missing.id]]
    );
    assert_eq!(
        *fixture.post_cache.set_posts_calls.lock().unwrap(),
        vec![vec![missing.id]]
    );
}

#[tokio::test]
async fn blocked_relationship_hides_public_post_detail() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000680");
    let author_id = uuid("00000000-0000-0000-0000-000000000681");
    let post = post(
        uuid("00000000-0000-0000-0000-000000000682"),
        author_id,
        "blocked",
    );
    fixture.posts.insert_detail(PostDetail {
        post: post.clone(),
        comments: Vec::new(),
    });
    fixture.friend.policies.lock().unwrap().insert(
        (viewer_id, author_id),
        RelationshipPolicySnapshot {
            blocked_viewer: true,
            ..RelationshipPolicySnapshot::default()
        },
    );

    let error = fixture
        .service
        .get_post_detail(post.id, viewer_id)
        .await
        .expect_err("blocked viewer must not see the post");

    assert_eq!(error.to_string(), "Post not found");
}

#[tokio::test]
async fn muted_author_is_removed_from_cached_feed() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000683");
    let author_id = uuid("00000000-0000-0000-0000-000000000684");
    let muted = post(
        uuid("00000000-0000-0000-0000-000000000685"),
        author_id,
        "muted",
    );
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![muted.id],
        next_cursor: None,
    });
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(muted.id, muted.clone());
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(muted.id, muted);
    fixture.friend.policies.lock().unwrap().insert(
        (viewer_id, author_id),
        RelationshipPolicySnapshot {
            muted_by_viewer: true,
            ..RelationshipPolicySnapshot::default()
        },
    );

    let response = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("muted feed");

    assert!(response.posts.is_empty());
}

#[tokio::test]
async fn blocked_author_is_removed_from_cached_feed() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000693");
    let author_id = uuid("00000000-0000-0000-0000-000000000694");
    let blocked = post(
        uuid("00000000-0000-0000-0000-000000000695"),
        author_id,
        "blocked cached post",
    );
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![blocked.id],
        next_cursor: None,
    });
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(blocked.id, blocked.clone());
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(blocked.id, blocked);
    fixture.friend.policies.lock().unwrap().insert(
        (viewer_id, author_id),
        RelationshipPolicySnapshot {
            blocked_viewer: true,
            ..RelationshipPolicySnapshot::default()
        },
    );

    let response = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("blocked content should be filtered from the cached page");

    assert!(response.posts.is_empty());
}

#[tokio::test]
async fn moderation_restore_refetches_content_for_a_stale_cached_feed_candidate() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000699");
    let author_id = uuid("00000000-0000-0000-0000-000000000700");
    let post_id = uuid("00000000-0000-0000-0000-000000000701");
    let stale_cached = post(post_id, author_id, "stale cached moderation copy");
    let restored = post(post_id, author_id, "restored authoritative content");
    *fixture.friend.following_ids.lock().unwrap() = vec![author_id];
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![post_id],
        next_cursor: None,
    });
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(post_id, stale_cached.clone());
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(post_id, stale_cached);

    // A committed hide removes the authoritative access snapshot, while the
    // feed's candidate IDs can remain cached until asynchronous invalidation.
    fixture.posts.posts_by_id.lock().unwrap().remove(&post_id);
    let hidden = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("hidden cached content is filtered");
    assert!(hidden.posts.is_empty());

    // RESTORE invalidates the post body but need not rebuild the cached ID page.
    // The reader must therefore fetch the current body before serving it again.
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(post_id, restored.clone());
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .remove(&post_id);
    let visible = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("restored content is refetched from the authoritative repository");

    assert_eq!(visible.posts.len(), 1);
    assert_eq!(visible.posts[0].id, post_id.to_string());
    assert_eq!(visible.posts[0].content, "restored authoritative content");
}

#[tokio::test]
async fn block_and_unblock_during_cached_feed_reads_use_the_current_policy_snapshot() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000696");
    let author_id = uuid("00000000-0000-0000-0000-000000000697");
    let cached = post(
        uuid("00000000-0000-0000-0000-000000000698"),
        author_id,
        "cached across relationship changes",
    );
    *fixture.friend.following_ids.lock().unwrap() = vec![author_id];
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![cached.id],
        next_cursor: None,
    });
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(cached.id, cached.clone());
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(cached.id, cached);

    for (blocked, expected_posts) in [(true, 0), (false, 1)] {
        // Hold the request at the authorization boundary after it has read and
        // resolved the warm cache. The relationship state changes while the
        // request is in flight, before FriendClient reads its policy snapshot.
        let gate = Arc::new(RelationshipPolicyReadGate::new());
        *fixture.friend.next_policy_read_gate.lock().unwrap() = Some(gate.clone());
        let service = fixture.service.clone();
        let read = tokio::spawn(async move { service.get_feed(viewer_id, None, 20).await });

        gate.wait_until_reached().await;
        fixture.friend.policies.lock().unwrap().insert(
            (viewer_id, author_id),
            RelationshipPolicySnapshot {
                blocked_viewer: blocked,
                following: true,
                ..RelationshipPolicySnapshot::default()
            },
        );
        gate.release();

        let response = read.await.expect("feed read task").expect("feed read");
        assert_eq!(response.posts.len(), expected_posts);
    }
}

#[tokio::test]
async fn cached_home_feed_hides_public_posts_after_the_viewer_unfollows_the_author() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-0000000006a1");
    let author_id = uuid("00000000-0000-0000-0000-0000000006a2");
    let cached = post(
        uuid("00000000-0000-0000-0000-0000000006a3"),
        author_id,
        "public post from a formerly followed author",
    );
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![cached.id],
        next_cursor: None,
    });
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(cached.id, cached.clone());
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(cached.id, cached);
    fixture.friend.policies.lock().unwrap().insert(
        (viewer_id, author_id),
        RelationshipPolicySnapshot::default(),
    );

    let response = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("current relationship policy is read before serving the warm page");

    assert!(response.posts.is_empty());
}

#[tokio::test]
async fn cached_home_feed_uses_an_unfollow_committed_during_the_policy_read() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-0000000006a4");
    let author_id = uuid("00000000-0000-0000-0000-0000000006a5");
    let cached = post(
        uuid("00000000-0000-0000-0000-0000000006a6"),
        author_id,
        "cached before the unfollow",
    );
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![cached.id],
        next_cursor: None,
    });
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(cached.id, cached.clone());
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(cached.id, cached);
    fixture.friend.policies.lock().unwrap().insert(
        (viewer_id, author_id),
        RelationshipPolicySnapshot {
            following: true,
            ..RelationshipPolicySnapshot::default()
        },
    );
    let gate = Arc::new(RelationshipPolicyReadGate::new());
    *fixture.friend.next_policy_read_gate.lock().unwrap() = Some(gate.clone());

    let service = fixture.service.clone();
    let read = tokio::spawn(async move { service.get_feed(viewer_id, None, 20).await });
    gate.wait_until_reached().await;
    fixture.friend.policies.lock().unwrap().insert(
        (viewer_id, author_id),
        RelationshipPolicySnapshot::default(),
    );
    gate.release();

    let response = read.await.expect("feed task").expect("feed read");
    assert!(response.posts.is_empty());
}

#[tokio::test]
async fn uncached_home_feed_excludes_an_author_missing_from_current_follow_policy() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-0000000006a7");
    let author_id = uuid("00000000-0000-0000-0000-0000000006a8");
    let candidate = post(
        uuid("00000000-0000-0000-0000-0000000006a9"),
        author_id,
        "public but no longer followed",
    );
    *fixture.friend.following_ids.lock().unwrap() = vec![author_id];
    *fixture.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![candidate],
        next_cursor: None,
    });
    fixture.friend.policies.lock().unwrap().insert(
        (viewer_id, author_id),
        RelationshipPolicySnapshot::default(),
    );

    let response = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("stale following IDs do not authorize a home-feed candidate");

    assert!(response.posts.is_empty());
    let calls = fixture.posts.fetch_feed_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(!calls[0].author_ids.contains(&author_id));
    assert!(calls[0].author_ids.contains(&viewer_id));
}

#[tokio::test]
async fn cached_feed_fails_closed_when_relationship_batch_omits_an_author() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000686");
    let author_id = uuid("00000000-0000-0000-0000-000000000687");
    let cached = post(
        uuid("00000000-0000-0000-0000-000000000688"),
        author_id,
        "policy unavailable",
    );
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![cached.id],
        next_cursor: None,
    });
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(cached.id, cached.clone());
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(cached.id, cached);
    *fixture.friend.omit_batch_policy_for.lock().unwrap() = Some(author_id);

    let error = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect_err("a cached page must not be returned without current relationship policy");

    assert!(error
        .to_string()
        .contains("omitted a requested relationship policy"));
}

#[tokio::test]
async fn uncached_feed_rejects_missing_observed_author_policy_before_cache_fill() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000689");
    let author_id = uuid("00000000-0000-0000-0000-00000000068a");
    let followed_post = post(
        uuid("00000000-0000-0000-0000-00000000068b"),
        author_id,
        "followed post",
    );
    *fixture.friend.following_ids.lock().unwrap() = vec![author_id];
    *fixture.friend.omit_batch_policy_for.lock().unwrap() = Some(author_id);
    *fixture.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![followed_post],
        next_cursor: None,
    });

    let error = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect_err("a missing relationship policy must stop feed selection");

    assert!(error
        .to_string()
        .contains("omitted a requested relationship policy"));
    assert_eq!(fixture.posts.fetch_feed_calls.lock().unwrap().len(), 1);
    assert!(fixture.feed_cache.set_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cached_feed_uses_authoritative_author_and_visibility_state() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000689");
    let cached_author_id = uuid("00000000-0000-0000-0000-000000000690");
    let current_author_id = uuid("00000000-0000-0000-0000-000000000691");
    let cached = post(
        uuid("00000000-0000-0000-0000-000000000692"),
        cached_author_id,
        "stale public cache entry",
    );
    let mut current = cached.clone();
    current.author_id = current_author_id;
    current.visibility = "PRIVATE".to_owned();

    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![cached.id],
        next_cursor: None,
    });
    fixture
        .post_cache
        .cached_posts
        .lock()
        .unwrap()
        .insert(cached.id, cached);
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(current.id, current);

    let response = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("current private content should be filtered");

    assert!(response.posts.is_empty());
}

#[tokio::test]
async fn feed_cache_miss_fetches_following_plus_self_and_stores_cache_entries() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000607");
    let followed_id = uuid("00000000-0000-0000-0000-000000000608");
    let post = post(
        uuid("00000000-0000-0000-0000-000000000609"),
        followed_id,
        "feed",
    );
    *fixture.friend.following_ids.lock().unwrap() = vec![followed_id];
    *fixture.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![post.clone()],
        next_cursor: Some("next-feed".to_owned()),
    });

    let response = fixture
        .service
        .get_feed(viewer_id, None, 10_000)
        .await
        .expect("feed miss");

    assert_eq!(response.posts.len(), 1);
    assert_eq!(response.posts[0].id, post.id.to_string());
    assert_eq!(
        *fixture.friend.fetch_following_calls.lock().unwrap(),
        vec![viewer_id]
    );
    assert_eq!(
        *fixture.posts.fetch_feed_calls.lock().unwrap(),
        vec![FetchFeedCall {
            viewer_id,
            author_ids: vec![followed_id, viewer_id],
            cursor: None,
            limit: 100,
        }]
    );
    assert_eq!(
        *fixture.feed_cache.set_calls.lock().unwrap(),
        vec![SetFeedCall {
            user_id: viewer_id,
            cursor: None,
            limit: 100,
            post_ids: vec![post.id],
            next_cursor: Some("next-feed".to_owned()),
            generation: 0,
        }]
    );
}

#[tokio::test]
async fn author_post_counts_preserve_request_order_and_collapse_duplicates() {
    // The caller lines the response up with its request, so order has to be
    // preserved and a repeated id must not produce a repeated row.
    let fixture = Fixture::new();
    let first = uuid("00000000-0000-0000-0000-000000000700");
    let second = uuid("00000000-0000-0000-0000-000000000701");
    {
        let mut counts = fixture.posts.post_counts.lock().unwrap();
        counts.insert(first, 4);
        counts.insert(second, 11);
    }

    let counts = fixture
        .service
        .author_post_counts(vec![second, first, second])
        .await
        .expect("counts");

    assert_eq!(counts.len(), 2, "a duplicate id must collapse to one row");
    assert_eq!(counts[0].author_id, second.to_string());
    assert_eq!(counts[0].post_count, 11);
    assert_eq!(counts[1].author_id, first.to_string());
    assert_eq!(counts[1].post_count, 4);
    // One batched query, not one per author.
    assert_eq!(fixture.posts.count_posts_calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn author_post_counts_report_a_real_zero_for_an_author_without_posts() {
    // The caller only asks about authors it already knows exist, so no rows
    // means the author has not posted. Reporting zero states that, which is a
    // different claim from a lookup that failed.
    let fixture = Fixture::new();
    let author = uuid("00000000-0000-0000-0000-000000000710");

    let counts = fixture
        .service
        .author_post_counts(vec![author])
        .await
        .expect("counts");

    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].post_count, 0);
}

#[tokio::test]
async fn author_post_counts_do_not_query_when_nothing_was_requested() {
    let fixture = Fixture::new();

    let counts = fixture
        .service
        .author_post_counts(Vec::new())
        .await
        .expect("counts");

    assert!(counts.is_empty());
}

#[tokio::test]
async fn feed_cache_fill_is_refused_when_a_mutation_lands_mid_fetch() {
    // A delete that commits while the page is being computed bumps the
    // generation. If the fill were stored anyway it would land under the new
    // generation and serve the deleted post from cache, so the delete would
    // appear to resurrect itself. This models that interleaving precisely: the
    // bump happens after the generation is read and before the store.
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000660");
    let followed_id = uuid("00000000-0000-0000-0000-000000000661");
    let doomed = post(
        uuid("00000000-0000-0000-0000-000000000662"),
        followed_id,
        "deleted",
    );
    *fixture.friend.following_ids.lock().unwrap() = vec![followed_id];
    *fixture.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![doomed.clone()],
        next_cursor: None,
    });
    *fixture
        .feed_cache
        .bump_after_generation_read
        .lock()
        .unwrap() = true;

    // When: the feed is served and the concurrent mutation bumps the generation.
    let response = fixture
        .service
        .get_feed(viewer_id, None, 10)
        .await
        .expect("feed with a racing mutation");

    // Then: the response is still correct for this request...
    assert_eq!(response.posts.len(), 1);
    // ...but nothing was cached, so the stale page cannot be served later.
    assert!(
        fixture.feed_cache.set_calls.lock().unwrap().is_empty(),
        "a fill that raced a mutation must not be stored"
    );
    assert_eq!(
        *fixture.feed_cache.generation.lock().unwrap(),
        1,
        "the mutation must have bumped the generation"
    );
}

#[tokio::test]
async fn feed_cache_fill_is_refused_when_mutation_occurs_during_repository_read() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000663");
    let followed_id = uuid("00000000-0000-0000-0000-000000000664");
    let candidate = post(
        uuid("00000000-0000-0000-0000-000000000665"),
        followed_id,
        "candidate from the earlier database snapshot",
    );
    let candidate_id = candidate.id;
    *fixture.friend.following_ids.lock().unwrap() = vec![followed_id];
    *fixture.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![candidate],
        next_cursor: None,
    });
    let gate = Arc::new(FeedFetchGate::new());
    *fixture.posts.next_feed_read_gate.lock().unwrap() = Some(gate.clone());

    let service = fixture.service.clone();
    let request = tokio::spawn(async move { service.get_feed(viewer_id, None, 10).await });
    gate.wait_until_entered().await;

    // The repository already captured its earlier page. Commit the mutation
    // and advance the generation while that read is paused, then release it.
    *fixture.posts.feed_page.lock().unwrap() = Some(empty_feed_page());
    *fixture.feed_cache.generation.lock().unwrap() = 1;
    gate.release();
    let response = request
        .await
        .expect("feed request task")
        .expect("in-flight request may finish from its earlier snapshot");

    assert_eq!(response.posts.len(), 1);
    assert_eq!(response.posts[0].id, candidate_id.to_string());
    assert!(fixture.feed_cache.set_calls.lock().unwrap().is_empty());
    assert_eq!(*fixture.feed_cache.generation.lock().unwrap(), 1);
}

#[tokio::test]
async fn block_or_mute_during_uncached_feed_repository_read_uses_current_policy() {
    for (blocked_viewer, muted_by_viewer) in [(true, false), (false, true)] {
        let fixture = Fixture::new();
        let viewer_id = uuid("00000000-0000-0000-0000-000000000666");
        let author_id = uuid("00000000-0000-0000-0000-000000000667");
        let candidate = post(
            uuid("00000000-0000-0000-0000-000000000668"),
            author_id,
            "candidate captured before a relationship change",
        );
        *fixture.friend.following_ids.lock().unwrap() = vec![author_id];
        *fixture.posts.feed_page.lock().unwrap() = Some(FeedPage {
            posts: vec![candidate],
            next_cursor: None,
        });
        let gate = Arc::new(FeedFetchGate::new());
        *fixture.posts.next_feed_read_gate.lock().unwrap() = Some(gate.clone());

        let service = fixture.service.clone();
        let request = tokio::spawn(async move { service.get_feed(viewer_id, None, 10).await });
        gate.wait_until_entered().await;

        // The repository captured an allowed candidate. A block or mute
        // commits while the read is paused; current policy filters it on return.
        fixture.friend.policies.lock().unwrap().insert(
            (viewer_id, author_id),
            RelationshipPolicySnapshot {
                blocked_viewer,
                muted_by_viewer,
                following: true,
                ..RelationshipPolicySnapshot::default()
            },
        );
        *fixture.posts.feed_page.lock().unwrap() = Some(empty_feed_page());
        *fixture.feed_cache.generation.lock().unwrap() = 1;
        gate.release();

        let response = request
            .await
            .expect("feed request task")
            .expect("current relationship policy filters an earlier repository snapshot");

        assert!(response.posts.is_empty());
        assert!(fixture
            .posts
            .fetch_feed_calls
            .lock()
            .unwrap()
            .first()
            .expect("repository read captured a candidate")
            .author_ids
            .contains(&author_id));
        assert!(fixture.feed_cache.set_calls.lock().unwrap().is_empty());
        assert_eq!(*fixture.feed_cache.generation.lock().unwrap(), 1);
    }
}

#[tokio::test]
async fn feed_cache_failures_fall_back_to_postgres_and_do_not_fail_the_response() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000650");
    let post = post(
        uuid("00000000-0000-0000-0000-000000000651"),
        viewer_id,
        "database fallback",
    );
    *fixture.feed_cache.fail_get.lock().unwrap() = true;
    *fixture.feed_cache.fail_set.lock().unwrap() = true;
    *fixture.post_cache.fail_set.lock().unwrap() = true;
    *fixture.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![post.clone()],
        next_cursor: None,
    });

    let response = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("cache outage must not fail the feed");

    assert_eq!(response.posts.len(), 1);
    assert_eq!(response.posts[0].id, post.id.to_string());
    assert_eq!(fixture.posts.fetch_feed_calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn feed_cache_generation_failure_falls_back_without_storing_a_page() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000654");
    let post = post(
        uuid("00000000-0000-0000-0000-000000000655"),
        viewer_id,
        "safe database fallback",
    );
    *fixture.feed_cache.fail_get.lock().unwrap() = true;
    *fixture.feed_cache.fail_generation.lock().unwrap() = true;
    *fixture.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![post.clone()],
        next_cursor: None,
    });

    let response = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("generation outage must not fail the PostgreSQL fallback");

    assert_eq!(response.posts.len(), 1);
    assert_eq!(response.posts[0].id, post.id.to_string());
    assert!(
        fixture.feed_cache.set_calls.lock().unwrap().is_empty(),
        "a generation-less fallback result must not be cached"
    );
}

#[tokio::test]
async fn post_cache_read_failure_uses_batch_postgres_fallback() {
    let fixture = Fixture::new();
    let viewer_id = uuid("00000000-0000-0000-0000-000000000652");
    let post = post(
        uuid("00000000-0000-0000-0000-000000000653"),
        viewer_id,
        "cache read fallback",
    );
    *fixture.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: vec![post.id],
        next_cursor: None,
    });
    *fixture.post_cache.fail_get.lock().unwrap() = true;
    fixture
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(post.id, post.clone());

    let response = fixture
        .service
        .get_feed(viewer_id, None, 20)
        .await
        .expect("post cache outage must use postgres");

    assert_eq!(response.posts[0].id, post.id.to_string());
    assert_eq!(
        *fixture.posts.fetch_posts_by_ids_calls.lock().unwrap(),
        vec![vec![post.id]]
    );
}

#[tokio::test]
async fn forbidden_delete_post_records_audit_without_soft_delete() {
    let fixture = Fixture::new();
    let post_id = uuid("00000000-0000-0000-0000-000000000610");
    let author_id = uuid("00000000-0000-0000-0000-000000000611");
    let actor_id = uuid("00000000-0000-0000-0000-000000000612");
    fixture.posts.insert_detail(PostDetail {
        post: post(post_id, author_id, "owned"),
        comments: Vec::new(),
    });

    let error = fixture
        .service
        .delete_post(DeletePostDto {
            post_id: post_id.to_string(),
            actor_id: actor_id.to_string(),
            request_id: Some(" req-1 ".to_owned()),
            ip_address: Some(" 127.0.0.1 ".to_owned()),
            user_agent: Some(" unit-test ".to_owned()),
        })
        .await
        .expect_err("forbidden delete");

    assert_eq!(error.to_string(), "Forbidden");
    assert!(fixture
        .posts
        .soft_delete_post_event_calls
        .lock()
        .unwrap()
        .is_empty());
    let audits = fixture.posts.audit_events.lock().unwrap();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].actor_id, Some(actor_id));
    assert_eq!(audits[0].post_id, Some(post_id));
    assert_eq!(audits[0].action, "POST_DELETED");
    assert_eq!(audits[0].outcome, "FORBIDDEN");
}

#[tokio::test]
async fn owner_delete_post_soft_deletes_with_event_and_audit() {
    let fixture = Fixture::new();
    let post_id = uuid("00000000-0000-0000-0000-000000000613");
    let actor_id = uuid("00000000-0000-0000-0000-000000000614");
    fixture.posts.insert_detail(PostDetail {
        post: post(post_id, actor_id, "owned"),
        comments: Vec::new(),
    });

    let response = fixture
        .service
        .delete_post(DeletePostDto {
            post_id: post_id.to_string(),
            actor_id: actor_id.to_string(),
            request_id: Some(" req-2 ".to_owned()),
            ip_address: None,
            user_agent: Some("  ".to_owned()),
        })
        .await
        .expect("delete post");

    assert!(response.deleted);
    let calls = fixture.posts.soft_delete_post_event_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].post_id, post_id);
    assert_eq!(calls[0].actor_id, actor_id);
    assert_eq!(calls[0].reason, "user_request");
    assert_eq!(calls[0].event.event_type, "POST_DELETED");
    assert_eq!(calls[0].audit_event.outcome, "SUCCESS");
    assert_eq!(calls[0].audit_event.request_id.as_deref(), Some("req-2"));
    assert_eq!(calls[0].audit_event.user_agent, None);
}

#[tokio::test]
async fn like_post_checks_visibility_and_returns_repository_like_count() {
    let fixture = Fixture::new();
    let post_id = uuid("00000000-0000-0000-0000-000000000615");
    let author_id = uuid("00000000-0000-0000-0000-000000000616");
    let user_id = uuid("00000000-0000-0000-0000-000000000617");
    fixture.posts.insert_detail(PostDetail {
        post: Post {
            visibility: "FRIENDS".to_owned(),
            like_count: 3,
            ..post(post_id, author_id, "likeable")
        },
        comments: Vec::new(),
    });
    *fixture.friend.is_following_result.lock().unwrap() = true;
    *fixture.likes.add_like_count.lock().unwrap() = 4;

    let response = fixture
        .service
        .like_post(post_id, user_id)
        .await
        .expect("like post");

    assert!(response.liked);
    assert_eq!(response.like_count, 4);
    assert_eq!(
        *fixture.posts.fetch_post_header_calls.lock().unwrap(),
        vec![post_id]
    );
    assert!(fixture
        .posts
        .fetch_post_detail_calls
        .lock()
        .unwrap()
        .is_empty());
    assert_eq!(
        *fixture.friend.is_following_calls.lock().unwrap(),
        vec![IsFollowingCall {
            follower_id: user_id,
            followee_id: author_id,
        }]
    );
    let calls = fixture.likes.add_like_event_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].post_id, post_id);
    assert_eq!(calls[0].user_id, user_id);
    assert_eq!(calls[0].event.event_type, "POST_LIKED");
}

#[tokio::test]
async fn replay_events_dispatches_to_both_targets_with_filter_and_limit() {
    let fixture = Fixture::new();
    let actor_id = uuid("00000000-0000-0000-0000-000000000618");
    let event = stored_event(
        uuid("00000000-0000-0000-0000-000000000619"),
        "POST_CREATED",
        Some(actor_id),
    );
    *fixture.outbox.events.lock().unwrap() = vec![event.clone()];

    let response = fixture
        .service
        .replay_events(ReplayRequestDto {
            actor_id: Some(actor_id.to_string()),
            event_type: Some("POST_CREATED".to_owned()),
            attribute: Some("public".to_owned()),
            from: Some(instant(1_000).to_rfc3339()),
            to: Some(instant(2_000).to_rfc3339()),
            published: Some(false),
            target: Some(ReplayTarget::Both),
            limit: Some(7),
        })
        .await
        .expect("replay events");

    assert_eq!(response.replayed_count, 1);
    let calls = fixture.outbox.find_events_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].filter.actor_id, Some(actor_id));
    assert_eq!(calls[0].filter.event_type.as_deref(), Some("POST_CREATED"));
    assert_eq!(calls[0].filter.attribute.as_deref(), Some("public"));
    assert_eq!(calls[0].filter.published, Some(false));
    assert_eq!(calls[0].limit, 7);
    assert_eq!(
        *fixture.internal.dispatched.lock().unwrap(),
        vec![event.event_id]
    );
    assert_eq!(
        *fixture.publisher.published.lock().unwrap(),
        vec![event.event_id]
    );
    assert!(fixture
        .outbox
        .mark_published_calls
        .lock()
        .unwrap()
        .is_empty());
    // Delivery/lease/history behavior is verified against PostgreSQL. This
    // recording boundary verifies orchestration only, never persistence.
    assert!(fixture.outbox.delivery_attempts.lock().unwrap().is_empty());
}

#[tokio::test]
async fn replay_events_rejects_an_oversized_limit_before_loading_or_dispatching_events() {
    let fixture = Fixture::new();

    let error = fixture
        .service
        .replay_events(ReplayRequestDto {
            actor_id: None,
            event_type: None,
            attribute: None,
            from: None,
            to: None,
            published: None,
            target: Some(ReplayTarget::Both),
            limit: Some(501),
        })
        .await
        .expect_err("oversized replay limit");

    assert_eq!(error.to_string(), "Replay limit must be between 1 and 500");
    assert!(fixture.outbox.find_events_calls.lock().unwrap().is_empty());
    assert!(fixture.internal.dispatched.lock().unwrap().is_empty());
    assert!(fixture.publisher.published.lock().unwrap().is_empty());
}

#[tokio::test]
async fn purge_expired_posts_trims_optional_metadata_and_returns_split_counts() {
    let fixture = Fixture::new();
    let actor_id = uuid("00000000-0000-0000-0000-000000000620");
    *fixture.posts.purge_result.lock().unwrap() = RetentionPurgeResult {
        post_ids: vec![uuid("00000000-0000-0000-0000-000000000621")],
        comment_ids: vec![
            uuid("00000000-0000-0000-0000-000000000622"),
            uuid("00000000-0000-0000-0000-000000000623"),
        ],
    };

    let response = fixture
        .service
        .purge_expired_posts(PurgeExpiredPostsDto {
            actor_id: format!(" {actor_id} "),
            request_id: Some(" req-3 ".to_owned()),
            ip_address: Some(" 10.0.0.3 ".to_owned()),
            user_agent: Some("  ".to_owned()),
            dry_run: true,
        })
        .await
        .expect("purge posts");

    assert_eq!(response.purged_count, 3);
    assert_eq!(response.purged_post_count, 1);
    assert_eq!(response.purged_comment_count, 2);
    let calls = fixture.posts.purge_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].actor_id, actor_id);
    assert_eq!(calls[0].request_id.as_deref(), Some("req-3"));
    assert_eq!(calls[0].ip_address.as_deref(), Some("10.0.0.3"));
    assert_eq!(calls[0].user_agent, None);
    assert!(calls[0].dry_run);
}

struct Fixture {
    service: PostApplicationService,
    posts: Arc<FakePostRepository>,
    likes: Arc<FakeLikeRepository>,
    feed_cache: Arc<FakeFeedCache>,
    post_cache: Arc<FakePostCache>,
    friend: Arc<FakeFriendClient>,
    profile: Arc<FakeProfileClient>,
    outbox: Arc<FakeOutbox>,
    publisher: Arc<SpyPublisher>,
    internal: Arc<SpyInternalDispatcher>,
}

impl Fixture {
    fn new() -> Self {
        let posts = Arc::new(FakePostRepository::default());
        let likes = Arc::new(FakeLikeRepository::default());
        let feed_cache = Arc::new(FakeFeedCache::default());
        let post_cache = Arc::new(FakePostCache::default());
        let friend = Arc::new(FakeFriendClient::default());
        let profile = Arc::new(FakeProfileClient::default());
        let outbox = Arc::new(FakeOutbox::default());
        let publisher = Arc::new(SpyPublisher::default());
        let internal = Arc::new(SpyInternalDispatcher::default());
        let service = PostApplicationService {
            posts: posts.clone(),
            query_witnesses: Arc::new(UnforwardedQueryFixture),
            likes: likes.clone(),
            feed_cache: feed_cache.clone(),
            post_cache: post_cache.clone(),
            friend_client: friend.clone(),
            profile_client: profile.clone(),
            outbox: outbox.clone(),
            replay_delivery: Arc::new(RecordingReplayDelivery {
                publisher: publisher.clone(),
                internal: internal.clone(),
            }),
            publisher: publisher.clone(),
            internal_dispatcher: internal.clone(),
        };
        Self {
            service,
            posts,
            likes,
            feed_cache,
            post_cache,
            friend,
            profile,
            outbox,
            publisher,
            internal,
        }
    }

    /// Replace only the Profile existence dependency, keeping every other
    /// recorded double so the create path is otherwise unchanged.
    fn application_with_profile_client(
        &self,
        profile: Arc<dyn ProfileClientPort>,
    ) -> PostApplicationService {
        let mut service = self.service.clone();
        service.profile_client = profile;
        service
    }
}

#[derive(Default)]
struct FakePostRepository {
    details: Mutex<HashMap<Uuid, PostDetail>>,
    posts_by_id: Mutex<HashMap<Uuid, Post>>,
    feed_page: Mutex<Option<FeedPage>>,
    next_feed_read_gate: Mutex<Option<Arc<FeedFetchGate>>>,
    purge_result: Mutex<RetentionPurgeResult>,
    save_post_with_event_calls: Mutex<Vec<SavePostWithEventCall>>,
    fetch_feed_calls: Mutex<Vec<FetchFeedCall>>,
    fetch_posts_by_ids_calls: Mutex<Vec<Vec<Uuid>>>,
    fetch_post_header_calls: Mutex<Vec<Uuid>>,
    fetch_post_detail_calls: Mutex<Vec<Uuid>>,
    soft_delete_post_event_calls: Mutex<Vec<SoftDeletePostEventCall>>,
    audit_events: Mutex<Vec<PostAuditEvent>>,
    purge_calls: Mutex<Vec<PurgeCall>>,
    /// Authoritative post counts keyed by author, and the requests that asked
    /// for them. An author absent from the map has no rows in the posts table.
    post_counts: Mutex<HashMap<Uuid, i64>>,
    count_posts_calls: Mutex<Vec<Vec<Uuid>>>,
}

struct FeedFetchGate {
    reached: Notify,
    release_permit: Arc<Semaphore>,
}

impl FeedFetchGate {
    fn new() -> Self {
        Self {
            reached: Notify::new(),
            release_permit: Arc::new(Semaphore::new(0)),
        }
    }

    async fn wait_until_entered(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(10), self.reached.notified())
            .await
            .expect("feed repository read should reach its test barrier");
    }

    async fn pause(&self) {
        self.reached.notify_one();
        self.release_permit
            .clone()
            .acquire_owned()
            .await
            .expect("feed read gate remains open")
            .forget();
    }

    fn release(&self) {
        self.release_permit.add_permits(1);
    }
}

impl FakePostRepository {
    fn insert_detail(&self, detail: PostDetail) {
        self.posts_by_id
            .lock()
            .unwrap()
            .insert(detail.post.id, detail.post.clone());
        self.details.lock().unwrap().insert(detail.post.id, detail);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SavePostWithEventCall {
    post: Post,
    event: StoredEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FetchFeedCall {
    viewer_id: Uuid,
    author_ids: Vec<Uuid>,
    cursor: Option<String>,
    limit: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SoftDeletePostEventCall {
    post_id: Uuid,
    actor_id: Uuid,
    reason: String,
    retention_until: DateTime<Utc>,
    event: StoredEvent,
    audit_event: PostAuditEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PurgeCall {
    actor_id: Uuid,
    request_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    dry_run: bool,
}

#[async_trait]
impl PostRepositoryPort for FakePostRepository {
    async fn count_posts_by_author(&self, author_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>> {
        self.count_posts_calls
            .lock()
            .unwrap()
            .push(author_ids.to_vec());
        let counts = self.post_counts.lock().unwrap();
        Ok(author_ids
            .iter()
            .filter_map(|author_id| counts.get(author_id).map(|count| (*author_id, *count)))
            .collect())
    }

    async fn save_post(&self, post: &Post) -> Result<Post> {
        Ok(post.clone())
    }

    async fn save_post_with_event(
        &self,
        post: &Post,
        event: &StoredEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<(Post, StoredEvent)> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        self.save_post_with_event_calls
            .lock()
            .unwrap()
            .push(SavePostWithEventCall {
                post: post.clone(),
                event: event.clone(),
            });
        Ok((post.clone(), event.clone()))
    }

    async fn save_comment(&self, comment: &Comment) -> Result<Comment> {
        Ok(comment.clone())
    }

    async fn save_comment_with_counter_event(
        &self,
        comment: &Comment,
        event: &StoredEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<(Comment, StoredEvent)> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        Ok((comment.clone(), event.clone()))
    }

    async fn fetch_comment(&self, _comment_id: Uuid) -> Result<Option<Comment>> {
        Ok(None)
    }

    async fn fetch_feed(
        &self,
        viewer_id: Uuid,
        author_ids: &[Uuid],
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FeedPage> {
        self.fetch_feed_calls.lock().unwrap().push(FetchFeedCall {
            viewer_id,
            author_ids: author_ids.to_vec(),
            cursor: cursor.map(str::to_owned),
            limit,
        });
        let page = self
            .feed_page
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(empty_feed_page);
        let gate = self.next_feed_read_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.pause().await;
        }
        Ok(page)
    }

    async fn fetch_by_author(
        &self,
        _author_id: Uuid,
        _cursor: Option<&str>,
        _limit: i32,
    ) -> Result<FeedPage> {
        Ok(empty_feed_page())
    }

    async fn search_by_content(
        &self,
        _query: &str,
        _cursor: Option<&str>,
        _limit: i32,
    ) -> Result<FeedPage> {
        Ok(empty_feed_page())
    }

    async fn fetch_trending(&self, _cursor: Option<&str>, _limit: i32) -> Result<FeedPage> {
        Ok(empty_feed_page())
    }

    async fn fetch_posts_by_ids(&self, post_ids: &[Uuid]) -> Result<Vec<Post>> {
        self.fetch_posts_by_ids_calls
            .lock()
            .unwrap()
            .push(post_ids.to_vec());
        let posts = self.posts_by_id.lock().unwrap();
        Ok(post_ids
            .iter()
            .filter_map(|post_id| posts.get(post_id).cloned())
            .collect())
    }

    async fn fetch_post_access_snapshots(
        &self,
        post_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, PostAccessSnapshot>> {
        let posts = self.posts_by_id.lock().unwrap();
        Ok(post_ids
            .iter()
            .filter_map(|post_id| {
                posts.get(post_id).map(|post| {
                    (
                        *post_id,
                        PostAccessSnapshot {
                            author_id: post.author_id,
                            visibility: post.visibility.clone(),
                        },
                    )
                })
            })
            .collect())
    }

    async fn fetch_post_header(&self, post_id: Uuid) -> Result<Option<Post>> {
        self.fetch_post_header_calls.lock().unwrap().push(post_id);
        Ok(self.posts_by_id.lock().unwrap().get(&post_id).cloned())
    }

    async fn fetch_post(&self, post_id: Uuid) -> Result<Option<PostDetail>> {
        self.fetch_post_detail_calls.lock().unwrap().push(post_id);
        Ok(self.details.lock().unwrap().get(&post_id).cloned())
    }

    async fn soft_delete_post(
        &self,
        _post_id: Uuid,
        _actor_id: Uuid,
        _reason: &str,
        _retention_until: DateTime<Utc>,
    ) -> Result<bool> {
        Ok(true)
    }

    async fn soft_delete_post_with_event_audit(
        &self,
        post_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        event: &StoredEvent,
        audit_event: &PostAuditEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<bool> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        self.soft_delete_post_event_calls
            .lock()
            .unwrap()
            .push(SoftDeletePostEventCall {
                post_id,
                actor_id,
                reason: reason.to_owned(),
                retention_until,
                event: event.clone(),
                audit_event: audit_event.clone(),
            });
        Ok(self.details.lock().unwrap().contains_key(&post_id))
    }

    async fn soft_delete_comment_with_event_audit(
        &self,
        _comment_id: Uuid,
        _post_id: Uuid,
        _actor_id: Uuid,
        _reason: &str,
        _retention_until: DateTime<Utc>,
        _event: &StoredEvent,
        _audit_event: &PostAuditEvent,
    ) -> Result<bool> {
        Ok(true)
    }

    async fn purge_expired_posts(
        &self,
        _cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<RetentionPurgeResult> {
        self.purge_calls.lock().unwrap().push(PurgeCall {
            actor_id,
            request_id,
            ip_address,
            user_agent,
            dry_run,
        });
        Ok(self.purge_result.lock().unwrap().clone())
    }

    async fn record_audit_event(&self, event: &PostAuditEvent) -> Result<()> {
        self.audit_events.lock().unwrap().push(event.clone());
        Ok(())
    }

    async fn adjust_like_count(&self, _post_id: Uuid, _delta: i64) -> Result<()> {
        Ok(())
    }

    async fn adjust_comment_count(&self, _post_id: Uuid, _delta: i64) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeLikeRepository {
    like_counts: Mutex<HashMap<Uuid, i64>>,
    liked_ids: Mutex<HashSet<Uuid>>,
    add_like_count: Mutex<i64>,
    add_like_event_calls: Mutex<Vec<LikeEventCall>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LikeEventCall {
    post_id: Uuid,
    user_id: Uuid,
    event: StoredEvent,
}

#[async_trait]
impl PostLikeRepositoryPort for FakeLikeRepository {
    async fn add_like(&self, _post_id: Uuid, _user_id: Uuid) -> Result<bool> {
        Ok(true)
    }

    async fn add_like_with_event(
        &self,
        post_id: Uuid,
        user_id: Uuid,
        event: &StoredEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<(bool, i64)> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        self.add_like_event_calls
            .lock()
            .unwrap()
            .push(LikeEventCall {
                post_id,
                user_id,
                event: event.clone(),
            });
        Ok((true, *self.add_like_count.lock().unwrap()))
    }

    async fn remove_like(&self, _post_id: Uuid, _user_id: Uuid) -> Result<bool> {
        Ok(true)
    }

    async fn remove_like_with_event(
        &self,
        _post_id: Uuid,
        _user_id: Uuid,
        _event: &StoredEvent,
        operation: Option<&post_service::application::dto::mutation_dto::MutationOperationDto>,
    ) -> Result<(bool, i64)> {
        assert!(
            operation.is_none(),
            "Use real PostgreSQL to test owner receipts"
        );
        Ok((true, 0))
    }

    async fn count_likes(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, i64>> {
        let counts = self.like_counts.lock().unwrap();
        Ok(post_ids
            .iter()
            .filter_map(|post_id| counts.get(post_id).copied().map(|count| (*post_id, count)))
            .collect())
    }

    async fn liked_post_ids(&self, _user_id: Uuid, post_ids: &[Uuid]) -> Result<HashSet<Uuid>> {
        let liked = self.liked_ids.lock().unwrap();
        Ok(post_ids
            .iter()
            .copied()
            .filter(|post_id| liked.contains(post_id))
            .collect())
    }
}

#[derive(Default)]
struct FakeFeedCache {
    cached: Mutex<Option<FeedCachePage>>,
    set_calls: Mutex<Vec<SetFeedCall>>,
    /// The generation reported to callers, and the generation the store is
    /// compared against. A test can bump this between the two calls to model a
    /// mutation arriving while a page is being computed.
    generation: Mutex<u64>,
    /// When set, reading the generation also bumps it. This models a mutation
    /// landing in the window between reading the generation and computing the
    /// page, which is exactly the interleaving the store must refuse.
    bump_after_generation_read: Mutex<bool>,
    fail_get: Mutex<bool>,
    fail_generation: Mutex<bool>,
    fail_set: Mutex<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetFeedCall {
    user_id: Uuid,
    cursor: Option<String>,
    limit: i32,
    post_ids: Vec<Uuid>,
    next_cursor: Option<String>,
    generation: u64,
}

#[async_trait]
impl FeedCachePort for FakeFeedCache {
    async fn get_feed(
        &self,
        _user_id: Uuid,
        _cursor: Option<&str>,
        _limit: i32,
    ) -> Result<Option<FeedCachePage>> {
        if *self.fail_get.lock().unwrap() {
            return Err(anyhow::anyhow!("feed cache unavailable"));
        }
        Ok(self.cached.lock().unwrap().clone())
    }

    async fn set_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        page: &FeedCachePage,
        generation: u64,
    ) -> Result<()> {
        if *self.fail_set.lock().unwrap() {
            return Err(anyhow::anyhow!("feed cache unavailable"));
        }
        // A fill that raced a mutation is discarded, exactly as the real Redis
        // adapter does, so the use case cannot pass this test while the adapter
        // would refuse the same write.
        if generation != *self.generation.lock().unwrap() {
            return Ok(());
        }
        self.set_calls.lock().unwrap().push(SetFeedCall {
            user_id,
            cursor: cursor.map(str::to_owned),
            limit,
            post_ids: page.post_ids.clone(),
            next_cursor: page.next_cursor.clone(),
            generation,
        });
        Ok(())
    }

    async fn current_generation(&self, _user_id: Uuid) -> Result<u64> {
        if *self.fail_generation.lock().unwrap() {
            return Err(anyhow::anyhow!("feed cache generation unavailable"));
        }
        let observed = *self.generation.lock().unwrap();
        if *self.bump_after_generation_read.lock().unwrap() {
            *self.generation.lock().unwrap() += 1;
        }
        Ok(observed)
    }

    async fn invalidate_users(&self, _user_ids: &[Uuid]) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FakePostCache {
    cached_posts: Mutex<HashMap<Uuid, Post>>,
    set_posts_calls: Mutex<Vec<Vec<Uuid>>>,
    fail_get: Mutex<bool>,
    fail_set: Mutex<bool>,
}

#[async_trait]
impl PostCachePort for FakePostCache {
    async fn get_posts(&self, post_ids: &[Uuid]) -> Result<HashMap<Uuid, Post>> {
        if *self.fail_get.lock().unwrap() {
            return Err(anyhow::anyhow!("post cache unavailable"));
        }
        let posts = self.cached_posts.lock().unwrap();
        Ok(post_ids
            .iter()
            .filter_map(|post_id| posts.get(post_id).cloned().map(|post| (*post_id, post)))
            .collect())
    }

    async fn set_post(&self, _post: &Post) -> Result<()> {
        Ok(())
    }

    async fn set_posts(&self, posts: &[Post]) -> Result<()> {
        if *self.fail_set.lock().unwrap() {
            return Err(anyhow::anyhow!("post cache unavailable"));
        }
        self.set_posts_calls
            .lock()
            .unwrap()
            .push(posts.iter().map(|post| post.id).collect());
        Ok(())
    }

    async fn evict(&self, _post_id: Uuid) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeFriendClient {
    feed_viewer_override: Mutex<Option<Uuid>>,
    following_ids: Mutex<Vec<Uuid>>,
    fetch_following_calls: Mutex<Vec<Uuid>>,
    is_following_result: Mutex<bool>,
    is_following_calls: Mutex<Vec<IsFollowingCall>>,
    policies: Mutex<HashMap<(Uuid, Uuid), RelationshipPolicySnapshot>>,
    omit_batch_policy_for: Mutex<Option<Uuid>>,
    next_policy_read_gate: Mutex<Option<Arc<RelationshipPolicyReadGate>>>,
}

struct RelationshipPolicyReadGate {
    reached: Notify,
    release_permit: Arc<Semaphore>,
}

impl RelationshipPolicyReadGate {
    fn new() -> Self {
        Self {
            reached: Notify::new(),
            release_permit: Arc::new(Semaphore::new(0)),
        }
    }

    async fn wait_until_reached(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(10), self.reached.notified())
            .await
            .expect("relationship policy read should reach its test barrier");
    }

    async fn pause(&self) {
        self.reached.notify_one();
        self.release_permit
            .clone()
            .acquire_owned()
            .await
            .expect("policy gate semaphore remains open")
            .forget();
    }

    fn release(&self) {
        self.release_permit.add_permits(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IsFollowingCall {
    follower_id: Uuid,
    followee_id: Uuid,
}

#[async_trait]
impl FriendClientPort for FakeFriendClient {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_service::application::feed_authors::FeedAuthorsDto> {
        let ids = self.fetch_following_ids(viewer_id).await?;
        let mut eligible = Vec::new();
        for id in ids {
            let policy = self.relationship_policy(viewer_id, id, None).await?;
            if policy.following && !policy.is_blocked() && !policy.muted_by_viewer {
                eligible.push(id);
            }
        }
        post_service::application::feed_authors::FeedAuthorsDto::new(
            self.feed_viewer_override
                .lock()
                .unwrap()
                .unwrap_or(viewer_id),
            eligible,
        )
    }

    async fn fetch_following_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        self.fetch_following_calls.lock().unwrap().push(user_id);
        Ok(self.following_ids.lock().unwrap().clone())
    }

    async fn fetch_follower_ids(&self, _user_id: Uuid) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }

    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        self.is_following_calls
            .lock()
            .unwrap()
            .push(IsFollowingCall {
                follower_id,
                followee_id,
            });
        Ok(*self.is_following_result.lock().unwrap())
    }

    async fn relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<RelationshipPolicySnapshot> {
        if let Some(policy) = self
            .policies
            .lock()
            .unwrap()
            .get(&(viewer_id, target_id))
            .copied()
        {
            return Ok(policy);
        }
        let following = self.is_following(viewer_id, target_id).await?
            || self.following_ids.lock().unwrap().contains(&target_id);
        Ok(RelationshipPolicySnapshot {
            following,
            ..RelationshipPolicySnapshot::default()
        })
    }

    async fn batch_relationship_policies(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<HashMap<Uuid, RelationshipPolicySnapshot>> {
        let gate = self.next_policy_read_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.pause().await;
        }
        let omitted = *self.omit_batch_policy_for.lock().unwrap();
        let mut policies = HashMap::with_capacity(target_ids.len());
        for target_id in target_ids.iter().copied() {
            if Some(target_id) == omitted {
                continue;
            }
            policies.insert(
                target_id,
                self.relationship_policy(viewer_id, target_id, _stage)
                    .await?,
            );
        }
        Ok(policies)
    }
}

#[derive(Default)]
struct FakeProfileClient {
    fetch_calls: Mutex<Vec<Uuid>>,
}

#[async_trait]
impl ProfileClientPort for FakeProfileClient {
    async fn require_author(
        &self,
        user_id: Uuid,
    ) -> Result<post_service::application::profile_author::ExistingProfileAuthorDto> {
        self.fetch_calls.lock().unwrap().push(user_id);
        post_service::application::profile_author::ExistingProfileAuthorDto::new(user_id)
    }
}

/// An owner that answers for a different user than the one Post asked about.
/// Post must refuse the write rather than trusting the returned identity.
#[derive(Default)]
struct ForeignProfileClient {
    fetch_calls: Mutex<Vec<Uuid>>,
    answers_with: Uuid,
}

#[async_trait]
impl ProfileClientPort for ForeignProfileClient {
    async fn require_author(
        &self,
        user_id: Uuid,
    ) -> Result<post_service::application::profile_author::ExistingProfileAuthorDto> {
        self.fetch_calls.lock().unwrap().push(user_id);
        post_service::application::profile_author::ExistingProfileAuthorDto::new(self.answers_with)
    }
}

#[derive(Default)]
struct FakeOutbox {
    events: Mutex<Vec<StoredEvent>>,
    find_events_calls: Mutex<Vec<FindEventsCall>>,
    mark_published_calls: Mutex<Vec<Uuid>>,
    delivery_attempts: Mutex<Vec<DeliveryAttempt>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FindEventsCall {
    filter: EventReplayFilter,
    limit: i32,
}

#[async_trait]
impl EventOutboxPort for FakeOutbox {
    async fn save(&self, event: &StoredEvent) -> Result<StoredEvent> {
        Ok(event.clone())
    }

    async fn find_events(
        &self,
        filter: &EventReplayFilter,
        limit: i32,
    ) -> Result<Vec<StoredEvent>> {
        self.find_events_calls.lock().unwrap().push(FindEventsCall {
            filter: filter.clone(),
            limit,
        });
        Ok(self.events.lock().unwrap().clone())
    }

    async fn find_unpublished(
        &self,
        _older_than: DateTime<Utc>,
        _limit: i32,
    ) -> Result<Vec<StoredEvent>> {
        Ok(Vec::new())
    }

    async fn append_delivery_attempt(&self, attempt: &DeliveryAttempt) -> Result<()> {
        self.delivery_attempts.lock().unwrap().push(attempt.clone());
        Ok(())
    }

    async fn mark_published(&self, event_id: Uuid, _published_at: DateTime<Utc>) -> Result<()> {
        self.mark_published_calls.lock().unwrap().push(event_id);
        Ok(())
    }

    async fn record_publish_failure(
        &self,
        _event_id: Uuid,
        _error: &str,
        _max_attempts: i32,
    ) -> Result<()> {
        Ok(())
    }

    async fn metrics_snapshot(&self, _now: DateTime<Utc>) -> Result<OutboxMetricsSnapshot> {
        Ok(OutboxMetricsSnapshot::default())
    }
}

#[derive(Default)]
struct SpyPublisher {
    published: Mutex<Vec<Uuid>>,
}

#[async_trait]
impl ExternalEventPublisherPort for SpyPublisher {
    async fn publish(&self, event: &StoredEvent) -> Result<()> {
        self.published.lock().unwrap().push(event.event_id);
        Ok(())
    }
}

#[derive(Default)]
struct SpyInternalDispatcher {
    dispatched: Mutex<Vec<Uuid>>,
}

#[async_trait]
impl InternalEventDispatcherPort for SpyInternalDispatcher {
    async fn dispatch(&self, event: &StoredEvent) -> Result<()> {
        self.dispatched.lock().unwrap().push(event.event_id);
        Ok(())
    }
}

fn post(id: Uuid, author_id: Uuid, content: &str) -> Post {
    Post {
        id,
        author_id,
        content: content.to_owned(),
        visibility: "PUBLIC".to_owned(),
        created_at: instant(1_000),
        comment_count: 0,
        like_count: 0,
    }
}

fn empty_feed_page() -> FeedPage {
    FeedPage {
        posts: Vec::new(),
        next_cursor: None,
    }
}

fn stored_event(event_id: Uuid, event_type: &str, actor_id: Option<Uuid>) -> StoredEvent {
    StoredEvent {
        event_id,
        event_type: event_type.to_owned(),
        aggregate_id: uuid("00000000-0000-0000-0000-000000000699"),
        aggregate_version: None,
        origin_traceparent: None,
        actor_id,
        attributes: vec!["public".to_owned()],
        payload: "{}".to_owned(),
        occurred_at: instant(1_500),
        created_at: instant(1_500),
        published: false,
        published_at: None,
        replay_resolution_id: None,
    }
}

fn instant(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .expect("valid instant")
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid uuid")
}

struct RecordingReplayDelivery {
    publisher: Arc<SpyPublisher>,
    internal: Arc<SpyInternalDispatcher>,
}
#[async_trait::async_trait]
impl post_service::application::ports::ReplayEventDeliveryPort for RecordingReplayDelivery {
    async fn replay_target(
        &self,
        event_id: Uuid,
        target: post_service::domain::post::DeliveryTarget,
    ) -> anyhow::Result<()> {
        match target {
            post_service::domain::post::DeliveryTarget::Internal => {
                self.internal.dispatched.lock().unwrap().push(event_id)
            }
            post_service::domain::post::DeliveryTarget::External => {
                self.publisher.published.lock().unwrap().push(event_id)
            }
            _ => anyhow::bail!("unsupported unit replay target"),
        }
        Ok(())
    }
}

#[tokio::test]
async fn cold_feed_rejects_owner_author_data_for_another_viewer_before_post_query() {
    let fixture = Fixture::new();
    let viewer = Uuid::new_v4();
    *fixture.friend.feed_viewer_override.lock().unwrap() = Some(Uuid::new_v4());
    assert!(fixture.service.get_feed(viewer, None, 20).await.is_err());
    assert!(fixture.posts.fetch_feed_calls.lock().unwrap().is_empty());
    assert!(fixture.feed_cache.set_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cold_feed_owner_contract_filters_authors_before_query_and_keeps_own_posts() {
    let fixture = Fixture::new();
    let viewer = Uuid::new_v4();
    let allowed = Uuid::new_v4();
    let blocked = Uuid::new_v4();
    let muted = Uuid::new_v4();
    *fixture.friend.following_ids.lock().unwrap() = vec![allowed, allowed, blocked, muted];
    fixture.friend.policies.lock().unwrap().insert(
        (viewer, blocked),
        RelationshipPolicySnapshot {
            following: true,
            blocked_viewer: true,
            ..Default::default()
        },
    );
    fixture.friend.policies.lock().unwrap().insert(
        (viewer, muted),
        RelationshipPolicySnapshot {
            following: true,
            muted_by_viewer: true,
            ..Default::default()
        },
    );
    fixture
        .service
        .get_feed(viewer, None, 20)
        .await
        .expect("owner-filtered query");
    assert_eq!(
        fixture.posts.fetch_feed_calls.lock().unwrap()[0].author_ids,
        vec![allowed, viewer]
    );
}
