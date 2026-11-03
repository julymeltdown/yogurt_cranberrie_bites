use super::support::{post, Harness};
use post_service::domain::feed::{FeedCachePage, FeedPage};
use post_service::domain::post::RelationshipPolicySnapshot;
use std::collections::HashSet;
use uuid::Uuid;

fn targets(count: usize) -> Vec<Uuid> {
    (1..=count)
        .map(|value| Uuid::from_u128(value as u128))
        .collect()
}

fn following(harness: &Harness, viewer: Uuid, authors: &[Uuid]) {
    *harness.friend.following_ids.lock().unwrap() = authors.to_vec();
    for &author in authors {
        harness.friend.policies.lock().unwrap().insert(
            (viewer, author),
            RelationshipPolicySnapshot {
                following: true,
                ..Default::default()
            },
        );
    }
}

#[tokio::test]
async fn cold_feed_preserves_all_authors_at_100_and_101_target_boundaries() {
    for count in [100, 101] {
        let harness = Harness::new();
        let viewer = Uuid::from_u128(10_000);
        let authors = targets(count);
        following(&harness, viewer, &authors);
        let last = post(
            Uuid::from_u128(20_000),
            authors[count - 1],
            "last followed author",
        );
        *harness.posts.feed_page.lock().unwrap() = Some(FeedPage {
            posts: vec![last.clone()],
            next_cursor: Some("owner-next-page".into()),
        });
        let result = harness
            .service
            .get_feed(viewer, None, 20)
            .await
            .expect("all follows checked through bounded requests");
        assert_eq!(result.posts[0].id, last.id.to_string());
        assert_eq!(result.next_cursor.as_deref(), Some("owner-next-page"));
        let calls = harness.posts.fetch_feed_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].author_ids,
            [authors.clone(), vec![viewer]].concat()
        );
        let policies = harness.friend.batch_policy_calls.lock().unwrap();
        assert_eq!(
            policies.len(),
            1,
            "policy reads follow observed Post authors, not the full follow list"
        );
        assert_eq!(policies[0].1, vec![authors[count - 1]]);
        assert!(policies
            .iter()
            .all(|(actor, ids)| *actor == viewer && ids.len() <= 100));
    }
}

#[tokio::test]
async fn cold_feed_deduplicates_owner_filtered_follow_set_and_checks_observed_authors() {
    let harness = Harness::new();
    let viewer = Uuid::from_u128(10_000);
    let authors = targets(205);
    following(&harness, viewer, &authors);
    harness
        .friend
        .following_ids
        .lock()
        .unwrap()
        .extend([authors[0], authors[204]]);
    for (author, policy) in [
        (authors[0], RelationshipPolicySnapshot::default()),
        (
            authors[101],
            RelationshipPolicySnapshot {
                following: true,
                muted_by_viewer: true,
                ..Default::default()
            },
        ),
        (
            authors[201],
            RelationshipPolicySnapshot {
                following: true,
                blocked_viewer: true,
                ..Default::default()
            },
        ),
    ] {
        harness
            .friend
            .policies
            .lock()
            .unwrap()
            .insert((viewer, author), policy);
    }
    let first = post(Uuid::from_u128(20_000), authors[204], "late valid author");
    let second = post(Uuid::from_u128(20_001), authors[1], "earlier valid author");
    *harness.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: vec![first.clone(), second.clone()],
        next_cursor: Some("unchanged-owner-cursor".into()),
    });
    let result = harness.service.get_feed(viewer, None, 20).await.unwrap();
    assert_eq!(
        result
            .posts
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>(),
        vec![first.id.to_string(), second.id.to_string()]
    );
    assert_eq!(
        result.next_cursor.as_deref(),
        Some("unchanged-owner-cursor")
    );
    let calls = harness.friend.batch_policy_calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        1,
        "only actual Post candidate authors need relationship policy"
    );
    assert_eq!(calls[0].1, vec![authors[1], authors[204]]);
    let expected = authors
        .into_iter()
        .filter(|a| {
            ![
                Uuid::from_u128(1),
                Uuid::from_u128(102),
                Uuid::from_u128(202),
            ]
            .contains(a)
        })
        .chain([viewer])
        .collect::<Vec<_>>();
    assert_eq!(
        harness.posts.fetch_feed_calls.lock().unwrap()[0].author_ids,
        expected
    );
}

