//! Canonical Auth source provenance, not a downstream deletion acknowledgement.
//! No SQL mutation, actor delegation, source cache or portable hash authority.
use chrono::{DateTime, Utc};
use platform::{
    grpc_auth::{CallerRegistry, VerifiedCaller, Workload},
    grpc_method_policy::{guard_without_delegation, GuardedService, MethodPolicy},
    grpc_tls::TlsMaterial,
};
use prost::Message;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tonic::{
    body::Body,
    codegen::{http, Service},
    server::NamedService,
    Request, Response, Status,
};
use uuid::Uuid;
pub mod proto {
    tonic::include_proto!("authority.deletion.v1");
}
pub const MAX_OPERATION: Duration = Duration::from_secs(5);
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "authority.deletion.v1.AuthDeletionSourceService",
    workload: Some(Workload::Auth),
    methods: &[("VerifySource", &[Workload::Concert])],
    actor_fields: &[],
};
pub struct SourceServer {
    pool: PgPool,
}
type Generated =
    proto::auth_deletion_source_service_server::AuthDeletionSourceServiceServer<SourceServer>;
pub fn guarded_server(
    pool: PgPool,
    registry: CallerRegistry,
) -> anyhow::Result<Deadline<GuardedService<Generated>>> {
    Ok(Deadline(guard_without_delegation(
        &POLICY,
        Generated::new(SourceServer { pool })
            .max_decoding_message_size(2048)
            .max_encoding_message_size(2048),
        registry,
    )?))
}
/// Explicit opt-in; missing transport/credentials cannot create a local positive.
pub fn configured_server(
    pool: PgPool,
    tls: Option<&TlsMaterial>,
) -> anyhow::Result<Option<Deadline<GuardedService<Generated>>>> {
    match std::env::var("AUTH_CONCERT_DELETION_SOURCE_ENABLED").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("false") => return Ok(None),
        Ok("true") => (),
        _ => anyhow::bail!("AUTH_CONCERT_DELETION_SOURCE_ENABLED must be true or false"),
    }
    anyhow::ensure!(tls.is_some(), "Auth deletion source requires internal mTLS");
    let registry =
        platform::config::build_caller_registry(true, &[Workload::Concert], &[Workload::Concert])?;
    Ok(Some(guarded_server(pool, registry)?))
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid deletion source request")
}
fn denied() -> Status {
    Status::permission_denied("Canonical deletion source unavailable")
}
fn unavailable() -> Status {
    Status::unavailable("Auth deletion source unavailable")
}
fn expired() -> Status {
    Status::deadline_exceeded("Deletion source deadline expired")
}
fn id(s: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(s)
        .ok()
        .filter(|u| !u.is_nil() && u.to_string() == s)
        .ok_or_else(invalid)
}
fn target(value: i32) -> Result<&'static str, Status> {
    match value {
        1 => Ok("CONCERT_TICKETS"),
        2 => Ok("CONCERT_SESSIONS"),
        3 => Ok("CONCERT_SHARING"),
        4 => Ok("CONCERT_PRIVATE_DATA"),
        _ => Err(invalid()),
    }
}
/// Structural validation is public for producer/consumer contract qualification;
/// it never checks a store or returns verified authority.
pub fn validate_command(c: &proto::DeletionCommand) -> Result<(), Status> {
    id(&c.saga_id)?;
    id(&c.user_id)?;
    id(&c.actor_id)?;
    target(c.target)?;
    if !(0..1_000_000_000).contains(&c.retention_nanos)
        || DateTime::<Utc>::from_timestamp(c.retention_seconds, c.retention_nanos as u32).is_none()
    {
        return Err(invalid());
    }
    Ok(())
}
#[derive(Clone)]
pub struct Deadline<S>(S);
impl<S: NamedService> NamedService for Deadline<S> {
    const NAME: &'static str = S::NAME;
}
impl<S, B> Service<http::Request<Body>> for Deadline<S>
where
    S: Service<http::Request<Body>, Response = http::Response<B>, Error = Infallible>,
    S::Future: Send + 'static,
    B: tonic::codegen::Body<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<tonic::codegen::StdError>,
{
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }
    fn call(&mut self, q: http::Request<Body>) -> Self::Future {
        let now = Instant::now();
        let Some(left) = platform::call_deadline::received_timeout(q.headers())
            .filter(|d| !d.is_zero() && *d <= MAX_OPERATION)
        else {
            return Box::pin(async { Ok(expired().into_http()) });
        };
        let stop = platform::call_deadline::current().map_or(now + left, |p| p.min(now + left));
        let f = self.0.call(q);
        Box::pin(platform::call_deadline::scope(stop, async move {
            let mut r = match tokio::time::timeout_at(stop.into(), f).await {
                Ok(v) => v?.map(Body::new),
                Err(_) => expired().into_http(),
            };
            r.headers_mut().insert(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static("no-store"),
            );
            Ok(r)
        }))
    }
}
#[tonic::async_trait]
impl proto::auth_deletion_source_service_server::AuthDeletionSourceService for SourceServer {
    async fn verify_source(
        &self,
        q: Request<proto::VerifyDeletionSourceRequest>,
    ) -> Result<Response<proto::VerifyDeletionSourceResponse>, Status> {
        // Require the actual CA-validated peer certificate, including for direct
        // handler invocations. The registry independently binds its SAN to Concert.
        if !matches!(
            q.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Concert))
        ) || q.peer_certs().map_or(true, |c| c.is_empty())
        {
            return Err(Status::unauthenticated(
                "Verified mTLS Concert workload required",
            ));
        }
        let stop = platform::call_deadline::current().ok_or_else(expired)?;
        let q = q.into_inner();
        let c = q.command.as_ref().ok_or_else(invalid)?;
        validate_command(c)?;
        if q.request_nonce.len() != 16 || q.request_nonce.iter().all(|b| *b == 0) {
            return Err(invalid());
        }
        let left = q
            .expires_unix_micros
            .checked_sub(Utc::now().timestamp_micros())
            .ok_or_else(expired)?;
        if left <= 0 {
            return Err(expired());
        }
        if left > 5_000_000 {
            return Err(invalid());
        }
        let stop = stop.min(Instant::now() + Duration::from_micros(left as u64));
        tokio::time::timeout_at(stop.into(),async {
        let mut tx=self.pool.begin().await.map_err(|_|unavailable())?;
        sqlx::query("set transaction isolation level repeatable read").execute(&mut *tx).await.map_err(|_|unavailable())?;
        let millis=stop.saturating_duration_since(Instant::now()).as_millis();
        if millis==0 {return Err(expired());}
        sqlx::query("select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true),set_config('idle_in_transaction_session_timeout',$1,true)")
            .bind(format!("{millis}ms")).execute(&mut *tx).await.map_err(|_|unavailable())?;
        let retention=DateTime::<Utc>::from_timestamp(c.retention_seconds,c.retention_nanos as u32).ok_or_else(invalid)?;
        let row=sqlx::query(r#"
select g.id,g.user_id,g.actor_id,g.retention_until,g.created_at,s.target,a.id audit_id,a.metadata_json audit_metadata
from public.user_deletion_sagas g
join public.users u on u.id=g.user_id
join public.user_deletion_steps s on s.saga_id=g.id and s.target=$4
join lateral (
 select a.id,a.metadata_json from public.auth_audit_events a
 where a.subject_user_id=g.user_id and a.actor_id=g.actor_id
 and a.action='USER_DELETION_REQUESTED' and a.outcome='SUCCESS'
 and a.occurred_at=g.created_at
 and a.metadata_json::jsonb->>'sagaId'=g.id::text
 order by a.id limit 1
) a on true
where g.id=$1 and g.user_id=$2 and g.actor_id=$3 and g.retention_until=$5
and g.status in ('IN_PROGRESS','COMPLETED','DEAD_LETTERED')
and s.status in ('PENDING','IN_PROGRESS','RETRY','SUCCEEDED','DEAD_LETTERED')
and u.status='DELETED' and u.deleted_at is not null
and u.deleted_by=g.actor_id and u.retention_until=g.retention_until and u.deleted_at=g.created_at
and not exists(select 1 from public.refresh_tokens r where r.user_id=u.id and r.revoked_at is null)
and not exists(select 1 from public.user_identities i where i.user_id=u.id and i.deleted_at is null)
and exists(select 1 from public.auth_concert_user_security_generation x where x.user_id=u.id and x.generation>0)
for share of g,u,s
"#).bind(id(&c.saga_id)?).bind(id(&c.user_id)?).bind(id(&c.actor_id)?)
            .bind(target(c.target)?).bind(retention).fetch_optional(&mut *tx).await.map_err(|_|unavailable())?.ok_or_else(denied)?;
        let stored:DateTime<Utc>=row.try_get("retention_until").map_err(|_|unavailable())?;
        // SQLx writes Chrono timestamps at PostgreSQL microsecond precision.
        // Parse the original JSON timestamp in Rust: casting its nanosecond
        // text to timestamptz would round instead of preserving SQLx truncation.
        let metadata:String=row.try_get("audit_metadata").map_err(|_|unavailable())?;
        let metadata:serde_json::Value=serde_json::from_str(&metadata).map_err(|_|denied())?;
        let original=metadata.get("retentionUntil").and_then(|v|v.as_str())
            .and_then(|s|DateTime::parse_from_rfc3339(s).ok()).ok_or_else(denied)?;
        if original.timestamp_micros()!=stored.timestamp_micros() {return Err(denied());}
        let canonical=proto::DeletionCommand {saga_id:row.try_get::<Uuid,_>("id").map_err(|_|unavailable())?.to_string(),
            user_id:row.try_get::<Uuid,_>("user_id").map_err(|_|unavailable())?.to_string(),
            actor_id:row.try_get::<Uuid,_>("actor_id").map_err(|_|unavailable())?.to_string(),
            target:c.target,retention_seconds:stored.timestamp(),retention_nanos:stored.timestamp_subsec_nanos() as i32};
        if canonical!=*c {return Err(denied());}
        let created:DateTime<Utc>=row.try_get("created_at").map_err(|_|unavailable())?;
        let audit:Uuid=row.try_get("audit_id").map_err(|_|unavailable())?;
        let mut h=Sha256::new();h.update(b"MCR-AUTH-DELETION-SOURCE-1\0");h.update(canonical.encode_to_vec());h.update(audit.as_bytes());
        h.update(created.timestamp().to_be_bytes());h.update(created.timestamp_subsec_nanos().to_be_bytes());
        tx.commit().await.map_err(|_|unavailable())?;
        if Instant::now()>=stop || Utc::now().timestamp_micros()>=q.expires_unix_micros {return Err(expired());}
        let mut r=Response::new(proto::VerifyDeletionSourceResponse {command:Some(canonical),source_binding:h.finalize().to_vec(),
            request_nonce:q.request_nonce,expires_unix_micros:q.expires_unix_micros});
        r.metadata_mut().insert("cache-control","no-store".parse().expect("static metadata"));Ok(r)
    }).await.map_err(|_|expired())?
    }
}
