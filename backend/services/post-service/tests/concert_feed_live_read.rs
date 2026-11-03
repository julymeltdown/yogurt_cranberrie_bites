//! Read wire validation only until the canonical guard producer is integrated.
use chrono::{Duration, Utc};
use platform::grpc_auth::{VerifiedCaller, Workload};
use post_service::adapters::r#in::grpc::concert_feed::{
    proto,
    read_wire::{self, DecodedRead, ReadQuery},
    PostConcertFeedGrpcServer,
};
use proto::post_concert_feed_service_server::PostConcertFeedService;
use tonic::{Code, Request};
use uuid::Uuid;
fn context() -> proto::Context {
    proto::Context {
        actor_id: Uuid::from_u128(1).to_string(),
        event_id: Uuid::from_u128(2).to_string(),
        session_family_id: Uuid::from_u128(3).to_string(),
        request_id: Uuid::from_u128(4).to_string(),
        original_authority_digest: vec![5; 32],
        expires_at: (Utc::now() + Duration::seconds(2)).to_rfc3339(),
    }
}
fn page() -> proto::ListPostsRequest {
    proto::ListPostsRequest {
        context: Some(context()),
        upper: Utc::now().to_rfc3339(),
        after: None,
        filter: "ALL".into(),
        limit: 20,
    }
}
#[test]
fn all_read_bindings_are_required_and_never_synthesized() {
    let good = proto::GetPostRequest {
        context: Some(context()),
        post_id: Uuid::from_u128(9).to_string(),
    };
    let decoded = DecodedRead::try_from(good.clone()).unwrap();
    assert_eq!(decoded.context.original_authority, [5; 32]);
    assert!(matches!(decoded.query,ReadQuery::Post(id) if id==Uuid::from_u128(9)));
    let mut missing = good.clone();
    missing.context = None;
    assert!(DecodedRead::try_from(missing).is_err());
    for field in 0..6 {
        let mut invalid = good.clone();
        let c = invalid.context.as_mut().unwrap();
        match field {
            0 => c.actor_id = Uuid::nil().to_string(),
            1 => c.event_id = "event".into(),
            2 => c.session_family_id = Uuid::nil().to_string(),
            3 => c.request_id = "request".into(),
            4 => c.original_authority_digest = vec![5; 31],
            _ => c.expires_at = (Utc::now() - Duration::seconds(1)).to_rfc3339(),
        };
        assert!(DecodedRead::try_from(invalid).is_err());
    }
    let mut invalid = good;
    invalid.post_id = "ABCDEF00-0000-0000-0000-000000000001".into();
    assert!(DecodedRead::try_from(invalid).is_err());
}
#[test]
fn list_window_filter_and_limit_fail_before_storage() {
    assert!(DecodedRead::try_from(page()).is_ok());
    for limit in [0, 101, u32::MAX] {
        let mut r = page();
        r.limit = limit;
        assert!(DecodedRead::try_from(r).is_err());
    }
    let mut r = page();
    r.filter = "PRIVATE".into();
    assert!(DecodedRead::try_from(r).is_err());
    let mut r = page();
    r.upper = (Utc::now() + Duration::seconds(1)).to_rfc3339();
    assert!(DecodedRead::try_from(r).is_err());
    let mut r = page();
    r.after = Some(proto::Position {
        created_at: (Utc::now() + Duration::seconds(1)).to_rfc3339(),
        id: Uuid::from_u128(8).to_string(),
    });
    assert!(DecodedRead::try_from(r).is_err());
}
#[test]
fn comment_reads_keep_actual_parent_and_exact_scan_boundary() {
    let request = proto::ListCommentsRequest {
        context: Some(context()),
        post_id: Uuid::from_u128(9).to_string(),
        upper: Utc::now().to_rfc3339(),
        after: None,
        limit: 20,
    };
    assert!(
        matches!(DecodedRead::try_from(request).unwrap().query,ReadQuery::Comments {post,limit:20,..} if post==Uuid::from_u128(9))
    );
    let position = post_application::concert_feed::Position {
        created_at: Utc::now(),
        id: Uuid::from_u128(11),
    };
    let response = read_wire::comments(post_application::concert_feed::Page {
        items: vec![],
        scanned_through: Some(position),
        has_more: true,
    });
    assert!(response.comments.is_empty());
    assert!(response.has_more);
    assert_eq!(
        response.scanned_through.unwrap().id,
        position.id.to_string()
    );
}
#[test]
fn owner_errors_are_redacted_and_authority_failure_is_not_empty_success() {
    use post_domain::concert_feed::Error;
    assert_eq!(read_wire::status(Error::Forbidden).code(), Code::NotFound);
    assert_eq!(read_wire::status(Error::NotFound).code(), Code::NotFound);
    assert_eq!(
        read_wire::status(Error::Forbidden).message(),
        read_wire::status(Error::NotFound).message()
    );
    assert_eq!(
        read_wire::status(Error::Authority).code(),
        Code::Unavailable
    );
    assert_eq!(
        read_wire::status(Error::Unavailable).code(),
        Code::Unavailable
    );
}
#[tokio::test]
async fn workload_and_digest_cannot_enable_any_read_or_write_handler() {
    let server = PostConcertFeedGrpcServer;
    let mut request = Request::new(page());
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Concert));
    assert_eq!(
        server.list_posts(request).await.unwrap_err().code(),
        Code::Unavailable
    );
    let mut request = Request::new(proto::GetPostRequest {
        context: Some(context()),
        post_id: Uuid::from_u128(9).to_string(),
    });
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Concert));
    assert_eq!(
        server.get_post(request).await.unwrap_err().code(),
        Code::Unavailable
    );
    let mut request = Request::new(proto::GetCommentRequest {
        context: Some(context()),
        comment_id: Uuid::from_u128(11).to_string(),
    });
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Concert));
    assert_eq!(
        server.get_comment(request).await.unwrap_err().code(),
        Code::Unavailable
    );
    let mut request = Request::new(proto::ListCommentsRequest {
        context: Some(context()),
        post_id: Uuid::from_u128(9).to_string(),
        upper: Utc::now().to_rfc3339(),
        after: None,
        limit: 20,
    });
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Concert));
    assert_eq!(
        server.list_comments(request).await.unwrap_err().code(),
        Code::Unavailable
    );
    let mut request = Request::new(proto::ExecuteCommandRequest::default());
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Concert));
    assert_eq!(
        server.execute_command(request).await.unwrap_err().code(),
        Code::Unavailable
    );
}

