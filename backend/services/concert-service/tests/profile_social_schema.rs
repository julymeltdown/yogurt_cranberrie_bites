//! WP-02 T-106 … T-111: concert-service profile-owned schema invariants.
//!
//! Every negative case asserts the specific constraint name that fires. The
//! database is the runner-owned private cluster described by
//! `backend/scripts/mcr/migration_smoke.py`; the URL arrives through
//! `CONCERT_TEST_DATABASE_URL` and must name the `mcr_owner` role.
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

async fn pool() -> anyhow::Result<PgPool> {
    let url = std::env::var("CONCERT_TEST_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("CONCERT_TEST_DATABASE_URL is required"))?;
    let options: sqlx::postgres::PgConnectOptions = url.parse()?;
    anyhow::ensure!(
        options.get_username() == "mcr_owner",
        "schema fixture must use the owned cluster"
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp02-concert-migrations',0))")
        .execute(&mut *tx)
        .await?;
    let applied: bool =
        sqlx::query_scalar("select to_regclass('public.profile_media') is not null")
            .fetch_one(&mut *tx)
            .await?;
    if applied {
        tx.commit().await?;
        return Ok(());
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries: Vec<_> = std::fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "sql")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".down.sql"))
        })
        .collect();
    entries.sort();
    for path in entries {
        let sql = std::fs::read_to_string(&path)?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

fn constraint(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(database) => database.constraint().map(str::to_owned),
        _ => None,
    }
}

async fn profile(pool: &PgPool) -> anyhow::Result<Uuid> {
    let profile = Uuid::new_v4();
    sqlx::query("insert into wp02_test_profiles(profile_id) values($1)")
        .bind(profile)
        .execute(pool)
        .await?;
    Ok(profile)
}

/// The concert service has no foreign key to the profile service's database, so
/// these fixtures stand in for the profile ids the real product would carry.
async fn install_fixture_table(pool: &PgPool) -> anyhow::Result<()> {
    // Concurrent fixtures race on `create table if not exists`, which is not
    // atomic in PostgreSQL 16; take the fixture lock first.
    let mut tx = pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('wp02-concert-fixture',0))")
        .execute(&mut *tx)
        .await?;
    sqlx::raw_sql(
        "create table if not exists wp02_test_profiles(profile_id uuid primary key)",
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// A READY media row, which most children need.
async fn ready_media(pool: &PgPool, owner: Uuid, purpose: &str) -> anyhow::Result<Uuid> {
    let media = Uuid::new_v4();
    sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,width,height,input_sha256) \
         values($1,$2,$3,'image/png','READY',4,decode('89504e47','hex'),2,2,\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(media)
    .bind(owner)
    .bind(purpose)
    .execute(pool)
    .await?;
    Ok(media)
}

async fn intro(pool: &PgPool, author: Uuid) -> anyhow::Result<Uuid> {
    let intro = Uuid::new_v4();
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at) \
         values($1,$2,'hello','PUBLISHED',clock_timestamp())",
    )
    .bind(intro)
    .bind(author)
    .execute(pool)
    .await?;
    Ok(intro)
}

// ---------------------------------------------------------------------------
// T-106 profile_media
// ---------------------------------------------------------------------------

#[tokio::test]
async fn profile_media_purpose_is_the_closed_profile_set() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let owner = profile(&pool).await?;
    for purpose in ["PROFILE_IMAGE", "PROFILE_POST", "INTRO_POST", "ROOM_POSTER"] {
        ready_media(&pool, owner, purpose).await?;
    }
    // The concert purposes belong to concert_media and must be refused here.
    for purpose in ["POST", "GIVEAWAY", "REPORT"] {
        let error = sqlx::query(
            "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
             declared_bytes,input_sha256) \
             values($1,$2,$3,'image/png','PENDING',4,decode(repeat('ab',32),'hex'))",
        )
        .bind(Uuid::new_v4())
        .bind(owner)
        .bind(purpose)
        .execute(&pool)
        .await
        .expect_err("a concert purpose must be refused by profile_media");
        assert_eq!(
            constraint(&error).as_deref(),
            Some("profile_media_purpose"),
            "purpose {purpose}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn profile_media_state_machine_is_enforced() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let owner = profile(&pool).await?;
    // READY without sanitized bytes.
    let error = sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,width,height,input_sha256) \
         values($1,$2,'PROFILE_POST','image/png','READY',4,2,2,decode(repeat('ab',32),'hex'))",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("READY without bytes must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_media_state_bytes")
    );
    // READY without dimensions.
    let error = sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,input_sha256) \
         values($1,$2,'PROFILE_POST','image/png','READY',4,decode('89504e47','hex'),\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("READY without dimensions must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_media_state_bytes")
    );
    // PENDING with bytes.
    let error = sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,input_sha256) \
         values($1,$2,'PROFILE_POST','image/png','PENDING',4,decode('89504e47','hex'),\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("PENDING with bytes must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_media_state_bytes")
    );
    Ok(())
}

