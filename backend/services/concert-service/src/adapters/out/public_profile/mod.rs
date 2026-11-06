//! Current arbitrary-subject PublicProfile assembled only inside a canonical guard.
//! Constructors never accept context identifiers as admission or fabricate avatars.
use super::relationship_authority::guarded::{
    ChildRecoveryReference, GuardedSubject, HeldChildGuard,
};
use concert_application::{bootstrap::ValidatedProductRelease, feed::PublicAuthor};
use concert_domain::{
    avatar::{config_value, AvatarCatalog},
    relationship::Field,
    Revision,
};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tonic::Status;
mod local;
mod profile;

pub struct PublicProfileReader {
    pool: PgPool,
    profile: profile::ProfileReader,
    catalog: AvatarCatalog,
    event: uuid::Uuid,
    local: Mutex<BTreeMap<LocalKey, Arc<tokio::sync::Mutex<Weak<local::LocalChild>>>>>,
}
type LocalKey = ([u8; 32], uuid::Uuid, uuid::Uuid, i64, uuid::Uuid, i64);
/// Current local facts, usable only with the still-held actual owner scope.
#[derive(Clone, Copy)]
pub struct LocalPolicySnapshot {
    pub event_revision: i64,
    pub pair_revision: i64,
    pub valid_until: chrono::DateTime<chrono::Utc>,
    pub phase: concert_domain::EventPhase,
}
impl PublicProfileReader {
    pub fn connect(
        pool: PgPool,
        release: Arc<ValidatedProductRelease>,
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: platform::actor_delegation::relationship_read::RelationshipReadSigner,
    ) -> anyhow::Result<Self> {
        // Reuse the owner product-art/provenance gate; no self-read grant is made.
        super::repository::avatar::AvatarPersistence::from_product(pool.clone(), release.clone())
            .map_err(|_| anyhow::anyhow!("Public Profile product avatar unavailable"))?;
        let document: Value = serde_json::from_slice(release.release().bootstrap_json())?;
        let event = uuid::Uuid::parse_str(
            document["event"]["event_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Public Profile release event unavailable"))?,
        )?;
        anyhow::ensure!(!event.is_nil(), "Public Profile release event unavailable");
        let catalog = AvatarCatalog::from_validated_manifest(&document["avatar_manifest"])
            .map_err(|_| anyhow::anyhow!("Public Profile catalog unavailable"))?;
        Ok(Self {
            pool,
            profile: profile::ProfileReader::connect(endpoint, token, tls, signer)?,
            catalog,
            event,
            local: Mutex::new(BTreeMap::new()),
        })
    }
    /// Settlement only: configured owners and exact closed private references.
    /// The root ledger must authenticate the saved reference before calling this.
    pub async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        let q = reference.query();
        reference.intent_id()?;
        if q.event != self.event
            || [q.actor, q.event, q.subject, q.family, q.request]
                .iter()
                .any(uuid::Uuid::is_nil)
            || q.binding_digest == [0; 32]
            || q.expires_unix_micros <= 0
        {
            return Err(unavailable());
        }
        match reference {
            ChildRecoveryReference::TargetRead { .. } => Err(Status::failed_precondition(
                "Public Profile recovery does not support Notification target children",
            )),
            ChildRecoveryReference::NotificationInbox { .. } => Err(Status::failed_precondition(
                "Public Profile recovery does not support Notification inbox children",
            )),
            ChildRecoveryReference::ProfilePublic { .. } => {
                self.profile.recover_one(reference).await
            }
            ChildRecoveryReference::LocalPublic { .. } => {
                local::recover_one(&self.pool, reference).await
            }
            ChildRecoveryReference::GiftPrivate { .. } => Err(Status::failed_precondition(
                "Public Profile recovery does not support Gift private children",
            )),
            ChildRecoveryReference::SocialRead { .. } => Err(Status::failed_precondition(
                "Public Profile recovery does not support Social read children",
            )),
        }
    }
    /// Root must retain/run the actual owner scope through final C04 serialization
    /// and settle all children only after the entire response has been assembled.
    pub async fn read(
        &self,
        subject: &GuardedSubject,
    ) -> Result<PublicAuthor, concert_domain::feed::FeedError> {
        let deadline = subject.binding().deadline();
        platform::call_deadline::scope(
            deadline.stop().into_std(),
            tokio::time::timeout_at(deadline.stop(), self.read_current(subject)),
        )
        .await
        .map_err(|_| concert_domain::feed::FeedError::DependencyUnavailable)?
        .map_err(feed_error)
    }

    pub async fn local_subject(
        &self,
        subject: &GuardedSubject,
    ) -> Result<LocalPolicySnapshot, concert_domain::feed::FeedError> {
        self.policy(subject).await
    }
    pub async fn policy(
        &self,
        subject: &GuardedSubject,
    ) -> Result<LocalPolicySnapshot, concert_domain::feed::FeedError> {
        let deadline = subject.binding().deadline();
        platform::call_deadline::scope(
            deadline.stop().into_std(),
            tokio::time::timeout_at(deadline.stop(), async {
                let held = self.local(subject).await?;
                held.check().await?;
                subject.check().await?;
                Ok::<_, Status>(held.policy)
            }),
        )
        .await
        .map_err(|_| concert_domain::feed::FeedError::DependencyUnavailable)?
        .map_err(feed_error)
    }
    async fn read_current(&self, subject: &GuardedSubject) -> Result<PublicAuthor, Status> {
        let held = self.local(subject).await?;
        let avatar = held
            .avatar
            .as_ref()
            .map_err(|_| unavailable())?
            .as_ref()
            .ok_or_else(|| Status::not_found("Public Profile unavailable"))?;
        let mut content = held.content.lock().await;
        if content.is_none() {
            *content = Some(self.profile.read(subject).await?);
        }
        subject.check().await?;
        held.check().await?;
        let content = content.as_ref().ok_or_else(unavailable)?;
        let q = subject.query();
        let value = json!({"user_id":q.subject.uuid(),"display_name":content.display_name,"bio":content.bio,"avatar":avatar});
        validate(&value)?;
        let json = serde_json::to_vec(&value).map_err(|_| unavailable())?;
        subject.check().await?;
        Ok(PublicAuthor {
            user_id: q.subject.uuid(),
            json,
        })
    }
    async fn local(&self, subject: &GuardedSubject) -> Result<Arc<local::LocalChild>, Status> {
        subject.check().await?;
        let q = subject.query();
        let b = subject.binding();
        let v = subject.revision_vector();
        if q.field != Field::PublicProfile
            || q.actor.uuid() != b.actor()
            || q.event.uuid() != b.event()
            || q.event.uuid() != self.event
            || q.subject.uuid().is_nil()
        {
            return Err(unavailable());
        }
        // Keys retain both complete owner generations. Entries retain only an
        // actual held child, never a standalone allow or post-settlement DTO.
        let key = (
            b.digest(),
            q.subject.uuid(),
            v.auth.epoch,
            v.auth.revision,
            v.friend.epoch,
            v.friend.revision,
        );
        let slot = {
            let mut cache = self.local.lock().map_err(|_| unavailable())?;
            cache.retain(|_, slot| {
                Arc::strong_count(slot) > 1
                    || slot.try_lock().map_or(true, |v| v.strong_count() > 0)
            });
            if !cache.contains_key(&key) && cache.len() >= 16 * 101 {
                return Err(unavailable());
            }
            cache
                .entry(key)
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(Weak::new())))
                .clone()
        };
        let mut stored = slot.lock().await;
        if let Some(held) = stored.upgrade() {
            held.check().await?;
            subject.check().await?;
            return Ok(held);
        }
        let held = self.acquire_local(subject).await?;
        subject.retain_child(held.clone()).await?;
        subject.check().await?;
        *stored = Arc::downgrade(&held);
        Ok(held)
    }
    async fn acquire_local(
        &self,
        subject: &GuardedSubject,
    ) -> Result<Arc<local::LocalChild>, Status> {
        let q = subject.query();
        let b = subject.binding();
        let deadline = b.deadline();
        let mut tx = self.pool.begin().await.map_err(dependency_error)?;
        local::budget(&mut tx, deadline).await?;
        let event = sqlx::query(
            "select phase,revision from public.concert_events where event_id=$1 for share",
        )
        .bind(self.event)
        .fetch_optional(&mut *tx)
        .await
        .map_err(dependency_error)?;
        let event = event.ok_or_else(unavailable)?;
        let phase = event
            .try_get::<String, _>("phase")
            .map_err(dependency_error)?
            .parse::<concert_domain::EventPhase>()
            .map_err(dependency_error)?;
        let event_revision = Revision::new(event.try_get("revision").map_err(dependency_error)?)
            .map_err(dependency_error)?;
        if !matches!(
            phase,
            concert_domain::EventPhase::Open
                | concert_domain::EventPhase::Live
                | concert_domain::EventPhase::ReadOnly
        ) {
            return Err(unavailable());
        }
        let pair_revision = local_pair_and_members_in(
            &mut tx,
            self.event,
            b.actor(),
            q.subject.uuid(),
        )
        .await?;
        let pair_revision =
            Revision::new(pair_revision).map_err(dependency_error)?;
        local::budget(&mut tx, deadline).await?;
        let row=sqlx::query("select event_id,user_id,revision,contract_version,representation,preset_id,manifest_id,manifest_version,body_id,eyes_id,hair_id,outfit_id,accessory_id,skin_tone_id from public.concert_avatar_settings where event_id=$1 and user_id=$2 for share")
            .bind(self.event).bind(q.subject.uuid()).fetch_optional(&mut *tx).await.map_err(dependency_error)?;
        let avatar = (|| -> Result<Option<Value>, ()> {
            if let Some(row) = row {
                if row.try_get::<uuid::Uuid, _>("event_id").map_err(|_| ())? != self.event
                    || row.try_get::<uuid::Uuid, _>("user_id").map_err(|_| ())? != q.subject.uuid()
                {
                    return Err(());
                }
                let selection =
                    super::repository::avatar::selection_from_row(&row).map_err(|_| ())?;
                self.catalog.validate(&selection).map_err(|_| ())?;
                let revision =
                    Revision::new(row.try_get("revision").map_err(|_| ())?).map_err(|_| ())?;
                Ok(Some(
                    json!({"revision":revision.get(),"config":config_value(&selection)}),
                ))
            } else {
                Ok(None)
            }
        })();
        subject.check().await?;
        let valid_until = chrono::DateTime::from_timestamp_micros(deadline.expires_micros())
            .ok_or_else(unavailable)?;
        let policy = LocalPolicySnapshot {
            event_revision: event_revision.get(),
            pair_revision: pair_revision.get(),
            valid_until,
            phase,
        };
        Ok(Arc::new(
            local::prepare(self.pool.clone(), tx, deadline, policy, avatar, subject).await?,
        ))
    }
}
/// Closed C03 schema including the canonical 1.0/1.1 config union. No network refs.
/// Actual local Profile/Feed visibility: normalized pair, both admissions and
/// both current participants. revision is the pair revision observed.
pub(crate) async fn local_pair_and_members_in(
    c: &mut sqlx::PgConnection,
    event: uuid::Uuid,
    actor: uuid::Uuid,
    subject: uuid::Uuid,
) -> Result<i64, Status> {
    // A missing pair is reserved transactionally exactly as the owning
    // policy does; terminal ROLLBACK removes this metadata-only reservation.
    // This closes concurrent first-block inserts as well as block updates.
    let (low, high) = if actor <= subject {
        (actor, subject)
    } else {
        (subject, actor)
    };
    sqlx::query("insert into public.concert_relationship_pairs(event_id,low_user,high_user) values($1,$2,$3) on conflict do nothing")
        .bind(event).bind(low).bind(high).execute(&mut *c).await.map_err(dependency_error)?;
    let pair=sqlx::query("select revision,low_blocks_high,high_blocks_low from public.concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3 for share")
        .bind(event).bind(low).bind(high).fetch_optional(&mut *c).await.map_err(dependency_error)?.ok_or_else(unavailable)?;
    let pair_revision: i64 = pair.try_get("revision").map_err(dependency_error)?;
    pair_visibility(
        pair.try_get::<bool, _>("low_blocks_high")
            .map_err(dependency_error)?,
        pair.try_get::<bool, _>("high_blocks_low")
            .map_err(dependency_error)?,
    )?;
    // Sorted local order: event, normalized pair, admission, participant, avatar.
    let mut users = vec![actor, subject];
    users.sort();
    users.dedup();
    for user in users {
        let state:Option<String>=sqlx::query_scalar("select state from public.concert_event_admissions where event_id=$1 and user_id=$2 for share").bind(event).bind(user).fetch_optional(&mut *c).await.map_err(dependency_error)?;
        if state.as_deref() != Some("ACTIVE") {
            return Err(unavailable());
        }
        let exists:Option<uuid::Uuid>=sqlx::query_scalar("select user_id from public.concert_participants where event_id=$1 and user_id=$2 for share").bind(event).bind(user).fetch_optional(&mut *c).await.map_err(dependency_error)?;
        if exists != Some(user) {
            return Err(unavailable());
        }
    }
    Ok(pair_revision)
}

