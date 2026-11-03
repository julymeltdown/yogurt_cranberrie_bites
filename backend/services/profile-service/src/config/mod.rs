use crate::adapters::out::runtime::SystemProfileRuntime;
pub fn load() {}
use crate::adapters::out::media::image_avatar_processor::ImageAvatarProcessor;
use crate::adapters::out::persistence::avatar_repository::PgAvatarRepository;
use crate::adapters::out::persistence::concert_identity::PgConcertProfileRepository;
use crate::adapters::out::persistence::profile_repository::PgProfileRepository;
use crate::adapters::r#in::messaging::mutation_receipt_retention_worker::ProfileMutationReceiptRetentionWorker;
use crate::adapters::r#in::messaging::retention_purge_worker::ProfileRetentionPurgeWorker;
use crate::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use crate::application::use_cases::delete_profile::DeleteProfileUseCase;
use crate::application::use_cases::get_avatar::GetAvatarUseCase;
use crate::application::use_cases::get_profile::GetProfileUseCase;
use crate::application::use_cases::get_profile_by_username::GetProfileByUsernameUseCase;
use crate::application::use_cases::get_profiles_batch::GetProfilesBatchUseCase;
use crate::application::use_cases::purge_expired_mutation_outcomes::PurgeExpiredMutationOutcomesUseCase;
use crate::application::use_cases::purge_expired_profiles::PurgeExpiredProfilesUseCase;
use crate::application::use_cases::search_profiles::SearchProfilesUseCase;
use crate::application::use_cases::update_privacy::UpdateProfilePrivacyUseCase;
use crate::application::use_cases::update_profile::UpdateProfileUseCase;
use crate::application::use_cases::upload_avatar::UploadAvatarUseCase;
use crate::domain::profile::{AccountVisibility, Profile};
use platform::config::{
    env_or, parse_bool_setting, parse_i32_setting, parse_u32_setting, parse_u64_setting,
    require_production_disabled, require_production_postgres_url,
    require_production_safe_socket_addr, runtime_mode_is_production,
};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const LOCAL_RETENTION_PURGE_ACTOR_ID: &str = "99999999-9999-9999-9999-999999999999";
const AVATAR_DECODE_WORKERS: usize = 2;
const AVATAR_DECODE_QUEUE_LIMIT: usize = 4;

#[derive(Clone)]
pub struct AppConfig {
    pub http_addr: SocketAddr,
    pub metrics_addr: SocketAddr,
    pub grpc_addr: SocketAddr,
    pub database_url: String,
    pub database_migration_mode: migration_runtime::MigrationMode,
    /// Largest number of pooled database connections this service opens.
    ///
    /// A single hardcoded value forced every service to share one concurrency
    /// ceiling. That is wrong in both directions: a read-heavy service that
    /// fans out is throttled to the same limit as an edge that holds a
    /// connection per in-flight request, and raising it everywhere to suit one
    /// service multiplies the load on PostgreSQL instead. The bound is
    /// configuration so a deployment can size each service to its own work.
    pub database_pool_max_connections: u32,
    pub seed_enabled: bool,
    /// Callers this service accepts, each with its own credential.
    pub caller_registry: platform::grpc_auth::CallerRegistry,
    /// Verifies Gateway actor delegations on actor-bound methods.
    pub actor_delegation: platform::actor_delegation::ActorDelegationVerifier,
    /// Mutual TLS material for internal gRPC, resolved at load time.
    ///
    /// `None` means the documented plaintext development path. Production cannot
    /// reach a running service with `None` because `load` fails first.
    pub tls_material: Option<platform::grpc_tls::TlsMaterial>,
    pub retention_purge_worker_enabled: bool,
    pub retention_purge_worker_interval_ms: u64,
    pub retention_purge_actor_id: String,
    pub mutation_receipt_retention_worker_enabled: bool,
    pub mutation_receipt_retention_worker_interval_ms: u64,
    pub mutation_receipt_retention_batch_size: i32,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let production_mode = runtime_mode_is_production(
            &env_or("PROFILE_RUNTIME_MODE", "production"),
            "PROFILE_RUNTIME_MODE",
        )?;
        let tls_material = platform::config::resolve_tls_material(production_mode, "profile")?;
        // The edge owns user-facing profile operations, post-service reads
        // profiles for feeds and search, and the auth coordinator alone may
        // apply account deletion.
        let caller_registry = platform::config::build_caller_registry(
            production_mode,
            &[
                platform::grpc_auth::Workload::Gateway,
                platform::grpc_auth::Workload::Post,
                platform::grpc_auth::Workload::Friend,
                platform::grpc_auth::Workload::Auth,
            ],
            &[platform::grpc_auth::Workload::Gateway],
        )?;
        let retention_purge_actor_id = resolve_retention_purge_actor_id(
            production_mode,
            platform::config::optional_env("PROFILE_RETENTION_PURGE_ACTOR_ID"),
        )?;

