-- Apply as the schema owner to a fresh, dedicated non-owner login, using:
-- psql --set=ON_ERROR_STOP=1 --single-transaction --set=runtime_role=ROLE --file=runtime-grants.sql
-- Role/database provisioning is separate. No grants to future objects or
-- controller-only functions are implied. Keep this allowlist with migrations.
grant usage on schema public to :"runtime_role";
grant select on
    public._sqlx_migrations
to :"runtime_role";
grant select, insert, update, delete on
    public.notification_audit_events,
    public.notification_recipient_suppressions,
    public.notification_subject_states,
    public.notification_user_deletion_receipts,
    public.notifications
to :"runtime_role";

-- BE33 v1 deletion proofs are insert-only; no mutation/deletion privileges.
grant select, insert on
    public.notification_deletion_fences_v1,
    public.notification_deletion_receipts_v1
to :"runtime_role";
grant usage on sequence public.notification_deletion_fences_v1_fence_revision_seq
to :"runtime_role";