#[cfg(test)]
pub(crate) async fn probe_visible_in(
    c: &mut sqlx::PgConnection,
    event: uuid::Uuid,
    actor: uuid::Uuid,
    subject: uuid::Uuid,
) -> bool {
    let deadline = platform::actor_delegation::authority_deadline::AuthorityRpcDeadline::new(
        chrono::Utc::now().timestamp_micros() + 2_000_000,
        std::time::Duration::from_secs(2),
    );
    let _ = sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,false)").execute(&mut *c).await;
    match deadline {
        Ok(_) => local_pair_and_members_in(c, event, actor, subject).await.is_ok(),
        Err(_) => false,
    }
}

fn dependency_error_status() -> Status {
    Status::unavailable("Current public Profile unavailable")
}

fn pair_visibility(low_blocks_high: bool, high_blocks_low: bool) -> Result<(), Status> {
    if low_blocks_high || high_blocks_low {
        Err(Status::not_found("Public Profile unavailable"))
    } else {
        Ok(())
    }
}

pub fn validate(value: &Value) -> Result<(), Status> {
    static VALIDATOR: OnceLock<Result<jsonschema::Validator, ()>> = OnceLock::new();
    let validator=VALIDATOR.get_or_init(||{
        let source:Value=serde_json::from_str(include_str!("../../../../../../../contracts/generated/mcr-domain.openapi.json")).map_err(|_|())?;
        let schema=json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":"#/components/schemas/PublicProfile","components":source["components"]});
        jsonschema::options().should_validate_formats(true).build(&schema).map_err(|_|())
    }).as_ref().map_err(|_|unavailable())?;
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(unavailable())
    }
}
fn dependency_error(error: impl std::fmt::Debug) -> Status {
    // Native qualification needs the original SQL/type failure, while product
    // callers retain the same closed dependency error and no internal detail.
    #[cfg(test)]
    eprintln!("Public Profile local dependency failure: {error:?}");
    #[cfg(not(test))]
    let _ = error;
    unavailable()
}
fn unavailable() -> Status {
    Status::unavailable("Current public Profile unavailable")
}