        let actor_delegation = platform::actor_delegation::ActorDelegationVerifier::from_env(
            production_mode,
            platform::grpc_auth::Workload::Profile,
        )?;
        let config = Self {
            database_migration_mode: migration_runtime::MigrationMode::from_env_value(
                std::env::var("PROFILE_DATABASE_MIGRATION_MODE"),
            )?,
            http_addr: env_or("PROFILE_HTTP_ADDR", "127.0.0.1:8082").parse()?,
            metrics_addr: env_or("PROFILE_METRICS_ADDR", "127.0.0.1:9082").parse()?,
            grpc_addr: env_or("PROFILE_GRPC_ADDR", "127.0.0.1:50052").parse()?,
            database_url: env_or(
                "PROFILE_DATABASE_URL",
                "postgres://postgres:postgres@localhost:5432/profile_db",
            ),
            database_pool_max_connections: parse_u32_setting(
                "PROFILE_DB_POOL_MAX_CONNECTIONS",
                &env_or("PROFILE_DB_POOL_MAX_CONNECTIONS", "5"),
                1,
                512,
            )?,
            seed_enabled: parse_bool_setting(
                "PROFILE_SEED_ENABLED",
                &env_or("PROFILE_SEED_ENABLED", "false"),
            )?,
            caller_registry,
            actor_delegation,
            tls_material,
            retention_purge_worker_enabled: parse_bool_setting(
                "PROFILE_RETENTION_PURGE_WORKER_ENABLED",
                &env_or("PROFILE_RETENTION_PURGE_WORKER_ENABLED", "true"),
            )?,
            retention_purge_worker_interval_ms: parse_u64_setting(
                "PROFILE_RETENTION_PURGE_WORKER_INTERVAL_MS",
                &env_or("PROFILE_RETENTION_PURGE_WORKER_INTERVAL_MS", "3600000"),
                1_000,
                86_400_000,
            )?,
            retention_purge_actor_id,
            mutation_receipt_retention_worker_enabled: parse_bool_setting(
                "PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
                &env_or("PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED", "true"),
            )?,
            mutation_receipt_retention_worker_interval_ms: parse_u64_setting(
                "PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
                &env_or(
                    "PROFILE_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
                    "3600000",
                ),
                1_000,
                86_400_000,
            )?,
            mutation_receipt_retention_batch_size: parse_i32_setting(
                "PROFILE_MUTATION_RECEIPT_RETENTION_BATCH_SIZE",
                &env_or("PROFILE_MUTATION_RECEIPT_RETENTION_BATCH_SIZE", "1000"),
                1,
                5_000,
            )?,
        };
        validate_production_config(production_mode, &config)?;
        Ok(config)
    }
}

