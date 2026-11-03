-- Apply as the schema owner to a fresh, dedicated non-owner login, using:
-- psql --set=ON_ERROR_STOP=1 --single-transaction --set=runtime_role=ROLE --file=runtime-grants.sql
-- Role/database provisioning is separate. No grants to future objects or
-- controller-only functions are implied. Keep this allowlist with migrations.
grant usage on schema public to :"runtime_role";
grant select on
    public._sqlx_migrations
to :"runtime_role";
grant select, insert on
    public.friend_user_deletion_tombstones
to :"runtime_role";
grant select, insert, update, delete on
    public.follow_requests,
    public.follows,
    public.friend_audit_events,
    public.friend_mutation_receipts,
    public.people_recommendation_candidates,
    public.people_recommendation_events,
    public.people_recommendation_impressions,
    public.people_recommendation_refresh_jobs,
    public.people_recommendation_snapshot_items,
    public.people_recommendation_snapshots,
    public.user_blocks,
    public.user_mutes
to :"runtime_role";

-- Model publication belongs to the owner, not the serving identity.
grant select on public.people_recommendation_models, public.people_recommendation_model_readiness, public.people_recommendation_rollouts to :"runtime_role";
grant select, delete on public.people_experiment_assignments, public.people_embeddings to :"runtime_role";
-- Row-lock privilege for bounded retention; no population privilege.
grant update (viewer_id) on public.people_experiment_assignments to :"runtime_role";
grant update (subject_id) on public.people_embeddings to :"runtime_role";
