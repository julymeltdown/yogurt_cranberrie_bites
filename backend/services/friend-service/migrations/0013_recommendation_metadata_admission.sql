-- Additive repair: retained identity comparison requires valid object metadata.
-- Existing rows are not rewritten or adopted; unproven old raw data stays held.
create function friend_recommendation_metadata_admission() returns trigger
language plpgsql set search_path=pg_catalog,public,pg_temp as $$
declare parsed jsonb;
begin
    if new.metadata_json is null or octet_length(new.metadata_json)>4096 then
        raise exception 'Recommendation metadata must be a bounded JSON object'
            using errcode='23514',constraint='friend_recommendation_metadata_admission';
    end if;
    begin
        parsed:=new.metadata_json::jsonb;
    exception when data_exception then
        raise exception 'Recommendation metadata must be a JSON object'
            using errcode='23514',constraint='friend_recommendation_metadata_admission';
    end;
    if jsonb_typeof(parsed) is distinct from 'object' then
        raise exception 'Recommendation metadata must be a JSON object'
            using errcode='23514',constraint='friend_recommendation_metadata_admission';
    end if;
    return new;
end;
$$;
revoke all on function friend_recommendation_metadata_admission() from public;
-- Pure validation before identity lookup; no subject/row/advisory lock is taken.
create trigger a0000_people_event_metadata before insert on people_recommendation_events
    for each row execute function friend_recommendation_metadata_admission();
