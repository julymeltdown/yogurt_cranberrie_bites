//! The closed profile route set served by the concert service, and the producer
//! that answers it.
//!
//! Fifteen frozen profile operations read concert-owned tables. Their producers
//! already exist in this crate:
//!
//! | operations | module |
//! | --- | --- |
//! | `listProfilePosts`, `createProfilePost`, `deleteProfilePost` | [`crate::profile_posts`] |
//! | `initProfileMediaUpload`, `putProfileMediaBytes`, `finalizeProfileMedia`, `getProfileMedia`, `readProfileMedia` | [`crate::profile_media`] |
//! | `getProfileRoom`, `getOwnRoom`, `saveOwnRoom`, `setOwnRoomVisibility`, `uploadRoomPoster` | [`crate::profile_room`] |
//! | `getOwnProfileAvatar`, `setOwnProfileAvatar` | [`crate::profile_avatar`] |
//!
//! This module is their transport home. It adds **no** product behaviour: it
//! parses the target with the module's own closed parser and then calls the
//! module's own `handle`/command function. What it does add is the thing a bare
//! persistence call does not supply — the operation is resolved from a closed
//! table, so an operation whose tables live in the *profile* database is refused
//! here rather than answered from the wrong database.
//!
//! The table is closed in both directions: a `(method, path)` outside it is not a
//! concert-hosted profile operation, and a profile operation whose tables live in
//! `mcr_profile` (`getOwnProfile`, `getProfile`, `searchCatalog`, the identity
//! operations, …) is deliberately absent, so this service can never answer it.
use super::{proto, transport::VerifiedProfile};
use crate::profile_avatar::repository::{
    ProfileAvatarActor, ProfileAvatarPersistence,
};
use crate::profile_media::upload::{ProfileMediaId, ProfileMediaStore, UploadPurpose};
use crate::profile_posts::{self, commands::CreateProfilePost, read::ProfilePostQuery};
use crate::profile_room::repository::{RoomActor, RoomPersistence};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

/// The closed set of profile operations this service serves.
///
/// Every variant names a concert-owned table set. An operation whose tables live
/// in the profile database is not a variant, so it cannot be dispatched here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileOperation {
    /// `listProfilePosts` — `profile_posts`, `profile_post_attachments`.
    ListProfilePosts { profile_id: Uuid },
    /// `createProfilePost` — `profile_posts`.
    CreateProfilePost,
    /// `deleteProfilePost` — `profile_posts`.
    DeleteProfilePost { post_id: Uuid },
    /// `initProfileMediaUpload` — `profile_media`.
    InitProfileMediaUpload,
    /// `putProfileMediaBytes` — `profile_media`.
    PutProfileMediaBytes { media_id: Uuid },
    /// `finalizeProfileMedia` — `profile_media`.
    FinalizeProfileMedia { media_id: Uuid },
    /// `getProfileMedia` — `profile_media`.
    GetProfileMedia { media_id: Uuid },
    /// `readProfileMedia` — `profile_media`.
    ReadProfileMedia { media_id: Uuid },
    /// `getProfileRoom` — `profile_rooms`.
    GetProfileRoom { profile_id: Uuid },
    /// `getOwnRoom` — `profile_rooms`.
    GetOwnRoom,
    /// `saveOwnRoom` — `profile_rooms`.
    SaveOwnRoom,
    /// `setOwnRoomVisibility` — `profile_rooms`.
    SetOwnRoomVisibility,
    /// `uploadRoomPoster` — `profile_rooms`, `profile_media`.
    UploadRoomPoster,
    /// `getOwnProfileAvatar` — `profile_avatars`.
    GetOwnProfileAvatar,
    /// `setOwnProfileAvatar` — `profile_avatars`.
    SetOwnProfileAvatar,
    /// One of WP-05's intro operations this transport serves. The intro
    /// producer owns the dispatch; this variant is the routing marker so the
    /// closed table below stays the single statement of what is served.
    ///
    /// `listIntroPosts` and `getIntroPost` are deliberately absent: they need a
    /// visibility-filtered query that WP-04 and WP-05 have not built yet, so
    /// they are refused by `parse` rather than served by a query this transport
    /// does not own.
    Intro(&'static str),
    /// `listNotifications` — the recipient's own inbox over
    /// `intro_like_notifications`.
    ListNotifications,
    /// `markNotificationRead` — records that the recipient opened one.
    MarkNotificationRead { notification_id: Uuid },
}

