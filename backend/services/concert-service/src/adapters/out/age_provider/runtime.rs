//! Root-composable C04 age producer. Provider activation is explicit; configured
//! adapters must independently verify proofs. No public DTO can supply a proof.
use super::{
    claim_reader::PgClaimProjection,
    scoped::{ScopedAgeAuthority, ScopedProjectionAuthority},
    PgAgeClaims,
};
use crate::{
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    age_verification::{
        Error, OwnStatus, ProviderConfig, ProviderResponse, StateToken, UnavailableVerifier,
        Verifier,
    },
    product_rpc::{
        proto,
        scoped::{ProductRepresentation, ScopedProductProducer},
        VerifiedProduct,
    },
};
use chrono::{DateTime, Utc};
use concert_application::{
    age_policy::Context,
    commands::{FingerprintKeys, SafeReceipt},
    Clock,
};
use concert_domain::{age_policy::AgeStatus, AuthUserId, EventId};
use prost::Message;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

#[derive(Clone)]
pub enum ProviderMode {
    Unavailable,
    Configured {
        config: ProviderConfig,
        verifier: Arc<dyn Verifier>,
    },
}
#[derive(Clone)]
pub struct AgeRuntime {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    keys: Arc<FingerprintKeys>,
    mode: ProviderMode,
}
impl AgeRuntime {
    pub fn new(
        pool: PgPool,
        clock: Arc<dyn Clock>,
        keys: Arc<FingerprintKeys>,
        mode: ProviderMode,
    ) -> Self {
        Self {
            pool,
            clock,
            keys,
            mode,
        }
    }
    pub fn verification_available(&self) -> bool {
        match &self.mode {
            ProviderMode::Unavailable => false,
            ProviderMode::Configured { verifier, .. } => verifier.available(),
        }
    }
    /// Intro supplies its original held scope and registers its own native TX.
    pub fn projection(&self, scope: Arc<OwnerGuardScope>) -> PgClaimProjection {
        PgClaimProjection::new(
            self.pool.clone(),
            Arc::new(ScopedProjectionAuthority::new(
                scope,
                self.verification_available(),
            )),
            self.clock.clone(),
        )
    }
    pub fn claims(&self, scope: Arc<OwnerGuardScope>) -> PgAgeClaims {
        let (config, verifier): (_, Arc<dyn Verifier>) = match &self.mode {
            ProviderMode::Unavailable => (None, Arc::new(UnavailableVerifier)),
            ProviderMode::Configured { config, verifier } => {
                (Some(config.clone()), verifier.clone())
            }
        };
        // Missing provider never removes canonical own-status read authority.
        PgAgeClaims::new(
            self.pool.clone(),
            Arc::new(ScopedAgeAuthority::new(scope)),
            verifier,
            config,
            self.clock.clone(),
            self.keys.clone(),
        )
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartBody {
    return_target: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallbackBody {
    verification_id: Uuid,
    state_token: String,
    provider_response_token: String,
}
fn uuid(v: &str) -> Result<Uuid, Error> {
    let id = Uuid::parse_str(v).map_err(|_| Error::Invalid)?;
    if id.is_nil() || id.to_string() != v {
        return Err(Error::Invalid);
    }
    Ok(id)
}
fn json(status: u32, body: serde_json::Value) -> ProductRepresentation {
    ProductRepresentation {
        status,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&body).expect("fixed safe DTO"),
    }
}
fn status(value: OwnStatus) -> ProductRepresentation {
    let name = match value.status {
        AgeStatus::Unknown => "UNKNOWN",
        AgeStatus::Verified => "VERIFIED",
        AgeStatus::Expired => "EXPIRED",
        AgeStatus::Unsupported => "UNSUPPORTED",
        AgeStatus::ProviderUnavailable => "PROVIDER_UNAVAILABLE",
        AgeStatus::Revoked => "REVOKED",
    };
    json(
        200,
        serde_json::json!({"user_id":value.user_id,"event_id":value.event_id,"status":name,
        "verified_at":value.verified_at,"expires_at":value.expires_at,"verification_available":value.verification_available}),
    )
}
fn receipt(r: SafeReceipt) -> ProductRepresentation {
    json(
        202,
        serde_json::json!({"receipt_id":r.id,"event_id":r.scope.event,"actor_id":r.scope.actor,"operation_id":r.scope.operation.id(),
        "idempotency_key":r.scope.key,"state":r.state.as_str(),"resource_id":r.reference.map(|v|v.resource),
        "resource_revision":r.reference.map(|v|v.revision),"failure_reason":r.failure.map(|v|v.as_str()),
        "created_at":r.created_at,"expires_at":r.expires_at,"result_status":r.result_status}),
    )
}
fn error(e: Error, request: Uuid) -> ProductRepresentation {
    let (code, reason) = match e {
        Error::Invalid => (400, "INVALID_INPUT"),
        Error::Denied => (403, "PERMISSION_DENIED"),
        Error::ProviderUnavailable => (503, "PROVIDER_UNAVAILABLE"),
        Error::Conflict => (409, "IDEMPOTENCY_CONFLICT"),
        Error::Expired => (409, "IDEMPOTENCY_EXPIRED"),
        Error::Pending | Error::Unavailable => (503, "DEPENDENCY_UNAVAILABLE"),
    };
    json(
        code,
        serde_json::json!({"code":format!("CONCERT_{code}"),"message":"Age verification request unavailable","traceId":request,
        "timestamp":Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),"details":{"reason":reason}}),
    )
}
#[async_trait::async_trait]
impl ScopedProductProducer for AgeRuntime {
    async fn dispatch(
        &self,
        verified: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        verified.check()?;
        let original = proto::ProductRequest::decode(
            verified
                .parent_authority()
                .message()
                .map_err(|_| Status::permission_denied("Invalid sealed age frame"))?,
        )
        .map_err(|_| Status::permission_denied("Invalid sealed age message"))?;
        if original != request {
            return Err(Status::permission_denied("Age original request mismatch"));
        }
        let b = scope.binding();
        b.check()?;
        let expires_at = DateTime::from_timestamp_micros(b.deadline().expires_micros())
            .ok_or_else(|| Status::invalid_argument("Invalid age deadline"))?;
        b.assert_context(
            verified.actor(),
            verified.event(),
            verified.session(),
            verified.request(),
            verified.parent_authority().parent_digest(),
            expires_at,
        )?;
        if b.operation() != verified.operation().contract_id()
            || request.user_id != verified.actor().to_string()
            || request.event_id != verified.event().to_string()
            || request.session_family_id != verified.session().to_string()
            || request.request_id != verified.request().to_string()
            || b.http_method() != request.http_method
            || b.http_target() != request.http_target
            || b.body_digest() != <[u8; 32]>::from(Sha256::digest(&request.http_body))
        {
            return Err(Status::permission_denied("Age scope binding mismatch"));
        }
        let c = Context {
            actor: AuthUserId::from_uuid(b.actor()),
            event: EventId::from_uuid(b.event()),
            session: b.session_family(),
            request: b.request_id(),
            expires_at,
        };
        let base = format!("/api/concert/events/{}/me", c.event.uuid());
        let operation = b.operation().to_owned();
        let claims = self.claims(scope);
        let outcome: Result<ProductRepresentation,Error> = async {
            if request.http_body.len()>8192 { return Err(Error::Invalid); }
            match operation.as_str() {
                "getConcertAgeStatus" if request.http_method=="GET" && request.http_target==format!("{base}/age-status") && request.http_body.is_empty() => Ok(status(claims.status(c).await?)),
                "startConcertAgeVerification" if request.http_method=="POST" && request.http_target==format!("{base}/age-verifications") => {
                    if request.content_type!="application/json" { return Err(Error::Invalid); }
                    let body:StartBody=serde_json::from_slice(&request.http_body).map_err(|_|Error::Invalid)?;
                    let result=claims.start(c,uuid(&request.idempotency_key)?,&body.return_target).await?;
                    match result.authorization {
                        Some(a)=>Ok(json(200,serde_json::json!({"verification_id":a.verification_id,"state":"PENDING","expires_at":a.expires_at,"authorization_url":a.authorization_url}))),
                        None=>Ok(receipt(result.receipt)),
                    }
                },
                "completeConcertAgeVerification" if request.http_method=="POST" && request.http_target==format!("{base}/age-verifications/callback") => {
                    if request.content_type!="application/json" { return Err(Error::Invalid); }
                    let body:CallbackBody=serde_json::from_slice(&request.http_body).map_err(|_|Error::Invalid)?;
                    let result=claims.callback(c,uuid(&request.idempotency_key)?,body.verification_id,
                        StateToken::parse(body.state_token)?,ProviderResponse::new(body.provider_response_token)?).await?;
                    Ok(match result.status { Some(value)=>status(value),None=>receipt(result.receipt) })
                },
                _=>Err(Error::Invalid),
            }
        }.await;
        Ok(outcome.unwrap_or_else(|e| error(e, c.request)))
    }
}
