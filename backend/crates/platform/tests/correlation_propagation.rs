//! Prove one request identifier composes across a call boundary.
//!
//! The unit tests cover the scope and the metadata helpers. What they cannot show
//! is the interaction that decides whether a chain of services is followable: a
//! server receiving a call continues the caller scope, so a call it makes in turn
//! carries the same identifier. These tests assemble that sequence directly, using
//! the same layer the services install.

use platform::correlation::{self, CORRELATION_ID_HEADER, MAX_CORRELATION_ID_LENGTH};
use tonic::Request;

#[tokio::test]
async fn a_scope_survives_a_simulated_service_hop() {
    // Hop 1: the edge receives a request and establishes the scope.
    correlation::scope("chain-1".to_owned(), async {
        // Hop 2: the edge calls a service. The client interceptor attaches the
        // current identifier, so the outbound request carries it.
        let mut outbound = Request::new(());
        correlation::attach(&mut outbound).expect("attach outbound");
        let received = correlation::from_metadata(&outbound);
        assert_eq!(received, Some("chain-1".to_owned()));

        // Hop 3: the service runs the inbound identifier as its own scope, then
        // calls another service from within it.
        let inbound = received.expect("inbound identifier");
        let forwarded = correlation::scope(inbound, async {
            let mut onward = Request::new(());
            correlation::attach(&mut onward).expect("attach onward");
            correlation::from_metadata(&onward)
        })
        .await;

        // One identifier reaches both hops, which is what makes the chain
        // followable in logs.
        assert_eq!(forwarded, Some("chain-1".to_owned()));
    })
    .await;
}

#[tokio::test]
async fn a_hop_without_an_identifier_does_not_invent_one() {
    // An internal caller that sent no identifier leaves the service without a
    // scope. Fabricating a value per hop would produce identifiers that look
    // meaningful and join nothing.
    correlation::scope(String::new(), async {
        // An empty scope value is not a usable identifier, so it is not attached.
        let mut onward = Request::new(());
        correlation::attach(&mut onward).expect("attach");
        let carried = onward
            .metadata()
            .get(CORRELATION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(correlation::normalize);
        assert_eq!(carried, None);
    })
    .await;
}

#[test]
fn an_identifier_longer_than_the_bound_is_refused() {
    // The value reaches log lines, so an unbounded one would let a single request
    // inflate every entry written while handling it.
    let oversized = "a".repeat(MAX_CORRELATION_ID_LENGTH + 1);
    assert_eq!(correlation::normalize(&oversized), None);
    let at_bound = "a".repeat(MAX_CORRELATION_ID_LENGTH);
    assert!(correlation::normalize(&at_bound).is_some());
}