#[tokio::test]
async fn profile_media_dimensions_and_pixels_are_bounded() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let owner = profile(&pool).await?;
    // Width above 4096.
    let error = sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,width,height,input_sha256) \
         values($1,$2,'PROFILE_POST','image/png','READY',4,decode('89504e47','hex'),4097,2,\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("width above 4096 must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_media_dimension")
    );
    // Height above 4096.
    let error = sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,width,height,input_sha256) \
         values($1,$2,'PROFILE_POST','image/png','READY',4,decode('89504e47','hex'),2,4097,\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("height above 4096 must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_media_height")
    );
    // 4096 x 4096 exceeds 16 M pixels.
    let error = sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,width,height,input_sha256) \
         values($1,$2,'PROFILE_POST','image/png','READY',4,decode('89504e47','hex'),4096,4096,\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("more than 16 M pixels must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_media_pixels")
    );
    // Exactly 4000 x 4000 is allowed.
    sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,sanitized,width,height,input_sha256) \
         values($1,$2,'PROFILE_POST','image/png','READY',4,decode('89504e47','hex'),4000,4000,\
         decode(repeat('ab',32),'hex'))",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn profile_media_rejects_gif_and_has_no_event_column() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let owner = profile(&pool).await?;
    let error = sqlx::query(
        "insert into profile_media(media_id,owner_profile_id,purpose,content_type,state,\
         declared_bytes,input_sha256) \
         values($1,$2,'PROFILE_POST','image/gif','PENDING',4,decode(repeat('ab',32),'hex'))",
    )
    .bind(Uuid::new_v4())
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("GIF must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_media_content_type")
    );
    // No profile-owned table may carry an event column.
    let columns: Vec<String> = sqlx::query_scalar(
        "select table_name || '.' || column_name from information_schema.columns \
         where table_schema='public' and table_name in \
           ('profile_media','profile_rooms','intro_posts','intro_attachments',\
            'intro_likes','intro_comments','profile_posts','profile_post_attachments') \
           and column_name like '%event%'",
    )
    .fetch_all(&pool)
    .await?;
    assert!(
        columns.is_empty(),
        "no profile-owned table may carry an event column: {columns:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T-107 profile_rooms
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_profile_has_at_most_one_room() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let owner = profile(&pool).await?;
    sqlx::query("insert into profile_rooms(owner_profile_id,layout) values($1,'{}'::jsonb)")
        .bind(owner)
        .execute(&pool)
        .await?;
    let error = sqlx::query("insert into profile_rooms(owner_profile_id,layout) values($1,'{}'::jsonb)")
        .bind(owner)
        .execute(&pool)
        .await
        .expect_err("a second room must be refused");
    assert_eq!(constraint(&error).as_deref(), Some("profile_rooms_pkey"));
    Ok(())
}

