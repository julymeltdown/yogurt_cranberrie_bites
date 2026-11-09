//! T-310: the liker list.
//!
//! The contract's `social.LikerPage` is author-only, paginated, and omits likers
//! who are blocked, deleted or otherwise inaccessible. A page may therefore be
//! shorter than `limit` and must still carry a correct `next_cursor`.
//!
//! The privacy property this file pins hardest: the response contains **no
//! numeric age and no gender of the liker**. The author sees who liked them, not
//! the attributes that decided whether they could.
mod intro_support;

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use concert_application::identity_claims::{
    Attribute, ClaimSource, ClaimsError, Gender, IdentityClaims, IdentityClaimsPort, ProfileId,
};
use concert_application::intro_posts::{
    audience::{Audience, GenderCondition},
    content::IntroContent,
    snapshot::Snapshot,
};
use concert_domain::age_policy::arithmetic::{derive_exact_age_claim, SeoulDate};
use concert_service::intro_posts::{
    commands::{IntroPostStore, SaveDraft},
    facts::PgIntroFacts,
    likes::{LikeError, LikeStore},
    visibility::{ComposedPolicy, VisibilityPolicy},
};
use intro_support::{pool, profile};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 9, 5, 0, 0).unwrap()
}

fn age_claim(age: i16) -> concert_domain::age_policy::arithmetic::ExactAgeClaim {
    let year = 2026 - age;
    derive_exact_age_claim(
        Some(&format!("{year:04}")),
        Some("11-09"),
        SeoulDate::new(NaiveDate::from_ymd_opt(2026, 10, 10).unwrap()),
    )
    .expect("a supported birth date")
}

fn source() -> ClaimSource {
    ClaimSource {
        verifier: "naver".to_owned(),
        source_revision: "naver.nid.me.v1".to_owned(),
        verified_at: now() - Duration::days(30),
        expires_at: now() + Duration::days(60),
    }
}

struct Fixed(Result<IdentityClaims, ClaimsError>);

#[async_trait::async_trait]
impl IdentityClaimsPort for Fixed {
    async fn read(
        &self,
        profile: ProfileId,
        at: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        match &self.0 {
            Ok(snapshot) => Ok(IdentityClaims {
                profile,
                age: snapshot.age,
                gender: snapshot.gender,
                source: snapshot.source.clone(),
                evaluated_at: at,
            }),
            Err(error) => Err(*error),
        }
    }
}

fn live_woman_25() -> IdentityClaims {
    IdentityClaims {
        profile: ProfileId::from_uuid(Uuid::nil()),
        age: Attribute::Known(age_claim(25)),
        gender: Attribute::Known(Gender::Female),
        source: Some(source()),
        evaluated_at: now(),
    }
}

fn store(pool: &PgPool) -> LikeStore {
    let policy = VisibilityPolicy::with(Arc::new(ComposedPolicy::new(
        Arc::new(Fixed(Ok(live_woman_25()))),
        Arc::new(PgIntroFacts::new(pool.clone())),
    )));
    LikeStore::with_visibility(pool.clone(), policy)
}

async fn published(pool: &PgPool, author: Uuid, audience: Audience) -> anyhow::Result<Uuid> {
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(
            author,
            intro,
            &SaveDraft {
                expected_revision: 0,
                content: IntroContent {
                    body: "An introduction with many hearts.".to_owned(),
                    currently_seeking: None,
                    interaction_style: None,
                },
                favorites_snapshot: Snapshot::empty(),
                attachment_media_ids: Vec::new(),
                audience,
                expires_at: None,
            },
        )
        .await?;
    store.publish(author, intro, saved.revision).await?;
    Ok(intro)
}

