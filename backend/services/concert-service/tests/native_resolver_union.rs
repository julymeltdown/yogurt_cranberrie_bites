//! CU01 component checks. Route tables are not authority/runtime evidence.
#[path = "native_resolver_union_support/environment.rs"]
mod environment;
#[path = "native_resolver_union_support/fixture.rs"]
mod fixture;
#[path = "native_resolver_union_support/native.rs"]
mod native;
#[path = "native_resolver_union_support/owners.rs"]
mod owners;
#[path = "native_resolver_union_support/participant_capture.rs"]
mod participant_capture;
#[path = "native_resolver_union_support/product_capture.rs"]
mod product_capture;

use concert_service::config::native_resolver::{
    NativeResolverDomain as Domain, NativeResolverUnion,
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use sqlx::postgres::PgPoolOptions;
use tonic::Code;
use uuid::Uuid;

fn binding(operation: &str, method: &str, suffix: &str) -> OwnerBinding {
    let event = Uuid::from_u128(1);
    OwnerBinding {
        actor_id: Uuid::from_u128(2),
        event_id: event,
        session_family_id: Uuid::from_u128(3),
        request_id: Uuid::from_u128(4),
        operation: operation.into(),
        expires_unix_micros: 1_800_000_000_000_000,
        parent_digest: [1; 32],
        parent_body_digest: [2; 32],
        body_digest: [3; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: method.into(),
        http_target: format!("/api/concert/events/{event}/{suffix}"),
    }
}

#[test]
fn closed_route_table_matches_actual_producer_ownership() {
    let id = "00000000-0000-0000-0000-000000000009";
    let rows = [
        (
            "listConcertIntros",
            "GET",
            "intros?limit=10&q=music".into(),
            Domain::Intro,
        ),
        (
            "getConcertIntro",
            "GET",
            format!("intros/{id}"),
            Domain::Intro,
        ),
        (
            "getOwnConcertIntro",
            "GET",
            "me/intro".into(),
            Domain::Intro,
        ),
        ("setConcertIntro", "PUT", "me/intro".into(), Domain::Intro),
        (
            "publishConcertIntro",
            "POST",
            "me/intro/publish".into(),
            Domain::Intro,
        ),
        (
            "closeConcertIntro",
            "POST",
            "me/intro/close".into(),
            Domain::Intro,
        ),
        (
            "getConcertAgeStatus",
            "GET",
            "me/age-status".into(),
            Domain::Age,
        ),
        (
            "startConcertAgeVerification",
            "POST",
            "me/age-verifications".into(),
            Domain::Age,
        ),
        (
            "completeConcertAgeVerification",
            "POST",
            "me/age-verifications/callback".into(),
            Domain::Age,
        ),
        (
            "eraseConcertAgePrivateData",
            "POST",
            "me/age-private-data/erase".into(),
            Domain::Age,
        ),
        (
            "listOwnConcertHandles",
            "GET",
            "me/social-handles".into(),
            Domain::Social,
        ),
        (
            "setConcertHandle",
            "PUT",
            "me/social-handles/INSTAGRAM".into(),
            Domain::Social,
        ),
        (
            "removeConcertHandle",
            "DELETE",
            "me/social-handles/X?expected_revision=1".into(),
            Domain::Social,
        ),
        (
            "createConcertSocialRequest",
            "POST",
            "social-requests".into(),
            Domain::Social,
        ),
        (
            "listConcertSocialRequests",
            "GET",
            "me/social-requests?limit=10".into(),
            Domain::Social,
        ),
        (
            "getConcertSocialRequest",
            "GET",
            format!("social-requests/{id}"),
            Domain::Social,
        ),
        (
            "decideConcertSocialRequest",
            "POST",
            format!("social-requests/{id}/decision"),
            Domain::Social,
        ),
        (
            "cancelConcertSocialRequest",
            "DELETE",
            format!("social-requests/{id}?expected_revision=1"),
            Domain::Social,
        ),
        (
            "revokeConcertSocialRequest",
            "POST",
            format!("social-requests/{id}/revoke"),
            Domain::Social,
        ),
        (
            "revealConcertSocialHandle",
            "GET",
            format!("social-requests/{id}/reveal?platform=INSTAGRAM"),
            Domain::Social,
        ),
        (
            "listConcertBlocks",
            "GET",
            "me/blocks".into(),
            Domain::Social,
        ),
        (
            "blockConcertUser",
            "PUT",
            format!("me/blocks/{id}"),
            Domain::Social,
        ),
        (
            "unblockConcertUser",
            "DELETE",
            format!("me/blocks/{id}"),
            Domain::Social,
        ),
        (
            "createConcertReport",
            "POST",
            "reports".into(),
            Domain::Operations,
        ),
        (
            "listOwnConcertReports",
            "GET",
            "me/reports?limit=10".into(),
            Domain::Operations,
        ),
        (
            "listConcertAdminReports",
            "GET",
            "admin/reports".into(),
            Domain::Operations,
        ),
        (
            "getConcertAdminReport",
            "GET",
            format!("admin/reports/{id}"),
            Domain::Operations,
        ),
        (
            "getConcertSettings",
            "GET",
            "admin/settings".into(),
            Domain::Operations,
        ),
        (
            "setConcertSettings",
            "PUT",
            "admin/settings".into(),
            Domain::Operations,
        ),
        (
            "setConcertEventPhase",
            "POST",
            "admin/phase".into(),
            Domain::Operations,
        ),
        (
            "listConcertAdminPlaces",
            "GET",
            "admin/places".into(),
            Domain::Operations,
        ),
        (
            "setConcertPlace",
            "PUT",
            format!("admin/places/{id}"),
            Domain::Operations,
        ),
        (
            "listConcertNotices",
            "GET",
            "notices".into(),
            Domain::Operations,
        ),
        (
            "getConcertNotice",
            "GET",
            format!("notices/{id}"),
            Domain::Operations,
        ),
        (
            "listConcertAdminNotices",
            "GET",
            "admin/notices".into(),
            Domain::Operations,
        ),
        (
            "createConcertNotice",
            "POST",
            "admin/notices".into(),
            Domain::Operations,
        ),
        (
            "updateConcertNotice",
            "PUT",
            format!("admin/notices/{id}"),
            Domain::Operations,
        ),
    ];
    for (op, method, target, domain) in &rows {
        let b = binding(op, method, target);
        assert_eq!(NativeResolverUnion::domain(&b).unwrap(), *domain, "{op}");
        // Cross-domain relabeling never selects the named producer's adapter.
        for (foreign, _, _, other) in &rows {
            if other != domain {
                let mut swapped = b.clone();
                swapped.operation = foreign.to_string();
                assert_eq!(
                    NativeResolverUnion::domain(&swapped).unwrap(),
                    Domain::AbortOnly,
                    "{op} -> {foreign}"
                );
            }
        }
    }
    for (op, method, target, rpc) in [
        ("setConcertAvatar", "PUT", "me/avatar", "SetAvatar"),
        (
            "updateConcertParticipant",
            "PATCH",
            "me",
            "UpdatePreferences",
        ),
    ] {
        let mut b = binding(op, method, target);
        assert_eq!(NativeResolverUnion::domain(&b).unwrap(), Domain::AbortOnly);
        b.parent_method = format!("/concert.participant.v1.ConcertParticipantService/{rpc}");
        assert_eq!(
            NativeResolverUnion::domain(&b).unwrap(),
            Domain::Participant
        );
        b.http_target.push_str("?extra=1");
        assert_eq!(NativeResolverUnion::domain(&b).unwrap(), Domain::AbortOnly);
    }
}

#[test]
fn foreign_malformed_and_unsupported_routes_have_no_commit_adapter() {
    for (op, method, target) in [
        ("createConcertParty", "POST", "parties"),
        ("listConcertIntros", "GET", "intros?unknown=1"),
        ("listConcertIntros", "GET", "intros?limit=10&limit=10"),
        ("listConcertIntros", "GET", "intros?limit=0"),
        ("listConcertIntros", "GET", "intros?"),
        ("listConcertIntros", "GET", "intros?q=%ZZ"),
        ("getConcertIntro", "GET", "intros/not-a-uuid"),
        ("setConcertIntro", "PUT", "me/intro?extra=1"),
        ("setConcertIntro", "POST", "me/intro"),
        ("publishConcertIntro", "POST", "me/intro/close"),
        ("setConcertSettings", "POST", "admin/settings"),
        ("setConcertSettings", "PUT", "admin/settings?x=1"),
        ("setConcertPlace", "PUT", "admin/places/not-a-uuid"),
        ("getConcertAgeStatus", "GET", "me/age-status?x=1"),
        ("setConcertHandle", "PUT", "me/social-handles/TIKTOK"),
        ("setConcertSettings", "PUT", "admin//settings"),
        ("setConcertSettings", "PUT", "admin/%73ettings"),
    ] {
        assert_eq!(
            NativeResolverUnion::domain(&binding(op, method, target)).unwrap(),
            Domain::AbortOnly
        );
    }
    let mut b = binding("getConcertAgeStatus", "GET", "me/age-status");
    b.http_target = b
        .http_target
        .replace(&b.event_id.to_string(), &Uuid::from_u128(99).to_string());
    assert_eq!(
        NativeResolverUnion::domain(&b).unwrap_err().code(),
        Code::InvalidArgument
    );
    let mut b = binding("getConcertAgeStatus", "GET", "me/age-status");
    b.parent_method = "/foreign/Dispatch".into();
    assert!(NativeResolverUnion::domain(&b).is_err());
    let mut b = binding("getConcertAgeStatus", "GET", "me/age-status");
    b.body_digest = [0; 32];
    assert!(NativeResolverUnion::domain(&b).is_err());
}

#[tokio::test]
async fn typed_constructor_refuses_nil_closed_and_foreign_database_pools() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/mcr_concert_cu01")
        .unwrap();
    assert!(NativeResolverUnion::new(pool.clone(), Uuid::new_v4()).is_ok());
    assert!(
        matches!(NativeResolverUnion::new(pool.clone(),Uuid::nil()),Err(e) if e.code()==Code::InvalidArgument)
    );
    pool.close().await;
    assert!(NativeResolverUnion::new(pool, Uuid::new_v4()).is_err());
    for name in [
        "postgres",
        "production",
        "mcr_concert_",
        "mcr_concert_Upper",
    ] {
        let pool = PgPoolOptions::new()
            .connect_lazy(&format!("postgresql://localhost/{name}"))
            .unwrap();
        assert!(NativeResolverUnion::new(pool, Uuid::new_v4()).is_err());
    }
}
