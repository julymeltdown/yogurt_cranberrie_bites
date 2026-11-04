//! Original BE27 product facade. Root alone acquires and settles owner scopes.
mod authority;
mod cursor;
mod profile;
pub mod protocol;
mod witness;
use crate::{
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    adapters::out::{
        public_profile::PublicProfileReader,
        repository::intro::{owner_dto, public_dto, public_page_dto, PgIntro},
    },
    product_rpc::{
        proto,
        scoped::{ProductRepresentation, ScopedProductProducer},
        VerifiedProduct,
    },
};
use authority::{scope_context, ScopedIntroExecutor, ScopedIntroPolicy};
use chrono::Utc;
use concert_application::{commands::FingerprintKeys, intro};
pub use cursor::IntroCursor;
use prost::Message;
use protocol::{parse_intro_command, parse_intro_target, parse_query};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;
pub use witness::IntroNativeResolver;
/// Root supplies immutable configuration and BE26's actual AgeRuntime, which
/// constructs ScopedProjectionAuthority on this exact scope. Unavailable status
/// must build an unavailable-status projection, not disable unrelated Intro work.
#[derive(Clone)]
pub struct IntroRuntime {
    pool: PgPool,
    profiles: Arc<PublicProfileReader>,
    cursor: IntroCursor,
    keys: Arc<FingerprintKeys>,
    age: Arc<crate::adapters::out::age_provider::runtime::AgeRuntime>,
}
impl IntroRuntime {
    pub fn new(
        pool: PgPool,
        profiles: Arc<PublicProfileReader>,
        cursor: IntroCursor,
        keys: Arc<FingerprintKeys>,
        age: Arc<crate::adapters::out::age_provider::runtime::AgeRuntime>,
    ) -> Self {
        Self {
            pool,
            profiles,
            cursor,
            keys,
            age,
        }
    }
}
impl IntroRuntime {
    /// Accept only Root's acquired scope in the original sealed task context.
    /// This method neither acquires a scope nor acknowledges response delivery.
    pub async fn dispatch_for_scope(
        &self,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let parent = platform::actor_delegation::exchange::VerifiedParentAuthority::current()
            .ok_or_else(|| Status::permission_denied("Original Intro parent required"))?;
        let original = proto::ProductRequest::decode(
            parent
                .message()
                .map_err(|_| Status::permission_denied("Invalid Intro parent"))?,
        )
        .map_err(|_| Status::permission_denied("Invalid Intro parent"))?;
        let binding = scope.binding();
        binding.check()?;
        binding
            .owner_binding()
            .matches_parent(&parent)
            .map_err(|_| Status::permission_denied("Intro parent mismatch"))?;
        if original != request
            || parent.parent_digest() != binding.parent_digest()
            || parent.actor() != binding.actor()
            || request.user_id != binding.actor().to_string()
            || request.event_id != binding.event().to_string()
            || request.session_family_id != binding.session_family().to_string()
            || request.request_id != binding.request_id().to_string()
            || request.http_method != binding.http_method()
            || request.http_target != binding.http_target()
            || binding.body_digest() != <[u8; 32]>::from(Sha256::digest(&request.http_body))
            || request.authority_expires_unix_micros < binding.deadline().expires_micros()
        {
            return Err(Status::permission_denied("Intro scope binding mismatch"));
        }
        let (operation, intro_id) =
            parse_intro_target(binding.event(), &request.http_method, &request.http_target)
                .map_err(|_| Status::invalid_argument("Invalid Intro target"))?;
        if operation != binding.operation() {
            return Err(Status::permission_denied("Intro operation mismatch"));
        }
        let read_context = scope_context(&scope)
            .map_err(|_| Status::unavailable("Intro authority unavailable"))?;
        binding.assert_context(
            read_context.actor.uuid(),
            read_context.event.uuid(),
            read_context.session,
            read_context.request,
            parent.parent_digest(),
            read_context.expires_at,
        )?;
        let executor = ScopedIntroExecutor::new(
            self.pool.clone(),
            scope.clone(),
            self.profiles.clone(),
            self.age.projection(scope.clone()),
        )
        .map_err(|_| Status::unavailable("Intro authority unavailable"))?;
        let service = PgIntro::with_executor(
            self.pool.clone(),
            ScopedIntroPolicy::new(executor.clone()),
            executor.profiles(),
            self.cursor.clone(),
            executor,
            self.keys.clone(),
        );
        let outcome = async {
            match operation {
                "listConcertIntros" => {
                    if !request.http_body.is_empty() || !request.idempotency_key.is_empty() {
                        return Err(intro::IntroError::Invalid);
                    }
                    let (items, next) = service
                        .discover(read_context, parse_query(&request.http_target)?)
                        .await?;
                    public_page_dto(items, next)
                }
                "getOwnConcertIntro" => {
                    if !request.http_body.is_empty() || !request.idempotency_key.is_empty() {
                        return Err(intro::IntroError::Invalid);
                    }
                    owner_dto(&service.own(read_context).await?)
                }
                "getConcertIntro" => {
                    if !request.http_body.is_empty() || !request.idempotency_key.is_empty() {
                        return Err(intro::IntroError::Invalid);
                    }
                    let (value, author) = service
                        .direct(read_context, intro_id.ok_or(intro::IntroError::Invalid)?)
                        .await?;
                    public_dto(&value, author)
                }
                _ => {
                    if request.content_type != "application/json" {
                        return Err(intro::IntroError::Invalid);
                    }
                    let key = crate::product_rpc::canonical_uuid(&request.idempotency_key)
                        .map_err(|_| intro::IntroError::Invalid)?;
                    let command = parse_intro_command(
                        read_context.actor,
                        read_context.event,
                        key,
                        operation,
                        &request.http_body,
                    )?;
                    owner_dto(&service.execute(&command).await?.owner)
                }
            }
        }
        .await;
        let (status, body) = match outcome {
            Ok(body) => (200, body),
            Err(e) => intro_error_dto(e, read_context.request),
        };
        let body = serde_json::to_vec(&body)
            .map_err(|_| Status::unavailable("Intro projection unavailable"))?;
        scope.binding().check()?;
        scope.disclosure_ceiling()?;
        Ok(ProductRepresentation {
            status,
            content_type: "application/json".into(),
            body,
        })
    }
}
#[async_trait::async_trait]
impl ScopedProductProducer for IntroRuntime {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        context.check()?;
        let b = scope.binding();
        if context.operation().contract_id() != b.operation()
            || context.parent_authority().parent_digest() != b.parent_digest()
        {
            return Err(Status::permission_denied(
                "Intro verified operation mismatch",
            ));
        }
        b.assert_context(
            context.actor(),
            context.event(),
            context.session(),
            context.request(),
            context.parent_authority().parent_digest(),
            chrono::DateTime::from_timestamp_micros(b.deadline().expires_micros())
                .ok_or_else(|| Status::permission_denied("Invalid Intro deadline"))?,
        )?;
        let result = self.dispatch_for_scope(scope, request).await?;
        context.check()?;
        Ok(result)
    }
}
fn intro_error_dto(error: intro::IntroError, request: Uuid) -> (u32, Value) {
    let (status, reason) = match error {
        intro::IntroError::Invalid => (400, "INVALID_INPUT"),
        intro::IntroError::NotFound => (404, "RESOURCE_UNAVAILABLE"),
        intro::IntroError::Denied => (403, "PERMISSION_DENIED"),
        intro::IntroError::ReadOnly => (409, "EVENT_READ_ONLY"),
        intro::IntroError::Conflict => (409, "VERSION_CONFLICT"),
        intro::IntroError::Expired => (409, "IDEMPOTENCY_EXPIRED"),
        intro::IntroError::Unavailable => (503, "DEPENDENCY_UNAVAILABLE"),
    };
    (
        status,
        json!({"code":format!("CONCERT_{status}"),"message":"Concert request unavailable",
        "traceId":request.to_string(),"timestamp":Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),
        "details":{"reason":reason}}),
    )
}
