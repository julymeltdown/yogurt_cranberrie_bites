//! Revisioned operational service policy read for the pre-World decision.
//!
//! The public HTTP contract (`getConcertServicePolicy`) already defines the closed
//! decision shapes. This module serves the current publication for one event inside
//! the caller's verified product scope: no default NORMAL, no cached decision and no
//! operator authority is inferred from the client's release claim.
use crate::product_rpc::{
    proto,
    scoped::{ProductRepresentation, ScopedProductOwner, ScopedProductProducer},
    Operation, ProductOwner, VerifiedProduct,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::{fmt, sync::Arc};
use tonic::Status;

/// Longest decision window the public contract accepts.
pub const MAX_DECISION_SECONDS: i64 = 30;

/// Canonical numeric `M.m.p`; no ranges, prereleases or leading zeroes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClientVersion([u16; 3]);

impl ClientVersion {
    pub fn parse(value: &str) -> Option<Self> {
        if value.is_empty() || value.len() > 17 {
            return None;
        }
        let mut parts = value.split('.');
        let mut parsed = [0u16; 3];
        for part in &mut parsed {
            let text = parts.next()?;
            if text.is_empty()
                || !text.bytes().all(|byte| byte.is_ascii_digit())
                || (text.len() > 1 && text.starts_with('0'))
            {
                return None;
            }
            *part = text.parse().ok()?;
        }
        parts.next().is_none().then_some(Self(parsed))
    }
}

impl fmt::Display for ClientVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0[0], self.0[1], self.0[2])
    }
}

/// Closed query subset. Unknown, duplicate or malformed members are refused
/// before any read; absent client input never grants a version decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Query {
    pub version: Option<ClientVersion>,
}

pub fn parse_query(target: &str) -> Result<Query, Status> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok(Query { version: None });
    };
    let (mut version, mut release) = (None, None);
    for pair in query.split('&') {
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(|| Status::invalid_argument("Invalid service policy query"))?;
        match name {
            "client_version" if version.is_none() => {
                version = Some(
                    ClientVersion::parse(value)
                        .ok_or_else(|| Status::invalid_argument("Invalid client version"))?,
                )
            }
            "client_release" if release.is_none() => {
                if value.len() != 64
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(Status::invalid_argument("Invalid client release"));
                }
                release = Some(())
            }
            _ => return Err(Status::invalid_argument("Invalid service policy query")),
        }
    }
    Ok(Query { version })
}

