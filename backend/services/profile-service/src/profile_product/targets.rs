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
use crate::catalog;
use crate::identity::{
    CallbackCommand, CallbackError, CallbackResult, IdentityRuntime, StartError,
    VerificationState,
};
use chrono::Utc;
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
    /// `getOwnIdentityVerification` — the owner's own claim summary.
    GetOwnIdentityVerification,
    /// `startIdentityVerification` — begin a one-use provider state.
    StartIdentityVerification,
    /// `disconnectIdentityVerification` — revoke the live claim.
    DisconnectIdentityVerification,
    /// `searchCatalog` — search one favorites category.
    SearchCatalog(catalog::Category),
}

impl ProfileOperation {
    pub fn contract_id(self) -> &'static str {
        match self {
            Self::GetOwnProfile => "getOwnProfile",
            Self::GetProfile(_) => "getProfile",
            Self::GetOwnIdentityVerification => "getOwnIdentityVerification",
            Self::StartIdentityVerification => "startIdentityVerification",
            Self::DisconnectIdentityVerification => "disconnectIdentityVerification",
            Self::SearchCatalog(_) => "searchCatalog",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(
            self,
            Self::StartIdentityVerification | Self::DisconnectIdentityVerification
        )
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
    // `searchCatalog` is the one served operation that carries a query
    // (`q`, `cursor`, `limit`). It is matched before the generic "no query"
    // rule below, and its query is validated by the catalog module.
    if method == "GET" {
        if let Some(rest) = path.strip_prefix("/api/catalog/") {
            let mut segments = rest.split('/');
            let (Some(category), Some("search"), None) =
                (segments.next(), segments.next(), segments.next())
            else {
                return Err(Status::permission_denied("Unsupported profile operation"));
            };
            let category = catalog::Category::parse(category)
                .ok_or_else(|| Status::not_found("Catalog category is unavailable"))?;
            return Ok(ProfileOperation::SearchCatalog(category));
        }
    }
    let result = match (method, path) {
        ("GET", "/api/me/profile") => ProfileOperation::GetOwnProfile,
        // The identity routes are the frozen `core` identity operations. The
        // provider segment is a closed enum with one value; anything else is
        // `RESOURCE_UNAVAILABLE`, which the gateway renders as a 404.
        ("GET", "/api/me/identity/naver") => ProfileOperation::GetOwnIdentityVerification,
        ("POST", "/api/me/identity/naver/verifications") => {
            ProfileOperation::StartIdentityVerification
        }
        ("DELETE", "/api/me/identity/naver") => ProfileOperation::DisconnectIdentityVerification,
        ("GET", rest) => match rest.strip_prefix("/api/profiles/") {
            Some(id) if !id.contains('/') => {
                ProfileOperation::GetProfile(super::canonical_uuid(id)?)
            }
            _ => return Err(Status::permission_denied("Unsupported profile operation")),
        },
        _ => return Err(Status::permission_denied("Unsupported profile operation")),
    };
    // Only the start operation carries a query, and it carries none either: the
    // return path is a JSON body field. Every other frozen read and mutation in
    // this closed set rejects a query so a later addition cannot be silently
    // ignored.
    if query.is_some() {
        return Err(Status::invalid_argument("Unexpected profile query"));
    }
    Ok(result)
}

/// The local producer. It performs real local work under the held scope and
/// returns a representation; it never mints authority or completes the scope.
pub struct ProfileProductProducer {
    pool: sqlx::PgPool,
    /// `None` when the NAVER provider is not configured. Every identity
    /// operation then fails closed with `DEPENDENCY_UNAVAILABLE`; it never
    /// fabricates a claim.
    identity: Option<IdentityRuntime>,
}

impl ProfileProductProducer {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            identity: None,
        }
    }

    /// The producer with the NAVER verification flow attached.
    pub fn with_identity(pool: sqlx::PgPool, identity: IdentityRuntime) -> Self {
        Self {
            pool,
            identity: Some(identity),
        }
    }

    fn identity(&self) -> Result<&IdentityRuntime, Status> {
        self.identity
            .as_ref()
            .ok_or_else(|| Status::unavailable("Identity verification is unavailable"))
    }

    /// The catalog items the viewer and `profile_id` both selected.
    ///
    /// One statement over `profile_favorites` and `catalog_items`, both in this
    /// database. A favorite whose catalog row is missing is skipped rather than
    /// emitted with an invented label: the label is the only thing the card
    /// prints, and a blank one would be a fabricated value.
    ///
    /// A viewer with no profile of their own (an account that never created one)
    /// has no favorites, so the overlap is empty. That is a real answer, not an
    /// error: the card simply shows no shared interest.
    async fn shared_items(
        &self,
        profile_id: Uuid,
        viewer_account: Uuid,
    ) -> Result<Vec<serde_json::Value>, Status> {
        // The viewer's own profile, if any. `None` means no overlap.
        let viewer_profile: Option<Uuid> = sqlx::query_scalar(
            "select profile_id from profile_identities \
             where account_id=$1 and deleted_at is null",
        )
        .bind(viewer_account)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| Status::unavailable("Profile store unavailable"))?;
        let Some(viewer_profile) = viewer_profile else {
            return Ok(Vec::new());
        };
        if viewer_profile == profile_id {
            // The owner viewing their own profile: every favorite is "shared".
            // The visitor card is not shown to the owner, so this is not a
            // privacy question — but reporting the owner's own set as an overlap
            // would be misleading, so it is empty.
            return Ok(Vec::new());
        }
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "select c.catalog_item_id, c.label, c.category \
             from profile_favorites mine \
             join profile_favorites theirs \
               on theirs.catalog_item_id = mine.catalog_item_id \
              and theirs.profile_id = $1 \
             join catalog_items c on c.catalog_item_id = mine.catalog_item_id \
             where mine.profile_id = $2 \
             order by c.category, c.label, c.catalog_item_id \
             limit 15",
        )
        .bind(profile_id)
        .bind(viewer_profile)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| Status::unavailable("Profile store unavailable"))?;
        Ok(rows
            .into_iter()
            .map(|(id, label, category)| {
                serde_json::json!({
                    "catalog_item_id": id,
                    "label": label,
                    "category": category.to_uppercase(),
                })
            })
            .collect())
    }

    /// The owner's gender bucket, only when the owner opted in to showing it.
    ///
    /// The opt-in is `display_on_profile`. It is `false` until a profile display
    /// setting exists, so this returns `None` today — by construction, not by a
    /// missing query. The claim is still read so that the moment the setting
    /// exists this reports the verified value instead of a placeholder, and so
    /// that the "no claim" and "not opted in" cases are distinct in the code.
    ///
    /// An expired or revoked claim is not a live claim and is never shown.
    async fn visible_gender_bucket(
        &self,
        owner_account: Uuid,
        _profile_id: Uuid,
        _viewer_account: Uuid,
    ) -> Result<Option<&'static str>, Status> {
        // The identity runtime is optional: without a configured provider there
        // is no claim to read, and no claim is never shown.
        let Ok(identity) = self.identity() else {
            return Ok(None);
        };
        let verification = identity.read().read(owner_account, Utc::now()).await;
        // The owner's opt-in is the gate. `display_on_profile` is `false` until a
        // profile display setting exists, so this returns `None` today.
        if !verification.display_on_profile {
            return Ok(None);
        }
        // Only a live, verified claim is shown. An expired or revoked claim is
        // not a current fact about the owner.
        if verification.status != VerificationState::Verified {
            return Ok(None);
        }
        Ok(verification
            .claims
            .as_ref()
            .map(|claims| claims.gender_bucket.as_str()))
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
                // ── The visitor fields ─────────────────────────────────────
                //
                // All three are read from THIS database. A cross-database join
                // is forbidden (domain-contract §1.1), and none is needed:
                // `profile_favorites`/`catalog_items` and `identity_claims` both
                // live in `mcr_profile` alongside `profile_identities`.
                //
                // `shared_items` is the catalog overlap between the viewer and
                // the profile owner, computed from the two favorite sets rather
                // than from the concert service's `discovery_favorite_items`
                // projection: the projection is a display copy written by a
                // background worker, and a read must not depend on a worker
                // having run.
                let shared_items = self.shared_items(profile_id, context.actor()).await?;
                // `room_available` is whether the owner's room is visible to
                // this viewer. `profile_rooms` lives in `mcr_concert`, so this
                // service cannot read its `visibility` column without a
                // cross-database join. The owner's own room visibility is
                // therefore reported as unavailable here (see the module note),
                // and the field is `false` — the closed answer a viewer gets for
                // a room they may not see, which is also what an absent room
                // means. It never claims a room exists that this service cannot
                // verify.
                let room_available = false;
                // `gender_bucket` is the owner's verified claim, and only when
                // the owner opted in. The opt-in is `display_on_profile`, which
                // is `false` until a profile display setting exists
                // (`identity::status`), so this is `null` today by construction:
                // the value is never sent without the owner's consent.
                let gender_bucket = self
                    .visible_gender_bucket(owner, profile_id, context.actor())
                    .await?;
                json(200, serde_json::json!({
                    "profile_id": profile_id,
                    "nickname": nickname,
                    "bio": bio,
                    "profile_image_media_id": image,
                    "shared_items": shared_items,
                    "room_available": room_available,
                    "gender_bucket": gender_bucket,
                }))
            }
            ProfileOperation::GetOwnIdentityVerification => {
                let now = Utc::now();
                let verification = self.identity()?.read().read(context.actor(), now).await;
                json(200, verification.to_json())
            }
            ProfileOperation::StartIdentityVerification => {
                let body: serde_json::Value = if request.http_body.is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_slice(&request.http_body).map_err(|_| {
                        Status::invalid_argument("Invalid identity verification request")
                    })?
                };
                // `core.StartIdentityVerification` has exactly one field and
                // `additionalProperties: false`. An unknown field is refused
                // rather than ignored, so a client cannot smuggle a provider
                // value into the flow.
                let object = body
                    .as_object()
                    .ok_or_else(|| Status::invalid_argument("Invalid identity verification request"))?;
                if object.keys().any(|key| key != "return_path") {
                    return Err(Status::invalid_argument(
                        "Invalid identity verification request",
                    ));
                }
                let return_path = match object.get("return_path") {
                    Some(serde_json::Value::String(value)) => Some(value.as_str()),
                    None => None,
                    Some(_) => {
                        return Err(Status::invalid_argument(
                            "Invalid identity verification request",
                        ))
                    }
                };
                let started = self
                    .identity()?
                    .start()
                    .begin(context.actor(), return_path, Utc::now())
                    .await
                    .map_err(start_error)?;
                json(
                    200,
                    serde_json::json!({
                        "authorization_url": started.authorization_url,
                        "expires_at": started
                            .expires_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    }),
                )
            }
            ProfileOperation::DisconnectIdentityVerification => {
                let now = Utc::now();
                let verification = self
                    .identity()?
                    .disconnect()
                    .disconnect(context.actor(), now)
                    .await;
                json(200, verification.to_json())
            }
            ProfileOperation::SearchCatalog(category) => {
                let query = catalog_query(&request.http_target)?;
                let page = catalog::search(&self.pool, category, &query)
                    .await
                    .map_err(catalog_error)?;
                json(200, serde_json::to_value(&page).map_err(|_| {
                    Status::internal("Catalog projection")
                })?)
            }
        }
    }
}

