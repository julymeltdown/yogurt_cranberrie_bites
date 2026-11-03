//! The declared edge table must hold its own invariants, and the retry
//! classification must be conservative where a duplicate effect is possible.

use platform::edge::{classify, declared_budgets, Edge, FailurePolicy, RetryClass};
use std::collections::HashSet;
use std::time::Duration;

#[test]
fn every_declared_budget_satisfies_its_own_invariants() {
    // A table entry with a deadline below its expected latency would abandon
    // normal calls, and one with an unreachable connect deadline is a number
    // nobody can act on. Both are silent until load exposes them.
    let budgets = declared_budgets().expect("the declared table must be valid");
    assert_eq!(budgets.len(), Edge::ALL.len());

    for (edge, budget) in &budgets {
        assert_eq!(
            *edge,
            edge_of(budget.edge),
            "{} names the wrong edge",
            budget.edge
        );
        assert!(
            budget.deadline > budget.expected_latency,
            "{} must allow its own expected latency",
            budget.edge
        );
        assert!(
            budget.connect_deadline <= budget.deadline,
            "{} must be able to establish a connection within its request budget",
            budget.edge
        );
    }
}

#[test]
fn every_edge_is_declared_exactly_once() {
    // A duplicated name would make two edges indistinguishable in a log, and a
    // missing one means a client somewhere is using a budget nobody declared.
    let budgets = declared_budgets().expect("valid table");
    let names: HashSet<&str> = budgets.iter().map(|(_, b)| b.edge).collect();
    assert_eq!(names.len(), budgets.len(), "edge names must be unique");
}

#[test]
fn a_deadline_below_the_expected_latency_is_rejected() {
    let broken = Edge::GatewayPost
        .budget()
        .validate()
        .expect("the declared budget is valid");
    let impossible = platform::edge::EdgeBudget {
        deadline: Duration::from_millis(1),
        ..broken
    };
    assert!(
        impossible.validate().is_err(),
        "a deadline under the expected latency must be refused"
    );
}

#[test]
fn a_connect_deadline_above_the_request_deadline_is_rejected() {
    let base = Edge::GatewayPost.budget().validate().expect("valid");
    let unreachable = platform::edge::EdgeBudget {
        connect_deadline: base.deadline + Duration::from_secs(1),
        ..base
    };
    assert!(
        unreachable.validate().is_err(),
        "a connect deadline that cannot be reached must be refused"
    );
}

#[test]
fn transport_status_alone_never_proves_a_write_was_not_applied() {
    // Either case can describe losing a reply after the owner committed.
    assert_eq!(classify(None), RetryClass::RetryOnlyWithIdempotencyKey);
    assert_eq!(
        classify(Some(tonic::Code::Unavailable)),
        RetryClass::RetryOnlyWithIdempotencyKey
    );
    for code in 0..=16 {
        assert!(
            !classify(Some(tonic::Code::from_i32(code))).allows_bare_retry(),
            "status {code} cannot prove a mutation did not commit"
        );
    }
}

#[test]
fn an_edge_that_carries_mutations_never_authorizes_a_bare_retry() {
    for edge in [
        Edge::GatewayAuth,
        Edge::GatewayPost,
        Edge::GatewayProfile,
        Edge::GatewayFriend,
        Edge::GatewayNotification,
        Edge::PostNotification,
    ] {
        assert!(
            !edge.budget().retry.allows_bare_retry(),
            "{edge:?} carries writes as well as reads"
        );
    }
}

#[test]
fn an_expired_deadline_is_never_retried() {
    // The request was sent and the reply was lost, so the peer may have
    // committed the write. This is the case a naive retry duplicates.
    assert_eq!(
        classify(Some(tonic::Code::DeadlineExceeded)),
        RetryClass::Never
    );
    assert!(!classify(Some(tonic::Code::DeadlineExceeded)).allows_bare_retry());
}