fn feed_error(status: Status) -> concert_domain::feed::FeedError {
    match status.code() {
        // Only the dedicated owner's explicit absence/privacy result is skippable.
        // Guard/caller PermissionDenied also covers sealed or rebound scopes.
        tonic::Code::NotFound => concert_domain::feed::FeedError::ResourceUnavailable,
        _ => concert_domain::feed::FeedError::DependencyUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::{feed_error, pair_visibility, unavailable};
    use concert_domain::feed::FeedError;
    use tonic::{Code, Status};

    #[test]
    fn blocked_pair_is_skippable_while_an_owner_outage_stays_closed() {
        for (low_blocks_high, high_blocks_low) in [(true, false), (false, true)] {
            let denied = pair_visibility(low_blocks_high, high_blocks_low).unwrap_err();
            assert_eq!(denied.code(), Code::NotFound);
            assert_eq!(feed_error(denied), FeedError::ResourceUnavailable);
        }
        assert!(pair_visibility(false, false).is_ok());
        assert_eq!(feed_error(unavailable()), FeedError::DependencyUnavailable);
    }

    #[test]
    fn guard_or_transport_failure_cannot_become_a_skippable_author() {
        for code in [
            Code::PermissionDenied,
            Code::Unauthenticated,
            Code::DeadlineExceeded,
            Code::Cancelled,
            Code::Unavailable,
            Code::FailedPrecondition,
        ] {
            assert!(matches!(
                feed_error(Status::new(code, "authority failure")),
                FeedError::DependencyUnavailable
            ));
        }
        assert!(matches!(
            feed_error(Status::not_found("Public Profile unavailable")),
            FeedError::ResourceUnavailable
        ));
    }
}

#[cfg(test)]
mod native;