fn validate_production_config(production_mode: bool, config: &AppConfig) -> anyhow::Result<()> {
    require_production_safe_socket_addr(production_mode, "PROFILE_HTTP_ADDR", config.http_addr)?;
    require_production_safe_socket_addr(
        production_mode,
        "PROFILE_METRICS_ADDR",
        config.metrics_addr,
    )?;
    require_production_safe_socket_addr(production_mode, "PROFILE_GRPC_ADDR", config.grpc_addr)?;
    require_production_postgres_url(
        production_mode,
        "PROFILE_DATABASE_URL",
        &config.database_url,
    )?;
    require_production_disabled(
        production_mode,
        "PROFILE_SEED_ENABLED",
        config.seed_enabled,
        "demo seed data must not run in production",
    )?;
    Ok(())
}

fn resolve_retention_purge_actor_id(
    production_mode: bool,
    configured: Option<String>,
) -> anyhow::Result<String> {
    let actor_id = match configured
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(value) => value,
        None if !production_mode => LOCAL_RETENTION_PURGE_ACTOR_ID.into(),
        None => anyhow::bail!("PROFILE_RETENTION_PURGE_ACTOR_ID is required in production"),
    };
    Uuid::parse_str(&actor_id)
        .map_err(|_| anyhow::anyhow!("PROFILE_RETENTION_PURGE_ACTOR_ID must be a UUID"))?;
    Ok(actor_id)
}

#[derive(Clone)]
pub struct AppState {
    pub concert_self_profile: profile_application::concert_identity::ReadConcertSelfProfile,
    pub apply_user_deletion: ApplyUserDeletionUseCase<Arc<PgProfileRepository>>,
    pub get_profile: GetProfileUseCase<Arc<PgProfileRepository>>,
    pub get_profile_by_username: GetProfileByUsernameUseCase<Arc<PgProfileRepository>>,
    pub get_profiles_batch: GetProfilesBatchUseCase<Arc<PgProfileRepository>>,
    pub update_profile: UpdateProfileUseCase<Arc<PgProfileRepository>>,
    pub update_privacy: UpdateProfilePrivacyUseCase<Arc<PgProfileRepository>>,
    pub delete_profile: DeleteProfileUseCase<Arc<PgProfileRepository>>,
    pub purge_expired_profiles: PurgeExpiredProfilesUseCase<Arc<PgProfileRepository>>,
    pub retention_purge_worker: ProfileRetentionPurgeWorker<Arc<PgProfileRepository>>,
    pub mutation_receipt_retention_worker:
        ProfileMutationReceiptRetentionWorker<Arc<PgProfileRepository>>,
    pub search_profiles: SearchProfilesUseCase<Arc<PgProfileRepository>>,
    pub upload_avatar: UploadAvatarUseCase<Arc<PgAvatarRepository>, ImageAvatarProcessor>,
    pub get_avatar: GetAvatarUseCase<Arc<PgAvatarRepository>>,
    pub metrics: Arc<ProfileMetrics>,
}

#[derive(Clone, Default)]
pub struct ProfileMetrics {
    counters: Arc<Mutex<BTreeMap<String, i64>>>,
}

impl ProfileMetrics {
    pub fn record_retention_purge_run(&self, outcome: &str) {
        self.inc(
            "profile_retention_purge_runs_total",
            &[("outcome", outcome)],
            1,
        );
    }

    pub fn record_retention_purged_profiles(&self, outcome: &str, count: i64) {
        self.inc(
            "profile_retention_purged_profiles_total",
            &[("outcome", outcome)],
            count,
        );
    }

    pub fn record_mutation_receipt_retention_run(&self, outcome: &str) {
        self.inc(
            "profile_mutation_receipt_retention_runs_total",
            &[("outcome", outcome)],
            1,
        );
    }

    pub fn record_mutation_receipt_outcome_bodies_purged(&self, count: i64) {
        self.inc(
            "profile_mutation_receipt_outcome_bodies_purged_total",
            &[],
            count.max(0),
        );
    }

    pub fn render_prometheus(&self) -> String {
        let counters = self.counters.lock().expect("profile metrics");
        let mut output = String::new();
        for (sample, value) in counters.iter() {
            output.push_str(sample);
            output.push(' ');
            output.push_str(&value.to_string());
            output.push('\n');
        }
        output
    }

