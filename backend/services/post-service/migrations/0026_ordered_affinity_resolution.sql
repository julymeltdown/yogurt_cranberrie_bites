-- Controlled fresh-test ordered reducer. No production activation/restore
-- authority, HOLD adoption, retention duration or dependency scheduler.
alter table feed_affinity_pair_provenance drop constraint feed_affinity_origin_mode;
alter table feed_affinity_pair_provenance add constraint feed_affinity_origin_mode
    check(origin_mode in ('HOLD_CAPTURE_ONLY','ORDERED_FRESH_BLOCKED','ORDERED_FRESH'));

create table feed_affinity_pair_heads (
    epoch_id uuid not null,
    viewer_id uuid not null,
    author_id uuid not null,
    provisioning_id uuid not null,
    feature_epoch uuid not null,
    writer_generation uuid not null,
    feature_version text not null check(feature_version='feed-affinity-v1'),
    positive_score float8 not null check(positive_score between 0 and 1),
    negative_score float8 not null check(negative_score between 0 and 1),
    last_interacted_at timestamptz null,
    resolved_sequence bigint not null check(resolved_sequence>=0),
    primary key(epoch_id,viewer_id,author_id),
    foreign key(epoch_id,viewer_id,author_id) references feed_affinity_pair_baselines
        on delete cascade
);
create table feed_affinity_resolution_receipts (
    namespace text not null check(namespace in ('OUTBOX','RECOMMENDATION')),
    source_id uuid not null,
    identity_sha256 text not null check(identity_sha256 ~ '^[0-9a-f]{64}$'),
    epoch_id uuid not null,
    viewer_id uuid not null,
    author_id uuid not null,
    actor_id uuid null,
    source_sequence bigint not null check(source_sequence>0),
    post_id uuid not null,
    occurred_at timestamptz not null,
    positive_delta float8 not null check(positive_delta between 0 and 1),
    negative_delta float8 not null check(negative_delta between 0 and 1),
    disposition text not null check(disposition in ('APPLIED','SUPPRESSED')),
    before_state jsonb not null,
    after_state jsonb not null,
    projection_identity_sha256 text null,
    delivery_event_id uuid null,
    delivery_attempt_id uuid null,
    delivery_attempt_number integer null,
    resolved_at timestamptz not null default clock_timestamp(),
    primary key(namespace,source_id),
    unique(epoch_id,viewer_id,author_id,source_sequence),
    foreign key(epoch_id,viewer_id,author_id) references feed_affinity_pair_baselines on delete cascade,
    check((namespace='OUTBOX' and projection_identity_sha256 is not null and projection_identity_sha256 ~ '^[0-9a-f]{64}$' and delivery_event_id is not null and delivery_attempt_id is not null
        and delivery_attempt_number is not null)
        or (namespace='RECOMMENDATION' and projection_identity_sha256 is null and delivery_event_id is null and delivery_attempt_id is null
        and delivery_attempt_number is null))
);
create index feed_affinity_resolution_actor_idx on feed_affinity_resolution_receipts(actor_id) where actor_id is not null;
create table feed_affinity_write_permits (
    transaction_id xid8 not null,
    invocation_id uuid not null,
    namespace text not null,
    source_id uuid not null,
    epoch_id uuid not null,
    viewer_id uuid not null,
    author_id uuid not null,
    source_sequence bigint not null,
    target_table text not null check(target_table in ('feed_author_affinities','feed_affinity_pair_heads')),
    operation text not null check(operation in ('INSERT','UPDATE')),
    expected_old jsonb null,
    expected_new jsonb not null,
    primary key(transaction_id,target_table,epoch_id,viewer_id,author_id)
);
revoke all on feed_affinity_pair_heads,feed_affinity_resolution_receipts,feed_affinity_write_permits from public;