/// One current publication, already scoped to the requested event.
pub struct Publication {
    pub policy_revision: i64,
    pub state: String,
    pub mode: Option<String>,
    pub minimum_client_version: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Decision {
    pub event_revision: i64,
    pub policy_revision: Option<i64>,
    pub kind: &'static str,
    pub minimum_client_version: Option<String>,
    pub server_time: String,
    pub valid_until: String,
}

fn stamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Closed precedence: unusable publication, maintenance, unavailable client,
/// minimum version, normal. A usable NORMAL decision is the only state the
/// pre-World gate accepts.
pub fn decide(
    event_revision: i64,
    publication: Option<&Publication>,
    client: Query,
    now: DateTime<Utc>,
) -> Decision {
    let (kind, policy_revision, minimum) = match publication {
        None => ("UNAVAILABLE", None, None),
        Some(row) if row.state != "PUBLISHED" => ("UNAVAILABLE", Some(row.policy_revision), None),
        Some(row) if row.expires_at.is_none_or(|expires| expires <= now) => {
            ("UNAVAILABLE", Some(row.policy_revision), None)
        }
        Some(row) => {
            let minimum = row.minimum_client_version.clone();
            let revision = Some(row.policy_revision);
            if row.mode.as_deref() == Some("MAINTENANCE") {
                ("MAINTENANCE", revision, minimum)
            } else {
                match (client.version, minimum.as_deref().and_then(ClientVersion::parse)) {
                    (None, _) | (_, None) => ("UNAVAILABLE", revision, minimum),
                    (Some(client), Some(required)) if client < required => {
                        ("CLIENT_VERSION_REQUIRED", revision, minimum)
                    }
                    _ => ("NORMAL", revision, minimum),
                }
            }
        }
    };
    let ttl = publication
        .and_then(|row| row.expires_at)
        .map(|expires| (expires - now).num_seconds().clamp(1, MAX_DECISION_SECONDS))
        .unwrap_or(MAX_DECISION_SECONDS);
    Decision {
        event_revision,
        policy_revision,
        kind,
        minimum_client_version: minimum,
        server_time: stamp(now),
        valid_until: stamp(now + Duration::seconds(ttl)),
    }
}

/// Read-only producer for the exact pre-World decision route.
pub struct ServicePolicyProducer {
    pool: PgPool,
}

impl ServicePolicyProducer {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn read_error(_: sqlx::Error) -> Status {
    Status::unavailable("Service policy read unavailable")
}

#[async_trait]
impl ScopedProductProducer for ServicePolicyProducer {
    async fn dispatch(
        &self,
        verified: &VerifiedProduct,
        scope: Arc<crate::adapters::out::relationship_authority::guarded::OwnerGuardScope>,
        _request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        verified.check()?;
        if verified.operation() != Operation::ServicePolicy {
            return Err(Status::permission_denied(
                "Unsupported service policy operation",
            ));
        }
        let query = verified
            .service_policy_query()
            .ok_or_else(|| Status::invalid_argument("Invalid service policy query"))?;
        let row = sqlx::query(
            "select e.revision as event_revision,
                    p.policy_revision, p.publication_state, p.mode,
                    p.minimum_client_version, p.expires_at,
                    clock_timestamp() as server_time
             from public.concert_events e
             left join lateral (
                 select policy_revision, publication_state, mode, minimum_client_version, expires_at
                 from public.concert_event_service_policy
                 where event_id = e.event_id
                 order by policy_revision desc
                 limit 1
             ) p on true
             where e.event_id = $1",
        )
        .bind(verified.event())
        .fetch_optional(&self.pool)
        .await
        .map_err(read_error)?
        .ok_or_else(|| Status::not_found("Service policy event unavailable"))?;
        let publication = row
            .try_get::<Option<i64>, _>("policy_revision")
            .map_err(read_error)?
            .map(|policy_revision| Publication {
                policy_revision,
                state: row.try_get("publication_state").unwrap_or_default(),
                mode: row.try_get("mode").ok().flatten(),
                minimum_client_version: row.try_get("minimum_client_version").ok().flatten(),
                expires_at: row.try_get("expires_at").ok().flatten(),
            });
        let value = decide(
            row.try_get("event_revision").map_err(read_error)?,
            publication.as_ref(),
            query,
            row.try_get::<DateTime<Utc>, _>("server_time")
                .map_err(read_error)?,
        );
        verified.check()?;
        scope.binding().check()?;
        let body = serde_json::to_vec(&value)
            .map_err(|_| Status::unavailable("Service policy representation unavailable"))?;
        Ok(ProductRepresentation {
            status: 200,
            content_type: "application/json".into(),
            body,
        })
    }
}

/// The operational read is the only operation this layer owns. Every other
/// operation keeps the retained product dispatch (or stays refused when the
/// configuration composes no other owner).
pub struct ServicePolicyOwners {
    policy: ScopedProductOwner,
    base: Option<Arc<dyn ProductOwner>>,
}

impl ServicePolicyOwners {
    pub fn new(policy: ScopedProductOwner, base: Option<Arc<dyn ProductOwner>>) -> Self {
        Self { policy, base }
    }
}

#[async_trait]
impl ProductOwner for ServicePolicyOwners {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, Status> {
        if context.operation() == Operation::ServicePolicy {
            self.policy.dispatch(context, request).await
        } else if let Some(base) = &self.base {
            base.dispatch(context, request).await
        } else {
            Err(Status::permission_denied("Unsupported product operation"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000, 0).expect("fixed timestamp")
    }

    fn published(mode: &str, minimum: &str, expires_in: i64) -> Publication {
        Publication {
            policy_revision: 3,
            state: "PUBLISHED".into(),
            mode: Some(mode.into()),
            minimum_client_version: Some(minimum.into()),
            expires_at: Some(now() + Duration::seconds(expires_in)),
        }
    }

    #[test]
    fn parse_accepts_the_closed_query_and_rejects_duplicates_or_bad_versions() {
        let version = "1.2.3";
        let release = "a".repeat(64);
        let ok = format!("/service-policy?client_version={version}&client_release={release}");
        assert_eq!(
            parse_query(&ok).expect("closed query").version,
            ClientVersion::parse(version)
        );
        for bad in [
            format!("/service-policy?client_version={version}&client_version={version}"),
            "/service-policy?client_version=01.2.3".to_owned(),
            "/service-policy?client_version=1.2".to_owned(),
            "/service-policy?client_version=1.2.3&client_release=zz".to_owned(),
            "/service-policy?client_version=1.2.3&extra=1".to_owned(),
        ] {
            assert!(parse_query(&bad).is_err(), "accepted {bad}");
        }
        assert_eq!(parse_query("/service-policy").expect("absent query").version, None);
    }

    #[test]
    fn the_product_route_classifies_the_policy_read() {
        let event = uuid::Uuid::new_v4();
        let target = format!(
            "/api/concert/events/{event}/service-policy?client_version=1.2.3&client_release={}",
            "a".repeat(64)
        );
        assert_eq!(
            crate::product_rpc::operation(event, "GET", &target).expect("classified"),
            Operation::ServicePolicy
        );
        assert_eq!(
            crate::product_rpc::operation(
                event,
                "GET",
                &format!("/api/concert/events/{event}/service-policy")
            )
            .expect("bodyless read"),
            Operation::ServicePolicy
        );
        assert!(parse_query(&format!(
            "/api/concert/events/{event}/service-policy?client_version=1.2"
        ))
        .is_err());
    }

    #[test]
    fn decision_precedence_never_defaults_to_normal() {
        let query = parse_query(&format!(
            "/service-policy?client_version=1.0.0&client_release={}",
            "a".repeat(64)
        ))
        .expect("query");
        assert_eq!(decide(9, None, query, now()).kind, "UNAVAILABLE");
        let withdrawn = Publication {
            state: "WITHDRAWN".into(),
            mode: None,
            minimum_client_version: None,
            expires_at: None,
            ..published("NORMAL", "0.0.0", 600)
        };
        assert_eq!(
            decide(9, Some(&withdrawn), query, now()).kind,
            "UNAVAILABLE"
        );
        assert_eq!(
            decide(9, Some(&published("NORMAL", "0.0.0", 0)), query, now()).kind,
            "UNAVAILABLE"
        );
        assert_eq!(
            decide(9, Some(&published("MAINTENANCE", "0.0.0", 600)), query, now()).kind,
            "MAINTENANCE"
        );
        assert_eq!(
            decide(9, Some(&published("NORMAL", "2.0.0", 600)), query, now()).kind,
            "CLIENT_VERSION_REQUIRED"
        );
        assert_eq!(
            decide(9, Some(&published("NORMAL", "1.0.0", 600)), query, now()).kind,
            "NORMAL"
        );
        assert_eq!(
            decide(
                9,
                Some(&published("NORMAL", "1.0.0", 600)),
                Query { version: None },
                now()
            )
            .kind,
            "UNAVAILABLE"
        );
        let decision = decide(9, Some(&published("NORMAL", "1.0.0", 600)), query, now());
        assert!(decision.valid_until > decision.server_time);
        assert_eq!(decision.policy_revision, Some(3));
    }
}
