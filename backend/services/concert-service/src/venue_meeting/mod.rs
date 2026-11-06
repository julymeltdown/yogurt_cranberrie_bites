//! VENU-BE-M01 PostgreSQL adapter. Authority is evaluated in the effect transaction.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::venue_meeting::{
    decision_transition, require_invitation_landmark, require_policy, require_selection_friend,
    safe_ceiling, CreateInvitation, DecideInvitation, LandmarkReview, PairFacts, SendMessage,
    VenueMeetingAuthority,
};
use concert_domain::venue_meeting::{
    InvitationState, LandmarkSelection, MeetingError, MeetingPolicy, MeetingSelection,
    StoredInvitation, StoredMessage, PAGE_SIZE,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

pub struct PgVenueMeeting<A> {
    pool: PgPool,
    authority: A,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
}

#[async_trait]
impl<A: VenueMeetingAuthority> VenueMeetingAuthority for PgVenueMeeting<A> {
    async fn friend(&self, event: Uuid, actor: Uuid, peer: Uuid) -> Result<PairFacts, MeetingError> {
        self.authority.friend(event, actor, peer).await
    }
    async fn reviewed_landmark(
        &self,
        event: Uuid,
        selection: &LandmarkSelection,
    ) -> Result<LandmarkReview, MeetingError> {
        self.authority.reviewed_landmark(event, selection).await
    }
    async fn policy(&self, event: Uuid) -> Result<MeetingPolicy, MeetingError> {
        self.authority.policy(event).await
    }
}

impl<A: VenueMeetingAuthority> PgVenueMeeting<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }

    pub fn authority(&self) -> &A {
        &self.authority
    }

    pub async fn context(
        &self,
        event: Uuid,
        actor: Uuid,
        selection: &MeetingSelection,
    ) -> Result<Value, MeetingError> {
        let mut c = self.pool.acquire().await.map_err(storage)?;
        let now = database_now(&mut c).await?;
        let policy = require_policy(&self.authority, event).await?;
        let pair = require_selection_friend(&self.authority, event, actor, selection).await?;
        let review = match &selection.landmark {
            Some(_) => Some(
                require_invitation_landmark(&self.authority, event, selection, now).await?,
            ),
            None => None,
        };
        let valid_until = safe_ceiling(&policy, review.as_ref(), now)?;
        let context_id = selection.context_id(actor, event);
        let conversation: Option<Uuid> = sqlx::query_scalar(
            "select id from concert_venue_meeting_conversations where event_id=$1 and context_id=$2",
        )
        .bind(event)
        .bind(&context_id)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?;
        let (content_expires, purge) = policy.content_deadline(now)?;
        let invitation_expires = policy.invitation_deadline(now).ok().map(|(value, _)| value);
        Ok(json!({
            "contextId": context_id,
            "eventId": event,
            "actorId": actor,
            "selection": selection,
            "conversationId": conversation,
            "canRead": true,
            "canSend": true,
            "canInvite": review.is_some(),
            "relationshipRevision": pair.relationship_revision,
            "policyRevision": policy.policy_revision,
            "serverTime": now,
            "validUntil": valid_until,
            "eventTimezone": policy.event_timezone,
            "invitationExpiresAt": invitation_expires,
            "contentExpiresAt": content_expires,
            "purgeAt": purge,
        }))
    }

    pub async fn invitation(
        &self,
        event: Uuid,
        actor: Uuid,
        id: Uuid,
    ) -> Result<Value, MeetingError> {
        let mut c = self.pool.acquire().await.map_err(storage)?;
        let now = database_now(&mut c).await?;
        require_policy(&self.authority, event).await?;
        let invitation = load_invitation(&mut c, event, id).await?;
        let selection = other_selection(&invitation, actor)?;
        require_selection_friend(&self.authority, event, actor, &selection).await?;
        require_invitation_landmark(&self.authority, event, &selection, now).await?;
        invitation_json(&invitation, actor, now)
    }

    pub async fn inbox(
        &self,
        event: Uuid,
        actor: Uuid,
        direction: Direction,
        cursor: i64,
    ) -> Result<Value, MeetingError> {
        if cursor < 0 || cursor > concert_domain::MAX_REVISION {
            return Err(MeetingError::InvalidInput);
        }
        let mut c = self.pool.acquire().await.map_err(storage)?;
        let now = database_now(&mut c).await?;
        require_policy(&self.authority, event).await?;
        let column = match direction {
            Direction::Incoming => "recipient_id",
            Direction::Outgoing => "sender_id",
        };
        let rows = sqlx::query(&format!(
            "select * from concert_venue_meeting_invitations
             where event_id=$1 and {column}=$2 and ordinal>$3 order by ordinal limit 51"
        ))
        .bind(event)
        .bind(actor)
        .bind(cursor)
        .fetch_all(&mut *c)
        .await
        .map_err(storage)?;
        let mut items = Vec::new();
        let mut last_ordinal = None;
        for row in rows.into_iter().take(PAGE_SIZE as usize) {
            let invitation = invitation_from_row(&row)?;
            last_ordinal = Some(invitation.ordinal);
            let selection = other_selection(&invitation, actor)?;
            if require_selection_friend(&self.authority, event, actor, &selection)
                .await
                .is_err()
            {
                continue;
            }
            if selection.landmark.is_some()
                && require_invitation_landmark(&self.authority, event, &selection, now)
                    .await
                    .is_err()
            {
                continue;
            }
            items.push(json!({
                "selection": selection,
                "invitation": invitation_json(&invitation, actor, now)?,
            }));
        }
        Ok(json!({
            "items": items,
            "nextCursor": last_ordinal.map(|value| value.to_string()),
            "serverTime": now,
        }))
    }

    pub async fn create_invitation(
        &self,
        event: Uuid,
        actor: Uuid,
        key: Uuid,
        command: &CreateInvitation,
    ) -> Result<Value, MeetingError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let now = database_now(&mut tx).await?;
        let digest = digest("createVenueMeetingInvitation", event, None, &command.selection)?;
        if let Some(replayed) = replay(&mut tx, event, actor, key, &digest).await? {
            require_policy(&self.authority, event).await?;
            require_selection_friend(&self.authority, event, actor, &command.selection).await?;
            require_invitation_landmark(&self.authority, event, &command.selection, now).await?;
            tx.commit().await.map_err(storage)?;
            return Ok(replayed);
        }
        let policy = require_policy(&self.authority, event).await?;
        require_selection_friend(&self.authority, event, actor, &command.selection).await?;
        let review =
            require_invitation_landmark(&self.authority, event, &command.selection, now).await?;
        let ceiling = safe_ceiling(&policy, Some(&review), now)?;
        set_ceiling(&mut tx, ceiling).await?;
        let (expires, purge) = policy.invitation_deadline(now)?;
        let (low, high) = command.selection.pair(actor);
        let context_id = command.selection.context_id(actor, event);
        ensure_context(
            &mut tx,
            event,
            &context_id,
            low,
            high,
            command.selection.landmark.as_ref(),
        )
        .await?;
        let id = Uuid::new_v4();
        let selection = serde_json::to_value(&command.selection).map_err(|_| storage_err())?;
        let row = sqlx::query(
            "insert into concert_venue_meeting_invitations
             (id,event_id,context_id,sender_id,recipient_id,selection,state,revision,
              created_at,expires_at,purge_at,policy_revision)
             values($1,$2,$3,$4,$5,$6::jsonb,'PENDING',1,$7,$8,$9,$10) returning *",
        )
        .bind(id)
        .bind(event)
        .bind(&context_id)
        .bind(actor)
        .bind(command.selection.selected_friend_id)
        .bind(selection)
        .bind(now)
        .bind(expires)
        .bind(purge)
        .bind(policy.policy_revision)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let invitation = invitation_from_row(&row)?;
        let result = json!({
            "kind": "INVITATION",
            "invitation": invitation_json(&invitation, actor, now)?,
        });
        record(
            &mut tx,
            event,
            actor,
            key,
            "createVenueMeetingInvitation",
            &digest,
            &context_id,
            &result,
            now,
            &policy,
        )
        .await?;
        tx.commit().await.map_err(storage)?;
        Ok(applied(key, "createVenueMeetingInvitation", result, false))
    }

    pub async fn decide(
        &self,
        event: Uuid,
        actor: Uuid,
        id: Uuid,
        key: Uuid,
        command: &DecideInvitation,
    ) -> Result<Value, MeetingError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let now = database_now(&mut tx).await?;
        let digest = digest("decideVenueMeetingInvitation", event, Some(id), &command.selection)?;
        if let Some(replayed) = replay(&mut tx, event, actor, key, &digest).await? {
            require_policy(&self.authority, event).await?;
            let invitation = load_invitation(&mut tx, event, id).await?;
            let selection = other_selection(&invitation, actor)?;
            require_selection_friend(&self.authority, event, actor, &selection).await?;
            require_invitation_landmark(&self.authority, event, &selection, now).await?;
            tx.commit().await.map_err(storage)?;
            return Ok(replayed);
        }
        let policy = require_policy(&self.authority, event).await?;
        let locked = load_invitation_for_update(&mut tx, event, id).await?;
        let selection = other_selection(&locked, actor)?;
        if locked.selection.landmark != command.selection.landmark
            || selection.selected_friend_id != command.selection.selected_friend_id
        {
            return Err(MeetingError::ResourceUnavailable);
        }
        require_selection_friend(&self.authority, event, actor, &selection).await?;
        let review = require_invitation_landmark(&self.authority, event, &selection, now).await?;
        let ceiling = safe_ceiling(&policy, Some(&review), now)?;
        set_ceiling(&mut tx, ceiling).await?;
        if locked.revision != command.expected_revision {
            return Err(MeetingError::RevisionConflict);
        }
        let state = decision_transition(&locked, actor, command.decision, now)?;
        let row = sqlx::query(
            "update concert_venue_meeting_invitations
             set state=$3, revision=revision+1
             where event_id=$1 and id=$2 and revision=$4 returning *",
        )
        .bind(event)
        .bind(id)
        .bind(state.as_str())
        .bind(command.expected_revision)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .ok_or(MeetingError::RevisionConflict)?;
        let invitation = invitation_from_row(&row)?;
        let result = json!({
            "kind": "INVITATION",
            "invitation": invitation_json(&invitation, actor, now)?,
        });
        record(
            &mut tx,
            event,
            actor,
            key,
            "decideVenueMeetingInvitation",
            &digest,
            &locked.context_id,
            &result,
            now,
            &policy,
        )
        .await?;
        tx.commit().await.map_err(storage)?;
        Ok(applied(key, "decideVenueMeetingInvitation", result, false))
    }

    pub async fn messages(
        &self,
        event: Uuid,
        actor: Uuid,
        selection: &MeetingSelection,
        cursor: i64,
    ) -> Result<Value, MeetingError> {
        if !(0..=concert_domain::MAX_REVISION).contains(&cursor) {
            return Err(MeetingError::InvalidInput);
        }
        let mut c = self.pool.acquire().await.map_err(storage)?;
        let now = database_now(&mut c).await?;
        require_policy(&self.authority, event).await?;
        require_selection_friend(&self.authority, event, actor, selection).await?;
        if selection.landmark.is_some() {
            require_invitation_landmark(&self.authority, event, selection, now).await?;
        }
        let context_id = selection.context_id(actor, event);
        let conversation: Option<Uuid> = sqlx::query_scalar(
            "select id from concert_venue_meeting_conversations where event_id=$1 and context_id=$2",
        )
        .bind(event)
        .bind(&context_id)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?;
        let Some(conversation_id) = conversation else {
            return Ok(json!({
                "contextId": context_id,
                "items": [],
                "cursor": 0,
                "hasMore": false,
                "gap": false,
                "serverTime": now,
                "conversationId": null,
            }));
        };
        let rows = sqlx::query(
            "select * from concert_venue_meeting_messages
             where event_id=$1 and conversation_id=$2 and sequence>$3 and expires_at>$4
             order by sequence limit 51",
        )
        .bind(event)
        .bind(conversation_id)
        .bind(cursor)
        .bind(now)
        .fetch_all(&mut *c)
        .await
        .map_err(storage)?;
        let has_more = rows.len() > PAGE_SIZE as usize;
        let mut items = Vec::new();
        let mut next = cursor;
        let mut gap = false;
        for row in rows.into_iter().take(PAGE_SIZE as usize) {
            let message = message_from_row(&row)?;
            if next == cursor && gap_probe(&mut c, event, conversation_id, next, message.sequence)
                .await?
            {
                gap = true;
            }
            next = message.sequence;
            if message.expires_at > now {
                items.push(message_json(&message, actor)?);
            }
        }
        Ok(json!({
            "contextId": context_id,
            "items": items,
            "cursor": next,
            "hasMore": has_more,
            "gap": gap,
            "serverTime": now,
            "conversationId": conversation_id,
        }))
    }

    pub async fn send(
        &self,
        event: Uuid,
        actor: Uuid,
        key: Uuid,
        command: &SendMessage,
    ) -> Result<Value, MeetingError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let now = database_now(&mut tx).await?;
        let digest = digest("sendVenueMeetingMessage", event, None, &command.selection)?;
        if let Some(replayed) = replay(&mut tx, event, actor, key, &digest).await? {
            require_policy(&self.authority, event).await?;
            require_selection_friend(&self.authority, event, actor, &command.selection).await?;
            if command.selection.landmark.is_some() {
                require_invitation_landmark(&self.authority, event, &command.selection, now).await?;
            }
            tx.commit().await.map_err(storage)?;
            return Ok(replayed);
        }
        let policy = require_policy(&self.authority, event).await?;
        require_selection_friend(&self.authority, event, actor, &command.selection).await?;
        let review = match &command.selection.landmark {
            Some(_) => Some(
                require_invitation_landmark(&self.authority, event, &command.selection, now)
                    .await?,
            ),
            None => None,
        };
        let ceiling = safe_ceiling(&policy, review.as_ref(), now)?;
        set_ceiling(&mut tx, ceiling).await?;
        if let Some(invitation_id) = command.invitation_id {
            let invitation = load_invitation(&mut tx, event, invitation_id).await?;
            let pair = command.selection.pair(actor);
            let mut members = [invitation.sender_id, invitation.recipient_id];
            members.sort_unstable();
            if members != [pair.0, pair.1]
                || command
                    .selection
                    .landmark
                    .as_ref()
                    .is_some_and(|landmark| invitation.selection.landmark.as_ref() != Some(landmark))
            {
                return Err(MeetingError::ResourceUnavailable);
            }
        }
        let (low, high) = command.selection.pair(actor);
        let context_id = command.selection.context_id(actor, event);
        ensure_context(
            &mut tx,
            event,
            &context_id,
            low,
            high,
            command.selection.landmark.as_ref(),
        )
        .await?;
        let (conversation_id, sequence) = next_message(
            &mut tx,
            event,
            &context_id,
            low,
            high,
            command.selection.landmark.as_ref(),
        )
        .await?;
        let (expires, purge) = policy.content_deadline(now)?;
        let id = Uuid::new_v4();
        let row = sqlx::query(
            "insert into concert_venue_meeting_messages
             (id,event_id,conversation_id,sequence,sender_id,body,invitation_id,created_at,
              expires_at,purge_at,policy_revision)
             values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) returning *",
        )
        .bind(id)
        .bind(event)
        .bind(conversation_id)
        .bind(sequence)
        .bind(actor)
        .bind(&command.body)
        .bind(command.invitation_id)
        .bind(now)
        .bind(expires)
        .bind(purge)
        .bind(policy.policy_revision)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let message = message_from_row(&row)?;
        let result = json!({
            "kind": "MESSAGE",
            "message": {
                "id": message.id,
                "conversationId": message.conversation_id,
                "sequence": message.sequence,
                "createdAt": message.created_at,
                "expiresAt": message.expires_at,
            },
        });
        record(
            &mut tx,
            event,
            actor,
            key,
            "sendVenueMeetingMessage",
            &digest,
            &context_id,
            &result,
            now,
            &policy,
        )
        .await?;
        tx.commit().await.map_err(storage)?;
        Ok(applied(key, "sendVenueMeetingMessage", result, false))
    }

    pub async fn command(
        &self,
        event: Uuid,
        actor: Uuid,
        key: Uuid,
        selection: &MeetingSelection,
    ) -> Result<Value, MeetingError> {
        let mut c = self.pool.acquire().await.map_err(storage)?;
        require_policy(&self.authority, event).await?;
        require_selection_friend(&self.authority, event, actor, selection).await?;
        let row = sqlx::query(
            "select result from concert_venue_meeting_receipts
             where event_id=$1 and actor_id=$2 and command_key=$3 and context_id=$4",
        )
        .bind(event)
        .bind(actor)
        .bind(key)
        .bind(selection.context_id(actor, event))
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?;
        let server_time = database_now(&mut c).await?;
        match row {
            Some(row) => {
                let result: Value = field(&row, "result")?;
                Ok(json!({
                    "commandKey": key,
                    "state": "APPLIED",
                    "result": result,
                    "serverTime": server_time,
                }))
            }
            None => Ok(json!({
                "commandKey": key,
                "state": "UNKNOWN",
                "serverTime": server_time,
            })),
        }
    }
}

