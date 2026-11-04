//! CF02 production factory inputs and real owned stores. No positive authority mock.
use super::{environment, owners, release};
use concert_application::{
    bootstrap::ValidatedProductRelease,
    community::giveaway::{PickupKey, PickupKeys},
};
use concert_service::config::participant_runtime::{
    self, ParticipantProductOwners, ParticipantRuntime, ParticipantRuntimeSettings,
};
use platform::{
    actor_delegation::{
        concert_self_profile::ConcertSelfProfileSigner, ActorDelegationSigner,
        ActorDelegationVerifier,
    },
    grpc_auth::Workload,
};
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    sync::{atomic::AtomicBool, Arc},
};
use uuid::Uuid;

pub(crate) fn self_signer() -> anyhow::Result<ConcertSelfProfileSigner> {
    ConcertSelfProfileSigner::from_seed(&[76; 32], &owners::gateway(Workload::Concert)?)
}
pub(crate) fn decision_key() -> anyhow::Result<[u8; 32]> {
    Ok(ActorDelegationSigner::from_seed(Workload::Profile, &[77; 32])?.public_key())
}
pub(crate) fn raw_settings(
    instance: Uuid,
    digest: &str,
    keys: Option<serde_json::Value>,
) -> anyhow::Result<participant_runtime::ParticipantSettings> {
    let v: BTreeMap<String,String> = [
        ("CONCERT_PARTICIPANT_MODE","live".into()),
        ("CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT",environment::url("auth")?),
        ("CONCERT_PARTICIPANT_FRIEND_GRPC_ENDPOINT",environment::url("friend")?),
        ("CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT",environment::url("profile")?),
        ("CONCERT_PRODUCER_INSTANCE_ID",instance.to_string()),
        ("CONCERT_PARTICIPANT_RELEASE_DIGEST",digest.into()),
        ("CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON",keys.unwrap_or_else(||serde_json::json!([{"id":"cf02-active","secret_hex":"5d".repeat(32)},{"id":"cf02-retained","secret_hex":"5e".repeat(32)}])).to_string()),
        ("GATEWAY_SERVICE_CREDENTIAL","gateway-fixture".into()),
        ("AUTH_SERVICE_CREDENTIAL","auth-fixture".into()),
        ("FRIEND_SERVICE_CREDENTIAL","friend-fixture".into()),
        ("PROFILE_SERVICE_CREDENTIAL","profile-fixture".into()),
        ("CONCERT_SERVICE_CREDENTIAL","concert-fixture".into()),
    ].into_iter().map(|(k,v)|(k.into(),v)).collect();
    participant_runtime::settings(|k| Ok(v.get(k).cloned()), true)?
        .ok_or_else(|| anyhow::anyhow!("actual Participant settings"))
}
pub(crate) fn configured(
    instance: Uuid,
    digest: &str,
) -> anyhow::Result<ParticipantRuntimeSettings> {
    configured_with_keys(instance, digest, None)
}
pub(crate) fn configured_with_keys(
    instance: Uuid,
    digest: &str,
    keys: Option<serde_json::Value>,
) -> anyhow::Result<ParticipantRuntimeSettings> {
    let settings = raw_settings(instance, digest, keys)?;
    ParticipantRuntimeSettings::from_validated(
        settings,
        environment::tls(Workload::Concert)?,
        owners::gateway(Workload::Concert)?,
        owners::signer()?,
        self_signer()?,
        &[decision_key()?],
    )
}
pub(crate) fn additional() -> anyhow::Result<ParticipantProductOwners> {
    Ok(ParticipantProductOwners {
        profile_endpoint: environment::url("profile")?,
        notification_endpoint: environment::url("notification")?,
        pickup_keys: Arc::new(
            PickupKeys::new(
                PickupKey::new("cf02-pickup", &[96; 32])
                    .map_err(|_| anyhow::anyhow!("pickup key"))?,
                vec![],
            )
            .map_err(|_| anyhow::anyhow!("pickup keyring"))?,
        ),
    })
}
pub(crate) struct Fixture {
    pub auth: PgPool,
    pub friend: PgPool,
    pub concert: PgPool,
    pub profile: PgPool,
    pub product: Arc<ValidatedProductRelease>,
    pub actor: Uuid,
    pub family: Uuid,
    pub subject: Uuid,
    pub event: Uuid,
    pub deny_complete: Arc<AtomicBool>,
    _servers: Vec<owners::Running>,
}
impl Fixture {
    pub(crate) async fn new() -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let profile = environment::pool("profile").await?;
        let product = release::product_release()?;
        let document: serde_json::Value =
            serde_json::from_slice(product.release().bootstrap_json())?;
        let event: Uuid = document["event"]["event_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("event"))?
            .parse()?;
        let (actor, family, subjects) = environment::identities(&auth, 1).await?;
        let subject = subjects[0];
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'CF02 actual shared','OPEN')").bind(event).bind(format!("cf02-{event}")).execute(&concert).await?;
        for user in [actor, subject] {
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')").bind(event).bind(user).execute(&concert).await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event).bind(user).execute(&concert).await?;
            sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)").bind(event).bind(user).execute(&concert).await?;
            sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio,account_visibility) values($1,$2,'CF02 actual canonical','private-canary','CF02 native bio','PUBLIC')").bind(user).bind(format!("cf02-{user}")).execute(&profile).await?;
        }
        let deny_complete = Arc::new(AtomicBool::new(false));
        let servers = vec![
            owners::auth(auth.clone()).await?,
            owners::friend(friend.clone()).await?,
            owners::profile(profile.clone(), deny_complete.clone()).await?,
        ];
        Ok(Self {
            auth,
            friend,
            concert,
            profile,
            product,
            actor,
            family,
            subject,
            event,
            deny_complete,
            _servers: servers,
        })
    }
    /// A different producer instance must get fresh actual owner dispatch
    /// channels. Never reuse cached calls into the preceding Concert listener.
    pub(crate) async fn restart_owners(&mut self) -> anyhow::Result<()> {
        for server in self._servers.drain(..) {
            server.stop().await?;
        }
        self._servers = vec![
            owners::auth(self.auth.clone()).await?,
            owners::friend(self.friend.clone()).await?,
            owners::profile(self.profile.clone(), self.deny_complete.clone()).await?,
        ];
        Ok(())
    }
    pub(crate) async fn prepared(&self) -> anyhow::Result<Vec<i64>> {
        let mut out = Vec::new();
        for p in [&self.auth, &self.friend, &self.concert, &self.profile] {
            out.push(
                sqlx::query_scalar(
                    "select count(*) from pg_prepared_xacts where database=current_database()",
                )
                .fetch_one(p)
                .await?,
            );
        }
        Ok(out)
    }
}
pub(crate) struct Mounted {
    pub core: concert_service::config::canonical_runtime::CanonicalRuntimeCore,
    pub fence:
        Arc<concert_service::adapters::out::relationship_authority::guarded::CanonicalOwnerFence>,
    pub service: concert_service::participant::live::service::GuardedParticipantService,
    pub callers: platform::grpc_auth::CallerRegistry,
    pub gateway: ActorDelegationVerifier,
    root: Option<owners::Running>,
    shutdown: tokio::sync::watch::Sender<bool>,
    recovery: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for Mounted {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.recovery.as_ref() {
            task.abort();
        }
    }
}
impl Mounted {
    pub(crate) async fn new(runtime: ParticipantRuntime) -> anyhow::Result<Self> {
        let root = owners::concert(
            runtime.settlement,
            runtime.decision,
            runtime.decision_verifier,
        )
        .await?;
        let (shutdown, rx) = tokio::sync::watch::channel(false);
        let recovery = tokio::spawn(runtime.recovery.run(rx));
        Ok(Self {
            core: runtime.canonical,
            fence: runtime.fence,
            service: runtime.participant,
            callers: runtime.callers,
            gateway: runtime.gateway_verifier,
            root: Some(root),
            shutdown,
            recovery: Some(recovery),
        })
    }
    pub(crate) async fn stop(mut self) -> anyhow::Result<()> {
        let _ = self.shutdown.send(true);
        if let Some(task) = self.recovery.take() {
            tokio::time::timeout(std::time::Duration::from_secs(3), task).await??;
        }
        if let Some(root) = self.root.take() {
            root.stop().await?;
        }
        Ok(())
    }
}
