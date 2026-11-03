-- Service-owned selected structural requirements for external migration mode.
-- Metadata only: no business-table SELECT or protected function EXECUTE grant.
-- Receipts are checked separately in the same bounded read-only snapshot.
-- This is not exhaustive physical-schema drift or runtime privilege acceptance.
WITH required_tables(name) AS (
    VALUES ('comments'),
        ('event_publications'),
        ('feed_affinity_capture_epoch'),
        ('feed_affinity_legacy_effect_observations'),
        ('feed_affinity_pair_baselines'),
        ('feed_affinity_pair_heads'),
        ('feed_affinity_pair_provenance'),
        ('feed_affinity_resolution_receipts'),
        ('feed_affinity_source_contributions'),
        ('feed_affinity_source_pairs'),
        ('feed_affinity_test_provisioning'),
        ('feed_affinity_write_permits'),
        ('feed_author_affinities'),
        ('feed_author_affinity'),
        ('feed_embeddings'),
        ('feed_experiment_assignments'),
        ('feed_fanout_jobs'),
        ('feed_impressions'),
        ('feed_projection_inbox'),
        ('feed_recent_actions'),
        ('feed_recommendation_events'),
        ('feed_recommendation_impressions'),
        ('feed_recommendation_model_readiness'),
        ('feed_recommendation_models'),
        ('feed_recommendation_retry_contexts'),
        ('feed_recommendation_rollouts'),
        ('feed_recommendation_snapshot_items'),
        ('feed_recommendation_snapshots'),
        ('feed_snapshot_items'),
        ('feed_snapshots'),
        ('high_fanout_posts'),
        ('home_timeline_entries'),
        ('post_audit_events'),
        ('post_authority_resource_witnesses'),
        ('post_engagement_velocity'),
        ('post_event_aggregate_sequences'),
        ('post_event_dead_letter_resolutions'),
        ('post_event_delivery_attempts'),
        ('post_event_delivery_jobs'),
        ('post_event_outbox'),
        ('post_likes'),
        ('post_moderation_actions'),
        ('post_mutation_receipts'),
        ('post_user_deletion_tombstones'),
        ('posts'),
        ('recommendation_model_registry'),
        ('recommendation_rollouts'),
        ('timeline_projection_cursors'),
        ('timeline_projection_receipts')
), required_columns(relation, name, kind) AS (
    VALUES ('posts', 'id', 'uuid'),
        ('posts', 'author_id', 'uuid'),
        ('post_event_outbox', 'event_id', 'uuid'),
        ('post_event_outbox', 'delivery_required_targets', 'text[]'),
        ('post_event_delivery_jobs', 'event_id', 'uuid'),
        ('post_event_delivery_jobs', 'lease_token', 'uuid'),
        ('post_event_delivery_jobs', 'state', 'text')
), required_triggers(relation, name, function_name, kind) AS (
    VALUES ('posts', 'posts_user_deletion_fence', 'post_guard_user_deletion_fence', 23),
        ('post_event_outbox', 'post_event_outbox_assign_aggregate_version', 'assign_post_event_aggregate_version', 7),
        ('post_user_deletion_tombstones', 'post_terminal_retirement', 'post_guard_terminal_retirement', 27),
        ('timeline_projection_receipts', 'post_projection_receipt_identity', 'post_guard_projection_receipt_identity', 19),
        ('feed_experiment_assignments', 'feed_assignment_owner_update', 'post_guard_owner_managed_recommendation', 19),
        ('feed_embeddings', 'feed_embedding_owner_update', 'post_guard_owner_managed_recommendation', 19)
), required_unique_indexes(relation, name, columns) AS (
    VALUES ('post_event_delivery_jobs', 'post_event_delivery_jobs_pkey', ARRAY['event_id','target']::text[])
)
SELECT NOT EXISTS (
    SELECT 1 FROM required_tables r
    WHERE NOT EXISTS (
        SELECT 1 FROM pg_catalog.pg_class c
        WHERE c.oid = pg_catalog.to_regclass('public.' || r.name)
          AND c.relkind = 'r'
    )
) AND NOT EXISTS (
    SELECT 1 FROM required_columns r
    WHERE NOT EXISTS (
        SELECT 1 FROM pg_catalog.pg_attribute a
        WHERE a.attrelid = pg_catalog.to_regclass('public.' || r.relation)
          AND a.attname = r.name AND a.attnum > 0 AND NOT a.attisdropped
          AND a.atttypid = pg_catalog.to_regtype('pg_catalog.' || r.kind)
    )
) AND NOT EXISTS (
    SELECT 1 FROM required_triggers r
    WHERE NOT EXISTS (
        SELECT 1 FROM pg_catalog.pg_trigger t
        JOIN pg_catalog.pg_proc f ON f.oid = t.tgfoid
        WHERE t.tgrelid = pg_catalog.to_regclass('public.' || r.relation)
          AND t.tgname = r.name AND NOT t.tgisinternal
          AND t.tgenabled IN ('O', 'A') AND t.tgtype = r.kind
          AND f.oid = pg_catalog.to_regprocedure('public.' || r.function_name || '()')
          AND f.prorettype = pg_catalog.to_regtype('pg_catalog.trigger')
    )
) AND NOT EXISTS (
    SELECT 1 FROM required_unique_indexes r
    WHERE NOT EXISTS (
        SELECT 1 FROM pg_catalog.pg_index i
        WHERE i.indrelid = pg_catalog.to_regclass('public.' || r.relation)
          AND i.indexrelid = pg_catalog.to_regclass('public.' || r.name)
          AND i.indisunique AND i.indisvalid AND i.indisready AND i.indimmediate
          AND i.indpred IS NULL AND i.indexprs IS NULL
          AND i.indnkeyatts = pg_catalog.cardinality(r.columns)
          AND ARRAY(
              SELECT a.attname::text
              FROM pg_catalog.unnest(i.indkey::smallint[]) WITH ORDINALITY AS k(attnum, ordinal)
              JOIN pg_catalog.pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = k.attnum
              WHERE k.ordinal <= i.indnkeyatts ORDER BY k.ordinal
          ) = r.columns
    )
);
