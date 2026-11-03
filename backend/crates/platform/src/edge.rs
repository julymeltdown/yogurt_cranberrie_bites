//! What each internal call is allowed to cost, and what may be retried.
//!
//! Before this module the budgets existed but were uniform and implicit: every
//! internal call shared one request deadline and one connect deadline, chosen as
//! a safe default rather than an expressed intent. A uniform budget cannot say
//! whether the number was chosen for a call that fans out to a database or for
//! one that reads a cached row, so nobody can tell whether three seconds is
//! generous or already too late.
//!
//! It also cannot answer the retry question, which is the more dangerous gap.
//! Retrying is only safe when the operation is idempotent *or* the first attempt
//! provably did not take effect. "The connection was refused" is safe to retry;
//! "the deadline expired after the request was sent" is not, because the peer may
//! have committed the write and lost only the reply. Classifying by exception type
//! alone is what makes a retry loop duplicate a business effect, so the
//! classification here is explicit per edge and the default is the conservative
//! one.

use std::time::Duration;

/// How a failed call may be treated by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryClass {
    /// The operation is replay-safe (for example, a read), or transport evidence
    /// independently proves that it was never dispatched. A status code alone
    /// does not establish either condition.
    SafeToRetry,
    /// Attempt again only when the caller holds an idempotency key that the
    /// owner uses to deduplicate, because the first attempt may have taken
    /// effect.
    RetryOnlyWithIdempotencyKey,
    /// Never retried automatically. The outcome is unknown, so a retry could
    /// duplicate a committed write, and the operation needs reconciliation
    /// rather than another attempt.
    Never,
}

impl RetryClass {
    /// Whether a bare retry (no idempotency key) is permitted.
    pub fn allows_bare_retry(self) -> bool {
        matches!(self, RetryClass::SafeToRetry)
    }
}

/// What happens when an edge cannot answer within its budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailurePolicy {
    /// Fail the caller's request. The work cannot be completed without this
    /// answer and guessing would be wrong.
    FailRequest,
    /// Answer with what is known and record the degradation, when the value is
    /// an enrichment rather than a precondition.
    DegradeWithoutValue,
}

/// The declared cost and failure contract of one internal edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeBudget {
    /// A human-readable edge name, used in logs and assertions.
    pub edge: &'static str,
    /// Latency the caller expects under normal operation.
    pub expected_latency: Duration,
    /// Hard bound on one attempt, after which the attempt is abandoned.
    pub deadline: Duration,
    /// Bound on establishing the connection for this attempt.
    pub connect_deadline: Duration,
    pub retry: RetryClass,
    pub failure_policy: FailurePolicy,
}

impl EdgeBudget {
    /// Validate the contract holds together before it is used.
    ///
    /// The invariant that matters is ordering: a deadline shorter than the
    /// expected latency would abandon calls that normally succeed, and a
    /// connect deadline longer than the request deadline is unreachable because
    /// the request budget expires first. Both are silent misconfigurations that
    /// only show up as unexplained failures under load.
    pub fn validate(self) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !self.edge.is_empty(),
            "an edge budget must name the edge it describes"
        );
        anyhow::ensure!(
            self.expected_latency > Duration::ZERO,
            "edge {} declares a zero expected latency, which no real call has",
            self.edge
        );
        anyhow::ensure!(
            self.deadline > self.expected_latency,
            "edge {} declares a {} ms deadline below its {} ms expected latency, so a normal call would be abandoned",
            self.edge,
            self.deadline.as_millis(),
            self.expected_latency.as_millis()
        );
        anyhow::ensure!(
            self.connect_deadline <= self.deadline,
            "edge {} declares a {} ms connect deadline above its {} ms request deadline, which can never be reached",
            self.edge,
            self.connect_deadline.as_millis(),
            self.deadline.as_millis()
        );
        Ok(self)
    }
}

/// The declared budget for one edge, keyed by the pair it connects.
///
/// Each entry states what the caller expects and what it will tolerate, so a
/// later reader can tell whether a number was chosen or copied. The table is
/// also what a test can iterate: a contract that is not enumerated cannot be
/// checked for completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    /// Gateway to auth, for token and session operations.
    GatewayAuth,
    /// Gateway to post, for reads and mutations.
    GatewayPost,
    /// Gateway to profile, for profile reads and writes.
    GatewayProfile,
    /// Gateway to friend, for follow and relationship policy.
    GatewayFriend,
    /// Gateway to notification, for notification reads and writes.
    GatewayNotification,
    /// Post to profile, for author display data on a post.
    PostProfile,
    /// Post to friend, for relationship policy on feed and search.
    PostFriend,
    /// Post to notification, for fanout on a new post.
    PostNotification,
    /// Post to the Gateway, to exchange an admitted request's authority for a
    /// one-hop child before a Friend or Profile call on the request path.
    PostGateway,
    /// Friend to profile, for privacy checks on a relationship.
    FriendProfile,
    /// Auth to profile, for the account-deletion saga.
    AuthProfile,
    /// Auth to friend, for the account-deletion saga.
    AuthFriend,
    /// Auth to post, for the account-deletion saga.
    AuthPost,
    /// Auth to notification, for the account-deletion saga.
    AuthNotification,
}

