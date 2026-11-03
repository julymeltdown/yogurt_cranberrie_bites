-- Service-owned selected structural requirements for external migration mode.
-- Metadata only: no business-table SELECT or protected function EXECUTE grant.
-- Receipts are checked separately in the same bounded read-only snapshot.
-- This is not exhaustive physical-schema drift or runtime privilege acceptance.
WITH required_tables(name) AS (
    VALUES ('follow_requests'),
        ('follows'),
        ('friend_audit_events'),
        ('friend_mutation_receipts'),
        ('friend_user_deletion_tombstones'),
        ('people_embeddings'),
        ('people_experiment_assignments'),
        ('people_recommendation_candidates'),
        ('people_recommendation_events'),
        ('people_recommendation_impressions'),
        ('people_recommendation_model_readiness'),
        ('people_recommendation_models'),
        ('people_recommendation_refresh_jobs'),
        ('people_recommendation_rollouts'),
        ('people_recommendation_snapshot_items'),
        ('people_recommendation_snapshots'),
        ('user_blocks'),
        ('user_mutes')
), required_columns(relation, name, kind) AS (
    VALUES ('follows', 'follower_id', 'uuid'),
        ('follows', 'followee_id', 'uuid'),
        ('friend_user_deletion_tombstones', 'user_id', 'uuid')
), required_triggers(relation, name, function_name, kind) AS (
    VALUES ('follows', 'follows_user_deletion_fence', 'friend_guard_user_deletion_fence', 23),
        ('follow_requests', 'follow_requests_user_deletion_fence', 'friend_guard_user_deletion_fence', 23),
        ('user_blocks', 'user_blocks_user_deletion_fence', 'friend_guard_user_deletion_fence', 23),
        ('user_mutes', 'user_mutes_user_deletion_fence', 'friend_guard_user_deletion_fence', 23),
        ('people_experiment_assignments', 'people_assignment_owner_update', 'friend_guard_owner_managed_recommendation', 19),
        ('people_embeddings', 'people_embedding_owner_update', 'friend_guard_owner_managed_recommendation', 19)
), required_unique_indexes(relation, name, columns) AS (
    VALUES ('follows', 'follows_pkey', ARRAY['follower_id','followee_id']::text[])
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
