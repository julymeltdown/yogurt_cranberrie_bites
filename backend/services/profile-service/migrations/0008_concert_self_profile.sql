-- Additive canonical self-content concurrency. Existing nickname/bio remain the
-- only content store; all retained update paths participate through this trigger.
alter table profiles
    add column concert_self_epoch uuid not null default gen_random_uuid(),
    add column concert_self_revision bigint not null default 1,
    add constraint concert_self_epoch_nonnil check (concert_self_epoch <> '00000000-0000-0000-0000-000000000000'::uuid),
    add constraint concert_self_revision_range check (concert_self_revision between 1 and 9007199254740991);

create function concert_self_content_version() returns trigger language plpgsql as $$
begin
    new.concert_self_epoch := old.concert_self_epoch;
    new.concert_self_revision := old.concert_self_revision;
    if (new.nickname, new.bio, new.deleted_at) is distinct from (old.nickname, old.bio, old.deleted_at) then
        if old.concert_self_revision >= 9007199254740991 then
            raise exception 'Profile version exhausted' using errcode = '23514';
        end if;
        new.concert_self_revision := old.concert_self_revision + 1;
    end if;
    return new;
end $$;
create trigger concert_self_content_version before update on profiles
    for each row execute function concert_self_content_version();

-- Immutable control-plane identity survives an unknown PREPARE/COMMIT result.
-- Neither absence of a receipt nor request expiry is an abort decision.
create table concert_self_profile_operations (
    operation_id uuid primary key,
    actor_id uuid not null,
    event_id uuid not null,
    session_family_id uuid not null,
    request_id uuid not null,
    binding_digest bytea not null check (octet_length(binding_digest)=32),
    binding_proto bytea not null check (octet_length(binding_proto) between 1 and 32768),
    command_digest bytea not null check (octet_length(command_digest)=32),
    expected_epoch uuid not null,
    expected_revision bigint not null check (expected_revision between 1 and 9007199254740991),
    prepared_gid text not null unique check (prepared_gid ~ '^mcr_sp_[0-9a-f]{32}$'),
    full_xid text not null check (full_xid ~ '^[1-9][0-9]{0,19}$'),
    system_identifier text not null check (system_identifier ~ '^[1-9][0-9]{0,19}$'),
    database_oid bigint not null check (database_oid between 1 and 4294967295),
    backend_pid integer not null check (backend_pid>0),
    backend_started_at timestamptz not null,
    producer_instance_id uuid not null,
    effect_binding_digest bytea not null check (octet_length(effect_binding_digest)=32),
    created_at timestamptz not null default clock_timestamp(),
    check (operation_id=request_id),
    check (actor_id<>'00000000-0000-0000-0000-000000000000'::uuid and event_id<>'00000000-0000-0000-0000-000000000000'::uuid and session_family_id<>'00000000-0000-0000-0000-000000000000'::uuid),
    check (expected_epoch<>'00000000-0000-0000-0000-000000000000'::uuid and producer_instance_id<>'00000000-0000-0000-0000-000000000000'::uuid)
);
-- Inserted in the SAME actual prepared transaction as the selected content write.
-- This evidence cannot become visible until that exact effect commits.
create table concert_self_profile_receipts (
    operation_id uuid primary key references concert_self_profile_operations(operation_id),
    effect_binding_digest bytea not null check (octet_length(effect_binding_digest)=32),
    command_digest bytea not null check (octet_length(command_digest)=32),
    profile_epoch uuid not null,
    profile_revision bigint not null check (profile_revision between 1 and 9007199254740991),
    recorded_at timestamptz not null default clock_timestamp()
);