pub fn storage(_: sqlx::Error) -> MeetingError {
    MeetingError::StorageUnavailable
}

fn storage_err() -> MeetingError {
    MeetingError::StorageUnavailable
}

async fn database_now(c: &mut PgConnection) -> Result<DateTime<Utc>, MeetingError> {
    sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(c)
        .await
        .map_err(storage)
}

fn digest(
    operation: &str,
    event: Uuid,
    target: Option<Uuid>,
    selection: &MeetingSelection,
) -> Result<Vec<u8>, MeetingError> {
    let mut value = Sha256::new();
    value.update(operation.as_bytes());
    value.update(b"\0");
    value.update(event.as_bytes());
    value.update(b"\0");
    value.update(target.map_or([0; 16], |id| *id.as_bytes()));
    value.update(b"\0");
    value.update(serde_json::to_vec(selection).map_err(|_| storage_err())?);
    Ok(value.finalize().to_vec())
}

async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    event: Uuid,
    actor: Uuid,
    key: Uuid,
    digest: &[u8],
) -> Result<Option<Value>, MeetingError> {
    let row = sqlx::query(
        "select request_digest, operation, result from concert_venue_meeting_receipts
         where event_id=$1 and actor_id=$2 and command_key=$3 for update",
    )
    .bind(event)
    .bind(actor)
    .bind(key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage)?;
    match row {
        None => Ok(None),
        Some(row) => {
            let stored: Vec<u8> = field(&row, "request_digest")?;
            if stored != digest {
                return Err(MeetingError::RevisionConflict);
            }
            let operation: String = field(&row, "operation")?;
            let result: Value = field(&row, "result")?;
            Ok(Some(applied(key, &operation, result, true)))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn record(
    tx: &mut Transaction<'_, Postgres>,
    event: Uuid,
    actor: Uuid,
    key: Uuid,
    operation: &str,
    digest: &[u8],
    context_id: &str,
    result: &Value,
    now: DateTime<Utc>,
    policy: &MeetingPolicy,
) -> Result<(), MeetingError> {
    let (expires, purge) = policy.content_deadline(now)?;
    sqlx::query(
        "insert into concert_venue_meeting_receipts
         (event_id,actor_id,command_key,operation,request_digest,context_id,result,expires_at,purge_at)
         values($1,$2,$3,$4,$5,$6,$7::jsonb,$8,$9)",
    )
    .bind(event)
    .bind(actor)
    .bind(key)
    .bind(operation)
    .bind(digest)
    .bind(context_id)
    .bind(serde_json::to_value(result).map_err(|_| storage_err())?)
    .bind(expires)
    .bind(purge)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}

fn applied(key: Uuid, operation: &str, result: Value, replayed: bool) -> Value {
    json!({
        "commandKey": key,
        "state": "APPLIED",
        "operation": operation,
        "result": result,
        "replayed": replayed,
    })
}

async fn set_ceiling(
    tx: &mut Transaction<'_, Postgres>,
    ceiling: DateTime<Utc>,
) -> Result<(), MeetingError> {
    sqlx::query("select set_config('mcr.venue_meeting_authority_until',$1,true)")
        .bind(ceiling.to_rfc3339())
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}

async fn ensure_context(
    tx: &mut Transaction<'_, Postgres>,
    event: Uuid,
    context_id: &str,
    low: Uuid,
    high: Uuid,
    landmark: Option<&LandmarkSelection>,
) -> Result<(), MeetingError> {
    sqlx::query(
        "insert into concert_venue_meeting_contexts
         (id,event_id,context_id,member_low,member_high,landmark_id)
         values($1,$2,$3,$4,$5,$6) on conflict (event_id,context_id) do nothing",
    )
    .bind(Uuid::new_v4())
    .bind(event)
    .bind(context_id)
    .bind(low)
    .bind(high)
    .bind(landmark.map(|value| value.landmark_id.as_str()))
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}

async fn next_message(
    tx: &mut Transaction<'_, Postgres>,
    event: Uuid,
    context_id: &str,
    low: Uuid,
    high: Uuid,
    landmark: Option<&LandmarkSelection>,
) -> Result<(Uuid, i64), MeetingError> {
    sqlx::query(
        "insert into concert_venue_meeting_conversations
         (id,event_id,context_id,member_low,member_high,landmark_id)
         values($1,$2,$3,$4,$5,$6) on conflict (event_id,context_id) do nothing",
    )
    .bind(Uuid::new_v4())
    .bind(event)
    .bind(context_id)
    .bind(low)
    .bind(high)
    .bind(landmark.map(|value| value.landmark_id.as_str()))
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    let sequence: i64 = sqlx::query_scalar(
        "update concert_venue_meeting_conversations
         set next_sequence=next_sequence+1
         where event_id=$1 and context_id=$2 returning next_sequence",
    )
    .bind(event)
    .bind(context_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage)?;
    let conversation: Uuid = sqlx::query_scalar(
        "select id from concert_venue_meeting_conversations where event_id=$1 and context_id=$2",
    )
    .bind(event)
    .bind(context_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage)?;
    Ok((conversation, sequence))
}

async fn gap_probe(
    c: &mut PgConnection,
    event: Uuid,
    conversation: Uuid,
    cursor: i64,
    first: i64,
) -> Result<bool, MeetingError> {
    if cursor == 0 || first <= cursor + 1 {
        return Ok(false);
    }
    let missing: bool = sqlx::query_scalar(
        "select exists(select 1 from concert_venue_meeting_messages
          where event_id=$1 and conversation_id=$2 and sequence>$3 and sequence<$4
            and expires_at<=clock_timestamp())",
    )
    .bind(event)
    .bind(conversation)
    .bind(cursor)
    .bind(first)
    .fetch_one(&mut *c)
    .await
    .map_err(storage)?;
    Ok(missing)
}

async fn load_invitation(
    c: &mut PgConnection,
    event: Uuid,
    id: Uuid,
) -> Result<StoredInvitation, MeetingError> {
    let row = sqlx::query("select * from concert_venue_meeting_invitations where event_id=$1 and id=$2")
        .bind(event)
        .bind(id)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?
        .ok_or(MeetingError::ResourceUnavailable)?;
    invitation_from_row(&row)
}

async fn load_invitation_for_update(
    tx: &mut Transaction<'_, Postgres>,
    event: Uuid,
    id: Uuid,
) -> Result<StoredInvitation, MeetingError> {
    let row = sqlx::query(
        "select * from concert_venue_meeting_invitations where event_id=$1 and id=$2 for update",
    )
    .bind(event)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage)?
    .ok_or(MeetingError::ResourceUnavailable)?;
    invitation_from_row(&row)
}

fn field<'r, T>(row: &'r sqlx::postgres::PgRow, name: &str) -> Result<T, MeetingError>
where
    T: sqlx::Decode<'r, Postgres> + sqlx::Type<Postgres>,
{
    row.try_get(name).map_err(|_| storage_err())
}

