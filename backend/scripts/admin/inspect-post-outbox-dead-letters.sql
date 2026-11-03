-- Run with: psql "$DATABASE_URL" --file scripts/admin/inspect-post-outbox-dead-letters.sql
-- Requires migrations through 0018. This snapshot grants no replay authority.
-- Use a read-only operator role; payloads, errors and lease tokens are omitted.
\set ON_ERROR_STOP on
BEGIN READ ONLY;
SET LOCAL statement_timeout = '10s';
\ir post-outbox-dead-letter-inspection.sql
COMMIT;
