//! Native production owner adapter tests; requires the marker/inode-owned smoke harness.
use auth_application::concert_identity::*;
use auth_application::{
    dto::repository_dto::AuthIdentityProviderDto,
    ports::{OAuthIdentityRepositoryPort, RefreshTokenStorePort},
};
use auth_service::adapters::out::{
    cache::session_store::PgRefreshTokenStore,
    persistence::{concert_identity::PgCurrentAdmission, user_repository::PgUserRepository},
    runtime::SystemAuthRuntime,
};
use auth_service::enrollment_policy::{
    ApprovedPublication, ConsentRequest, EnrollmentStore, PolicyDocument,
};
use chrono::{DateTime, Utc};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;
async fn owned_pool(database: &str) -> anyhow::Result<PgPool> {
    anyhow::ensure!(
        matches!(
            database,
            "mcr_auth_identity" | "mcr_auth_identity_exhaustion"
        ),
        "foreign database refused"
    );
    let root = PathBuf::from(
        std::env::var("AUTH_IDENTITY_TEST_OWNER_ROOT")
            .map_err(|_| anyhow::anyhow!("owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && info.is_dir()
            && !info.file_type().is_symlink()
            && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino())
            == std::env::var("AUTH_IDENTITY_TEST_OWNER_IDENTITY")?,
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
        marker["token"].as_str() == Some(std::env::var("AUTH_IDENTITY_TEST_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var(if database == "mcr_auth_identity" {
        "AUTH_IDENTITY_TEST_DATABASE_URL"
    } else {
        "AUTH_IDENTITY_TEST_EXHAUSTION_URL"
    })?)
    .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some(database)
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

fn budget() -> ReadBudget {
    ReadBudget::new(
        Instant::now() + Duration::from_secs(1),
        ReadCancellation::default(),
    )
    .unwrap()
}

const FIXTURE_POLICY_REVISION: &str = "3a1f4c62-9b7e-4f0d-8a52-6c1d2e3f4a5b";
const FIXTURE_ACKNOWLEDGEMENT: &str = "rpc-fixture-terms-v1";

async fn seed_fixture_policy(pool: &PgPool, user: Uuid) -> anyhow::Result<()> {
    let publication = ApprovedPublication {
        revision: Uuid::parse_str(FIXTURE_POLICY_REVISION)?,
        documents: vec![PolicyDocument {
            id: "rpc-fixture-terms".into(),
            title: "Synthetic admission RPC fixture terms".into(),
            url: "https://fixture.example.test/rpc-terms".into(),
            sha256: "ab".repeat(32),
            required_acknowledgement_id: FIXTURE_ACKNOWLEDGEMENT.into(),
        }],
        approval_reference: "SYNTHETIC_TEST_FIXTURE_NOT_PRODUCTION_APPROVAL".into(),
        published_at: chrono::DateTime::from_timestamp(1_577_836_800, 0)
            .ok_or_else(|| anyhow::anyhow!("fixture policy publish time"))?,
        expires_at: chrono::DateTime::from_timestamp(4_102_444_800, 0)
            .ok_or_else(|| anyhow::anyhow!("fixture policy expiry"))?,
    };
    let store = EnrollmentStore::new(pool.clone());
    store.publish(&publication).await?;
    store
        .consent(
            user,
            ConsentRequest {
                policy_revision: publication.revision,
                acknowledgement_ids: vec![FIXTURE_ACKNOWLEDGEMENT.into()],
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await?;
    Ok(())
}

async fn fixture() -> anyhow::Result<(PgPool, AdmissionRead)> {
    let pool = owned_pool("mcr_auth_identity").await?;
    let user = Uuid::new_v4();
    let family = Uuid::new_v4();
    sqlx::query("insert into users(id, primary_email, password_hash, status) values($1,$2,'private-password-hash','ACTIVE')")
        .bind(user).bind(format!("{user}@private.invalid")).execute(&pool).await?;
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'private-token-hash',$2,$3,statement_timestamp()+interval '1 hour')")
        .bind(Uuid::new_v4().to_string()).bind(user).bind(family).execute(&pool).await?;
    seed_fixture_policy(&pool, user).await?;
    Ok((pool, AdmissionRead::new(user, family, None)?))
}
async fn read(pool: &PgPool, request: AdmissionRead) -> Result<CurrentAdmission, ReadError> {
    PgCurrentAdmission::new(pool.clone())
        .read_current(request, &budget())
        .await
}
async fn identity(pool: &PgPool, user: Uuid) -> anyhow::Result<ProviderBinding> {
    let id = Uuid::new_v4();
    let epoch = sqlx::query_scalar("insert into user_identities(id,user_id,provider_type,provider_subject,email,email_verified) values($1,$2,'X',$3,'private-provider@example.invalid',true) returning binding_epoch")
        .bind(id).bind(user).bind(Uuid::new_v4().to_string()).fetch_one(pool).await?;
    Ok(ProviderBinding::new(id, BindingProvider::X, epoch)?)
}
fn with_binding(request: AdmissionRead, binding: ProviderBinding) -> AdmissionRead {
    AdmissionRead::new(request.user_id(), request.family_id(), Some(binding)).unwrap()
}

#[tokio::test]
async fn email_only_requires_no_provider_relation_access() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let mut lock = pool.begin().await?;
    sqlx::query("lock table user_identities in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let before: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    let result = read(&pool, request).await?;
    let after: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    assert!(result.eligible());
    assert_eq!(result.binding, BindingEligibility::NotRequested);
    assert!(result.observed_at >= before && result.observed_at <= after);
    lock.rollback().await?;
    Ok(())
}
#[tokio::test]
async fn missing_pending_deleted_and_unknown_accounts_fail_closed() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let missing = AdmissionRead::new(Uuid::new_v4(), request.family_id(), None)?;
    // Consent is global: an identity without an acceptance is refused before
    // account facts, including an account that does not exist.
    assert_eq!(
        read(&pool, missing).await.unwrap_err(),
        ReadError::ConsentRequired
    );
    for status in [
        "PENDING_VERIFICATION",
        "DELETED",
        "UNKNOWN",
        "SUSPENDED",
        "active",
    ] {
        sqlx::query("update users set status=$2 where id=$1")
            .bind(request.user_id())
            .bind(status)
            .execute(&pool)
            .await?;
        let result = read(&pool, request).await?;
        assert_eq!(result.account, AccountEligibility::Ineligible);
        assert!(!result.eligible());
    }
    sqlx::query("delete from users where id=$1")
        .bind(request.user_id())
        .execute(&pool)
        .await?;
    // Deleting the account cascades its acceptance; consent precedes facts.
    assert_eq!(
        read(&pool, request).await.unwrap_err(),
        ReadError::ConsentRequired
    );
    Ok(())
}
#[tokio::test]
async fn active_status_with_deleted_timestamp_is_ineligible() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    sqlx::query("update users set deleted_at=now() where id=$1")
        .bind(request.user_id())
        .execute(&pool)
        .await?;
    assert_eq!(
        read(&pool, request).await?.account,
        AccountEligibility::Ineligible
    );
    Ok(())
}
#[tokio::test]
async fn live_family_must_belong_to_exact_user() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let (_, other) = fixture().await?;
    for bad in [
        AdmissionRead::new(request.user_id(), other.family_id(), None)?,
        AdmissionRead::new(other.user_id(), request.family_id(), None)?,
        AdmissionRead::new(request.user_id(), Uuid::new_v4(), None)?,
    ] {
        assert_eq!(read(&pool, bad).await?.session, SessionEligibility::Ended);
    }
    assert!(read(&pool, request).await?.eligible());
    Ok(())
}
#[tokio::test]
async fn expired_and_revoked_family_fail_without_revoking_other_families() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    sqlx::query("update refresh_tokens set expires_at=statement_timestamp() where family_id=$1")
        .bind(request.family_id())
        .execute(&pool)
        .await?;
    assert_eq!(
        read(&pool, request).await?.session,
        SessionEligibility::Ended
    );
    sqlx::query("update refresh_tokens set expires_at=now()+interval '1 hour',revoked_at=now() where family_id=$1").bind(request.family_id()).execute(&pool).await?;
    assert!(!read(&pool, request).await?.eligible());
    let other = Uuid::new_v4();
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'private',$2,$3,now()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(request.user_id()).bind(other).execute(&pool).await?;
    assert!(
        read(&pool, AdmissionRead::new(request.user_id(), other, None)?)
            .await?
            .eligible()
    );
    let store = PgRefreshTokenStore::new(pool.clone(), Arc::new(SystemAuthRuntime));
    store.revoke_family(request.family_id()).await?;
    assert!(
        store
            .session_is_live(request.user_id(), other, Utc::now())
            .await?
    );
    assert!(
        !store
            .session_is_live(request.user_id(), request.family_id(), Utc::now())
            .await?
    );
    assert!(
        read(&pool, AdmissionRead::new(request.user_id(), other, None)?)
            .await?
            .eligible()
    );
    Ok(())
}
#[tokio::test]
async fn rotated_predecessor_with_live_successor_is_live() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let mut tx = pool.begin().await?;
    sqlx::query("update refresh_tokens set revoked_at=now(),rotated_at=now() where family_id=$1")
        .bind(request.family_id())
        .execute(&mut *tx)
        .await?;
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'successor-private',$2,$3,now()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(request.user_id()).bind(request.family_id()).execute(&mut *tx).await?;
    tx.commit().await?;
    assert!(read(&pool, request).await?.eligible());
    Ok(())
}
#[tokio::test]
async fn provider_requires_exact_user_row_provider_and_owner_epoch() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let binding = identity(&pool, request.user_id()).await?;
    assert!(read(&pool, with_binding(request, binding))
        .await?
        .eligible());
    let (_, other) = fixture().await?;
    let foreign = identity(&pool, other.user_id()).await?;
    for bad in [
        foreign,
        ProviderBinding::new(
            binding.identity_id(),
            BindingProvider::Google,
            binding.epoch(),
        )?,
        ProviderBinding::new(
            binding.identity_id(),
            BindingProvider::X,
            binding.epoch() + 1,
        )?,
        ProviderBinding::new(Uuid::new_v4(), BindingProvider::X, binding.epoch())?,
    ] {
        let result = read(&pool, with_binding(request, bad)).await?;
        assert_eq!(result.binding, BindingEligibility::NotBound);
        assert!(!result.eligible());
    }
    Ok(())
}
#[tokio::test]
async fn legacy_null_blank_control_and_oversized_oauth_subjects_fail_closed() -> anyhow::Result<()>
{
    let (pool, request) = fixture().await?;
    let binding = identity(&pool, request.user_id()).await?;
    for provider in [
        BindingProvider::Google,
        BindingProvider::Apple,
        BindingProvider::X,
    ] {
        for subject in [
            None,
            Some(String::new()),
            Some(" ".into()),
            Some("\u{00a0}\u{2003}\u{3000}".into()),
            Some("bad\nsubject".into()),
            Some("bad\u{007f}subject".into()),
            Some("bad\u{009f}subject".into()),
            Some("x".repeat(513)),
            Some("ü".repeat(257)),
        ] {
            let epoch=sqlx::query_scalar("update user_identities set provider_type=$2,provider_subject=$3 where id=$1 returning binding_epoch").bind(binding.identity_id()).bind(provider.owner_name()).bind(subject).fetch_one(&pool).await?;
            let result = read(
                &pool,
                with_binding(
                    request,
                    ProviderBinding::new(binding.identity_id(), provider, epoch)?,
                ),
            )
            .await?;
            assert_eq!(result.binding, BindingEligibility::NotBound);
            assert!(!result.eligible());
        }
        // email_verified is not OAuth subject verification or consent.
        let epoch=sqlx::query_scalar("update user_identities set provider_subject=$2,email_verified=false where id=$1 returning binding_epoch").bind(binding.identity_id()).bind(Uuid::new_v4().to_string()).fetch_one(&pool).await?;
        assert!(read(
            &pool,
            with_binding(
                request,
                ProviderBinding::new(binding.identity_id(), provider, epoch)?
            )
        )
        .await?
        .eligible());
    }
    Ok(())
}
#[tokio::test]
async fn email_binding_requires_owner_verification_without_oauth_subject() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let id = Uuid::new_v4();
    let epoch=sqlx::query_scalar("insert into user_identities(id,user_id,provider_type,email,email_verified) values($1,$2,'EMAIL','verified@private.invalid',false) returning binding_epoch").bind(id).bind(request.user_id()).fetch_one(&pool).await?;
    let unverified = ProviderBinding::new(id, BindingProvider::Email, epoch)?;
    assert!(!read(&pool, with_binding(request, unverified))
        .await?
        .eligible());
    // Ordinary admission still needs no optional email-identity proof.
    assert!(read(&pool, request).await?.eligible());
    let epoch = sqlx::query_scalar(
        "update user_identities set email_verified=true where id=$1 returning binding_epoch",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert!(read(
        &pool,
        with_binding(
            request,
            ProviderBinding::new(id, BindingProvider::Email, epoch)?
        )
    )
    .await?
    .eligible());
    assert!(!read(&pool, with_binding(request, unverified))
        .await?
        .eligible());
    let epoch = sqlx::query_scalar(
        "update user_identities set email='' where id=$1 returning binding_epoch",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert!(!read(
        &pool,
        with_binding(
            request,
            ProviderBinding::new(id, BindingProvider::Email, epoch)?
        )
    )
    .await?
    .eligible());
    Ok(())
}
#[tokio::test]
async fn soft_deleted_and_restored_binding_cannot_reuse_old_epoch() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let binding = identity(&pool, request.user_id()).await?;
    sqlx::query("update user_identities set deleted_at=now() where id=$1")
        .bind(binding.identity_id())
        .execute(&pool)
        .await?;
    assert!(!read(&pool, with_binding(request, binding))
        .await?
        .eligible());
    let epoch = sqlx::query_scalar(
        "update user_identities set deleted_at=null where id=$1 returning binding_epoch",
    )
    .bind(binding.identity_id())
    .fetch_one(&pool)
    .await?;
    assert_ne!(binding.epoch(), epoch);
    assert!(!read(&pool, with_binding(request, binding))
        .await?
        .eligible());
    assert!(read(
        &pool,
        with_binding(
            request,
            ProviderBinding::new(binding.identity_id(), BindingProvider::X, epoch)?
        )
    )
    .await?
    .eligible());
    Ok(())
}
#[tokio::test]
async fn same_transaction_equal_linked_at_and_caller_epoch_overwrite_are_invalidated(
) -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let binding = identity(&pool, request.user_id()).await?;
    let mut tx = pool.begin().await?;
    let first = sqlx::query("update user_identities set linked_at=now(),binding_epoch=$2 where id=$1 returning linked_at,binding_epoch").bind(binding.identity_id()).bind(binding.epoch()).fetch_one(&mut *tx).await?;
    let second = sqlx::query("update user_identities set linked_at=now(),binding_epoch=$2 where id=$1 returning linked_at,binding_epoch").bind(binding.identity_id()).bind(binding.epoch()).fetch_one(&mut *tx).await?;
    assert_eq!(
        first.get::<DateTime<Utc>, _>("linked_at"),
        second.get::<DateTime<Utc>, _>("linked_at")
    );
    let first_epoch: i64 = first.get("binding_epoch");
    let second_epoch: i64 = second.get("binding_epoch");
    assert!(binding.epoch() < first_epoch && first_epoch < second_epoch);
    tx.commit().await?;
    for epoch in [binding.epoch(), first_epoch] {
        assert!(!read(
            &pool,
            with_binding(
                request,
                ProviderBinding::new(binding.identity_id(), BindingProvider::X, epoch)?
            )
        )
        .await?
        .eligible());
    }
    assert!(read(
        &pool,
        with_binding(
            request,
            ProviderBinding::new(binding.identity_id(), BindingProvider::X, second_epoch)?
        )
    )
    .await?
    .eligible());
    Ok(())
}
#[tokio::test]
async fn deleted_and_reinserted_same_identity_id_gets_fresh_epoch() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let binding = identity(&pool, request.user_id()).await?;
    sqlx::query("delete from user_identities where id=$1")
        .bind(binding.identity_id())
        .execute(&pool)
        .await?;
    assert!(!read(&pool, with_binding(request, binding))
        .await?
        .eligible());
    let epoch:i64=sqlx::query_scalar("insert into user_identities(id,user_id,provider_type,provider_subject,email,binding_epoch) values($1,$2,'X','private-replacement','replacement@private.invalid',$3) returning binding_epoch").bind(binding.identity_id()).bind(request.user_id()).bind(binding.epoch()).fetch_one(&pool).await?;
    assert!(epoch > binding.epoch());
    assert!(!read(&pool, with_binding(request, binding))
        .await?
        .eligible());
    assert!(read(
        &pool,
        with_binding(
            request,
            ProviderBinding::new(binding.identity_id(), BindingProvider::X, epoch)?
        )
    )
    .await?
    .eligible());
    Ok(())
}
#[tokio::test]
async fn production_relink_repository_advances_generation() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let binding = identity(&pool, request.user_id()).await?;
    let subject: String =
        sqlx::query_scalar("select provider_subject from user_identities where id=$1")
            .bind(binding.identity_id())
            .fetch_one(&pool)
            .await?;
    let repo = PgUserRepository::new(pool.clone(), Arc::new(SystemAuthRuntime));
    repo.link_oauth_identity(
        request.user_id(),
        AuthIdentityProviderDto::X,
        &subject,
        "private-provider@example.invalid",
        true,
    )
    .await?;
    assert!(!read(&pool, with_binding(request, binding))
        .await?
        .eligible());
    let epoch = sqlx::query_scalar("select binding_epoch from user_identities where id=$1")
        .bind(binding.identity_id())
        .fetch_one(&pool)
        .await?;
    assert!(read(
        &pool,
        with_binding(
            request,
            ProviderBinding::new(binding.identity_id(), BindingProvider::X, epoch)?
        )
    )
    .await?
    .eligible());
    Ok(())
}
#[tokio::test]
async fn deactivation_and_deletion_snapshots_are_coherent_then_fresh() -> anyhow::Result<()> {
    for change in [
        "update users set status='PENDING_VERIFICATION' where id=$1",
        "update users set deleted_at=now() where id=$1",
        "delete from users where id=$1",
    ] {
        let (pool, request) = fixture().await?;
        let mut writer = pool.begin().await?;
        sqlx::query(change)
            .bind(request.user_id())
            .execute(&mut *writer)
            .await?;
        // The uncommitted writer includes session-revocation trigger effects.
        // A reader sees the previously committed account+family together.
        let before = read(&pool, request).await?;
        assert!(before.eligible());
        writer.commit().await?;
        if change.starts_with("delete") {
            // Account deletion cascades the acceptance; consent is enforced first.
            assert_eq!(
                read(&pool, request).await.unwrap_err(),
                ReadError::ConsentRequired
            );
            continue;
        }
        let after = read(&pool, request).await?;
        assert_eq!(after.account, AccountEligibility::Ineligible);
        assert_eq!(after.session, SessionEligibility::Ended);
        assert!(!after.eligible());
    }
    Ok(())
}
#[tokio::test]
async fn pre_cancelled_and_expired_requests_never_need_connection() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    pool.close().await;
    let repo = PgCurrentAdmission::new(pool);
    let cancel = ReadCancellation::default();
    cancel.cancel();
    assert_eq!(
        repo.read_current(
            request,
            &ReadBudget::new(Instant::now() + Duration::from_secs(1), cancel)?
        )
        .await,
        Err(ReadError::Cancelled)
    );
    assert_eq!(
        repo.read_current(
            request,
            &ReadBudget::new(
                Instant::now() - Duration::from_secs(1),
                ReadCancellation::default()
            )?
        )
        .await,
        Err(ReadError::DeadlineExceeded)
    );
    Ok(())
}
#[tokio::test]
async fn pool_wait_observes_cancellation_and_original_deadline() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let limited = PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let held = limited.acquire().await?;
    let repo = PgCurrentAdmission::new(limited.clone());
    let cancel = ReadCancellation::default();
    let pending = cancel.clone();
    let b = ReadBudget::new(Instant::now() + Duration::from_secs(1), cancel)?;
    let task = tokio::spawn(async move { repo.read_current(request, &b).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    let start = Instant::now();
    pending.cancel();
    assert_eq!(task.await?, Err(ReadError::Cancelled));
    assert!(start.elapsed() < Duration::from_millis(250));
    let b = ReadBudget::new(
        Instant::now() + Duration::from_millis(50),
        ReadCancellation::default(),
    )?;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let start = Instant::now();
    assert_eq!(
        PgCurrentAdmission::new(limited)
            .read_current(request, &b)
            .await,
        Err(ReadError::DeadlineExceeded)
    );
    assert!(start.elapsed() < Duration::from_millis(150));
    drop(held);
    Ok(())
}
#[tokio::test]
async fn blocked_statement_is_bounded_and_next_read_recovers() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let limited = PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let mut lock = pool.begin().await?;
    sqlx::query("lock table users in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let b = ReadBudget::new(
        Instant::now() + Duration::from_millis(75),
        ReadCancellation::default(),
    )?;
    assert_eq!(
        PgCurrentAdmission::new(limited.clone())
            .read_current(request, &b)
            .await,
        Err(ReadError::DeadlineExceeded)
    );
    let cancel = ReadCancellation::default();
    let pending = cancel.clone();
    let b = ReadBudget::new(Instant::now() + Duration::from_secs(1), cancel)?;
    let repo = PgCurrentAdmission::new(limited.clone());
    let task = tokio::spawn(async move { repo.read_current(request, &b).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    let start = Instant::now();
    pending.cancel();
    assert_eq!(task.await?, Err(ReadError::Cancelled));
    assert!(start.elapsed() < Duration::from_millis(250));
    lock.rollback().await?;
    assert!(read(&limited, request).await?.eligible());
    let timeout: String = sqlx::query_scalar("show statement_timeout")
        .fetch_one(&limited)
        .await?;
    assert_eq!(timeout, "0");
    Ok(())
}
#[tokio::test]
async fn expiry_uses_database_time_after_pool_wait() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let limited = PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let held = limited.acquire().await?;
    sqlx::query("update refresh_tokens set expires_at=statement_timestamp()+interval '100 milliseconds' where family_id=$1").bind(request.family_id()).execute(&pool).await?;
    let repo = PgCurrentAdmission::new(limited);
    let task = tokio::spawn(async move { repo.read_current(request, &budget()).await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(held);
    assert_eq!(task.await??.session, SessionEligibility::Ended);
    Ok(())
}
#[tokio::test]
async fn expiry_uses_database_time_after_statement_lock_wait() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    sqlx::query("update refresh_tokens set expires_at=statement_timestamp()+interval '100 milliseconds' where family_id=$1").bind(request.family_id()).execute(&pool).await?;
    let mut lock = pool.begin().await?;
    sqlx::query("lock table users in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let repo = PgCurrentAdmission::new(pool.clone());
    let task = tokio::spawn(async move { repo.read_current(request, &budget()).await });
    tokio::time::timeout(Duration::from_millis(500),async {
        loop {
            let blocked:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like '%owner_clock%' and pid<>pg_backend_pid())").fetch_one(&pool).await.unwrap();
            if blocked {break;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    lock.rollback().await?;
    assert_eq!(task.await??.session, SessionEligibility::Ended);
    Ok(())
}
#[tokio::test]
async fn deadline_is_not_reset_after_connection_acquisition() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let limited = PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let held = limited.acquire().await?;
    let mut lock = pool.begin().await?;
    sqlx::query("lock table users in access exclusive mode")
        .execute(&mut *lock)
        .await?;
    let repo = PgCurrentAdmission::new(limited);
    let b = ReadBudget::new(
        Instant::now() + Duration::from_millis(100),
        ReadCancellation::default(),
    )?;
    let start = Instant::now();
    let task = tokio::spawn(async move { repo.read_current(request, &b).await });
    tokio::time::sleep(Duration::from_millis(65)).await;
    drop(held);
    assert_eq!(task.await?, Err(ReadError::DeadlineExceeded));
    assert!(start.elapsed() < Duration::from_millis(150));
    lock.rollback().await?;
    Ok(())
}
#[tokio::test]
async fn closed_owner_is_unavailable_and_diagnostics_are_redacted() -> anyhow::Result<()> {
    let (pool, request) = fixture().await?;
    let result = read(&pool, request).await?;
    let repo = PgCurrentAdmission::new(pool.clone());
    let diagnostic = format!("{repo:?} {request:?} {result:?}");
    assert!(!diagnostic.contains(&request.user_id().to_string()));
    assert!(!diagnostic.contains(&request.family_id().to_string()));
    for secret in [
        "private-password",
        "private-token",
        "@private",
        "postgresql://",
    ] {
        assert!(!diagnostic.contains(secret));
    }
    pool.close().await;
    assert_eq!(
        repo.read_current(request, &budget()).await,
        Err(ReadError::Unavailable)
    );
    Ok(())
}
#[tokio::test]
async fn epoch_exhaustion_rolls_back_writes_without_wrap() -> anyhow::Result<()> {
    let pool = owned_pool("mcr_auth_identity_exhaustion").await?;
    let user = Uuid::new_v4();
    let id = Uuid::new_v4();
    sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,'exhaustion@private.invalid','private','ACTIVE')").bind(user).execute(&pool).await?;
    let old:i64=sqlx::query_scalar("insert into user_identities(id,user_id,provider_type,email) values($1,$2,'EMAIL','private@example.invalid') returning binding_epoch").bind(id).bind(user).fetch_one(&pool).await?;
    // Only the dedicated disposable exhaustion DB. Never reset an owner sequence.
    sqlx::query("select setval('public.auth_identity_binding_epoch',9007199254740991,true)")
        .execute(&pool)
        .await?;
    let mut tx = pool.begin().await?;
    assert!(sqlx::query(
        "update user_identities set email='must-rollback@private.invalid' where id=$1"
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .is_err());
    tx.rollback().await?;
    let row = sqlx::query("select email,binding_epoch from user_identities where id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(row.get::<i64, _>("binding_epoch"), old);
    assert_eq!(row.get::<String, _>("email"), "private@example.invalid");
    assert!(sqlx::query("insert into user_identities(id,user_id,provider_type,email) values($1,$2,'X','no-insert@private.invalid')").bind(Uuid::new_v4()).bind(user).execute(&pool).await.is_err());
    let last: i64 = sqlx::query_scalar("select last_value from public.auth_identity_binding_epoch")
        .fetch_one(&pool)
        .await?;
    assert_eq!(last, ProviderBinding::MAX_EPOCH);
    Ok(())
}