create function feed_affinity_has_write_permit(target text,operation text,prior jsonb,next_row jsonb)
returns boolean language sql security definer set search_path=pg_catalog,public,pg_temp as $$
    select exists(select 1 from public.feed_affinity_write_permits p
        where p.transaction_id=pg_current_xact_id() and p.target_table=$1 and p.operation=$2
            and p.viewer_id=(next_row->>'viewer_id')::uuid and p.author_id=(next_row->>'author_id')::uuid
            and p.expected_old is not distinct from prior and p.expected_new=next_row
            and exists(select 1 from public.feed_affinity_source_contributions c
                where c.namespace=p.namespace and c.source_id=p.source_id
                and (c.epoch_id,c.viewer_id,c.author_id,c.source_sequence)=
                    (p.epoch_id,p.viewer_id,p.author_id,p.source_sequence)));
$$;
revoke all on function feed_affinity_has_write_permit(text,text,jsonb,jsonb) from public;

create function feed_affinity_head_guard() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if tg_op='DELETE' then
        if exists(select 1 from public.post_user_deletion_tombstones where user_id in(old.viewer_id,old.author_id)) then return old; end if;
    elsif tg_op='INSERT' then
        if new.positive_score=0 and new.negative_score=0 and new.last_interacted_at is null and new.resolved_sequence=0
            and exists(select 1 from public.feed_affinity_pair_baselines b
                join public.feed_affinity_pair_provenance o using(epoch_id,viewer_id,author_id)
                where o.origin_mode='ORDERED_FRESH' and to_jsonb(b)-'initialized_at'=to_jsonb(new)) then return new; end if;
    elsif public.feed_affinity_has_write_permit(tg_table_name,tg_op,to_jsonb(old),to_jsonb(new)) then return new;
    end if;
    raise exception 'Pair head requires exact resolver authority'
        using errcode='23514',constraint='feed_affinity_ordered_writer_required';
end;
$$;
create trigger feed_affinity_head_guard before insert or update or delete on feed_affinity_pair_heads
    for each row execute function feed_affinity_head_guard();
revoke all on function feed_affinity_head_guard() from public;

create function feed_affinity_resolution_immutable() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if tg_op='DELETE' and exists(select 1 from public.post_user_deletion_tombstones
        where user_id in(old.viewer_id,old.author_id,old.actor_id)) then return old; end if;
    raise exception 'Affinity resolution receipt is immutable'
        using errcode='23514',constraint='feed_affinity_resolution_immutable';
end;
$$;
create trigger feed_affinity_resolution_immutable before update or delete on feed_affinity_resolution_receipts
    for each row execute function feed_affinity_resolution_immutable();
revoke all on function feed_affinity_resolution_immutable() from public;

create function feed_affinity_permit_drained() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if exists(select 1 from public.feed_affinity_write_permits where transaction_id=new.transaction_id) then
        raise exception 'Resolver left temporary authority behind' using errcode='23514',constraint='feed_affinity_permit_drained';
    end if;
    return null;
end;
$$;
create constraint trigger feed_affinity_permit_drained after insert on feed_affinity_write_permits
    deferrable initially deferred for each row execute function feed_affinity_permit_drained();
revoke all on function feed_affinity_permit_drained() from public;

create or replace function feed_affinity_pair_baseline_guard() returns trigger language plpgsql
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
                    and o.origin_mode in ('ORDERED_FRESH_BLOCKED','ORDERED_FRESH') and b.provisioning_id=authority.provisioning_id
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
                and o.origin_mode in ('ORDERED_FRESH_BLOCKED','ORDERED_FRESH') and b.provisioning_id=authority.provisioning_id
                and b.writer_generation=authority.writer_generation and b.feature_epoch=authority.feature_epoch) then
        raise exception 'Invalid ordered pair allocation'
            using errcode='23514',constraint='feed_affinity_baseline_authority';
    end if;
    return new;
end;
$$;

