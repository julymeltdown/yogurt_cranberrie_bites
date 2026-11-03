-- Auth, Post, and the feed projections own separate lifecycle state. Keep a
-- permanent local tombstone so stale Gateway writes and delayed feed work
-- cannot recreate user-owned content after this saga step commits.
create table post_user_deletion_tombstones (
    user_id uuid primary key,
    saga_id uuid not null unique,
    actor_id uuid not null,
    retention_until timestamptz not null,
    applied_at timestamptz not null default now()
);

create or replace function post_lock_subject_ids(subject_ids uuid[])
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

create or replace function post_assert_subjects_not_deleted(subject_ids uuid[])
returns void
language plpgsql
as $$
declare
    subject_id uuid;
begin
    perform post_lock_subject_ids(subject_ids);

    for subject_id in
        select distinct candidate
        from unnest(coalesce(subject_ids, '{}'::uuid[])) as subjects(candidate)
        where candidate is not null
        order by candidate
    loop
        if exists (
            select 1 from post_user_deletion_tombstones tombstone
            where tombstone.user_id = subject_id
        ) then
            raise exception 'Post mutation references a deleted user'
                using errcode = '23514',
                      constraint = 'post_user_deletion_fence';
        end if;
    end loop;
end;
$$;

create or replace function post_guard_user_deletion_fence()
returns trigger
language plpgsql
as $$
declare
    subject_ids uuid[];
    viewer_id uuid;
    post_author_id uuid;
begin
    if tg_table_name = 'posts' then
        if new.deleted_at is not null then
            return new;
        end if;
        subject_ids := array[new.author_id];
    elsif tg_table_name = 'comments' then
        if new.deleted_at is not null then
            return new;
        end if;
        subject_ids := array[new.author_id];
    elsif tg_table_name = 'post_likes' then
        if new.deleted_at is not null then
            return new;
        end if;
        subject_ids := array[new.user_id];
    elsif tg_table_name = 'post_event_outbox' then
        subject_ids := array[new.actor_id];
    elsif tg_table_name = 'home_timeline_entries' then
        subject_ids := array[new.viewer_id, new.author_id];
    elsif tg_table_name = 'high_fanout_posts' then
        subject_ids := array[new.author_id];
    elsif tg_table_name = 'feed_author_affinity'
       or tg_table_name = 'feed_author_affinities' then
        subject_ids := array[new.viewer_id, new.author_id];
    elsif tg_table_name = 'feed_snapshots'
       or tg_table_name = 'feed_recommendation_snapshots'
       or tg_table_name = 'feed_experiment_assignments' then
        subject_ids := array[new.viewer_id];
    elsif tg_table_name = 'feed_snapshot_items' then
        select snapshot.viewer_id into viewer_id
        from feed_snapshots snapshot
        where snapshot.id = new.snapshot_id;
        subject_ids := array[viewer_id, new.author_id];
    elsif tg_table_name = 'feed_impressions'
       or tg_table_name = 'feed_recommendation_impressions' then
        subject_ids := array[new.viewer_id, new.author_id];
    elsif tg_table_name = 'feed_recommendation_events' then
        subject_ids := array[new.viewer_id, new.actor_id];
        if new.subject_type = 'USER' then
            subject_ids := array_append(subject_ids, new.subject_id);
        elsif new.subject_type = 'POST' then
            select post.author_id into post_author_id
            from posts post where post.id = new.subject_id;
            subject_ids := array_append(subject_ids, post_author_id);
        elsif new.subject_type = 'COMMENT' then
            select comment.author_id into post_author_id
            from comments comment where comment.id = new.subject_id;
            subject_ids := array_append(subject_ids, post_author_id);
        end if;
    elsif tg_table_name = 'feed_recommendation_snapshot_items' then
        select snapshot.viewer_id into viewer_id
        from feed_recommendation_snapshots snapshot
        where snapshot.snapshot_id = new.snapshot_id;
        subject_ids := array[viewer_id, new.author_id];
    elsif tg_table_name = 'feed_fanout_jobs' then
        subject_ids := array[new.author_id];
    elsif tg_table_name = 'feed_recent_actions' then
        select post.author_id into post_author_id
        from posts post where post.id = new.post_id;
        subject_ids := array[new.viewer_id, post_author_id];
    elsif tg_table_name = 'post_engagement_velocity' then
        select post.author_id into post_author_id
        from posts post where post.id = new.post_id;
        subject_ids := array[post_author_id];
    elsif tg_table_name = 'feed_embeddings' then
        if new.subject_type = 'USER' then
            subject_ids := array[new.subject_id];
        else
            select post.author_id into post_author_id
            from posts post where post.id = new.subject_id;
            subject_ids := array[post_author_id];
        end if;
    else
        raise exception 'Unsupported PostService deletion-fence table: %', tg_table_name;
    end if;

    perform post_assert_subjects_not_deleted(subject_ids);
    return new;
end;
$$;

create trigger posts_user_deletion_fence
    before insert or update on posts
    for each row execute function post_guard_user_deletion_fence();
create trigger comments_user_deletion_fence
    before insert or update on comments
    for each row execute function post_guard_user_deletion_fence();
create trigger post_likes_user_deletion_fence
    before insert or update on post_likes
    for each row execute function post_guard_user_deletion_fence();
create trigger post_event_outbox_user_deletion_fence
    before insert on post_event_outbox
    for each row execute function post_guard_user_deletion_fence();

create trigger home_timeline_entries_user_deletion_fence
    before insert or update on home_timeline_entries
    for each row execute function post_guard_user_deletion_fence();
create trigger high_fanout_posts_user_deletion_fence
    before insert or update on high_fanout_posts
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_author_affinity_user_deletion_fence
    before insert or update on feed_author_affinity
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_author_affinities_user_deletion_fence
    before insert or update on feed_author_affinities
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_snapshots_user_deletion_fence
    before insert or update on feed_snapshots
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_snapshot_items_user_deletion_fence
    before insert or update on feed_snapshot_items
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_impressions_user_deletion_fence
    before insert or update on feed_impressions
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_recommendation_events_user_deletion_fence
    before insert or update on feed_recommendation_events
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_recommendation_snapshots_user_deletion_fence
    before insert or update on feed_recommendation_snapshots
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_recommendation_snapshot_items_user_deletion_fence
    before insert or update on feed_recommendation_snapshot_items
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_recommendation_impressions_user_deletion_fence
    before insert or update on feed_recommendation_impressions
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_fanout_jobs_user_deletion_fence
    before insert on feed_fanout_jobs
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_recent_actions_user_deletion_fence
    before insert or update on feed_recent_actions
    for each row execute function post_guard_user_deletion_fence();
create trigger post_engagement_velocity_user_deletion_fence
    before insert or update on post_engagement_velocity
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_experiment_assignments_user_deletion_fence
    before insert or update on feed_experiment_assignments
    for each row execute function post_guard_user_deletion_fence();
create trigger feed_embeddings_user_deletion_fence
    before insert or update on feed_embeddings
    for each row execute function post_guard_user_deletion_fence();
