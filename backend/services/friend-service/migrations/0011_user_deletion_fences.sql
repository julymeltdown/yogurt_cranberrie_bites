-- Keep a permanent local tombstone so stale access tokens and delayed events
-- cannot recreate FriendService relationship or recommendation data after the
-- Auth-owned deletion saga has completed this service's cleanup step.
create table friend_user_deletion_tombstones (
    user_id uuid primary key,
    saga_id uuid not null unique,
    actor_id uuid not null,
    retention_until timestamptz not null,
    applied_at timestamptz not null default now()
);

-- Advisory transaction locks avoid a persistent "active user" row for every
-- identity. Hash collisions only serialize unrelated subjects; sorting the
-- lock keys gives concurrent multi-subject mutations one lock order.
create or replace function friend_lock_subject_ids(subject_ids uuid[])
returns void
language plpgsql
as $$
declare
    lock_key bigint;
begin
    for lock_key in
        select distinct hashtextextended(subject_id::text, 0) as lock_key
        from unnest(coalesce(subject_ids, '{}'::uuid[])) as subjects(subject_id)
        where subject_id is not null
        order by lock_key
    loop
        perform pg_advisory_xact_lock(lock_key);
    end loop;
end;
$$;

create or replace function friend_assert_subjects_not_deleted(subject_ids uuid[])
returns void
language plpgsql
as $$
declare
    subject_id uuid;
begin
    perform friend_lock_subject_ids(subject_ids);

    for subject_id in
        select distinct candidate
        from unnest(coalesce(subject_ids, '{}'::uuid[])) as subjects(candidate)
        where candidate is not null
        order by candidate
    loop
        if exists (
            select 1 from friend_user_deletion_tombstones tombstone
            where tombstone.user_id = subject_id
        ) then
            raise exception 'Relationship mutation references a deleted user'
                using errcode = '23514',
                      constraint = 'friend_user_deletion_fence';
        end if;
    end loop;
end;
$$;

create or replace function friend_guard_user_deletion_fence()
returns trigger
language plpgsql
as $$
declare
    subject_ids uuid[];
    snapshot_viewer_id uuid;
begin
    if tg_table_name = 'follows' then
        subject_ids := array[new.follower_id, new.followee_id];
        -- Soft-delete operations remove an effect and must remain available;
        -- all creates and reactivations are fenced.
        if new.deleted_at is not null then
            return new;
        end if;
    elsif tg_table_name = 'follow_requests' then
        subject_ids := array[new.follower_id, new.followee_id];
        if new.deleted_at is not null then
            return new;
        end if;
    elsif tg_table_name = 'user_blocks' then
        subject_ids := array[new.blocker_id, new.blocked_id];
        if new.deleted_at is not null then
            return new;
        end if;
    elsif tg_table_name = 'user_mutes' then
        subject_ids := array[new.muter_id, new.muted_id];
        if new.deleted_at is not null then
            return new;
        end if;
    elsif tg_table_name = 'people_recommendation_candidates'
       or tg_table_name = 'people_recommendation_impressions' then
        subject_ids := array[new.viewer_id, new.candidate_user_id];
    elsif tg_table_name = 'people_recommendation_snapshots'
       or tg_table_name = 'people_recommendation_refresh_jobs'
       or tg_table_name = 'people_experiment_assignments' then
        subject_ids := array[new.viewer_id];
    elsif tg_table_name = 'people_recommendation_snapshot_items' then
        select snapshot.viewer_id into snapshot_viewer_id
        from people_recommendation_snapshots snapshot
        where snapshot.id = new.snapshot_id;
        subject_ids := array[snapshot_viewer_id, new.candidate_user_id];
    elsif tg_table_name = 'people_recommendation_events' then
        subject_ids := array[new.viewer_id, new.subject_id, new.actor_id];
    elsif tg_table_name = 'people_embeddings' then
        subject_ids := array[new.subject_id];
    else
        raise exception 'Unsupported FriendService deletion-fence table: %', tg_table_name;
    end if;

    perform friend_assert_subjects_not_deleted(subject_ids);
    return new;
