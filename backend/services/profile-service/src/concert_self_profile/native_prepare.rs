//! Actual owner Prepare and expired-scope quarantine. Only the actual Concert journal
//! can supply a terminal decision; this component deliberately supplies none.
use super::{
    fingerprint::effect_digest,
    guard::binding_digest,
    model::Version,
    native,
    native_rpc::{required, signed, tls},
    wire::{lease, proto},
};
use platform::{
    actor_delegation::{
        concert_self_profile::ConcertSelfProfileSigner, relationship_read::RelationshipReadSigner,
        ActorDelegationClient, ActorDelegationVerifier,
    },
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};
use tonic::{service::interceptor::InterceptedService, transport::Channel};
use uuid::Uuid;

type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
type Profile =
    proto::concert_self_profile_service_client::ConcertSelfProfileServiceClient<Transport>;
const RAW: &[u8] = br#"{"expected_revision":1,"display_name":"Prepared native SELF","bio":null}"#;

async fn profile() -> anyhow::Result<Profile> {
    let gateway = ActorDelegationVerifier::from_env(false, Workload::Profile)?;
    let signer = ConcertSelfProfileSigner::from_env(&gateway)?;
    let channel = tls(Workload::Concert)?
        .endpoint(
            &required("MCR_SELF_PROFILE_PROFILE_ENDPOINT")?,
            Workload::Profile,
        )?
        .connect()
        .await?;
    Ok(Profile::with_interceptor(
        signer.client(channel),
        ServiceIdentityClientInterceptor::new(
            Workload::Concert,
            &required("CONCERT_SERVICE_CREDENTIAL")?,
        )?,
    ))
}

async fn acquire(actor: Uuid) -> anyhow::Result<proto::SelfScope> {
    let family = Uuid::parse_str(&required("MCR_SELF_PROFILE_FAMILY")?)?;
    let event = Uuid::parse_str(&required("MCR_SELF_PROFILE_EVENT")?)?;
    let gateway = ActorDelegationVerifier::from_env(false, Workload::Profile)?;
    let signer = RelationshipReadSigner::from_env(&gateway)?;
    let tls = tls(Workload::Concert)?;
    let token = required("CONCERT_SERVICE_CREDENTIAL")?;
    let binding = lease::GuardBinding {
        actor_id: actor.to_string(),
        event_id: event.to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        operation: "updateConcertParticipant".into(),
        subject_ids: Vec::new(),
        expires_unix_micros: chrono::Utc::now().timestamp_micros() + 4_500_000,
        // A trusted fixture root envelope, never a claimed Gateway admission.
        parent_digest: Sha256::digest(b"native Prepare fixture root").to_vec(),
        parent_body_digest: Sha256::digest(b"native Prepare fixture framed parent").to_vec(),
        parent_method: "/concert.participant.v1.ConcertParticipantService/UpdatePreferences".into(),
        http_method: "PATCH".into(),
        http_target: format!("/api/concert/events/{event}/me"),
        body_digest: Sha256::digest(RAW).to_vec(),
    };
    let request = lease::AcquireRequest {
        response_scope: Some(lease::ProducerScope {
            instance_id: Uuid::new_v4().to_string(),
            scope_id: Uuid::new_v4().to_string(),
            binding_digest: binding_digest(&binding)?.to_vec(),
        }),
        binding: Some(binding.clone()),
    };
    let channel = tls
        .endpoint(&required("MCR_SELF_PROFILE_AUTH_ENDPOINT")?, Workload::Auth)?
        .connect()
        .await?;
    let mut auth = lease::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient::with_interceptor(
        signer.client(channel, Workload::Auth)?, ServiceIdentityClientInterceptor::new(Workload::Concert, &token)?);
    let channel = tls
        .endpoint(
            &required("MCR_SELF_PROFILE_FRIEND_ENDPOINT")?,
            Workload::Friend,
        )?
        .connect()
        .await?;
    let mut friend = lease::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient::with_interceptor(
        signer.client(channel, Workload::Friend)?, ServiceIdentityClientInterceptor::new(Workload::Concert, &token)?);
    let a = auth
        .acquire(signed(request.clone(), actor)?)
        .await?
        .into_inner();
    let f = friend.acquire(signed(request, actor)?).await?.into_inner();
    anyhow::ensure!(
        a.session_live && a.view.is_some() && f.view.is_some(),
        "actual owner views required"
    );
    Ok(proto::SelfScope {
        binding: Some(binding),
        auth_view: a.view,
        friend_view: f.view,
    })
}

