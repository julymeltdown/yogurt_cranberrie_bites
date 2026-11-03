use super::*;
use post_application::concert_feed::Access;
use post_domain::concert_feed::{next_revision, Command, Comment, Mutation, Outcome, Post, State};
use sqlx::Row;
use uuid::Uuid;

impl Owner {
    pub async fn execute(&self, context: &Context, command: &Command) -> Result<Outcome, Error> {
        command.validate()?;
        context.binds(command)?;
        context.validate(Utc::now())?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // Two actor/key and operation locks precede the parent lock. Domain-tagged
        // hashes avoid ambiguous identities; collisions only serialize requests.
        let fingerprint = digest(command)?;
        let key = digest(&command.idempotency_key)?;
        lock(&mut tx, "key", &(command.actor_id, command.event_id, key)).await?;
        lock(&mut tx, "operation", &command.operation_id).await?;
        lock(&mut tx, "post", &command.post_id).await?;
        let existing=sqlx::query("select operation_id,command_digest,outcome from post_concert_receipts where operation_id=$1 or (actor_id=$2 and event_id=$3 and key_digest=$4)")
            .bind(command.operation_id).bind(command.actor_id).bind(command.event_id).bind(key.as_slice())
            .fetch_all(&mut *tx).await.map_err(storage)?;
        if !existing.is_empty() {
            if existing.len() != 1
                || existing[0].get::<Vec<u8>, _>("command_digest") != fingerprint
                || existing[0].get::<Uuid, _>("operation_id") != command.operation_id
            {
                return Err(Error::Idempotency);
            }
            // Replay is metadata-only and still requires current authority. No
            // hidden/deleted post/comment body can be recovered from receipts.
            let parent = load_post(&mut tx, command.event_id, command.post_id).await?;
            let comment = if let Some(id) = command.mutation.comment_id() {
                load_comment(&mut tx, command.event_id, command.post_id, id).await?
            } else {
                None
            };
            validate_replay(command, parent.as_ref(), comment.as_ref())?;
            let access = Access::Receipt {
                command: command.clone(),
                parent,
                comment,
            };
            let guard = self.admit(context, &access, &mut tx).await?;
            let result: Outcome = serde_json::from_value(existing[0].get("outcome"))
                .map_err(|_| Error::Unavailable)?;
            checked_commit(tx, context, &[guard]).await?;
            return Ok(result);
        }
        let mut parent = load_post(&mut tx, command.event_id, command.post_id).await?;
        let mut comment = if let Some(id) = command.mutation.comment_id() {
            load_comment(&mut tx, command.event_id, command.post_id, id).await?
        } else {
            None
        };
        validate_owner(command, parent.as_ref(), comment.as_ref())?;
        let access = Access::Write {
            command: command.clone(),
            parent: parent.clone(),
            comment: comment.clone(),
        };
        let guard = self.admit(context, &access, &mut tx).await?;
        let now = db_now(&mut tx).await?;
        let mut reacted = false;
        match &command.mutation {
            Mutation::CreatePost { content } => {
                parent = Some(Post {
                    post_id: command.post_id,
                    event_id: command.event_id,
                    author_id: command.actor_id,
                    content: content.clone(),
                    revision: 1,
                    state: State::Published,
                    created_at: now,
                    edited_at: None,
                    comment_count: 0,
                    reaction_count: 0,
                });
                let p = parent.as_ref().ok_or(Error::Unavailable)?;
                sqlx::query("insert into post_concert_posts(post_id,event_id,author_id,content,revision,state,created_at) values($1,$2,$3,$4,1,'PUBLISHED',$5)")
                    .bind(p.post_id).bind(p.event_id).bind(p.author_id).bind(serde_json::to_value(&p.content).map_err(|_|Error::Invalid)?).bind(now).execute(&mut *tx).await.map_err(|e|if e.as_database_error().is_some_and(|e| e.is_unique_violation()){Error::Conflict}else{storage(e)})?;
            }
            Mutation::EditPost { content, .. } => {
                let p = parent.as_mut().ok_or(Error::NotFound)?;
                p.content = content.clone();
                p.edited_at = Some(now);
                p.advance()?;
                save_post(&mut tx, p).await?;
            }
            Mutation::DeletePost { .. } => {
                let p = parent.as_mut().ok_or(Error::NotFound)?;
                p.state = State::Deleted;
                p.advance()?;
                save_post(&mut tx, p).await?;
            }
            Mutation::CreateComment {
                comment_id, text, ..
            } => {
                let p = parent.as_mut().ok_or(Error::NotFound)?;
                let c = Comment {
                    comment_id: *comment_id,
                    event_id: command.event_id,
                    post_id: command.post_id,
                    author_id: command.actor_id,
                    text: text.clone(),
                    revision: 1,
                    state: State::Published,
                    created_at: now,
                    edited_at: None,
                };
                sqlx::query("insert into post_concert_comments(comment_id,event_id,post_id,author_id,text,revision,state,created_at) values($1,$2,$3,$4,$5,1,'PUBLISHED',$6)")
                    .bind(c.comment_id).bind(c.event_id).bind(c.post_id).bind(c.author_id).bind(&c.text).bind(now).execute(&mut *tx).await.map_err(|e|if e.as_database_error().is_some_and(|e|e.is_unique_violation()){Error::Conflict}else{storage(e)})?;
                comment = Some(c);
                p.comment_count = p
                    .comment_count
                    .checked_add(1)
                    .filter(|v| *v <= i32::MAX as i64)
                    .ok_or(Error::Conflict)?;
                p.advance()?;
                save_post(&mut tx, p).await?;
            }
            Mutation::EditComment { .. } | Mutation::DeleteComment { .. } => {
                let p = parent.as_mut().ok_or(Error::NotFound)?;
                let c = comment.as_mut().ok_or(Error::NotFound)?;
                if matches!(command.mutation, Mutation::DeleteComment { .. }) {
                    c.state = State::Deleted;
                    p.comment_count = p
                        .comment_count
                        .checked_sub(1)
                        .filter(|v| *v >= 0)
                        .ok_or(Error::Conflict)?;
                } else {
                    if let Mutation::EditComment { text, .. } = &command.mutation {
                        c.text = text.clone();
                    }
                    c.edited_at = Some(now);
                }
                c.revision = next_revision(c.revision)?;
                sqlx::query("update post_concert_comments set text=$2,state=$3,revision=$4,edited_at=$5 where comment_id=$1")
                    .bind(c.comment_id).bind(&c.text).bind(state_name(c.state)).bind(c.revision).bind(c.edited_at).execute(&mut *tx).await.map_err(storage)?;
                p.advance()?;
                save_post(&mut tx, p).await?;
            }
            Mutation::SetReaction { active, .. } => {
                let p = parent.as_mut().ok_or(Error::NotFound)?;
                reacted = *active;
                let changed = if *active {
                    sqlx::query("insert into post_concert_reactions(event_id,post_id,actor_id,created_at) values($1,$2,$3,$4) on conflict do nothing").bind(command.event_id).bind(command.post_id).bind(command.actor_id).bind(now).execute(&mut *tx).await.map_err(storage)?.rows_affected()
                } else {
                    sqlx::query("delete from post_concert_reactions where event_id=$1 and post_id=$2 and actor_id=$3").bind(command.event_id).bind(command.post_id).bind(command.actor_id).execute(&mut *tx).await.map_err(storage)?.rows_affected()
                };
                if changed > 0 {
                    p.reaction_count = (p.reaction_count + if *active { 1 } else { -1 }).max(0);
                    if p.reaction_count > i32::MAX as i64 {
                        return Err(Error::Conflict);
                    }
                }
                // Even an accepted no-op receives its own revision/receipt/outbox.
                p.advance()?;
                save_post(&mut tx, p).await?;
            }
        }
        let p = parent.ok_or(Error::Unavailable)?;
        let outcome = Outcome {
            actor_id: command.actor_id,
            operation_id: command.operation_id,
            event_id: command.event_id,
            post_id: command.post_id,
            comment_id: command.mutation.comment_id(),
            revision: comment.as_ref().map_or(p.revision, |c| c.revision),
            post_revision: p.revision,
            deleted: matches!(
                command.mutation,
                Mutation::DeletePost { .. } | Mutation::DeleteComment { .. }
            ),
            reacted_by_me: reacted,
            reaction_count: p.reaction_count,
            committed_at: now,
        };
        let json = serde_json::to_value(&outcome).map_err(|_| Error::Unavailable)?;
        sqlx::query("insert into post_concert_receipts(operation_id,actor_id,event_id,post_id,comment_id,key_digest,command_digest,original_authority,original_deadline,outcome,committed_at) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
            .bind(command.operation_id).bind(command.actor_id).bind(command.event_id).bind(command.post_id).bind(outcome.comment_id).bind(key.as_slice()).bind(fingerprint.as_slice()).bind(context.original_authority.as_slice()).bind(context.expires_at).bind(&json).bind(now).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("insert into post_concert_outbox(operation_id,event_id,post_id,post_revision,kind,payload,occurred_at) values($1,$2,$3,$4,$5,$6,$7)")
            .bind(command.operation_id).bind(command.event_id).bind(command.post_id).bind(p.revision).bind(command.mutation.name()).bind(json).bind(now).execute(&mut *tx).await.map_err(storage)?;
        checked_commit(tx, context, &[guard]).await?;
        Ok(outcome)
    }
}
pub(super) fn validate_replay(
    command: &Command,
    parent: Option<&Post>,
    comment: Option<&Comment>,
) -> Result<(), Error> {
    let p = parent.ok_or(Error::NotFound)?;
    if matches!(command.mutation, Mutation::DeletePost { .. }) {
        if p.event_id != command.event_id
            || p.author_id != command.actor_id
            || p.state != State::Deleted
        {
            return Err(Error::NotFound);
        }
    } else {
        p.visible_in(command.event_id)?;
    }
    if command.mutation.comment_id().is_some() {
        let c = comment.ok_or(Error::NotFound)?;
        let expected = if matches!(command.mutation, Mutation::DeleteComment { .. }) {
            State::Deleted
        } else {
            State::Published
        };
        if c.event_id != command.event_id
            || c.post_id != command.post_id
            || c.author_id != command.actor_id
            || c.state != expected
        {
            return Err(Error::NotFound);
        }
    }
    Ok(())
}
fn validate_owner(
    command: &Command,
    parent: Option<&Post>,
    comment: Option<&Comment>,
) -> Result<(), Error> {
    match &command.mutation {
        Mutation::CreatePost { .. } => {
            if parent.is_some() {
                return Err(Error::Conflict);
            }
        }
        Mutation::EditPost {
            expected_revision, ..
        }
        | Mutation::DeletePost { expected_revision } => parent.ok_or(Error::NotFound)?.owner(
            command.actor_id,
            command.event_id,
            *expected_revision,
        )?,
        Mutation::CreateComment { post_revision, .. } => {
            let p = parent.ok_or(Error::NotFound)?;
            p.visible_in(command.event_id)?;
            p.expect(*post_revision)?;
            if comment.is_some() {
                return Err(Error::Conflict);
            }
        }
        Mutation::EditComment {
            expected_revision, ..
        }
        | Mutation::DeleteComment {
            expected_revision, ..
        } => {
            parent
                .ok_or(Error::NotFound)?
                .visible_in(command.event_id)?;
            comment.ok_or(Error::NotFound)?.owner(
                command.actor_id,
                command.event_id,
                command.post_id,
                *expected_revision,
            )?;
        }
        Mutation::SetReaction {
            expected_revision, ..
        } => {
            let p = parent.ok_or(Error::NotFound)?;
            p.visible_in(command.event_id)?;
            p.expect(*expected_revision)?;
        }
    }
    Ok(())
}
async fn lock<T: serde::Serialize>(
    tx: &mut Transaction<'_, Postgres>,
    domain: &str,
    key: &T,
) -> Result<(), Error> {
    // Hash locally: raw idempotency keys never become SQL parameters, including
    // advisory locks that an operator might capture in database diagnostics.
    let hash = digest(&("mcr-post-concert", domain, key))?;
    let id = i64::from_be_bytes(hash[..8].try_into().map_err(|_| Error::Unavailable)?);
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}
pub(super) async fn load_post(
    tx: &mut Transaction<'_, Postgres>,
    event: Uuid,
    id: Uuid,
) -> Result<Option<Post>, Error> {
    let row =
        sqlx::query("select * from post_concert_posts where event_id=$1 and post_id=$2 for share")
            .bind(event)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?;
    row.as_ref().map(post_row).transpose()
}
pub(super) async fn load_comment(
    tx: &mut Transaction<'_, Postgres>,
    event: Uuid,
    post: Uuid,
    id: Uuid,
) -> Result<Option<Comment>, Error> {
    let row=sqlx::query("select * from post_concert_comments where event_id=$1 and post_id=$2 and comment_id=$3 for share").bind(event).bind(post).bind(id).fetch_optional(&mut **tx).await.map_err(storage)?;
    row.as_ref().map(comment_row).transpose()
}
pub(super) fn post_row(row: &sqlx::postgres::PgRow) -> Result<Post, Error> {
    Ok(Post {
        post_id: row.get("post_id"),
        event_id: row.get("event_id"),
        author_id: row.get("author_id"),
        content: serde_json::from_value(row.get("content")).map_err(|_| Error::Unavailable)?,
        revision: row.get("revision"),
        state: parse_state(row.get("state"))?,
        created_at: row.get("created_at"),
        edited_at: row.get("edited_at"),
        comment_count: row.get("comment_count"),
        reaction_count: row.get("reaction_count"),
    })
}
pub(super) fn comment_row(row: &sqlx::postgres::PgRow) -> Result<Comment, Error> {
    Ok(Comment {
        comment_id: row.get("comment_id"),
        event_id: row.get("event_id"),
        post_id: row.get("post_id"),
        author_id: row.get("author_id"),
        text: row.get("text"),
        revision: row.get("revision"),
        state: parse_state(row.get("state"))?,
        created_at: row.get("created_at"),
        edited_at: row.get("edited_at"),
    })
}
fn parse_state(state: &str) -> Result<State, Error> {
    match state {
        "PUBLISHED" => Ok(State::Published),
        "HIDDEN" => Ok(State::Hidden),
        "DELETED" => Ok(State::Deleted),
        _ => Err(Error::Unavailable),
    }
}
fn state_name(state: State) -> &'static str {
    match state {
        State::Published => "PUBLISHED",
        State::Hidden => "HIDDEN",
        State::Deleted => "DELETED",
    }
}
async fn save_post(tx: &mut Transaction<'_, Postgres>, p: &Post) -> Result<(), Error> {
    sqlx::query("update post_concert_posts set content=$2,revision=$3,state=$4,edited_at=$5,comment_count=$6,reaction_count=$7 where post_id=$1")
        .bind(p.post_id).bind(serde_json::to_value(&p.content).map_err(|_|Error::Unavailable)?).bind(p.revision).bind(state_name(p.state)).bind(p.edited_at).bind(p.comment_count).bind(p.reaction_count).execute(&mut **tx).await.map_err(storage)?;
    Ok(())
}
