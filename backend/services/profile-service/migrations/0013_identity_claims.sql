-- WP-02 T-105: provider-neutral identity claim store.
--
-- This is the donor's `naver_verifications` model, renamed to a provider-neutral
-- shape and moved out of the concert service. The donor table was event-scoped
-- (`concert_age_claims` is keyed by `(user_id, event_id)` and its event cleanup
-- can erase a user's verification when a concert ends). A personal profile must
-- not inherit that, so this table is keyed by `user_id` only and has no
-- `event_id` column anywhere.
--
-- Every constraint below is named so a negative test can assert the exact
-- constraint that fires rather than "some error occurred".

create table identity_claims (
    claim_id uuid primary key check (claim_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    user_id uuid not null check (user_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    verifier text not null,
    source_revision text not null,
    -- Salted hash of the provider subject: the same NAVER account cannot back
    -- two users. The raw subject is never stored.
    subject_hash bytea not null,
    gender_bucket text not null,
    age_at_verification smallint,
    next_birthday_at date,
    verified_at timestamptz not null,
    expires_at timestamptz not null,
    status text not null,
    created_at timestamptz not null default clock_timestamp(),
    constraint identity_claims_verifier_shape
        check (verifier ~ '^[a-z][a-z0-9._-]{0,63}$'),
    constraint identity_claims_source_revision_shape
        check (source_revision ~ '^[A-Za-z0-9._-]{1,100}$'),
    constraint identity_claims_subject_hash_length
        check (octet_length(subject_hash) = 32),
    constraint identity_claims_gender_bucket
        check (gender_bucket in ('female','male','unknown')),
    constraint identity_claims_age_range
        check (age_at_verification is null or age_at_verification between 14 and 100),
    constraint identity_claims_status
        check (status in ('VERIFIED','REPLACED','REVOKED')),
    -- A live claim must carry the attributes the audience decision needs.
    constraint identity_claims_verified_requires_attributes
        check (status <> 'VERIFIED'
               or (age_at_verification is not null and next_birthday_at is not null)),
    -- A claim lives at most 90 days and never ends before it starts.
    constraint identity_claims_lifetime
        check (expires_at > verified_at and expires_at <= verified_at + interval '90 days'),
    -- The next birthday is strictly after the Seoul calendar date of
    -- verification, so the derived age is never already stale on arrival.
    constraint identity_claims_next_birthday_future
        check (next_birthday_at is null
               or next_birthday_at > (verified_at at time zone 'Asia/Seoul')::date)
);

-- At most one live claim per user. Re-verification replaces the old row's status
-- before the new row may become VERIFIED.
create unique index identity_claims_one_verified
    on identity_claims (user_id) where status = 'VERIFIED';

-- Canonical selection order, exactly as the donor does it: newest verified_at,
-- then newest expires_at, then newest claim_id. Never fall back to an older
-- usable claim.
create index identity_claims_canonical
    on identity_claims (user_id, verified_at desc, expires_at desc, claim_id desc);

-- The same provider subject may not back two different users.
create unique index identity_claims_subject
    on identity_claims (verifier, subject_hash);

-- One-use OAuth state. The donor's `naver_verifications` is the model: a state
-- digest is stored, the raw state token never is, and the row expires within ten
-- minutes of creation.
create table identity_verifications (
    verification_id uuid primary key check (verification_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    user_id uuid not null check (user_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    verifier text not null,
    source_revision text not null,
    state text not null,
    state_digest bytea not null,
    return_path text not null,
    created_at timestamptz not null default clock_timestamp(),
    expires_at timestamptz not null,
    completed_at timestamptz,
    constraint identity_verifications_verifier_shape
        check (verifier ~ '^[a-z][a-z0-9._-]{0,63}$'),
    constraint identity_verifications_source_revision_shape
        check (source_revision ~ '^[A-Za-z0-9._-]{1,100}$'),
    constraint identity_verifications_state_digest_length
        check (octet_length(state_digest) = 32),
    constraint identity_verifications_state
        check (state in ('PENDING','RESERVED','EXCHANGING','DONE','EXPIRED')),
    -- Only a same-origin absolute path is a valid return target.
    constraint identity_verifications_return_path
        check (return_path ~ '^/(?!/)[A-Za-z0-9._~!$&''()*+,;=:@%/-]*$'
               and char_length(return_path) <= 512),
    constraint identity_verifications_expiry
        check (expires_at > created_at and expires_at <= created_at + interval '10 minutes'),
    constraint identity_verifications_completion
        check (completed_at is null or (state = 'DONE' and completed_at <= expires_at))
);

create unique index identity_verifications_state_digest
    on identity_verifications (verifier, state_digest);

create index identity_verifications_pending
    on identity_verifications (user_id, expires_at, verification_id)
    where state <> 'EXPIRED';

-- Claims are private identity evidence. They are never truncated wholesale.
create function identity_claims_no_truncate() returns trigger language plpgsql as $$
begin
    raise exception 'identity claim history cannot be truncated'
        using errcode = '23514', constraint = 'identity_claims_no_truncate';
end $$;

create trigger identity_claims_no_truncate
    before truncate on identity_claims
    for each statement execute function identity_claims_no_truncate();

create trigger identity_verifications_no_truncate
    before truncate on identity_verifications
    for each statement execute function identity_claims_no_truncate();

-- The verified attributes are immutable once written. A re-verification writes a
-- new row; it never rewrites the evidence of an earlier one.
create function identity_claims_immutable_evidence() returns trigger language plpgsql as $$
begin
    if (new.claim_id, new.user_id, new.verifier, new.source_revision, new.subject_hash,
        new.gender_bucket, new.age_at_verification, new.next_birthday_at,
        new.verified_at, new.created_at)
       is distinct from
       (old.claim_id, old.user_id, old.verifier, old.source_revision, old.subject_hash,
        old.gender_bucket, old.age_at_verification, old.next_birthday_at,
        old.verified_at, old.created_at) then
        raise exception 'identity claim evidence is immutable'
            using errcode = '23514', constraint = 'identity_claims_immutable_evidence';
    end if;
    -- A claim can only leave VERIFIED. It can never be revived.
    if old.status <> 'VERIFIED' and new.status = 'VERIFIED' then
        raise exception 'identity claim cannot be revived'
            using errcode = '23514', constraint = 'identity_claims_no_revival';
    end if;
    return new;
end $$;

create trigger identity_claims_immutable_evidence
    before update on identity_claims
    for each row execute function identity_claims_immutable_evidence();
