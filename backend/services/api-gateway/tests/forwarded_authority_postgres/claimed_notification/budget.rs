//! Event-scoped observations for the original-deadline network acceptance.
//! No substitute owner evidence or acknowledgement is returned by these probes.
use super::*;
use post_service::application::{
    claimed_notification::*, delivery_effect::DeliveryEffectDeadlineDto,
};
use std::{sync::Mutex, time::Instant};
use tokio::sync::{Notify, Semaphore};

#[derive(Clone, Default)]
pub(super) struct Probe(Arc<Mutex<Option<Arc<Case>>>>);

#[derive(Debug)]
struct Phase {
    name: &'static str,
    entered: Instant,
    finished: Option<Instant>,
    outcome: &'static str,
}

#[derive(Default)]
struct Observations {
    original: Option<DeliveryEffectDeadlineDto>,
    issue_remaining: Option<Duration>,
    // Used only to associate the actual destination request with this event.
    // Never include the signed token in diagnostics.
    issued_token: Option<String>,
    ingress_budget: Vec<Duration>,
    phases: Vec<Phase>,
    deadline_mismatch: bool,
    original_exceeds_config: bool,
}

pub(super) struct Case {
    event: Uuid,
    actor: Uuid,
    recipient: Uuid,
    observations: Mutex<Observations>,
    release: Semaphore,
    changed: Notify,
}

pub(super) struct Armed {
    pub case: Arc<Case>,
    probe: Probe,
}

impl Drop for Armed {
    fn drop(&mut self) {
        self.case.release.close();
        self.probe.0.lock().unwrap().take();
    }
}

pub(super) struct PhaseGuard {
    case: Arc<Case>,
    index: usize,
}
impl PhaseGuard {
    pub fn complete(&self, success: bool) {
        self.case.observations.lock().unwrap().phases[self.index].outcome =
            if success { "OK" } else { "ERR" };
    }
}
impl Drop for PhaseGuard {
    fn drop(&mut self) {
        self.case.observations.lock().unwrap().phases[self.index].finished = Some(Instant::now());
        self.case.changed.notify_one();
    }
}

impl Probe {
    pub fn arm(&self, event: Uuid, actor: Uuid, recipient: Uuid) -> Armed {
        let case = Arc::new(Case {
            event,
            actor,
            recipient,
            observations: Mutex::default(),
            release: Semaphore::new(0),
            changed: Notify::new(),
        });
        assert!(self.0.lock().unwrap().replace(case.clone()).is_none());
        Armed {
            case,
            probe: self.clone(),
        }
    }

    fn event(&self, event: Uuid) -> Option<Arc<Case>> {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .filter(|c| c.event == event)
            .cloned()
    }

    pub fn policy(&self, actor: Uuid, recipient: Uuid) -> Option<Arc<Case>> {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .filter(|c| c.actor == actor && c.recipient == recipient)
            .cloned()
    }

    pub fn ingress(&self, token: Option<&str>, budget: Duration) -> Option<PhaseGuard> {
        let case = self.0.lock().unwrap().clone()?;
        let mut o = case.observations.lock().unwrap();
        if token.is_none() || token != o.issued_token.as_deref() {
            return None;
        }
        o.ingress_budget.push(budget);
        drop(o);
        Some(case.enter("destination"))
    }
}

impl Case {
    pub fn enter(self: &Arc<Self>, name: &'static str) -> PhaseGuard {
        let mut o = self.observations.lock().unwrap();
        let index = o.phases.len();
        o.phases.push(Phase {
            name,
            entered: Instant::now(),
            finished: None,
            outcome: "CANCELLED",
        });
        PhaseGuard {
            case: self.clone(),
            index,
        }
    }

    pub fn refreshing(&self) -> bool {
        self.observations
            .lock()
            .unwrap()
            .phases
            .iter()
            .any(|p| p.name == "refresh")
    }

