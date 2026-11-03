use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use friend_service::application::dto::friend_dto::ApplyUserDeletionDto;
use friend_service::application::ports::UserLifecycleRepositoryPort;
use friend_service::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

type UserDeletionCall = (Uuid, Uuid, Uuid, DateTime<Utc>);

#[derive(Clone, Default)]
struct RecordingRepository {
    calls: Arc<Mutex<Vec<UserDeletionCall>>>,
}

#[async_trait]
impl UserLifecycleRepositoryPort for RecordingRepository {
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push((saga_id, user_id, actor_id, retention_until));
        Ok(())
    }
}

#[tokio::test]
async fn account_deletion_application_is_repeatable_and_preserves_saga_context() {
    let repository = RecordingRepository::default();
    let use_case = ApplyUserDeletionUseCase::new(repository.clone());
    let saga_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let retention = Utc.with_ymd_and_hms(2026, 9, 2, 12, 0, 0).unwrap();
    let request = ApplyUserDeletionDto {
        saga_id: saga_id.to_string(),
        user_id: user_id.to_string(),
        actor_id: user_id.to_string(),
        retention_until: retention.to_rfc3339(),
    };

    assert!(
        use_case
            .execute(request.clone())
            .await
            .expect("first")
            .applied
    );
    assert!(use_case.execute(request).await.expect("repeat").applied);
    assert_eq!(
        repository.calls.lock().unwrap().as_slice(),
        &[
            (saga_id, user_id, user_id, retention),
            (saga_id, user_id, user_id, retention),
        ]
    );
}