impl Edge {
    pub const ALL: [Edge; 14] = [
        Edge::GatewayAuth,
        Edge::GatewayPost,
        Edge::GatewayProfile,
        Edge::GatewayFriend,
        Edge::GatewayNotification,
        Edge::PostProfile,
        Edge::PostFriend,
        Edge::PostNotification,
        Edge::PostGateway,
        Edge::FriendProfile,
        Edge::AuthProfile,
        Edge::AuthFriend,
        Edge::AuthPost,
        Edge::AuthNotification,
    ];

    pub fn caller(self) -> crate::grpc_auth::Workload {
        use crate::grpc_auth::Workload;
        match self {
            Self::GatewayAuth
            | Self::GatewayPost
            | Self::GatewayProfile
            | Self::GatewayFriend
            | Self::GatewayNotification => Workload::Gateway,
            Self::PostProfile | Self::PostFriend | Self::PostNotification | Self::PostGateway => {
                Workload::Post
            }
            Self::FriendProfile => Workload::Friend,
            Self::AuthProfile | Self::AuthFriend | Self::AuthPost | Self::AuthNotification => {
                Workload::Auth
            }
        }
    }

    pub fn peer(self) -> crate::grpc_auth::Workload {
        use crate::grpc_auth::Workload;
        match self {
            Self::GatewayAuth => Workload::Auth,
            Self::GatewayPost | Self::AuthPost => Workload::Post,
            Self::GatewayProfile | Self::PostProfile | Self::FriendProfile | Self::AuthProfile => {
                Workload::Profile
            }
            Self::GatewayFriend | Self::PostFriend | Self::AuthFriend => Workload::Friend,
            Self::GatewayNotification | Self::PostNotification | Self::AuthNotification => {
                Workload::Notification
            }
            Self::PostGateway => Workload::Gateway,
        }
    }

    /// Async deletion and notification delivery can queue while a peer is down.
    /// Synchronous authorization dependencies must answer before traffic is admitted.
    pub fn required_for_readiness(self) -> bool {
        !matches!(
            self,
            Self::AuthProfile
                | Self::AuthFriend
                | Self::AuthPost
                | Self::AuthNotification
                | Self::PostProfile
                | Self::PostNotification
        )
    }

    /// Most calls this process may have outstanding on this edge at once.
    ///
    /// A call beyond the limit is refused locally with RESOURCE_EXHAUSTED
    /// before anything is sent, so a slow dependency cannot accumulate an
    /// unbounded queue of waiting requests and their memory in the caller.
    /// The refusal proves no dispatch, unlike a peer error.
    ///
    /// These are provisional per-process ceilings, set well above the
    /// observed working concurrency so that they only bite during a
    /// dependency stall. The values for the target replica count and load
    /// are a D5 decision and replace these once frozen.
    pub fn max_in_flight(self) -> usize {
        match self {
            Edge::GatewayAuth
            | Edge::GatewayPost
            | Edge::GatewayProfile
            | Edge::GatewayFriend
            | Edge::GatewayNotification => 256,
            Edge::PostProfile | Edge::PostFriend | Edge::FriendProfile | Edge::PostGateway => 128,
            Edge::PostNotification => 64,
            Edge::AuthProfile | Edge::AuthFriend | Edge::AuthPost | Edge::AuthNotification => 32,
        }
    }

