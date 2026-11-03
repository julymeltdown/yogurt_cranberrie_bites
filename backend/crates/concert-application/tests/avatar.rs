use concert_application::avatar::*;
use concert_domain::{AuthUserId, EventId, ParticipantKey};
struct Authority(Result<AvatarGrant, AvatarError>);
impl AvatarAuthority for Authority {
    fn check(&self) -> Result<AvatarGrant, AvatarError> {
        self.0.clone()
    }
}
fn key() -> ParticipantKey {
    ParticipantKey {
        event_id: "00000000-0000-4000-8000-000000000101"
            .parse::<EventId>()
            .unwrap(),
        user_id: "00000000-0000-4000-8000-000000000201"
            .parse::<AuthUserId>()
            .unwrap(),
    }
}
#[test]
fn grant_binds_canonical_actor_event_operation_and_release() {
    let key = key();
    let grant = AvatarGrant {
        key,
        operation: AvatarOperation::SetAvatar,
        release_digest: "a".repeat(64),
    };
    assert!(authorize(
        &Authority(Ok(grant.clone())),
        key,
        AvatarOperation::SetAvatar,
        &"a".repeat(64)
    )
    .is_ok());
    assert_eq!(
        authorize(
            &Authority(Ok(grant.clone())),
            key,
            AvatarOperation::Read,
            &"a".repeat(64)
        ),
        Err(AvatarError::Denied)
    );
    assert_eq!(
        authorize(
            &Authority(Ok(grant.clone())),
            key,
            AvatarOperation::SetAvatar,
            &"b".repeat(64)
        ),
        Err(AvatarError::Unavailable)
    );
    for other in [
        ParticipantKey {
            user_id: "00000000-0000-4000-8000-000000000202".parse().unwrap(),
            ..key
        },
        ParticipantKey {
            event_id: "00000000-0000-4000-8000-000000000102".parse().unwrap(),
            ..key
        },
    ] {
        assert_eq!(
            authorize(
                &Authority(Ok(grant.clone())),
                other,
                AvatarOperation::SetAvatar,
                &"a".repeat(64)
            ),
            Err(AvatarError::Denied)
        );
    }
    for e in [
        AvatarError::Unauthenticated,
        AvatarError::Denied,
        AvatarError::ReadOnly,
        AvatarError::Unavailable,
    ] {
        assert_eq!(
            authorize(
                &Authority(Err(e)),
                key,
                AvatarOperation::SetAvatar,
                &"a".repeat(64)
            ),
            Err(e)
        );
    }
}
