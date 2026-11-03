-- Apply as the schema owner to a fresh, dedicated non-owner login, using:
-- psql --set=ON_ERROR_STOP=1 --single-transaction --set=runtime_role=ROLE --file=runtime-grants.sql
-- Role/database provisioning is separate. No grants to future objects or
-- controller-only functions are implied. Keep this allowlist with migrations.
grant usage on schema public to :"runtime_role";
grant select on
    public._sqlx_migrations
to :"runtime_role";
grant select, insert on
    public.auth_user_tombstones
to :"runtime_role";
grant select, insert, update, delete on
    public.auth_audit_events,
    public.auth_mail_outbox,
    public.auth_user_roles,
    public.email_verifications,
    public.login_audit,
    public.oauth_authorizations,
    public.password_reset_tokens,
    public.refresh_tokens,
    public.user_deletion_retry_resolutions,
    public.user_deletion_sagas,
    public.user_deletion_steps,
    public.user_identities,
    public.users
to :"runtime_role";