create or replace function feed_affinity_initialize_baseline() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare authority public.feed_affinity_test_provisioning%rowtype;
begin
    if not exists(select 1 from public.feed_affinity_test_provisioning) then return new; end if;
    authority:=public.feed_affinity_validate_test_authority();
    insert into public.feed_affinity_pair_provenance(epoch_id,viewer_id,author_id,origin_mode)
        values(new.epoch_id,new.viewer_id,new.author_id,case when exists(select 1 from public._sqlx_migrations where version=26 and success) then 'ORDERED_FRESH' else 'ORDERED_FRESH_BLOCKED' end);
    insert into public.feed_affinity_pair_baselines(epoch_id,viewer_id,author_id,provisioning_id,
        feature_epoch,writer_generation,feature_version)
        values(new.epoch_id,new.viewer_id,new.author_id,authority.provisioning_id,
            authority.feature_epoch,authority.writer_generation,authority.feature_version);
    if exists(select 1 from public.feed_affinity_pair_provenance where epoch_id=new.epoch_id
        and viewer_id=new.viewer_id and author_id=new.author_id and origin_mode='ORDERED_FRESH') then
        insert into public.feed_affinity_pair_heads select epoch_id,viewer_id,author_id,provisioning_id,
            feature_epoch,writer_generation,feature_version,positive_score,negative_score,last_interacted_at,resolved_sequence
            from public.feed_affinity_pair_baselines where epoch_id=new.epoch_id and viewer_id=new.viewer_id and author_id=new.author_id;
    end if;
    return new;
end;
$$;

create or replace function feed_affinity_observe_source() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if exists(select 1 from public.feed_affinity_pair_provenance where epoch_id=new.epoch_id
        and viewer_id=new.viewer_id and author_id=new.author_id and origin_mode in ('ORDERED_FRESH_BLOCKED','ORDERED_FRESH')) then
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

create or replace function feed_affinity_observe_legacy_write() returns trigger language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
declare capture_epoch uuid;
begin
    if public.feed_affinity_has_write_permit(tg_table_name,tg_op,
        case when tg_op='UPDATE' then to_jsonb(old) else null end,to_jsonb(new)) then return new; end if;
    select epoch_id into strict capture_epoch from feed_affinity_capture_epoch where singleton;
    insert into feed_affinity_pair_provenance as origin (
        epoch_id,viewer_id,author_id,scalar_observed,legacy_scalar_observed,legacy_write_observed)
    values (capture_epoch,new.viewer_id,new.author_id,
        tg_table_name='feed_author_affinities',tg_table_name='feed_author_affinity',true)
    on conflict (epoch_id,viewer_id,author_id) do update set
        scalar_observed=origin.scalar_observed or excluded.scalar_observed,
        legacy_scalar_observed=origin.legacy_scalar_observed or excluded.legacy_scalar_observed,
        legacy_write_observed=true;
    return new;
end;
$$;
revoke all on function feed_affinity_observe_legacy_write() from public;

create function feed_affinity_validate_resolution_source(c public.feed_affinity_source_contributions)
returns void language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
declare
    e public.post_event_outbox%rowtype;
    r public.feed_recommendation_events%rowtype;
    owner_id uuid; expected_positive float8; expected_negative float8;
    actual_hash text; delta float8;
begin
    if c.namespace='OUTBOX' then
        select * into strict e from public.post_event_outbox where event_id=c.source_id;
        select author_id into owner_id from public.posts where id=e.aggregate_id;
        -- Physical Post retention must not invent an owner. Use the positive
        -- retained owner creation source plus its actual projection receipt.
        if owner_id is null and exists(select 1 from public.post_event_outbox genesis
            join public.timeline_projection_receipts receipt on receipt.event_id=genesis.event_id
            where genesis.aggregate_id=e.aggregate_id and genesis.aggregate_version=1 and genesis.event_type='POST_CREATED'
                and genesis.actor_id=c.author_id and (genesis.payload::jsonb->>'authorId')::uuid=c.author_id
                and (genesis.payload::jsonb->>'postId')::uuid=e.aggregate_id
                and receipt.aggregate_id=e.aggregate_id and receipt.aggregate_version=1
                and exists(select 1 from public.feed_affinity_pair_provenance
                    where (epoch_id,viewer_id,author_id)=(c.epoch_id,c.viewer_id,c.author_id) and origin_mode='ORDERED_FRESH')) then
            owner_id:=c.author_id;
        end if;
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
end;
$$;
revoke all on function feed_affinity_validate_resolution_source(public.feed_affinity_source_contributions) from public;

