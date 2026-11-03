//! Bounded internal party projections. Public adapters still resolve canonical public profiles.
use super::*;
use chrono::{DateTime, Utc};
#[derive(Clone, Default)]
pub struct PartyFilter {
    pub ticket_section: Option<String>,
    pub state: Option<PartyState>,
    pub has_space: Option<bool>,
    pub starts_after: Option<DateTime<Utc>>,
    pub starts_before: Option<DateTime<Utc>>,
    pub after: Option<(DateTime<Utc>, Uuid)>,
}
pub struct PartyPage {
    pub items: Vec<PublicParty>,
    pub next: Option<(DateTime<Utc>, Uuid)>,
}
impl<A: CommunityAuthority> PgCommunity<A> {
    pub async fn list_parties(
        &self,
        actor: Uuid,
        event: Uuid,
        filter: PartyFilter,
        limit: i64,
    ) -> Result<PartyPage, CommunityError> {
        if !(1..=100).contains(&limit)
            || filter
                .starts_after
                .zip(filter.starts_before)
                .is_some_and(|(a, b)| a >= b)
            || filter
                .ticket_section
                .as_ref()
                .is_some_and(|s| !text(s, 1, 120))
        {
            return Err(CommandError::InvalidInput.into());
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.authority
            .actor_in(&mut tx, actor, event, CommunityAccess::ReadParties)
            .await?;
        let rows=sqlx::query("select party_id,aggregate->>'created_at' as created_at from concert_community_parties where event_id=$1 and ($2::text is null or aggregate->'fields'->>'ticket_section'=$2) and ($3::text is null or state=$3) and ($4::boolean is null or (member_count<capacity)=$4) and ($5::timestamptz is null or (aggregate->'fields'->>'meeting_starts_at')::timestamptz >=$5) and ($6::timestamptz is null or (aggregate->'fields'->>'meeting_starts_at')::timestamptz<$6) and ($7::timestamptz is null or ((aggregate->>'created_at')::timestamptz,party_id)>($7,$8)) order by (aggregate->>'created_at')::timestamptz,party_id limit $9")
   .bind(event).bind(&filter.ticket_section).bind(filter.state.map(concert_application::community::state_name)).bind(filter.has_space).bind(filter.starts_after).bind(filter.starts_before).bind(filter.after.map(|x|x.0)).bind(filter.after.map(|x|x.1)).bind(limit).fetch_all(&mut *tx).await.map_err(db)?;
        tx.commit().await.map_err(db)?;
        let mut items = vec![];
        let mut next = None;
        for r in rows {
            let mut tx = self.pool.begin().await.map_err(db)?;
            let id: Uuid = r.try_get("party_id").map_err(db)?;
            let created: String = r.try_get("created_at").map_err(db)?;
            let at: DateTime<Utc> = created.parse().map_err(|_| CommandError::InvalidState)?;
            next = Some((at, id));
            let p = load(&mut tx, event, id, false)
                .await?
                .ok_or(CommandError::InvalidState)?;
            match self
                .authority
                .authorize_in(&mut tx, actor, event, p.owner, CommunityAccess::ReadParties)
                .await
            {
                Ok(()) => {}
                Err(CommandError::Denied) => continue,
                Err(e) => return Err(e.into()),
            }
            items.push(public_party(&p));
            tx.commit().await.map_err(db)?;
        }
        Ok(PartyPage { items, next })
    }
    pub async fn applications(
        &self,
        actor: Uuid,
        event: Uuid,
        party: Uuid,
    ) -> Result<Vec<Application>, CommunityError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let p = load(&mut tx, event, party, false)
            .await?
            .ok_or(CommandError::Denied)?;
        let candidates: Vec<_> = p
            .applications
            .iter()
            .filter(|a| p.owner == actor || a.user == actor)
            .take(100)
            .collect();
        let permitted = permitted(
            &self.authority,
            &mut tx,
            actor,
            event,
            p.owner,
            candidates.iter().map(|a| a.user).collect(),
        )
        .await?;
        let result = candidates
            .into_iter()
            .filter(|a| permitted.contains(&a.user))
            .cloned()
            .collect();
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
    pub async fn members(
        &self,
        actor: Uuid,
        event: Uuid,
        party: Uuid,
    ) -> Result<Vec<Member>, CommunityError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let p = load(&mut tx, event, party, false)
            .await?
            .ok_or(CommandError::Denied)?;
        if !p.member_access(actor) {
            return Err(CommandError::Denied.into());
        }
        let permitted = permitted(
            &self.authority,
            &mut tx,
            actor,
            event,
            p.owner,
            p.members.iter().map(|m| m.user).collect(),
        )
        .await?;
        let result = p
            .members
            .iter()
            .filter(|m| permitted.contains(&m.user))
            .cloned()
            .collect();
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
}
fn public_party(p: &Party) -> PublicParty {
    PublicParty {
        id: p.id,
        event: p.event,
        owner: p.owner,
        fields: p.fields.clone(),
        state: p.state,
        revision: p.revision,
        meeting_revision: p.meeting_revision,
        member_count: p.member_count(),
        created_at: p.created_at,
    }
}

async fn permitted<A: CommunityAuthority>(
    authority: &A,
    c: &mut PgConnection,
    actor: Uuid,
    event: Uuid,
    owner: Uuid,
    mut subjects: Vec<Uuid>,
) -> Result<std::collections::HashSet<Uuid>, CommandError> {
    subjects.push(owner);
    subjects.sort();
    subjects.dedup();
    let mut permitted = std::collections::HashSet::new();
    for subject in subjects {
        match authority
            .authorize_in(c, actor, event, subject, CommunityAccess::ReadParties)
            .await
        {
            Ok(()) => {
                permitted.insert(subject);
            }
            Err(CommandError::Denied) if subject != owner => {}
            Err(e) => return Err(e),
        }
    }
    Ok(permitted)
}
