-- Retained-event retry identity and actual impression admission. No historical
-- backfill, new receipt lifetime, post-body-purge replay or public stable ID.
create function friend_recommendation_events_equal(left_event public.people_recommendation_events,
    right_event public.people_recommendation_events)
returns boolean language sql immutable set search_path=pg_catalog,public,pg_temp as $$
    select (to_jsonb(left_event)-'metadata_json')=(to_jsonb(right_event)-'metadata_json')
        and left_event.metadata_json::jsonb=right_event.metadata_json::jsonb;
$$;
revoke all on function friend_recommendation_events_equal(public.people_recommendation_events,public.people_recommendation_events) from public;

create function friend_recommendation_event_admission() returns trigger
language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
declare retained public.people_recommendation_events%rowtype; observed boolean;
begin
    -- Separate from bigint lifecycle locks; a hash collision only serializes.
    -- Take global identity before any incoming/retained subject lock, including
    -- disjoint viewers that reuse the same source UUID.
    perform pg_advisory_xact_lock(1380270916,hashtext(new.event_id::text));
    select * into retained from public.people_recommendation_events where event_id=new.event_id;
    observed:=found;
    perform public.friend_assert_subjects_not_deleted(array[new.viewer_id,new.subject_id,new.actor_id,
        retained.viewer_id,retained.subject_id,retained.actor_id]);
    select * into retained from public.people_recommendation_events where event_id=new.event_id for share;
    if found then
        if not public.friend_recommendation_events_equal(retained,new) then
            raise exception 'Recommendation event identity conflict'
                using errcode='23514',constraint='friend_recommendation_event_identity';
        end if;
        -- Exact retained retry creates no event or refresh job and does not
        -- require an impression that may have been independently pruned.
        return null;
    elsif observed then
        raise exception 'Retained recommendation disappeared during admission'
            using errcode='23514',constraint='friend_recommendation_retry_unavailable';
    end if;
    if new.subject_type<>'USER' or new.surface<>'PEOPLE_RECOMMENDATION'
        or (new.action='IMPRESSION' and new.impression_id is distinct from new.event_id) then
        raise exception 'Invalid people recommendation context'
            using errcode='23514',constraint='friend_recommendation_impression_admission';
    end if;
    perform 1 from public.people_recommendation_impressions
        where impression_id=new.impression_id and viewer_id=new.viewer_id
          and candidate_user_id=new.subject_id for share;
    if not found then
        raise exception 'Recommendation impression is not bound to viewer and candidate'
            using errcode='23514',constraint='friend_recommendation_impression_admission';
    end if;
    return new;
end;
$$;
revoke all on function friend_recommendation_event_admission() from public;
create trigger a000_people_event_admission before insert on people_recommendation_events
    for each row execute function friend_recommendation_event_admission();

create function friend_recommendation_event_immutable() returns trigger
language plpgsql security definer set search_path=pg_catalog,public,pg_temp as $$
begin
    -- Retention and actual subject erasure still own DELETE. Preserved facts
    -- cannot be rewritten into another identity, including by rolling writers.
    -- Reject even no-op UPDATE: PostgreSQL already owns the event row before
    -- running this trigger. Continuing to the older subject-lock trigger would
    -- reverse lifecycle lock order and can deadlock with account erasure.
    raise exception 'Recommendation event is immutable'
        using errcode='23514',constraint='friend_recommendation_event_identity';
end;
$$;
revoke all on function friend_recommendation_event_immutable() from public;
create trigger a000_people_event_immutable before update on people_recommendation_events
    for each row execute function friend_recommendation_event_immutable();
