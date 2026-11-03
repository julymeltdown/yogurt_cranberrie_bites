use async_trait::async_trait;
use chrono::Utc;
use concert_application::feed::*;
use concert_domain::feed::*;
use concert_service::feed::owner::{proto, PostOwner, PostTransport, UnavailablePostTransport};
use uuid::Uuid;
fn context() -> Context {
    Context {
        actor: Uuid::from_u128(1),
        event: Uuid::from_u128(2),
        session_family: Uuid::from_u128(3),
        request: Uuid::from_u128(4),
        authority_digest: [5; 32],
        expires_at: Utc::now() + chrono::Duration::seconds(2),
    }
}
struct Transport {
    wrong_event: bool,
}
fn record(c: &proto::Context) -> proto::PostRecord {
    proto::PostRecord {
        post_id: Uuid::from_u128(9).to_string(),
        event_id: c.event_id.clone(),
        author_id: Uuid::from_u128(10).to_string(),
        content: Some(proto::Content {
            text: "Current post".into(),
            media_ids: vec![],
            links: vec![],
        }),
        revision: 3,
        state: "PUBLISHED".into(),
        created_at: (Utc::now() - chrono::Duration::seconds(5)).to_rfc3339(),
        edited_at: None,
        comment_count: 1,
        reaction_count: 0,
        reacted_by_me: false,
        is_public: true,
    }
}
#[async_trait]
impl PostTransport for Transport {
    async fn post(
        &self,
        r: proto::GetPostRequest,
    ) -> Result<proto::GetPostResponse, tonic::Status> {
        let c = r.context.unwrap();
        assert_eq!(c.original_authority_digest, vec![5; 32]);
        let mut p = record(&c);
        if self.wrong_event {
            p.event_id = Uuid::new_v4().to_string()
        }
        Ok(proto::GetPostResponse { post: Some(p) })
    }
    async fn posts(
        &self,
        r: proto::ListPostsRequest,
    ) -> Result<proto::ListPostsResponse, tonic::Status> {
        assert_eq!(r.filter, "PARTY_LINK");
        assert_eq!(r.limit, 20);
        assert!(r.after.is_none());
        Ok(proto::ListPostsResponse {
            posts: vec![],
            scanned_through: Some(proto::Position {
                created_at: (Utc::now() - chrono::Duration::seconds(5)).to_rfc3339(),
                id: Uuid::from_u128(9).to_string(),
            }),
            has_more: true,
        })
    }
}
#[tokio::test]
async fn owner_reader_preserves_scan_metadata_and_exact_context() {
    let c = context();
    let reader = PostOwner {
        transport: Transport { wrong_event: false },
    };
    let p = reader.post(c, Uuid::from_u128(9)).await.unwrap().unwrap();
    assert_eq!(p.event, c.event);
    assert_eq!(p.revision, 3);
    let page = reader
        .page(
            c,
            Window {
                upper: Utc::now(),
                after: None,
            },
            Filter::PartyLink,
            20,
        )
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert!(page.has_more);
    assert!(page.scanned_through.is_some());
}
#[tokio::test]
async fn owner_wrong_event_expiry_and_default_transport_fail_closed() {
    let c = context();
    assert!(matches!(
        PostOwner {
            transport: Transport { wrong_event: true }
        }
        .post(c, Uuid::from_u128(9))
        .await,
        Err(FeedError::DependencyUnavailable)
    ));
    assert!(matches!(
        PostOwner {
            transport: UnavailablePostTransport
        }
        .post(c, Uuid::from_u128(9))
        .await,
        Err(FeedError::DependencyUnavailable)
    ));
    assert!(matches!(
        PostOwner {
            transport: Transport { wrong_event: false }
        }
        .post(
            Context {
                expires_at: Utc::now(),
                ..c
            },
            Uuid::from_u128(9)
        )
        .await,
        Err(FeedError::PermissionDenied)
    ));
}
