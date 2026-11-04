//! Native trace from qualified READ_ONLY fixture48335f67.
use super::*;
#[derive(Clone)]
pub(crate) struct Trace {
    pub(crate) native: SocketTransport,
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) waits: Arc<AtomicUsize>,
    pub(crate) sent: Arc<Mutex<Vec<(Binding, Value, Instant)>>>,
    pub(crate) hold_user: Arc<Mutex<Option<Uuid>>>,
    pub(crate) held: Arc<AtomicBool>,
    pub(crate) release: Arc<AtomicBool>,
}
impl TransportPort for Trace {
    fn installed(&mut self, b: &Binding, n: u64) -> Result<(), WriteError> {
        let result = self.native.installed(b, n);
        eprintln!("native transport installation success={}", result.is_ok());
        result
    }
    fn ready(&self, b: &Binding) -> bool {
        if self.paused.load(Ordering::Acquire) {
            self.waits.fetch_add(1, Ordering::Relaxed);
            false
        } else {
            self.native.ready(b)
        }
    }
    fn send(
        &mut self,
        b: &Binding,
        d: crate::world::replication::Delivery,
    ) -> Result<(), WriteError> {
        let body = serde_json::from_slice(&d.bytes).unwrap();
        let hold = {
            let mut user = self.hold_user.lock().unwrap();
            if *user == Some(b.key.user_id.uuid()) {
                user.take();
                true
            } else {
                false
            }
        };
        if hold {
            self.held.store(true, Ordering::Release);
            let began = Instant::now();
            while !self.release.load(Ordering::Acquire) {
                assert!(
                    began.elapsed() < Duration::from_secs(1),
                    "native handoff hold not released"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        self.native.send(b, d)?;
        let mut sent = self.sent.lock().unwrap();
        assert!(sent.len() < 1000);
        sent.push((b.clone(), body, Instant::now()));
        Ok(())
    }
    fn close(&mut self, b: &Binding, r: SafeClose) {
        eprintln!("native transport close {} {}", r.code, r.label);
        self.native.close(b, r)
    }
}
