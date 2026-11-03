-- Apply as the schema owner to a fresh, dedicated non-owner login, using:
-- psql --set=ON_ERROR_STOP=1 --single-transaction --set=runtime_role=ROLE --file=runtime-grants.sql
-- Role/database provisioning is separate. No grants to future objects or
-- controller-only functions are implied. Keep this allowlist with migrations.
grant usage on schema public to :"runtime_role";
grant select on
    public._sqlx_migrations,
    public.feed_affinity_capture_epoch,
    public.feed_affinity_legacy_effect_observations,
    public.feed_affinity_pair_baselines,
    public.feed_affinity_pair_heads,
    public.feed_affinity_pair_provenance,
    public.feed_affinity_resolution_receipts,
    public.feed_affinity_source_contributions,
    public.feed_affinity_source_pairs,
    public.feed_recommendation_retry_contexts
to :"runtime_role";
grant select, insert on
    public.post_user_deletion_tombstones,
    public.timeline_projection_receipts
to :"runtime_role";
grant select, insert, update, delete on
    public.comments,
    public.event_publications,
    public.feed_author_affinities,
    public.feed_author_affinity,
    public.feed_fanout_jobs,
    public.feed_impressions,
    public.feed_projection_inbox,
    public.feed_recent_actions,
    public.feed_recommendation_events,
    public.feed_recommendation_impressions,
    public.feed_recommendation_snapshot_items,
    public.feed_recommendation_snapshots,
    public.feed_snapshot_items,
    public.feed_snapshots,
    public.high_fanout_posts,
    public.home_timeline_entries,
    public.post_audit_events,
    public.post_authority_resource_witnesses,
    public.post_engagement_velocity,
    public.post_event_aggregate_sequences,
    public.post_event_dead_letter_resolutions,
    public.post_event_delivery_attempts,
    public.post_event_delivery_jobs,
    public.post_event_outbox,
    public.post_likes,
    public.post_moderation_actions,
    public.post_mutation_receipts,
    public.posts,
    public.timeline_projection_cursors
to :"runtime_role";
grant usage, select on sequence public.event_publications_id_seq to :"runtime_role";
-- Service-called capabilities; issuance/permits/capture internals stay private.
grant execute on function public.feed_affinity_forget_subject(uuid),
    public.feed_affinity_resolve_ordered(text,uuid,uuid,uuid,uuid,integer,text),
    public.feed_recommendation_receipt_matches(public.feed_recommendation_events),
    public.feed_affinity_apply_legacy_effect(text,uuid,uuid,uuid,uuid,uuid,text,timestamptz,float8,float8),
    public.post_timeline_affinity_ready(uuid)
to :"runtime_role";

-- Model publication belongs to the owner, not the serving identity.
grant select on public.feed_recommendation_models, public.feed_recommendation_model_readiness, public.feed_recommendation_rollouts, public.recommendation_model_registry, public.recommendation_rollouts to :"runtime_role";
grant select, delete on public.feed_experiment_assignments, public.feed_embeddings to :"runtime_role";
-- Row-lock privilege for bounded retention; no population privilege.
grant update (viewer_id) on public.feed_experiment_assignments to :"runtime_role";
grant update (subject_id) on public.feed_embeddings to :"runtime_role";
