use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::{Request, Status};

const SERVICE_TOKEN_HEADER: &str = "x-service-token";

/// Header carrying the calling workload's own identity.
///
/// The caller name is not a secret and is never trusted on its own. It selects
/// which credential the receiving service must match, so a credential stolen
/// from one service cannot be replayed as another.
pub const SERVICE_CALLER_HEADER: &str = "x-service-caller";

/// A workload that may call an internal gRPC service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Workload {
    Gateway,
    Auth,
    Post,
    Profile,
    Friend,
    Notification,
    Concert,
}

impl Workload {
    pub const ALL: [Workload; 7] = [
        Workload::Gateway,
        Workload::Auth,
        Workload::Post,
        Workload::Profile,
        Workload::Friend,
        Workload::Notification,
        Workload::Concert,
    ];

    /// Stable wire identifier. Lowercase and ASCII so it satisfies HTTP/2 metadata.
    pub fn as_str(self) -> &'static str {
        match self {
            Workload::Gateway => "gateway",
            Workload::Auth => "auth",
            Workload::Post => "post",
            Workload::Profile => "profile",
            Workload::Friend => "friend",
            Workload::Notification => "notification",
            Workload::Concert => "concert",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "gateway" => Some(Workload::Gateway),
            "auth" => Some(Workload::Auth),
            "post" => Some(Workload::Post),
            "profile" => Some(Workload::Profile),
            "friend" => Some(Workload::Friend),
            "notification" => Some(Workload::Notification),
            "concert" => Some(Workload::Concert),
            _ => None,
        }
    }
}

/// Identity established from a verified credential, exposed to handlers.
///
/// Handlers read this from request extensions; they never parse headers or
/// certificates themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedCaller(pub Workload);

/// A sensitive internal operation that is authorized per caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RpcAction {
    /// Irreversible account cleanup issued by the deletion saga.
    ApplyUserDeletion,
    /// Moderator-driven content action.
    ApplyContentModeration,
    /// Authenticated operator release of one PostService dead-lettered event.
    ResolveDeadLetterReplay,
    /// Authenticated administrator restart of a dead-lettered account deletion.
    RetryUserDeletion,
    /// Read-only account-deletion investigation performed through the edge.
    InspectUserDeletions,
    /// User-facing writes proxied by the edge.
    UserMutation,
    /// ID-only owner-receipt status; restricted to the trusted edge.
    MutationReceiptLookup,
    /// Read paths proxied by the edge or by another service.
    Read,
    /// The anonymous NAVER verification callback. It arrives from the provider
    /// with no session, so the Gateway forwards it without an actor; the
    /// profile service reads the user from the stored one-use state row.
    VerificationCallback,
}

/// Which callers may invoke which sensitive action.
///
/// This is the authorization half of the trust boundary: authenticating a
/// workload does not imply it may perform every operation. Methods absent from
/// this table deny by default, so adding an RPC requires an explicit decision.
#[derive(Clone, Copy, Debug, Default)]
pub struct CallerPolicy;

impl CallerPolicy {
    pub fn allows(caller: Workload, action: RpcAction) -> bool {
        match action {
            // Only the account-deletion coordinator fans out cleanup. A gateway or
            // a sibling service credential must not be able to erase accounts.
            RpcAction::ApplyUserDeletion => matches!(caller, Workload::Auth),
            // Moderation is an operator action delivered from the edge today.
            RpcAction::ApplyContentModeration => matches!(caller, Workload::Gateway),
            // Only the Gateway can carry an administrator identity into PostService.
            RpcAction::ResolveDeadLetterReplay => matches!(caller, Workload::Gateway),
            // The Gateway is the only edge that can validate an admin principal.
            RpcAction::RetryUserDeletion => matches!(caller, Workload::Gateway),
            // Deletion sagas include cross-user lifecycle state and are exposed
            // only through the Gateway's fresh administrator-session check.
            RpcAction::InspectUserDeletions => matches!(caller, Workload::Gateway),
            // User-facing writes arrive through the edge.
            RpcAction::UserMutation => matches!(caller, Workload::Gateway),
            // Receipt IDs can reveal cross-user mutation history, so this
            // lookup is intentionally narrower than ordinary service reads.
            RpcAction::MutationReceiptLookup => matches!(caller, Workload::Gateway),
            // Only the Gateway receives the provider's callback. No sibling
            // service can complete a verification on a user's behalf.
            RpcAction::VerificationCallback => matches!(caller, Workload::Gateway),
            // Reads are needed by the edge and by services that aggregate data.
            RpcAction::Read => matches!(
                caller,
                Workload::Gateway | Workload::Post | Workload::Friend | Workload::Profile
            ),
        }
    }
}