    /// The budget this edge declares.
    ///
    /// The numbers follow one rule: an edge the Gateway forwards on the request
    /// path gets a deadline strictly below the Gateway outer timeout, so the
    /// dependency fails before the client deadline and the failure has a name
    /// instead of looking like a Gateway stall. Edges that are enrichment
    /// rather than a precondition are allowed longer.
    pub fn budget(self) -> EdgeBudget {
        match self {
            Edge::GatewayPost => EdgeBudget {
                edge: "gateway->post",
                expected_latency: Duration::from_millis(40),
                deadline: Duration::from_millis(800),
                connect_deadline: Duration::from_millis(500),
                retry: RetryClass::RetryOnlyWithIdempotencyKey,
                failure_policy: FailurePolicy::FailRequest,
            },
            Edge::GatewayProfile => EdgeBudget {
                edge: "gateway->profile",
                expected_latency: Duration::from_millis(40),
                deadline: Duration::from_millis(800),
                connect_deadline: Duration::from_millis(500),
                retry: RetryClass::RetryOnlyWithIdempotencyKey,
                failure_policy: FailurePolicy::FailRequest,
            },
            Edge::GatewayFriend => EdgeBudget {
                edge: "gateway->friend",
                expected_latency: Duration::from_millis(40),
                deadline: Duration::from_millis(800),
                connect_deadline: Duration::from_millis(500),
                retry: RetryClass::RetryOnlyWithIdempotencyKey,
                failure_policy: FailurePolicy::FailRequest,
            },
            Edge::GatewayNotification => EdgeBudget {
                edge: "gateway->notification",
                expected_latency: Duration::from_millis(40),
                deadline: Duration::from_millis(800),
                connect_deadline: Duration::from_millis(500),
                retry: RetryClass::RetryOnlyWithIdempotencyKey,
                failure_policy: FailurePolicy::FailRequest,
            },
            Edge::GatewayAuth => EdgeBudget {
                edge: "gateway->auth",
                expected_latency: Duration::from_millis(60),
                deadline: Duration::from_millis(900),
                connect_deadline: Duration::from_millis(500),
                retry: RetryClass::RetryOnlyWithIdempotencyKey,
                failure_policy: FailurePolicy::FailRequest,
            },
            Edge::PostProfile => EdgeBudget {
                edge: "post->profile",
                expected_latency: Duration::from_millis(50),
                deadline: Duration::from_secs(1),
                connect_deadline: Duration::from_millis(500),
                retry: RetryClass::SafeToRetry,
                failure_policy: FailurePolicy::DegradeWithoutValue,
            },
            Edge::PostFriend => EdgeBudget {
                edge: "post->friend",
                expected_latency: Duration::from_millis(50),
                deadline: Duration::from_secs(1),
                connect_deadline: Duration::from_millis(500),
                retry: RetryClass::SafeToRetry,
                failure_policy: FailurePolicy::FailRequest,
            },
            Edge::FriendProfile => EdgeBudget {
                edge: "friend->profile",
                expected_latency: Duration::from_millis(50),
                // This edge keeps its own per-call wrapper, which is what turns a
                // timeout into a named `DeadlineExceeded` rather than the transport's
                // generic `Cancelled`. The endpoint deadline therefore sits *above*
                // that wrapper so the local one always fires first, the same inner-
                // answers-first ordering the request journal uses.
                deadline: Duration::from_secs(3),
                connect_deadline: Duration::from_secs(2),
                retry: RetryClass::SafeToRetry,
                failure_policy: FailurePolicy::FailRequest,
            },
            Edge::PostNotification => EdgeBudget {
                edge: "post->notification",
                expected_latency: Duration::from_millis(50),
                deadline: Duration::from_secs(2),
                connect_deadline: Duration::from_millis(500),
                retry: RetryClass::RetryOnlyWithIdempotencyKey,
                failure_policy: FailurePolicy::DegradeWithoutValue,
            },
            // Owner decision (2026-10-16): the exchange takes the Gateway to Post
            // budget values. Only the name differs, so logs and metrics identify
            // the hop. The client also stops at the parent authority's expiry.
            Edge::PostGateway => EdgeBudget {
                edge: "post->gateway",
                ..Edge::GatewayPost.budget()
            },
            Edge::AuthProfile | Edge::AuthFriend | Edge::AuthPost | Edge::AuthNotification => {
                EdgeBudget {
                    edge: match self {
                        Edge::AuthProfile => "auth->profile",
                        Edge::AuthFriend => "auth->friend",
                        Edge::AuthPost => "auth->post",
                        Edge::AuthNotification => "auth->notification",
                        _ => unreachable!(),
                    },
                    expected_latency: Duration::from_millis(80),
                    deadline: Duration::from_secs(2),
                    connect_deadline: Duration::from_millis(500),
                    retry: RetryClass::Never,
                    failure_policy: FailurePolicy::FailRequest,
                }
            }
        }
    }
}

/// Every declared budget, validated.
///
/// A table entry that violates its own invariants is a configuration error
/// rather than a value to use at runtime, so this is the entry point a caller
/// or a test uses to read the table.
pub fn declared_budgets() -> anyhow::Result<Vec<(Edge, EdgeBudget)>> {
    Edge::ALL
        .into_iter()
        .map(|edge| Ok((edge, edge.budget().validate()?)))
        .collect()
}

/// Conservative mutation policy when only a gRPC status is known.
///
/// UNAVAILABLE may be returned by the server after execution or after response
/// transport loss. A missing status is also ambiguous. Neither proves that no
/// write occurred, so this function never authorizes a bare retry. A supplied key
/// is useful only when the owner durably deduplicates it; a header alone is not
/// sufficient. Known replay-safe reads may use a separate operation policy.
pub fn classify(status: Option<tonic::Code>) -> RetryClass {
    match status {
        // The deadline elapsed with the outcome unknown.
        Some(tonic::Code::DeadlineExceeded) => RetryClass::Never,
        // A refusal the peer made deliberately: retrying it unchanged would be
        // refused again.
        Some(tonic::Code::Ok)
        | Some(tonic::Code::DataLoss)
        | Some(tonic::Code::InvalidArgument)
        | Some(tonic::Code::NotFound)
        | Some(tonic::Code::AlreadyExists)
        | Some(tonic::Code::PermissionDenied)
        | Some(tonic::Code::Unauthenticated)
        | Some(tonic::Code::FailedPrecondition)
        | Some(tonic::Code::OutOfRange)
        | Some(tonic::Code::Unimplemented) => RetryClass::Never,
        // Includes UNAVAILABLE, INTERNAL, cancellation and a missing status:
        // without independent dispatch evidence, the outcome may have committed.
        Some(_) | None => RetryClass::RetryOnlyWithIdempotencyKey,
    }
}
