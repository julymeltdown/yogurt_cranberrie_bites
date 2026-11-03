use std::path::PathBuf;

pub fn proto_root() -> PathBuf {
    // Build dependencies can be reused from a shared target directory after
    // their original checkout is archived. Resolve the calling build script's
    // workspace instead of embedding this dependency's compilation path.
    PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("Cargo must provide the current build script's manifest directory"),
    )
    .join("../../proto")
}

pub fn source_auth_proto() -> PathBuf {
    proto_root().join("auth/v1/auth.proto")
}

pub fn source_auth_include_dir() -> PathBuf {
    proto_root()
}

pub fn source_profile_proto() -> PathBuf {
    proto_root().join("profile/v1/profile.proto")
}

pub fn source_profile_include_dir() -> PathBuf {
    proto_root()
}

pub fn source_friend_proto() -> PathBuf {
    proto_root().join("friend/v1/friend.proto")
}

pub fn source_friend_include_dir() -> PathBuf {
    proto_root()
}

pub fn source_notification_proto() -> PathBuf {
    proto_root().join("notification/v1/notification.proto")
}

pub fn source_notification_include_dir() -> PathBuf {
    proto_root()
}

pub fn source_post_proto() -> PathBuf {
    proto_root().join("post/v1/post.proto")
}

pub fn source_post_include_dir() -> PathBuf {
    proto_root()
}

pub fn source_authority_proto() -> PathBuf {
    proto_root().join("authority/v1/authority.proto")
}

pub fn source_concert_proto() -> PathBuf {
    proto_root().join("concert/v1/concert.proto")
}