fn invitation_from_row(row: &sqlx::postgres::PgRow) -> Result<StoredInvitation, MeetingError> {
    let selection: Value = field(row, "selection")?;
    Ok(StoredInvitation {
        id: field(row, "id")?,
        ordinal: field(row, "ordinal")?,
        event_id: field(row, "event_id")?,
        context_id: field(row, "context_id")?,
        sender_id: field(row, "sender_id")?,
        recipient_id: field(row, "recipient_id")?,
        selection: serde_json::from_value(selection).map_err(|_| storage_err())?,
        state: InvitationState::parse(
            row.try_get::<String, _>("state")
                .map_err(|_| storage_err())?
                .as_str(),
        )?,
        revision: field(row, "revision")?,
        created_at: field(row, "created_at")?,
        expires_at: field(row, "expires_at")?,
        purge_at: field(row, "purge_at")?,
        policy_revision: field(row, "policy_revision")?,
    })
}

fn message_from_row(row: &sqlx::postgres::PgRow) -> Result<StoredMessage, MeetingError> {
    Ok(StoredMessage {
        id: field(row, "id")?,
        event_id: field(row, "event_id")?,
        conversation_id: field(row, "conversation_id")?,
        sequence: field(row, "sequence")?,
        sender_id: field(row, "sender_id")?,
        body: field(row, "body")?,
        invitation_id: field(row, "invitation_id")?,
        created_at: field(row, "created_at")?,
        expires_at: field(row, "expires_at")?,
        purge_at: field(row, "purge_at")?,
        policy_revision: field(row, "policy_revision")?,
    })
}