#[test]
fn a_deliberate_refusal_is_never_retried() {
    // Retrying unchanged would be refused again, so a retry only spends budget.
    for status in [
        tonic::Code::InvalidArgument,
        tonic::Code::NotFound,
        tonic::Code::AlreadyExists,
        tonic::Code::PermissionDenied,
        tonic::Code::Unauthenticated,
        tonic::Code::FailedPrecondition,
        tonic::Code::Unimplemented,
    ] {
        assert_eq!(classify(Some(status)), RetryClass::Never, "{status:?}");
    }
}

#[test]
fn an_unrecognized_server_answer_requires_an_idempotency_key() {
    // The conservative default for a status this build does not name is that
    // the request was received, so a bare retry is not permitted.
    assert_eq!(
        classify(Some(tonic::Code::Aborted)),
        RetryClass::RetryOnlyWithIdempotencyKey
    );
    assert!(!classify(Some(tonic::Code::Aborted)).allows_bare_retry());
}

#[test]
fn an_enrichment_edge_degrades_rather_than_failing_the_request() {
    // Author display data is an enrichment; a relationship policy is a
    // disclosure decision. Treating them the same would either fail posts for
    // a cosmetic lookup or guess at visibility.
    assert_eq!(
        Edge::PostProfile.budget().failure_policy,
        FailurePolicy::DegradeWithoutValue
    );
    assert_eq!(
        Edge::PostFriend.budget().failure_policy,
        FailurePolicy::FailRequest
    );
}

#[test]
fn deletion_edges_do_not_retry_themselves() {
    // The deletion saga owns retry and reconciliation. A blind retry here
    // could re-run a step that already committed.
    for edge in [
        Edge::AuthProfile,
        Edge::AuthFriend,
        Edge::AuthPost,
        Edge::AuthNotification,
    ] {
        assert_eq!(edge.budget().retry, RetryClass::Never, "{edge:?}");
    }
}

#[test]
fn an_edge_with_its_own_timeout_wrapper_declares_a_longer_endpoint_deadline() {
    // The friend-to-profile client wraps each call in its own timeout so a
    // timeout becomes a named DeadlineExceeded instead of the transport's
    // generic Cancelled. That wrapper only wins if the endpoint deadline is
    // longer, so this ordering is load-bearing rather than cosmetic: the
    // contract tests observe the error code it produces.
    let budget = Edge::FriendProfile.budget();
    assert!(
        budget.deadline > FRIEND_PROFILE_RPC_TIMEOUT,
        "{} declares a {} ms deadline that the {} ms local wrapper cannot beat",
        budget.edge,
        budget.deadline.as_millis(),
        FRIEND_PROFILE_RPC_TIMEOUT.as_millis()
    );
}

/// The per-call timeout the friend-to-profile client applies itself.
///
/// Duplicated from the client rather than imported because the assertion above
/// is about the relationship between two numbers owned by different files, and
/// a test that imported both could not notice one being changed to break it.
const FRIEND_PROFILE_RPC_TIMEOUT: Duration = Duration::from_secs(2);

#[test]
fn gateway_read_edges_answer_inside_the_gateway_outer_timeout() {
    // The Gateway answers a forwarded read within its own outer deadline. A
    // dependency budget above that would be reported as a Gateway stall
    // instead of naming the dependency that was slow.
    let gateway_timeout = Duration::from_millis(1000);
    for edge in [
        Edge::GatewayAuth,
        Edge::GatewayPost,
        Edge::GatewayProfile,
        Edge::GatewayFriend,
        Edge::GatewayNotification,
    ] {
        let budget = edge.budget();
        assert!(
            budget.deadline < gateway_timeout,
            "{} declares a {} ms deadline at or above the {} ms Gateway timeout",
            budget.edge,
            budget.deadline.as_millis(),
            gateway_timeout.as_millis()
        );
    }
}

/// Map a declared name back to its enum variant, so a copy-paste error in the
/// table is caught rather than merely looking plausible.
fn edge_of(name: &str) -> Edge {
    Edge::ALL
        .into_iter()
        .find(|edge| edge.budget().edge == name)
        .unwrap_or_else(|| panic!("no edge declares the name {name}"))
}
