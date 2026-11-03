//! Adapted from Auth RPC qualification at d901b133; Friend owner assertions are separate.
use friend_service::adapters::r#in::grpc::{
    concert_invalidation::{guarded_server, InvalidationServer, POLICY},
    concert_relationship::proto,
};
use platform::grpc_auth::{CallerRegistry, VerifiedCaller, Workload};
use prost::Message;
use std::time::{Duration, Instant};
use tonic::codegen::http;
use tonic::{Code, Request};
use uuid::Uuid;

fn head_query() -> proto::ReadInvalidationHeadRequest {
    proto::ReadInvalidationHeadRequest {
        consumer_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
    }
}
fn after_query() -> proto::ReadInvalidationsRequest {
    let q = head_query();
    proto::ReadInvalidationsRequest {
        consumer_id: q.consumer_id,
        request_id: q.request_id,
        after: Some(proto::InvalidationCursor {
            owner_epoch: Uuid::new_v4().to_string(),
            sequence: 0,
        }),
        limit: 256,
    }
}
fn registry() -> CallerRegistry {
    Workload::ALL
        .into_iter()
        .fold(CallerRegistry::new(), |r, w| {
            r.register(w, &format!("fixture-{}", w.as_str())).unwrap()
        })
}
fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://refused:fixture@127.0.0.1:1/must_not_connect")
        .unwrap()
}
fn framed<M: Message>(m: &M) -> Vec<u8> {
    let bytes = m.encode_to_vec();
    let mut out = vec![0];
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend(bytes);
    out
}
async fn wire_status<B>(body: B, method: &str, identity: Option<(&str, &str)>, forged: bool) -> Code
where
    B: http_body_util::BodyExt<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let pool = lazy_pool();
    pool.close().await;
    let svc = guarded_server(pool, registry()).unwrap();
    let mut req = http::Request::builder()
        .method("POST")
        .uri(format!("/{}/{}", POLICY.service, method))
        .header("content-type", "application/grpc")
        .header("te", "trailers");
    if let Some((who, token)) = identity {
        req = req
            .header("x-service-caller", who)
            .header("x-service-token", token);
    }
    let mut req = req.body(tonic::body::Body::new(body)).unwrap();
    if forged {
        req.extensions_mut()
            .insert(VerifiedCaller(Workload::Concert));
    }
    let resp = svc.oneshot(req).await.unwrap();
    let h = resp.headers().clone();
    let b = resp.into_body().collect().await.unwrap();
    Code::from_i32(
        h.get("grpc-status")
            .or_else(|| b.trailers().and_then(|t| t.get("grpc-status")))
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap(),
    )
}

#[tokio::test]
async fn direct_handler_requires_independent_verified_concert_and_original_budget() {
    use proto::friend_concert_invalidation_service_server::FriendConcertInvalidationService;
    let svc = InvalidationServer::from_pool(lazy_pool());
    for who in std::iter::once(None).chain(
        Workload::ALL
            .into_iter()
            .filter(|w| *w != Workload::Concert)
            .map(Some),
    ) {
        let mut req = Request::new(head_query());
        if let Some(w) = who {
            req.extensions_mut().insert(VerifiedCaller(w));
        }
        assert!(matches!(
            svc.read_head(req).await.unwrap_err().code(),
            Code::Unauthenticated | Code::PermissionDenied
        ));
    }
    let mut req = Request::new(head_query());
    req.extensions_mut()
        .insert(VerifiedCaller(Workload::Concert));
    assert_eq!(
        svc.read_head(req).await.unwrap_err().code(),
        Code::Unavailable
    );
    let mut req = Request::new(head_query());
    req.extensions_mut()
        .insert(VerifiedCaller(Workload::Concert));
    assert_eq!(
        platform::call_deadline::scope(
            Instant::now() - Duration::from_millis(1),
            svc.read_head(req)
        )
        .await
        .unwrap_err()
        .code(),
        Code::DeadlineExceeded
    );
}

