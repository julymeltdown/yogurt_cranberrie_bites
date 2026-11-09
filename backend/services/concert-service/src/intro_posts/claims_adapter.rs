//! T-302: the claim adapter over the profile service.
//!
//! Contract: [domain-contract.md §1.3](../../../../docs/mcr/integration/domain-contract.md)
//! and [identity_claim_reader.proto](../../../../proto/profile/v1/identity_claim_reader.proto).
//!
//! The concert service owns the Introduction audience decision but not the claim
//! store. This adapter is the only way it learns a viewer's age or gender, and it
//! is the place where every failure mode is turned into
//! [`Attribute::Unavailable`].
//!
//! ## The mapping, and why each arm is what it is
//!
//! | Stored state | Attribute |
//! | --- | --- |
//! | live `VERIFIED` with an age | `Known(age claim)` |
//! | live `VERIFIED` with `female`/`male` | `Known(gender)` |
//! | live `VERIFIED` with `unknown` | `Unspecified` |
//! | `REPLACED`, `REVOKED`, expired, unsupported | `Unavailable` |
//! | no row at all | `Unavailable` |
//! | transport error, timeout, malformed reply | `Unavailable` |
//!
//! The last two rows are the important ones. A transport error is not "no
//! restriction": it is "we do not know", and the decision denies on unknown. The
//! adapter never falls back to an older claim, never invents a default, and never
//! caches a value across requests.
//!
//! ## No cross-database join
//!
//! The claim store is in the profile service's database. This module holds a gRPC
//! client and nothing else: it never opens a connection to the profile database,
//! never embeds a SQL statement against `identity_claims`, and never reads a
//! mirrored copy. The `profile_id` is resolved to an account by the profile
//! service itself, so neither side trusts the other's mapping.
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use concert_application::identity_claims::{
    Attribute, ClaimSource, ClaimsError, Gender, IdentityClaims, IdentityClaimsPort, ProfileId,
};
use concert_domain::age_policy::arithmetic::{ExactAgeClaim, SeoulDate};
use platform::grpc_auth::Workload;
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request, Status};

/// The generated client for the profile service's internal claim read.
pub mod proto {
    tonic::include_proto!("profile.claims.v1");
}

/// The per-call budget. A claim read happens inside an audience decision, which
/// happens inside a page query; it must never dominate the request.
const BUDGET: Duration = Duration::from_secs(2);

type Transport = InterceptedService<Channel, platform::grpc_auth::ServiceIdentityClientInterceptor>;

/// The claim adapter.
///
/// One instance per process. It holds a lazily connected channel, so it is cheap
/// to clone and safe to share across requests.
#[derive(Clone)]
pub struct ProfileClaimsAdapter {
    client: proto::identity_claim_reader_client::IdentityClaimReaderClient<Transport>,
}

impl ProfileClaimsAdapter {
    /// Connect to the profile service's claim reader.
    ///
    /// `token` is the concert workload's service identity; the profile service
    /// admits only the Concert workload on this method.
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Profile,
            platform::edge::EdgeBudget {
                edge: "concert->profile-claims",
                expected_latency: Duration::from_millis(50),
                deadline: BUDGET,
                connect_deadline: Duration::from_millis(500),
                retry: platform::edge::RetryClass::SafeToRetry,
                failure_policy: platform::edge::FailurePolicy::FailRequest,
            },
        )?;
        let identity = platform::grpc_auth::ServiceIdentityClientInterceptor::new(
            Workload::Concert,
            token,
        )?;
        Ok(Self {
            client: proto::identity_claim_reader_client::IdentityClaimReaderClient::new(
                InterceptedService::new(channel, identity),
            )
            .max_decoding_message_size(16 * 1024)
            .max_encoding_message_size(16 * 1024),
        })
    }

    /// The read, with the transport error mapped to `ClaimsError::Unavailable`.
    ///
    /// Every arm of this function is a fail-closed arm. There is deliberately no
    /// `unwrap_or_default` anywhere: a missing field, an unknown status string
    /// and an out-of-range age are all "we do not know".
    async fn read_inner(
        &self,
        profile: ProfileId,
        now: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        let mut client = self.client.clone();
        let mut request = Request::new(proto::ReadClaimRequest {
            profile_id: profile.uuid().to_string(),
            evaluated_at_unix_micros: now.timestamp_micros(),
            request_id: uuid::Uuid::new_v4().to_string(),
        });
        request.set_timeout(BUDGET);
        let reply = client
            .read_claim(request)
            .await
            .map_err(|_| ClaimsError::Unavailable)?
            .into_inner();
        // A reply for another subject is refused rather than trusted.
        if reply.profile_id != profile.uuid().to_string() {
            return Err(ClaimsError::Unavailable);
        }
        let Some(claim) = reply.claim else {
            // No claim row at all: a normal, unverified user.
            return Ok(IdentityClaims::unavailable(profile, now));
        };
        // A row that is not live at the evaluation instant is unavailable,
        // whatever its status says. This is the expiry boundary: the profile
        // service evaluated it at the same instant the decision uses.
        if !claim.live {
            return Ok(IdentityClaims::unavailable(profile, now));
        }
        let source = ClaimSource {
            verifier: claim.verifier.clone(),
            source_revision: claim.source_revision.clone(),
            verified_at: parse_timestamp(&claim.verified_at)?,
            expires_at: parse_timestamp(&claim.expires_at)?,
        };
        let age = age_attribute(&claim)?;
        let gender = gender_attribute(&claim)?;
        Ok(IdentityClaims {
            profile,
            age,
            gender,
            source: Some(source),
            evaluated_at: now,
        })
    }
}

