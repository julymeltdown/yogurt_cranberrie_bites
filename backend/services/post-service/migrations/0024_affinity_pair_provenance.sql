-- Conservative observations, not a baseline, adoption receipt or activation.
-- Existing pairs have unknown history. No backfill/lifetime absence scan grants
-- fresh authority; a missing observation row means HOLD_CAPTURE_ONLY as well.
create table feed_affinity_pair_provenance (
    epoch_id uuid not null references feed_affinity_capture_epoch(epoch_id),
    viewer_id uuid not null,
    author_id uuid not null,
    origin_mode text not null default 'HOLD_CAPTURE_ONLY'
        constraint feed_affinity_origin_mode check (origin_mode='HOLD_CAPTURE_ONLY'),
    first_source_sequence bigint null check (first_source_sequence>0),
    first_namespace text null check (first_namespace in ('OUTBOX','RECOMMENDATION')),
    first_source_id uuid null,
    first_identity_sha256 text null check (first_identity_sha256 ~ '^[0-9a-f]{64}$'),
    first_source_erased boolean not null default false,
    capture_prefix_preexisted boolean not null default false,
    scalar_observed boolean not null default false,
    legacy_scalar_observed boolean not null default false,
    legacy_write_observed boolean not null default false,
    observation_started_at timestamptz not null default clock_timestamp(),
    primary key (epoch_id,viewer_id,author_id),
    constraint feed_affinity_origin_first_source check (
        (first_source_sequence is null and first_namespace is null
            and first_source_id is null and first_identity_sha256 is null
            and not first_source_erased)
        or (first_source_sequence is not null and (
            (not first_source_erased and first_namespace is not null
                and first_source_id is not null and first_identity_sha256 is not null)
            or (first_source_erased and first_namespace is null
                and first_source_id is null and first_identity_sha256 is null)))
    )
);
create index feed_affinity_origin_viewer_idx on feed_affinity_pair_provenance(viewer_id);
create index feed_affinity_origin_author_idx on feed_affinity_pair_provenance(author_id);
create index feed_affinity_origin_source_idx on feed_affinity_pair_provenance(first_namespace,first_source_id)
    where first_source_id is not null;

-- Source-bound observations of the COMPATIBILITY reducer's actual committed
-- write. These are not ordered receipts, do not establish a baseline, and do
-- not advance a feature cursor. Actor is retained solely for complete erasure.
create table feed_affinity_legacy_effect_observations (
    namespace text not null check (namespace in ('OUTBOX','RECOMMENDATION')),
    source_id uuid not null,
    epoch_id uuid not null,
    viewer_id uuid not null,
    author_id uuid not null,
    actor_id uuid null,
    source_sequence bigint not null check (source_sequence>0),
    source_identity_sha256 text not null check (source_identity_sha256 ~ '^[0-9a-f]{64}$'),
    effect_identity_sha256 text not null check (effect_identity_sha256 ~ '^[0-9a-f]{64}$'),
    observed_at timestamptz not null default clock_timestamp(),
    primary key (namespace,source_id),
    foreign key (epoch_id,viewer_id,author_id)
        references feed_affinity_pair_provenance(epoch_id,viewer_id,author_id)
);
create index feed_affinity_legacy_effect_viewer_idx on feed_affinity_legacy_effect_observations(viewer_id);
create index feed_affinity_legacy_effect_author_idx on feed_affinity_legacy_effect_observations(author_id);
create index feed_affinity_legacy_effect_actor_idx on feed_affinity_legacy_effect_observations(actor_id)
    where actor_id is not null;

-- Validate the ACTUAL reducer input and perform the scalar mutation inside the
-- same routine as its observation. Reading an unrelated pre-existing score is
-- never evidence that this source was applied.
create function feed_affinity_apply_legacy_effect(
    source_namespace text, effect_source uuid, effect_viewer uuid, effect_author uuid,
    effect_actor uuid, effect_post uuid, effect_action text, effect_time timestamptz,
    positive float8, negative float8
) returns boolean language plpgsql as $$
declare
    contribution feed_affinity_source_contributions%rowtype;
    scalar feed_author_affinities%rowtype;
    prior_scalar feed_author_affinities%rowtype;
    prior_exists boolean;
    expected_positive float8;
    expected_negative float8;
    expected_time timestamptz;
    stored_event post_event_outbox%rowtype;
    stored_recommendation feed_recommendation_events%rowtype;
    bound_source boolean;
    source_hash text;
    source_action text;
    effect_hash text;