create or replace function feed_affinity_verify_allocated_source() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare c public.feed_affinity_source_contributions%rowtype;
begin
    select * into c from public.feed_affinity_source_contributions
        where (epoch_id,viewer_id,author_id,source_sequence)=(new.epoch_id,new.viewer_id,new.author_id,new.allocated_sequence);
    if not found then
        if exists(select 1 from public.post_user_deletion_tombstones where user_id in(new.viewer_id,new.author_id)) then return null; end if;
        raise exception 'Allocated source disappeared' using errcode='23514',constraint='feed_affinity_baseline_source';
    end if;
    if not exists(select 1 from public.feed_affinity_pair_baselines
        where (epoch_id,viewer_id,author_id)=(new.epoch_id,new.viewer_id,new.author_id)) then
        if exists(select 1 from public.feed_affinity_test_provisioning) then
            raise exception 'Qualified allocation lost baseline' using errcode='23514',constraint='feed_affinity_baseline_source';
        end if;
        return null;
    end if;
    perform public.feed_affinity_validate_resolution_source(c);
    if c.namespace='RECOMMENDATION' and not exists(select 1 from public.feed_affinity_resolution_receipts r
        where r.namespace=c.namespace and r.source_id=c.source_id and r.identity_sha256=c.identity_sha256
            and (r.epoch_id,r.viewer_id,r.author_id,r.source_sequence)=(c.epoch_id,c.viewer_id,c.author_id,c.source_sequence)) then
        raise exception 'Recommendation requires its exact resolution' using errcode='23514',constraint='feed_affinity_ordered_writer_required';
    end if;
    return null;
end;
$$;

create or replace function feed_affinity_source_subject_lock() returns trigger
language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
declare
    owner_id uuid;
    impression_author uuid;
    locked_owner uuid;
    locked_impression uuid;
begin
    if tg_table_name='post_event_outbox' then
        if new.event_type not in ('POST_LIKED','POST_UNLIKED','POST_COMMENTED','POST_COMMENT_DELETED') then
            return new;
        end if;
        select author_id into owner_id from posts where id=new.aggregate_id;
        perform public.post_assert_subjects_not_deleted(array[new.actor_id,owner_id]);
        select author_id into locked_owner from public.posts where id=new.aggregate_id for update;
        if locked_owner is distinct from owner_id then raise exception 'Source owner changed before allocation'; end if;
    else
        if new.action='IMPRESSION' then return new; end if;
        select author_id into owner_id from posts where id=new.subject_id;
        select author_id into impression_author from feed_recommendation_impressions
            where impression_id=new.impression_id and viewer_id=new.viewer_id and post_id=new.subject_id;
        perform public.post_assert_subjects_not_deleted(array[new.viewer_id,new.actor_id,owner_id,impression_author]);
        select author_id into locked_owner from public.posts where id=new.subject_id for update;
        select author_id into locked_impression from public.feed_recommendation_impressions
            where impression_id=new.impression_id and viewer_id=new.viewer_id and post_id=new.subject_id for share;
        if locked_owner is distinct from owner_id or locked_impression is distinct from impression_author then
            raise exception 'Source subjects changed before allocation'; end if;
    end if;
    return new;
end;
$$;
revoke all on function feed_affinity_source_subject_lock() from public;

create or replace function feed_affinity_ordered_scalar_fence() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare viewer uuid; author uuid;
begin
    if tg_op='DELETE' then viewer:=old.viewer_id;author:=old.author_id;
        if exists(select 1 from public.post_user_deletion_tombstones where user_id in(viewer,author)) then return old; end if;
    else viewer:=new.viewer_id;author:=new.author_id; end if;
    if exists(select 1 from public.feed_affinity_test_provisioning) or exists(
        select 1 from public.feed_affinity_pair_provenance where viewer_id=viewer and author_id=author
            and origin_mode in ('ORDERED_FRESH_BLOCKED','ORDERED_FRESH')) then
        if tg_table_name='feed_author_affinities' and tg_op<>'DELETE' and public.feed_affinity_has_write_permit(
            tg_table_name,tg_op,case when tg_op='UPDATE' then to_jsonb(old) else null end,to_jsonb(new)) then return new; end if;
        raise exception 'Affinity scalar requires exact resolver permit'
            using errcode='23514',constraint='feed_affinity_ordered_writer_required';
    end if;
    if tg_op='DELETE' then return old; end if;
    return new;
