use super::persistence::{comment_row, load_comment, load_post, post_row};
use super::*;
use post_application::concert_feed::{Access, Filter, Page, Position, PostView, Window};
use post_domain::concert_feed::{Command, Comment, Outcome, State};
use sqlx::Row;
use uuid::Uuid;

impl Owner {
    pub async fn post(&self, context: &Context, id: Uuid) -> Result<Option<PostView>, Error> {
        context.validate(Utc::now())?;
        if id.is_nil() {
            return Err(Error::Invalid);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let Some(p) = load_post(&mut tx, context.event_id, id).await? else {
            return Ok(None);
        };
        p.visible_in(context.event_id)?;
        let access = Access::ReadPost { post: p.clone() };
        let guard = self.admit(context, &access, &mut tx).await?;
        let reacted_by_me = reaction(&mut tx, id, context.actor_id).await?;
        checked_commit(tx, context, &[guard]).await?;
        Ok(Some(PostView {
            post: p,
            reacted_by_me,
        }))
    }
    pub async fn page(&self, context: &Context, window: &Window) -> Result<Page<PostView>, Error> {
        context.validate(Utc::now())?;
        window.validate()?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let list_guard = self
            .admit(
                context,
                &Access::ListPosts {
                    window: window.clone(),
                },
                &mut tx,
            )
            .await?;
        let after_time = window.after.map(|p| p.created_at);
        let after_id = window.after.map(|p| p.id);
        let filter = match window.filter {
            Filter::All => "ALL",
            Filter::WithMedia => "WITH_MEDIA",
            Filter::PartyLink => "PARTY",
            Filter::GiveawayLink => "GIVEAWAY",
        };
        // Bound owner scans rather than loading unbounded content before policy.
        // Concert can continue at scanned_through even if an entire batch is denied.
        let rows=sqlx::query("select p.* from post_concert_posts p where event_id=$1 and state='PUBLISHED' and created_at <= $2 and ($3::timestamptz is null or (created_at,post_id)<($3,$4::uuid)) and ($5='ALL' or ($5='WITH_MEDIA' and jsonb_array_length(content->'media_ids')>0) or ($5 in ('PARTY','GIVEAWAY') and exists(select 1 from jsonb_array_elements(content->'links') l where l->>'target_type'=$5))) order by created_at desc,post_id desc limit $6 for share")
            .bind(context.event_id).bind(window.upper).bind(after_time).bind(after_id).bind(filter).bind(i64::from(window.limit)+1).fetch_all(&mut *tx).await.map_err(storage)?;
        let has_more = rows.len() > window.limit as usize;
        let mut items = vec![];
        let mut guards = vec![list_guard];
        let mut scanned_through = None;
        for row in rows.iter().take(window.limit as usize) {
            let p = post_row(row)?;
            scanned_through = Some(Position {
                created_at: p.created_at,
                id: p.post_id,
            });
            let access = Access::ReadPost { post: p.clone() };
            match self.admit(context, &access, &mut tx).await {
                Ok(guard) => {
                    let reacted_by_me = reaction(&mut tx, p.post_id, context.actor_id).await?;
                    guards.push(guard);
                    items.push(PostView {
                        post: p,
                        reacted_by_me,
                    });
                }
                Err(Error::Forbidden | Error::NotFound) => continue,
                Err(e) => return Err(e),
            }
        }
        checked_commit(tx, context, &guards).await?;
        Ok(Page {
            items,
            scanned_through,
            has_more,
        })
    }
    pub async fn comments(
        &self,
        context: &Context,
        post_id: Uuid,
        upper: DateTime<Utc>,
        after: Option<Position>,
        limit: u32,
    ) -> Result<Page<Comment>, Error> {
        context.validate(Utc::now())?;
        if !(1..=100).contains(&limit)
            || post_id.is_nil()
            || after.is_some_and(|a| a.id.is_nil() || a.created_at > upper)
        {
            return Err(Error::Invalid);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let p = load_post(&mut tx, context.event_id, post_id)
            .await?
            .ok_or(Error::NotFound)?;
        p.visible_in(context.event_id)?;
        let access = Access::ReadPost { post: p.clone() };
        let mut guards = vec![self.admit(context, &access, &mut tx).await?];
        let rows=sqlx::query("select * from post_concert_comments where event_id=$1 and post_id=$2 and state='PUBLISHED' and ($3::timestamptz is null or (created_at,comment_id)<($3,$4::uuid)) and created_at <= $6 order by created_at desc,comment_id desc limit $5 for share")
            .bind(context.event_id).bind(post_id).bind(after.map(|a|a.created_at)).bind(after.map(|a|a.id)).bind(i64::from(limit)+1).bind(upper).fetch_all(&mut *tx).await.map_err(storage)?;
        let has_more = rows.len() > limit as usize;
        let mut items = vec![];
        let mut scanned_through = None;
        for row in rows.iter().take(limit as usize) {
            let c = comment_row(row)?;
            scanned_through = Some(Position {
                created_at: c.created_at,
                id: c.comment_id,
            });
            if c.state != State::Published {
                continue;
            }
            let access = Access::ReadComment {
                parent: p.clone(),
                comment: c.clone(),
            };
            match self.admit(context, &access, &mut tx).await {
                Ok(guard) => {
                    guards.push(guard);
                    items.push(c);
                }
                Err(Error::Forbidden | Error::NotFound) => continue,
                Err(e) => return Err(e),
            }
        }
        checked_commit(tx, context, &guards).await?;
        Ok(Page {
            items,
            scanned_through,
            has_more,
        })
    }
    pub async fn comment(&self, context: &Context, id: Uuid) -> Result<Option<Comment>, Error> {
        context.validate(Utc::now())?;
        if id.is_nil() {
            return Err(Error::Invalid);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let post_id: Option<Uuid> = sqlx::query_scalar(
            "select post_id from post_concert_comments where event_id=$1 and comment_id=$2",
        )
        .bind(context.event_id)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        let Some(post_id) = post_id else {
            return Ok(None);
        };
        let p = load_post(&mut tx, context.event_id, post_id)
            .await?
            .ok_or(Error::NotFound)?;
        p.visible_in(context.event_id)?;
        let c = load_comment(&mut tx, context.event_id, post_id, id)
            .await?
            .ok_or(Error::NotFound)?;
        if c.state != State::Published {
            return Err(Error::NotFound);
        }
        let parent_guard = self
            .admit(context, &Access::ReadPost { post: p.clone() }, &mut tx)
            .await?;
        let guard = self
            .admit(
                context,
                &Access::ReadComment {
                    parent: p,
                    comment: c.clone(),
                },
                &mut tx,
            )
            .await?;
        checked_commit(tx, context, &[parent_guard, guard]).await?;
        Ok(Some(c))
    }
    /// Lookup never executes a missing command and returns no historical body.
    pub async fn receipt(
        &self,
        context: &Context,
        command: &Command,
    ) -> Result<Option<Outcome>, Error> {
        context.validate(Utc::now())?;
        context.binds(command)?;
        command.validate()?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let parent = load_post(&mut tx, command.event_id, command.post_id).await?;
        let comment = if let Some(id) = command.mutation.comment_id() {
            load_comment(&mut tx, command.event_id, command.post_id, id).await?
        } else {
            None
        };
        let guard = self
            .admit(
                context,
                &Access::Receipt {
                    command: command.clone(),
                    parent,
                    comment,
                },
                &mut tx,
            )
            .await?;
        let row=sqlx::query("select command_digest,outcome from post_concert_receipts where operation_id=$1 and actor_id=$2 and event_id=$3")
            .bind(command.operation_id).bind(context.actor_id).bind(context.event_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        let result = if let Some(row) = row {
            if row.get::<Vec<u8>, _>("command_digest") != digest(command)? {
                return Err(Error::Idempotency);
            }
            Some(serde_json::from_value(row.get("outcome")).map_err(|_| Error::Unavailable)?)
        } else {
            None
        };
        checked_commit(tx, context, &[guard]).await?;
        Ok(result)
    }
}
async fn reaction(
    tx: &mut Transaction<'_, Postgres>,
    post: Uuid,
    actor: Uuid,
) -> Result<bool, Error> {
    sqlx::query_scalar(
        "select exists(select 1 from post_concert_reactions where post_id=$1 and actor_id=$2)",
    )
    .bind(post)
    .bind(actor)
    .fetch_one(&mut **tx)
    .await
    .map_err(storage)
}
