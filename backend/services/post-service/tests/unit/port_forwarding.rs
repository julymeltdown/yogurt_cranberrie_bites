use anyhow::Result;
use async_trait::async_trait;
use post_service::application::ports::FriendClientPort;
use post_service::domain::recommendation::FollowerPage;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Default)]
struct OptimizedFriendClient {
    raw_calls: AtomicUsize,
    feed_calls: Mutex<Vec<Uuid>>,
    bounded_calls: Mutex<Vec<(Uuid, usize)>>,
    page_calls: Mutex<Vec<(Uuid, Option<String>, i32)>>,
    count_calls: Mutex<Vec<Uuid>>,
}

#[async_trait]
impl FriendClientPort for OptimizedFriendClient {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_service::application::feed_authors::FeedAuthorsDto> {
        self.feed_calls.lock().unwrap().push(viewer_id);
        post_service::application::feed_authors::FeedAuthorsDto::new(viewer_id, vec![viewer_id])
    }

    async fn fetch_following_ids(&self, _: Uuid) -> Result<Vec<Uuid>> {
        self.raw_calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("full following lookup is forbidden by this bounded fixture")
    }

    async fn fetch_follower_ids(&self, _: Uuid) -> Result<Vec<Uuid>> {
        self.raw_calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("full follower lookup is forbidden by this paginated fixture")
    }

    async fn is_following(&self, _: Uuid, _: Uuid) -> Result<bool> {
        anyhow::bail!("unrelated relationship call")
    }

    async fn fetch_following_ids_bounded(&self, user_id: Uuid, max: usize) -> Result<Vec<Uuid>> {
        self.bounded_calls.lock().unwrap().push((user_id, max));
        Ok(vec![user_id])
    }

    async fn fetch_follower_page(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<FollowerPage> {
        self.page_calls
            .lock()
            .unwrap()
            .push((user_id, cursor.map(str::to_owned), limit));
        Ok(FollowerPage {
            follower_ids: vec![user_id],
            next_cursor: Some("owner-opaque-next-cursor".into()),
            total_count: 50_000,
        })
    }

    async fn follower_count(&self, user_id: Uuid) -> Result<i64> {
        self.count_calls.lock().unwrap().push(user_id);
        Ok(50_000)
    }
}

// Retain generic Arc bounds as well as the production Arc<dyn ...> receiver.
async fn bounded<C: FriendClientPort>(client: &C, id: Uuid) -> Result<Vec<Uuid>> {
    client.fetch_following_ids_bounded(id, 27).await
}

async fn page<C: FriendClientPort>(client: &C, id: Uuid) -> Result<FollowerPage> {
    client
        .fetch_follower_page(id, Some("owner-signed-input-cursor"), 37)
        .await
}

async fn count<C: FriendClientPort>(client: &C, id: Uuid) -> Result<i64> {
    client.follower_count(id).await
}

#[tokio::test]
async fn friend_arc_forwards_bounded_following_without_full_lookup() -> Result<()> {
    let concrete = Arc::new(OptimizedFriendClient::default());
    let production_receiver: Arc<dyn FriendClientPort> = concrete.clone();
    let user = Uuid::new_v4();
    assert_eq!(bounded(&production_receiver, user).await?, vec![user]);
    assert_eq!(bounded(&concrete, user).await?, vec![user]);
    assert_eq!(concrete.raw_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        *concrete.bounded_calls.lock().unwrap(),
        vec![(user, 27), (user, 27)]
    );
    Ok(())
}

#[tokio::test]
async fn friend_arc_preserves_owner_follower_cursor_and_limit() -> Result<()> {
    let concrete = Arc::new(OptimizedFriendClient::default());
    let production_receiver: Arc<dyn FriendClientPort> = concrete.clone();
    let user = Uuid::new_v4();
    for result in [
        page(&production_receiver, user).await?,
        page(&concrete, user).await?,
    ] {
        assert_eq!(result.follower_ids, vec![user]);
        assert_eq!(
            result.next_cursor.as_deref(),
            Some("owner-opaque-next-cursor")
        );
        assert_eq!(result.total_count, 50_000);
    }
    assert_eq!(concrete.raw_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        *concrete.page_calls.lock().unwrap(),
        vec![
            (user, Some("owner-signed-input-cursor".into()), 37),
            (user, Some("owner-signed-input-cursor".into()), 37)
        ]
    );
    Ok(())
}

#[tokio::test]
async fn friend_arc_forwards_count_without_materializing_followers() -> Result<()> {
    let concrete = Arc::new(OptimizedFriendClient::default());
    let production_receiver: Arc<dyn FriendClientPort> = concrete.clone();
    let user = Uuid::new_v4();
    assert_eq!(count(&production_receiver, user).await?, 50_000);
    assert_eq!(count(&concrete, user).await?, 50_000);
    assert_eq!(concrete.raw_calls.load(Ordering::SeqCst), 0);
    assert_eq!(*concrete.count_calls.lock().unwrap(), vec![user, user]);
    Ok(())
}

#[tokio::test]
async fn repository_arc_preserves_receipt_identity_and_all_states() -> Result<()> {
    use post_service::application::dto::mutation_dto::{
        MutationReceiptStateDto, MutationReceiptStatusDto,
    };
    use post_service::application::ports::PostRepositoryPort;
    let concrete = Arc::new(super::support::FakePostRepository::default());
    let receiver: Arc<dyn PostRepositoryPort> = concrete.clone();
    let operation_id = Uuid::new_v4();
    for state in [
        MutationReceiptStateDto::Replayable,
        MutationReceiptStateDto::Expired,
        MutationReceiptStateDto::Incomplete,
    ] {
        let expected = MutationReceiptStatusDto {
            operation_id,
            actor_id: Uuid::new_v4(),
            scope: "post.v1.CreatePost".into(),
            committed_at: super::support::instant(1_700_000_000),
            replay_until: super::support::instant(1_700_086_400),
            state,
        };
        *concrete.receipt_status.lock().unwrap() = Some(expected.clone());
        assert_eq!(
            receiver.mutation_receipt_status(operation_id).await?,
            Some(expected.clone())
        );
        assert_eq!(
            concrete.mutation_receipt_status(operation_id).await?,
            Some(expected)
        );
    }
    let missing = Uuid::new_v4();
    assert!(receiver.mutation_receipt_status(missing).await?.is_none());
    assert!(concrete.mutation_receipt_status(missing).await?.is_none());
    assert_eq!(
        *concrete.receipt_queries.lock().unwrap(),
        vec![
            operation_id,
            operation_id,
            operation_id,
            operation_id,
            operation_id,
            operation_id,
            missing,
            missing
        ]
    );
    Ok(())
}

#[tokio::test]
async fn friend_arc_forwards_required_owner_filtered_author_data() -> Result<()> {
    let concrete = Arc::new(OptimizedFriendClient::default());
    let receiver: Arc<dyn FriendClientPort> = Arc::new(concrete.clone());
    let viewer = Uuid::new_v4();
    let data = receiver.fetch_feed_authors(viewer).await?;
    assert_eq!(data.viewer_id(), viewer);
    assert_eq!(data.author_ids(), &[viewer]);
    assert_eq!(*concrete.feed_calls.lock().unwrap(), vec![viewer]);
    assert_eq!(concrete.raw_calls.load(Ordering::SeqCst), 0);
    Ok(())
}
