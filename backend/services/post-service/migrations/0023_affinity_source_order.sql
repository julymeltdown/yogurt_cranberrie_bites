-- Capture new owner facts in a shared source order. This is a capture epoch,
-- NOT a feature baseline or authority to adopt existing scores/history. The
-- current feature reducers are unchanged; consumer/claim gating is a separate
-- required implementation step. No retention duration or historical reset.
create table feed_affinity_capture_epoch (
    singleton boolean primary key default true check (singleton),
    epoch_id uuid not null unique default gen_random_uuid(),
    installed_at timestamptz not null default clock_timestamp()
);
insert into feed_affinity_capture_epoch(singleton) values (true);
create table feed_affinity_source_pairs (
    epoch_id uuid not null references feed_affinity_capture_epoch(epoch_id),
    viewer_id uuid not null,
    author_id uuid not null,
    allocated_sequence bigint not null check (allocated_sequence > 0),
    primary key (epoch_id, viewer_id, author_id)
);
create index feed_affinity_pair_viewer_idx on feed_affinity_source_pairs(viewer_id);
create index feed_affinity_pair_author_idx on feed_affinity_source_pairs(author_id);
create table feed_affinity_source_contributions (
    namespace text not null check (namespace in ('OUTBOX','RECOMMENDATION')),
    source_id uuid not null,
    epoch_id uuid not null,
    viewer_id uuid not null,
    author_id uuid not null,
    source_sequence bigint not null check (source_sequence > 0),
    actor_id uuid null,
    post_id uuid not null,
    occurred_at timestamptz not null,
    positive_delta float8 not null check (positive_delta between 0 and 1),
    negative_delta float8 not null check (negative_delta between 0 and 1),
    feature_version text not null check (feature_version='feed-affinity-v1'),
    identity_sha256 text not null check (identity_sha256 ~ '^[0-9a-f]{64}$'),
    captured_at timestamptz not null default clock_timestamp(),
    primary key (namespace,source_id),
    foreign key (epoch_id,viewer_id,author_id)
        references feed_affinity_source_pairs(epoch_id,viewer_id,author_id),
    unique (epoch_id,viewer_id,author_id,source_sequence)
);
create index feed_affinity_source_viewer_idx on feed_affinity_source_contributions(viewer_id);
create index feed_affinity_source_author_idx on feed_affinity_source_contributions(author_id);
create index feed_affinity_source_actor_idx on feed_affinity_source_contributions(actor_id) where actor_id is not null;