/// Map a start refusal to a closed gRPC status.
///
/// A disallowed return path is a caller error; too many in-flight states is a
/// bounded refusal the client must wait out; everything else is the store or the
/// entropy source being unavailable. None of these messages carries a value from
/// the request.
fn start_error(error: StartError) -> Status {
    match error {
        StartError::ReturnPathNotAllowed => {
            Status::invalid_argument("Return path is not an allowed destination")
        }
        StartError::TooManyInFlight => {
            Status::resource_exhausted("Too many verifications are already in progress")
        }
        StartError::Entropy | StartError::Store => {
            Status::unavailable("Identity verification is unavailable")
        }
    }
}

/// The callback outcome, as the gateway needs it.
///
/// The callback is not a profile product operation: it arrives at the gateway
/// without a session, so there is no actor to delegate. The gateway therefore
/// calls it through the profile service's own gRPC surface
/// (`IdentityVerificationService/Complete`) rather than through this producer.
/// This function is the single place that maps a callback outcome to the
/// redirect the gateway will emit.
pub fn callback_redirect(outcome: &crate::identity::CallbackOutcome) -> (u32, String) {
    (303, callback_location(&outcome.return_path, outcome.result))
}

/// Build the `Location` for a callback outcome.
///
/// The path is the value stored at start time, which was validated against the
/// start allowlist. The marker is a closed string. Nothing else is appended, so
/// the redirect cannot carry a provider code, an age or a subject.
pub fn callback_location(return_path: &str, result: CallbackResult) -> String {
    let separator = if return_path.contains('?') { '&' } else { '?' };
    format!("{return_path}{separator}identity={}", result.as_str())
}

