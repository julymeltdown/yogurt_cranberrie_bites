-- Compatibility reduction must not require direct runtime INSERT into protected
-- provenance. Retain the existing source binding, duplicate, scalar and ordered
-- writer guards; execute their internal observation write as the schema owner.
-- Pin all unqualified objects to trusted schemas and grant this entry point only
-- through the service's separately applied runtime policy. No fresh authority,
-- baseline, backfill or migration-history rewrite is introduced.
alter function public.feed_affinity_apply_legacy_effect(text,uuid,uuid,uuid,uuid,uuid,text,timestamptz,float8,float8)
    security definer;
alter function public.feed_affinity_apply_legacy_effect(text,uuid,uuid,uuid,uuid,uuid,text,timestamptz,float8,float8)
    set search_path=pg_catalog,public,pg_temp;
revoke all on function public.feed_affinity_apply_legacy_effect(text,uuid,uuid,uuid,uuid,uuid,text,timestamptz,float8,float8) from public;

-- A serving role needs column UPDATE privilege for FOR UPDATE SKIP LOCKED
-- retention, but that must not permit reassignment of externally published data.
-- Keep this SECURITY INVOKER: current_user must be the actual caller. Owners
-- and their authorized inherited roles retain publication/update capability.
create function public.post_guard_owner_managed_recommendation() returns trigger
language plpgsql security invoker set search_path=pg_catalog,public,pg_temp as $$
begin
    if not pg_catalog.pg_has_role(current_user,
        (select relowner from pg_catalog.pg_class where oid=tg_relid), 'USAGE') then
        raise exception 'Recommendation publication update requires its owner'
            using errcode='42501',constraint='post_recommendation_owner_update';
    end if;
    return new;
end;
$$;
revoke all on function public.post_guard_owner_managed_recommendation() from public;
create trigger feed_assignment_owner_update before update on public.feed_experiment_assignments
    for each row execute function public.post_guard_owner_managed_recommendation();
create trigger feed_embedding_owner_update before update on public.feed_embeddings
    for each row execute function public.post_guard_owner_managed_recommendation();