    pub async fn hold_refresh(self: &Arc<Self>) {
        let phase = self.enter("refresh_hold");
        // Explicit release after the worker returns. Closing also unblocks a
        // waiter that starts late, so cancellation cannot lose a notification.
        let _ = self.release.acquire().await;
        phase.complete(true);
    }

    pub async fn finish(&self, returned: Instant, wall_returned: i64) -> anyhow::Result<()> {
        self.release.close();
        let drained = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let changed = self.changed.notified();
                if self
                    .observations
                    .lock()
                    .unwrap()
                    .phases
                    .iter()
                    .all(|p| p.finished.is_some())
                {
                    break;
                }
                changed.await;
            }
        })
        .await;
        let o = self.observations.lock().unwrap();
        eprintln!("NOTIFICATION_ORIGINAL_BUDGET: event={} phases={:?} issue_remaining={:?} ingress_budgets={:?} deadline_mismatch={} drained={}",
            self.event, o.phases, o.issue_remaining, o.ingress_budget, o.deadline_mismatch, drained.is_ok());
        anyhow::ensure!(drained.is_ok(), "observed notification work did not drain");
        anyhow::ensure!(
            !o.original_exceeds_config,
            "delivery fence exceeded configured 480 ms budget"
        );
        let original = o
            .original
            .ok_or_else(|| anyhow::anyhow!("presenter not reached"))?;
        anyhow::ensure!(
            returned >= original.monotonic_stop()
                || wall_returned >= original.expires_at().timestamp_micros(),
            "worker returned before the original deadline"
        );
        anyhow::ensure!(!o.deadline_mismatch, "RPC changed the original deadline");
        for (name, must_succeed) in [
            ("delivery", false),
            ("issue", true),
            ("issue_policy", true),
            ("destination", false),
            ("refresh", false),
            ("refresh_policy", false),
            ("refresh_query", true),
            ("refresh_hold", false),
        ] {
            let phases: Vec<_> = o.phases.iter().filter(|p| p.name == name).collect();
            anyhow::ensure!(phases.len() == 1, "expected exactly one {name} phase");
            anyhow::ensure!(
                phases[0].entered < original.monotonic_stop(),
                "{name} entered after deadline"
            );
            if must_succeed {
                anyhow::ensure!(phases[0].outcome == "OK", "{name} did not complete");
            }
        }
        let bound = o
            .issue_remaining
            .ok_or_else(|| anyhow::anyhow!("Issue did not return a grant"))?;
        anyhow::ensure!(
            o.ingress_budget.len() == 1,
            "actual destination timeout missing or repeated"
        );
        // The client computes its timeout AFTER Issue returns. The server-side
        // Issue completion is an earlier observation, hence a safe upper bound
        // independent of network transit time. Tonic truncates to the finest unit
        // fitting eight digits; no scheduler-dependent tolerance is needed.
        anyhow::ensure!(
            !o.ingress_budget[0].is_zero() && o.ingress_budget[0] <= bound,
            "destination received a renewed budget: {:?} exceeds {:?}",
            o.ingress_budget[0],
            bound
        );
        Ok(())
    }
}

/// Observe the deadline at the fence's delivery port, BEFORE owner queries.
/// Neither the use case nor transport can redefine what "original" means.
pub(super) struct Delivery {
    pub inner: Arc<dyn ClaimedNotificationDeliveryPort>,
    pub probe: Probe,
}
#[tonic::async_trait]
impl ClaimedNotificationDeliveryPort for Delivery {
    async fn deliver_claimed_notifications(
        &self,
        claim: &TargetDeliveryClaimDto,
        deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        let case = self.probe.event(claim.event.event_id);
        if let Some(case) = &case {
            let mut o = case.observations.lock().unwrap();
            o.original_exceeds_config |= deadline
                .monotonic_stop()
                .saturating_duration_since(Instant::now())
                > Duration::from_millis(480)
                || deadline.expires_at() > chrono::Utc::now() + chrono::Duration::milliseconds(480);
            o.deadline_mismatch |= o.original.is_some();
            o.original = Some(deadline);
        }
        let phase = case.as_ref().map(|c| c.enter("delivery"));
        let result = self
            .inner
            .deliver_claimed_notifications(claim, deadline)
            .await;
        if let Some(p) = &phase {
            p.complete(result.is_ok());
        }
        result
    }
}

