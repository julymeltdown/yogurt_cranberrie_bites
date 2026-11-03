/// Bound the expired-attempt window BEFORE parent locks and receipt checks.
/// No-progress attempts are examined once; a fresh attempt ID becomes eligible
/// automatically without renewing policy or inventing a retry epoch.
pub const CANDIDATES_SQL: &str = r#"
with candidates as materialized (
    select event_id,lease_expires_at from post_event_delivery_jobs
    where target='TIMELINE' and state='LEASED'
      and progress_checked_attempt_id is distinct from attempt_id
      and lease_expires_at<=statement_timestamp()
    order by lease_expires_at,event_id limit $1
)
select candidate.event_id as candidate_event_id,
       candidate.lease_expires_at as candidate_expiry,parent.event_id
from candidates candidate
left join lateral (
    select event_id from post_event_outbox
    where event_id=candidate.event_id
    for update skip locked
) parent on true
order by candidate.lease_expires_at,candidate.event_id
"#;

/// A separate seek form keeps the tuple range in the index condition instead
/// of a nullable-parameter OR filter that could scan the already-visited prefix.
pub const AFTER_CANDIDATES_SQL: &str = r#"
with candidates as materialized (
    select event_id,lease_expires_at from post_event_delivery_jobs
    where target='TIMELINE' and state='LEASED'
      and progress_checked_attempt_id is distinct from attempt_id
      and lease_expires_at<=statement_timestamp()
      and (lease_expires_at,event_id)>($2::timestamptz,$3::uuid)
    order by lease_expires_at,event_id limit $1
)
select candidate.event_id as candidate_event_id,
       candidate.lease_expires_at as candidate_expiry,parent.event_id
from candidates candidate
left join lateral (
    select event_id from post_event_outbox
    where event_id=candidate.event_id
    for update skip locked
) parent on true
order by candidate.lease_expires_at,candidate.event_id
"#;