end;
$$;

-- Single-source entry point. Never walks or consumes a different predecessor.
-- Explicit execute grant only to the dedicated controlled-test runtime.
create function feed_affinity_resolve_ordered(source_namespace text,effect_source uuid,
    claim_event uuid,claim_token uuid,claim_attempt uuid,claim_number integer,projection_hash text)
returns boolean language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
declare c public.feed_affinity_source_contributions%rowtype;
    a public.feed_affinity_test_provisioning%rowtype;
    h public.feed_affinity_pair_heads%rowtype; next_head public.feed_affinity_pair_heads%rowtype;
    s public.feed_author_affinities%rowtype; next_scalar public.feed_author_affinities%rowtype;
    r public.feed_affinity_resolution_receipts%rowtype;
    e public.post_event_outbox%rowtype; claim_source public.post_event_outbox%rowtype;
    owner_before uuid; owner_locked uuid; post_deleted boolean; suppressed boolean;
    scalar_exists boolean; last_post bigint; invocation uuid:=gen_random_uuid(); rows_changed bigint;
    result_row jsonb;
begin
    if source_namespace is null or source_namespace not in ('OUTBOX','RECOMMENDATION') then
        raise exception 'Unsupported ordered source' using errcode='23514',constraint='feed_affinity_baseline_source'; end if;
    select * into c from public.feed_affinity_source_contributions where namespace=source_namespace and source_id=effect_source;
    if not found then return false; end if;
    if not exists(select 1 from public.feed_affinity_pair_provenance
        where (epoch_id,viewer_id,author_id)=(c.epoch_id,c.viewer_id,c.author_id) and origin_mode='ORDERED_FRESH') then return false; end if;
    -- Discover complete subjects before any locking. No predecessor traversal.
    select author_id into owner_before from public.posts where id=c.post_id;
    perform public.post_lock_subject_ids(array[c.viewer_id,c.author_id,c.actor_id,owner_before]);
    select author_id,deleted_at is not null into owner_locked,post_deleted from public.posts where id=c.post_id for update;
    if owner_before is distinct from owner_locked then
        raise exception 'Resolver owner changed during locking' using errcode='23514',constraint='feed_affinity_baseline_source'; end if;
    suppressed:=(c.namespace='OUTBOX' and coalesce(post_deleted,true)) or exists(select 1 from public.post_user_deletion_tombstones
        where user_id in(c.viewer_id,c.author_id,c.actor_id));
    a:=public.feed_affinity_validate_test_authority();
    perform public.feed_affinity_validate_resolution_source(c);
    select * into r from public.feed_affinity_resolution_receipts where namespace=c.namespace and source_id=c.source_id;
    if found then
        if (r.identity_sha256,r.epoch_id,r.viewer_id,r.author_id,r.source_sequence)
            is distinct from (c.identity_sha256,c.epoch_id,c.viewer_id,c.author_id,c.source_sequence) then
            raise exception 'Conflicting resolution identity' using errcode='23514',constraint='feed_affinity_resolution_immutable'; end if;
        return true;
    end if;
    if c.namespace='OUTBOX' then
        select * into strict e from public.post_event_outbox where event_id=c.source_id for share;
        select * into claim_source from public.post_event_outbox where event_id=claim_event for share;
        if not found or claim_source.aggregate_id<>c.post_id or claim_source.aggregate_version is null
            or e.aggregate_version is null or claim_source.aggregate_version<e.aggregate_version
            or claim_token is null or claim_attempt is null or claim_number is null
            or projection_hash is null or projection_hash !~ '^[0-9a-f]{64}$' then
            raise exception 'Resolver needs owning Post claim' using errcode='23514',constraint='feed_affinity_resolution_authority'; end if;
        perform 1 from public.post_event_delivery_jobs where event_id=claim_event and target='TIMELINE'
            and state='LEASED' and lease_token=claim_token and attempt_id=claim_attempt and attempt_count=claim_number
            and lease_expires_at>clock_timestamp() for update;
        if not found or not exists(select 1 from public.post_event_delivery_attempts
            where event_id=claim_event and attempt_id=claim_attempt and target='TIMELINE' and phase='STARTED') then
            raise exception 'Resolver delivery authority lost' using errcode='23514',constraint='feed_affinity_resolution_authority'; end if;
        select last_version into last_post from public.timeline_projection_cursors where aggregate_id=c.post_id for update;
        if last_post is null or e.aggregate_version<>last_post+1
            or exists(select 1 from public.post_event_outbox where aggregate_id=c.post_id and aggregate_version is null)
            or not exists(select 1 from public.timeline_projection_receipts where aggregate_id=c.post_id and aggregate_version=1) then
            raise exception 'Resolver Post prefix unavailable' using errcode='23514',constraint='feed_affinity_resolution_authority'; end if;
    elsif claim_event is not null or claim_token is not null or claim_attempt is not null or claim_number is not null or projection_hash is not null then
        raise exception 'Recommendation cannot use outbox authority' using errcode='23514',constraint='feed_affinity_resolution_authority';
    end if;
    select * into h from public.feed_affinity_pair_heads where (epoch_id,viewer_id,author_id)=(c.epoch_id,c.viewer_id,c.author_id) for update;
    if not found or (h.provisioning_id,h.feature_epoch,h.writer_generation,h.feature_version)
        is distinct from (a.provisioning_id,a.feature_epoch,a.writer_generation,a.feature_version) then
        raise exception 'Resolver head/baseline authority unavailable' using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
    if c.source_sequence<>h.resolved_sequence+1 then
        raise exception 'Ordered predecessor unresolved' using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
    select * into s from public.feed_author_affinities where viewer_id=c.viewer_id and author_id=c.author_id for update;
    scalar_exists:=found;
    if (scalar_exists is distinct from (h.last_interacted_at is not null)) or (scalar_exists and
        (s.affinity_score,s.negative_score,s.last_interacted_at,s.feature_version)
            is distinct from (h.positive_score,h.negative_score,h.last_interacted_at,h.feature_version)) then
        raise exception 'Serving feature differs from ordered head' using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
    next_head:=h;next_head.resolved_sequence:=c.source_sequence;
    if not suppressed then
        next_head.positive_score:=greatest(0,least(1,h.positive_score*0.95+c.positive_delta));
        next_head.negative_score:=greatest(0,least(1,h.negative_score*0.95+c.negative_delta));
        next_head.last_interacted_at:=greatest(h.last_interacted_at,c.occurred_at);
        next_scalar.viewer_id:=c.viewer_id;next_scalar.author_id:=c.author_id;
        next_scalar.affinity_score:=next_head.positive_score;next_scalar.negative_score:=next_head.negative_score;
        next_scalar.last_interacted_at:=next_head.last_interacted_at;next_scalar.feature_version:=h.feature_version;
        insert into public.feed_affinity_write_permits values(pg_current_xact_id(),invocation,c.namespace,c.source_id,
            c.epoch_id,c.viewer_id,c.author_id,c.source_sequence,'feed_author_affinities',
            case when scalar_exists then 'UPDATE' else 'INSERT' end,
            case when scalar_exists then to_jsonb(s) else null end,to_jsonb(next_scalar));
        if scalar_exists then
            update public.feed_author_affinities set affinity_score=next_scalar.affinity_score,
                negative_score=next_scalar.negative_score,last_interacted_at=next_scalar.last_interacted_at,
                feature_version=next_scalar.feature_version where viewer_id=c.viewer_id and author_id=c.author_id
                returning to_jsonb(feed_author_affinities) into result_row;
        else
            insert into public.feed_author_affinities values(next_scalar.viewer_id,next_scalar.author_id,next_scalar.affinity_score,next_scalar.negative_score,next_scalar.last_interacted_at,next_scalar.feature_version) returning to_jsonb(feed_author_affinities) into result_row;
        end if;
        get diagnostics rows_changed=row_count;
        if rows_changed<>1 or result_row is distinct from to_jsonb(next_scalar) then
            raise exception 'Resolver scalar mutation skipped or altered' using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
    end if;
    insert into public.feed_affinity_write_permits values(pg_current_xact_id(),invocation,c.namespace,c.source_id,
        c.epoch_id,c.viewer_id,c.author_id,c.source_sequence,'feed_affinity_pair_heads','UPDATE',to_jsonb(h),to_jsonb(next_head));
    update public.feed_affinity_pair_heads set positive_score=next_head.positive_score,negative_score=next_head.negative_score,
        last_interacted_at=next_head.last_interacted_at,resolved_sequence=next_head.resolved_sequence
        where (epoch_id,viewer_id,author_id)=(c.epoch_id,c.viewer_id,c.author_id)
        returning to_jsonb(feed_affinity_pair_heads) into result_row;
    get diagnostics rows_changed=row_count;
    if rows_changed<>1 or result_row is distinct from to_jsonb(next_head) then
        raise exception 'Resolver head mutation skipped or altered' using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
    insert into public.feed_affinity_resolution_receipts(namespace,source_id,identity_sha256,epoch_id,viewer_id,author_id,actor_id,
        source_sequence,post_id,occurred_at,positive_delta,negative_delta,disposition,before_state,after_state,
        projection_identity_sha256,delivery_event_id,delivery_attempt_id,delivery_attempt_number)
    values(c.namespace,c.source_id,c.identity_sha256,c.epoch_id,c.viewer_id,c.author_id,c.actor_id,
        c.source_sequence,c.post_id,c.occurred_at,c.positive_delta,c.negative_delta,
        case when suppressed then 'SUPPRESSED' else 'APPLIED' end,to_jsonb(h),to_jsonb(next_head),projection_hash,claim_event,claim_attempt,claim_number);
    get diagnostics rows_changed=row_count;
    if rows_changed<>1 then raise exception 'Resolver receipt skipped' using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
    delete from public.feed_affinity_write_permits where transaction_id=pg_current_xact_id() and invocation_id=invocation;
    get diagnostics rows_changed=row_count;
    if rows_changed<>(case when suppressed then 1 else 2 end) then
        raise exception 'Resolver permit drain failed' using errcode='23514',constraint='feed_affinity_permit_drained'; end if;
    return true;
