-- Controlled-test foundation only. No startup issuance, historical adoption,
-- production clone/restore authority or ordered resolver is installed here.
create table feed_affinity_test_provisioning (
    singleton boolean primary key default true check (singleton),
    provisioning_id uuid not null unique,
    system_identifier text not null,
    database_oid bigint not null,
    database_name text not null,
    capture_epoch uuid not null references feed_affinity_capture_epoch(epoch_id),
    feature_epoch uuid not null,
    writer_generation uuid not null,
    feature_version text not null check (feature_version='feed-affinity-v1'),
    migration_manifest jsonb not null,
    retired boolean not null default false,
    issued_at timestamptz not null default clock_timestamp()
);
revoke all on feed_affinity_test_provisioning from public;

alter table feed_affinity_pair_provenance drop constraint feed_affinity_origin_mode;
alter table feed_affinity_pair_provenance add constraint feed_affinity_origin_mode
    check (origin_mode in ('HOLD_CAPTURE_ONLY','ORDERED_FRESH_BLOCKED'));

create table feed_affinity_pair_baselines (
    epoch_id uuid not null,
    viewer_id uuid not null,
    author_id uuid not null,
    provisioning_id uuid not null references feed_affinity_test_provisioning(provisioning_id),
    feature_epoch uuid not null,
    writer_generation uuid not null,
    feature_version text not null check (feature_version='feed-affinity-v1'),
    positive_score float8 not null default 0 check (positive_score=0),
    negative_score float8 not null default 0 check (negative_score=0),
    last_interacted_at timestamptz null check (last_interacted_at is null),
    resolved_sequence bigint not null default 0 check (resolved_sequence=0),
    initialized_at timestamptz not null default clock_timestamp(),
    primary key(epoch_id,viewer_id,author_id),
    foreign key(epoch_id,viewer_id,author_id)
        references feed_affinity_source_pairs(epoch_id,viewer_id,author_id) on delete cascade
);
create index feed_affinity_baseline_viewer_idx on feed_affinity_pair_baselines(viewer_id);
create index feed_affinity_baseline_author_idx on feed_affinity_pair_baselines(author_id);
revoke all on feed_affinity_pair_baselines from public;

create function feed_affinity_migration_manifest() returns jsonb language plpgsql stable
set search_path=pg_catalog,public,pg_temp as $$
declare manifest jsonb;
begin
    select jsonb_agg(jsonb_build_object('version',version,'checksum',encode(checksum,'hex')) order by version)
    into manifest from public._sqlx_migrations;
    return manifest;
end;
$$;
revoke all on function feed_affinity_migration_manifest() from public;

create function feed_affinity_issue_test_provisioning(
    provision uuid, expected_system text, expected_oid bigint, expected_name text,
    expected_capture uuid, feature uuid, generation uuid, expected_manifest jsonb
) returns void language plpgsql set search_path=pg_catalog,public,pg_temp as $$
begin
    perform pg_advisory_xact_lock(hashtextextended('affinity-provision:'||current_database(),0));
    if provision is null or feature is null or generation is null or provision=generation
        or provision='00000000-0000-0000-0000-000000000000'::uuid
        or feature='00000000-0000-0000-0000-000000000000'::uuid
        or generation='00000000-0000-0000-0000-000000000000'::uuid
        or expected_system is distinct from (select system_identifier::text from pg_control_system())
        or expected_oid is distinct from (select oid::bigint from pg_database where datname=current_database())
        or expected_name is distinct from current_database()
        or expected_capture is distinct from (select epoch_id from public.feed_affinity_capture_epoch where singleton)
        or current_setting('server_version_num')::int/10000<>16
        or expected_manifest is null or jsonb_typeof(expected_manifest)<>'array'
        or not exists(select 1 from public._sqlx_migrations where version=25 and success)
        or exists(select 1 from public._sqlx_migrations where not success)
        or expected_manifest is distinct from public.feed_affinity_migration_manifest() then
        raise exception 'Invalid controlled provisioning identity/manifest'
            using errcode='23514',constraint='feed_affinity_provisioning_authority';
    end if;
    -- This is a contradiction check within trusted observed creation, not an
    -- absence-based production freshness certificate. The controller must own
    -- creation/lifetime evidence and install this before starting the writer.
    if exists(select 1 from public.posts) or exists(select 1 from public.post_event_outbox)
        or exists(select 1 from public.feed_recommendation_events)
        or exists(select 1 from public.feed_author_affinity)
        or exists(select 1 from public.feed_author_affinities)
        or exists(select 1 from public.feed_projection_inbox)
        or exists(select 1 from public.feed_affinity_source_pairs)
        or exists(select 1 from public.feed_affinity_source_contributions)
        or exists(select 1 from public.feed_affinity_pair_provenance)
        or exists(select 1 from public.feed_affinity_legacy_effect_observations)
        or exists(select 1 from public.post_user_deletion_tombstones) then
        raise exception 'Controlled provisioning has pre-existing facts/effects/history'
            using errcode='23514',constraint='feed_affinity_provisioning_authority';
    end if;
    insert into public.feed_affinity_test_provisioning(singleton,provisioning_id,system_identifier,
        database_oid,database_name,capture_epoch,feature_epoch,writer_generation,feature_version,migration_manifest)
    values(true,provision,expected_system,expected_oid,expected_name,expected_capture,feature,generation,
        'feed-affinity-v1',expected_manifest);
