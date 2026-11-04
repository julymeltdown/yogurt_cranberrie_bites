-- Service-owned selected structural requirements for external migration mode.
-- Metadata only: no business-table SELECT or protected function EXECUTE grant.
-- Receipts are checked separately in the same bounded read-only snapshot.
-- This is not exhaustive physical-schema drift or runtime privilege acceptance.
WITH required_tables(name) AS (
    VALUES ('auth_audit_events'),
        ('auth_mail_outbox'),
        ('auth_user_roles'),
        ('auth_user_tombstones'),
        ('email_verifications'),
        ('login_audit'),
        ('oauth_authorizations'),
        ('password_reset_tokens'),
        ('refresh_tokens'),
        ('user_deletion_retry_resolutions'),
        ('user_deletion_sagas'),
        ('user_deletion_steps'),
        ('user_identities'),
        ('users'),
        ('mcr_enrollment_policies'),
        ('mcr_enrollment_current'),
        ('mcr_enrollment_attempts'),
        ('mcr_enrollment_acceptances'),
        ('mcr_enrollment_resend_requests'),
        ('mcr_enrollment_consent_requests')
), required_columns(relation, name, kind) AS (
    VALUES ('users', 'id', 'uuid'),
        ('users', 'primary_email', 'text'),
        ('users', 'status', 'text'),
        ('users', 'deleted_at', 'timestamptz'),
        ('refresh_tokens', 'jti', 'text'),
        ('refresh_tokens', 'user_id', 'uuid'),
        ('auth_mail_outbox', 'id', 'uuid'),
        ('auth_user_tombstones', 'user_id', 'uuid'),
        ('mcr_enrollment_policies','revision','uuid'),
        ('mcr_enrollment_policies','publication','jsonb'),
        ('mcr_enrollment_policies','expires_at','timestamptz'),
        ('mcr_enrollment_current','revision','uuid'),
        ('mcr_enrollment_attempts','binding_hash','bytea'),
        ('mcr_enrollment_attempts','signup_receipt','jsonb'),
        ('mcr_enrollment_acceptances','user_id','uuid'),
        ('mcr_enrollment_acceptances','revision','uuid'),
        ('mcr_enrollment_resend_requests','receipt','jsonb'),
        ('mcr_enrollment_consent_requests','receipt','jsonb')
), required_triggers(relation, name, function_name, kind) AS (
    VALUES ('users', 'auth_retire_user_id', 'auth_retire_user_id', 11),
        ('users', 'auth_reject_retired_user_id', 'auth_reject_retired_user_id', 7),
        ('refresh_tokens', 'auth_validate_new_session', 'auth_validate_new_session', 7),
        ('users', 'auth_revoke_changed_credentials', 'auth_revoke_changed_credentials', 17),
        ('auth_user_roles', 'auth_revoke_changed_roles', 'auth_revoke_changed_roles', 29)
), required_unique_indexes(relation, name, columns) AS (
    VALUES ('users', 'users_pkey', ARRAY['id']::text[])
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
