//! Stable effect identity for the coordinator's durable planned Profile reference.
use super::model::{Error, Version};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn effect_digest(
    binding_digest: [u8; 32],
    operation: Uuid,
    version: Version,
    raw_patch: &[u8],
) -> Result<[u8; 32], Error> {
    if binding_digest == [0; 32] || operation.is_nil() {
        return Err(Error::Invalid);
    }
    let mut hash = Sha256::new();
    hash.update(b"mcr profile self effect v1\0");
    hash.update(binding_digest);
    hash.update(operation.as_bytes());
    hash.update(version.epoch().as_bytes());
    hash.update(version.revision().to_be_bytes());
    hash.update(Sha256::digest(raw_patch));
    Ok(hash.finalize().into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_any_effect_input_changes_the_planned_identity() {
        let operation = Uuid::from_u128(1);
        let epoch = Uuid::from_u128(2);
        let version = Version::new(epoch, 3).unwrap();
        let original = effect_digest(
            [7; 32],
            operation,
            version,
            br#"{"expected_revision":1,"bio":null}"#,
        )
        .unwrap();
        for digest in [
            effect_digest(
                [8; 32],
                operation,
                version,
                br#"{"expected_revision":1,"bio":null}"#,
            )
            .unwrap(),
            effect_digest(
                [7; 32],
                Uuid::from_u128(4),
                version,
                br#"{"expected_revision":1,"bio":null}"#,
            )
            .unwrap(),
            effect_digest(
                [7; 32],
                operation,
                Version::new(Uuid::from_u128(5), 3).unwrap(),
                br#"{"expected_revision":1,"bio":null}"#,
            )
            .unwrap(),
            effect_digest(
                [7; 32],
                operation,
                Version::new(epoch, 4).unwrap(),
                br#"{"expected_revision":1,"bio":null}"#,
            )
            .unwrap(),
            effect_digest(
                [7; 32],
                operation,
                version,
                br#"{"expected_revision":1,"bio":""}"#,
            )
            .unwrap(),
        ] {
            assert_ne!(digest, original);
        }
        assert!(effect_digest([0; 32], operation, version, b"{}").is_err());
        assert!(effect_digest([7; 32], Uuid::nil(), version, b"{}").is_err());
    }
}
