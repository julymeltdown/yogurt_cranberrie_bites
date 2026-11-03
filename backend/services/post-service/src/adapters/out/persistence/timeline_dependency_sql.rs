//! Bound the raw unpublished window before dependency predicates and locks.
//! The position is only scheduling fairness; it proves no work or authority.
pub const CANDIDATES_SQL: &str = r#"
with candidates as materialized (
    select event_id,created_at from post_event_outbox
    where not published and created_at<=$2 and (created_at,event_id)<=($4::timestamptz,$5::uuid)
    order by created_at,event_id limit $3
)
select candidate.event_id as candidate_event_id,
       candidate.created_at as candidate_created_at,parent.event_id
from candidates candidate
left join lateral (
    select event.event_id from post_event_outbox event
    join post_event_delivery_jobs job on job.event_id=event.event_id and job.target=$1
               where event.event_id=candidate.event_id and not event.published and event.created_at<=$2
                 and (event.locked_at is null or event.locked_at<=clock_timestamp()-interval '5 minutes')
                 and ((job.state in ('PENDING','RETRY') and job.next_attempt_at<=clock_timestamp())
                      or (job.state='LEASED' and job.lease_expires_at<=clock_timestamp())
                      or (job.state in ('SUCCEEDED','SKIPPED') and not exists (
                          select 1 from post_event_delivery_jobs outstanding
                          where outstanding.event_id=event.event_id
                            and (event.delivery_required_targets is null
                                 or outstanding.target=any(event.delivery_required_targets))
                            and outstanding.state<>'SUCCEEDED')))
                 and not exists(
                     select 1 from post_event_outbox earlier join post_event_delivery_jobs previous
                       on previous.event_id=earlier.event_id and previous.target=job.target
                     where earlier.aggregate_id=event.aggregate_id and earlier.event_id<>event.event_id
                       and previous.state not in ('SUCCEEDED','SKIPPED')
                       and not(previous.state='LEASED' and previous.lease_kind='REPLAY' and previous.replay_restore_state in ('SUCCEEDED','SKIPPED'))
                       and not exists(select 1 from event_publications p where p.event_id=previous.event_id and p.target=previous.target)
                       and ((event.aggregate_version is null and earlier.aggregate_version is null and (earlier.created_at,earlier.event_id)<(event.created_at,event.event_id))
                            or (event.aggregate_version is not null and earlier.aggregate_version is null)
                            or (event.aggregate_version is not null and earlier.aggregate_version is not null and earlier.aggregate_version<event.aggregate_version))
                 )

                 and (job.state in ('SUCCEEDED','SKIPPED') or job.attempt_count-job.progress_credits>=job.max_attempts
                      or public.post_timeline_affinity_ready(event.event_id))

    for update of event skip locked
) parent on true
order by candidate.created_at,candidate.event_id
"#;

pub const AFTER_CANDIDATES_SQL: &str = r#"
with candidates as materialized (
    select event_id,created_at from post_event_outbox
    where not published and created_at<=$2 and (created_at,event_id)<=($4::timestamptz,$5::uuid)
      and (created_at,event_id)>($6::timestamptz,$7::uuid)
    order by created_at,event_id limit $3
)
select candidate.event_id as candidate_event_id,
       candidate.created_at as candidate_created_at,parent.event_id
from candidates candidate
left join lateral (
    select event.event_id from post_event_outbox event
    join post_event_delivery_jobs job on job.event_id=event.event_id and job.target=$1
               where event.event_id=candidate.event_id and not event.published and event.created_at<=$2
                 and (event.locked_at is null or event.locked_at<=clock_timestamp()-interval '5 minutes')
                 and ((job.state in ('PENDING','RETRY') and job.next_attempt_at<=clock_timestamp())
                      or (job.state='LEASED' and job.lease_expires_at<=clock_timestamp())
                      or (job.state in ('SUCCEEDED','SKIPPED') and not exists (
                          select 1 from post_event_delivery_jobs outstanding
                          where outstanding.event_id=event.event_id
                            and (event.delivery_required_targets is null
                                 or outstanding.target=any(event.delivery_required_targets))
                            and outstanding.state<>'SUCCEEDED')))
                 and not exists(
                     select 1 from post_event_outbox earlier join post_event_delivery_jobs previous
                       on previous.event_id=earlier.event_id and previous.target=job.target
                     where earlier.aggregate_id=event.aggregate_id and earlier.event_id<>event.event_id
                       and previous.state not in ('SUCCEEDED','SKIPPED')
                       and not(previous.state='LEASED' and previous.lease_kind='REPLAY' and previous.replay_restore_state in ('SUCCEEDED','SKIPPED'))
                       and not exists(select 1 from event_publications p where p.event_id=previous.event_id and p.target=previous.target)
                       and ((event.aggregate_version is null and earlier.aggregate_version is null and (earlier.created_at,earlier.event_id)<(event.created_at,event.event_id))
                            or (event.aggregate_version is not null and earlier.aggregate_version is null)
                            or (event.aggregate_version is not null and earlier.aggregate_version is not null and earlier.aggregate_version<event.aggregate_version))
                 )

                 and (job.state in ('SUCCEEDED','SKIPPED') or job.attempt_count-job.progress_credits>=job.max_attempts
                      or public.post_timeline_affinity_ready(event.event_id))

    for update of event skip locked
) parent on true
order by candidate.created_at,candidate.event_id
"#;