end;
$$;
revoke all on function feed_affinity_issue_test_provisioning(uuid,text,bigint,text,uuid,uuid,uuid,jsonb) from public;

create function feed_affinity_provisioning_guard() returns trigger language plpgsql
set search_path=pg_catalog,public,pg_temp as $$
begin
    if tg_op='DELETE' or (to_jsonb(new)-'retired') is distinct from (to_jsonb(old)-'retired')
        or (old.retired and not new.retired) then
        raise exception 'Provisioning authority cannot be replaced or resurrected'
            using errcode='23514',constraint='feed_affinity_provisioning_authority';
    end if;
    return new;
end;
$$;
create trigger feed_affinity_provisioning_guard before update or delete on feed_affinity_test_provisioning
    for each row execute function feed_affinity_provisioning_guard();

create function feed_affinity_validate_test_authority() returns feed_affinity_test_provisioning
language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
declare authority public.feed_affinity_test_provisioning%rowtype;
begin
    select * into authority from public.feed_affinity_test_provisioning where singleton for share;
    if not found or authority.retired
        or authority.system_identifier is distinct from (select system_identifier::text from pg_control_system())
        or authority.database_oid is distinct from (select oid::bigint from pg_database where datname=current_database())
        or authority.database_name is distinct from current_database()
        or authority.capture_epoch is distinct from (select epoch_id from public.feed_affinity_capture_epoch where singleton)
        or exists(select 1 from public._sqlx_migrations where not success)
        or authority.migration_manifest is distinct from public.feed_affinity_migration_manifest() then
        raise exception 'Controlled affinity authority missing, retired or mismatched'
            using errcode='23514',constraint='feed_affinity_provisioning_authority';
    end if;
    return authority;
end;
$$;
revoke all on function feed_affinity_validate_test_authority() from public;

create function feed_affinity_lock_source_authority() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare eligible boolean;
begin
    -- a00 already acquires the complete eligible-source subject set. This lock
    -- serializes issuance and allocation before the actual source INSERT.
    perform pg_advisory_xact_lock_shared(hashtextextended('affinity-provision:'||current_database(),0));
    if tg_table_name='post_event_outbox' then
        eligible:=new.event_type in ('POST_LIKED','POST_UNLIKED','POST_COMMENTED','POST_COMMENT_DELETED');
    else eligible:=new.action<>'IMPRESSION'; end if;
    if eligible and exists(select 1 from public.feed_affinity_test_provisioning) then
        perform public.feed_affinity_validate_test_authority();
    end if;
    return new;
end;
$$;
create trigger a01_affinity_authority before insert on post_event_outbox
    for each row execute function feed_affinity_lock_source_authority();
create trigger a01_affinity_authority before insert on feed_recommendation_events
    for each row execute function feed_affinity_lock_source_authority();
