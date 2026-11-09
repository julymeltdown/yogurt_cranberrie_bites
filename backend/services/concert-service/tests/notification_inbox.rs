//! The notification inbox: `listNotifications` and `markNotificationRead`.
//!
//! Two properties matter more than the shape:
//!
//! * **The inbox is the recipient's own.** The recipient comes from the verified
//!   session, so one profile cannot read another's notifications, and marking
//!   someone else's notification is `Unavailable` — the same answer as one that
//!   does not exist.
//!
//! * **A notification grants nothing.** The payload carries a reference — a
//!   `target_id` naming the intro post — and never the post's body, author,
//!   attachments or audience. Opening it means fetching the post, which runs the
//!   full visibility decision again. The tests below assert the payload has no
//!   content field at all, so a later change that embedded one fails here.
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
    notifications::{notification_wire, page_wire, NotificationError, NotificationInbox, NotificationQuery},
    facts::PgIntroFacts,
    likes::LikeStore,
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

/// A like store that admits a matching live claim, so a restricted post can be
/// liked and therefore notify.
fn like_store(pool: &PgPool) -> LikeStore {
    let policy = VisibilityPolicy::with(Arc::new(ComposedPolicy::new(
        Arc::new(Fixed(Ok(live_woman_25()))),
        Arc::new(PgIntroFacts::new(pool.clone())),
    )));
    LikeStore::with_visibility(pool.clone(), policy)
}

async fn published(pool: &PgPool, author: Uuid) -> anyhow::Result<Uuid> {
    let store = IntroPostStore::new(pool.clone());
    let intro = Uuid::new_v4();
    let saved = store
        .save_draft(
            author,
            intro,
            &SaveDraft {
                expected_revision: 0,
                content: IntroContent {
                    body: "A SECRET BODY that must never appear in a notification.".to_owned(),
                    currently_seeking: None,
                    interaction_style: None,
                },
                favorites_snapshot: Snapshot::empty(),
                attachment_media_ids: Vec::new(),
                audience: Audience::new(GenderCondition::Female, Some(18), Some(30))
                    .expect("a restricted audience"),
                expires_at: None,
            },
        )
        .await?;
    store.publish(author, intro, saved.revision).await?;
    Ok(intro)
}

fn query() -> NotificationQuery {
    NotificationQuery::parse(None).expect("the default query")
}

/// A like produces exactly one notification for the author.
#[tokio::test]
async fn a_like_appears_in_the_authors_inbox() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "inbox-author").await?;
    let liker = profile(&pool, "inbox-liker").await?;
    let intro = published(&pool, author.profile_id).await?;
    like_store(&pool).like(intro, liker.profile_id).await?;
    let inbox = NotificationInbox::new(pool.clone());
    let page = inbox.list(author.profile_id, &query()).await?;
    assert_eq!(page.items.len(), 1, "one like, one notification");
    let item = &page.items[0];
    assert_eq!(item.kind, "INTRO_LIKED");
    assert_eq!(item.target.target_type, "INTRO_POST");
    assert_eq!(item.target.target_id, intro);
    assert!(item.read_at.is_none(), "a fresh notification is unread");
    assert_eq!(item.revision, 1);
    Ok(())
}

/// The payload carries a reference and nothing else.
#[tokio::test]
async fn the_notification_carries_a_reference_and_no_content() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "inbox-author2").await?;
    let liker = profile(&pool, "inbox-liker2").await?;
    let intro = published(&pool, author.profile_id).await?;
    like_store(&pool).like(intro, liker.profile_id).await?;
    let inbox = NotificationInbox::new(pool.clone());
    let page = inbox.list(author.profile_id, &query()).await?;
    let wire = page_wire(&page);
    let rendered = serde_json::to_string(&wire)?;
    // The post's body must not appear anywhere in the payload.
    assert!(
        !rendered.contains("SECRET BODY"),
        "a notification must not carry the post body: {rendered}"
    );
    // Nor may the payload carry any content-bearing field.
    let item = wire["items"][0].as_object().expect("an object");
    let keys: std::collections::BTreeSet<&str> = item.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "notification_id",
            "event_id",
            "kind",
            "target",
            "created_at",
            "read_at",
            "revision"
        ]
        .into_iter()
        .collect(),
        "the notification carries exactly the contract's seven fields"
    );
    // The target is a type and an id, nothing more.
    let target = item["target"].as_object().expect("an object");
    let target_keys: std::collections::BTreeSet<&str> =
        target.keys().map(String::as_str).collect();
    assert_eq!(
        target_keys,
        ["target_type", "target_id"].into_iter().collect()
    );
    Ok(())
}

/// One profile cannot read another's inbox.
#[tokio::test]
async fn another_profile_cannot_read_the_inbox() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "inbox-author3").await?;
    let liker = profile(&pool, "inbox-liker3").await?;
    let stranger = profile(&pool, "inbox-stranger").await?;
    let intro = published(&pool, author.profile_id).await?;
    like_store(&pool).like(intro, liker.profile_id).await?;
    let inbox = NotificationInbox::new(pool.clone());
    let page = inbox.list(stranger.profile_id, &query()).await?;
    assert!(
        page.items.is_empty(),
        "a stranger's inbox is empty, not the author's"
    );
    Ok(())
}