/// The age attribute from a live claim.
///
/// The reply carries the age at verification and the next birthday, never a
/// current age: the caller computes the current age with the Seoul-date rule it
/// already imports. A claim missing either half is not a usable age.
fn age_attribute(claim: &proto::ClaimObservation) -> Result<Attribute<ExactAgeClaim>, ClaimsError> {
    let Some(age_at_verification) = claim.age_at_verification else {
        return Ok(Attribute::Unavailable);
    };
    let Some(next_birthday_at) = claim.next_birthday_at.as_deref() else {
        return Ok(Attribute::Unavailable);
    };
    let birthday = NaiveDate::parse_from_str(next_birthday_at, "%Y-%m-%d")
        .map_err(|_| ClaimsError::Unavailable)?;
    let age = i16::try_from(age_at_verification).map_err(|_| ClaimsError::Unavailable)?;
    // The range is the arithmetic module's own supported window, so a pair that
    // could not have come from `derive_exact_age_claim` is refused.
    ExactAgeClaim::from_parts(age, SeoulDate::new(birthday))
        .map(Attribute::Known)
        .ok_or(ClaimsError::Unavailable)
}

/// The gender attribute from a live claim.
///
/// `unknown` is the provider's explicit "not specified" answer and maps to
/// `Unspecified`. Any other unrecognized value is a malformed reply and is
/// refused rather than coerced.
fn gender_attribute(claim: &proto::ClaimObservation) -> Result<Attribute<Gender>, ClaimsError> {
    match claim.gender_bucket.as_str() {
        "female" => Ok(Attribute::Known(Gender::Female)),
        "male" => Ok(Attribute::Known(Gender::Male)),
        "unknown" => Ok(Attribute::Unspecified),
        _ => Err(ClaimsError::Unavailable),
    }
}

/// Parse the contract's RFC 3339 UTC timestamp.
fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, ClaimsError> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|_| ClaimsError::Unavailable)
}

#[async_trait]
impl IdentityClaimsPort for ProfileClaimsAdapter {
    async fn read(
        &self,
        profile: ProfileId,
        now: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        if profile.uuid().is_nil() {
            return Err(ClaimsError::Unavailable);
        }
        self.read_inner(profile, now).await
    }
}

/// A port that always fails. The composition root uses it when the profile
/// service endpoint is not configured, so an unconfigured deployment denies
/// every restricted post rather than admitting them.
pub struct UnconfiguredClaims;

#[async_trait]
impl IdentityClaimsPort for UnconfiguredClaims {
    async fn read(
        &self,
        _: ProfileId,
        _: DateTime<Utc>,
    ) -> Result<IdentityClaims, ClaimsError> {
        Err(ClaimsError::Unavailable)
    }
}

/// Map a gRPC status to the port's single error, for the composition root's
/// health reporting. Never carries a claim value.
pub fn status_is_unavailable(status: &Status) -> bool {
    matches!(
        status.code(),
        tonic::Code::Unavailable
            | tonic::Code::DeadlineExceeded
            | tonic::Code::Internal
            | tonic::Code::Unknown
            | tonic::Code::PermissionDenied
            | tonic::Code::Unauthenticated
            | tonic::Code::InvalidArgument
            | tonic::Code::NotFound
    )
}