/// Map a callback error to a closed gRPC status.
pub fn callback_error(error: CallbackError) -> Status {
    match error {
        CallbackError::InvalidState => {
            Status::invalid_argument("Invalid identity verification state")
        }
        CallbackError::Store => Status::unavailable("Identity verification is unavailable"),
    }
}

/// The callback command from a provider query.
pub fn callback_command(
    state: &str,
    code: Option<&str>,
    provider_error: Option<&str>,
    now: chrono::DateTime<Utc>,
) -> CallbackCommand {
    CallbackCommand {
        state: state.to_owned(),
        code: code.map(str::to_owned),
        provider_error: provider_error.map(str::to_owned),
        now,
    }
}

/// Parse the frozen `core.CatalogQuery` from a target's query string.
///
/// The field set is closed (`q`, `cursor`, `limit`), a repeated field is a
/// client error, and an unknown field is refused rather than ignored. Value
/// bounds are enforced by `catalog::validate`.
fn catalog_query(target: &str) -> Result<catalog::CatalogQuery, Status> {
    let invalid = || Status::invalid_argument("Invalid catalog query");
    let mut q = None;
    let mut cursor = None;
    let mut limit = catalog::DEFAULT_LIMIT;
    let mut limit_seen = false;
    if let Some((_, query)) = target.split_once('?') {
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            match name {
                "q" => {
                    if q.is_some() {
                        return Err(invalid());
                    }
                    q = Some(value.to_owned());
                }
                "cursor" => {
                    if cursor.is_some() {
                        return Err(invalid());
                    }
                    cursor = Some(value.to_owned());
                }
                "limit" => {
                    if limit_seen {
                        return Err(invalid());
                    }
                    limit_seen = true;
                    limit = value.parse::<i64>().map_err(|_| invalid())?;
                }
                _ => return Err(invalid()),
            }
        }
    }
    catalog::validate(catalog::CatalogQuery { q, cursor, limit }).map_err(catalog_error)
}

/// Map a catalog refusal to a closed gRPC status. No message carries a value
/// from the request.
fn catalog_error(error: catalog::CatalogError) -> Status {
    match error {
        catalog::CatalogError::InvalidQuery(_) | catalog::CatalogError::InvalidCursor => {
            Status::invalid_argument("Invalid catalog query")
        }
        catalog::CatalogError::Unavailable => {
            Status::unavailable("Catalog store is unavailable")
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
