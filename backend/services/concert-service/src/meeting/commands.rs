//! Receipt-backed commands. Canonical fingerprints are built from the exact effect input.
use super::{beacon, grant, MeetingAuthority};
use crate::adapters::out::repository::receipt::{CommandProducer, PgCommandRepository};
use async_trait::async_trait;
use concert_application::{beacon::*, commands::*};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub struct BeaconCommands<A> {
    pool: PgPool,
    authority: A,
}
impl<A: MeetingAuthority> BeaconCommands<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }
    pub async fn execute(
        &self,
        actor: MeetingActor,
        id: Uuid,
        key: Uuid,
        mutation: BeaconMutation,
        keys: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        if !mutation.valid() || id.is_nil() {
            return Err(CommandError::InvalidInput);
        }
        let scope = Scope {
            actor: actor.actor,
            event: actor.event,
            key,
            operation: RegisteredOperation::find(mutation.operation_id())?,
        };
        let event = actor.event.to_string();
        let target = id.to_string();
        let mut parameters = vec![("eventId", event.as_str())];
        let (body, query) = match &mutation {
            BeaconMutation::Create(value) => (
                Some(serde_json::to_value(value).map_err(|_| CommandError::InvalidInput)?),
                vec![],
            ),
            BeaconMutation::Update(value) => {
                parameters.push(("beaconId", target.as_str()));
                (
                    Some(serde_json::to_value(value).map_err(|_| CommandError::InvalidInput)?),
                    vec![],
                )
            }
            BeaconMutation::Revoke { expected_revision } => {
                parameters.push(("beaconId", target.as_str()));
                (
                    None,
                    vec![(
                        "expected_revision".to_owned(),
                        expected_revision.to_string(),
                    )],
                )
            }
        };
        let body = body.as_ref().map(canonical).transpose()?;
        let query = query
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect::<Vec<_>>();
        let request = CanonicalRequest::json(scope, &parameters, &query, body.as_ref())?;
        let producer = BeaconProducer {
            authority: &self.authority,
            actor,
            id,
            mutation: &mutation,
        };
        PgCommandRepository::new(self.pool.clone())
            .execute(&request, id, keys, &producer)
            .await
    }
}
pub(crate) fn canonical(value: &Value) -> Result<CanonicalValue, CommandError> {
    Ok(match value {
        Value::Null => CanonicalValue::Null,
        Value::Bool(v) => CanonicalValue::Bool(*v),
        Value::Number(v) => CanonicalValue::Integer(v.as_i64().ok_or(CommandError::InvalidInput)?),
        Value::String(v) => CanonicalValue::String(v.clone()),
        Value::Array(v) => {
            CanonicalValue::Array(v.iter().map(canonical).collect::<Result<_, _>>()?)
        }
        Value::Object(v) => CanonicalValue::Object(
            v.iter()
                .map(|(k, v)| Ok((k.clone(), canonical(v)?)))
                .collect::<Result<_, CommandError>>()?,
        ),
    })
}
struct BeaconProducer<'a, A> {
    authority: &'a A,
    actor: MeetingActor,
    id: Uuid,
    mutation: &'a BeaconMutation,
}
#[async_trait]
impl<A: MeetingAuthority> CommandProducer for BeaconProducer<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if scope.actor != self.actor.actor
            || scope.event != self.actor.event
            || target != self.id
            || scope.operation.id() != self.mutation.operation_id()
        {
            return Err(CommandError::Denied);
        }
        self.authority
            .actor_in(c, self.actor)
            .await
            .map_err(command_error)?;
        if !matches!(self.mutation, BeaconMutation::Create(_)) {
            let value = beacon::load(c, self.actor.event, target, true)
                .await
                .map_err(command_error)?;
            if value.owner_id != self.actor.actor {
                return Err(CommandError::Denied);
            }
            let expected_party = beacon::stored_fence(c, self.actor.event, target)
                .await
                .map_err(command_error)?;
            grant(
                self.authority,
                c,
                AudienceQuery {
                    actor: self.actor,
                    owner: value.owner_id,
                    audience: value.meeting.audience,
                    expected_party: if matches!(self.mutation, BeaconMutation::Revoke { .. }) {
                        None
                    } else {
                        expected_party
                    },
                    operation: if matches!(self.mutation, BeaconMutation::Revoke { .. }) {
                        MeetingOperation::Revoke
                    } else {
                        MeetingOperation::Read
                    },
                },
            )
            .await
            .map_err(command_error)?;
        }
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        _: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        match beacon::mutate_in(c, self.authority, self.actor, target, self.mutation).await {
            Ok(value) => {
                let reference = SafeReference {
                    resource: value.beacon_id,
                    revision: value.revision,
                };
                Ok((
                    Completion::Succeeded {
                        reference,
                        status: if matches!(self.mutation, BeaconMutation::Create(_)) {
                            201
                        } else {
                            200
                        },
                    },
                    Some(Invalidation { reference }),
                ))
            }
            Err(error) => safe_failure(error),
        }
    }
}
pub(crate) fn command_error(error: MeetingError) -> CommandError {
    match error {
        MeetingError::ResourceUnavailable => CommandError::Denied,
        MeetingError::RevisionConflict => CommandError::Conflict,
        MeetingError::InvalidInput => CommandError::InvalidInput,
        MeetingError::InvalidState => CommandError::InvalidState,
        _ => CommandError::Unavailable,
    }
}
pub(crate) fn safe_failure(
    error: MeetingError,
) -> Result<(Completion, Option<Invalidation>), CommandError> {
    let reason = match error {
        MeetingError::RevisionConflict => SafeFailure::VersionConflict,
        MeetingError::PolicyUnavailable => SafeFailure::DependencyUnavailable,
        // Specific domain failure variants are reconciled with delivery-owned 0009.
        MeetingError::ResourceUnavailable => return Err(CommandError::Denied),
        MeetingError::InvalidInput => return Err(CommandError::InvalidInput),
        MeetingError::MapMismatch | MeetingError::EventReadOnly => SafeFailure::InvalidTransition,
        MeetingError::StorageUnavailable => return Err(CommandError::Unavailable),
        MeetingError::InvalidState => return Err(CommandError::InvalidState),
    };
    Ok((Completion::Failed(reason), None))
}