impl ProfileOperation {
    /// The frozen operation id.
    pub fn contract_id(self) -> &'static str {
        match self {
            Self::ListProfilePosts { .. } => "listProfilePosts",
            Self::CreateProfilePost => "createProfilePost",
            Self::DeleteProfilePost { .. } => "deleteProfilePost",
            Self::InitProfileMediaUpload => "initProfileMediaUpload",
            Self::PutProfileMediaBytes { .. } => "putProfileMediaBytes",
            Self::FinalizeProfileMedia { .. } => "finalizeProfileMedia",
            Self::GetProfileMedia { .. } => "getProfileMedia",
            Self::ReadProfileMedia { .. } => "readProfileMedia",
            Self::GetProfileRoom { .. } => "getProfileRoom",
            Self::GetOwnRoom => "getOwnRoom",
            Self::SaveOwnRoom => "saveOwnRoom",
            Self::SetOwnRoomVisibility => "setOwnRoomVisibility",
            Self::UploadRoomPoster => "uploadRoomPoster",
            Self::GetOwnProfileAvatar => "getOwnProfileAvatar",
            Self::SetOwnProfileAvatar => "setOwnProfileAvatar",
            Self::Intro(operation) => operation,
            Self::ListNotifications => "listNotifications",
            Self::MarkNotificationRead { .. } => "markNotificationRead",
        }
    }

    /// Whether this operation changes state.
    pub fn mutation(self) -> bool {
        matches!(
            self,
            Self::CreateProfilePost
                | Self::DeleteProfilePost { .. }
                | Self::InitProfileMediaUpload
                | Self::PutProfileMediaBytes { .. }
                | Self::FinalizeProfileMedia { .. }
                | Self::SaveOwnRoom
                | Self::SetOwnRoomVisibility
                | Self::UploadRoomPoster
                | Self::SetOwnProfileAvatar
                | Self::MarkNotificationRead { .. }
        )
    }

    /// Whether this operation mutates state, consulting WP-05's own parser for
    /// the intro operations so the two cannot disagree.
    pub fn is_mutation(self, method: &str, target: &str) -> bool {
        match self {
            Self::Intro(_) => crate::intro_posts::http::parse(method, target)
                .map(|parsed| parsed.mutation())
                .unwrap_or(false),
            other => other.mutation(),
        }
    }
}

