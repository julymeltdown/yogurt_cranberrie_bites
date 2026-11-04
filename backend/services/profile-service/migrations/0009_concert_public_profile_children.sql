-- Owner-local control receipts only. No Profile content or plaintext capability.
-- Independently reserved before PREPARE so a lost read reply remains recoverable.
create table public.concert_public_profile_children (
    child_id text primary key check (child_id ~ '^pp:[A-Za-z0-9_-]{107}:[0-9a-f]{32}$'),
    completion_digest text not null check (completion_digest ~ '^[A-Za-z0-9_-]{43}$'),
    system_identifier text not null check (system_identifier ~ '^[1-9][0-9]*$'),
    database_oid bigint not null check (database_oid > 0),
    owner_oid bigint not null check (owner_oid > 0),
    full_xid text check (full_xid ~ '^[1-9][0-9]*$'),
    state text not null check (state in ('REGISTERED','CANCELLED','SETTLED')),
    created_at timestamptz not null default clock_timestamp(),
    settled_at timestamptz,
    check ((state = 'CANCELLED' and full_xid is null) or
           (state in ('REGISTERED','SETTLED') and full_xid is not null)),
    check ((state = 'REGISTERED') = (settled_at is null))
);
-- No TTL, absence-based settlement or automatic receipt deletion is permitted.