#[tokio::test]
async fn cold_feed_rejects_missing_observed_policy_even_if_earlier_chunk_supplies_it() {
    for unsolicited in [false, true] {
        let harness = Harness::new();
        let viewer = Uuid::from_u128(10_000);
        let authors = targets(101);
        following(&harness, viewer, &authors);
        *harness.posts.feed_page.lock().unwrap() = Some(FeedPage {
            posts: authors
                .iter()
                .enumerate()
                .map(|(n, author)| {
                    post(
                        Uuid::from_u128(20_000 + n as u128),
                        *author,
                        "observed boundary candidate",
                    )
                })
                .collect(),
            next_cursor: None,
        });
        *harness.friend.omit_batch_policy_for.lock().unwrap() = Some(authors[100]);
        if unsolicited {
            *harness.friend.extra_batch_policy_for.lock().unwrap() = Some(authors[100]);
        }
        let error = harness
            .service
            .get_feed(viewer, None, 20)
            .await
            .expect_err("missing later chunk fails the whole operation");
        assert!(
            error
                .to_string()
                .contains("omitted a requested relationship policy"),
            "{error}"
        );
        assert_eq!(
            harness.posts.fetch_feed_calls.lock().unwrap().len(),
            1,
            "Post observation precedes target policy"
        );
        assert!(harness.feed_cache.set_calls.lock().unwrap().is_empty());
        assert!(harness.post_cache.set_posts.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn cold_feed_rejects_failed_observed_policy_chunk_without_cache_fill() {
    let harness = Harness::new();
    let viewer = Uuid::from_u128(10_000);
    let authors = targets(101);
    following(&harness, viewer, &authors);
    *harness.posts.feed_page.lock().unwrap() = Some(FeedPage {
        posts: authors
            .iter()
            .enumerate()
            .map(|(n, author)| {
                post(
                    Uuid::from_u128(20_000 + n as u128),
                    *author,
                    "observed boundary candidate",
                )
            })
            .collect(),
        next_cursor: None,
    });
    *harness.friend.fail_batch_policy_for.lock().unwrap() = Some(authors[100]);
    let error = harness
        .service
        .get_feed(viewer, None, 20)
        .await
        .expect_err("later chunk outage cannot return partial content");
    assert!(
        error
            .to_string()
            .contains("injected relationship batch failure"),
        "{error}"
    );
    assert_eq!(harness.friend.batch_policy_calls.lock().unwrap().len(), 2);
    assert_eq!(
        harness.posts.fetch_feed_calls.lock().unwrap().len(),
        1,
        "Post observation precedes target policy"
    );
    assert!(harness.feed_cache.set_calls.lock().unwrap().is_empty());
    assert!(harness.post_cache.set_posts.lock().unwrap().is_empty());
}

#[tokio::test]
async fn large_cached_candidate_set_checks_every_unique_author_and_preserves_order() {
    let harness = Harness::new();
    let viewer = Uuid::from_u128(10_000);
    let authors = targets(205);
    following(&harness, viewer, &authors);
    let mut candidates = authors
        .iter()
        .enumerate()
        .map(|(i, a)| post(Uuid::from_u128(20_000 + i as u128), *a, "cached candidate"))
        .collect::<Vec<_>>();
    candidates.push(post(
        Uuid::from_u128(30_000),
        authors[204],
        "same late author",
    ));
    for p in &candidates {
        harness
            .posts
            .posts_by_id
            .lock()
            .unwrap()
            .insert(p.id, p.clone());
        harness
            .post_cache
            .cached
            .lock()
            .unwrap()
            .insert(p.id, p.clone());
    }
    *harness.feed_cache.cached.lock().unwrap() = Some(FeedCachePage {
        post_ids: candidates.iter().map(|p| p.id).collect(),
        next_cursor: Some("cached-next".into()),
    });
    harness.friend.policies.lock().unwrap().insert(
        (viewer, authors[201]),
        RelationshipPolicySnapshot {
            following: true,
            blocked_by_viewer: true,
            ..Default::default()
        },
    );
    let result = harness.service.get_feed(viewer, None, 100).await.unwrap();
    assert_eq!(
        result
            .posts
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>(),
        candidates
            .iter()
            .filter(|p| p.author_id != authors[201])
            .map(|p| p.id.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(result.next_cursor.as_deref(), Some("cached-next"));
    let calls = harness.friend.batch_policy_calls.lock().unwrap();
    assert_eq!(
        calls.iter().map(|(_, ids)| ids.len()).collect::<Vec<_>>(),
        vec![100, 100, 5]
    );
    assert_eq!(
        calls
            .iter()
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect::<HashSet<_>>(),
        authors.into_iter().collect()
    );
    assert!(harness.posts.fetch_feed_calls.lock().unwrap().is_empty());
}
