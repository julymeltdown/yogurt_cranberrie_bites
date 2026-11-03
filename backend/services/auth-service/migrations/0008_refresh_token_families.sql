-- Refresh tokens become a family lineage so a rotation race can be told apart
-- from genuine token reuse.
--
-- Before this, rotation set revoked_at on the predecessor and *every* unusual
-- presentation (missing, mismatched, or lost race) revoked every session the user
-- had. Two browser tabs refreshing at once therefore logged the user out
-- everywhere, and presenting any already-revoked token was a global-logout
-- primitive available to anyone holding one stale token.
--
-- The family columns let a service distinguish three different situations:
--   * active          -> rotate
--   * superseded      -> a rotation race (benign) or reuse (attack)
--   * ended by logout -> a stale tab, not an attack
alter table refresh_tokens
    add column if not exists family_id uuid,
    add column if not exists rotated_at timestamptz,
    add column if not exists successor_jti text,
    add column if not exists revoked_reason text;

-- Existing rows become single-token families. A token that already has
-- revoked_at set was revoked for a reason this migration cannot reconstruct, so
-- it is recorded as revoked without a family-wide consequence.
--
-- jti is text and family_id is uuid, so a derived value is needed. md5() is used
-- rather than gen_random_uuid() because the latter requires PostgreSQL 13+ and
-- this image is 11. A digest of the jti is unique per token, deterministic, and
-- needs no extension, which keeps the migration portable across supported
-- versions and re-runnable against a restored database.
update refresh_tokens
set family_id = md5(jti)::uuid
where family_id is null;

alter table refresh_tokens
    alter column family_id set not null;

create index if not exists refresh_tokens_family_idx
    on refresh_tokens (family_id);

-- The reuse check looks up a family's live descendants by time, and the grace
-- window compares against rotated_at for the immediate predecessor.
create index if not exists refresh_tokens_rotated_idx
    on refresh_tokens (family_id, rotated_at desc)
    where rotated_at is not null;