#[tokio::test]
async fn room_layout_must_be_a_json_object() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let owner = profile(&pool).await?;
    for layout in ["[]", "\"text\"", "1", "null"] {
        let error = sqlx::query("insert into profile_rooms(owner_profile_id,layout) values($1,$2::jsonb)")
            .bind(owner)
            .bind(layout)
            .execute(&pool)
            .await
            .expect_err("a non-object layout must be refused");
        assert_eq!(
            constraint(&error).as_deref(),
            Some("profile_rooms_layout_object"),
            "layout {layout}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn room_visibility_and_revision_are_closed() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let owner = profile(&pool).await?;
    let error = sqlx::query(
        "insert into profile_rooms(owner_profile_id,layout,visibility) values($1,'{}'::jsonb,'FRIENDS')",
    )
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("an unknown visibility must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_rooms_visibility")
    );
    sqlx::query("insert into profile_rooms(owner_profile_id,layout) values($1,'{}'::jsonb)")
        .bind(owner)
        .execute(&pool)
        .await?;
    // A save must advance the revision exactly once.
    let error = sqlx::query(
        "update profile_rooms set layout='{\"v\":1}'::jsonb, revision=revision+2 where owner_profile_id=$1",
    )
    .bind(owner)
    .execute(&pool)
    .await
    .expect_err("a skipped revision must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_rooms_revision_step")
    );
    sqlx::query("update profile_rooms set layout='{\"v\":1}'::jsonb, revision=revision+1 where owner_profile_id=$1")
        .bind(owner)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn a_room_has_no_intro_or_event_reference() -> anyhow::Result<()> {
    let pool = pool().await?;
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name::text from information_schema.columns \
         where table_schema='public' and table_name='profile_rooms' \
           and column_name in ('intro_id','event_id','room_id')",
    )
    .fetch_all(&pool)
    .await?;
    assert!(columns.is_empty(), "profile_rooms must not carry {columns:?}");
    Ok(())
}

// ---------------------------------------------------------------------------
// T-108 intro_posts
// ---------------------------------------------------------------------------

#[tokio::test]
async fn intro_audience_bounds_must_be_paired() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    // Only a minimum.
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at,\
         audience_min_age) values($1,$2,'hello','PUBLISHED',clock_timestamp(),18)",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("an unpaired minimum must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_posts_audience_bounds_paired")
    );
    // Only a maximum.
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at,\
         audience_max_age_exclusive) values($1,$2,'hello','PUBLISHED',clock_timestamp(),30)",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("an unpaired maximum must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_posts_audience_bounds_paired")
    );
    Ok(())
}

#[tokio::test]
async fn intro_audience_bounds_are_ordered_and_in_range() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    // minimum >= maximum.
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at,\
         audience_min_age,audience_max_age_exclusive) \
         values($1,$2,'hello','PUBLISHED',clock_timestamp(),30,30)",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("minimum >= maximum must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_posts_audience_bounds")
    );
    // Below the supported floor.
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at,\
         audience_min_age,audience_max_age_exclusive) \
         values($1,$2,'hello','PUBLISHED',clock_timestamp(),13,30)",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("a minimum below 14 must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_posts_audience_bounds")
    );
    // Above the supported ceiling.
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at,\
         audience_min_age,audience_max_age_exclusive) \
         values($1,$2,'hello','PUBLISHED',clock_timestamp(),18,102)",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("a maximum above 101 must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_posts_audience_bounds")
    );
    // A well-formed window is accepted.
    sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at,\
         audience_min_age,audience_max_age_exclusive) \
         values($1,$2,'hello','PUBLISHED',clock_timestamp(),18,30)",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn intro_audience_gender_and_state_are_closed() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    for gender in ["any", "female", "male", "unknown"] {
        sqlx::query(
            "insert into intro_posts(intro_id,author_profile_id,body,state,published_at,\
             audience_gender) values($1,$2,'hello','PUBLISHED',clock_timestamp(),$3)",
        )
        .bind(Uuid::new_v4())
        .bind(author)
        .bind(gender)
        .execute(&pool)
        .await?;
    }
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at,\
         audience_gender) values($1,$2,'hello','PUBLISHED',clock_timestamp(),'all')",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("an unknown gender condition must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_posts_audience_gender")
    );
    // The state enum is closed too, and a published post must be published.
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state) \
         values($1,$2,'hello','CLOSED')",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("an unknown state must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_posts_state")
    );
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state) \
         values($1,$2,'hello','PUBLISHED')",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("a published post without a publication instant must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_posts_published_shape")
    );
    Ok(())
}

