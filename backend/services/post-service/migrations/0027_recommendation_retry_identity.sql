-- Controlled ordered recommendations retain only the original normalization
-- defaults needed to verify a retry. No metadata/event payload, retention period,
-- production activation or source-pruning authorization is introduced.
create table feed_recommendation_retry_contexts (
    namespace text not null default 'RECOMMENDATION' check(namespace='RECOMMENDATION'),
    source_id uuid not null,
    normalization_version text not null default 'feed-recommendation-v1' check(normalization_version='feed-recommendation-v1'),
    viewer_id uuid not null,
    author_id uuid not null,
    actor_id uuid null,
    post_id uuid not null,
    impression_id uuid not null,
    position integer not null,
    request_id text not null,
    session_id uuid null,
    source text not null,
    model_version text not null,
    propensity_micros integer not null check(propensity_micros between 1 and 1000000),
    primary key(namespace,source_id),
    foreign key(namespace,source_id) references feed_affinity_resolution_receipts on delete cascade
);
revoke all on feed_recommendation_retry_contexts from public;

create function feed_recommendation_retry_context_guard() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    if tg_op='DELETE' and exists(select 1 from public.post_user_deletion_tombstones
        where user_id in(old.viewer_id,old.author_id,old.actor_id)) then return old; end if;
    if tg_op='INSERT' and exists(select 1 from public.feed_affinity_resolution_receipts receipt
        join public.feed_recommendation_events event on event.event_id=receipt.source_id
        join public.feed_recommendation_impressions impression on impression.impression_id=event.impression_id
            and impression.viewer_id=event.viewer_id and impression.post_id=event.subject_id
        where receipt.namespace='RECOMMENDATION' and receipt.source_id=new.source_id
            and receipt.identity_sha256=public.feed_affinity_identity_hash(
                jsonb_set(to_jsonb(event)-'event_id','{occurred_at}',to_jsonb(extract(epoch from event.occurred_at))))
            and (new.viewer_id,new.author_id,new.actor_id,new.post_id,new.impression_id,
                 new.position,new.request_id,new.session_id,new.source,new.model_version,new.propensity_micros)
              is not distinct from
                (receipt.viewer_id,receipt.author_id,receipt.actor_id,receipt.post_id,impression.impression_id,
                 impression.position,impression.request_id,impression.session_id,impression.source,
                 impression.rank_version,impression.propensity_micros)
            and impression.author_id=receipt.author_id) then return new; end if;
    raise exception 'Retry context requires its actual accepted source'
        using errcode='23514',constraint='feed_recommendation_retry_identity';
end;
$$;
revoke all on function feed_recommendation_retry_context_guard() from public;
create trigger feed_recommendation_retry_context_guard before insert or update or delete
    on feed_recommendation_retry_contexts for each row execute function feed_recommendation_retry_context_guard();

create function feed_recommendation_capture_retry_context() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
declare inserted bigint;
begin
    if new.namespace<>'RECOMMENDATION' then return new; end if;
    insert into public.feed_recommendation_retry_contexts(namespace,source_id,viewer_id,author_id,actor_id,
        post_id,impression_id,position,request_id,session_id,source,model_version,propensity_micros)
    select new.namespace,new.source_id,new.viewer_id,new.author_id,new.actor_id,new.post_id,
        impression.impression_id,impression.position,impression.request_id,impression.session_id,
        impression.source,impression.rank_version,impression.propensity_micros
    from public.feed_recommendation_events event
    join public.feed_recommendation_impressions impression on impression.impression_id=event.impression_id
        and impression.viewer_id=event.viewer_id and impression.post_id=event.subject_id
    where event.event_id=new.source_id;
    get diagnostics inserted=row_count;
    if inserted<>1 then raise exception 'Accepted recommendation lost retry context'
        using errcode='23514',constraint='feed_recommendation_retry_identity'; end if;
    return new;
end;
$$;
revoke all on function feed_recommendation_capture_retry_context() from public;
create trigger feed_recommendation_capture_retry_context after insert on feed_affinity_resolution_receipts
    for each row execute function feed_recommendation_capture_retry_context();

-- Serialize every actual source insertion before any subject/row locks. A
-- receipt cannot appear after admission subject discovery through another
-- source writer, even when the same global event ID names different subjects.
create function feed_recommendation_event_identity_guard() returns trigger language plpgsql
security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    perform pg_advisory_xact_lock(1380270915,hashtext(new.event_id::text));
    if exists(select 1 from public.feed_affinity_resolution_receipts
        where namespace='RECOMMENDATION' and source_id=new.event_id) then
        raise exception 'Resolved recommendation cannot recreate its owner body'
            using errcode='23514',constraint='feed_recommendation_retry_identity';
    end if;
    return new;
end;
$$;
revoke all on function feed_recommendation_event_identity_guard() from public;
create trigger a000_recommendation_event_identity before insert on feed_recommendation_events
    for each row execute function feed_recommendation_event_identity_guard();

-- Typed ROW arguments use exactly the same PG/SQLx normalization as INSERT:
-- timestamp microseconds, JSONB numeric precision and NULL distinctions.
-- NULL means no retained proof; false means identity conflict; true is an
-- effect-free duplicate. No INSERT, pair allocation or impression admission.
create function feed_recommendation_receipt_matches(candidate public.feed_recommendation_events)
returns boolean language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
declare receipt public.feed_affinity_resolution_receipts%rowtype;
    contribution public.feed_affinity_source_contributions%rowtype;
    head public.feed_affinity_pair_heads%rowtype;