fn registry_context() -> post_application::concert_feed::Context {
    DecodedRead::try_from(proto::GetPostRequest {
        context: Some(context()),
        post_id: Uuid::from_u128(9).to_string(),
    })
    .unwrap()
    .context
}
#[tokio::test]
async fn settlement_seals_late_reads_and_waits_for_real_finish() {
    use post_service::concert_feed::read_registry::ReadRegistry;
    let registry = ReadRegistry::new(2).unwrap();
    let context = registry_context();
    let guard = Uuid::new_v4();
    let first = registry.enter(guard, &context).unwrap();
    let second = registry.enter(guard, &context).unwrap();
    let settling = registry.seal(
        registry.process_epoch(),
        guard,
        &context,
        tokio::time::Instant::now() + std::time::Duration::from_secs(1),
    );
    tokio::pin!(settling);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), &mut settling)
            .await
            .is_err()
    );
    assert!(registry.enter(guard, &context).is_err());
    first.finish().unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), &mut settling)
            .await
            .is_err()
    );
    second.finish().unwrap();
    let settled = settling.await.unwrap();
    assert_eq!(settled.guard_id(), guard);
    assert_eq!(settled.process_epoch(), registry.process_epoch());
    assert!(settled.context() == &context);
    assert!(registry.enter(guard, &context).is_err());
}
#[tokio::test]
async fn cancelled_or_unknown_read_is_never_positive_settlement() {
    use post_service::concert_feed::read_registry::ReadRegistry;
    let registry = ReadRegistry::new(1).unwrap();
    let context = registry_context();
    let guard = Uuid::new_v4();
    drop(registry.enter(guard, &context).unwrap());
    assert!(registry
        .seal(
            registry.process_epoch(),
            guard,
            &context,
            tokio::time::Instant::now() + std::time::Duration::from_secs(1)
        )
        .await
        .is_err());
    assert!(registry.enter(guard, &context).is_err());
    assert!(registry.enter(Uuid::new_v4(), &context).is_err());
}
#[tokio::test]
async fn restarted_producer_and_wrong_context_cannot_settle_old_work() {
    use post_service::concert_feed::read_registry::ReadRegistry;
    let old = ReadRegistry::new(2).unwrap();
    let new = ReadRegistry::new(2).unwrap();
    let context = registry_context();
    let guard = Uuid::new_v4();
    old.enter(guard, &context).unwrap().finish().unwrap();
    let stop = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    assert!(new
        .seal(old.process_epoch(), guard, &context, stop)
        .await
        .is_err());
    assert!(new
        .seal(new.process_epoch(), guard, &context, stop)
        .await
        .is_err());
    let mut other = context.clone();
    other.request_id = Uuid::new_v4();
    assert!(old
        .seal(old.process_epoch(), guard, &other, stop)
        .await
        .is_err());
    assert!(old.enter(guard, &other).is_err());
    assert!(old
        .seal(old.process_epoch(), guard, &context, stop)
        .await
        .is_ok());
}
