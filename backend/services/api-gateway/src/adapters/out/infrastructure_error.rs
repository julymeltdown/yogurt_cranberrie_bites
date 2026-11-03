//! Diagnostic objects are retained only inside the adapter that performs the work.
use crate::application::infrastructure_error::GatewayInfrastructureError;

pub fn classify_infrastructure_error(error: anyhow::Error) -> GatewayInfrastructureError {
    if let Some(controlled) = error.downcast_ref::<GatewayInfrastructureError>() {
        return *controlled;
    }
    if error.downcast_ref::<sqlx::Error>().is_some() {
        return GatewayInfrastructureError::Storage;
    }
    if error.downcast_ref::<serde_json::Error>().is_some() {
        return GatewayInfrastructureError::Encoding;
    }
    GatewayInfrastructureError::Unexpected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_text_cannot_forge_an_infrastructure_category() {
        for text in [
            "Storage",
            "database unavailable",
            "AUTH_BUSY",
            "permission denied",
        ] {
            assert_eq!(
                classify_infrastructure_error(anyhow::anyhow!(text)),
                GatewayInfrastructureError::Unexpected
            );
        }
        let encoding =
            serde_json::from_str::<serde_json::Value>("secret malformed JSON").unwrap_err();
        let error = classify_infrastructure_error(encoding.into());
        assert_eq!(error, GatewayInfrastructureError::Encoding);
        assert!(std::error::Error::source(&error).is_none());
        assert!(!format!("{error:?} {error}").contains("secret"));
        for category in [
            GatewayInfrastructureError::Storage,
            GatewayInfrastructureError::Encoding,
            GatewayInfrastructureError::Unexpected,
        ] {
            assert_eq!(
                classify_infrastructure_error(
                    anyhow::Error::new(category).context("secret-context")
                ),
                category
            );
        }
    }
}