pub(super) struct Transport {
    pub inner: Arc<dyn ClaimedNotificationTransportPort>,
    pub probe: Probe,
}
#[tonic::async_trait]
impl ClaimedNotificationTransportPort for Transport {
    async fn select_page(
        &self,
        r: &NotificationPageRequestDto,
    ) -> anyhow::Result<NotificationPageResultDto> {
        self.inner.select_page(r).await
    }
    async fn write(
        &self,
        r: &ClaimedNotificationWriteDto,
    ) -> anyhow::Result<ClaimedNotificationAckDto> {
        if let Some(case) = self.probe.event(r.owner().lookup().claim().event.event_id) {
            let mut o = case.observations.lock().unwrap();
            o.deadline_mismatch |= o.original.is_none_or(|original| {
                r.deadline().monotonic_stop() > original.monotonic_stop()
                    || r.deadline().expires_at() > original.expires_at()
            });
        }
        self.inner.write(r).await
    }
}

pub(super) struct GrantServer {
    pub inner: NotificationGrantGrpcServer,
    pub probe: Probe,
}
#[tonic::async_trait]
impl authority::notification_grant_service_server::NotificationGrantService for GrantServer {
    async fn issue_notification_grant(
        &self,
        r: Request<authority::IssueNotificationGrantRequest>,
    ) -> Result<Response<authority::IssueNotificationGrantResponse>, Status> {
        let case = NotificationBinding::decode(&r.get_ref().binding)
            .ok()
            .and_then(|b| self.probe.event(b.claim().event_id));
        let phase = case.as_ref().map(|c| c.enter("issue"));
        if let Some(c) = &case {
            let mut o = c.observations.lock().unwrap();
            o.deadline_mismatch |= o
                .original
                .is_none_or(|d| r.get_ref().deadline_micros > d.expires_at().timestamp_micros());
        }
        let result = self.inner.issue_notification_grant(r).await;
        if let Some(c) = &case {
            if let Ok(reply) = &result {
                if let Some(authority::issue_notification_grant_response::Result::Granted(g)) =
                    &reply.get_ref().result
                {
                    let mut o = c.observations.lock().unwrap();
                    if let Some(d) = o.original {
                        o.issue_remaining = Some(
                            d.monotonic_stop()
                                .saturating_duration_since(Instant::now())
                                .min(
                                    (d.expires_at() - chrono::Utc::now())
                                        .to_std()
                                        .unwrap_or_default(),
                                ),
                        );
                    }
                    o.issued_token = Some(g.token.clone());
                }
            }
        }
        if let Some(p) = &phase {
            p.complete(result.is_ok());
        }
        result
    }
    async fn validate_notification_grant(
        &self,
        r: Request<authority::ValidateNotificationGrantRequest>,
    ) -> Result<Response<authority::ValidateNotificationGrantResponse>, Status> {
        let case = NotificationBinding::decode(&r.get_ref().binding)
            .ok()
            .and_then(|b| self.probe.event(b.claim().event_id));
        let phase = case.as_ref().map(|c| c.enter("refresh"));
        if let Some(c) = &case {
            let mut o = c.observations.lock().unwrap();
            o.deadline_mismatch |= o
                .original
                .is_none_or(|d| r.get_ref().deadline_micros > d.expires_at().timestamp_micros());
        }
        let result = self.inner.validate_notification_grant(r).await;
        if let Some(p) = &phase {
            p.complete(result.is_ok());
        }
        result
    }
}
