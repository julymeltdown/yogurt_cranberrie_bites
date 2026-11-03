//! Pure presentation decisions. The inbound adapter owns wire codes/metadata.
use crate::dto::mutation_dto::MutationError;
use crate::errors::ProfileError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileFailureKind {
    InvalidInput,
    Missing,
    Denied,
    Busy,
    Internal,
    Conflict,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilePresentationMode {
    Input,
    Lookup,
    Mutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMutationReason {
    Conflict,
    Expired,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFailureDto {
    pub kind: ProfileFailureKind,
    pub message: String,
    pub mutation_reason: Option<ProfileMutationReason>,
}

pub fn present_profile_error(
    error: ProfileError,
    mode: ProfilePresentationMode,
) -> ProfileFailureDto {
    use ProfileFailureKind as Kind;
    use ProfilePresentationMode as Mode;
    let mut reason = None;
    let kind = match error {
        ProfileError::AccountDeleted => Kind::Denied,
        ProfileError::Busy => Kind::Busy,
        ProfileError::Persistence if mode == Mode::Mutation => Kind::Uncertain,
        ProfileError::Persistence => Kind::Internal,
        ProfileError::ReceiptUnavailable => Kind::Uncertain,
        ProfileError::ProfileNotFound if mode != Mode::Input => Kind::Missing,
        ProfileError::Forbidden if mode != Mode::Input => Kind::Denied,
        ProfileError::Mutation(mutation) if mode == Mode::Mutation => match mutation {
            MutationError::InvalidContext => Kind::InvalidInput,
            MutationError::Conflict | MutationError::Expired | MutationError::NotCommitted => {
                reason = Some(match mutation {
                    MutationError::Conflict => ProfileMutationReason::Conflict,
                    MutationError::Expired => ProfileMutationReason::Expired,
                    _ => ProfileMutationReason::Unknown,
                });
                Kind::Conflict
            }
            MutationError::Unavailable => Kind::Uncertain,
        },
        _ => Kind::InvalidInput,
    };
    let message = if kind == Kind::Uncertain {
        "Owner operation outcome unavailable".to_owned()
    } else {
        error.to_string()
    };
    ProfileFailureDto {
        kind,
        message,
        mutation_reason: reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_failure_matrix_preserves_operation_modes_and_receipt_reasons() {
        use ProfileFailureKind as K;
        use ProfilePresentationMode as M;
        for mode in [M::Input, M::Lookup, M::Mutation] {
            for (error, expected) in [
                (ProfileError::AccountDeleted, K::Denied),
                (ProfileError::Busy, K::Busy),
                (ProfileError::ReceiptUnavailable, K::Uncertain),
                (
                    ProfileError::InvalidInput("User ID is required"),
                    K::InvalidInput,
                ),
                (
                    ProfileError::ProfileNotFound,
                    if mode == M::Input {
                        K::InvalidInput
                    } else {
                        K::Missing
                    },
                ),
                (
                    ProfileError::Forbidden,
                    if mode == M::Input {
                        K::InvalidInput
                    } else {
                        K::Denied
                    },
                ),
                (
                    ProfileError::Persistence,
                    if mode == M::Mutation {
                        K::Uncertain
                    } else {
                        K::Internal
                    },
                ),
            ] {
                let presented = present_profile_error(error, mode);
                assert_eq!(presented.kind, expected);
                assert_eq!(presented.mutation_reason, None);
                assert_eq!(
                    presented.message,
                    if expected == K::Uncertain {
                        "Owner operation outcome unavailable".to_owned()
                    } else {
                        error.to_string()
                    }
                );
            }
        }
        for (error, reason) in [
            (MutationError::Conflict, ProfileMutationReason::Conflict),
            (MutationError::Expired, ProfileMutationReason::Expired),
            (MutationError::NotCommitted, ProfileMutationReason::Unknown),
        ] {
            let result = present_profile_error(error.into(), M::Mutation);
            assert_eq!(result.kind, K::Conflict);
            assert_eq!(result.mutation_reason, Some(reason));
            assert_eq!(result.message, error.to_string());
        }
        assert_eq!(
            present_profile_error(MutationError::Unavailable.into(), M::Mutation).kind,
            K::Uncertain
        );
        assert_eq!(
            present_profile_error(MutationError::InvalidContext.into(), M::Mutation).kind,
            K::InvalidInput
        );
    }
}
