-- WP-04b follow-up: durable profile scope lifecycle for the concert-hosted
-- profile product transport.
--
-- Fifteen frozen profile operations read tables that live in this database
-- (`profile_posts`, `profile_post_attachments`, `profile_media`,
-- `profile_rooms`, `profile_avatars`). The profile service runs against a
-- different database and `domain-contract.md` §1.1 forbids a cross-database
-- join, so the Gateway routes those operations here and this service serves
-- them under its own authority discipline.
--
-- This table is the durable record of that discipline. It is the exact analogue
-- of the profile service's `profile_product_scopes`
-- (`backend/services/profile-service/migrations/0014_profile_product_scopes.sql`),
-- with two differences that matter:
--
--  * it lives in the concert database, because the scopes it records are held
--    by this process; and
--  * it is keyed by `profile_id`, never an event. A profile operation is not
--    event-scoped, and the concert product's own
--    `concert_product_owner_scopes` requires an event binding, so it cannot be
--    reused here.
--
-- A scope that was registered and published but never settled stays visible as
-- unresolved instead of being silently completed: a lost reply must not look
-- like a finished request.
create table concert_profile_product_scopes (
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
    constraint concert_profile_product_scopes_terminal_requires_delivery
        check (phase <> 'ROOT_FINISHED' or delivery_secret_hash is not null),
    constraint concert_profile_product_scopes_completion_time
        check ((phase = 'ROOT_FINISHED') = (completed_at is not null)),
    constraint concert_profile_product_scopes_times
        check (updated_at >= created_at)
);

create index concert_profile_product_scopes_unresolved
    on concert_profile_product_scopes (created_at)
    where phase <> 'ROOT_FINISHED';

-- The binding fingerprint is the identity of one exact request. A second
-- registration of the same digest under a different scope id would be a
-- duplicate, so it is refused by the primary key plus this lookup index.
create index concert_profile_product_scopes_binding
    on concert_profile_product_scopes (binding_digest);

-- A scope transitions forward only, and only once per phase. The lifecycle is
-- ACTIVE (registered, no delivery) -> SEALED (delivery published, producer may
-- run) -> ROOT_FINISHED (the Gateway reported its actual HTTP body terminal).
-- Evidence is immutable: no update may rewrite the binding or the creation
-- time, and no row may be deleted.
create function concert_profile_product_scope_transition() returns trigger
language plpgsql as $$
begin
    if tg_op = 'DELETE' then
        raise exception 'profile scope lifecycle evidence is immutable'
            using errcode = '23514', constraint = 'concert_profile_product_scopes_no_delete';
    end if;
    if (new.instance_id, new.scope_id, new.binding_digest, new.actor_id, new.profile_id,
        new.operation, new.http_method, new.http_target, new.created_at)
       is distinct from
       (old.instance_id, old.scope_id, old.binding_digest, old.actor_id, old.profile_id,
        old.operation, old.http_method, old.http_target, old.created_at) then
        raise exception 'profile scope binding is immutable'
            using errcode = '23514', constraint = 'concert_profile_product_scopes_immutable_binding';
    end if;
    if not coalesce(
        -- Publication: ACTIVE -> SEALED, once, and only with a secret.
        (old.phase = 'ACTIVE' and new.phase = 'SEALED'
         and old.delivery_secret_hash is null and new.delivery_secret_hash is not null
         and new.completed_at is null)
        -- Settlement: SEALED -> ROOT_FINISHED, keeping the published secret.
        or (old.phase = 'SEALED' and new.phase = 'ROOT_FINISHED'
            and new.delivery_secret_hash = old.delivery_secret_hash
            and new.completed_at is not null),
        false) then
        raise exception 'invalid profile scope lifecycle transition'
            using errcode = '23514', constraint = 'concert_profile_product_scopes_transition';
    end if;
    return new;
end $$;

create trigger concert_profile_product_scope_transition
    before update or delete on concert_profile_product_scopes
    for each row execute function concert_profile_product_scope_transition();

-- Scope evidence is never truncated wholesale, exactly as the profile-owned
-- claim tables refuse it.
create function concert_profile_product_scopes_no_truncate() returns trigger
language plpgsql as $$
begin
    raise exception 'profile scope lifecycle evidence cannot be truncated'
        using errcode = '23514', constraint = 'concert_profile_product_scopes_no_truncate';
end $$;

create trigger concert_profile_product_scopes_no_truncate
    before truncate on concert_profile_product_scopes
    for each statement execute function concert_profile_product_scopes_no_truncate();
