//! Real guarded read/denial cases, each with its own five-database cluster.
use super::*;
use crate::feed::live_read::CommentsQuery;
use concert_application::feed::Context;
use concert_domain::feed::FeedError;

impl Fixture {
    fn context(&self) -> anyhow::Result<Context> {
        let b = self.scope.binding();
        Ok(Context {
            actor: b.actor(),
            event: b.event(),
            session_family: b.session_family(),
            request: b.request_id(),
            authority_digest: b.parent_digest(),
            expires_at: chrono::DateTime::from_timestamp_micros(b.deadline().expires_micros())
                .ok_or_else(|| anyhow::anyhow!("Original deadline required"))?,
        })
    }
    fn feed(&self, endpoint: &str) -> anyhow::Result<QualifiedReadFeed<WallClock>> {
        let gateway =
            ActorDelegationVerifier::new(Workload::Post, &[capture::gateway()?.public_key()])?;
        QualifiedReadFeed::connect(
            endpoint,
            "concert-fixture",
            &environment::tls(Workload::Concert)?,
            ConcertPostReadSigner::from_seed(&[76; 32], &gateway)?,
            self.scope.clone(),
            self.reader.clone(),
            CursorCodec::new(&[79; 32])?,
            WallClock,
        )
    }
    async fn seed_post(&self, post: Uuid) -> anyhow::Result<()> {
        sqlx::query("insert into post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at) values($1,$2,$3,$4,1,'PUBLISHED',clock_timestamp()-interval '1 minute')")
            .bind(post).bind(self.scope.binding().event()).bind(self.subject)
            .bind(serde_json::json!({"text":"Actual stored parent","links":[],"media_ids":[]}))
            .execute(&self.post).await?;
        Ok(())
    }
    async fn seed_comment(&self, post: Uuid) -> anyhow::Result<Uuid> {
        let comment = Uuid::new_v4();
        sqlx::query("insert into post_concert_comments(comment_id,event_id,post_id,author_id,text,revision,state,created_at) values($1,$2,$3,$4,'Actual stored comment',1,'PUBLISHED',clock_timestamp()-interval '30 seconds')")
            .bind(comment).bind(self.scope.binding().event()).bind(post).bind(self.subject)
            .execute(&self.post).await?;
        sqlx::query("update post_concert_posts set comment_count=1 where post_id=$1")
            .bind(post)
            .execute(&self.post)
            .await?;
        Ok(comment)
    }
    async fn tracking_count(&self) -> anyhow::Result<i64> {
        Ok(
            sqlx::query_scalar("select count(*) from post_concert_read_tracking")
                .fetch_one(&self.post)
                .await?,
        )
    }
    async fn assert_exclusions_held(&self) -> anyhow::Result<()> {
        for pool in [&self.auth, &self.friend, &self.concert, &self.profile] {
            let count: i64 = sqlx::query_scalar(
                "select count(*) from pg_prepared_xacts where database=current_database()",
            )
            .fetch_one(pool)
            .await?;
            assert!(
                count > 0,
                "Actual owner/local/Profile exclusions remain held after DTO assembly"
            );
        }
        assert!(
            self.tracking_count().await? > 0,
            "Protected reads require real Post transaction tracking"
        );
        Ok(())
    }
}
fn assert_author(value: &Value, subject: Uuid) {
    assert_eq!(value["user_id"], subject.to_string());
    assert_eq!(value["display_name"], "Current canonical name");
    assert!(!value.to_string().contains("private-url-canary"));
}

#[tokio::test]
#[ignore = "requires frozen owned five-database PG16/mTLS Feed join controller"]
async fn actual_detail_reads_post_and_comments_under_held_guards() -> anyhow::Result<()> {
    let post = Uuid::new_v4();
    let f = Fixture::new(capture::Route::Detail(post)).await?;
    f.seed_post(post).await?;
    let comment = f.seed_comment(post).await?;
    let context = f.context()?;
    let feed = f.feed(&f.post_url)?;
    let body = f
        .scope
        .run(async {
            feed.detail(context, post)
                .await
                .map_err(|e| Status::unavailable(format!("native detail: {e:?}")))
        })
        .await?;
    assert_eq!(body["post"]["post_id"], post.to_string());
    assert_eq!(body["post"]["text"], "Actual stored parent");
    assert_author(&body["post"]["author"], f.subject);
    assert_eq!(body["comments"].as_array().unwrap().len(), 1);
    assert_eq!(body["comments"][0]["comment_id"], comment.to_string());
    assert_author(&body["comments"][0]["author"], f.subject);
    f.assert_exclusions_held().await?;
    f.finish().await
}

