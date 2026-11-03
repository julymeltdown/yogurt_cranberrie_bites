//! Typed commands build their own BE15 canonical request; callers cannot substitute a fingerprint/body.
use crate::commands::{
    CanonicalRequest, CanonicalValue as V, CommandError, RegisteredOperation, Scope,
};
use concert_domain::community::party::{PartyAction, PartyFields, PartyState};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct PartyCommand {
    pub actor: Uuid,
    pub event: Uuid,
    pub key: Uuid,
    pub party: Uuid,
    pub action: PartyAction,
}
impl PartyCommand {
    pub fn operation(&self) -> &'static str {
        match self.action {
            PartyAction::Create(_) => "createConcertParty",
            PartyAction::Update { .. } => "updateConcertParty",
            PartyAction::Cancel { .. } => "cancelConcertParty",
            PartyAction::Transfer { .. } => "transferConcertParty",
            PartyAction::Apply { .. } => "applyConcertParty",
            PartyAction::Decide { .. } => "decideConcertApplication",
            PartyAction::CancelApplication { .. } => "cancelConcertApplication",
            PartyAction::Remove { .. } => "removeConcertMember",
        }
    }
    pub fn safety(&self) -> bool {
        matches!(
            self.action,
            PartyAction::Cancel { .. }
                | PartyAction::CancelApplication { .. }
                | PartyAction::Remove { .. }
        )
    }
    /// A stable server derivation makes lost create/apply responses recoverable after restart.
    /// Domain separation covers exact canonical actor/event/operation/key; no user supplied resource ID.
    pub fn created_id(&self) -> Uuid {
        resource_id(self.actor, self.event, self.key, self.operation())
    }
    pub fn target(&self) -> Uuid {
        match self.action {
            PartyAction::Create(_) | PartyAction::Apply { .. } => self.created_id(),
            PartyAction::Decide { application, .. }
            | PartyAction::CancelApplication { application, .. } => application,
            PartyAction::Remove { user, .. } => user,
            _ => self.party,
        }
    }
    pub fn party_id(&self) -> Uuid {
        if matches!(self.action, PartyAction::Create(_)) {
            self.created_id()
        } else {
            self.party
        }
    }
    pub fn scope(&self) -> Result<Scope, CommandError> {
        Ok(Scope {
            actor: self.actor,
            event: self.event,
            key: self.key,
            operation: RegisteredOperation::find(self.operation())?,
        })
    }
    pub fn request(&self) -> Result<CanonicalRequest, CommandError> {
        let event = self.event.to_string();
        let party = self.party_id().to_string();
        let target = self.target().to_string();
        let mut parameters = vec![("eventId", event.as_str())];
        let (body, query) = match &self.action {
            PartyAction::Create(f) => (Some(V::Object(fields(f))), vec![]),
            PartyAction::Update {
                expected,
                fields: f,
                state,
            } => {
                parameters.push(("partyId", &party));
                let mut b = fields(f);
                b.push(("expected_revision".into(), V::Integer(*expected)));
                b.push(("state".into(), string(state_name(*state))));
                (Some(V::Object(b)), vec![])
            }
            PartyAction::Cancel { expected } => {
                parameters.push(("partyId", &party));
                (
                    Some(object(vec![("expected_revision", V::Integer(*expected))])),
                    vec![],
                )
            }
            PartyAction::Transfer { expected, user } => {
                parameters.push(("partyId", &party));
                (
                    Some(object(vec![
                        ("expected_revision", V::Integer(*expected)),
                        ("new_owner_id", string(user.to_string())),
                    ])),
                    vec![],
                )
            }
            PartyAction::Apply {
                party_revision,
                note,
            } => {
                parameters.push(("partyId", &party));
                (
                    Some(object(vec![
                        ("party_revision", V::Integer(*party_revision)),
                        ("note", string(note)),
                    ])),
                    vec![],
                )
            }
            PartyAction::Decide {
                expected,
                party_revision,
                approve,
                ..
            } => {
                parameters.push(("applicationId", &target));
                (
                    Some(object(vec![
                        ("expected_revision", V::Integer(*expected)),
                        ("party_revision", V::Integer(*party_revision)),
                        (
                            "decision",
                            string(if *approve { "APPROVED" } else { "REJECTED" }),
                        ),
                    ])),
                    vec![],
                )
            }
            PartyAction::CancelApplication { expected, .. } => {
                parameters.push(("applicationId", &target));
                (None, vec![("expected_revision", expected.to_string())])
            }
            PartyAction::Remove {
                expected,
                party_revision,
                ..
            } => {
                parameters.push(("partyId", &party));
                parameters.push(("userId", &target));
                (
                    None,
                    vec![
                        ("expected_revision", expected.to_string()),
                        ("party_revision", party_revision.to_string()),
                    ],
                )
            }
        };
        let query: Vec<_> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        CanonicalRequest::json(self.scope()?, &parameters, &query, body.as_ref())
    }
}
pub fn resource_id(actor: Uuid, event: Uuid, key: Uuid, operation: &str) -> Uuid {
    let mut hash = Sha256::new();
    hash.update(b"mcr-community-resource:v1");
    hash.update(actor.as_bytes());
    hash.update(event.as_bytes());
    hash.update(key.as_bytes());
    hash.update(operation.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
fn string(s: impl ToString) -> V {
    V::String(s.to_string())
}
fn object(v: Vec<(&str, V)>) -> V {
    V::Object(v.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn fields(f: &PartyFields) -> Vec<(String, V)> {
    vec![
        ("title".into(), string(&f.title)),
        ("description".into(), string(&f.description)),
        (
            "ticket_section".into(),
            f.ticket_section.as_ref().map(string).unwrap_or(V::Null),
        ),
        ("capacity".into(), V::Integer(f.capacity.into())),
        (
            "meeting_starts_at".into(),
            string(
                f.meeting_starts_at
                    .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
            ),
        ),
        (
            "meeting_ends_at".into(),
            string(
                f.meeting_ends_at
                    .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
            ),
        ),
        ("requirements".into(), string(&f.requirements)),
        ("public_place_id".into(), string(f.public_place_id)),
    ]
}
pub fn state_name(s: PartyState) -> &'static str {
    match s {
        PartyState::Open => "OPEN",
        PartyState::Closed => "CLOSED",
        PartyState::Cancelled => "CANCELLED",
        PartyState::Completed => "COMPLETED",
    }
}