revoke all on function feed_affinity_lock_source_authority() from public;

create function feed_affinity_pair_baseline_guard() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare authority public.feed_affinity_test_provisioning%rowtype;
begin
    if tg_op='DELETE' then
        if exists(select 1 from public.feed_affinity_pair_baselines b
            where (b.epoch_id,b.viewer_id,b.author_id)=(old.epoch_id,old.viewer_id,old.author_id))
            and not exists(select 1 from public.post_user_deletion_tombstones
                where user_id in (old.viewer_id,old.author_id)) then
            raise exception 'Ordered pair cannot be erased and reinitialized'
                using errcode='23514',constraint='feed_affinity_baseline_authority';
        end if;
        return old;
    end if;
    if not exists(select 1 from public.feed_affinity_test_provisioning) and not exists(
        select 1 from public.feed_affinity_pair_baselines b
        where (b.epoch_id,b.viewer_id,b.author_id)=(new.epoch_id,new.viewer_id,new.author_id)) then return new; end if;
    authority:=public.feed_affinity_validate_test_authority();
    if tg_op='INSERT' then
        -- INSERT ... ON CONFLICT runs BEFORE INSERT even for an existing pair.
        -- Its actual +1 allocation is checked by the separate UPDATE branch.
        if exists(select 1 from public.feed_affinity_source_pairs
            where (epoch_id,viewer_id,author_id)=(new.epoch_id,new.viewer_id,new.author_id)) then
            if not exists(select 1 from public.feed_affinity_pair_baselines b
                join public.feed_affinity_pair_provenance o using(epoch_id,viewer_id,author_id)
                where (b.epoch_id,b.viewer_id,b.author_id)=(new.epoch_id,new.viewer_id,new.author_id)
                    and o.origin_mode='ORDERED_FRESH_BLOCKED' and b.provisioning_id=authority.provisioning_id
                    and b.writer_generation=authority.writer_generation and b.feature_epoch=authority.feature_epoch) then
                raise exception 'Existing pair has no qualified baseline'
                    using errcode='23514',constraint='feed_affinity_baseline_authority';
            end if;
            return new;
        end if;
        if new.allocated_sequence<>1 or new.epoch_id<>authority.capture_epoch
            or exists(select 1 from public.feed_affinity_source_pairs where viewer_id=new.viewer_id and author_id=new.author_id)
            or exists(select 1 from public.feed_affinity_pair_provenance where viewer_id=new.viewer_id and author_id=new.author_id)
            or exists(select 1 from public.feed_affinity_source_contributions where viewer_id=new.viewer_id and author_id=new.author_id)
            or exists(select 1 from public.feed_author_affinity where viewer_id=new.viewer_id and author_id=new.author_id)
            or exists(select 1 from public.feed_author_affinities where viewer_id=new.viewer_id and author_id=new.author_id)
            or exists(select 1 from public.feed_affinity_legacy_effect_observations where viewer_id=new.viewer_id and author_id=new.author_id) then
            raise exception 'Fresh baseline contradicts retained pair history'
                using errcode='23514',constraint='feed_affinity_baseline_authority';
        end if;
    elsif (new.epoch_id,new.viewer_id,new.author_id) is distinct from (old.epoch_id,old.viewer_id,old.author_id)
        or new.allocated_sequence<>old.allocated_sequence+1
        or not exists(select 1 from public.feed_affinity_pair_baselines b
            join public.feed_affinity_pair_provenance o using(epoch_id,viewer_id,author_id)
            where (b.epoch_id,b.viewer_id,b.author_id)=(old.epoch_id,old.viewer_id,old.author_id)
                and o.origin_mode='ORDERED_FRESH_BLOCKED' and b.provisioning_id=authority.provisioning_id
                and b.writer_generation=authority.writer_generation and b.feature_epoch=authority.feature_epoch) then
        raise exception 'Invalid ordered pair allocation'
            using errcode='23514',constraint='feed_affinity_baseline_authority';
    end if;
    return new;