/// Enforce [`CallerPolicy`] for the verified identity on a request.
///
/// A handler calls this before touching state. An unauthenticated request is
/// rejected as unauthenticated; an authenticated caller lacking authority is
/// rejected as permission denied, so operators can tell the two apart.
// `tonic::Status` is a large error value and is the required return type of an
// RPC handler, so the lint's suggested boxing is not applicable here.
#[allow(clippy::result_large_err)]
pub fn authorize_call<T>(request: &Request<T>, action: RpcAction) -> Result<Workload, Status> {
    let caller = request
        .extensions()
        .get::<VerifiedCaller>()
        .ok_or_else(|| Status::unauthenticated("verified service identity required"))?
        .0;
    if CallerPolicy::allows(caller, action) {
        Ok(caller)
    } else {
        tracing::warn!(
            caller = caller.as_str(),
            action = ?action,
            "internal caller is not authorized for this operation"
        );
        Err(Status::permission_denied(
            "service is not authorized for this operation",
        ))
    }
}

/// Compare two byte strings without early exit on the first differing byte.
///
/// A length mismatch is folded into the result rather than returned early, so a
/// caller cannot learn a secret's length or prefix from response timing.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = (left.len() ^ right.len()) as u8;
    let length = left.len().max(right.len());
    for index in 0..length {
        let left = left.get(index).copied().unwrap_or(0);
        let right = right.get(index).copied().unwrap_or(0);
        difference |= left ^ right;
    }
    difference == 0
}

/// Client-side identity: sends this workload's name with its own credential.
#[derive(Clone)]
pub struct ServiceIdentityClientInterceptor {
    caller: MetadataValue<Ascii>,
    token: MetadataValue<Ascii>,
}

impl ServiceIdentityClientInterceptor {
    pub fn new(workload: Workload, token: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(!token.trim().is_empty(), "internal service token is empty");
        Ok(Self {
            caller: workload.as_str().parse()?,
            token: token.parse()?,
        })
    }
}

impl Interceptor for ServiceIdentityClientInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        request
            .metadata_mut()
            .insert(SERVICE_CALLER_HEADER, self.caller.clone());
        request
            .metadata_mut()
            .insert(SERVICE_TOKEN_HEADER, self.token.clone());
        // The request identifier rides along here rather than at each call site,
        // so a new internal call carries it by construction. A call made outside
        // any request scope is left without one instead of gaining a fabricated
        // value that would join nothing.
        crate::correlation::attach(&mut request)?;
        // Propagate only the validated context established at this service's
        // ingress. The trace identifier is diagnostic context, never identity.
        crate::trace_context::attach(&mut request)?;
        Ok(request)
    }
}

/// Server-side registry of accepted callers and their distinct credentials.
///
/// Each entry pairs one workload with the credential that workload must present
/// for this service. There is deliberately no single shared secret: leaking the
/// credential one peer uses does not authenticate any other peer.
#[derive(Clone, Default)]
pub struct CallerRegistry {
    entries: Vec<(Workload, Vec<Vec<u8>>)>,
}

impl CallerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an accepted caller. A duplicate caller is rejected so a
    /// misconfiguration cannot silently create two credentials for one identity.
    pub fn register(self, workload: Workload, token: &str) -> anyhow::Result<Self> {
        self.register_with_previous(workload, token, None)
    }

    /// Register the current and, during rotation, one previous credential.
    ///
    /// The overlap is deliberately capped at two values: receivers can be
    /// rolled before senders without opening an unbounded credential history.
    /// Remove the previous value once every sender presents the current one.
    pub fn register_with_previous(
        mut self,
        workload: Workload,
        current_token: &str,
        previous_token: Option<&str>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !current_token.trim().is_empty(),
            "internal service token is empty"
        );
        anyhow::ensure!(
            !self.entries.iter().any(|(known, _)| *known == workload),
            "duplicate caller registration for {}",
            workload.as_str()
        );
        let mut credentials = vec![current_token.as_bytes().to_vec()];
        if let Some(previous_token) = previous_token {
            anyhow::ensure!(
                !previous_token.trim().is_empty(),
                "previous internal service token is empty"
            );
            anyhow::ensure!(
                !constant_time_eq(current_token.as_bytes(), previous_token.as_bytes()),
                "current and previous internal service tokens must differ"
            );
            credentials.push(previous_token.as_bytes().to_vec());
        }
        self.entries.push((workload, credentials));
        Ok(self)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn authenticate(&self, caller: &str, token: &[u8]) -> Option<Workload> {
        let workload = Workload::parse(caller)?;
        let (known, credentials) = self.entries.iter().find(|(known, _)| *known == workload)?;
        let matches = credentials.iter().fold(false, |matched, expected| {
            matched | constant_time_eq(expected, token)
        });
        matches.then_some(*known)
    }
}

