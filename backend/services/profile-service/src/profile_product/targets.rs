//! Closed profile route set and the profile producer's local work.
//!
//! The route parser is the profile analogue of the concert
//! `product_rpc::operation` parser, with two differences that are the whole
//! point of this work package:
//!
//! * there is no event prefix to strip, because there is no event;
//! * an event path is rejected outright rather than parsed, so a profile
//!   operation can never be executed under an event's authority scope.
//!
//! The producer in this module implements the operations that this work package
//! owns. Operations whose product behavior arrives in a later work package are
//! deliberately absent from the closed set and are refused, never defaulted.
use super::{
    proto, rejects_event_path, scoped::ProfileRepresentation, VerifiedProfile,
};
use crate::adapters::out::profile_owner_guard::ProfileOwnerScope;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

/// Closed set of profile product operations this transport serves.
///
/// The list is the profile subset of the frozen operation register. A route not
/// in this set is refused with `permission_denied`; there is no generic
/// forwarding fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileOperation {
    GetOwnProfile,
    GetProfile(Uuid),
}

impl ProfileOperation {
    pub fn contract_id(self) -> &'static str {
        match self {
            Self::GetOwnProfile => "getOwnProfile",
            Self::GetProfile(_) => "getProfile",
        }
    }
    pub fn mutation(self) -> bool {
        false
    }
}

/// Parse a profile target. `profile` is the authenticated actor's profile, used
/// to bind `{profileId}` targets to the caller when the operation is owner-only.
pub fn parse(method: &str, target: &str) -> Result<ProfileOperation, Status> {
    if target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(Status::invalid_argument("Invalid profile target"));
    }
    if rejects_event_path(target) {
        return Err(Status::invalid_argument(
            "Profile operations never carry an event path",
        ));
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if !path.starts_with("/api/") {
        return Err(Status::permission_denied("Unsupported profile operation"));
    }
    let result = match (method, path) {
        ("GET", "/api/me/profile") => ProfileOperation::GetOwnProfile,
        ("GET", rest) => match rest.strip_prefix("/api/profiles/") {
            Some(id) if !id.contains('/') => {
                ProfileOperation::GetProfile(super::canonical_uuid(id)?)
            }
            _ => return Err(Status::permission_denied("Unsupported profile operation")),
        },
        _ => return Err(Status::permission_denied("Unsupported profile operation")),
    };
    // Neither frozen read carries a query. Reject one so a later addition
    // cannot be silently ignored.
    if query.is_some() {
        return Err(Status::invalid_argument("Unexpected profile query"));
    }
    Ok(result)
}

/// The local producer. It performs real local work under the held scope and
/// returns a representation; it never mints authority or completes the scope.
pub struct ProfileProductProducer {
    pool: sqlx::PgPool,
}

impl ProfileProductProducer {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl super::scoped::ScopedProfileProducer for ProfileProductProducer {
    async fn dispatch(
        &self,
        context: &VerifiedProfile,
        _scope: Arc<ProfileOwnerScope>,
        request: proto::ProfileRequest,
    ) -> Result<ProfileRepresentation, Status> {
        let operation = parse(&request.http_method, &request.http_target)?;
        match operation {
            ProfileOperation::GetOwnProfile => {
                let row: Option<(Uuid, Uuid, String, Option<String>, Option<Uuid>, String, i64)> =
                    sqlx::query_as(
                        "select profile_id,account_id,nickname,bio,profile_image_media_id,\
                         visibility,revision from profile_identities \
                         where account_id=$1 and deleted_at is null",
                    )
                    .bind(context.actor())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|_| Status::unavailable("Profile store unavailable"))?;
                let Some((profile_id, account_id, nickname, bio, image, visibility, revision)) = row
                else {
                    return Err(Status::not_found("Profile is unavailable"));
                };
                json(200, serde_json::json!({
                    "profile_id": profile_id,
                    "account_id": account_id,
                    "nickname": nickname,
                    "bio": bio,
                    "profile_image_media_id": image,
                    "visibility": visibility,
                    "revision": revision,
                }))
            }
            ProfileOperation::GetProfile(profile_id) => {
                let row: Option<(Uuid, String, Option<String>, Option<Uuid>, String)> =
                    sqlx::query_as(
                        "select profile_id,nickname,bio,profile_image_media_id,visibility \
                         from profile_identities where profile_id=$1 and deleted_at is null",
                    )
                    .bind(profile_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|_| Status::unavailable("Profile store unavailable"))?;
                let Some((profile_id, nickname, bio, image, visibility)) = row else {
                    return Err(Status::not_found("Profile is unavailable"));
                };
                // A private profile is unavailable to anyone but its owner, and
                // the refusal is indistinguishable from absence. Ownership is
                // decided by the account binding, not by comparing the viewer's
                // account id with the profile id.
                let owner: Uuid = sqlx::query_scalar(
                    "select account_id from profile_identities where profile_id=$1",
                )
                .bind(profile_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| Status::unavailable("Profile store unavailable"))?;
                if visibility != "PUBLIC" && context.actor() != owner {
                    return Err(Status::not_found("Profile is unavailable"));
                }
                json(200, serde_json::json!({
                    "profile_id": profile_id,
                    "nickname": nickname,
                    "bio": bio,
                    "profile_image_media_id": image,
                }))
            }
        }
    }
}

fn json(status: u32, body: serde_json::Value) -> Result<ProfileRepresentation, Status> {
    Ok(ProfileRepresentation {
        status,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&body).map_err(|_| Status::internal("Profile projection"))?,
    })
}