#[tokio::test]
async fn intro_posts_have_no_room_or_event_column() -> anyhow::Result<()> {
    let pool = pool().await?;
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name::text from information_schema.columns \
         where table_schema='public' and table_name='intro_posts' \
           and column_name in ('room_id','event_id')",
    )
    .fetch_all(&pool)
    .await?;
    assert!(
        columns.is_empty(),
        "intro_posts must not carry a room or event column: {columns:?}"
    );
    // The discovery index is partial on PUBLISHED.
    let predicate: Option<String> = sqlx::query_scalar(
        "select pg_get_expr(i.indpred, i.indrelid) from pg_index i \
         where i.indexrelid = to_regclass('public.intro_posts_discovery')",
    )
    .fetch_one(&pool)
    .await?;
    let predicate = predicate.expect("intro_posts_discovery must be partial");
    assert!(
        predicate.contains("PUBLISHED"),
        "discovery index must be restricted to published posts: {predicate}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T-109 intro children
// ---------------------------------------------------------------------------

#[tokio::test]
async fn intro_attachments_are_limited_to_four_slots() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let intro = intro(&pool, author).await?;
    for position in 0..4i16 {
        let media = ready_media(&pool, author, "INTRO_POST").await?;
        sqlx::query("insert into intro_attachments(intro_id,media_id,position) values($1,$2,$3)")
            .bind(intro)
            .bind(media)
            .bind(position)
            .execute(&pool)
            .await?;
    }
    let media = ready_media(&pool, author, "INTRO_POST").await?;
    let error = sqlx::query("insert into intro_attachments(intro_id,media_id,position) values($1,$2,4)")
        .bind(intro)
        .bind(media)
        .execute(&pool)
        .await
        .expect_err("a fifth attachment must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_attachments_position")
    );
    Ok(())
}

#[tokio::test]
async fn intro_attachments_require_intro_purpose_and_author_ownership() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let other = profile(&pool).await?;
    let intro = intro(&pool, author).await?;
    // Wrong purpose.
    let media = ready_media(&pool, author, "PROFILE_POST").await?;
    let error = sqlx::query("insert into intro_attachments(intro_id,media_id,position) values($1,$2,0)")
        .bind(intro)
        .bind(media)
        .execute(&pool)
        .await
        .expect_err("a PROFILE_POST asset must not attach to an intro");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_attachments_purpose")
    );
    // Another user's media.
    let media = ready_media(&pool, other, "INTRO_POST").await?;
    let error = sqlx::query("insert into intro_attachments(intro_id,media_id,position) values($1,$2,0)")
        .bind(intro)
        .bind(media)
        .execute(&pool)
        .await
        .expect_err("another user's media must not attach");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_attachments_owner")
    );
    Ok(())
}

#[tokio::test]
async fn a_like_is_unique_and_the_author_cannot_like_their_own_post() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let viewer = profile(&pool).await?;
    let intro = intro(&pool, author).await?;
    sqlx::query("insert into intro_likes(intro_id,actor_profile_id) values($1,$2)")
        .bind(intro)
        .bind(viewer)
        .execute(&pool)
        .await?;
    let error = sqlx::query("insert into intro_likes(intro_id,actor_profile_id) values($1,$2)")
        .bind(intro)
        .bind(viewer)
        .execute(&pool)
        .await
        .expect_err("a duplicate like must be refused");
    assert_eq!(constraint(&error).as_deref(), Some("intro_likes_pkey"));
    let error = sqlx::query("insert into intro_likes(intro_id,actor_profile_id) values($1,$2)")
        .bind(intro)
        .bind(author)
        .execute(&pool)
        .await
        .expect_err("a self-like must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_likes_no_self_like")
    );
    Ok(())
}

