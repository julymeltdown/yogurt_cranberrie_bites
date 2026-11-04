//! One actual factory core, real PG16 stores and real mTLS owner services.
use super::{
    capture, environment,
    notification::{Observation, Observed},
    owners, release,
};
use concert_application::community::giveaway::{PickupKey, PickupKeys};
use concert_service::{
    config::canonical_runtime::{
        CanonicalOwnerComposition, CanonicalOwnerConfig, CanonicalOwnerEndpoints,
    },
    product_rpc::{
        scope_ledger::{ChildRecoveryKey, ChildRecoveryKeys},
        settlement::{PgNativeAbortResolver, POLICY},
    },
};
use platform::{
    actor_delegation::{relationship_read::RelationshipReadVerifier, ActorDelegationVerifier},
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

fn incoming(
    listener: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(listener, |l| async {
        Some((l.accept().await.map(|(s, _)| s), l))
    })
}
fn spawn(
    f: impl std::future::Future<Output = Result<(), tonic::transport::Error>> + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move { f.await.expect("actual CF01 service") })
}
pub(super) struct Fixture {
    pub auth: PgPool,
    pub friend: PgPool,
    pub concert: PgPool,
    pub profile: PgPool,
    pub notification: PgPool,
    pub owners: CanonicalOwnerComposition,
    pub event: Uuid,
    pub observation: Arc<Observation>,
    endpoints: CanonicalOwnerEndpoints,
    _auth: owners::Running,
    _friend: owners::Running,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}
