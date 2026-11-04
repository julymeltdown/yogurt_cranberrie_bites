use super::*;
use concert_application::notification::Query;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

pub struct PgConcertInbox<A> {
    pool: PgPool,
    authority: A,
}
impl<A: FencedInboxAuthority> PgConcertInbox<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }

    /// Only a verified Notification-delivery workload adapter may invoke this.
    /// Envelope identity and fanout commit atomically. Retry ignores changed
    /// source projection after the first commit; it never resurrects purged rows.
    pub async fn ingest(&self, envelope: Envelope, mut recipients: Vec<Recipient>) -> Result<()> {
        envelope.validate()?;
        if recipients.len() > 1000 {
            return Err(NotificationError::InvalidInput);
        }
        for r in &recipients {
            r.validate()?;
        }
        recipients.sort_by_key(|r| r.user_id);
        if recipients.windows(2).any(|r| r[0].user_id == r[1].user_id) {
            return Err(NotificationError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        let inserted = sqlx::query("insert into concert_notification_deliveries(outbox_id,receipt_id,event_id,resource_id,resource_revision) values($1,$2,$3,$4,$5) on conflict do nothing")
            .bind(envelope.outbox_id).bind(envelope.receipt_id).bind(envelope.event_id).bind(envelope.resource_id).bind(envelope.resource_revision)
            .execute(&mut *tx).await.map_err(db)?.rows_affected() == 1;
        let row = sqlx::query("select receipt_id,event_id,resource_id,resource_revision from concert_notification_deliveries where outbox_id=$1 for update")
            .bind(envelope.outbox_id).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(NotificationError::IdempotencyConflict)?;
        if row.get::<Uuid, _>("receipt_id") != envelope.receipt_id
            || row.get::<Uuid, _>("event_id") != envelope.event_id
            || row.get::<Uuid, _>("resource_id") != envelope.resource_id
            || row.get::<i64, _>("resource_revision") != envelope.resource_revision
        {
            return Err(NotificationError::IdempotencyConflict);
        }
        if inserted {
            let mut subjects: Vec<_> = recipients
                .iter()
                .flat_map(|r| [Some(r.user_id), r.source_actor_id])
                .flatten()
                .collect();
            subjects.sort();
            subjects.dedup();
            lock_subjects(&mut tx, &subjects).await?;
            for r in recipients {
                let suppressed: bool = sqlx::query_scalar("select exists(select 1 from notification_subject_states where (subject_id=$1 and suppress_as_recipient) or (subject_id=$2 and suppress_as_actor))")
                    .bind(r.user_id).bind(r.source_actor_id).fetch_one(&mut *tx).await.map_err(db)?;
                if suppressed {
                    continue;
                }
                inbox_lock(&mut tx, r.user_id, envelope.event_id).await?;
                let id = Uuid::new_v4();
                sqlx::query("insert into notifications(id,user_id,type,payload_json,created_at,event_id,actor_id,occurred_at) values($1,$2,$3,'',clock_timestamp(),$4,$5,clock_timestamp())")
                    .bind(id).bind(r.user_id).bind(format!("MCR_{}",r.kind.as_str())).bind(envelope.outbox_id).bind(r.source_actor_id).execute(&mut *tx).await.map_err(db)?;
                sqlx::query("insert into concert_notification_items(notification_id,outbox_id,event_id,kind,target_type,target_id) values($1,$2,$3,$4,$5,$6)")
                    .bind(id).bind(envelope.outbox_id).bind(envelope.event_id).bind(r.kind.as_str()).bind(r.target.map(|v|v.target_type.as_str())).bind(r.target.map(|v|v.target_id)).execute(&mut *tx).await.map_err(db)?;
            }
        }
        tx.commit().await.map_err(db)
    }

    /// Full authorized unread snapshot; deliberately has no cursor/limit/filter.
    pub async fn unread_count(&self, context: Context) -> Result<UnreadCount> {
        let snapshot = self.snapshot(context).await?;
        let unread_total = snapshot
            .rows
            .iter()
            .zip(&snapshot.proof.decisions)
            .filter(|(r, (_, v))| r.value.read_at.is_none() && *v != Visibility::Hidden)
            .count();
        let result = UnreadCount {
            actor_id: context.actor_id,
            event_id: context.event_id,
            unread_total: i64::try_from(unread_total)
                .map_err(|_| NotificationError::Unavailable)?,
            as_of: snapshot.as_of,
        };
        snapshot.finish(context).await?;
        Ok(result)
    }

    pub async fn list(&self, context: Context, query: Query) -> Result<NotificationList> {
        if !(1..=100).contains(&query.limit) {
            return Err(NotificationError::InvalidInput);
        }
        let mut s = self.snapshot(context).await?;
        let (upper, after) = if let Some(cursor) = query.cursor {
            let token = canonical_uuid(&cursor)?;
            let r = sqlx::query("select upper_at,after_at,after_id from concert_notification_cursors where token=$1 and actor_id=$2 and event_id=$3 and unread_only=$4 and expires_at>clock_timestamp()")
                .bind(token).bind(context.actor_id).bind(context.event_id).bind(query.unread_only).fetch_optional(&mut *s.tx).await.map_err(db)?.ok_or(NotificationError::InvalidInput)?;
            (
                r.get("upper_at"),
                Some((r.get("after_at"), r.get("after_id"))),
            )
        } else {
            (s.as_of, None)
        };
        let mut items: Vec<Notification> = s
            .rows
            .iter()
            .zip(&s.proof.decisions)
            .filter(|(r, (_, v))| {
                *v != Visibility::Hidden
                    && (!query.unread_only || r.value.read_at.is_none())
                    && r.value.created_at <= upper
                    && after.is_none_or(|p| (r.value.created_at, r.value.notification_id) < p)
            })
            .map(|(r, (_, v))| projected(r, *v))
            .take(usize::from(query.limit) + 1)
            .collect();
        let next_cursor = if items.len() > usize::from(query.limit) {
            items.truncate(usize::from(query.limit));
            let last = items.last().ok_or(NotificationError::Unavailable)?;
            let token = Uuid::new_v4();
            sqlx::query("insert into concert_notification_cursors(token,actor_id,event_id,unread_only,upper_at,after_at,after_id,expires_at) values($1,$2,$3,$4,$5,$6,$7,clock_timestamp()+interval '15 minutes')")
                .bind(token).bind(context.actor_id).bind(context.event_id).bind(query.unread_only).bind(upper).bind(last.created_at).bind(last.notification_id).execute(&mut *s.tx).await.map_err(db)?;
            Some(token.to_string())
        } else {
            None
        };
        s.finish(context).await?;
        Ok(NotificationList { items, next_cursor })
    }

    pub async fn mark_read(
        &self,
        context: Context,
        id: Uuid,
        expected: i64,
        key: Uuid,
    ) -> Result<Notification> {
        if id.is_nil() || key.is_nil() || !(1..=concert_domain::MAX_REVISION).contains(&expected) {
            return Err(NotificationError::InvalidInput);
        }
        let mut s = self.snapshot(context).await?;
        inbox_lock(&mut s.tx, context.actor_id, context.event_id).await?;
        let index = s
            .rows
            .iter()
            .position(|r| r.value.notification_id == id)
            .ok_or(NotificationError::ResourceUnavailable)?;
        let visibility = s.proof.decisions[index].1;
        if visibility == Visibility::Hidden {
            return Err(NotificationError::ResourceUnavailable);
        }
        let receipt = read_receipt(&mut s.tx, context, "SINGLE", key).await?;
        if let Some(r) = receipt {
            if r.get::<Option<Uuid>, _>("notification_id") != Some(id)
                || r.get::<Option<i64>, _>("expected_revision") != Some(expected)
            {
                return Err(NotificationError::IdempotencyConflict);
            }
            check_receipt(&r, s.as_of)?;
        } else {
            // FOR UPDATE sees concurrent inherited read/deletion writes. A repeatable
            // read serialization failure is unavailable, never a fabricated success.
            let current = sqlx::query("select read_at,deleted_at from notifications where id=$1 and user_id=$2 for update")
                .bind(id).bind(context.actor_id).fetch_one(&mut *s.tx).await.map_err(db)?;
            if current
                .get::<Option<DateTime<Utc>>, _>("deleted_at")
                .is_some()
            {
                return Err(NotificationError::ResourceUnavailable);
            }
            let read_at: Option<DateTime<Utc>> = current.get("read_at");
            if expected != if read_at.is_some() { 2 } else { 1 } {
                return Err(NotificationError::VersionConflict);
            }
            if read_at.is_none() {
                sqlx::query("update notifications set read_at=$2 where id=$1 and read_at is null")
                    .bind(id)
                    .bind(s.as_of)
                    .execute(&mut *s.tx)
                    .await
                    .map_err(db)?;
            }
            save_receipt(
                &mut s.tx,
                context,
                "SINGLE",
                key,
                Some(id),
                Some(expected),
                i32::from(read_at.is_none()),
                s.as_of,
            )
            .await?;
            s.rows[index].value.read_at = Some(read_at.unwrap_or(s.as_of));
            s.rows[index].value.revision = 2;
        }
        let result = projected(&s.rows[index], visibility);
        s.finish(context).await?;
        Ok(result)
    }

    /// Server cutoff and exact receipt survive restart and key replay; later
    /// notifications stay unread. Only currently authorized rows are affected.
    pub async fn mark_all_read(&self, context: Context, key: Uuid) -> Result<ReadAllResult> {
        if key.is_nil() {
            return Err(NotificationError::InvalidInput);
        }
        let mut s = self.snapshot(context).await?;
        inbox_lock(&mut s.tx, context.actor_id, context.event_id).await?;
        let result = if let Some(r) = read_receipt(&mut s.tx, context, "ALL", key).await? {
            check_receipt(&r, s.as_of)?;
            ReadAllResult {
                read_count: r.get("read_count"),
                read_at: r.get("read_at"),
            }
        } else {
            let mut ids: Vec<_> = s
                .rows
                .iter()
                .zip(&s.proof.decisions)
                .filter(|(r, (_, v))| {
                    r.value.read_at.is_none()
                        && *v != Visibility::Hidden
                        && r.value.created_at <= s.as_of
                })
                .map(|(r, _)| r.value.notification_id)
                .collect();
            ids.sort();
            let changed = sqlx::query("update notifications set read_at=$3 where user_id=$1 and id=any($2) and read_at is null and deleted_at is null")
                .bind(context.actor_id).bind(&ids).bind(s.as_of).execute(&mut *s.tx).await.map_err(db)?.rows_affected();
            let count = i32::try_from(changed).map_err(|_| NotificationError::Unavailable)?;
            save_receipt(&mut s.tx, context, "ALL", key, None, None, count, s.as_of).await?;
            ReadAllResult {
                read_count: count,
                read_at: s.as_of,
            }
        };
        s.finish(context).await?;
        Ok(result)
    }

    async fn snapshot(&self, context: Context) -> Result<Snapshot<'_>> {
        context.check(Utc::now())?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("set transaction isolation level repeatable read")
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        // Bound all SQL/lock waits and commit by the original authority; zero never disables it.
        let remaining = (context.expires_at - Utc::now()).num_milliseconds();
        if remaining <= 0 {
            return Err(NotificationError::PermissionDenied);
        }
        sqlx::query("select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true),set_config('mcr.notification_authority_until',$2,true)")
            .bind(format!("{remaining}ms")).bind(context.expires_at.to_rfc3339()).execute(&mut *tx).await.map_err(db)?;
        let as_of: DateTime<Utc> = sqlx::query_scalar("select transaction_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        let records = sqlx::query("select n.id,n.user_id,n.actor_id,n.created_at,n.read_at,i.event_id,i.kind,i.target_type,i.target_id from notifications n join concert_notification_items i on i.notification_id=n.id where n.user_id=$1 and i.event_id=$2 and n.deleted_at is null order by n.created_at desc,n.id desc")
            .bind(context.actor_id).bind(context.event_id).fetch_all(&mut *tx).await.map_err(db)?;
        let rows = records
            .into_iter()
            .map(parse_row)
            .collect::<Result<Vec<_>>>()?;
        let mut subjects = vec![context.actor_id];
        subjects.extend(rows.iter().filter_map(|r| r.candidate.source_actor_id));
        subjects.sort();
        subjects.dedup();
        lock_subjects(&mut tx, &subjects).await?;
        // These permanent rows fence the inherited deletion saga through commit.
        let suppressed: Vec<(Uuid,bool,bool)> = sqlx::query_as("select subject_id,suppress_as_recipient,suppress_as_actor from notification_subject_states where subject_id=any($1) order by subject_id")
            .bind(&subjects).fetch_all(&mut *tx).await.map_err(db)?;
        if suppressed
            .iter()
            .any(|(id, recipient, _)| *id == context.actor_id && *recipient)
        {
            return Err(NotificationError::PermissionDenied);
        }
        let candidates: Vec<_> = rows.iter().map(|r| r.candidate).collect();
        let now = Utc::now();
        let mut proof = self
            .authority
            .authorize_in(&mut tx, context, &candidates, now)
            .await?;
        proof.validate(context, &candidates, Utc::now())?;
        for (candidate, visibility) in &mut proof.decisions {
            if suppressed
                .iter()
                .any(|(id, _, actor)| *actor && Some(*id) == candidate.source_actor_id)
            {
                *visibility = Visibility::Hidden;
            }
        }
        sqlx::query("select set_config('mcr.notification_authority_until',$1,true)")
            .bind(proof.valid_until.to_rfc3339())
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        Ok(Snapshot {
            tx,
            rows,
            proof,
            as_of,
        })
    }
}

struct Stored {
    value: Notification,
    candidate: Candidate,
}
struct Snapshot<'a> {
    tx: Transaction<'a, Postgres>,
    rows: Vec<Stored>,
    proof: Authorization,
    as_of: DateTime<Utc>,
}
impl Snapshot<'_> {
    async fn finish(self, context: Context) -> Result<()> {
        let candidates: Vec<_> = self.rows.iter().map(|r| r.candidate).collect();
        self.proof.validate(context, &candidates, Utc::now())?;
        self.tx.commit().await.map_err(db)?;
        self.proof.validate(context, &candidates, Utc::now())
    }
}
fn parse_row(r: sqlx::postgres::PgRow) -> Result<Stored> {
    let target = match (
        r.get::<Option<String>, _>("target_type"),
        r.get::<Option<Uuid>, _>("target_id"),
    ) {
        (Some(t), Some(id)) => Some(Target {
            target_type: TargetType::parse(&t).map_err(|_| NotificationError::Unavailable)?,
            target_id: id,
        }),
        (None, None) => None,
        _ => return Err(NotificationError::Unavailable),
    };
    let read_at = r.get::<Option<DateTime<Utc>>, _>("read_at");
    Ok(Stored {
        value: Notification {
            notification_id: r.get("id"),
            event_id: r.get("event_id"),
            kind: Kind::parse(&r.get::<String, _>("kind"))
                .map_err(|_| NotificationError::Unavailable)?,
            target,
            created_at: r.get("created_at"),
            read_at,
            revision: if read_at.is_some() { 2 } else { 1 },
        },
        candidate: Candidate {
            notification_id: r.get("id"),
            source_actor_id: r.get("actor_id"),
            target,
        },
    })
}
fn projected(r: &Stored, v: Visibility) -> Notification {
    let mut n = r.value.clone();
    if v != Visibility::Visible {
        n.target = None;
    }
    n
}
async fn lock_subjects(tx: &mut Transaction<'_, Postgres>, subjects: &[Uuid]) -> Result<()> {
    // Match the inherited repository's two phases. Locking A before inserting a
    // missing B can deadlock an inherited writer that inserted B before locking A.
    let mut subjects = subjects.to_vec();
    subjects.sort_unstable();
    subjects.dedup();
    for id in &subjects {
        sqlx::query(
            "insert into notification_subject_states(subject_id) values($1) on conflict do nothing",
        )
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(db)?;
    }
    for id in &subjects {
        sqlx::query(
            "select subject_id from notification_subject_states where subject_id=$1 for update",
        )
        .bind(id)
        .fetch_one(&mut **tx)
        .await
        .map_err(db)?;
    }
    Ok(())
}
async fn inbox_lock(tx: &mut Transaction<'_, Postgres>, actor: Uuid, event: Uuid) -> Result<()> {
    sqlx::query("insert into concert_notification_inbox_states(actor_id,event_id) values($1,$2) on conflict do nothing").bind(actor).bind(event).execute(&mut **tx).await.map_err(db)?;
    sqlx::query("select actor_id from concert_notification_inbox_states where actor_id=$1 and event_id=$2 for update").bind(actor).bind(event).fetch_one(&mut **tx).await.map_err(db)?;
    Ok(())
}
async fn read_receipt(
    tx: &mut Transaction<'_, Postgres>,
    c: Context,
    op: &str,
    key: Uuid,
) -> Result<Option<sqlx::postgres::PgRow>> {
    sqlx::query("select notification_id,expected_revision,read_count,read_at,expires_at from concert_notification_read_receipts where actor_id=$1 and event_id=$2 and operation=$3 and idempotency_key=$4")
        .bind(c.actor_id).bind(c.event_id).bind(op).bind(key).fetch_optional(&mut **tx).await.map_err(db)
}
fn check_receipt(r: &sqlx::postgres::PgRow, now: DateTime<Utc>) -> Result<()> {
    if r.get::<DateTime<Utc>, _>("expires_at") <= now {
        Err(NotificationError::IdempotencyExpired)
    } else {
        Ok(())
    }
}
#[allow(clippy::too_many_arguments)]
async fn save_receipt(
    tx: &mut Transaction<'_, Postgres>,
    c: Context,
    op: &str,
    key: Uuid,
    id: Option<Uuid>,
    expected: Option<i64>,
    count: i32,
    at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query("insert into concert_notification_read_receipts(actor_id,event_id,operation,idempotency_key,notification_id,expected_revision,read_count,read_at,expires_at) values($1,$2,$3,$4,$5,$6,$7,$8,$8+interval '24 hours')")
        .bind(c.actor_id).bind(c.event_id).bind(op).bind(key).bind(id).bind(expected).bind(count).bind(at).execute(&mut **tx).await.map_err(db)?;
    Ok(())
}
fn canonical_uuid(s: &str) -> Result<Uuid> {
    Uuid::parse_str(s)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == s)
        .ok_or(NotificationError::InvalidInput)
}
fn db(_: sqlx::Error) -> NotificationError {
    NotificationError::Unavailable
}
