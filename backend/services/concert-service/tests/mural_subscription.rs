use async_trait::async_trait;
use chrono::Utc;
use concert_application::mural::*;
use concert_domain::mural::*;
use concert_service::world::mural_subscription::*;
use std::sync::Mutex;
use uuid::Uuid;
struct Source {
    snapshot: Mutex<MuralSnapshot>,
    delta: Mutex<MuralDelta>,
    allowed: Mutex<bool>,
}
#[async_trait]
impl MuralReader for Source {
    async fn authorize(&self, _: MuralActor) -> Result<(), MuralError> {
        if *self.allowed.lock().unwrap() {
            Ok(())
        } else {
            Err(MuralError::Denied)
        }
    }
    async fn snapshot(&self, a: MuralActor) -> Result<MuralSnapshot, MuralError> {
        self.authorize(a).await?;
        Ok(self.snapshot.lock().unwrap().clone())
    }
    async fn delta(&self, a: MuralActor, _: i64) -> Result<MuralDelta, MuralError> {
        self.authorize(a).await?;
        Ok(self.delta.lock().unwrap().clone())
    }
}
fn source(event: Uuid) -> Source {
    Source {
        snapshot: Mutex::new(MuralSnapshot {
            snapshot_id: Uuid::new_v4(),
            event_id: event,
            revision: 1,
            state: MuralState::Editable,
            width: 128,
            height: 128,
            palette: vec!["#123ABC".into(); 16],
            pixels: vec![0; PIXELS],
            created_at: Utc::now(),
            server_time: Utc::now(),
            cooldown_ms: 0,
            next_allowed_at: None,
        }),
        delta: Mutex::new(MuralDelta {
            event_id: event,
            from_revision: 1,
            to_revision: 1,
            reset_required: false,
            operations: vec![],
        }),
        allowed: Mutex::new(true),
    }
}
#[tokio::test]
async fn queued_snapshot_revocation_epoch_and_bad_delta_are_safe() -> anyhow::Result<()> {
    let a = MuralActor {
        event: Uuid::new_v4(),
        actor: Uuid::new_v4(),
    };
    let epoch = Uuid::new_v4();
    let source = source(a.event);
    let mut sub = MuralSubscription::new(a, epoch, 2)?;
    sub.poll(&source, epoch).await?;
    assert_eq!(sub.queued(), 1);
    *source.allowed.lock().unwrap() = false;
    assert!(matches!(
        sub.next(&source, epoch).await,
        Err(MuralError::Denied)
    ));
    assert_eq!(sub.queued(), 0);
    *source.allowed.lock().unwrap() = true;
    sub.poll(&source, epoch).await?;
    assert!(matches!(
        sub.next(&source, epoch).await?,
        Some(MuralDelivery::Snapshot { reset: true, .. })
    ));
    source.delta.lock().unwrap().to_revision = 3;
    source.snapshot.lock().unwrap().revision = 3;
    sub.poll(&source, epoch).await?;
    assert!(matches!(
        sub.next(&source, epoch).await?,
        Some(MuralDelivery::Snapshot { reset: true, .. })
    ));
    source.snapshot.lock().unwrap().revision = 1;
    assert_eq!(
        sub.poll(&source, Uuid::new_v4()).await,
        Err(MuralError::InvalidState),
        "new epoch cannot rewind prior delivered revision"
    );
    assert_eq!(sub.queued(), 0);
    source.snapshot.lock().unwrap().revision = 3;
    sub.poll(&source, Uuid::new_v4()).await?;
    assert_eq!(sub.queued(), 1);
    assert!(matches!(
        sub.next(&source, Uuid::nil()).await,
        Err(MuralError::InvalidInput)
    ));
    assert_eq!(sub.queued(), 0);
    assert!(MuralSubscription::new(a, epoch, 0).is_err());
    assert!(MuralSubscription::new(a, epoch, 17).is_err());
    Ok(())
}
#[test]
fn delta_requires_exact_sequence_bounds_and_event() {
    let event = Uuid::new_v4();
    let op = PublicPixelOperation {
        operation_id: Uuid::new_v4(),
        event_id: event,
        revision: 2,
        x: 127,
        y: 127,
        color_index: 15,
        committed_at: Utc::now(),
    };
    let mut delta = MuralDelta {
        event_id: event,
        from_revision: 1,
        to_revision: 2,
        reset_required: false,
        operations: vec![op],
    };
    assert!(delta.contiguous());
    delta.operations[0].x = 128;
    assert!(!delta.contiguous());
    delta.operations[0].x = 127;
    delta.operations[0].event_id = Uuid::new_v4();
    assert!(!delta.contiguous());
    delta.operations[0].event_id = event;
    delta.operations[0].revision = 3;
    assert!(!delta.contiguous());
}