/// Parse a profile target into one of the fifteen operations this service
/// serves.
///
/// The target is first rejected if it names an event, then handed to the owning
/// module's own closed parser. An operation the module does not recognise — and
/// every profile operation whose tables live in the profile database — is
/// `permission_denied`, which the gateway renders as a closed `403`.
pub fn parse(method: &str, target: &str) -> Result<ProfileOperation, Status> {
    // The event guard runs on the raw target, before any module parser, so a
    // percent-encoded or nested `/events/` segment cannot reach a producer.
    if super::transport::rejects_event_path(target) {
        return Err(Status::invalid_argument(
            "Profile operations never carry an event path",
        ));
    }
    if target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(Status::invalid_argument("Invalid profile target"));
    }
    let (path, _query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if !path.starts_with("/api/") {
        return Err(Status::permission_denied("Unsupported profile operation"));
    }

    // Profile posts and profile media. The module parser owns both families and
    // refuses a query on the operations that carry none: `route` validates the
    // query whereas `parse_operation` only resolves the route, and a query on an
    // operation that does not carry one is a caller error rather than a
    // silently ignored payload.
    if profile_posts::http::protects(path) {
        if let Ok((route, _query)) = profile_posts::http::route(method, target) {
            return Ok(match route {
                profile_posts::http::ProfilePostRoute::ListPosts { profile_id } => {
                    ProfileOperation::ListProfilePosts { profile_id }
                }
                profile_posts::http::ProfilePostRoute::CreatePost => {
                    ProfileOperation::CreateProfilePost
                }
                profile_posts::http::ProfilePostRoute::DeletePost { post_id } => {
                    ProfileOperation::DeleteProfilePost { post_id }
                }
                profile_posts::http::ProfilePostRoute::InitMediaUpload => {
                    ProfileOperation::InitProfileMediaUpload
                }
                profile_posts::http::ProfilePostRoute::PutMediaBytes { media_id } => {
                    ProfileOperation::PutProfileMediaBytes { media_id }
                }
                profile_posts::http::ProfilePostRoute::FinalizeMedia { media_id } => {
                    ProfileOperation::FinalizeProfileMedia { media_id }
                }
                profile_posts::http::ProfilePostRoute::GetMedia { media_id } => {
                    ProfileOperation::GetProfileMedia { media_id }
                }
                profile_posts::http::ProfilePostRoute::ReadMedia { media_id } => {
                    ProfileOperation::ReadProfileMedia { media_id }
                }
            });
        }
    }

    // The room. Its parser refuses a query and an intro path.
    if let Ok(route) = crate::profile_room::http::route(method, target) {
        return Ok(match route {
            crate::profile_room::RoomRoute::Own => ProfileOperation::GetOwnRoom,
            crate::profile_room::RoomRoute::Save => ProfileOperation::SaveOwnRoom,
            crate::profile_room::RoomRoute::Visibility => ProfileOperation::SetOwnRoomVisibility,
            crate::profile_room::RoomRoute::Poster => ProfileOperation::UploadRoomPoster,
            crate::profile_room::RoomRoute::ProfileRoom(profile) => {
                ProfileOperation::GetProfileRoom {
                    profile_id: profile.uuid(),
                }
            }
        });
    }

    // The avatar. Two operations, both event-free.
    if let Ok(route) = crate::profile_avatar::http::route(method, target) {
        return Ok(match route {
            crate::profile_avatar::http::ProfileAvatarRoute::Get => {
                ProfileOperation::GetOwnProfileAvatar
            }
            crate::profile_avatar::http::ProfileAvatarRoute::Put => {
                ProfileOperation::SetOwnProfileAvatar
            }
        });
    }

    // The notification inbox. Two operations, both addressed by the verified
    // session: the list reads the caller's own rows, and the mark names one of
    // them. A notification belongs to a profile, not to an event, so neither
    // path carries an event segment.
    if method == "GET" && path == "/api/me/notifications" {
        return Ok(ProfileOperation::ListNotifications);
    }
    if method == "PATCH" {
        if let Some(id) = path.strip_prefix("/api/me/notifications/") {
            if !id.contains('/') {
                return Ok(ProfileOperation::MarkNotificationRead {
                    notification_id: Uuid::parse_str(id)
                        .map_err(|_| Status::invalid_argument("Invalid notification id"))?,
                });
            }
        }
    }

    // WP-05's intro operations. Its own closed parser decides what is an intro
    // operation; this transport only decides whether it serves it. The two
    // reads that need a visibility-filtered query are refused here rather than
    // dispatched to a producer that would have to invent one.
    if let Ok(parsed) = crate::intro_posts::http::parse(method, target) {
        let operation = parsed.contract_id();
        if super::intro::is_served(operation) {
            return Ok(ProfileOperation::Intro(operation));
        }
        return Err(Status::permission_denied(
            "This intro operation is not served by the profile transport",
        ));
    }

    // Anything else is not a concert-hosted profile operation. That includes
    // every operation whose tables live in the profile database, so this service
    // can never answer one from the wrong database.
    Err(Status::permission_denied("Unsupported profile operation"))
}