    fn inc(&self, name: &str, labels: &[(&str, &str)], amount: i64) {
        let sample = format!("{name}{{{}}}", label_set(labels));
        let mut counters = self.counters.lock().expect("profile metrics");
        *counters.entry(sample).or_insert(0) += amount;
    }
}

fn label_set(labels: &[(&str, &str)]) -> String {
    labels
        .iter()
        .map(|(name, value)| format!("{name}=\"{}\"", escape_label_value(value)))
        .collect::<Vec<_>>()
        .join(",")
}

fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub async fn connect_pool(config: &AppConfig) -> anyhow::Result<PgPool> {
    let options = migration_runtime::serving_options(
        config.database_url.parse()?,
        config.database_migration_mode,
    );
    let pool = PgPoolOptions::new()
        .max_connections(config.database_pool_max_connections)
        .connect_with(options)
        .await?;

    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    migration_runtime::prepare_database(
        &pool,
        migrations.as_path(),
        config.database_migration_mode,
        &[migration_runtime::SchemaProbe {
            name: "PROFILE required schema objects",
            sql: include_str!("schema-probe.sql"),
        }],
    )
    .await?;
    Ok(pool)
}

pub async fn build_state(config: &AppConfig, pool: PgPool) -> anyhow::Result<AppState> {
    if config.seed_enabled {
        seed_profiles(&pool).await?;
    }

    let profile_repository = Arc::new(PgProfileRepository::new(pool.clone()));
    let concert_self_profile = profile_application::concert_identity::ReadConcertSelfProfile::new(
        Arc::new(PgConcertProfileRepository::new(pool.clone())),
    );
    let avatar_repository = Arc::new(PgAvatarRepository::new(pool));
    // Decoding is CPU- and memory-intensive, so keep it off Tokio's async
    // workers and isolate its concurrency/queue from other blocking work.
    let avatar_decode_pool = platform::cpu::WorkerPool::new(AVATAR_DECODE_WORKERS)?;
    let avatar_decode_admission = avatar_decode_pool.class(AVATAR_DECODE_QUEUE_LIMIT)?;
    let avatar_processor = ImageAvatarProcessor::new(avatar_decode_admission);

    let runtime = Arc::new(SystemProfileRuntime);
    let purge_expired_profiles =
        PurgeExpiredProfilesUseCase::new(profile_repository.clone(), runtime.clone());
    let metrics = Arc::new(ProfileMetrics::default());
    let retention_purge_worker = ProfileRetentionPurgeWorker::new(
        purge_expired_profiles.clone(),
        config.retention_purge_actor_id.clone(),
        metrics.clone(),
    );
    let mutation_receipt_retention_worker = ProfileMutationReceiptRetentionWorker::new(
        PurgeExpiredMutationOutcomesUseCase::new(profile_repository.clone()),
        metrics.clone(),
        config.mutation_receipt_retention_batch_size,
    );

    Ok(AppState {
        concert_self_profile,
        apply_user_deletion: ApplyUserDeletionUseCase::new(profile_repository.clone()),
        get_profile: GetProfileUseCase::new(profile_repository.clone()),
        get_profile_by_username: GetProfileByUsernameUseCase::new(profile_repository.clone()),
        get_profiles_batch: GetProfilesBatchUseCase::new(profile_repository.clone()),
        update_profile: UpdateProfileUseCase::new(profile_repository.clone()),
        update_privacy: UpdateProfilePrivacyUseCase::new(profile_repository.clone()),
        delete_profile: DeleteProfileUseCase::new(profile_repository.clone(), runtime),
        purge_expired_profiles,
        retention_purge_worker,
        mutation_receipt_retention_worker,
        search_profiles: SearchProfilesUseCase::new(profile_repository),
        upload_avatar: UploadAvatarUseCase::new(avatar_repository.clone(), avatar_processor),
        get_avatar: GetAvatarUseCase::new(avatar_repository),
        metrics,
    })
}

