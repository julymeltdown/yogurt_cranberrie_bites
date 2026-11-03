use super::*;

pub type NotificationAdmissionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<NotificationAdmission, Denial>> + Send + 'a>>;
/// The production implementation must independently call authenticated Gateway,
/// refreshing Post source/current visibility and Friend policy for delivery.
/// A signature check or process-local decision cache cannot implement this.
pub trait NotificationAdmissionCheck: Send + Sync {
    fn check<'a>(&'a self, proof: &'a VerifiedNotification) -> NotificationAdmissionFuture<'a>;
}

/// Private construction prevents a handler from accepting a decoded scope as
/// admission. Only the guard installs this after fresh checks and consumption.
#[derive(Clone)]
pub struct AdmittedNotification {
    proof: VerifiedNotification,
    decision: NotificationDecision,
    expires_micros: i64,
}
impl AdmittedNotification {
    pub fn binding(&self) -> &NotificationBinding {
        self.proof.binding()
    }
    pub fn decision(&self) -> NotificationDecision {
        self.decision
    }
    pub fn expires_micros(&self) -> i64 {
        self.expires_micros
    }
}

#[derive(Clone)]
pub struct NotificationGuard<S> {
    inner: S,
    verifier: NotificationVerifier,
    admission: Arc<dyn NotificationAdmissionCheck>,
    write_service: bool,
}
impl<S: NamedService> NotificationGuard<S> {
    /// Original Notification write RPC is protected in place. Other methods
    /// retain their existing outer workload/actor policy, not this grant.
    pub fn new(
        inner: S,
        verifier: NotificationVerifier,
        admission: Arc<dyn NotificationAdmissionCheck>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            matches!(
                S::NAME,
                "notification.v1.NotificationService"
                    | "authority.v1.NotificationProjectionService"
            ),
            "Notification write or projection service required"
        );
        Ok(Self {
            inner,
            verifier,
            admission,
            write_service: S::NAME == "notification.v1.NotificationService",
        })
    }
}
impl<S: NamedService> NamedService for NotificationGuard<S> {
    const NAME: &'static str = S::NAME;
}
impl<S, B> tower::Service<http::Request<B>> for NotificationGuard<S>
where
    S: tower::Service<http::Request<Body>, Response = http::Response<Body>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    B: http_body::Body<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = http::Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let verifier = self.verifier.clone();
        let admission = self.admission.clone();
        let write = self.write_service;
        Box::pin(async move {
            if write && request.uri().path() != WRITE_METHOD {
                return inner.call(request.map(Body::new)).await;
            }
            let (mut parts, body) = request.into_parts();
            let Some(VerifiedCaller(caller)) = parts.extensions.get::<VerifiedCaller>().copied()
            else {
                return Ok(status(Denial::Missing).into_http());
            };
            if caller != Workload::Post
                || parts.uri.path() != if write { WRITE_METHOD } else { PAGE_METHOD }
            {
                return Ok(status(Denial::WrongIssuer).into_http());
            }
            let tokens: Vec<_> = parts.headers.get_all(HEADER).iter().collect();
            if tokens.len() != 1 {
                return Ok(status(Denial::Missing).into_http());
            }
            let Ok(token) = tokens[0].to_str() else {
                return Ok(status(Denial::Malformed).into_http());
            };
            let Ok(collected) = Limited::new(body, MAX_BYTES).collect().await else {
                return Ok(status(Denial::BodyTooLarge).into_http());
            };
            let body = collected.to_bytes();
            let proof = match verifier.verify(
                token,
                caller,
                parts.uri.path(),
                &body,
                chrono::Utc::now().timestamp_micros(),
            ) {
                Ok(proof) => proof,
                Err(error) => return Ok(status(error).into_http()),
            };
            let refreshed = match admission.check(&proof).await {
                Ok(d) => d,
                Err(e) => return Ok(status(e).into_http()),
            };
            let decision = refreshed.decision;
            let expires_micros = proof.expires_micros().min(refreshed.expires_micros);
            if let NotificationDecision::Allowed { visibility } = decision {
                if let Err(e) = proof.binding().visibility(visibility) {
                    return Ok(status(e).into_http());
                }
            }
            if let Err(e) = verifier.consume(&proof, expires_micros, || {
                chrono::Utc::now().timestamp_micros()
            }) {
                return Ok(status(e).into_http());
            }
            if write && decision == NotificationDecision::Suppressed {
                // Original CreateNotificationResponse: no notification, bool
                // suppressed(field2)=true. Never invoke the write handler.
                let mut trailers = http::HeaderMap::new();
                trailers.insert("grpc-status", http::HeaderValue::from_static("0"));
                let body = Full::new(tonic::codegen::Bytes::from_static(&[0, 0, 0, 0, 2, 16, 1]))
                    .with_trailers(std::future::ready(Some(Ok(trailers))));
                let mut response = http::Response::new(Body::new(body));
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static("application/grpc"),
                );
                return Ok(response);
            }
            parts.extensions.insert(AdmittedNotification {
                proof,
                decision,
                expires_micros,
            });
            inner
                .call(http::Request::from_parts(parts, Body::new(Full::new(body))))
                .await
        })
    }
}
pub fn status(error: Denial) -> Status {
    match error {
        Denial::ReplayCacheFull => Status::unavailable("Notification admission unavailable"),
        Denial::BodyTooLarge => Status::resource_exhausted("Notification request too large"),
        // Keep the stable prefix callers may match on, and carry the documented
        // stable denial code. Without it every reason - expired, replayed,
        // wrong audience, actor mismatch - reads as one indistinguishable
        // "denied" and a failure cannot be attributed.
        _ => Status::permission_denied(format!("Notification authority denied ({})", error.code())),
    }
}
