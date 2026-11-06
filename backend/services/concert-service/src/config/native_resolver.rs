//! Closed native evidence dispatch. This composes existing adapters; it grants
//! no authority and does not activate any handler, listener or recovery worker.
use crate::{
    adapters::out::age_provider::settlement::AgeTransactionResolver,
    gift_live::{product::handles as handles_gift, settlement::GiftNativeResolver},
    intro_live::{protocol as intro_protocol, IntroNativeResolver},
    operations_live::settlement::OperationsNativeResolver,
    participant::live::settlement::ParticipantNativeResolver,
    party_live::{self, PartyNativeResolver},
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{
            NativeTransactionResolution, NativeTransactionResolver, PgNativeAbortResolver,
        },
    },
    social_live::{protocol, witness::SocialNativeResolver},
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use sqlx::PgPool;
use tonic::Status;
use uuid::Uuid;

const PRODUCT: &str = "/concert.product.v1.ProductService/Dispatch";
const PARTICIPANT: &str = "/concert.participant.v1.ConcertParticipantService/";

/// Routing information only, never a claim of COMMIT or browser authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeResolverDomain {
    Participant,
    Intro,
    Social,
    Operations,
    Age,
    Wall,
    Party,
    Gift,
    AbortOnly,
}

/// Every concrete adapter receives clones of exactly one configured pool and
/// exactly one instance. Callers cannot inject an adapter from another store.
/// Root must use that same pool/instance for its canonical ledger and registry.
pub struct NativeResolverUnion {
    participant: ParticipantNativeResolver,
    intro: IntroNativeResolver,
    social: SocialNativeResolver,
    operations: OperationsNativeResolver,
    age: AgeTransactionResolver,
    wall: crate::mural_tools_b::MuralToolsNativeResolver,
    party: PartyNativeResolver,
    gift: GiftNativeResolver,
    aborts: PgNativeAbortResolver,
}
impl NativeResolverUnion {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        let options = pool.connect_options();
        let name = options.get_database().unwrap_or_default();
        let dedicated = name == "mcr_concert"
            || name.strip_prefix("mcr_concert_").is_some_and(|suffix| {
                !suffix.is_empty()
                    && name.len() <= 63
                    && suffix
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            });
        if instance.is_nil() || pool.is_closed() || !dedicated {
            return Err(Status::invalid_argument(
                "Invalid native resolver pool or instance",
            ));
        }
        Ok(Self {
            participant: ParticipantNativeResolver::new(pool.clone(), instance)?,
            intro: IntroNativeResolver::new(pool.clone(), instance)?,
            social: SocialNativeResolver::new(pool.clone(), instance)?,
            operations: OperationsNativeResolver::new(pool.clone(), instance)?,
            age: AgeTransactionResolver::new(pool.clone(), instance)?,
            wall: crate::mural_tools_b::MuralToolsNativeResolver::new(pool.clone(), instance)?,
            party: PartyNativeResolver::new(pool.clone(), instance)?,
            gift: GiftNativeResolver::new(pool.clone(), instance)?,
            aborts: PgNativeAbortResolver::new(pool, instance)?,
        })
    }

    /// Select using the immutable signed parent/operation/HTTP tuple. Existing
    /// adapters still validate identity, registration and producer witnesses.
    /// Unknown or malformed routes have no committed-evidence adapter.
    pub fn domain(binding: &OwnerBinding) -> Result<NativeResolverDomain, Status> {
        binding.validate()?;
        let base = format!("/api/concert/events/{}/", binding.event_id);
        let target = binding.http_target.strip_prefix(&base).unwrap_or_default();
        let (path, query) = target
            .split_once('?')
            .map_or((target, None), |(p, q)| (p, Some(q)));
        let method = binding.http_method.as_str();
        let operation = binding.operation.as_str();
        if binding.parent_method == format!("{PARTICIPANT}SetAvatar")
            && (operation, method, target) == ("setConcertAvatar", "PUT", "me/avatar")
            || binding.parent_method == format!("{PARTICIPANT}UpdatePreferences")
                && (operation, method, target) == ("updateConcertParticipant", "PATCH", "me")
        {
            return Ok(NativeResolverDomain::Participant);
        }
        if binding.parent_method != PRODUCT {
            return Ok(NativeResolverDomain::AbortOnly);
        }
        if crate::mural_tools_b::protocol::WallOperation::parse(binding.event_id, method, &binding.http_target).is_ok_and(|route| route.id() == operation) { return Ok(NativeResolverDomain::Wall); }
        // The operation arms are disjoint. There is no first-success chain and
        // no second adapter after the selected adapter returns UNKNOWN/error.
        let domain = match operation {
            _ if intro_protocol::handles_operation(operation)
                && intro_protocol::parse_intro_target(
                    binding.event_id,
                    method,
                    &binding.http_target,
                )
                .is_ok_and(|(actual, _)| actual == operation)
                && (operation != "listConcertIntros"
                    || intro_protocol::parse_query(&binding.http_target).is_ok()) =>
            {
                NativeResolverDomain::Intro
            }
            "listOwnConcertHandles"
            | "setConcertHandle"
            | "removeConcertHandle"
            | "createConcertSocialRequest"
            | "listConcertSocialRequests"
            | "getConcertSocialRequest"
            | "decideConcertSocialRequest"
            | "cancelConcertSocialRequest"
            | "revokeConcertSocialRequest"
            | "revealConcertSocialHandle"
            | "listConcertBlocks"
            | "blockConcertUser"
            | "unblockConcertUser"
                if protocol::route(binding.event_id, method, &binding.http_target)
                    .is_ok_and(|route| route.id() == operation) =>
            {
                NativeResolverDomain::Social
            }
            "getConcertAgeStatus" if (method, target) == ("GET", "me/age-status") => {
                NativeResolverDomain::Age
            }
            "startConcertAgeVerification"
                if (method, target) == ("POST", "me/age-verifications") =>
            {
                NativeResolverDomain::Age
            }
            "completeConcertAgeVerification"
                if (method, target) == ("POST", "me/age-verifications/callback") =>
            {
                NativeResolverDomain::Age
            }
            "eraseConcertAgePrivateData"
                if (method, target) == ("POST", "me/age-private-data/erase") =>
            {
                NativeResolverDomain::Age
            }
            _ if party_live::handles_operation(operation) => NativeResolverDomain::Party,
            _ if handles_gift(operation)
                && crate::gift_live::GiftOperation::route(
                    binding.event_id,
                    method,
                    &binding.http_target,
                )
                .is_ok_and(|route| route.id() == operation) =>
            {
                NativeResolverDomain::Gift
            }
            _ if operations_route(operation, method, path, query) => {
                NativeResolverDomain::Operations
            }
            _ => NativeResolverDomain::AbortOnly,
        };
        Ok(domain)
    }
}