#[tokio::test]
#[ignore = "requires frozen owned five-database PG16/mTLS Feed join controller"]
async fn actual_comments_read_current_parent_and_public_author() -> anyhow::Result<()> {
    let post = Uuid::new_v4();
    let f = Fixture::new(capture::Route::Comments(post)).await?;
    f.seed_post(post).await?;
    let comment = f.seed_comment(post).await?;
    let context = f.context()?;
    let feed = f.feed(&f.post_url)?;
    let body = f
        .scope
        .run(async {
            feed.comments(context, post, CommentsQuery::default())
                .await
                .map_err(|e| Status::unavailable(format!("native comments: {e:?}")))
        })
        .await?;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["comment_id"], comment.to_string());
    assert_eq!(body["items"][0]["post_id"], post.to_string());
    assert_eq!(body["items"][0]["text"], "Actual stored comment");
    assert_author(&body["items"][0]["author"], f.subject);
    f.assert_exclusions_held().await?;
    f.finish().await
}

#[tokio::test]
#[ignore = "requires frozen owned five-database PG16/mTLS Feed join controller"]
async fn actual_private_author_is_filtered_after_real_post_scan() -> anyhow::Result<()> {
    let f = Fixture::new(capture::Route::Feed).await?;
    f.seed_post(Uuid::new_v4()).await?;
    sqlx::query("update profiles set account_visibility='PRIVATE' where user_id=$1")
        .bind(f.subject)
        .execute(&f.profile)
        .await?;
    let context = f.context()?;
    let feed = f.feed(&f.post_url)?;
    let body = f
        .scope
        .run(async {
            feed.page(context, FeedQuery::default())
                .await
                .map_err(|e| Status::unavailable(format!("native private author: {e:?}")))
        })
        .await?;
    assert_eq!(body["items"], serde_json::json!([]));
    assert!(
        f.tracking_count().await? > 0,
        "An actual Post scan precedes explicit privacy filtering"
    );
    let stored: i64 = sqlx::query_scalar("select count(*) from post_concert_posts")
        .fetch_one(&f.post)
        .await?;
    assert_eq!(
        stored, 1,
        "The empty page is a current privacy decision, not missing source content"
    );
    f.finish().await
}

#[tokio::test]
#[ignore = "requires frozen owned five-database PG16/mTLS Feed join controller"]
async fn actual_post_outage_cannot_become_an_empty_page() -> anyhow::Result<()> {
    let f = Fixture::new(capture::Route::Feed).await?;
    f.seed_post(Uuid::new_v4()).await?;
    // Reserve an actual private loopback port but serve no RPC there. This
    // supplies no fake authority and is a deterministic unavailable dependency.
    let unavailable = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", unavailable.local_addr()?);
    let feed = f.feed(&endpoint)?;
    let context = f.context()?;
    let error = feed.page(context, FeedQuery::default()).await.unwrap_err();
    assert_eq!(error, FeedError::DependencyUnavailable);
    assert_eq!(f.tracking_count().await?, 0);
    drop(unavailable);
    f.finish().await
}

#[tokio::test]
#[ignore = "requires frozen owned five-database PG16/mTLS Feed join controller"]
async fn actual_sealed_root_cannot_authorize_a_later_read() -> anyhow::Result<()> {
    let f = Fixture::new(capture::Route::Feed).await?;
    f.seed_post(Uuid::new_v4()).await?;
    let context = f.context()?;
    let feed = f.feed(&f.post_url)?;
    f.finish().await?;
    assert_eq!(
        feed.page(context, FeedQuery::default()).await,
        Err(FeedError::DependencyUnavailable)
    );
    assert_eq!(f.tracking_count().await?, 0);
    f.assert_clean().await
}