/// Marking a notification records the read and returns the row.
#[tokio::test]
async fn marking_a_notification_read_records_it() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "inbox-author4").await?;
    let liker = profile(&pool, "inbox-liker4").await?;
    let intro = published(&pool, author.profile_id).await?;
    like_store(&pool).like(intro, liker.profile_id).await?;
    let inbox = NotificationInbox::new(pool.clone());
    let page = inbox.list(author.profile_id, &query()).await?;
    let id = page.items[0].notification_id;
    let marked = inbox.mark_read(author.profile_id, id).await?;
    assert!(marked.read_at.is_some(), "the read marker is set");
    // A replay is a no-op that returns the same row, so the command is safe
    // without a receipt.
    let again = inbox.mark_read(author.profile_id, id).await?;
    assert_eq!(again.read_at, marked.read_at, "a replay does not move the marker");
    // `unread_only` no longer lists it.
    let unread = NotificationQuery::parse(Some("unread_only=true"))?;
    let page = inbox.list(author.profile_id, &unread).await?;
    assert!(page.items.is_empty(), "a read notification is not unread");
    Ok(())
}

/// Marking someone else's notification is `Unavailable`, the same as a missing
/// one.
#[tokio::test]
async fn another_profiles_notification_is_unavailable() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "inbox-author5").await?;
    let liker = profile(&pool, "inbox-liker5").await?;
    let stranger = profile(&pool, "inbox-stranger5").await?;
    let intro = published(&pool, author.profile_id).await?;
    like_store(&pool).like(intro, liker.profile_id).await?;
    let inbox = NotificationInbox::new(pool.clone());
    let page = inbox.list(author.profile_id, &query()).await?;
    let id = page.items[0].notification_id;
    assert_eq!(
        inbox.mark_read(stranger.profile_id, id).await,
        Err(NotificationError::Unavailable),
        "another profile's notification is indistinguishable from a missing one"
    );
    assert_eq!(
        inbox.mark_read(author.profile_id, Uuid::new_v4()).await,
        Err(NotificationError::Unavailable),
        "a missing notification is the same answer"
    );
    // The author's notification is still unread: the refusal wrote nothing.
    let page = inbox.list(author.profile_id, &query()).await?;
    assert!(page.items[0].read_at.is_none());
    Ok(())
}

/// Withdrawing the like removes the notification, so the inbox follows the like.
#[tokio::test]
async fn withdrawing_the_like_removes_the_notification() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "inbox-author6").await?;
    let liker = profile(&pool, "inbox-liker6").await?;
    let intro = published(&pool, author.profile_id).await?;
    let store = like_store(&pool);
    store.like(intro, liker.profile_id).await?;
    let inbox = NotificationInbox::new(pool.clone());
    assert_eq!(inbox.list(author.profile_id, &query()).await?.items.len(), 1);
    store.unlike(intro, liker.profile_id).await?;
    assert!(
        inbox.list(author.profile_id, &query()).await?.items.is_empty(),
        "the notification follows the like out"
    );
    Ok(())
}

/// The query parser is closed.
#[test]
fn the_query_parser_is_closed_and_bounded() {
    assert_eq!(NotificationQuery::parse(None).unwrap().limit, 20);
    assert_eq!(
        NotificationQuery::parse(Some("limit=5")).unwrap().limit,
        5
    );
    assert!(NotificationQuery::parse(Some("limit=0")).is_err());
    assert!(NotificationQuery::parse(Some("limit=101")).is_err());
    assert!(NotificationQuery::parse(Some("limit=abc")).is_err());
    // An unknown field is refused, never ignored.
    assert!(NotificationQuery::parse(Some("since=yesterday")).is_err());
    assert!(NotificationQuery::parse(Some("unread_only=maybe")).is_err());
    assert!(NotificationQuery::parse(Some("limit=5&limit=6")).is_err());
    assert!(NotificationQuery::parse(Some("unread_only=true"))
        .unwrap()
        .unread_only);
}

/// The cursor is opaque and a tampered one is refused.
#[tokio::test]
async fn the_cursor_pages_without_repeating() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "inbox-author7").await?;
    let intro = published(&pool, author.profile_id).await?;
    let store = like_store(&pool);
    for index in 0..5 {
        let liker = profile(&pool, &format!("inbox-many{index}")).await?;
        store.like(intro, liker.profile_id).await?;
    }
    let inbox = NotificationInbox::new(pool.clone());
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..20 {
        let mut q = NotificationQuery::parse(Some("limit=2"))?;
        q.cursor = cursor.clone();
        let page = inbox.list(author.profile_id, &q).await?;
        seen.extend(page.items.iter().map(|i| i.notification_id));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    let unique: std::collections::BTreeSet<Uuid> = seen.iter().copied().collect();
    assert_eq!(unique.len(), seen.len(), "no notification is repeated");
    assert_eq!(unique.len(), 5, "every notification is reachable");
    // A tampered cursor is refused.
    let mut bad = NotificationQuery::parse(Some("limit=2"))?;
    bad.cursor = Some("not-a-cursor".to_owned());
    assert_eq!(
        inbox.list(author.profile_id, &bad).await,
        Err(NotificationError::InvalidCursor)
    );
    Ok(())
}

/// The wire form matches the contract's seven fields exactly.
#[tokio::test]
async fn the_wire_form_matches_the_contract() -> anyhow::Result<()> {
    let pool = pool().await?;
    let author = profile(&pool, "inbox-author8").await?;
    let liker = profile(&pool, "inbox-liker8").await?;
    let intro = published(&pool, author.profile_id).await?;
    like_store(&pool).like(intro, liker.profile_id).await?;
    let inbox = NotificationInbox::new(pool.clone());
    let page = inbox.list(author.profile_id, &query()).await?;
    let wire = notification_wire(&page.items[0]);
    assert!(wire["notification_id"].is_string());
    assert!(wire["event_id"].is_string());
    assert_eq!(wire["kind"], "INTRO_LIKED");
    assert_eq!(wire["target"]["target_type"], "INTRO_POST");
    assert_eq!(wire["target"]["target_id"], intro.to_string());
    assert!(wire["created_at"].is_string());
    assert!(wire["read_at"].is_null());
    assert_eq!(wire["revision"], 1);
    Ok(())
}