fn canonical_suffix(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix).is_some_and(|value| {
        Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
    })
}
fn operations_route(operation: &str, method: &str, path: &str, query: Option<&str>) -> bool {
    // Lists retain their signed query bytes. Parsing/body validation was done
    // by the producer; this predicate cannot grant a new producer invocation.
    if query.is_some_and(|q| q.is_empty() || q.contains('#')) {
        return false;
    }
    match (operation, method, path) {
        ("listOwnConcertReports", "GET", "me/reports")
        | ("listConcertAdminReports", "GET", "admin/reports")
        | ("listConcertAdminPlaces", "GET", "admin/places")
        | ("listConcertNotices", "GET", "notices")
        | ("listConcertAdminNotices", "GET", "admin/notices") => true,
        _ if query.is_some() => false,
        ("getConcertSettings", "GET", "admin/settings")
        | ("setConcertSettings", "PUT", "admin/settings")
        | ("setConcertEventPhase", "POST", "admin/phase")
        | ("createConcertReport", "POST", "reports")
        | ("createConcertNotice", "POST", "admin/notices") => true,
        ("getConcertAdminReport", "GET", _) => canonical_suffix(path, "admin/reports/"),
        ("getConcertNotice", "GET", _) => canonical_suffix(path, "notices/"),
        ("updateConcertNotice", "PUT", _) => canonical_suffix(path, "admin/notices/"),
        ("setConcertPlace", "PUT", _) => canonical_suffix(path, "admin/places/"),
        _ => false,
    }
}

#[async_trait::async_trait]
impl NativeTransactionResolver for NativeResolverUnion {
    async fn resolve(
        &self,
        binding: &OwnerBinding,
        identity: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        let selected: &dyn NativeTransactionResolver = match Self::domain(binding)? {
            NativeResolverDomain::Participant => &self.participant,
            NativeResolverDomain::Intro => &self.intro,
            NativeResolverDomain::Social => &self.social,
            NativeResolverDomain::Operations => &self.operations,
            NativeResolverDomain::Age => &self.age,
            NativeResolverDomain::Wall => &self.wall,
            NativeResolverDomain::Party => &self.party,
            NativeResolverDomain::Gift => &self.gift,
            NativeResolverDomain::AbortOnly => &self.aborts,
        };
        selected.resolve(binding, identity).await
    }
}