async fn unchanged(pool: &PgPool, actor: Uuid, prepared: i64) -> anyhow::Result<()> {
    let row: (String, String, String) =
        sqlx::query_as("select nickname,bio,avatar_url from profiles where user_id=$1")
            .bind(actor)
            .fetch_one(pool)
            .await?;
    assert_eq!(
        row,
        (
            "Actual native SELF".into(),
            "Native bio".into(),
            "retained-avatar".into()
        )
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_self_profile_receipts")
            .fetch_one(pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_self_profile_operations")
            .fetch_one(pool)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from pg_prepared_xacts where database=current_database()"
        )
        .fetch_one(pool)
        .await?,
        prepared
    );
    Ok(())
}

fn save(path: PathBuf, query: &proto::SelfOperationRequest) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&query.encode_to_vec())?;
    file.sync_all()?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires real Auth/Friend/Profile MAIN mTLS processes and private PG16.14"]
async fn actual_owner_prepare_and_recovery_quarantine() -> anyhow::Result<()> {
    let pool = native::pool().await?;
    let actor = Uuid::parse_str(&required("MCR_SELF_PROFILE_ACTOR")?)?;
    let root = PathBuf::from(required("MCR_SELF_PROFILE_ROOT")?);
    let path = root.join("prepared-self-recovery.bin");
    let mut profile = profile().await?;
    match required("MCR_SELF_PROFILE_PREPARE_PHASE")?.as_str() {
        phase @ ("prepare" | "conflict") => {
            let scope = acquire(actor).await?;
            let binding = scope.binding.as_ref().unwrap();
            let current = profile
                .read(signed(
                    proto::ReadSelfRequest {
                        scope: Some(scope.clone()),
                        subject_id: actor.to_string(),
                    },
                    actor,
                )?)
                .await?
                .into_inner()
                .content
                .unwrap();
            let mut version = current.version.unwrap();
            if phase == "conflict" {
                // A valid but stale internal CAS token. The actual registered
                // transaction must abort; the RPC error alone proves nothing.
                version.revision += 1;
            }
            let query = proto::PrepareSelfUpdateRequest {
                scope: Some(scope.clone()),
                subject_id: actor.to_string(),
                operation_id: binding.request_id.clone(),
                expected_version: Some(version.clone()),
                raw_body: RAW.to_vec(),
            };
            let mut altered = query.clone();
            altered.raw_body.push(b' ');
            assert_eq!(
                profile
                    .prepare_update(signed(altered, actor)?)
                    .await
                    .unwrap_err()
                    .code(),
                tonic::Code::PermissionDenied
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "select count(*) from concert_self_profile_operations"
                )
                .fetch_one(&pool)
                .await?,
                0
            );
            let result = profile.prepare_update(signed(query.clone(), actor)?).await;
            if phase == "conflict" {
                assert_eq!(result.unwrap_err().code(), tonic::Code::Aborted);
                let recovery = proto::SelfOperationRequest {
                    operation_id: binding.request_id.clone(),
                    scope: Some(scope),
                };
                let stop = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
                loop {
                    let observed = profile
                        .read_operation(signed(recovery.clone(), actor)?)
                        .await?
                        .into_inner();
                    if observed.status == proto::SelfOperationStatus::Aborted as i32 {
                        assert!(observed.transaction.is_some());
                        assert_eq!(observed.effect_binding_digest.len(), 32);
                        break;
                    }
                    anyhow::ensure!(
                        tokio::time::Instant::now() < stop,
                        "actual CAS transaction did not abort"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                unchanged(&pool, actor, 0).await?;
                save(path, &recovery)?;
                pool.close().await;
                return Ok(());
            }
            let prepared = result?;
            assert_eq!(
                prepared.metadata().get("cache-control").unwrap(),
                "no-store"
            );
            let prepared = prepared.into_inner();
            assert_eq!(prepared.operation_id, binding.request_id);
            let operation = Uuid::parse_str(&binding.request_id)?;
            assert_eq!(
                prepared.prepared_gid,
                format!("mcr_sp_{}", operation.simple())
            );
            assert_eq!(
                prepared.effect_binding_digest,
                effect_digest(
                    binding_digest(binding)?,
                    operation,
                    Version::new(Uuid::parse_str(&version.epoch)?, version.revision)?,
                    RAW
                )?
                .to_vec()
            );
            let content = prepared.proposed_content.unwrap();
            assert_eq!(content.display_name, "Prepared native SELF");
            assert_eq!(content.bio, None);
            assert_eq!(content.version.unwrap().revision, version.revision + 1);
            let transaction = prepared.transaction.unwrap();
            let stored: (String, String, i64, i32) = sqlx::query_as("select full_xid,system_identifier,database_oid,backend_pid from concert_self_profile_operations where operation_id=$1")
                .bind(operation).fetch_one(&pool).await?;
            assert_eq!(
                (
                    transaction.full_xid,
                    transaction.system_identifier,
                    i64::from(transaction.database_oid),
                    transaction.backend_pid
                ),
                stored
            );
            assert_eq!(
                profile
                    .prepare_update(signed(query, actor)?)
                    .await
                    .unwrap_err()
                    .code(),
                tonic::Code::Aborted
            );
            unchanged(&pool, actor, 1).await?;
            let recovery = proto::SelfOperationRequest {
                scope: Some(scope),
                operation_id: operation.to_string(),
            };
            save(path, &recovery)?;
        }
        phase @ ("recover" | "recover-conflict") => {
            let info = std::fs::symlink_metadata(&path)?;
            anyhow::ensure!(
                info.is_file()
                    && !info.file_type().is_symlink()
                    && info.mode() & 0o077 == 0
                    && info.uid() == std::fs::metadata(&root)?.uid(),
                "owned private recovery query required"
            );
            let query = proto::SelfOperationRequest::decode(std::fs::read(path)?.as_slice())?;
            let binding = query.scope.as_ref().unwrap().binding.as_ref().unwrap();
            anyhow::ensure!(
                binding.actor_id == actor.to_string()
                    && binding.expires_unix_micros < chrono::Utc::now().timestamp_micros(),
                "original scope must be expired"
            );
            let status = profile
                .read_operation(signed(query.clone(), actor)?)
                .await?;
            assert_eq!(status.metadata().get("cache-control").unwrap(), "no-store");
            let expected = if phase == "recover-conflict" {
                proto::SelfOperationStatus::Aborted
            } else {
                proto::SelfOperationStatus::Prepared
            };
            assert_eq!(status.into_inner().status, expected as i32);
            let mut altered = query.clone();
            altered
                .scope
                .as_mut()
                .unwrap()
                .binding
                .as_mut()
                .unwrap()
                .expires_unix_micros += 1;
            assert_eq!(
                profile
                    .read_operation(signed(altered, actor)?)
                    .await
                    .unwrap_err()
                    .code(),
                tonic::Code::PermissionDenied
            );
            // A fresh signed recovery query may inspect only the registered
            // effect. A missing decision endpoint cannot choose rollback.
            let result = profile.reconcile(signed(query.clone(), actor)?).await;
            if phase == "recover-conflict" {
                assert_eq!(result?.into_inner().status, expected as i32);
            } else {
                assert_eq!(result.unwrap_err().code(), tonic::Code::Unavailable);
            }
            assert_eq!(
                profile
                    .read_operation(signed(query, actor)?)
                    .await?
                    .into_inner()
                    .status,
                expected as i32
            );
            unchanged(
                &pool,
                actor,
                if phase == "recover-conflict" { 0 } else { 1 },
            )
            .await?;
        }
        _ => anyhow::bail!("explicit prepare/recover phase required"),
    }
    pool.close().await;
    Ok(())
}
