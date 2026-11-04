// Strict owned fixture adapted from product_scope_ledger.rs and
// owner_authority_support/environment.rs at Root 73e89bce. Synthetic scope
// rows exercise lookup mechanics, never canonical owner admission. The parent
// alias permits reuse inside private library tests without exporting the reader.
use super::binding_test_types::{
    adapters::out::relationship_authority::guarded::{ChildQuery, ChildRecoveryReference},
    product_rpc::scope_ledger::{
        ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger,
    },
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};
use uuid::Uuid;

pub async fn pool() -> PgPool {
    let root = PathBuf::from(std::env::var("NB01_OWNER_ROOT").expect("owned root required"));
    let m = std::fs::symlink_metadata(&root).unwrap();
    assert!(
        root.is_absolute()
            && m.is_dir()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o077 == 0
    );
    assert_eq!(
        format!("{}:{}", m.dev(), m.ino()),
        std::env::var("NB01_OWNER_IDENTITY").unwrap()
    );
    let marker = root.join("mcr-owner.json");
    let mm = std::fs::symlink_metadata(&marker).unwrap();
    assert!(
        mm.is_file()
            && !mm.file_type().is_symlink()
            && mm.permissions().mode() & 0o077 == 0
            && mm.uid() == m.uid()
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker).unwrap()).unwrap();
    assert_eq!(value["uid"].as_u64(), Some(m.uid() as u64));
    assert_eq!(
        value["token"].as_str(),
        Some(std::env::var("NB01_OWNER_TOKEN").unwrap().as_str())
    );
    let options = PgConnectOptions::from_str(&std::env::var("NB01_DATABASE_URL").unwrap()).unwrap();
    assert_eq!(
        options.get_database(),
        Some("mcr_notification_root_binding")
    );
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(options.get_socket(), Some(&root.join("socket")));
    assert_eq!(options.get_port(), 5432);
    assert!(options.get_options().is_none());
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    let row = sqlx::query("SELECT current_database() db,current_setting('data_directory') data,current_setting('listen_addresses') listeners,(SELECT system_identifier::text FROM pg_control_system()) system")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(row.get::<String, _>("db"), "mcr_notification_root_binding");
    assert_eq!(
        row.get::<String, _>("data"),
        root.join("data").to_string_lossy()
    );
    assert!(row.get::<String, _>("listeners").is_empty());
    assert_eq!(
        row.get::<String, _>("system"),
        std::env::var("NB01_SYSTEM").unwrap()
    );
    pool
}

struct NoSettlement;
#[async_trait::async_trait]
impl ChildIntentSettler for NoSettlement {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), tonic::Status> {
        panic!("Binding lookup must never settle a child or grant authority")
    }
}
pub fn ledger(pool: &PgPool, instance: Uuid) -> PgScopeLedger {
    PgScopeLedger::new(
        pool.clone(),
        instance,
        Arc::new(
            ChildRecoveryKeys::new(
                ChildRecoveryKey::new("nb01-fixture-only", &[43; 32]).unwrap(),
                vec![],
            )
            .unwrap(),
        ),
        Arc::new(NoSettlement),
    )
    .unwrap()
}
pub fn binding() -> OwnerBinding {
    let event = Uuid::new_v4();
    OwnerBinding {
        actor_id: Uuid::new_v4(),
        event_id: event,
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        operation: "listConcertNotifications".into(),
        expires_unix_micros: 1_700_000_000_123_456,
        parent_digest: [11; 32],
        parent_body_digest: [22; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: "GET".into(),
        http_target: format!(
            "/api/concert/events/{event}/me/notifications?limit=7&unreadOnly=true"
        ),
        body_digest: [33; 32],
    }
}
pub fn query(b: &OwnerBinding) -> ChildQuery {
    ChildQuery {
        actor: b.actor_id,
        event: b.event_id,
        subject: b.actor_id,
        family: b.session_family_id,
        request: b.request_id,
        expires_unix_micros: b.expires_unix_micros,
        binding_digest: b.digest().unwrap(),
    }
}
pub async fn store(
    pool: &PgPool,
    instance: Uuid,
    digest: [u8; 32],
    value: serde_json::Value,
    phase: &str,
) {
    // Insert immutable, explicitly synthetic historical rows without disabling
    // constraints/triggers or manufacturing owner Track acknowledgements.
    sqlx::query("INSERT INTO public.concert_product_owner_scopes(instance_id,scope_id,binding_digest,binding,phase,sealed_at,root_finished_at) VALUES($1,$2,$3,$4,$5,CASE WHEN $5<>'ACTIVE' THEN clock_timestamp() END,CASE WHEN $5='ROOT_FINISHED' THEN clock_timestamp() END)")
        .bind(instance).bind(Uuid::new_v4()).bind(digest.as_slice()).bind(value).bind(phase).execute(pool).await.unwrap();
}