fn other_selection(
    invitation: &StoredInvitation,
    actor: Uuid,
) -> Result<MeetingSelection, MeetingError> {
    let peer = if actor == invitation.sender_id {
        invitation.recipient_id
    } else if actor == invitation.recipient_id {
        invitation.sender_id
    } else {
        return Err(MeetingError::ResourceUnavailable);
    };
    Ok(MeetingSelection {
        selected_friend_id: peer,
        landmark: invitation.selection.landmark.clone(),
    })
}

fn invitation_json(
    invitation: &StoredInvitation,
    actor: Uuid,
    now: DateTime<Utc>,
) -> Result<Value, MeetingError> {
    Ok(json!({
        "id": invitation.id,
        "eventId": invitation.event_id,
        "senderId": invitation.sender_id,
        "recipientId": invitation.recipient_id,
        "landmarkId": invitation
            .selection
            .landmark
            .as_ref()
            .map(|value| value.landmark_id.clone())
            .ok_or(MeetingError::StorageUnavailable)?,
        "revision": invitation.revision,
        "state": invitation.state.as_str(),
        "createdAt": invitation.created_at,
        "expiresAt": invitation.expires_at,
        "allowedInvitationActions": invitation.allowed_actions(actor, now),
    }))
}

fn message_json(message: &StoredMessage, _actor: Uuid) -> Result<Value, MeetingError> {
    Ok(json!({
        "id": message.id,
        "sequence": message.sequence,
        "senderId": message.sender_id,
        "body": message.body,
        "invitationId": message.invitation_id,
        "createdAt": message.created_at,
        "expiresAt": message.expires_at,
    }))
}

