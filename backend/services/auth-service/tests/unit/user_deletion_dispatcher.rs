use anyhow::Result;
use async_trait::async_trait;
use auth_service::adapters::out::grpc::user_deletion_dispatcher::{
    UserDeletionDispatchRequest, UserDeletionTargetClientPort,
};
use auth_service::application::ports::UserDeletionDispatcherPort;
use auth_service::domain::user_deletion::UserDeletionTarget;
use chrono::{TimeZone, Utc};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Default)]
struct RecordingClient {
    calls: Arc<Mutex<Vec<UserDeletionDispatchRequest>>>,
}

#[async_trait]
impl UserDeletionTargetClientPort for RecordingClient {
    async fn apply(&self, request: UserDeletionDispatchRequest) -> Result<()> {
        self.calls.lock().unwrap().push(request);
        Ok(())
    }
}

#[tokio::test]
async fn dispatcher_routes_exact_saga_context_to_the_selected_service() {
    let profile = RecordingClient::default();
    let friend = RecordingClient::default();
    let post = RecordingClient::default();
    let notification = RecordingClient::default();
    let dispatcher = auth_service::adapters::out::grpc::user_deletion_dispatcher::GrpcUserDeletionDispatcher::from_clients(
        Arc::new(profile.clone()),
        Arc::new(friend.clone()),
        Arc::new(post.clone()),
        Arc::new(notification.clone()),
    );
    let saga_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let retention_until = Utc.with_ymd_and_hms(2026, 9, 3, 12, 0, 0).unwrap();

    dispatcher
        .apply(
            (UserDeletionTarget::Post).into(),
            saga_id,
            user_id,
            actor_id,
            retention_until,
        )
        .await
        .expect("dispatch");

    assert!(profile.calls.lock().unwrap().is_empty());
    assert!(friend.calls.lock().unwrap().is_empty());
    assert!(notification.calls.lock().unwrap().is_empty());
    assert_eq!(
        post.calls.lock().unwrap().as_slice(),
        &[UserDeletionDispatchRequest {
            saga_id,
            user_id,
            actor_id,
            retention_until,
        }]
    );
}