-- JSONB equality ignores numeric scale; JSONB text serialization does not.
create function feed_affinity_normalize_identity(value jsonb) returns jsonb
language plpgsql immutable strict as $$
declare normalized jsonb;
begin
    case jsonb_typeof(value)
        when 'object' then
            select coalesce(jsonb_object_agg(key,feed_affinity_normalize_identity(item)),'{}'::jsonb)
                into normalized from jsonb_each(value) as entries(key,item);
        when 'array' then
            select coalesce(jsonb_agg(feed_affinity_normalize_identity(item) order by position),'[]'::jsonb)
                into normalized from jsonb_array_elements(value) with ordinality as entries(item,position);
        when 'number' then normalized:=to_jsonb(trim_scale((value#>>'{}')::numeric));
        else normalized:=value;
    end case;
    return normalized;
end;
$$;
create function feed_affinity_identity_hash(value jsonb) returns text
language sql immutable strict as $$
    select encode(sha256(convert_to(feed_affinity_normalize_identity(value)::text,'UTF8')),'hex');
$$;

create function feed_affinity_capture_source(
    source_namespace text, source_id uuid, viewer uuid, author uuid, source_actor uuid,
    post uuid, source_time timestamptz, positive float8, negative float8,
    identity_hash text
) returns void language plpgsql as $$
declare
    existing feed_affinity_source_contributions%rowtype;
    epoch uuid;
    sequence bigint;
begin
    -- BEFORE-source triggers acquire the COMPLETE subject set before source
    -- insertion. Rechecking here preserves account retirement fencing.
    perform post_assert_subjects_not_deleted(array[viewer,author,source_actor]);
    select * into existing from feed_affinity_source_contributions c
        where c.namespace=source_namespace and c.source_id=feed_affinity_capture_source.source_id;
    if found then
        if (existing.viewer_id,existing.author_id,existing.actor_id,existing.post_id,
            existing.occurred_at,existing.positive_delta,existing.negative_delta,
            existing.identity_sha256)
            is distinct from (viewer,author,source_actor,post,source_time,positive,negative,identity_hash) then
            raise exception 'Conflicting affinity source identity'
                using errcode='23514', constraint='feed_affinity_source_identity';
        end if;
        return;
    end if;
    select epoch_id into strict epoch from feed_affinity_capture_epoch where singleton;
    insert into feed_affinity_source_pairs(epoch_id,viewer_id,author_id,allocated_sequence)
        values (epoch,viewer,author,1)
        on conflict (epoch_id,viewer_id,author_id) do update
            set allocated_sequence=feed_affinity_source_pairs.allocated_sequence+1
        returning allocated_sequence into sequence;
    insert into feed_affinity_source_contributions(
        namespace,source_id,epoch_id,viewer_id,author_id,source_sequence,actor_id,post_id,
        occurred_at,positive_delta,negative_delta,feature_version,identity_sha256)
        values (source_namespace,source_id,epoch,viewer,author,sequence,source_actor,post,
                source_time,positive,negative,'feed-affinity-v1',identity_hash);
end;
$$;

create function feed_affinity_source_subject_lock() returns trigger
language plpgsql as $$
declare
    owner_id uuid;
    impression_author uuid;
begin
    if tg_table_name='post_event_outbox' then
        if new.event_type not in ('POST_LIKED','POST_UNLIKED','POST_COMMENTED','POST_COMMENT_DELETED') then
            return new;
        end if;
        select author_id into owner_id from posts where id=new.aggregate_id;
        perform post_assert_subjects_not_deleted(array[new.actor_id,owner_id]);
    else
        if new.action='IMPRESSION' then return new; end if;
        select author_id into owner_id from posts where id=new.subject_id;
        select author_id into impression_author from feed_recommendation_impressions
            where impression_id=new.impression_id and viewer_id=new.viewer_id and post_id=new.subject_id;
        perform post_assert_subjects_not_deleted(array[new.viewer_id,new.actor_id,owner_id,impression_author]);
    end if;
    return new;
end;
$$;
-- Alphabetically before the existing actor-only source fence. Supported owner
-- writers already acquire complete subjects before their post mutations.
create trigger a00_affinity_outbox_subjects before insert on post_event_outbox
    for each row execute function feed_affinity_source_subject_lock();
create trigger a00_affinity_recommendation_subjects before insert on feed_recommendation_events
    for each row execute function feed_affinity_source_subject_lock();

create function feed_affinity_capture_outbox() returns trigger language plpgsql as $$
declare
    owner_id uuid;
    body jsonb;
    delta float8;
    hash text;
begin
    if new.event_type not in ('POST_LIKED','POST_UNLIKED','POST_COMMENTED','POST_COMMENT_DELETED') then
        return new;
    end if;
    select author_id into owner_id from posts where id=new.aggregate_id;
    body:=new.payload::jsonb;
    if owner_id is null or new.actor_id is null or
       (body->>'postId')::uuid is distinct from new.aggregate_id or
       (body ? 'actorId' and (body->>'actorId')::uuid is distinct from new.actor_id) then
        raise exception 'Unproven affinity outbox source' using errcode='23514',constraint='feed_affinity_source_binding';
    end if;
    if new.event_type='POST_COMMENTED' then
        if (body->>'authorId')::uuid is distinct from new.actor_id or
           (body->>'notifyUserId')::uuid is distinct from owner_id then
            raise exception 'Conflicting affinity comment owner' using errcode='23514',constraint='feed_affinity_source_binding';
        end if;
        delta:=0.16;
    elsif new.event_type='POST_COMMENT_DELETED' then
        if (body->>'authorId')::uuid is distinct from new.actor_id or
           (body->>'postAuthorId')::uuid is distinct from owner_id then
            raise exception 'Conflicting affinity deleted-comment owner' using errcode='23514',constraint='feed_affinity_source_binding';
        end if;
        delta:=-0.16;
    elsif new.event_type='POST_UNLIKED' and not(body ? 'actorId') and not(body ? 'notifyUserId') then
        -- Actual existing unlike producer stores authorId=actor.
        if (body->>'authorId')::uuid is distinct from new.actor_id then
            raise exception 'Conflicting affinity unlike actor' using errcode='23514',constraint='feed_affinity_source_binding';
        end if;
        delta:=-0.08;
    else
        if (body->>'authorId')::uuid is distinct from owner_id then
            raise exception 'Conflicting affinity engagement owner' using errcode='23514',constraint='feed_affinity_source_binding';
        end if;
        delta:=case when new.event_type='POST_LIKED' then 0.08 else -0.08 end;
    end if;
    if body ? 'notifyUserId' and (body->>'notifyUserId')::uuid is distinct from owner_id then
        raise exception 'Conflicting affinity notification owner' using errcode='23514',constraint='feed_affinity_source_binding';
    end if;
    if new.actor_id=owner_id then return new; end if;
    hash:=feed_affinity_identity_hash(jsonb_build_object(
        'eventId',new.event_id,'eventType',new.event_type,'aggregateId',new.aggregate_id,
        'aggregateVersion',new.aggregate_version,'actorId',new.actor_id,
        'attributes',new.attributes,'payload',new.payload,
        'occurredAt',extract(epoch from new.occurred_at),
        'createdAt',extract(epoch from new.created_at)));
    perform feed_affinity_capture_source('OUTBOX',new.event_id,new.actor_id,owner_id,new.actor_id,
        new.aggregate_id,new.occurred_at,greatest(delta,0),greatest(-delta,0),hash);
    return new;
end;
$$;
-- AFTER actual insertion: skipped ON CONFLICT writes allocate nothing. Counter
-- updates use transactional rows, so source/effect failures leave no holes.
create trigger feed_affinity_capture_outbox after insert on post_event_outbox
    for each row execute function feed_affinity_capture_outbox();

create function feed_affinity_capture_recommendation() returns trigger language plpgsql as $$
declare
    owner_id uuid;
    positive float8:=0;
    negative float8:=0;
    hash text;
begin
    -- The read path's automatic source fact must not cause an extra decay.
    if new.action='IMPRESSION' then return new; end if;
    select author_id into owner_id from feed_recommendation_impressions
        where impression_id=new.impression_id and viewer_id=new.viewer_id and post_id=new.subject_id;
    if new.subject_type<>'POST' or owner_id is null then
        raise exception 'Unproven affinity recommendation source' using errcode='23514',constraint='feed_affinity_source_binding';
    end if;
    case new.action
        when 'LIKE' then positive:=0.20;
        when 'COMMENT' then positive:=0.30;
        when 'SAVE','SHARE' then positive:=0.35;
        when 'FOLLOW','ACCEPT_FOLLOW' then positive:=0.50;
        when 'CLICK' then positive:=0.08;
        when 'DWELL' then positive:=least(coalesce(new.dwell_ms,0),60000)::float8/60000*0.20;
        when 'FAST_SCROLL' then negative:=0.15;
        when 'HIDE','NOT_INTERESTED' then negative:=0.40;
        when 'MUTE' then negative:=0.70;
        when 'BLOCK','REPORT' then negative:=1.0;
        else raise exception 'Unsupported affinity recommendation action' using errcode='23514',constraint='feed_affinity_source_binding';
    end case;
    -- Source row normalization matches the retained identity comparison; no
    -- source payload/context copy is stored in the contribution ledger.
    hash:=feed_affinity_identity_hash(jsonb_set(to_jsonb(new)-'event_id',
        '{occurred_at}',to_jsonb(extract(epoch from new.occurred_at))));
    perform feed_affinity_capture_source('RECOMMENDATION',new.event_id,new.viewer_id,owner_id,new.actor_id,
        new.subject_id,new.occurred_at,positive,negative,hash);
    return new;
end;
$$;
create trigger feed_affinity_capture_recommendation after insert on feed_recommendation_events
    for each row execute function feed_affinity_capture_recommendation();

create function feed_affinity_immutable_contribution() returns trigger language plpgsql as $$
begin
    if new is distinct from old then
        raise exception 'Affinity source contributions are immutable'
            using errcode='23514',constraint='feed_affinity_source_immutable';
    end if;
    return new;
end;
$$;
create trigger feed_affinity_source_immutable before update on feed_affinity_source_contributions
    for each row execute function feed_affinity_immutable_contribution();

-- Account cleanup may erase retained personal source state. This is not a
-- periodic compaction/purge policy or permission to infer a feature baseline.
create function feed_affinity_forget_subject(subject uuid) returns void language plpgsql as $$
begin
    perform post_lock_subject_ids(array[subject]);
    delete from feed_affinity_source_contributions where viewer_id=subject or author_id=subject or actor_id=subject;
    -- Erasing a distinct actor's contribution leaves its pair's counter intact.
    -- Never disguise the resulting gap by rebasing or inventing feature adoption.
    delete from feed_affinity_source_pairs where viewer_id=subject or author_id=subject;
end;
$$;