fn config(
    instance: Uuid,
    endpoints: CanonicalOwnerEndpoints,
) -> anyhow::Result<CanonicalOwnerConfig> {
    Ok(CanonicalOwnerConfig {
        instance,
        child_keys: Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("cf01-current", &[93; 32])?,
            vec![],
        )?),
        pickup_keys: Arc::new(
            PickupKeys::new(
                PickupKey::new("cf01-pickup", &[94; 32])
                    .map_err(|_| anyhow::anyhow!("pickup key"))?,
                vec![],
            )
            .map_err(|_| anyhow::anyhow!("pickup keys"))?,
        ),
        endpoints,
        credential: "concert-fixture".into(),
        tls: environment::tls(Workload::Concert)?,
        relationship_signer: owners::signer()?,
    })
}
impl Fixture {
    pub async fn new() -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let profile = environment::pool("profile").await?;
        let notification = environment::pool("notification").await?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let root_url = format!("https://{}", listener.local_addr()?);
        let running_auth = owners::auth(auth.clone(), &root_url).await?;
        let running_friend = owners::friend(friend.clone(), &root_url).await?;
        use profile_service::adapters::r#in::grpc::concert_public_profile::{
            proto, PublicProfileServer, PUBLIC_PROFILE_POLICY,
        };
        let public = guard(
            &PUBLIC_PROFILE_POLICY,
            proto::concert_public_profile_service_server::ConcertPublicProfileServiceServer::new(
                PublicProfileServer::from_pool(profile.clone()),
            ),
            CallerRegistry::new().register(Workload::Concert, "concert-fixture")?,
            RelationshipReadVerifier::new(
                Workload::Profile,
                &[owners::signer()?.public_key()],
                &ActorDelegationVerifier::new(
                    Workload::Profile,
                    &[capture::gateway()?.public_key()],
                )?,
            )?
            .into_inner(),
        )?;
        let profile_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let profile_url = format!("https://{}", profile_listener.local_addr()?);
        let profile_task = spawn(
            tonic::transport::Server::builder()
                .tls_config(environment::tls(Workload::Profile)?.server_config()?)?
                .layer(CallDeadlineLayer)
                .add_service(public)
                .serve_with_incoming(incoming(profile_listener)),
        );
        use notification_service::concert::{
            authority::{NotificationInboxService, INBOX_POLICY},
            transport::proto as np,
        };
        let inbox = NotificationInboxService::connect(
            notification.clone(),
            &running_auth.url,
            &running_friend.url,
            "notification-fixture",
            Some(&environment::tls(Workload::Notification)?),
            None,
        )
        .await?;
        let observation = Arc::new(Observation::default());
        let observed = Observed {
            inner: inbox,
            state: observation.clone(),
        };
        let server = guard_without_delegation(
            &INBOX_POLICY,
            np::concert_inbox_service_server::ConcertInboxServiceServer::new(observed),
            CallerRegistry::new().register(Workload::Concert, "concert-fixture")?,
        )?;
        let notification_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let notification_url = format!("https://{}", notification_listener.local_addr()?);
        let notification_task = spawn(
            tonic::transport::Server::builder()
                .tls_config(environment::tls(Workload::Notification)?.server_config()?)?
                .layer(CallDeadlineLayer)
                .add_service(server)
                .serve_with_incoming(incoming(notification_listener)),
        );
        let endpoints = CanonicalOwnerEndpoints {
            auth: running_auth.url.clone(),
            friend: running_friend.url.clone(),
            profile: profile_url,
            notification: notification_url,
        };
        let product = release::product_release()?;
        let document: serde_json::Value =
            serde_json::from_slice(product.release().bootstrap_json())?;
        let event = document["event"]["event_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("event missing"))?
            .parse()?;
        let instance = Uuid::new_v4();
        let make_endpoints = || CanonicalOwnerEndpoints {
            auth: endpoints.auth.clone(),
            friend: endpoints.friend.clone(),
            profile: endpoints.profile.clone(),
            notification: endpoints.notification.clone(),
        };
        let owners = CanonicalOwnerComposition::compose(
            concert.clone(),
            product,
            config(instance, make_endpoints())?,
        )?;
        let root = guard_without_delegation(
            &POLICY,
            owners
                .core
                .settlement_service(Arc::new(PgNativeAbortResolver::new(
                    concert.clone(),
                    instance,
                )?))
                .server(),
            CallerRegistry::new()
                .register(Workload::Auth, "auth-fixture")?
                .register(Workload::Friend, "friend-fixture")?,
        )?;
        let root_task = spawn(
            tonic::transport::Server::builder()
                .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
                .layer(CallDeadlineLayer)
                .add_service(root)
                .serve_with_incoming(incoming(listener)),
        );
        Ok(Self {
            auth,
            friend,
            concert,
            profile,
            notification,
            owners,
            event,
            observation,
            endpoints,
            _auth: running_auth,
            _friend: running_friend,
            tasks: vec![profile_task, notification_task, root_task],
        })
    }
    pub fn compose(&self, instance: Uuid) -> anyhow::Result<CanonicalOwnerComposition> {
        let endpoints = CanonicalOwnerEndpoints {
            auth: self.endpoints.auth.clone(),
            friend: self.endpoints.friend.clone(),
            profile: self.endpoints.profile.clone(),
            notification: self.endpoints.notification.clone(),
        };
        CanonicalOwnerComposition::compose(
            self.concert.clone(),
            release::product_release()?,
            config(instance, endpoints)?,
        )
    }
    pub async fn seed(&self) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
        let (actor, family, subjects) = environment::identities(&self.auth, 1).await?;
        let subject = subjects[0];
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'CF01 native','OPEN') on conflict(event_id) do nothing").bind(self.event).bind(format!("cf01-{}",self.event.simple())).execute(&self.concert).await?;
        for user in [actor, subject] {
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')").bind(self.event).bind(user).execute(&self.concert).await?;
            sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(self.event).bind(user).execute(&self.concert).await?;
        }
        sqlx::query("insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) values($1,$2,'1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1)").bind(self.event).bind(subject).execute(&self.concert).await?;
        sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio,account_visibility) values($1,$2,'CF01 actual author','private-url-canary','CF01 actual bio','PUBLIC')").bind(subject).bind(format!("cf01-{}",subject.simple())).execute(&self.profile).await?;
        Ok((actor, family, subject))
    }
}
