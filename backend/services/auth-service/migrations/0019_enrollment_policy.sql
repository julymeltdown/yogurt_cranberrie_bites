-- Canonical global MCR service consent. No policy or acceptance is seeded.
-- Operator publication is explicit and distinct from restricted-age evidence.
create table mcr_enrollment_policies (
    revision uuid primary key check (revision <> '00000000-0000-0000-0000-000000000000'),
    publication jsonb not null,
    published_at timestamptz not null,
    expires_at timestamptz not null check (expires_at > published_at)
);
create table mcr_enrollment_current (
    singleton boolean primary key check (singleton),
    revision uuid references mcr_enrollment_policies(revision)
);
insert into mcr_enrollment_current(singleton,revision) values(true,null);
create table mcr_enrollment_attempts (
    id uuid primary key,
    binding_hash bytea not null check(octet_length(binding_hash)=32),
    revision uuid not null references mcr_enrollment_policies(revision),
    created_at timestamptz not null,
    expires_at timestamptz not null check(expires_at>created_at),
    state text not null check(state in('STARTED','PENDING','SHADOW','VERIFIED')),
    user_id uuid references users(id) on delete cascade,
    signup_key uuid,
    request_hash bytea check(request_hash is null or octet_length(request_hash)=32),
    signup_receipt jsonb,
    verified_receipt jsonb,
    resend_available_at timestamptz not null,
    check((state='STARTED' and user_id is null and signup_key is null and request_hash is null and signup_receipt is null)
       or (state='SHADOW' and user_id is null and signup_key is not null and request_hash is not null and signup_receipt is not null)
       or (state in('PENDING','VERIFIED') and user_id is not null and signup_key is not null and request_hash is not null and signup_receipt is not null)),
    check((state='VERIFIED')=(verified_receipt is not null))
);
create index mcr_enrollment_attempts_expiry on mcr_enrollment_attempts(expires_at);
create table mcr_enrollment_acceptances (
    user_id uuid not null references users(id) on delete cascade,
    revision uuid not null references mcr_enrollment_policies(revision),
    acknowledgement_ids text[] not null,
    attempt_id uuid references mcr_enrollment_attempts(id) on delete set null,
    accepted_at timestamptz not null,
    primary key(user_id,revision)
);
create table mcr_enrollment_resend_requests (
    attempt_id uuid not null references mcr_enrollment_attempts(id) on delete cascade,
    idempotency_key uuid not null,
    receipt jsonb not null,
    primary key(attempt_id,idempotency_key)
);
create table mcr_enrollment_consent_requests (
    user_id uuid not null references users(id) on delete cascade,
    idempotency_key uuid not null,
    revision uuid not null references mcr_enrollment_policies(revision),
    receipt jsonb not null,
    primary key(user_id,idempotency_key)
);