#[tokio::test]
async fn actual_guard_denies_public_gateway_siblings_forged_identity_and_unknown_method() {
    use http_body_util::Full;
    let body = || Full::new(tonic::codegen::Bytes::from(framed(&head_query())));
    assert_eq!(
        wire_status(body(), "ReadHead", None, true).await,
        Code::Unauthenticated
    );
    assert_eq!(
        wire_status(
            body(),
            "ReadHead",
            Some(("concert", "fixture-gateway")),
            true
        )
        .await,
        Code::Unauthenticated
    );
    for w in Workload::ALL
        .into_iter()
        .filter(|w| *w != Workload::Concert)
    {
        assert_eq!(
            wire_status(
                body(),
                "ReadHead",
                Some((w.as_str(), &format!("fixture-{}", w.as_str()))),
                false
            )
            .await,
            Code::PermissionDenied
        );
    }
    assert_eq!(
        wire_status(
            body(),
            "Unknown",
            Some(("concert", "fixture-concert")),
            false
        )
        .await,
        Code::PermissionDenied
    );
    // Valid system caller gets past the guard without an actor proof/JWT, then
    // fails on the deliberately closed owner pool, never as empty success.
    assert_eq!(
        wire_status(
            body(),
            "ReadHead",
            Some(("concert", "fixture-concert")),
            false
        )
        .await,
        Code::Unavailable
    );
    assert!(POLICY.actor_fields.is_empty());
    for method in ["ReadHead", "ReadAfter"] {
        assert_eq!(POLICY.callers(method), Some(&[Workload::Concert][..]));
    }
}

#[tokio::test]
async fn malformed_queries_bounds_truncated_and_oversized_frames_fail_before_owner_read() {
    use http_body_util::Full;
    for which in 0..12 {
        let mut q = after_query();
        match which {
            0 => q.consumer_id = Uuid::nil().to_string(),
            1 => q.request_id = "private-canary".into(),
            2 => q.request_id = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into(),
            3 => q.after = None,
            4 => q.after.as_mut().unwrap().owner_epoch = Uuid::nil().to_string(),
            5 => q.after.as_mut().unwrap().owner_epoch = Uuid::new_v4().simple().to_string(),
            6 => q.after.as_mut().unwrap().sequence = -1,
            7 => q.after.as_mut().unwrap().sequence = 9_007_199_254_740_992,
            8 => q.limit = 0,
            9 => q.limit = 257,
            10 => q.limit = u32::MAX,
            11 => q.consumer_id = Uuid::new_v4().simple().to_string(),
            _ => unreachable!(),
        }
        assert_eq!(
            wire_status(
                Full::new(tonic::codegen::Bytes::from(framed(&q))),
                "ReadAfter",
                Some(("concert", "fixture-concert")),
                false
            )
            .await,
            Code::InvalidArgument,
            "case {which}"
        );
    }
    for bytes in [
        vec![0, 0, 0, 0, 30, 0x0a],
        vec![0, 0, 0, 0, 1, 0xff],
        vec![0, 0, 0, 0, 0],
        vec![0, 0, 0, 0x20, 0],
    ] {
        assert_ne!(
            wire_status(
                Full::new(tonic::codegen::Bytes::from(bytes)),
                "ReadAfter",
                Some(("concert", "fixture-concert")),
                false
            )
            .await,
            Code::Ok
        );
    }
    let mut q = head_query();
    q.consumer_id = "x".repeat(5000);
    assert_ne!(
        wire_status(
            Full::new(tonic::codegen::Bytes::from(framed(&q))),
            "ReadHead",
            Some(("concert", "fixture-concert")),
            false
        )
        .await,
        Code::Ok
    );
}

#[tokio::test]
async fn original_ingress_budget_includes_waiting_for_request_body() {
    use http_body_util::StreamBody;
    // A body with no frames and no EOF must be bounded before decoding, not only
    // once the generated handler starts. Keep its sender alive until completion.
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<_, std::convert::Infallible>>(1);
    let body = StreamBody::new(tokio_stream::wrappers::ReceiverStream::new(receiver));
    let start = Instant::now();
    assert_eq!(
        wire_status(
            body,
            "ReadHead",
            Some(("concert", "fixture-concert")),
            false
        )
        .await,
        Code::DeadlineExceeded
    );
    assert!(
        start.elapsed() >= Duration::from_millis(900)
            && start.elapsed() < Duration::from_millis(1500)
    );
    drop(sender);
}

#[path = "concert_invalidation_rpc_support/native.rs"]
mod native;