end;
$$;

-- The existing relationship trigger normally queues recommendation refreshes
-- for both participants. During account deletion that could enqueue the newly
-- tombstoned user (and acquire a second subject lock in an unsafe order), so
-- skip only those saga-owned soft deletes; the saga removes the deleted user's
-- recommendation state in the same transaction.
create or replace function enqueue_people_recommendation_refresh()
returns trigger language plpgsql as $$
begin
    if new.deleted_at is not null and new.delete_reason = 'account_deletion' then
        return new;
    end if;

    if tg_table_name = 'follows' then
        insert into people_recommendation_refresh_jobs (viewer_id, status, next_attempt_at, updated_at)
        values (new.follower_id, 'PENDING', now(), now()),
               (new.followee_id, 'PENDING', now(), now())
        on conflict (viewer_id) do update
        set status='PENDING', next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at, now()), updated_at=now();
    elsif tg_table_name = 'follow_requests' then
        insert into people_recommendation_refresh_jobs (viewer_id, status, next_attempt_at, updated_at)
        values (new.follower_id, 'PENDING', now(), now()),
               (new.followee_id, 'PENDING', now(), now())
        on conflict (viewer_id) do update
        set status='PENDING', next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at, now()), updated_at=now();
    elsif tg_table_name = 'user_blocks' then
        insert into people_recommendation_refresh_jobs (viewer_id, status, next_attempt_at, updated_at)
        values (new.blocker_id, 'PENDING', now(), now()),
               (new.blocked_id, 'PENDING', now(), now())
        on conflict (viewer_id) do update
        set status='PENDING', next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at, now()), updated_at=now();
    elsif tg_table_name = 'user_mutes' then
        insert into people_recommendation_refresh_jobs (viewer_id, status, next_attempt_at, updated_at)
        values (new.muter_id, 'PENDING', now(), now()),
               (new.muted_id, 'PENDING', now(), now())
        on conflict (viewer_id) do update
        set status='PENDING', next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at, now()), updated_at=now();
    end if;
    return new;
end;
$$;

create trigger follows_user_deletion_fence
    before insert or update on follows
    for each row execute function friend_guard_user_deletion_fence();
create trigger follow_requests_user_deletion_fence
    before insert or update on follow_requests
    for each row execute function friend_guard_user_deletion_fence();
create trigger user_blocks_user_deletion_fence
    before insert or update on user_blocks
    for each row execute function friend_guard_user_deletion_fence();
create trigger user_mutes_user_deletion_fence
    before insert or update on user_mutes
    for each row execute function friend_guard_user_deletion_fence();

create trigger people_candidates_user_deletion_fence
    before insert or update on people_recommendation_candidates
    for each row execute function friend_guard_user_deletion_fence();
create trigger people_snapshots_user_deletion_fence
    before insert or update on people_recommendation_snapshots
    for each row execute function friend_guard_user_deletion_fence();
create trigger people_snapshot_items_user_deletion_fence
    before insert or update on people_recommendation_snapshot_items
    for each row execute function friend_guard_user_deletion_fence();
create trigger people_impressions_user_deletion_fence
    before insert or update on people_recommendation_impressions
    for each row execute function friend_guard_user_deletion_fence();
create trigger people_events_user_deletion_fence
    before insert or update on people_recommendation_events
    for each row execute function friend_guard_user_deletion_fence();
create trigger people_refresh_jobs_user_deletion_fence
    before insert on people_recommendation_refresh_jobs
    for each row execute function friend_guard_user_deletion_fence();
create trigger people_experiments_user_deletion_fence
    before insert or update on people_experiment_assignments
    for each row execute function friend_guard_user_deletion_fence();
create trigger people_embeddings_user_deletion_fence
    before insert or update on people_embeddings
    for each row execute function friend_guard_user_deletion_fence();
