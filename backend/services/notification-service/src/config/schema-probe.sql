-- Service-owned selected structural requirements for external migration mode.
-- Metadata only: no business-table SELECT or protected function EXECUTE grant.
-- Receipts are checked separately in the same bounded read-only snapshot.
-- This is not exhaustive physical-schema drift or runtime privilege acceptance.
WITH required_tables(name) AS (
    VALUES ('notification_audit_events'),
        ('notification_recipient_suppressions'),
        ('notification_subject_states'),
        ('notification_user_deletion_receipts'),
        ('notifications'),
        ('notification_deletion_fences_v1'),
        ('notification_deletion_receipts_v1')
), required_columns(relation, name, kind) AS (
    VALUES ('notifications', 'id', 'uuid'),
        ('notifications', 'event_id', 'uuid'),
        ('notifications', 'user_id', 'uuid'),
        ('notifications', 'actor_id', 'uuid'),
        ('notification_subject_states', 'subject_id', 'uuid'),
        ('notification_subject_states', 'suppress_as_actor', 'bool'),
        ('notification_deletion_fences_v1', 'subject_id', 'uuid'),
        ('notification_deletion_fences_v1', 'fence_revision', 'int8'),
        ('notification_deletion_fences_v1', 'established_xid', 'xid8'),
        ('notification_deletion_receipts_v1', 'saga_id', 'uuid'),
        ('notification_deletion_receipts_v1', 'user_id', 'uuid'),
        ('notification_deletion_receipts_v1', 'actor_id', 'uuid'),
        ('notification_deletion_receipts_v1', 'retention_until', 'timestamptz'),
        ('notification_deletion_receipts_v1', 'retention_seconds', 'int8'),
        ('notification_deletion_receipts_v1', 'retention_nanos', 'int4'),
        ('notification_deletion_receipts_v1', 'fence_revision', 'int8'),
        ('notification_deletion_receipts_v1', 'audit_event_id', 'uuid'),
        ('notification_deletion_receipts_v1', 'effect_xid', 'xid8')
), required_triggers(relation, name, function_name, kind) AS (
    VALUES ('notification_recipient_suppressions', 'notification_legacy_suppression_state_sync', 'sync_notification_subject_state_from_legacy', 21),
        ('notifications', 'notification_subject_state_guard', 'enforce_notification_subject_state', 23),
        ('notification_deletion_fences_v1', 'notification_deletion_fence_immutable_v1', 'notification_deletion_proof_immutable_v1', 58),
        ('notification_deletion_receipts_v1', 'notification_deletion_receipt_immutable_v1', 'notification_deletion_proof_immutable_v1', 58),
        ('notification_subject_states', 'notification_deletion_subject_guard_v1', 'notification_deletion_subject_guard_v1', 27),
        ('notification_subject_states', 'notification_deletion_subject_truncate_v1', 'notification_deletion_subject_guard_v1', 34),
        ('notification_audit_events', 'notification_deletion_audit_guard_v1', 'notification_deletion_link_guard_v1', 27),
        ('notification_audit_events', 'notification_deletion_audit_truncate_v1', 'notification_deletion_link_guard_v1', 34),
        ('notification_user_deletion_receipts', 'notification_deletion_legacy_guard_v1', 'notification_deletion_link_guard_v1', 27),
        ('notification_user_deletion_receipts', 'notification_deletion_legacy_truncate_v1', 'notification_deletion_link_guard_v1', 34),
        ('notifications', 'notification_deletion_recreation_guard_v1', 'notification_deletion_recreation_guard_v1', 19),
        ('notification_deletion_fences_v1', 'notification_deletion_fence_insert_v1', 'notification_deletion_fence_insert_v1', 7),
        ('notification_deletion_receipts_v1', 'notification_deletion_receipt_commit_v1', 'notification_deletion_receipt_commit_v1', 5)
), required_unique_indexes(relation, name, columns) AS (
    VALUES ('notifications', 'notifications_user_event_type_idx', ARRAY['user_id','event_id','type']::text[]),
        ('notification_deletion_fences_v1', 'notification_deletion_fences_v1_pkey', ARRAY['subject_id']::text[]),
        ('notification_deletion_fences_v1', 'notification_deletion_fences_v1_fence_revision_key', ARRAY['fence_revision']::text[]),
        ('notification_deletion_fences_v1', 'notification_deletion_fences_v1_subject_id_fence_revision_key', ARRAY['subject_id','fence_revision']::text[]),
        ('notification_deletion_receipts_v1', 'notification_deletion_receipts_v1_pkey', ARRAY['saga_id']::text[]),
        ('notification_deletion_receipts_v1', 'notification_deletion_receipts_v1_audit_event_id_key', ARRAY['audit_event_id']::text[])
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
)
AND EXISTS (
    SELECT 1 FROM pg_catalog.pg_attribute a
    WHERE a.attrelid = pg_catalog.to_regclass('public.notification_deletion_fences_v1')
      AND a.attname = 'fence_revision' AND a.attidentity = 'a' AND a.attnotnull
)
AND EXISTS (
    SELECT 1 FROM pg_catalog.pg_trigger t
    WHERE t.tgrelid = pg_catalog.to_regclass('public.notification_deletion_receipts_v1')
      AND t.tgname = 'notification_deletion_receipt_commit_v1'
      AND t.tgdeferrable AND t.tginitdeferred
)
AND NOT EXISTS (
    SELECT 1 FROM (VALUES
        ('notification_deletion_fences_v1', 'notification_subject_states'),
        ('notification_deletion_receipts_v1', 'notification_deletion_fences_v1'),
        ('notification_deletion_receipts_v1', 'notification_audit_events')
    ) AS required(relation, referenced)
    WHERE NOT EXISTS (
        SELECT 1 FROM pg_catalog.pg_constraint c
        WHERE c.conrelid = pg_catalog.to_regclass('public.' || required.relation)
          AND c.confrelid = pg_catalog.to_regclass('public.' || required.referenced)
          AND c.contype = 'f' AND c.convalidated
          AND c.confupdtype = 'a' AND c.confdeltype = 'a'
    )
);
