-- WP-02 T-101: durable profile scope lifecycle for the profile product transport.
--
-- A profile request acquires one owner scope, publishes one delivery capability,
-- runs its producer, and is settled only by the Gateway's separate
-- `CompleteResponse` after its own HTTP body terminal. This table is the durable
-- record of that lifecycle, so a lost reply leaves the scope visibly unresolved
-- instead of silently completed. It is the profile analogue of the concert
-- `concert_product_owner_scopes` ledger and is keyed by the profile, never an event.
create table profile_product_scopes (
    instance_id uuid not null check (instance_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    scope_id uuid not null check (scope_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    binding_digest bytea not null check (octet_length(binding_digest) = 32
                                         and binding_digest <> decode(repeat('00',32),'hex')),
    actor_id uuid not null check (actor_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    -- The profile this scope is bound to. There is deliberately no event column.
    profile_id uuid not null check (profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    operation text not null check (char_length(operation) between 1 and 100),
    http_method text not null check (http_method in ('GET','POST','PATCH','PUT','DELETE')),
    -- Profile operations never carry an event path.
    http_target text not null check (char_length(http_target) between 1 and 4096
                                     and http_target like '/api/%'
                                     and position('/events/' in http_target) = 0),
    phase text not null check (phase in ('ACTIVE','SEALED','ROOT_FINISHED')),
    delivery_secret_hash bytea check (delivery_secret_hash is null
                                      or octet_length(delivery_secret_hash) = 32),
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    completed_at timestamptz,
    primary key (instance_id, scope_id),
    -- A scope cannot be finished without a published delivery capability.
    constraint profile_product_scopes_terminal_requires_delivery
        check (phase <> 'ROOT_FINISHED' or delivery_secret_hash is not null),
    constraint profile_product_scopes_completion_time
        check ((phase = 'ROOT_FINISHED') = (completed_at is not null)),
    constraint profile_product_scopes_times
        check (updated_at >= created_at)
);

create index profile_product_scopes_unresolved
    on profile_product_scopes (created_at)
    where phase <> 'ROOT_FINISHED';

create function profile_product_scopes_guard() returns trigger language plpgsql as $$
begin
    if tg_op = 'DELETE' then
        raise exception 'profile scope lifecycle is immutable'
            using errcode = '23514', constraint = 'profile_product_scopes_immutable';
    end if;
    if (new.instance_id, new.scope_id, new.binding_digest, new.actor_id, new.profile_id,
        new.operation, new.http_method, new.http_target, new.created_at)
       is distinct from
       (old.instance_id, old.scope_id, old.binding_digest, old.actor_id, old.profile_id,
        old.operation, old.http_method, old.http_target, old.created_at) then
        raise exception 'profile scope binding is immutable'
            using errcode = '23514', constraint = 'profile_product_scopes_binding_immutable';
    end if;
    -- ACTIVE -> SEALED -> ROOT_FINISHED, or ACTIVE -> ROOT_FINISHED directly.
    if not ((old.phase = 'ACTIVE' and new.phase in ('SEALED','ROOT_FINISHED'))
            or (old.phase = 'SEALED' and new.phase = 'ROOT_FINISHED')
            or (old.phase = new.phase and new.phase = 'ROOT_FINISHED')) then
        raise exception 'profile scope phase cannot regress'
            using errcode = '23514', constraint = 'profile_product_scopes_phase';
    end if;
    -- The delivery secret is written once and never replaced.
    if old.delivery_secret_hash is not null
       and new.delivery_secret_hash is distinct from old.delivery_secret_hash then
        raise exception 'profile delivery capability is immutable'
            using errcode = '23514', constraint = 'profile_product_scopes_delivery_immutable';
    end if;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger profile_product_scopes_guard
    before update or delete on profile_product_scopes
    for each row execute function profile_product_scopes_guard();