/// Record a block between two profiles, through their mirrored accounts.
async fn block(pool: &PgPool, blocker: Uuid, blocked: Uuid) -> anyhow::Result<()> {
    let blocker_account: Uuid =
        sqlx::query_scalar("select account_id from discovery_profiles where profile_id=$1")
            .bind(blocker)
            .fetch_one(pool)
            .await?;
    let blocked_account: Uuid =
        sqlx::query_scalar("select account_id from discovery_profiles where profile_id=$1")
            .bind(blocked)
            .fetch_one(pool)
            .await?;
    sqlx::query(
        "insert into discovery_blocks(blocker_account_id,blocked_account_id) values($1,$2) \
         on conflict do nothing",
    )
    .bind(blocker_account)
    .bind(blocked_account)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn a_non_author_liker_list_is_403() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t310-author").await?;
    let other = profile(&pool, "t310-other").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let error = store(&pool)
        .likers(intro, other.profile_id, 20, None)
        .await
        .expect_err("only the author may list likers");
    assert_eq!(error, LikeError::PermissionDenied);
    assert_eq!(error.status(), 403);
    Ok(())
}

#[tokio::test]
async fn the_author_sees_the_likers() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t310-author2").await?;
    let one = profile(&pool, "t310-one").await?;
    let two = profile(&pool, "t310-two").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = store(&pool);
    store.like(intro, one.profile_id).await?;
    store.like(intro, two.profile_id).await?;
    let page = store.likers(intro, author.profile_id, 20, None).await?;
    assert_eq!(page.items.len(), 2);
    let ids: std::collections::BTreeSet<Uuid> =
        page.items.iter().map(|item| item.profile_id).collect();
    assert!(ids.contains(&one.profile_id));
    assert!(ids.contains(&two.profile_id));
    Ok(())
}

/// A liker the author has blocked is absent, and the page stays well formed.
#[tokio::test]
async fn a_blocked_liker_is_omitted_and_the_page_stays_well_formed() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t310-author3").await?;
    let visible = profile(&pool, "t310-visible").await?;
    let blocked = profile(&pool, "t310-blocked").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = store(&pool);
    store.like(intro, visible.profile_id).await?;
    store.like(intro, blocked.profile_id).await?;
    // The author blocks one of their likers.
    block(&pool, author.profile_id, blocked.profile_id).await?;
    let page = store.likers(intro, author.profile_id, 20, None).await?;
    let ids: Vec<Uuid> = page.items.iter().map(|item| item.profile_id).collect();
    assert!(
        ids.contains(&visible.profile_id),
        "the accessible liker is present"
    );
    assert!(
        !ids.contains(&blocked.profile_id),
        "the blocked liker is omitted"
    );
    assert!(page.next_cursor.is_none(), "the set is exhausted");
    Ok(())
}

/// A liker who blocked the author is omitted too: a block works in both
/// directions.
#[tokio::test]
async fn a_liker_who_blocked_the_author_is_omitted() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t310-author4").await?;
    let blocker = profile(&pool, "t310-blocker").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = store(&pool);
    store.like(intro, blocker.profile_id).await?;
    block(&pool, blocker.profile_id, author.profile_id).await?;
    let page = store.likers(intro, author.profile_id, 20, None).await?;
    assert!(
        page.items.is_empty(),
        "a liker who blocked the author is not disclosed to the author"
    );
    Ok(())
}

/// A page may be shorter than `limit` and still carry a cursor: the filter runs
/// inside the query, so the cursor is derived from the rows actually returned.
#[tokio::test]
async fn a_short_page_still_carries_a_correct_cursor() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t310-author5").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let store = store(&pool);
    let mut likers = Vec::new();
    for index in 0..5 {
        let liker = profile(&pool, &format!("t310-many{index}")).await?;
        store.like(intro, liker.profile_id).await?;
        likers.push(liker);
    }
    // Block the three oldest likers, so a page of two contains only the newest
    // two and the rest of the set is entirely blocked.
    for liker in likers.iter().take(3) {
        block(&pool, author.profile_id, liker.profile_id).await?;
    }
    let page = store.likers(intro, author.profile_id, 2, None).await?;
    assert!(
        page.items.len() <= 2,
        "a page never exceeds the limit: {}",
        page.items.len()
    );
    // Walk to exhaustion and assert every returned liker is accessible and no
    // liker is repeated.
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..20 {
        let page = store
            .likers(intro, author.profile_id, 2, cursor.as_deref())
            .await?;
        seen.extend(page.items.iter().map(|item| item.profile_id));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    let unique: std::collections::BTreeSet<Uuid> = seen.iter().copied().collect();
    assert_eq!(unique.len(), seen.len(), "no liker is repeated");
    for blocked in likers.iter().take(3) {
        assert!(
            !unique.contains(&blocked.profile_id),
            "a blocked liker is never returned"
        );
    }
    for visible in likers.iter().skip(3) {
        assert!(
            unique.contains(&visible.profile_id),
            "every accessible liker is reachable"
        );
    }
    Ok(())
}