begin
    perform post_assert_subjects_not_deleted(array[effect_viewer,effect_author,effect_actor]);
    if source_namespace is null or source_namespace not in ('OUTBOX','RECOMMENDATION') then
        raise exception 'Unsupported legacy affinity namespace'
            using errcode='23514',constraint='feed_affinity_legacy_effect_binding';
    end if;
    select * into contribution from feed_affinity_source_contributions
        where namespace=source_namespace and source_id=effect_source;
    bound_source:=found;
    if bound_source then
        if (effect_viewer,effect_author,effect_actor,effect_post,effect_time,positive,negative)
            is distinct from (contribution.viewer_id,contribution.author_id,contribution.actor_id,
                contribution.post_id,contribution.occurred_at,contribution.positive_delta,contribution.negative_delta) then
            raise exception 'Legacy affinity mutation differs from captured source'
                using errcode='23514',constraint='feed_affinity_legacy_effect_binding';
        end if;
        if source_namespace='OUTBOX' then
            select * into strict stored_event from post_event_outbox where event_id=effect_source;
            source_action:=case stored_event.event_type
                when 'POST_LIKED' then 'LIKE' when 'POST_UNLIKED' then 'UNLIKE'
                when 'POST_COMMENTED' then 'COMMENT' when 'POST_COMMENT_DELETED' then 'DELETE_COMMENT' end;
            source_hash:=feed_affinity_identity_hash(jsonb_build_object(
                'eventId',stored_event.event_id,'eventType',stored_event.event_type,
                'aggregateId',stored_event.aggregate_id,'aggregateVersion',stored_event.aggregate_version,
                'actorId',stored_event.actor_id,'attributes',stored_event.attributes,'payload',stored_event.payload,
                'occurredAt',extract(epoch from stored_event.occurred_at),
                'createdAt',extract(epoch from stored_event.created_at)));
        else
            select * into strict stored_recommendation from feed_recommendation_events where event_id=effect_source;
            source_action:=stored_recommendation.action;
            source_hash:=feed_affinity_identity_hash(jsonb_set(to_jsonb(stored_recommendation)-'event_id',
                '{occurred_at}',to_jsonb(extract(epoch from stored_recommendation.occurred_at))));
        end if;
        if effect_action is distinct from source_action or source_hash is distinct from contribution.identity_sha256 then
            raise exception 'Legacy affinity owner identity differs from captured source'
                using errcode='23514',constraint='feed_affinity_legacy_effect_binding';
        end if;
        if exists(select 1 from feed_affinity_legacy_effect_observations
            where namespace=source_namespace and source_id=effect_source) then
            raise exception 'Legacy source already observed; owning duplicate check required'
                using errcode='23514',constraint='feed_affinity_legacy_effect_identity';
        end if;
    end if;
    -- Missing/unsequenced sources use the compatibility write but receive only
    -- an unbound legacy marker. No source identity or baseline is fabricated.
    select * into prior_scalar from feed_author_affinities
        where viewer_id=effect_viewer and author_id=effect_author for update;
    prior_exists:=found;
    expected_positive:=case when prior_exists then
        greatest(0::float8,least(1::float8,prior_scalar.affinity_score*0.95+positive)) else positive end;
    expected_negative:=case when prior_exists then
        greatest(0::float8,least(1::float8,prior_scalar.negative_score*0.95+negative)) else negative end;
    expected_time:=case when prior_exists then greatest(prior_scalar.last_interacted_at,effect_time) else effect_time end;
    insert into feed_author_affinities(viewer_id,author_id,affinity_score,negative_score,
        last_interacted_at,feature_version)
    values(effect_viewer,effect_author,positive,negative,effect_time,'feed-affinity-v1')
    on conflict(viewer_id,author_id) do update set
        affinity_score=greatest(0::float8,least(1::float8,feed_author_affinities.affinity_score*0.95+excluded.affinity_score)),
        negative_score=greatest(0::float8,least(1::float8,feed_author_affinities.negative_score*0.95+excluded.negative_score)),
        last_interacted_at=greatest(feed_author_affinities.last_interacted_at,excluded.last_interacted_at),
        feature_version=excluded.feature_version
    returning * into scalar;
    if not found then
        raise exception 'Legacy affinity scalar mutation was skipped'
            using errcode='23514',constraint='feed_affinity_legacy_effect_mutation';
    end if;
    if (scalar.viewer_id,scalar.author_id,scalar.affinity_score,scalar.negative_score,
        scalar.last_interacted_at,scalar.feature_version) is distinct from
       (effect_viewer,effect_author,expected_positive,expected_negative,expected_time,'feed-affinity-v1'::text) then
        raise exception 'Legacy affinity scalar mutation differs from its source'
            using errcode='23514',constraint='feed_affinity_legacy_effect_mutation';
    end if;
    if not bound_source then return false; end if;
    effect_hash:=feed_affinity_identity_hash(jsonb_build_object(
        'sourceIdentity',contribution.identity_sha256,'sourceSequence',contribution.source_sequence,
        'positiveDelta',positive,'negativeDelta',negative,
        'positiveScore',scalar.affinity_score,'negativeScore',scalar.negative_score,
        'lastInteractedAt',extract(epoch from scalar.last_interacted_at),'featureVersion',scalar.feature_version));
    insert into feed_affinity_legacy_effect_observations(namespace,source_id,epoch_id,viewer_id,
        author_id,actor_id,source_sequence,source_identity_sha256,effect_identity_sha256)
    values(source_namespace,effect_source,contribution.epoch_id,contribution.viewer_id,contribution.author_id,
        contribution.actor_id,contribution.source_sequence,contribution.identity_sha256,effect_hash);
    return true;
