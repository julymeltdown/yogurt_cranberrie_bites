-- Apply as the schema owner to a fresh, dedicated non-owner login, using:
-- psql --set=ON_ERROR_STOP=1 --single-transaction --set=runtime_role=ROLE --file=runtime-grants.sql
-- Role/database provisioning is separate. No grants to future objects or
-- controller-only functions are implied. Keep this allowlist with migrations.
grant usage on schema public to :"runtime_role";
grant select on
    public._sqlx_migrations
to :"runtime_role";
grant select, insert, update, delete on
    public.admin_audit_events,
    public.aggregation_recipes,
    public.client_profiles,
    public.gateway_async_operation_queue,
    public.gateway_operation_manual_reconciliations,
    public.gateway_rate_limit_keys,
    public.gateway_rate_limit_overflow,
    public.gateway_request_journal,
    public.moderation_content_decision_outbox,
    public.moderation_content_decision_replays,
    public.moderation_decisions,
    public.moderation_report_operation_receipts,
    public.moderation_reports,
    public.policy_sets,
    public.rollouts,
    public.route_contract_versions,
    public.route_contracts,
    public.telemetry_records,
    public.user_restrictions
to :"runtime_role";