/// Reject anything but a registered caller presenting that caller's credential.
///
/// On success the verified [`Workload`] is attached to the request so handlers
/// can authorize the specific operation without re-reading headers. On TLS
/// connections the leaf certificate must identify that same workload.
#[derive(Clone)]
pub struct RequireRegisteredCallerInterceptor {
    registry: CallerRegistry,
}

impl RequireRegisteredCallerInterceptor {
    pub fn new(registry: CallerRegistry) -> anyhow::Result<Self> {
        anyhow::ensure!(!registry.is_empty(), "caller registry must not be empty");
        Ok(Self { registry })
    }
}

impl Interceptor for RequireRegisteredCallerInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let caller = request
            .metadata()
            .get(SERVICE_CALLER_HEADER)
            .and_then(|value| value.to_str().ok());
        let token = request
            .metadata()
            .get(SERVICE_TOKEN_HEADER)
            .map(MetadataValue::as_bytes);
        let (Some(caller), Some(token)) = (caller, token) else {
            return Err(Status::unauthenticated(
                "internal service identity required",
            ));
        };
        match self.registry.authenticate(caller, token) {
            Some(workload) => {
                // The transport has verified CA trust; bind the leaf identity to
                // the credential too. A certificate for one registered service
                // must not authorize another service's metadata and credential.
                use tonic::transport::server::{TcpConnectInfo, TlsConnectInfo};
                if request
                    .extensions()
                    .get::<TlsConnectInfo<TcpConnectInfo>>()
                    .is_some()
                {
                    let certificate_workload = request.peer_certs().and_then(|certificates| {
                        certificates.first().and_then(|certificate| {
                            crate::grpc_tls::workload_from_peer_certificate(certificate.as_ref())
                        })
                    });
                    if certificate_workload != Some(workload) {
                        return Err(Status::unauthenticated(
                            "internal service identity required",
                        ));
                    }
                }
                request.extensions_mut().insert(VerifiedCaller(workload));
                Ok(request)
            }
            None => Err(Status::unauthenticated(
                "internal service identity required",
            )),
        }
    }
}

#[cfg(test)]
mod trace_context_tests {
    use super::*;
    use std::convert::Infallible;
    use tower::{Layer, ServiceExt};

    #[tokio::test]
    async fn outbound_identity_interceptor_propagates_validated_trace_context() {
        const TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

        let mut headers = http::HeaderMap::new();
        headers.insert(
            crate::trace_context::TRACEPARENT_HEADER,
            http::HeaderValue::from_static(TRACEPARENT),
        );
        headers.insert(
            crate::trace_context::TRACESTATE_HEADER,
            http::HeaderValue::from_static("vendor=opaque"),
        );
        let service = crate::correlation::CorrelationLayer.layer(
            crate::trace_context::TraceContextLayer.layer(tower::service_fn(
                |_request: http::Request<()>| async move {
                    let mut interceptor =
                        ServiceIdentityClientInterceptor::new(Workload::Gateway, "gateway-token")
                            .expect("valid workload identity");
                    let outbound = interceptor
                        .call(Request::new(()))
                        .expect("outbound request metadata");
                    Ok::<_, Infallible>(outbound)
                },
            )),
        );
        let request = http::Request::builder()
            .header(crate::trace_context::TRACEPARENT_HEADER, TRACEPARENT)
            .header(crate::trace_context::TRACESTATE_HEADER, "vendor=opaque")
            .header(crate::correlation::CORRELATION_ID_HEADER, "request-123")
            .body(())
            .unwrap();

        let request = service.oneshot(request).await.unwrap();
        assert_eq!(
            request.metadata().get(SERVICE_CALLER_HEADER).unwrap(),
            "gateway"
        );
        assert_eq!(
            request.metadata().get(SERVICE_TOKEN_HEADER).unwrap(),
            "gateway-token"
        );
        assert_eq!(
            request
                .metadata()
                .get(crate::correlation::CORRELATION_ID_HEADER)
                .unwrap(),
            "request-123"
        );
        let traceparent = request
            .metadata()
            .get(crate::trace_context::TRACEPARENT_HEADER)
            .unwrap()
            .to_str()
            .unwrap();
        let parts = traceparent.split('-').collect::<Vec<_>>();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "00");
        assert_eq!(parts[1], "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_ne!(parts[2], "00f067aa0ba902b7");
        assert_eq!(parts[3], "01");
        assert_eq!(
            request
                .metadata()
                .get(crate::trace_context::TRACESTATE_HEADER)
                .unwrap(),
            "vendor=opaque"
        );
    }
}
