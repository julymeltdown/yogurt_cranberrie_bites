use super::{proto, GrpcAuthClient};
use crate::application::dto::verification_dto::{
    ResendVerificationRequest, ResendVerificationResponse,
};
use crate::application::verification::{VerificationClientPort, VerificationError};
use crate::config::GatewayMetrics;
use async_trait::async_trait;
use std::future::Future;
use std::time::Duration;
use tonic::{Code, Request};

/// Reuses the authenticated Auth channel; never opens an unauthenticated fallback.
#[derive(Clone)]
pub struct GrpcVerificationClient {
    auth: GrpcAuthClient,
    timeout: Duration,
}

impl GrpcVerificationClient {
    pub fn new(auth: GrpcAuthClient, timeout: Duration) -> anyhow::Result<Self> {
        validate_timeout(timeout)?;
        Ok(Self { auth, timeout })
    }
}

fn validate_timeout(timeout: Duration) -> anyhow::Result<()> {
    anyhow::ensure!(
        (Duration::from_millis(1)..=Duration::from_secs(60)).contains(&timeout),
        "Verification deadline must be between 1ms and 60s"
    );
    Ok(())
}

#[async_trait]
impl VerificationClientPort for GrpcVerificationClient {
    async fn resend_verification(
        &self,
        request: ResendVerificationRequest,
    ) -> Result<ResendVerificationResponse, VerificationError> {
        let email = request.email.trim();
        if email.is_empty() || email.len() > 254 {
            return Err(VerificationError::InvalidEmail);
        }
        let mut request = Request::new(proto::ResendVerificationRequest {
            email: email.to_owned(),
        });
        // The header informs the server; the local timeout also bounds waiting
        // for channel readiness and a server which ignores deadline metadata.
        request.set_timeout(self.timeout);
        let mut client = self.auth.client.clone();
        let response = await_response(
            self.timeout,
            self.auth.metrics.clone(),
            client.resend_verification(request),
        )
        .await?;
        if response.status != "ACCEPTED" {
            tracing::error!("verification RPC returned an unexpected status");
            return Err(VerificationError::InvalidResponse);
        }
        Ok(ResendVerificationResponse::accepted())
    }
}

async fn await_response(
    timeout: Duration,
    metrics: GatewayMetrics,
    call: impl Future<Output = Result<tonic::Response<proto::ResendVerificationResponse>, tonic::Status>>
        + Send,
) -> Result<proto::ResendVerificationResponse, VerificationError> {
    let result = match tokio::time::timeout(timeout, call).await {
        Ok(result) => result,
        Err(_) => {
            metrics
                .edge_deadline_metrics()
                .record_deadline_exhausted(platform::edge::Edge::GatewayAuth);
            return Err(VerificationError::Timeout);
        }
    };
    result.map(tonic::Response::into_inner).map_err(|status| {
        if status.code() == Code::DeadlineExceeded {
            metrics
                .edge_deadline_metrics()
                .record_deadline_exhausted(platform::edge::Edge::GatewayAuth);
        }
        // Raw upstream messages/details can contain SQL or account information.
        tracing::warn!(code = ?status.code(), "verification RPC failed");
        map_code(status.code())
    })
}

fn map_code(code: Code) -> VerificationError {
    match code {
        Code::InvalidArgument => VerificationError::InvalidEmail,
        Code::DeadlineExceeded => VerificationError::Timeout,
        // An old Auth deployment may not yet have this additive method. Never
        // interpret UNIMPLEMENTED, cancellation or capacity failure as ACCEPTED.
        Code::Unavailable | Code::Unimplemented | Code::ResourceExhausted | Code::Cancelled => {
            VerificationError::Unavailable
        }
        _ => VerificationError::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_expiry_is_reported_as_timeout() {
        let metrics = GatewayMetrics::default();
        let call = std::future::pending::<
            Result<tonic::Response<proto::ResendVerificationResponse>, tonic::Status>,
        >();
        assert!(matches!(
            await_response(Duration::from_millis(1), metrics.clone(), call).await,
            Err(VerificationError::Timeout)
        ));
        assert!(metrics
            .render_prometheus()
            .contains("grpc_edge_deadline_exhaustions_total{edge=\"gateway->auth\"} 1"));
    }

    #[test]
    fn deadlines_are_finite_and_positive() {
        assert!(validate_timeout(Duration::ZERO).is_err());
        assert!(validate_timeout(Duration::from_nanos(1)).is_err());
        assert!(validate_timeout(Duration::from_millis(1)).is_ok());
        assert!(validate_timeout(Duration::from_secs(60)).is_ok());
        assert!(validate_timeout(Duration::from_secs(61)).is_err());
    }

    #[test]
    fn rpc_failure_codes_are_not_success_or_raw_messages() {
        assert_eq!(
            map_code(Code::InvalidArgument),
            VerificationError::InvalidEmail
        );
        assert_eq!(map_code(Code::DeadlineExceeded), VerificationError::Timeout);
        for code in [
            Code::Unavailable,
            Code::Unimplemented,
            Code::ResourceExhausted,
            Code::Cancelled,
        ] {
            assert_eq!(map_code(code), VerificationError::Unavailable);
        }
        for code in [
            Code::Internal,
            Code::Unauthenticated,
            Code::PermissionDenied,
            Code::Unknown,
        ] {
            assert_eq!(map_code(code), VerificationError::Internal);
        }
    }
}
