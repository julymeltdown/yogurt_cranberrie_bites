//! Transport-only operation context. Only explicitly opted-in owner clients use
//! it; this is not propagated by the general identity/correlation interceptor.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tonic::{Request, Status};
use uuid::Uuid;

/// Metadata carrying the owner operation ID; an actor delegation binds it.
pub const OPERATION_ID_HEADER: &str = "x-owner-operation-id";
const ID: &str = OPERATION_ID_HEADER;
const ACTOR: &str = "x-owner-actor-id";
const KEY: &str = "x-owner-idempotency-key";
/// Bounded machine-readable error code; the edge never forwards arbitrary
/// downstream status text for a conflict.
pub const ERROR_CODE: &str = "x-owner-operation-error";

#[derive(Clone)]
pub struct MutationContext {
    pub operation_id: Uuid,
    pub actor_id: Uuid,
    pub idempotency_key: Option<String>,
    /// Local dispatch choice, NEVER trusted from incoming metadata.
    pub lookup_only: bool,
    /// Shared with the Gateway journal task so it can distinguish a proven
    /// pre-dispatch rejection from an ambiguous owner reply.
    dispatch_may_have_occurred: Arc<AtomicBool>,
}

impl MutationContext {
    pub fn new(
        operation_id: Uuid,
        actor_id: Uuid,
        idempotency_key: Option<String>,
        lookup_only: bool,
    ) -> Self {
        Self {
            operation_id,
            actor_id,
            idempotency_key,
            lookup_only,
            dispatch_may_have_occurred: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Record that the inbound adapter crossed a boundary where the owner write
    /// could have been dispatched. This signal is conservative: `true` never
    /// proves that the owner committed.
    pub fn mark_dispatch_may_have_occurred(&self) {
        self.dispatch_may_have_occurred
            .store(true, Ordering::Release);
    }

    pub fn dispatch_may_have_occurred(&self) -> bool {
        self.dispatch_may_have_occurred.load(Ordering::Acquire)
    }
}

tokio::task_local! { static CURRENT: Option<MutationContext>; }

pub async fn scope<F: std::future::Future>(
    context: Option<MutationContext>,
    future: F,
) -> F::Output {
    CURRENT.scope(context, future).await
}

pub fn current() -> Option<MutationContext> {
    CURRENT.try_with(Clone::clone).ok().flatten()
}

#[allow(clippy::result_large_err)]
pub fn attach<T>(message: T) -> Result<Request<T>, Status> {
    let mut request = Request::new(message);
    if let Some(context) = current() {
        request.metadata_mut().insert(
            ID,
            context
                .operation_id
                .to_string()
                .parse()
                .expect("UUID metadata"),
        );
        request.metadata_mut().insert(
            ACTOR,
            context.actor_id.to_string().parse().expect("UUID metadata"),
        );
        if let Some(key) = &context.idempotency_key {
            request.metadata_mut().insert(
                KEY,
                key.parse()
                    .map_err(|_| Status::invalid_argument("Invalid operation key"))?,
            );
        }
        context.mark_dispatch_may_have_occurred();
    }
    Ok(request)
}

/// Mark a local owner mutation boundary that does not use an outbound gRPC
/// request, such as Gateway-owned moderation-report persistence.
pub fn mark_local_dispatch_may_have_occurred() {
    if let Some(context) = current() {
        context.mark_dispatch_may_have_occurred();
    }
}

/// Call only after workload authentication and per-RPC authorization. A partial
/// or duplicate context is rejected instead of silently falling back to a write
/// without deduplication. The application also binds the actor to the command.
#[allow(clippy::result_large_err)]
pub fn from_request<T>(request: &Request<T>) -> Result<Option<MutationContext>, Status> {
    let get = |name: &'static str| -> Result<Option<&str>, Status> {
        let values = request.metadata().get_all(name);
        let mut values = values.iter();
        let value = values.next();
        if values.next().is_some() {
            return Err(Status::invalid_argument("Duplicate operation metadata"));
        }
        value
            .map(|v| {
                v.to_str()
                    .map_err(|_| Status::invalid_argument("Invalid operation metadata"))
            })
            .transpose()
    };
    let (id, actor, key) = (get(ID)?, get(ACTOR)?, get(KEY)?);
    if id.is_none() && actor.is_none() && key.is_none() {
        return Ok(None);
    }
    let parse = |value: Option<&str>| {
        value
            .and_then(|v| Uuid::parse_str(v).ok())
            .filter(|v| !v.is_nil())
            .ok_or_else(|| Status::invalid_argument("Incomplete operation identity"))
    };
    if key
        .is_some_and(|v| v.is_empty() || v.len() > 128 || !v.bytes().all(|b| b.is_ascii_graphic()))
    {
        return Err(Status::invalid_argument("Invalid operation key"));
    }
    Ok(Some(MutationContext::new(
        parse(id)?,
        parse(actor)?,
        key.map(str::to_owned),
        false,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scopes_do_not_leak_and_lookup_mode_is_not_accepted_from_the_wire() {
        let context = MutationContext::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("command-1".into()),
            true,
        );
        let request = scope(Some(context.clone()), async { attach(()).unwrap() }).await;
        let decoded = from_request(&request).unwrap().unwrap();
        assert_eq!(decoded.operation_id, context.operation_id);
        assert_eq!(decoded.actor_id, context.actor_id);
        assert_eq!(decoded.idempotency_key, context.idempotency_key);
        assert!(!decoded.lookup_only);
        assert!(context.dispatch_may_have_occurred());
        assert!(current().is_none());
        assert!(from_request(&attach(()).unwrap()).unwrap().is_none());
    }

    #[test]
    fn partial_duplicate_and_invalid_contexts_fail_closed() {
        let mut request = Request::new(());
        request.metadata_mut().insert(KEY, "test".parse().unwrap());
        assert!(from_request(&request).is_err());
        request
            .metadata_mut()
            .insert(ID, Uuid::new_v4().to_string().parse().unwrap());
        request
            .metadata_mut()
            .insert(ACTOR, Uuid::new_v4().to_string().parse().unwrap());
        assert!(from_request(&request).is_ok());
        request
            .metadata_mut()
            .append(ID, Uuid::new_v4().to_string().parse().unwrap());
        assert!(from_request(&request).is_err());
    }
}
