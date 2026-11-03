//! Resource authority only. Callers additionally require current canonical/BE37 policy.
use concert_application::commands::CommandError;
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartyAudience {
    pub event: Uuid,
    pub party: Uuid,
    pub actor: Uuid,
    pub owner: Uuid,
    pub party_revision: i64,
    pub meeting_revision: i64,
    pub audience_revision: i64,
}
/// Retains the party SHARE lock through the caller's transaction. Every BE09/BE19
/// private read/command must call this and the shared BE37 policy in that transaction.
/// Removal, terminal state or changed audience denies immediately, even with pending outbox.
/// This proves virtual community membership only; never physical presence or arrival.
pub async fn party_audience_in(
    c: &mut PgConnection,
    event: Uuid,
    party: Uuid,
    actor: Uuid,
    expected_audience: Option<i64>,
) -> Result<PartyAudience, CommandError> {
    let value = super::load(c, event, party, false)
        .await?
        .ok_or(CommandError::Denied)?;
    if !value.member_access(actor)
        || expected_audience.is_some_and(|v| v != value.audience_revision)
    {
        return Err(CommandError::Denied);
    }
    Ok(PartyAudience {
        event,
        party,
        actor,
        owner: value.owner,
        party_revision: value.revision,
        meeting_revision: value.meeting_revision,
        audience_revision: value.audience_revision,
    })
}
