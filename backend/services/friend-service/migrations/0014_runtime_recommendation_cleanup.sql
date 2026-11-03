
-- A serving role needs column UPDATE privilege for FOR UPDATE SKIP LOCKED
-- retention, but that must not permit reassignment of externally published data.
-- Keep this SECURITY INVOKER: current_user must be the actual caller. Owners
-- and their authorized inherited roles retain publication/update capability.
create function public.friend_guard_owner_managed_recommendation() returns trigger
language plpgsql security invoker set search_path=pg_catalog,public,pg_temp as $$
begin
    if not pg_catalog.pg_has_role(current_user,
        (select relowner from pg_catalog.pg_class where oid=tg_relid), 'USAGE') then
        raise exception 'Recommendation publication update requires its owner'
            using errcode='42501',constraint='friend_recommendation_owner_update';
    end if;
    return new;
end;
$$;
revoke all on function public.friend_guard_owner_managed_recommendation() from public;
create trigger people_assignment_owner_update before update on public.people_experiment_assignments
    for each row execute function public.friend_guard_owner_managed_recommendation();
create trigger people_embedding_owner_update before update on public.people_embeddings
    for each row execute function public.friend_guard_owner_managed_recommendation();