end;
$$;
create trigger feed_affinity_legacy_effect_immutable before update on feed_affinity_legacy_effect_observations
    for each row execute function feed_affinity_immutable_contribution();

create function feed_affinity_observe_source() returns trigger language plpgsql as $$
begin
    insert into feed_affinity_pair_provenance as origin (
        epoch_id,viewer_id,author_id,first_source_sequence,first_namespace,
        first_source_id,first_identity_sha256,capture_prefix_preexisted,
        scalar_observed,legacy_scalar_observed,legacy_write_observed)
    values (new.epoch_id,new.viewer_id,new.author_id,new.source_sequence,new.namespace,
        new.source_id,new.identity_sha256,new.source_sequence>1,
        exists(select 1 from feed_author_affinities where viewer_id=new.viewer_id and author_id=new.author_id),
        exists(select 1 from feed_author_affinity where viewer_id=new.viewer_id and author_id=new.author_id),
        new.namespace='OUTBOX' and exists(select 1 from feed_projection_inbox
            where consumer='feed-engagement' and event_id=new.source_id))
    on conflict (epoch_id,viewer_id,author_id) do update set
        first_source_sequence=coalesce(origin.first_source_sequence,excluded.first_source_sequence),
        first_namespace=case when origin.first_source_sequence is null then excluded.first_namespace else origin.first_namespace end,
        first_source_id=case when origin.first_source_sequence is null then excluded.first_source_id else origin.first_source_id end,
        first_identity_sha256=case when origin.first_source_sequence is null then excluded.first_identity_sha256 else origin.first_identity_sha256 end,
        capture_prefix_preexisted=origin.capture_prefix_preexisted or
            (origin.first_source_sequence is null and excluded.capture_prefix_preexisted),
        scalar_observed=origin.scalar_observed or excluded.scalar_observed,
        legacy_scalar_observed=origin.legacy_scalar_observed or excluded.legacy_scalar_observed,
        legacy_write_observed=origin.legacy_write_observed or excluded.legacy_write_observed;
    return new;
end;
$$;
create trigger feed_affinity_observe_source after insert on feed_affinity_source_contributions
    for each row execute function feed_affinity_observe_source();