end;
$$;
revoke all on function feed_affinity_resolve_ordered(text,uuid,uuid,uuid,uuid,integer,text) from public;

create function feed_affinity_verify_resolution() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare c public.feed_affinity_source_contributions%rowtype; h public.feed_affinity_pair_heads%rowtype;
    expected public.feed_affinity_pair_heads%rowtype; previous public.feed_affinity_pair_heads%rowtype;
begin
    if not exists(select 1 from public.feed_affinity_resolution_receipts where namespace=new.namespace and source_id=new.source_id) then
        if exists(select 1 from public.post_user_deletion_tombstones where user_id in(new.viewer_id,new.author_id,new.actor_id)) then return null; end if;
        raise exception 'Resolution disappeared' using errcode='23514',constraint='feed_affinity_resolution_receipt'; end if;
    perform public.feed_affinity_validate_test_authority();
    select * into c from public.feed_affinity_source_contributions where namespace=new.namespace and source_id=new.source_id;
    if not found then raise exception 'Resolution source missing' using errcode='23514',constraint='feed_affinity_resolution_receipt'; end if;
    perform public.feed_affinity_validate_resolution_source(c);
    if (new.identity_sha256,new.epoch_id,new.viewer_id,new.author_id,new.actor_id,new.source_sequence,new.post_id,new.occurred_at,new.positive_delta,new.negative_delta)
        is distinct from (c.identity_sha256,c.epoch_id,c.viewer_id,c.author_id,c.actor_id,c.source_sequence,c.post_id,c.occurred_at,c.positive_delta,c.negative_delta) then
        raise exception 'Resolution source differs' using errcode='23514',constraint='feed_affinity_resolution_receipt'; end if;
    previous:=jsonb_populate_record(null::public.feed_affinity_pair_heads,new.before_state);
    expected:=previous;expected.resolved_sequence:=previous.resolved_sequence+1;
    if expected.resolved_sequence<>c.source_sequence then raise exception 'Resolution ordinal differs' using errcode='23514',constraint='feed_affinity_resolution_receipt'; end if;
    if new.disposition='APPLIED' then
        expected.positive_score:=greatest(0,least(1,previous.positive_score*0.95+c.positive_delta));
        expected.negative_score:=greatest(0,least(1,previous.negative_score*0.95+c.negative_delta));
        expected.last_interacted_at:=greatest(previous.last_interacted_at,c.occurred_at);
    end if;
    if to_jsonb(expected)<>new.after_state then raise exception 'Resolution effect differs' using errcode='23514',constraint='feed_affinity_resolution_receipt'; end if;
    select * into h from public.feed_affinity_pair_heads where (epoch_id,viewer_id,author_id)=(c.epoch_id,c.viewer_id,c.author_id);
    if not found or h.resolved_sequence<c.source_sequence or
        (h.resolved_sequence=c.source_sequence and to_jsonb(h)<>new.after_state) or
        (h.resolved_sequence>c.source_sequence and not exists(select 1 from public.feed_affinity_resolution_receipts
            where (epoch_id,viewer_id,author_id,source_sequence)=(c.epoch_id,c.viewer_id,c.author_id,c.source_sequence+1)
                and before_state=new.after_state)) then
        raise exception 'Resolution head/chain differs' using errcode='23514',constraint='feed_affinity_resolution_receipt'; end if;
    if c.namespace='OUTBOX' then
        if not exists(select 1 from public.timeline_projection_receipts p join public.post_event_outbox e on e.event_id=p.event_id
            where p.event_id=c.source_id and p.aggregate_id=c.post_id and p.aggregate_version=e.aggregate_version
                and p.identity_sha256=new.projection_identity_sha256
                and p.disposition=new.disposition and p.delivery_event_id=new.delivery_event_id and p.delivery_attempt_id=new.delivery_attempt_id)
            or not exists(select 1 from public.post_event_delivery_jobs where event_id=new.delivery_event_id and target='TIMELINE'
                and state='LEASED' and attempt_id=new.delivery_attempt_id and attempt_count=new.delivery_attempt_number
                and lease_expires_at>clock_timestamp()) then
            raise exception 'Resolution lacks live owning Post receipt' using errcode='23514',constraint='feed_affinity_resolution_authority'; end if;
    end if;
    return null;
