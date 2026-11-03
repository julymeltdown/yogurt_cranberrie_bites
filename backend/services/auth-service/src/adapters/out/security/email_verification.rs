use crate::application::dto::verification_code_dto::VerificationCodeDto;
use crate::application::verification_crypto::VerificationCryptoPort;
use rand::RngCore;

#[derive(Clone, Copy, Default)]
pub struct SecureEmailVerification;

impl VerificationCryptoPort for SecureEmailVerification {
    fn generate(&self) -> crate::application::errors::AuthResult<VerificationCodeDto> {
        let result: anyhow::Result<_> = {
            sample_code(|raw| {
                rand::rngs::OsRng.try_fill_bytes(raw)?;
                Ok(())
            })
        };
        result.map_err(crate::adapters::out::errors::boundary_error)
    }
}

fn sample_code(
    mut fill: impl FnMut(&mut [u8]) -> anyhow::Result<()>,
) -> anyhow::Result<VerificationCodeDto> {
    // Rejection sampling preserves uniform six-digit challenges. A simple
    // modulo of all u32 values would bias the distribution.
    const VALUES: u32 = 1_000_000;
    const LIMIT: u32 = ((1_u64 << 32) / VALUES as u64 * VALUES as u64) as u32;
    for _ in 0..32 {
        let mut raw = [0_u8; 4];
        fill(&mut raw)?;
        let value = u32::from_le_bytes(raw);
        if value < LIMIT {
            return Ok(VerificationCodeDto {
                code: format!("{:06}", value % VALUES),
            });
        }
    }
    anyhow::bail!("verification entropy did not produce an admissible sample")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_sampling_preserves_bounds_leading_zeroes_and_redaction() {
        let mut samples = [u32::MAX, 0].into_iter();
        let code = sample_code(|raw| {
            raw.copy_from_slice(&samples.next().unwrap().to_le_bytes());
            Ok(())
        })
        .unwrap();
        assert_eq!(code.code, "000000");
        assert!(!format!("{code:?}").contains("000000"));
        assert_eq!(
            sample_code(|raw| {
                raw.copy_from_slice(&999_999_u32.to_le_bytes());
                Ok(())
            })
            .unwrap()
            .code,
            "999999"
        );
    }

    #[test]
    fn entropy_failure_and_repeated_rejection_are_bounded_errors() {
        assert!(sample_code(|_| anyhow::bail!("fixture entropy failure")).is_err());
        let mut calls = 0;
        assert!(sample_code(|raw| {
            calls += 1;
            raw.fill(255);
            Ok(())
        })
        .is_err());
        assert_eq!(calls, 32);
        let real = SecureEmailVerification.generate().unwrap();
        assert_eq!(real.code.len(), 6);
        assert!(real.code.bytes().all(|b| b.is_ascii_digit()));
    }
}