-- The observation commits/rolls back with the actual scalar write, including
-- current outbox and recommendation reducers. It is NOT an ordered resolution.
-- Historical writes before installation remain unknown, even if scores vanish.
create function feed_affinity_observe_legacy_write() returns trigger language plpgsql as $$
declare capture_epoch uuid;
begin
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
create trigger feed_affinity_observe_legacy_write after insert or update on feed_author_affinities
    for each row execute function feed_affinity_observe_legacy_write();
create trigger feed_affinity_observe_legacy_write after insert or update on feed_author_affinity
    for each row execute function feed_affinity_observe_legacy_write();

create function feed_affinity_provenance_guard() returns trigger language plpgsql as $$
declare erased_subject uuid;
begin
    if (new.epoch_id,new.viewer_id,new.author_id,new.origin_mode,new.observation_started_at)
        is distinct from (old.epoch_id,old.viewer_id,old.author_id,old.origin_mode,old.observation_started_at)
        or (old.capture_prefix_preexisted and not new.capture_prefix_preexisted)
        or (old.scalar_observed and not new.scalar_observed)
        or (old.legacy_scalar_observed and not new.legacy_scalar_observed)
        or (old.legacy_write_observed and not new.legacy_write_observed)
        or (old.first_source_erased and not new.first_source_erased) then
        raise exception 'Affinity provenance cannot be reset'
            using errcode='23514',constraint='feed_affinity_provenance_identity';
    end if;
    if old.first_source_sequence is not null and
       (new.first_source_sequence,new.first_namespace,new.first_source_id,new.first_identity_sha256,new.first_source_erased)
       is distinct from (old.first_source_sequence,old.first_namespace,old.first_source_id,old.first_identity_sha256,old.first_source_erased) then
        erased_subject:=nullif(current_setting('port_rust.affinity_erasure_subject',true),'')::uuid;
        if erased_subject is null or old.first_source_erased or not new.first_source_erased
            or new.first_source_sequence is distinct from old.first_source_sequence
            or new.first_namespace is not null or new.first_source_id is not null or new.first_identity_sha256 is not null
            or not exists(select 1 from feed_affinity_source_contributions c
                where c.namespace=old.first_namespace and c.source_id=old.first_source_id
                  and (c.viewer_id=erased_subject or c.author_id=erased_subject or c.actor_id=erased_subject)) then
            raise exception 'Affinity first observation is immutable'
                using errcode='23514',constraint='feed_affinity_provenance_identity';
        end if;
    end if;
    return new;
end;
$$;
create trigger feed_affinity_provenance_guard before update on feed_affinity_pair_provenance
    for each row execute function feed_affinity_provenance_guard();

-- Erase personally linkable first-source identity before its source disappears.
-- Preserve the ordinal/hold evidence for unrelated viewer-author pairs: actor
-- erasure cannot turn an observed pair into a new zero-baseline candidate.
create or replace function feed_affinity_forget_subject(subject uuid) returns void language plpgsql as $$
declare previous_erasure text;
begin
    perform post_lock_subject_ids(array[subject]);
    previous_erasure:=current_setting('port_rust.affinity_erasure_subject',true);
    perform set_config('port_rust.affinity_erasure_subject',subject::text,true);
    update feed_affinity_pair_provenance origin set first_namespace=null,first_source_id=null,
        first_identity_sha256=null,first_source_erased=true
    where origin.viewer_id<>subject and origin.author_id<>subject and exists(
        select 1 from feed_affinity_source_contributions c
        where c.namespace=origin.first_namespace and c.source_id=origin.first_source_id and c.actor_id=subject);
    perform set_config('port_rust.affinity_erasure_subject',coalesce(previous_erasure,''),true);
    delete from feed_affinity_legacy_effect_observations where viewer_id=subject or author_id=subject or actor_id=subject;
    delete from feed_affinity_source_contributions where viewer_id=subject or author_id=subject or actor_id=subject;
    delete from feed_affinity_pair_provenance where viewer_id=subject or author_id=subject;
    delete from feed_affinity_source_pairs where viewer_id=subject or author_id=subject;
end;
$$;