#[tokio::test]
async fn intro_likes_carry_no_reward_trigger() -> anyhow::Result<()> {
    let pool = pool().await?;
    // A reward trigger would be a non-internal trigger on intro_likes whose
    // function writes somewhere else. There must be exactly one trigger and it
    // must be the self-like guard, which only reads.
    let triggers: Vec<(String, String)> = sqlx::query_as(
        "select t.tgname::text, p.proname::text from pg_trigger t \
         join pg_proc p on p.oid = t.tgfoid \
         where t.tgrelid = 'public.intro_likes'::regclass and not t.tgisinternal \
         order by t.tgname",
    )
    .fetch_all(&pool)
    .await?;
    // Exactly two triggers are allowed: the self-like guard and the account
    // fence guard that every profile-owned table shares. Anything else is a
    // reaction to a like, which this product does not have.
    assert_eq!(
        triggers,
        vec![
            (
                "intro_likes_account_guard".to_owned(),
                "profile_social_write_guard".to_owned()
            ),
            (
                "intro_likes_no_self_like".to_owned(),
                "intro_likes_no_self_like".to_owned()
            ),
        ],
        "intro_likes must carry only its guards and no reward trigger"
    );
    // Neither guard writes anywhere: a like never mints a reward, score or
    // notification. The like notification is written by the command, which
    // carries the command's authority and idempotency key.
    for function in ["intro_likes_no_self_like()", "profile_social_write_guard()"] {
        let definition: String = sqlx::query_scalar(&format!(
            "select pg_get_functiondef('public.{function}'::regprocedure)"
        ))
        .fetch_one(&pool)
        .await?;
        let lowered = definition.to_lowercase();
        for forbidden in ["insert into", "update ", "delete from", "reward", "score", "balance", "point"] {
            assert!(
                !lowered.contains(forbidden),
                "{function} must not write or reward: found {forbidden}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn comments_are_bounded_and_one_level_deep() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let post = intro(&pool, author).await?;
    let other_intro = intro(&pool, author).await?;
    // Empty body.
    let error = sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,body) \
         values($1,$2,$3,'')",
    )
    .bind(Uuid::new_v4())
    .bind(post)
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("an empty comment must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_comments_body_length")
    );
    // Body above 500.
    let error = sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,body) \
         values($1,$2,$3,$4)",
    )
    .bind(Uuid::new_v4())
    .bind(post)
    .bind(author)
    .bind("z".repeat(501))
    .execute(&pool)
    .await
    .expect_err("a 501-character comment must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_comments_body_length")
    );
    let parent = Uuid::new_v4();
    sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,body) \
         values($1,$2,$3,'parent')",
    )
    .bind(parent)
    .bind(post)
    .bind(author)
    .execute(&pool)
    .await?;
    // A parent from a different intro.
    let error = sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,parent_comment_id,body) \
         values($1,$2,$3,$4,'reply')",
    )
    .bind(Uuid::new_v4())
    .bind(other_intro)
    .bind(author)
    .bind(parent)
    .execute(&pool)
    .await
    .expect_err("a cross-intro parent must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_comments_parent_intro")
    );
    // A reply to a reply.
    let reply = Uuid::new_v4();
    sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,parent_comment_id,body) \
         values($1,$2,$3,$4,'reply')",
    )
    .bind(reply)
    .bind(post)
    .bind(author)
    .bind(parent)
    .execute(&pool)
    .await?;
    let error = sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,parent_comment_id,body) \
         values($1,$2,$3,$4,'nested')",
    )
    .bind(Uuid::new_v4())
    .bind(post)
    .bind(author)
    .bind(reply)
    .execute(&pool)
    .await
    .expect_err("a reply to a reply must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_comments_one_level")
    );
    Ok(())
}

#[tokio::test]
async fn comment_state_is_closed_and_terminal() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let intro = intro(&pool, author).await?;
    let error = sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,body,state) \
         values($1,$2,$3,'hello','HIDDEN')",
    )
    .bind(Uuid::new_v4())
    .bind(intro)
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("an unknown comment state must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_comments_state")
    );
    let comment = Uuid::new_v4();
    sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,body) \
         values($1,$2,$3,'hello')",
    )
    .bind(comment)
    .bind(intro)
    .bind(author)
    .execute(&pool)
    .await?;
    sqlx::query("update intro_comments set state='DELETED' where comment_id=$1")
        .bind(comment)
        .execute(&pool)
        .await?;
    let error = sqlx::query("update intro_comments set state='PUBLISHED' where comment_id=$1")
        .bind(comment)
        .execute(&pool)
        .await
        .expect_err("a deleted comment must not be revived");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("intro_comments_deleted_terminal")
    );
    Ok(())
}