/// The local producer. It performs real local work under the held scope and
/// returns a representation; it never mints authority or completes the scope.
pub struct ProfileProductProducer {
    pool: sqlx::PgPool,
    rooms: Arc<RoomPersistence>,
    avatars: Option<Arc<ProfileAvatarPersistence>>,
    media: Option<Arc<ProfileMediaStore>>,
    intro: Option<Arc<super::intro::IntroProducer>>,
    notifications: Option<Arc<crate::intro_posts::notifications::NotificationInbox>>,
}

impl ProfileProductProducer {
    /// The producer with room and post support. The avatar and media stores are
    /// attached separately because each needs a validated release or a
    /// sanitizer that the composition supplies.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            rooms: RoomPersistence::new(pool.clone()),
            pool,
            avatars: None,
            media: None,
            intro: None,
            notifications: None,
        }
    }

    /// Attach the notification inbox.
    pub fn with_notifications(
        mut self,
        notifications: Arc<crate::intro_posts::notifications::NotificationInbox>,
    ) -> Self {
        self.notifications = Some(notifications);
        self
    }

    /// Attach WP-05's intro producer.
    ///
    /// The intro stores are built over the same pool this producer already
    /// holds, so the transport never opens a second connection pool for them.
    pub fn with_intro(mut self, intro: Arc<super::intro::IntroProducer>) -> Self {
        self.intro = Some(intro);
        self
    }

    fn intro(&self) -> Result<&Arc<super::intro::IntroProducer>, Status> {
        self.intro
            .as_ref()
            .ok_or_else(|| Status::unavailable("Intro posts are unavailable"))
    }

    /// Attach the avatar persistence, which is bound to one validated release.
    pub fn with_avatars(mut self, avatars: Arc<ProfileAvatarPersistence>) -> Self {
        self.avatars = Some(avatars);
        self
    }

    /// Attach the profile media store, which is bound to one sanitizer.
    pub fn with_media(mut self, media: Arc<ProfileMediaStore>) -> Self {
        self.media = Some(media);
        self
    }

    /// The notification inbox, over `intro_like_notifications`.
    fn notifications(&self) -> Result<&Arc<crate::intro_posts::notifications::NotificationInbox>, Status> {
        self.notifications
            .as_ref()
            .ok_or_else(|| Status::unavailable("Notification inbox is unavailable"))
    }

    fn avatars(&self) -> Result<&Arc<ProfileAvatarPersistence>, Status> {
        self.avatars
            .as_ref()
            .ok_or_else(|| Status::unavailable("Profile avatar is unavailable"))
    }

    fn media(&self) -> Result<&Arc<ProfileMediaStore>, Status> {
        self.media
            .as_ref()
            .ok_or_else(|| Status::unavailable("Profile media is unavailable"))
    }
}

