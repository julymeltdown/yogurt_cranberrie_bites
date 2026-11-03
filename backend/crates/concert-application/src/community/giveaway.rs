//! Typed gift requests and reservation-bound keyed pickup secrets; never log these values.
use super::resource_id;
use crate::commands::{
    CanonicalRequest, CanonicalValue as V, CommandError, RegisteredOperation, Scope,
};
use chrono::{DateTime, SecondsFormat, Utc};
use concert_domain::community::giveaway::*;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

pub struct GiftCommand {
    pub actor: Uuid,
    pub event: Uuid,
    pub key: Uuid,
    pub giveaway: Uuid,
    pub action: GiftAction,
    pub code: Option<PickupCode>,
}
impl GiftCommand {
    pub fn operation(&self) -> &'static str {
        match self.action {
            GiftAction::Create(_) => "createConcertGiveaway",
            GiftAction::Update { .. } => "updateConcertGiveaway",
            GiftAction::SetMeeting { .. } => "setConcertPickupMeeting",
            GiftAction::Reserve { .. } => "reserveConcertGiveaway",
            GiftAction::Cancel { .. } => "cancelConcertReservation",
            GiftAction::IssueCode { .. } => "issueConcertPickupCode",
            GiftAction::Claim { .. } => "claimConcertReservation",
        }
    }
    pub fn safety(&self) -> bool {
        matches!(self.action, GiftAction::Cancel { .. })
    }
    pub fn created_id(&self) -> Uuid {
        resource_id(self.actor, self.event, self.key, self.operation())
    }
    pub fn giveaway_id(&self) -> Uuid {
        if matches!(self.action, GiftAction::Create(_)) {
            self.created_id()
        } else {
            self.giveaway
        }
    }
    pub fn target(&self) -> Uuid {
        match self.action {
            GiftAction::Create(_) | GiftAction::Reserve { .. } => self.created_id(),
            GiftAction::Cancel { reservation, .. }
            | GiftAction::IssueCode { reservation, .. }
            | GiftAction::Claim { reservation, .. } => reservation,
            _ => self.giveaway,
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
        let gift = self.giveaway_id().to_string();
        let target = self.target().to_string();
        let mut params = vec![("eventId", event.as_str())];
        let (body, query) = match &self.action {
            GiftAction::Create(f) => (Some(V::Object(fields(f))), vec![]),
            GiftAction::Update {
                expected,
                fields: f,
                state,
            } => {
                params.push(("giveawayId", &gift));
                let mut b = fields(f);
                b.push(("expected_revision".into(), V::Integer(*expected)));
                b.push(("state".into(), s(state_name(*state))));
                (Some(V::Object(b)), vec![])
            }
            GiftAction::SetMeeting {
                expected,
                meeting: m,
                physical_acknowledged,
            } => {
                params.push(("giveawayId", &gift));
                (
                    Some(o(vec![
                        ("expected_revision", V::Integer(*expected)),
                        (
                            "physical_presence_acknowledged",
                            V::Bool(*physical_acknowledged),
                        ),
                        (
                            "meeting",
                            o(vec![
                                ("place_id", s(m.place_id)),
                                ("map_revision", V::Integer(m.map_revision)),
                                ("place_revision", V::Integer(m.place_revision)),
                                ("instructions", s(&m.instructions)),
                                ("identification", s(&m.identification)),
                                ("expires_at", s(timestamp(m.expires_at))),
                            ]),
                        ),
                    ])),
                    vec![],
                )
            }
            GiftAction::Reserve { giveaway_revision } => {
                params.push(("giveawayId", &gift));
                (
                    Some(o(vec![(
                        "giveaway_revision",
                        V::Integer(*giveaway_revision),
                    )])),
                    vec![],
                )
            }
            GiftAction::Cancel { expected, .. } => {
                params.push(("reservationId", &target));
                (None, vec![("expected_revision", expected.to_string())])
            }
            GiftAction::IssueCode { expected, .. } => {
                params.push(("reservationId", &target));
                (
                    Some(o(vec![("expected_revision", V::Integer(*expected))])),
                    vec![],
                )
            }
            GiftAction::Claim { expected, .. } => {
                params.push(("reservationId", &target));
                let code = self.code.as_ref().ok_or(CommandError::InvalidInput)?;
                (
                    Some(o(vec![
                        ("expected_revision", V::Integer(*expected)),
                        ("confirmation_code", s(code.expose())),
                    ])),
                    vec![],
                )
            }
        };
        let query: Vec<_> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        CanonicalRequest::json(self.scope()?, &params, &query, body.as_ref())
    }
}
fn s(s: impl ToString) -> V {
    V::String(s.to_string())
}
fn o(v: Vec<(&str, V)>) -> V {
    V::Object(v.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn timestamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}
fn fields(f: &GiftFields) -> Vec<(String, V)> {
    vec![
        ("title".into(), s(&f.title)),
        ("description".into(), s(&f.description)),
        (
            "image_media_id".into(),
            f.image_media_id.map(s).unwrap_or(V::Null),
        ),
        ("quantity_total".into(), V::Integer(f.quantity_total.into())),
        ("pickup_starts_at".into(), s(timestamp(f.pickup_starts_at))),
        ("pickup_ends_at".into(), s(timestamp(f.pickup_ends_at))),
        ("public_place_id".into(), s(f.public_place_id)),
    ]
}
pub fn state_name(state: GiftState) -> &'static str {
    match state {
        GiftState::Draft => "DRAFT",
        GiftState::Open => "OPEN",
        GiftState::Paused => "PAUSED",
        GiftState::Closed => "CLOSED",
        GiftState::Cancelled => "CANCELLED",
    }
}
/// No Debug/Serialize implementation. Plaintext is a one-shot no-store response only.
pub struct PickupCode(String);
impl PickupCode {
    pub fn parse(value: String) -> Result<Self, CommandError> {
        if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self(value))
    }
    pub fn generate() -> Self {
        loop {
            let b = Uuid::new_v4().into_bytes();
            let n = u32::from_be_bytes(b[..4].try_into().expect("four bytes"));
            if n < 4_294_000_000 {
                return Self(format!("{:06}", n % 1_000_000));
            }
        }
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
/// Trusted injected key material, independent from receipt keys and never persisted/logged.
pub struct PickupKey {
    pub id: String,
    secret: Vec<u8>,
}
impl PickupKey {
    pub fn new(id: &str, secret: &[u8]) -> Result<Self, CommandError> {
        if id.is_empty()
            || id.len() > 32
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            || !(32..=4096).contains(&secret.len())
        {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self {
            id: id.into(),
            secret: secret.into(),
        })
    }
}
pub struct PickupKeys {
    active: PickupKey,
    retained: Vec<PickupKey>,
}
impl PickupKeys {
    pub fn new(active: PickupKey, retained: Vec<PickupKey>) -> Result<Self, CommandError> {
        let mut ids = std::collections::HashSet::new();
        ids.insert(active.id.clone());
        if retained.len() > 31 || retained.iter().any(|k| !ids.insert(k.id.clone())) {
            return Err(CommandError::InvalidInput);
        }
        Ok(Self { active, retained })
    }
    fn mac(
        key: &PickupKey,
        event: Uuid,
        reservation: Uuid,
        generation: i64,
        expiry: DateTime<Utc>,
        code: &PickupCode,
    ) -> Result<Hmac<Sha256>, CommandError> {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&key.secret).map_err(|_| CommandError::Unavailable)?;
        mac.update(b"mcr-pickup:v1");
        mac.update(event.as_bytes());
        mac.update(reservation.as_bytes());
        mac.update(&generation.to_be_bytes());
        mac.update(timestamp(expiry).as_bytes());
        mac.update(code.expose().as_bytes());
        Ok(mac)
    }
    pub fn sign(
        &self,
        event: Uuid,
        reservation: Uuid,
        generation: i64,
        expiry: DateTime<Utc>,
        code: &PickupCode,
    ) -> Result<(String, Vec<u8>), CommandError> {
        Ok((
            self.active.id.clone(),
            Self::mac(&self.active, event, reservation, generation, expiry, code)?
                .finalize()
                .into_bytes()
                .to_vec(),
        ))
    }
    pub fn verify(
        &self,
        key_id: &str,
        hash: &[u8],
        event: Uuid,
        reservation: Uuid,
        generation: i64,
        expiry: DateTime<Utc>,
        code: &PickupCode,
    ) -> Result<bool, CommandError> {
        let key = std::iter::once(&self.active)
            .chain(self.retained.iter())
            .find(|k| k.id == key_id)
            .ok_or(CommandError::Unavailable)?;
        Ok(
            Self::mac(key, event, reservation, generation, expiry, code)?
                .verify_slice(hash)
                .is_ok(),
        )
    }
}