end;
$$;
create trigger feed_affinity_pair_baseline_guard before insert or update or delete on feed_affinity_source_pairs
    for each row execute function feed_affinity_pair_baseline_guard();
revoke all on function feed_affinity_pair_baseline_guard() from public;

create function feed_affinity_initialize_baseline() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare authority public.feed_affinity_test_provisioning%rowtype;
begin
    if not exists(select 1 from public.feed_affinity_test_provisioning) then return new; end if;
    authority:=public.feed_affinity_validate_test_authority();
    insert into public.feed_affinity_pair_provenance(epoch_id,viewer_id,author_id,origin_mode)
        values(new.epoch_id,new.viewer_id,new.author_id,'ORDERED_FRESH_BLOCKED');
    insert into public.feed_affinity_pair_baselines(epoch_id,viewer_id,author_id,provisioning_id,
        feature_epoch,writer_generation,feature_version)
        values(new.epoch_id,new.viewer_id,new.author_id,authority.provisioning_id,
            authority.feature_epoch,authority.writer_generation,authority.feature_version);
    return new;
end;
$$;
create trigger feed_affinity_initialize_baseline after insert on feed_affinity_source_pairs
    for each row execute function feed_affinity_initialize_baseline();
revoke all on function feed_affinity_initialize_baseline() from public;

-- Internal capture is callable only from the owner-source triggers. Trigger
-- entry points run with the migration owner's fixed path, not runtime grants.
revoke all on function feed_affinity_capture_source(text,uuid,uuid,uuid,uuid,uuid,timestamptz,float8,float8,text) from public;
alter function feed_affinity_capture_outbox() security definer;
alter function feed_affinity_capture_outbox() set search_path=pg_catalog,public,pg_temp;
alter function feed_affinity_capture_recommendation() security definer;
alter function feed_affinity_capture_recommendation() set search_path=pg_catalog,public,pg_temp;
revoke all on function feed_affinity_capture_outbox() from public;
revoke all on function feed_affinity_capture_recommendation() from public;

-- A counter/baseline may not commit without its actual owner-backed source.
create function feed_affinity_verify_allocated_source() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare c public.feed_affinity_source_contributions%rowtype;
    e public.post_event_outbox%rowtype;
    r public.feed_recommendation_events%rowtype;
    owner_id uuid; expected_positive float8; expected_negative float8;
    actual_hash text; delta float8;