/// The response carries no numeric age and no gender of the liker.
#[tokio::test]
async fn the_liker_list_carries_no_age_or_gender() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t310-author6").await?;
    let liker = profile(&pool, "t310-private").await?;
    // A restricted post, so the liker's claim was actually consulted.
    let intro = published(
        &pool,
        author.profile_id,
        Audience::new(GenderCondition::Female, Some(18), Some(30))?,
    )
    .await?;
    let store = store(&pool);
    store.like(intro, liker.profile_id).await?;
    let page = store.likers(intro, author.profile_id, 20, None).await?;
    assert_eq!(page.items.len(), 1);
    let item = &page.items[0];
    // The projection has no age and no gender field at all. This asserts the
    // *field names*, so adding one later fails here rather than in a privacy
    // review. Matching the whole rendered value would be wrong: a nickname is
    // free text and may contain any substring.
    let rendered = format!("{item:?}");
    let fields: Vec<&str> = rendered
        .split([',', '{', '}'])
        .filter_map(|part| part.split(':').next())
        .map(str::trim)
        .collect();
    for forbidden in ["age", "gender", "birthday", "claim", "audience"] {
        assert!(
            !fields.contains(&forbidden),
            "the liker projection must not carry a {forbidden} field: {fields:?}"
        );
    }
    // The fields it does carry are exactly the contract's set.
    for expected in [
        "profile_id",
        "nickname",
        "profile_image_media_id",
        "shared_with_author",
        "interaction_style",
        "liked_at",
    ] {
        assert!(
            fields.contains(&expected),
            "the liker projection carries {expected}: {fields:?}"
        );
    }
    Ok(())
}

/// Shared catalog items are computed from ids, not from labels.
#[tokio::test]
async fn shared_items_are_computed_from_catalog_ids() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "t310-author7").await?;
    let liker = profile(&pool, "t310-shared").await?;
    let intro = published(&pool, author.profile_id, Audience::ANYONE).await?;
    let shared_id = Uuid::new_v4();
    let author_only = Uuid::new_v4();
    // The author picks two items; the liker picks the same first id and a
    // different second id that happens to share its label.
    for (profile_id, id, label, position) in [
        (author.profile_id, shared_id, "Shared record", 0),
        (author.profile_id, author_only, "Another record", 1),
        (liker.profile_id, shared_id, "Shared record", 0),
        (liker.profile_id, Uuid::new_v4(), "Another record", 1),
    ] {
        sqlx::query(
            "insert into discovery_favorite_items(profile_id,category,catalog_item_id,label,position) \
             values($1,'music',$2,$3,$4) on conflict do nothing",
        )
        .bind(profile_id)
        .bind(id)
        .bind(label)
        .bind(position)
        .execute(&pool)
        .await?;
    }
    let store = store(&pool);
    store.like(intro, liker.profile_id).await?;
    let page = store.likers(intro, author.profile_id, 20, None).await?;
    assert_eq!(page.items.len(), 1);
    let shared = &page.items[0].shared_with_author;
    assert_eq!(shared.len(), 1, "only the id-identical item is shared");
    assert_eq!(shared[0].catalog_item_id, shared_id);
    Ok(())
}
