-- Service-owned selected structural requirements for external migration mode.
-- Metadata only: no business-table SELECT or protected function EXECUTE grant.
-- Receipts are checked separately in the same bounded read-only snapshot.
-- This is not exhaustive physical-schema drift or runtime privilege acceptance.
WITH required_tables(name) AS (
    VALUES ('notification_audit_events'),
        ('notification_recipient_suppressions'),
        ('notification_subject_states'),
        ('notification_user_deletion_receipts'),
        ('notifications')
), required_columns(relation, name, kind) AS (
    VALUES ('notifications', 'id', 'uuid'),
        ('notifications', 'event_id', 'uuid'),
        ('notifications', 'user_id', 'uuid'),
        ('notifications', 'actor_id', 'uuid'),
        ('notification_subject_states', 'subject_id', 'uuid'),
        ('notification_subject_states', 'suppress_as_actor', 'bool')
), required_triggers(relation, name, function_name, kind) AS (
    VALUES ('notification_recipient_suppressions', 'notification_legacy_suppression_state_sync', 'sync_notification_subject_state_from_legacy', 21),
        ('notifications', 'notification_subject_state_guard', 'enforce_notification_subject_state', 23)
), required_unique_indexes(relation, name, columns) AS (
    VALUES ('notifications', 'notifications_user_event_type_idx', ARRAY['user_id','event_id','type']::text[])
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