begin
    if not exists(select 1 from public.feed_affinity_pair_baselines b
        where (b.epoch_id,b.viewer_id,b.author_id)=(new.epoch_id,new.viewer_id,new.author_id)) then return null; end if;
    select * into c from public.feed_affinity_source_contributions
        where epoch_id=new.epoch_id and viewer_id=new.viewer_id and author_id=new.author_id
            and source_sequence=new.allocated_sequence;
    if not found then raise exception 'Allocation lacks an actual source'
        using errcode='23514',constraint='feed_affinity_baseline_source'; end if;
    if c.namespace='OUTBOX' then
        select * into strict e from public.post_event_outbox where event_id=c.source_id;
        select author_id into owner_id from public.posts where id=e.aggregate_id;
        delta:=case e.event_type when 'POST_LIKED' then 0.08 when 'POST_UNLIKED' then -0.08
            when 'POST_COMMENTED' then 0.16 when 'POST_COMMENT_DELETED' then -0.16 end;
        expected_positive:=greatest(delta,0); expected_negative:=greatest(-delta,0);
        actual_hash:=public.feed_affinity_identity_hash(jsonb_build_object('eventId',e.event_id,
            'eventType',e.event_type,'aggregateId',e.aggregate_id,'aggregateVersion',e.aggregate_version,
            'actorId',e.actor_id,'attributes',e.attributes,'payload',e.payload,
            'occurredAt',extract(epoch from e.occurred_at),'createdAt',extract(epoch from e.created_at)));
        if delta is null or (c.viewer_id,c.author_id,c.actor_id,c.post_id,c.occurred_at)
            is distinct from (e.actor_id,owner_id,e.actor_id,e.aggregate_id,e.occurred_at) then
            raise exception 'Allocation differs from actual outbox source'
                using errcode='23514',constraint='feed_affinity_baseline_source'; end if;
    else
        select * into strict r from public.feed_recommendation_events where event_id=c.source_id;
        select author_id into owner_id from public.feed_recommendation_impressions
            where impression_id=r.impression_id and viewer_id=r.viewer_id and post_id=r.subject_id;
        expected_positive:=case r.action when 'LIKE' then 0.20 when 'COMMENT' then 0.30
            when 'SAVE' then 0.35 when 'SHARE' then 0.35 when 'FOLLOW' then 0.50
            when 'ACCEPT_FOLLOW' then 0.50 when 'CLICK' then 0.08
            when 'DWELL' then least(coalesce(r.dwell_ms,0),60000)::float8/60000*0.20 else 0 end;
        expected_negative:=case r.action when 'FAST_SCROLL' then 0.15 when 'HIDE' then 0.40
            when 'NOT_INTERESTED' then 0.40 when 'MUTE' then 0.70 when 'BLOCK' then 1.0 when 'REPORT' then 1.0 else 0 end;
        actual_hash:=public.feed_affinity_identity_hash(jsonb_set(to_jsonb(r)-'event_id',
            '{occurred_at}',to_jsonb(extract(epoch from r.occurred_at))));
        if r.action='IMPRESSION' or r.subject_type<>'POST' or owner_id is null
            or (c.viewer_id,c.author_id,c.actor_id,c.post_id,c.occurred_at)
                is distinct from (r.viewer_id,owner_id,r.actor_id,r.subject_id,r.occurred_at) then
            raise exception 'Allocation differs from actual recommendation source'
                using errcode='23514',constraint='feed_affinity_baseline_source'; end if;
    end if;
    if (c.positive_delta,c.negative_delta,c.identity_sha256,c.feature_version)
        is distinct from (expected_positive,expected_negative,actual_hash,'feed-affinity-v1'::text) then
        raise exception 'Allocation source weights/identity differ'
            using errcode='23514',constraint='feed_affinity_baseline_source'; end if;
    if c.namespace='RECOMMENDATION' then
        -- Acceptance owes its affinity resolution in the same transaction.
        -- A raw runtime INSERT cannot bypass the blocked repository path.
        raise exception 'Ordered resolver required before recommendation acceptance'
            using errcode='23514',constraint='feed_affinity_ordered_writer_required';
    end if;
    return null;
end;
$$;
create constraint trigger feed_affinity_verify_allocated_source after insert or update on feed_affinity_source_pairs
    deferrable initially deferred for each row execute function feed_affinity_verify_allocated_source();
revoke all on function feed_affinity_verify_allocated_source() from public;

create function feed_affinity_ordered_metadata_guard() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if tg_op='DELETE' and exists(select 1 from public.post_user_deletion_tombstones
        where user_id in (old.viewer_id,old.author_id)) then return old; end if;
    if tg_table_name='feed_affinity_pair_provenance' then
        if old.origin_mode='HOLD_CAPTURE_ONLY' then return old; end if;
    end if;
    raise exception 'Ordered baseline/provenance cannot be mutated or reinitialized'
        using errcode='23514',constraint='feed_affinity_baseline_authority';
end;
$$;
create trigger feed_affinity_baseline_immutable before update or delete on feed_affinity_pair_baselines
    for each row execute function feed_affinity_ordered_metadata_guard();
create trigger feed_affinity_ordered_origin_delete before delete on feed_affinity_pair_provenance
    for each row execute function feed_affinity_ordered_metadata_guard();
revoke all on function feed_affinity_ordered_metadata_guard() from public;

create function feed_affinity_ordered_scalar_fence() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare viewer uuid; author uuid;
begin
    if tg_op='DELETE' then viewer:=old.viewer_id; author:=old.author_id;
        if exists(select 1 from public.post_user_deletion_tombstones where user_id in(viewer,author)) then return old; end if;
    else viewer:=new.viewer_id; author:=new.author_id; end if;
    if exists(select 1 from public.feed_affinity_test_provisioning) or exists(
        select 1 from public.feed_affinity_pair_provenance where viewer_id=viewer and author_id=author
            and origin_mode='ORDERED_FRESH_BLOCKED') then
        raise exception 'Ordered affinity resolver required; compatibility writes are forbidden'
            using errcode='23514',constraint='feed_affinity_ordered_writer_required';
    end if;
    if tg_op='DELETE' then return old; end if;
    return new;
