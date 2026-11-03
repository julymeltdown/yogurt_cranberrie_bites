//! Handler-level ownership for the author-existence authority projection.
//!
//! `CheckAuthorProfile` answers existence only, so these call the generated
//! handler directly with the extensions the delegation and identity
//! interceptors attach. Bypassing the server keeps the authorization path
//! under test rather than the transport, and needs no database.
use async_trait::async_trait;
use platform::actor_delegation::VerifiedActor;
use platform::grpc_auth::{VerifiedCaller, Workload};
use profile_application::author_existence::{
    AuthorProfileLookupPort, AuthorProfileQueryDto, ExistingAuthorProfileDto,
};
use profile_application::errors::ProfileResult;
use profile_service::adapters::r#in::grpc::author_authority::{
    proto::{self, profile_author_authority_service_server::ProfileAuthorAuthorityService},
    ProfileAuthorAuthorityGrpcServer,
};
use std::sync::Arc;
use tonic::{Code, Request};
use uuid::Uuid;

const AUTHOR: Uuid = Uuid::from_u128(0x1111_1111_1111_1111_1111_1111_1111_1111);
const OTHER: Uuid = Uuid::from_u128(0x2222_2222_2222_2222_2222_2222_2222_2222);

/// An owner lookup that finds exactly the seeded authors.
struct SeededAuthors(Vec<Uuid>);

#[async_trait]
impl AuthorProfileLookupPort for SeededAuthors {
    async fn find_existing_author(
        &self,
        query: AuthorProfileQueryDto,
    ) -> ProfileResult<Option<ExistingAuthorProfileDto>> {
        if self.0.contains(&query.user_id()) {
            return Ok(Some(ExistingAuthorProfileDto::new(query.user_id())?));
        }
        Ok(None)
    }
}

/// An owner lookup that answers with a different user than the one asked for.
/// The use case must refuse it rather than trusting the owner's own claim.
struct OtherOwner;

#[async_trait]
impl AuthorProfileLookupPort for OtherOwner {
    async fn find_existing_author(
        &self,
        _: AuthorProfileQueryDto,
    ) -> ProfileResult<Option<ExistingAuthorProfileDto>> {
        Ok(Some(ExistingAuthorProfileDto::new(OTHER)?))
    }
}

fn presented<T>(message: T, caller: Option<Workload>, actor: Option<Uuid>) -> Request<T> {
    let mut request = Request::new(message);
    if let Some(caller) = caller {
        request.extensions_mut().insert(VerifiedCaller(caller));
    }
    if let Some(actor) = actor {
        request.extensions_mut().insert(VerifiedActor(actor));
    }
    request
}

fn ask(user_id: Uuid) -> proto::CheckAuthorProfileRequest {
    proto::CheckAuthorProfileRequest {
        user_id: user_id.to_string(),
    }
}

#[tokio::test]
async fn check_author_profile_rejects_a_caller_that_is_not_an_authenticated_post() {
    let grpc = ProfileAuthorAuthorityGrpcServer::new(Arc::new(SeededAuthors(vec![AUTHOR])));
    for caller in [
        None,
        Some(Workload::Gateway),
        Some(Workload::Friend),
        Some(Workload::Profile),
    ] {
        let error = ProfileAuthorAuthorityService::check_author_profile(
            &grpc,
            presented(ask(AUTHOR), caller, Some(AUTHOR)),
        )
        .await
        .expect_err("only an authenticated Post presenter may ask")
        .code();
        assert_eq!(
            error,
            if caller.is_none() {
                Code::Unauthenticated
            } else {
                Code::PermissionDenied
            },
            "caller={caller:?}"
        );
    }
}

#[tokio::test]
async fn check_author_profile_binds_the_requested_author_to_the_verified_actor() {
    let grpc = ProfileAuthorAuthorityGrpcServer::new(Arc::new(SeededAuthors(vec![AUTHOR, OTHER])));

    // An actor asking about another user is refused before any owner lookup,
    // so one author's child authority cannot confirm a different author.
    let denied = ProfileAuthorAuthorityService::check_author_profile(
        &grpc,
        presented(ask(OTHER), Some(Workload::Post), Some(AUTHOR)),
    )
    .await
    .expect_err("actor and author must match");
    assert_eq!(denied.code(), Code::PermissionDenied);
    assert_eq!(denied.message(), "Author binding denied");

    // A child that verifies but carries no actor cannot be acted on at all.
    let unverified = ProfileAuthorAuthorityService::check_author_profile(
        &grpc,
        presented(ask(AUTHOR), Some(Workload::Post), None),
    )
    .await
    .expect_err("a verified child authority is required");
    assert_eq!(unverified.code(), Code::Unauthenticated);

    // The same caller asking about itself is answered, and only the id comes
    // back: this projection carries no profile content.
    let found = ProfileAuthorAuthorityService::check_author_profile(
        &grpc,
        presented(ask(AUTHOR), Some(Workload::Post), Some(AUTHOR)),
    )
    .await
    .expect("the owning author is confirmed")
    .into_inner();
    assert_eq!(found.user_id, AUTHOR.to_string());
}

#[tokio::test]
async fn check_author_profile_refuses_an_owner_that_answers_for_another_user() {
    let grpc = ProfileAuthorAuthorityGrpcServer::new(Arc::new(OtherOwner));
    let error = ProfileAuthorAuthorityService::check_author_profile(
        &grpc,
        presented(ask(AUTHOR), Some(Workload::Post), Some(AUTHOR)),
    )
    .await
    .expect_err("the existence result must be re-checked against the same id");
    assert_eq!(error.code(), Code::Internal);
    assert_eq!(error.message(), "Profile lookup failed");
}

#[tokio::test]
async fn check_author_profile_reports_absence_and_a_malformed_id_without_content() {
    let grpc = ProfileAuthorAuthorityGrpcServer::new(Arc::new(SeededAuthors(vec![AUTHOR])));

    let missing = ProfileAuthorAuthorityService::check_author_profile(
        &grpc,
        presented(ask(OTHER), Some(Workload::Post), Some(OTHER)),
    )
    .await
    .expect_err("an unknown author does not exist");
    assert_eq!(missing.code(), Code::NotFound);

    let malformed = ProfileAuthorAuthorityService::check_author_profile(
        &grpc,
        presented(
            proto::CheckAuthorProfileRequest {
                user_id: "not-a-uuid".into(),
            },
            Some(Workload::Post),
            Some(AUTHOR),
        ),
    )
    .await
    .expect_err("the author id must parse");
    assert_eq!(malformed.code(), Code::InvalidArgument);
}
