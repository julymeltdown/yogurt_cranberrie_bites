//! Library error classification belongs to the outbound adapters.
use crate::application::dto::mutation_dto::MutationError;
use crate::application::errors::ProfileError;

pub(super) fn repository_error(error: anyhow::Error) -> ProfileError {
    if error.chain().any(|cause| matches!(cause.downcast_ref::<sqlx::Error>(),
        Some(sqlx::Error::Database(database)) if database.constraint() == Some("profile_user_deletion_fence"))) {
        return ProfileError::AccountDeleted;
    }
    if let Some(known) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<ProfileError>())
    {
        return *known;
    }
    if let Some(mutation) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<MutationError>())
    {
        return ProfileError::Mutation(*mutation);
    }
    if error.chain().any(|cause| cause.is::<serde_json::Error>()) {
        return ProfileError::ReceiptUnavailable;
    }
    ProfileError::Persistence
}

pub(super) fn media_error(error: anyhow::Error) -> ProfileError {
    if error
        .chain()
        .any(|cause| cause.is::<platform::cpu::CapacityExhausted>())
    {
        return ProfileError::Busy;
    }
    if let Some(known) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<ProfileError>())
    {
        return *known;
    }
    // Library detail remains inside the adapter; only these controlled contexts
    // or fixed policy messages are allowed to leave it.
    let message = match error.to_string().as_str() {
        "Avatar content type is required" => "Avatar content type is required",
        "Avatar content type is unsupported" => "Avatar content type is unsupported",
        "Avatar content is empty" => "Avatar content is empty",
        "Avatar is not a decodable image" => "Avatar is not a decodable image",
        "Avatar dimensions cannot be read" => "Avatar dimensions cannot be read",
        "Avatar has no usable dimensions" => "Avatar has no usable dimensions",
        "Avatar could not be decoded as the image type it declares" => {
            "Avatar could not be decoded as the image type it declares"
        }
        "Avatar re-encoding produced no data" => "Avatar re-encoding produced no data",
        "Avatar could not be re-encoded within the configured size limit" => {
            "Avatar could not be re-encoded within the configured size limit"
        }
        "Avatar format is unsupported" => "Avatar format is unsupported",
        "Avatar content does not match its declared image type" => {
            "Avatar content does not match its declared image type"
        }
        _ => "Avatar could not be processed",
    };
    ProfileError::InvalidInput(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_classification_sanitizes_wrapped_database_json_cpu_and_codec_details() {
        let sql: anyhow::Error = sqlx::Error::PoolTimedOut.into();
        let error = repository_error(sql.context("private database password/host"));
        assert_eq!(error, ProfileError::Persistence);
        let json = serde_json::from_str::<serde_json::Value>("private-malformed-json").unwrap_err();
        assert_eq!(
            repository_error(json.into()),
            ProfileError::ReceiptUnavailable
        );
        assert_eq!(
            repository_error(
                anyhow::Error::new(MutationError::Conflict).context("private-command")
            ),
            ProfileError::Mutation(MutationError::Conflict)
        );
        assert_eq!(
            media_error(
                anyhow::Error::new(platform::cpu::CapacityExhausted).context("private-filename")
            ),
            ProfileError::Busy
        );
        let codec = media_error(anyhow::anyhow!("private codec filename/payload"));
        assert_eq!(
            codec,
            ProfileError::InvalidInput("Avatar could not be processed")
        );
        assert!(!format!("{error:?} {codec:?}").contains("private"));
    }
}