#[async_trait::async_trait]
impl super::scoped::ScopedProfileProducer for ProfileProductProducer {
    async fn dispatch(
        &self,
        context: &VerifiedProfile,
        _scope: Arc<super::guard::ProfileOwnerScope>,
        request: proto::ProfileRequest,
    ) -> Result<super::scoped::ProfileRepresentation, Status> {
        let operation = parse(&request.http_method, &request.http_target)?;
        let actor = context.profile();
        let target = request.http_target.as_str();
        match operation {
            ProfileOperation::ListProfilePosts { profile_id } => {
                let query = query_of(target)?;
                let page = profile_posts::read::list_profile_posts(
                    &self.pool,
                    actor,
                    profile_id,
                    &query,
                )
                .await
                .map_err(post_status)?;
                json(200, page.to_wire())
            }
            ProfileOperation::CreateProfilePost => {
                let command = CreateProfilePost::parse(&request.http_body)
                    .map_err(post_status)?;
                let created = profile_posts::commands::create_profile_post(
                    &self.pool, actor, &command,
                )
                .await
                .map_err(post_status)?;
                json(
                    200,
                    serde_json::json!({
                        "post_id": created.post_id,
                        "author_profile_id": created.author_profile_id,
                        "description": created.description,
                        "visibility": created.visibility.as_str(),
                        "revision": created.revision,
                        "created_at": created.created_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                        "expires_at": created.expires_at.map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
                        "media_ids": created.media_ids,
                    }),
                )
            }
            ProfileOperation::DeleteProfilePost { post_id } => {
                let revision = profile_posts::http::delete_revision(target)
                    .map_err(|refusal| Status::invalid_argument(refusal.reason))?;
                let deleted = profile_posts::commands::delete_profile_post(
                    &self.pool, actor, post_id, revision,
                )
                .await
                .map_err(post_status)?;
                json(200, deleted.to_wire())
            }
            ProfileOperation::InitProfileMediaUpload => {
                let input = profile_posts::http::parse_init_upload(
                    &request.http_body,
                    UploadPurpose::ProfilePost,
                )
                .map_err(|refusal| Status::invalid_argument(refusal.reason))?;
                let intent = self
                    .media()?
                    .init(actor, UploadPurpose::ProfilePost, input)
                    .await
                    .map_err(media_status)?;
                json(200, intent.to_wire())
            }
            ProfileOperation::PutProfileMediaBytes { media_id } => {
                let mime = profile_posts::http::parse_bytes_content_type(&request.content_type)
                    .map_err(|refusal| Status::invalid_argument(refusal.reason))?;
                let revision = self
                    .media()?
                    .put(
                        actor,
                        UploadPurpose::ProfilePost,
                        ProfileMediaId::from_uuid(media_id).map_err(media_status)?,
                        mime,
                        request.http_body.clone(),
                    )
                    .await
                    .map_err(media_status)?;
                json(200, serde_json::json!({"revision": revision}))
            }
            ProfileOperation::FinalizeProfileMedia { media_id } => {
                let revision = profile_posts::http::parse_finalize_upload(&request.http_body)
                    .map_err(|refusal| Status::invalid_argument(refusal.reason))?;
                let upload = self
                    .media()?
                    .finalize(
                        actor,
                        UploadPurpose::ProfilePost,
                        ProfileMediaId::from_uuid(media_id).map_err(media_status)?,
                        revision,
                    )
                    .await
                    .map_err(media_status)?;
                json(200, upload.to_wire())
            }
            ProfileOperation::GetProfileMedia { media_id } => {
                let upload = self
                    .media()?
                    .metadata(actor, ProfileMediaId::from_uuid(media_id).map_err(media_status)?)
                    .await
                    .map_err(media_status)?;
                json(200, upload.to_wire())
            }
            ProfileOperation::ReadProfileMedia { media_id } => {
                let (mime, bytes) = self
                    .media()?
                    .content(actor, ProfileMediaId::from_uuid(media_id).map_err(media_status)?)
                    .await
                    .map_err(media_status)?;
                Ok(super::scoped::ProfileRepresentation {
                    status: 200,
                    content_type: mime.as_str().to_owned(),
                    body: bytes,
                })
            }
            ProfileOperation::GetOwnRoom => {
                let response = crate::profile_room::http::handle(
                    &self.rooms,
                    RoomActor::new(profile_id(actor)?),
                    "GET",
                    "/api/me/room",
                    &[],
                )
                .await;
                json(response.status as u32, response.body)
            }
            ProfileOperation::SaveOwnRoom
            | ProfileOperation::SetOwnRoomVisibility
            | ProfileOperation::UploadRoomPoster => {
                let response = crate::profile_room::http::handle(
                    &self.rooms,
                    RoomActor::new(profile_id(actor)?),
                    &request.http_method,
                    target,
                    &request.http_body,
                )
                .await;
                json(response.status as u32, response.body)
            }
            ProfileOperation::GetProfileRoom { profile_id: target_profile } => {
                let response = crate::profile_room::http::handle(
                    &self.rooms,
                    RoomActor::new(profile_id(actor)?),
                    "GET",
                    &format!("/api/profiles/{target_profile}/room"),
                    &[],
                )
                .await;
                json(response.status as u32, response.body)
            }
            ProfileOperation::GetOwnProfileAvatar | ProfileOperation::SetOwnProfileAvatar => {
                let response = crate::profile_avatar::http::handle(
                    self.avatars()?,
                    ProfileAvatarActor::new(profile_id(actor)?),
                    &request.http_method,
                    target,
                    &request.http_body,
                )
                .await;
                json(response.status as u32, response.body)
            }
            ProfileOperation::ListNotifications => {
                // The recipient is the verified actor, never a request value, so
                // a caller cannot read another profile's inbox.
                let query = crate::intro_posts::notifications::NotificationQuery::parse(
                    target.split_once('?').map(|(_, query)| query),
                )
                .map_err(notification_status)?;
                let page = self
                    .notifications()?
                    .list(profile_id(actor)?.uuid(), &query)
                    .await
                    .map_err(notification_status)?;
                json(
                    200,
                    crate::intro_posts::notifications::page_wire(&page),
                )
            }
            ProfileOperation::MarkNotificationRead { notification_id } => {
                let notification = self
                    .notifications()?
                    .mark_read(profile_id(actor)?.uuid(), notification_id)
                    .await
                    .map_err(notification_status)?;
                json(
                    200,
                    crate::intro_posts::notifications::notification_wire(&notification),
                )
            }
            ProfileOperation::Intro(_) => {
                // WP-05's producer owns the whole intro dispatch. This transport
                // contributes only the verified context and the closed route
                // decision above.
                self.intro()?.dispatch(context, &request).await
            }
        }
    }
}

