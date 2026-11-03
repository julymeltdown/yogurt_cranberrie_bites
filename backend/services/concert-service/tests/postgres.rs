//! Run only through concert_core_smoke.py's marker/inode-owned native fixture.
use concert_application::{CoreRepository, PreferenceUpdate};
use concert_domain::*;
use concert_service::adapters::out::repository::PgConcertRepository;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{path::PathBuf, str::FromStr};
use uuid::Uuid;

async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(
        std::env::var("CONCERT_TEST_OWNER_ROOT")
            .map_err(|_| anyhow::anyhow!("owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("CONCERT_TEST_OWNER_IDENTITY")?,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("CONCERT_TEST_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_concert")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture connection failed"))?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}
async fn fixture(pool: &PgPool) -> anyhow::Result<ParticipantKey> {
    let event = Uuid::new_v4();
    let user = Uuid::new_v4();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Owned test event','OPEN')").bind(event).bind(format!("test-{}",event.simple())).execute(pool).await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event).bind(user).execute(pool).await?;
    Ok(ParticipantKey {
        event_id: EventId::from_uuid(event),
        user_id: AuthUserId::from_uuid(user),
    })
}
fn preferences(theme: Theme) -> Preferences {
    Preferences {
        theme,
        low_power: true,
        reduced_motion: true,
        sound_enabled: false,
    }
}
async fn rejects(pool: &PgPool, sql: &str, code: &str) {
    let error = sqlx::query(sql)
        .execute(pool)
        .await
        .expect_err("database must reject invalid state");
    assert_eq!(
        error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some(code)
    );
}
#[tokio::test]
async fn postgres_constraints_reject_invalid_identity_relationships_values_and_revisions(
) -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let key = fixture(&pool).await?;
    let event = key.event_id.uuid();
    let user = key.user_id.uuid();
    rejects(&pool,&format!("insert into concert_events(event_id,slug,title,phase) select '{}',slug,title,phase from concert_events where event_id='{event}'",Uuid::new_v4()),"23505").await;
    rejects(&pool,&format!("insert into concert_participants select * from concert_participants where event_id='{event}' and user_id='{user}'"),"23505").await;
    rejects(&pool,&format!("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values('{}','{user}','REVENGE',false,false,false)",Uuid::new_v4()),"23503").await;
    for assignment in [
        "revision=0",
        "revision=3",
        "phase='UNKNOWN'",
        "timezone='UTC'",
        "slug='Bad/slug'",
        "title=''",
        "title=repeat('é',121)",
    ] {
        let revision = if assignment.starts_with("revision=") {
            String::new()
        } else {
            ",revision=revision+1".into()
        };
        rejects(
            &pool,
            &format!("update concert_events set {assignment}{revision} where event_id='{event}'"),
            "23514",
        )
        .await;
    }
    rejects(&pool,&format!("update concert_participants set theme='OTHER',revision=revision+1 where event_id='{event}' and user_id='{user}'"),"23514").await;
    rejects(&pool,&format!("update concert_participants set user_id='{}',revision=revision+1 where event_id='{event}' and user_id='{user}'",Uuid::new_v4()),"23514").await;
    rejects(&pool,&format!("update concert_participants set revision=9007199254740992 where event_id='{event}' and user_id='{user}'"),"23514").await;
    let insert=format!("insert into concert_avatar_settings(event_id,user_id,contract_version,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,skin_tone_id) values('{event}','{user}','1.0.0','owned_test',1,'test_body','test_eyes','test_hair','test_outfit','test_tone')");
    sqlx::query(&insert).execute(&pool).await?;
    rejects(&pool, &insert, "23505").await;
    rejects(
        &pool,
        &insert.replace(&user.to_string(), &Uuid::new_v4().to_string()),
        "23503",
    )
    .await;
    for assignment in [
        "manifest_version=0",
        "manifest_version=9007199254740992",
        "body_id='https://hostile.test/body.png'",
        "accessory_id='../legacy'",
        "contract_version='2.0.0'",
    ] {
        rejects(&pool,&format!("update concert_avatar_settings set {assignment},revision=revision+1 where event_id='{event}' and user_id='{user}'"),"23514").await;
    }
    let repository = PgConcertRepository::new(pool.clone());
    let avatar = repository.avatar(key).await?.unwrap();
    assert_eq!(avatar.key, key);
    assert!(avatar.selection.accessory.is_none());
    let foreign = ParticipantKey {
        event_id: EventId::from_uuid(Uuid::new_v4()),
        user_id: key.user_id,
    };
    assert!(repository.avatar(foreign).await?.is_none());
    sqlx::query("delete from concert_events where event_id=$1")
        .bind(event)
        .execute(&pool)
        .await?;
    assert!(repository.participant(key).await?.is_none());
    assert!(repository.avatar(key).await?.is_none());
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn postgres_scopes_reads_and_atomically_rejects_stale_competing_writes() -> anyhow::Result<()>
{
    let pool = owned_pool().await?;
    let actor = fixture(&pool).await?;
    let other = fixture(&pool).await?;
    // The same canonical Auth user can independently participate in two events.
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'DANGER',false,false,false)").bind(other.event_id.uuid()).bind(actor.user_id.uuid()).execute(&pool).await?;
    let scoped = ParticipantKey {
        event_id: other.event_id,
        user_id: actor.user_id,
    };
    let application = concert_service::config::build_application(pool.clone());
    assert_eq!(
        application.own_participant(actor).await.unwrap_err(),
        concert_application::CoreError::DependencyUnavailable
    );
    assert_eq!(
        application
            .change_preferences(actor, Revision::initial(), preferences(Theme::Parade))
            .await
            .unwrap_err(),
        concert_application::CoreError::DependencyUnavailable
    );
    let repo = PgConcertRepository::new(pool.clone());
    assert_eq!(
        repo.event(actor.event_id).await?.unwrap().phase,
        EventPhase::Open
    );
    assert!(repo
        .participant(ParticipantKey {
            event_id: actor.event_id,
            user_id: other.user_id
        })
        .await?
        .is_none());
    let (first, second) = tokio::join!(
        repo.update_preferences(actor, Revision::initial(), preferences(Theme::Parade)),
        repo.update_preferences(actor, Revision::initial(), preferences(Theme::Danger))
    );
    let outcomes = [first?, second?];
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| matches!(v, PreferenceUpdate::Updated(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| matches!(v, PreferenceUpdate::Conflict))
            .count(),
        1
    );
    assert_eq!(repo.participant(actor).await?.unwrap().revision.get(), 2);
    assert_eq!(repo.participant(scoped).await?.unwrap().revision.get(), 1);
    assert_eq!(
        repo.participant(scoped).await?.unwrap().preferences.theme,
        Theme::Danger
    );
    pool.close().await;
    Ok(())
}
#[tokio::test]
async fn postgres_event_closure_serializes_with_preference_mutation() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let actor = fixture(&pool).await?;
    let mut closure = pool.begin().await?;
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(actor.event_id.uuid())
    .execute(&mut *closure)
    .await?;
    let repo = PgConcertRepository::new(pool.clone());
    let writer = repo.clone();
    let mut task = tokio::spawn(async move {
        writer
            .update_preferences(actor, Revision::initial(), preferences(Theme::Parade))
            .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut task)
            .await
            .is_err(),
        "mutation must wait for event closure transaction"
    );
    closure.commit().await?;
    assert_eq!(task.await??, PreferenceUpdate::EventReadOnly);
    assert_eq!(repo.participant(actor).await?.unwrap().revision.get(), 1);
    pool.close().await;
    Ok(())
}