begin
    select * into receipt from public.feed_affinity_resolution_receipts
        where namespace='RECOMMENDATION' and source_id=candidate.event_id;
    if not found then return null; end if;
    perform public.post_lock_subject_ids(array[candidate.viewer_id,candidate.actor_id,
        receipt.viewer_id,receipt.author_id,receipt.actor_id]);
    -- Erasure can win while waiting. Reread under the complete subject locks.
    select * into receipt from public.feed_affinity_resolution_receipts
        where namespace='RECOMMENDATION' and source_id=candidate.event_id;
    if not found then return null; end if;
    select * into contribution from public.feed_affinity_source_contributions
        where namespace=receipt.namespace and source_id=receipt.source_id;
    if not found or (receipt.identity_sha256,receipt.epoch_id,receipt.viewer_id,receipt.author_id,
        receipt.actor_id,receipt.source_sequence,receipt.post_id,receipt.occurred_at,
        receipt.positive_delta,receipt.negative_delta)
        is distinct from (contribution.identity_sha256,contribution.epoch_id,contribution.viewer_id,
        contribution.author_id,contribution.actor_id,contribution.source_sequence,contribution.post_id,
        contribution.occurred_at,contribution.positive_delta,contribution.negative_delta) then
        raise exception 'Retained recommendation proof is incomplete'
            using errcode='23514',constraint='feed_affinity_resolution_immutable'; end if;
    select * into head from public.feed_affinity_pair_heads
        where (epoch_id,viewer_id,author_id)=(receipt.epoch_id,receipt.viewer_id,receipt.author_id);
    if not found or head.resolved_sequence<receipt.source_sequence
        or not exists(select 1 from public.feed_recommendation_retry_contexts
            where namespace=receipt.namespace and source_id=receipt.source_id)
        or exists(select 1 from public.post_user_deletion_tombstones
            where user_id in(receipt.viewer_id,receipt.author_id,receipt.actor_id)) then
        raise exception 'Retained recommendation proof is unavailable'
            using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
    if (receipt.after_state->>'provisioning_id')::uuid is distinct from head.provisioning_id
        or (receipt.after_state->>'feature_epoch')::uuid is distinct from head.feature_epoch
        or (receipt.after_state->>'writer_generation')::uuid is distinct from head.writer_generation
        or (receipt.after_state->>'resolved_sequence')::bigint is distinct from receipt.source_sequence
        or not exists(select 1 from public.feed_affinity_pair_baselines b
            where (b.epoch_id,b.viewer_id,b.author_id)=(receipt.epoch_id,receipt.viewer_id,receipt.author_id)
                and (b.provisioning_id,b.feature_epoch,b.writer_generation,b.feature_version)=
                    (head.provisioning_id,head.feature_epoch,head.writer_generation,head.feature_version)) then
        raise exception 'Retained recommendation generation differs'
            using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
    return receipt.identity_sha256=public.feed_affinity_identity_hash(
        jsonb_set(to_jsonb(candidate)-'event_id','{occurred_at}',to_jsonb(extract(epoch from candidate.occurred_at))));
end;
$$;
revoke all on function feed_recommendation_receipt_matches(public.feed_recommendation_events) from public;

-- Preserve the existing resolver, moving only durable duplicate verification
-- ahead of owner-body validation. New resolutions still require actual sources.
create or replace function feed_affinity_resolve_ordered(source_namespace text,effect_source uuid,
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
    select * into r from public.feed_affinity_resolution_receipts where namespace=c.namespace and source_id=c.source_id;
    if found then
        if (r.identity_sha256,r.epoch_id,r.viewer_id,r.author_id,r.actor_id,r.source_sequence,
            r.post_id,r.occurred_at,r.positive_delta,r.negative_delta)
            is distinct from (c.identity_sha256,c.epoch_id,c.viewer_id,c.author_id,c.actor_id,c.source_sequence,
            c.post_id,c.occurred_at,c.positive_delta,c.negative_delta) then
            raise exception 'Conflicting resolution identity' using errcode='23514',constraint='feed_affinity_resolution_immutable'; end if;
        select * into h from public.feed_affinity_pair_heads
            where (epoch_id,viewer_id,author_id)=(c.epoch_id,c.viewer_id,c.author_id);
        if not found or h.resolved_sequence<c.source_sequence
            or (h.provisioning_id,h.feature_epoch,h.writer_generation,h.feature_version)
                is distinct from (a.provisioning_id,a.feature_epoch,a.writer_generation,a.feature_version)
            or (r.after_state->>'provisioning_id')::uuid is distinct from h.provisioning_id
            or (r.after_state->>'feature_epoch')::uuid is distinct from h.feature_epoch
            or (r.after_state->>'writer_generation')::uuid is distinct from h.writer_generation
            or (r.after_state->>'resolved_sequence')::bigint is distinct from c.source_sequence then
            raise exception 'Retained resolution generation differs'
                using errcode='23514',constraint='feed_affinity_ordered_writer_required'; end if;
        -- No owner body is needed to acknowledge the already resolved fact.
        -- This branch allocates nothing and creates no new effect/permission.
        return true;
    end if;
    perform public.feed_affinity_validate_resolution_source(c);
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