#[tokio::test]
async fn intro_likes_have_a_page_index() -> anyhow::Result<()> {
    let pool = pool().await?;
    let index: Option<String> = sqlx::query_scalar(
        "select pg_get_indexdef(to_regclass('public.intro_likes_page'))",
    )
    .fetch_one(&pool)
    .await?;
    let index = index.expect("intro_likes_page must exist");
    assert!(index.contains("intro_id"), "{index}");
    assert!(index.contains("created_at DESC"), "{index}");
    assert!(index.contains("actor_profile_id DESC"), "{index}");
    Ok(())
}

// ---------------------------------------------------------------------------
// T-110 profile posts
// ---------------------------------------------------------------------------

#[tokio::test]
async fn profile_posts_have_their_own_visibility_and_state() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let error = sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description,visibility) \
         values($1,$2,'hello','FRIENDS')",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("an unknown visibility must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_posts_visibility")
    );
    let error = sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description,state) \
         values($1,$2,'hello','DRAFT')",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("an unknown state must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_posts_state")
    );
    // A deleted post must carry a deletion instant.
    let error = sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description,state) \
         values($1,$2,'hello','DELETED')",
    )
    .bind(Uuid::new_v4())
    .bind(author)
    .execute(&pool)
    .await
    .expect_err("a deleted post without an instant must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_posts_deleted_shape")
    );
    Ok(())
}

#[tokio::test]
async fn a_profile_post_cannot_attach_intro_media() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let post = Uuid::new_v4();
    sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description) values($1,$2,'hello')",
    )
    .bind(post)
    .bind(author)
    .execute(&pool)
    .await?;
    let media = ready_media(&pool, author, "INTRO_POST").await?;
    let error = sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,0)")
        .bind(post)
        .bind(media)
        .execute(&pool)
        .await
        .expect_err("an INTRO_POST asset must not attach to a profile post");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_post_attachments_purpose")
    );
    // A PROFILE_POST asset owned by the author is accepted.
    let media = ready_media(&pool, author, "PROFILE_POST").await?;
    sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,0)")
        .bind(post)
        .bind(media)
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn profile_post_attachments_are_limited_to_four_slots() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let post = Uuid::new_v4();
    sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description) values($1,$2,'hello')",
    )
    .bind(post)
    .bind(author)
    .execute(&pool)
    .await?;
    for position in 0..4i16 {
        let media = ready_media(&pool, author, "PROFILE_POST").await?;
        sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,$3)")
            .bind(post)
            .bind(media)
            .bind(position)
            .execute(&pool)
            .await?;
    }
    let media = ready_media(&pool, author, "PROFILE_POST").await?;
    let error = sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,4)")
        .bind(post)
        .bind(media)
        .execute(&pool)
        .await
        .expect_err("a fifth attachment must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_post_attachments_position")
    );
    Ok(())
}

#[tokio::test]
async fn profile_posts_have_no_intro_reference() -> anyhow::Result<()> {
    let pool = pool().await?;
    let columns: Vec<String> = sqlx::query_scalar(
        "select column_name::text from information_schema.columns \
         where table_schema='public' and table_name in ('profile_posts','profile_post_attachments') \
           and column_name in ('intro_id','event_id')",
    )
    .fetch_all(&pool)
    .await?;
    assert!(
        columns.is_empty(),
        "profile posts must not reference an intro or an event: {columns:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T-111 account deletion coverage
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_media_file_shared_by_two_posts_survives_until_both_are_deleted() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let media = ready_media(&pool, author, "PROFILE_POST").await?;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    for post in [first, second] {
        sqlx::query(
            "insert into profile_posts(post_id,author_profile_id,description) values($1,$2,'hello')",
        )
        .bind(post)
        .bind(author)
        .execute(&pool)
        .await?;
    }
    sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,0)")
        .bind(first)
        .bind(media)
        .execute(&pool)
        .await?;
    sqlx::query("insert into profile_post_attachments(post_id,media_id,position) values($1,$2,0)")
        .bind(second)
        .bind(media)
        .execute(&pool)
        .await?;
    // Still referenced: the erase is refused and the file survives.
    let erased: bool = sqlx::query_scalar("select profile_media_erase_unreferenced($1)")
        .bind(media)
        .fetch_one(&pool)
        .await?;
    assert!(!erased, "a referenced media file must not be erased");
    let state: String = sqlx::query_scalar("select state from profile_media where media_id=$1")
        .bind(media)
        .fetch_one(&pool)
        .await?;
    assert_eq!(state, "READY");
    // Delete one post: still referenced by the other.
    sqlx::query("delete from profile_posts where post_id=$1")
        .bind(first)
        .execute(&pool)
        .await?;
    let erased: bool = sqlx::query_scalar("select profile_media_erase_unreferenced($1)")
        .bind(media)
        .fetch_one(&pool)
        .await?;
    assert!(!erased, "one remaining reference still protects the file");
    // Delete the second: now the file is unreferenced and is erased.
    sqlx::query("delete from profile_posts where post_id=$1")
        .bind(second)
        .execute(&pool)
        .await?;
    let erased: bool = sqlx::query_scalar("select profile_media_erase_unreferenced($1)")
        .bind(media)
        .fetch_one(&pool)
        .await?;
    assert!(erased, "an unreferenced media file must be erased");
    let (state, sanitized): (String, Option<Vec<u8>>) =
        sqlx::query_as("select state, sanitized from profile_media where media_id=$1")
            .bind(media)
            .fetch_one(&pool)
            .await?;
    assert_eq!(state, "DELETED");
    assert!(sanitized.is_none(), "erased media must not retain bytes");
    Ok(())
}