async fn seed_profiles(pool: &PgPool) -> anyhow::Result<()> {
    for profile in seed_rows() {
        sqlx::query(
            r#"
            insert into profiles (
                user_id, username, nickname, avatar_url, bio, follower_count, post_count, following_count
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            on conflict (user_id) do update
            set username = excluded.username,
                nickname = excluded.nickname,
                avatar_url = excluded.avatar_url,
                bio = excluded.bio,
                follower_count = excluded.follower_count,
                post_count = excluded.post_count,
                following_count = excluded.following_count,
                updated_at = now()
            "#,
        )
        .bind(profile.user_id)
        .bind(profile.username)
        .bind(profile.nickname)
        .bind(profile.avatar_url)
        .bind(profile.bio)
        .bind(profile.follower_count)
        .bind(profile.post_count)
        .bind(profile.following_count)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn seed_rows() -> Vec<Profile> {
    vec![
        seed_profile(
            "11111111-1111-1111-1111-111111111111",
            "hana_ani",
            "Hana Akiyama",
            "/avatars/cat.svg",
            "Anime cafe nights, storyboard notes, and soft color palettes.",
            0,
            10,
            4,
        ),
        seed_profile(
            "22222222-2222-2222-2222-222222222222",
            "jin_slugg",
            "Jin Park",
            "/avatars/fox.svg",
            "Baseball stats, bullpen chatter, and scorebook ink.",
            1,
            10,
            2,
        ),
        seed_profile(
            "33333333-3333-3333-3333-333333333333",
            "soyeon_emo",
            "Soyeon Lee",
            "/avatars/owl.svg",
            "MCR forever. Guitars, eyeliner, and loud choruses.",
            1,
            10,
            2,
        ),
        seed_profile(
            "44444444-4444-4444-4444-444444444444",
            "mira_ghibli",
            "Mira Han",
            "/avatars/dog.svg",
            "Ghibli skies, pencil dust, and quiet story beats.",
            1,
            10,
            2,
        ),
        seed_profile(
            "55555555-5555-5555-5555-555555555555",
            "tae_dingers",
            "Tae Kim",
            "/avatars/bear.svg",
            "Cage work, long toss, and night game energy.",
            1,
            10,
            2,
        ),
        seed_profile(
            "66666666-6666-6666-6666-666666666666",
            "minji_blackparade",
            "Minji Seo",
            "/avatars/rabbit.svg",
            "Black Parade mood. Writing hooks and chasing tone.",
            2,
            10,
            1,
        ),
        seed_profile(
            "77777777-7777-7777-7777-777777777777",
            "ryu_shonen",
            "Ryu Tanaka",
            "/avatars/tiger.svg",
            "Shonen panels, speed lines, and training arcs.",
            2,
            10,
            2,
        ),
        seed_profile(
            "88888888-8888-8888-8888-888888888888",
            "eun_base",
            "Eun Choi",
            "/avatars/panda.svg",
            "KBO chants, caps, and extra innings recaps.",
            3,
            10,
            0,
        ),
        seed_profile(
            "99999999-9999-9999-9999-999999999999",
            "lexi_pierce",
            "Lexi Moon",
            "/avatars/koala.svg",
            "Pierce the Veil fan. Loud riffs and late gigs.",
            3,
            10,
            0,
        ),
        seed_profile(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "noa_animator",
            "Noa Kim",
            "/avatars/whale.svg",
            "Animation student. Timing charts and clean-up passes.",
            2,
            10,
            1,
        ),
    ]
}

fn seed_profile(
    user_id: &str,
    username: &str,
    nickname: &str,
    avatar_url: &str,
    bio: &str,
    follower_count: i64,
    post_count: i64,
    following_count: i64,
) -> Profile {
    Profile {
        user_id: Uuid::parse_str(user_id).expect("seed uuid"),
        username: username.to_owned(),
        nickname: nickname.to_owned(),
        avatar_url: avatar_url.to_owned(),
        bio: bio.to_owned(),
        follower_count,
        post_count,
        following_count,
        account_visibility: AccountVisibility::Public,
        lifecycle_status: "ACTIVE".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_retention_purge_actor_id_must_be_explicit() {
        let error = resolve_retention_purge_actor_id(true, None)
            .expect_err("production should require explicit actor id")
            .to_string();

        assert!(error.contains("PROFILE_RETENTION_PURGE_ACTOR_ID"));
    }

    #[test]
    fn parity_retention_purge_actor_id_uses_local_default() {
        let actor_id = resolve_retention_purge_actor_id(false, None).expect("actor");

        assert_eq!(actor_id, LOCAL_RETENTION_PURGE_ACTOR_ID);
    }

    #[test]
    fn production_config_rejects_unsafe_profile_defaults() {
        let mut config = production_config();
        config.grpc_addr = "127.0.0.1:50052".parse().expect("addr");
        assert!(validate_production_config(true, &config)
            .expect_err("loopback gRPC bind must fail")
            .to_string()
            .contains("PROFILE_GRPC_ADDR"));

        let mut config = production_config();
        config.database_url = "postgres://postgres:postgres@localhost:5432/profile_db".into();
        assert!(validate_production_config(true, &config)
            .expect_err("local database must fail")
            .to_string()
            .contains("PROFILE_DATABASE_URL"));

        let mut config = production_config();
        config.seed_enabled = true;
        assert!(validate_production_config(true, &config)
            .expect_err("seed must fail")
            .to_string()
            .contains("PROFILE_SEED_ENABLED"));
    }

    #[test]
    fn production_config_accepts_explicit_remote_profile_settings() {
        validate_production_config(true, &production_config()).expect("production config");
    }

    #[test]
    fn parity_config_allows_local_profile_defaults() {
        let mut config = production_config();
        config.http_addr = "127.0.0.1:8082".parse().expect("addr");
        config.grpc_addr = "127.0.0.1:50052".parse().expect("addr");
        config.database_url = "postgres://postgres:postgres@localhost:5432/profile_db".into();
        config.seed_enabled = true;
        validate_production_config(false, &config).expect("parity config");
    }

    fn production_config() -> AppConfig {
        AppConfig {
            database_migration_mode: migration_runtime::MigrationMode::Startup,
            http_addr: "0.0.0.0:8082".parse().expect("addr"),
            metrics_addr: "0.0.0.0:9082".parse().expect("addr"),
            grpc_addr: "0.0.0.0:50052".parse().expect("addr"),
            database_pool_max_connections: 5,
            database_url: "postgres://profile:secret@postgres.internal:5432/profile_db".into(),
            seed_enabled: false,
            actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
                platform::grpc_auth::Workload::Profile,
            ),
            caller_registry: platform::grpc_auth::CallerRegistry::new()
                .register(
                    platform::grpc_auth::Workload::Gateway,
                    "production-service-token",
                )
                .expect("production caller registry"),
            tls_material: None,
            retention_purge_worker_enabled: true,
            retention_purge_worker_interval_ms: 3_600_000,
            retention_purge_actor_id: LOCAL_RETENTION_PURGE_ACTOR_ID.into(),
            mutation_receipt_retention_worker_enabled: true,
            mutation_receipt_retention_worker_interval_ms: 3_600_000,
            mutation_receipt_retention_batch_size: 1_000,
        }
    }

    mod external_pool_acceptance {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/migration-runtime/tests/support/service_pool.rs"
        ));
    }

    #[tokio::test]
    async fn external_migration_mode_rejects_schema_damage_without_owner_privileges(
    ) -> anyhow::Result<()> {
        external_pool_acceptance::verify(
            |url, mode| async move {
                let mut config = production_config();
                config.database_url = url;
                config.database_migration_mode = mode;
                connect_pool(&config).await
            },
            "profiles",
            "user_id",
            "profiles_user_deletion_fence",
        )
        .await
    }
}