end;
$$;
create trigger a00_affinity_ordered_fence before insert or update or delete on feed_author_affinities
    for each row execute function feed_affinity_ordered_scalar_fence();
create trigger a00_affinity_ordered_fence before insert or update or delete on feed_author_affinity
    for each row execute function feed_affinity_ordered_scalar_fence();
revoke all on function feed_affinity_ordered_scalar_fence() from public;

create or replace function feed_affinity_observe_source() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if exists(select 1 from public.feed_affinity_pair_provenance where epoch_id=new.epoch_id
        and viewer_id=new.viewer_id and author_id=new.author_id and origin_mode='ORDERED_FRESH_BLOCKED') then
        perform public.feed_affinity_validate_test_authority();
        update public.feed_affinity_pair_provenance origin set
            first_source_sequence=coalesce(origin.first_source_sequence,new.source_sequence),
            first_namespace=case when origin.first_source_sequence is null then new.namespace else origin.first_namespace end,
            first_source_id=case when origin.first_source_sequence is null then new.source_id else origin.first_source_id end,
            first_identity_sha256=case when origin.first_source_sequence is null then new.identity_sha256 else origin.first_identity_sha256 end
        where epoch_id=new.epoch_id and viewer_id=new.viewer_id and author_id=new.author_id;
        return new;
    end if;
    insert into public.feed_affinity_pair_provenance as origin (
        epoch_id,viewer_id,author_id,first_source_sequence,first_namespace,first_source_id,
        first_identity_sha256,capture_prefix_preexisted,scalar_observed,legacy_scalar_observed,legacy_write_observed)
    values(new.epoch_id,new.viewer_id,new.author_id,new.source_sequence,new.namespace,new.source_id,
        new.identity_sha256,new.source_sequence>1,
        exists(select 1 from public.feed_author_affinities where viewer_id=new.viewer_id and author_id=new.author_id),
        exists(select 1 from public.feed_author_affinity where viewer_id=new.viewer_id and author_id=new.author_id),
        new.namespace='OUTBOX' and exists(select 1 from public.feed_projection_inbox where consumer='feed-engagement' and event_id=new.source_id))
    on conflict(epoch_id,viewer_id,author_id) do update set
        first_source_sequence=coalesce(origin.first_source_sequence,excluded.first_source_sequence),
        first_namespace=case when origin.first_source_sequence is null then excluded.first_namespace else origin.first_namespace end,
        first_source_id=case when origin.first_source_sequence is null then excluded.first_source_id else origin.first_source_id end,
        first_identity_sha256=case when origin.first_source_sequence is null then excluded.first_identity_sha256 else origin.first_identity_sha256 end,
        capture_prefix_preexisted=origin.capture_prefix_preexisted or(origin.first_source_sequence is null and excluded.capture_prefix_preexisted),
        scalar_observed=origin.scalar_observed or excluded.scalar_observed,
        legacy_scalar_observed=origin.legacy_scalar_observed or excluded.legacy_scalar_observed,
        legacy_write_observed=origin.legacy_write_observed or excluded.legacy_write_observed;
    return new;
end;
$$;
revoke all on function feed_affinity_observe_source() from public;

-- Controlled runtime grants exclude metadata DML. Erasure is available only
-- through its subject-locked routine, with an actual retirement tombstone.
alter function feed_affinity_forget_subject(uuid) rename to feed_affinity_forget_subject_capture_only;
alter function feed_affinity_forget_subject_capture_only(uuid) security definer;
alter function feed_affinity_forget_subject_capture_only(uuid) set search_path=pg_catalog,public,pg_temp;
revoke all on function feed_affinity_forget_subject_capture_only(uuid) from public;
create function feed_affinity_forget_subject(subject uuid) returns void language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    perform public.post_lock_subject_ids(array[subject]);
    if exists(select 1 from public.feed_affinity_test_provisioning)
        and not exists(select 1 from public.post_user_deletion_tombstones where user_id=subject) then
        raise exception 'Ordered erasure requires actual subject retirement'
            using errcode='23514',constraint='feed_affinity_baseline_authority';
    end if;
    perform public.feed_affinity_forget_subject_capture_only(subject);
