-- Apply as the schema owner to a fresh, dedicated non-owner login, using:
-- psql --set=ON_ERROR_STOP=1 --single-transaction --set=runtime_role=ROLE --file=runtime-grants.sql
-- Role/database provisioning is separate. No grants to future objects or
-- controller-only functions are implied. Keep this allowlist with migrations.
grant usage on schema public to :"runtime_role";
grant select on
    public._sqlx_migrations
to :"runtime_role";
grant select, insert on
    public.profile_user_deletion_tombstones
to :"runtime_role";
grant select, insert, update, delete on
    public.avatars,
    public.profile_audit_events,
    public.profile_mutation_receipts,
    public.profiles
to :"runtime_role";