end;
$$;
create constraint trigger feed_affinity_verify_resolution after insert on feed_affinity_resolution_receipts
    deferrable initially deferred for each row execute function feed_affinity_verify_resolution();
revoke all on function feed_affinity_verify_resolution() from public;

create or replace function feed_affinity_fence_timeline_commit() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if tg_table_name='timeline_projection_receipts' then
        if exists(select 1 from public.feed_affinity_source_contributions c
            left join public.feed_affinity_pair_provenance o using(epoch_id,viewer_id,author_id)
            left join public.feed_affinity_pair_baselines b using(epoch_id,viewer_id,author_id)
            where c.namespace='OUTBOX' and c.source_id=new.event_id
                and (o.origin_mode='ORDERED_FRESH_BLOCKED' or (b.epoch_id is not null and not exists(
                    select 1 from public.feed_affinity_resolution_receipts r where r.namespace=c.namespace and r.source_id=c.source_id
                        and r.identity_sha256=c.identity_sha256 and r.source_sequence=c.source_sequence
                        and r.delivery_event_id=new.delivery_event_id and r.delivery_attempt_id=new.delivery_attempt_id
                        and r.disposition=new.disposition)))) then
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

-- Personal resolution links disappear with actual terminal actor retirement;
-- evolving scores on unrelated pairs are not reversed or reset.
create or replace function feed_affinity_forget_subject(subject uuid) returns void language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    perform public.post_lock_subject_ids(array[subject]);
    if exists(select 1 from public.feed_affinity_test_provisioning) and not exists(
        select 1 from public.post_user_deletion_tombstones where user_id=subject) then
        raise exception 'Ordered erasure requires terminal retirement' using errcode='23514',constraint='feed_affinity_baseline_authority'; end if;
    delete from public.feed_affinity_resolution_receipts where actor_id=subject or viewer_id=subject or author_id=subject;
    perform public.feed_affinity_forget_subject_capture_only(subject);
end;
$$;