#[tokio::test]
async fn deletion_receipt_progress_is_monotonic_and_terminal_only_when_clean(
) -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let user = Uuid::new_v4();
    let saga = Uuid::new_v4();
    // A room the account still owns, written before the fence: a fenced account
    // may not create content, so the ordering here mirrors the saga.
    sqlx::query("insert into profile_rooms(owner_profile_id,layout) values($1,'{}'::jsonb)")
        .bind(user)
        .execute(&pool)
        .await?;
    // The common fence row the saga establishes first.
    sqlx::query(
        "insert into concert_world_account_deletion_fences(user_id,saga_id,identity_binding) \
         values($1,$2,decode(repeat('11',32),'hex'))",
    )
    .bind(user)
    .bind(saga)
    .execute(&pool)
    .await?;
    let insert = "insert into profile_social_account_deletions(user_id,saga_id,actor_id,\
         retention_seconds,retention_nanos,identity_binding,command_binding,source_binding,\
         fence_revision) values($1,$2,$3,2592000,0,decode(repeat('11',32),'hex'),\
         decode(repeat('22',32),'hex'),decode(repeat('33',32),'hex'),1)";
    // A receipt must start empty.
    let error = sqlx::query(
        "insert into profile_social_account_deletions(user_id,saga_id,actor_id,\
         retention_seconds,retention_nanos,identity_binding,command_binding,source_binding,\
         fence_revision,progress_revision) values($1,$2,$3,2592000,0,\
         decode(repeat('11',32),'hex'),decode(repeat('22',32),'hex'),\
         decode(repeat('33',32),'hex'),1,3)",
    )
    .bind(user)
    .bind(saga)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect_err("a receipt with progress must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_social_account_deletions_start")
    );
    let actor = Uuid::new_v4();
    sqlx::query(insert)
        .bind(user)
        .bind(saga)
        .bind(actor)
        .execute(&pool)
        .await?;
    // A skipped revision is refused.
    let error = sqlx::query(
        "update profile_social_account_deletions set progress_revision=2 where user_id=$1",
    )
    .bind(user)
    .execute(&pool)
    .await
    .expect_err("a skipped revision must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_social_account_deletions_monotonic")
    );
    // Completion while the account still owns a room is refused. The room is
    // keyed by the account here because this fixture has no profile service
    // database to resolve the account's profile id. It is written before the
    // fence, because a fenced account may not create content.
    let error = sqlx::query(
        "update profile_social_account_deletions set progress_revision=1, \
         completed_at=clock_timestamp() where user_id=$1",
    )
    .bind(user)
    .execute(&pool)
    .await
    .expect_err("completion with a live room must be refused");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_social_account_deletions_terminal")
    );
    Ok(())
}