end;
$$;
revoke all on function feed_affinity_forget_subject(uuid) from public;

-- A permanent retirement barrier cannot be removed to manufacture freshness.
-- Keep the existing coordinator's INSERT/SELECT and exact duplicate identity.
create function post_guard_terminal_retirement() returns trigger language plpgsql
set search_path=pg_catalog,public,pg_temp as $$
begin
    if tg_op='TRUNCATE' or tg_op='DELETE' then
        raise exception 'Post retirement barriers are terminal'
            using errcode='23514',constraint='post_user_retirement_immutable';
    end if;
    if new is distinct from old then
        raise exception 'Post retirement identity is immutable'
            using errcode='23514',constraint='post_user_retirement_immutable';
    end if;
    return new;
end;
$$;
create trigger post_terminal_retirement before update or delete on post_user_deletion_tombstones
    for each row execute function post_guard_terminal_retirement();
create trigger post_terminal_retirement_truncate before truncate on post_user_deletion_tombstones
    for each statement execute function post_guard_terminal_retirement();

-- Older runtime code may omit the adapter gate. Enforce the same unresolved
-- source barrier independently at the actual receipt/cursor write boundaries.
-- Receipts are committed identities. Rewriting an earlier receipt must never
-- manufacture the receipt needed to advance a different event's cursor.
create function post_guard_projection_receipt_identity() returns trigger language plpgsql
set search_path=pg_catalog,public,pg_temp as $$
begin
    if new is distinct from old then
        raise exception 'Post projection receipt is immutable'
            using errcode='23514',constraint='timeline_projection_receipt_immutable';
    end if;
    return new;
end;
$$;
create trigger post_projection_receipt_identity before update on timeline_projection_receipts
    for each row execute function post_guard_projection_receipt_identity();
revoke all on function post_guard_projection_receipt_identity() from public;

create function feed_affinity_fence_timeline_commit() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if tg_table_name='timeline_projection_receipts' then
        if exists(select 1 from public.feed_affinity_source_contributions c
            left join public.feed_affinity_pair_provenance o using(epoch_id,viewer_id,author_id)
            left join public.feed_affinity_pair_baselines b using(epoch_id,viewer_id,author_id)
            where c.namespace='OUTBOX' and c.source_id=new.event_id
                and (o.origin_mode='ORDERED_FRESH_BLOCKED' or b.epoch_id is not null)) then
            raise exception 'Post receipt requires its ordered affinity resolution'
                using errcode='23514',constraint='feed_affinity_ordered_writer_required';
        end if;
    elsif exists(select 1 from public.feed_affinity_test_provisioning) then
        if tg_op='INSERT' then
            if new.last_version<>0 then
                raise exception 'Controlled post cursor must start at zero'
                    using errcode='23514',constraint='feed_affinity_ordered_writer_required';
            end if;
        elsif new.aggregate_id is distinct from old.aggregate_id
            or new.last_version<>old.last_version+1
            or not exists(select 1 from public.timeline_projection_receipts
                where aggregate_id=new.aggregate_id and aggregate_version=new.last_version) then
            raise exception 'Controlled post cursor requires its exact next receipt'
                using errcode='23514',constraint='feed_affinity_ordered_writer_required';
        end if;
    end if;
    return new;
end;
$$;
create trigger feed_affinity_timeline_receipt_fence before insert on timeline_projection_receipts
    for each row execute function feed_affinity_fence_timeline_commit();
create trigger feed_affinity_timeline_cursor_fence before insert or update on timeline_projection_cursors
    for each row execute function feed_affinity_fence_timeline_commit();
revoke all on function feed_affinity_fence_timeline_commit() from public;