/// A canonical profile id from the verified actor.
fn profile_id(value: Uuid) -> Result<concert_domain::profile_room::ProfileId, Status> {
    concert_domain::profile_room::ProfileId::from_uuid(value)
        .map_err(|_| Status::invalid_argument("Invalid profile identity"))
}

/// The page query of `listProfilePosts`, parsed by the module's own parser.
fn query_of(target: &str) -> Result<ProfilePostQuery, Status> {
    profile_posts::http::route("GET", target)
        .map(|(_, query)| query)
        .map_err(|refusal| Status::invalid_argument(refusal.reason))
}

fn json(status: u32, body: serde_json::Value) -> Result<super::scoped::ProfileRepresentation, Status> {
    Ok(super::scoped::ProfileRepresentation {
        status,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&body).map_err(|_| Status::internal("Profile projection"))?,
    })
}

fn post_status(error: profile_posts::ProfilePostError) -> Status {
    match error.status() {
        400 => Status::invalid_argument(error.reason()),
        403 => Status::permission_denied(error.reason()),
        404 => Status::not_found(error.reason()),
        409 => Status::already_exists(error.reason()),
        413 => Status::resource_exhausted(error.reason()),
        415 => Status::invalid_argument(error.reason()),
        _ => Status::unavailable("Profile posts unavailable"),
    }
}

/// Map an inbox refusal to a closed gRPC status.
///
/// `Unavailable` is one variant for "no such notification" and "not yours", so
/// the transport cannot distinguish them either.
fn notification_status(
    error: crate::intro_posts::notifications::NotificationError,
) -> Status {
    use crate::intro_posts::notifications::NotificationError as E;
    match error {
        E::InvalidQuery => Status::invalid_argument("Invalid notification query"),
        E::InvalidCursor => Status::invalid_argument("Invalid notification cursor"),
        E::Unavailable => Status::not_found("Notification unavailable"),
        E::Store => Status::unavailable("Notification inbox unavailable"),
    }
}

fn media_status(error: crate::profile_media::upload::ProfileMediaError) -> Status {
    use crate::profile_media::upload::ProfileMediaError as E;
    match error {
        E::Invalid | E::PurposeMismatch | E::BytesMismatch | E::UnsupportedMedia => {
            Status::invalid_argument(error.reason())
        }
        E::Denied => Status::permission_denied(error.reason()),
        // `Unavailable` is deliberately one variant for "absent" and "not
        // yours", so the transport must not distinguish them either.
        E::Unavailable | E::Expired => Status::not_found(error.reason()),
        E::InvalidState => Status::already_exists(error.reason()),
        E::TooLarge => Status::resource_exhausted(error.reason()),
        E::Unavailable503 => Status::unavailable("Profile media unavailable"),
    }
}