#[tokio::test]
async fn a_fenced_account_cannot_recreate_profile_content() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let user = Uuid::new_v4();
    sqlx::query(
        "insert into concert_world_account_deletion_fences(user_id,saga_id,identity_binding) \
         values($1,$2,decode(repeat('11',32),'hex'))",
    )
    .bind(user)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await?;
    let error = sqlx::query(
        "insert into profile_posts(post_id,author_profile_id,description) values($1,$2,'hello')",
    )
    .bind(Uuid::new_v4())
    .bind(user)
    .execute(&pool)
    .await
    .expect_err("a fenced account must not create a post");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_social_account_fenced")
    );
    let error = sqlx::query(
        "insert into intro_posts(intro_id,author_profile_id,body,state,published_at) \
         values($1,$2,'hello','PUBLISHED',clock_timestamp())",
    )
    .bind(Uuid::new_v4())
    .bind(user)
    .execute(&pool)
    .await
    .expect_err("a fenced account must not create an intro post");
    assert_eq!(
        constraint(&error).as_deref(),
        Some("profile_social_account_fenced")
    );
    Ok(())
}

#[tokio::test]
async fn deleting_an_intro_post_removes_its_children_but_not_its_media() -> anyhow::Result<()> {
    let pool = pool().await?;
    install_fixture_table(&pool).await?;
    let author = profile(&pool).await?;
    let viewer = profile(&pool).await?;
    let intro = intro(&pool, author).await?;
    let media = ready_media(&pool, author, "INTRO_POST").await?;
    sqlx::query("insert into intro_attachments(intro_id,media_id,position) values($1,$2,0)")
        .bind(intro)
        .bind(media)
        .execute(&pool)
        .await?;
    sqlx::query("insert into intro_likes(intro_id,actor_profile_id) values($1,$2)")
        .bind(intro)
        .bind(viewer)
        .execute(&pool)
        .await?;
    sqlx::query(
        "insert into intro_comments(comment_id,intro_id,author_profile_id,body) \
         values($1,$2,$3,'hello')",
    )
    .bind(Uuid::new_v4())
    .bind(intro)
    .bind(viewer)
    .execute(&pool)
    .await?;
    sqlx::query("delete from intro_posts where intro_id=$1")
        .bind(intro)
        .execute(&pool)
        .await?;
    let children: i64 = sqlx::query_scalar(
        "select (select count(*) from intro_attachments where intro_id=$1) \
              + (select count(*) from intro_likes where intro_id=$1) \
              + (select count(*) from intro_comments where intro_id=$1)",
    )
    .bind(intro)
    .fetch_one(&pool)
    .await?;
    assert_eq!(children, 0, "children must cascade with the intro post");
    // The media file itself survives: the attachment was only an association.
    let state: String = sqlx::query_scalar("select state from profile_media where media_id=$1")
        .bind(media)
        .fetch_one(&pool)
        .await?;
    assert_eq!(state, "READY", "media must survive the association");
    Ok(())
}

#[tokio::test]
async fn new_concert_migrations_follow_the_previous_head() -> anyhow::Result<()> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut numbers: Vec<u32> = std::fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".sql") || name.ends_with(".down.sql") {
                return None;
            }
            name.split('_').next().and_then(|number| number.parse().ok())
        })
        .collect();
    numbers.sort_unstable();
    let head = *numbers.last().expect("at least one migration");
    assert!(head >= 85, "the new migrations must be numbered 0080 and later");
    // WP-02 delivered 0080..0085. Later work packages add their own migrations
    // above that head (WP-03 discovery, WP-08 avatars, and so on), so this test
    // asserts the WP-02 block is present and in order rather than that it is the
    // entire set above 0079. `0086` is currently carried by two files from two
    // packages; the duplicate is an integrator renumbering task and is recorded
    // in the WP-06 evidence, not something this schema test can resolve.
    let mut new: Vec<u32> = numbers.iter().copied().filter(|number| *number >= 80).collect();
    new.dedup();
    for expected in [80, 81, 82, 83, 84, 85] {
        assert!(
            new.contains(&expected),
            "migration {expected:03} must follow the previous head of 0079: {new:?}"
        );
    }
    // The WP-02 block is contiguous and ordered.
    let block: Vec<u32> = new.iter().copied().filter(|number| *number <= 85).collect();
    assert_eq!(
        block,
        vec![80, 81, 82, 83, 84, 85],
        "the six new migrations must follow the previous head of 0079"
    );
    Ok(())
}
