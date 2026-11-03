//! Framework-free BE41 commands. HTTP amendment is integrator-owned.
use crate::commands::{
    CanonicalRequest, CanonicalValue as V, CommandError, RegisteredOperation, Scope,
};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Actor {
    pub actor: Uuid,
    pub event: Uuid,
}
#[derive(Clone, Debug)]
pub enum CloseCommand {
    Start {
        expected_revision: i64,
        reason: String,
    },
    Resume {
        job_id: Uuid,
        expected_revision: i64,
        reason: String,
    },
}
impl CloseCommand {
    pub fn request(&self, actor: Actor, key: Uuid) -> Result<CanonicalRequest, CommandError> {
        let (revision, reason, confirmation) = match self {
            Self::Start {
                expected_revision,
                reason,
            } => (*expected_revision, reason, "CLOSE_EVENT"),
            Self::Resume {
                job_id,
                expected_revision,
                reason,
            } if !job_id.is_nil() => (*expected_revision, reason, "RESUME_CLOSURE"),
            _ => return Err(CommandError::InvalidInput),
        };
        if [actor.actor, actor.event, key].iter().any(Uuid::is_nil)
            || !(1..=9_007_199_254_740_991).contains(&revision)
            || reason.trim().is_empty()
            || reason.chars().count() > 500
            || reason.chars().any(char::is_control)
        {
            return Err(CommandError::InvalidInput);
        }
        let mut body = vec![
            ("expected_revision".into(), V::Integer(revision)),
            ("confirmation".into(), V::String(confirmation.into())),
            ("reason".into(), V::String(reason.clone())),
        ];
        if let Self::Resume { job_id, .. } = self {
            body.push(("job_id".into(), V::String(job_id.to_string())))
        }
        CanonicalRequest::json(
            Scope {
                actor: actor.actor,
                event: actor.event,
                key,
                operation: RegisteredOperation::find("closeConcertEvent")?,
            },
            &[("eventId", &actor.event.to_string())],
            &[],
            Some(&V::Object(body)),
        )
    }
}
