-- Gift native identity and safe receipt snapshots are distinct evidence.
-- No pickup code, meeting text, request body or permission boolean is stored.
create table concert_gift_native_witnesses (
    producer_instance uuid not null,
    scope_id uuid not null,
    identity_digest bytea not null check (octet_length(identity_digest)=32),
    binding_digest bytea not null check (octet_length(binding_digest)=32),
    native_identity jsonb not null check (jsonb_typeof(native_identity)='object'),
    created_at timestamptz not null default clock_timestamp(),
    primary key(producer_instance,scope_id,identity_digest),
    foreign key(producer_instance,scope_id,identity_digest)
        references concert_product_owner_transactions(instance_id,scope_id,identity_digest)
);
create table concert_gift_receipt_witnesses (
    producer_instance uuid not null,
    scope_id uuid not null,
    identity_digest bytea not null,
    receipt_id uuid not null references concert_command_receipts(receipt_id),
    receipt_snapshot jsonb not null check (jsonb_typeof(receipt_snapshot)='object'),
    created_at timestamptz not null default clock_timestamp(),
    primary key(producer_instance,scope_id,identity_digest),
    foreign key(producer_instance,scope_id,identity_digest)
        references concert_gift_native_witnesses(producer_instance,scope_id,identity_digest)
);

create function concert_gift_native_witness_guard() returns trigger language plpgsql as $$
declare
    registered jsonb;
    actual_binding bytea;
    confirmed timestamptz;
begin
    if TG_OP <> 'INSERT' then
        raise exception 'Gift native evidence is immutable' using errcode='23000';
    end if;
    select identity,binding_digest,confirmed_at into registered,actual_binding,confirmed
        from concert_product_owner_transactions
        where instance_id=NEW.producer_instance and scope_id=NEW.scope_id
            and identity_digest=NEW.identity_digest;
    if registered is null or confirmed is null or registered <> NEW.native_identity
        or actual_binding <> NEW.binding_digest
        or registered->>'producer' <> 'CONCERT'
        or (registered->>'producer_instance')::uuid <> NEW.producer_instance
        or registered->>'system_identifier' <> (pg_control_system()).system_identifier::text
        or (registered->>'database_oid')::bigint <> (select oid::bigint from pg_database where datname=current_database())
        or registered->>'full_xid' <> pg_current_xact_id()::text
        or (registered->>'backend_pid')::int <> pg_backend_pid()
        or (registered->>'backend_started_unix_micros')::bigint <>
            (select (extract(epoch from backend_start)*1000000)::bigint from pg_stat_activity where pid=pg_backend_pid()) then
        raise exception 'Gift native identity mismatch' using errcode='23000';
    end if;
    return NEW;
end $$;
create trigger concert_gift_native_witness_guard before insert or update or delete
    on concert_gift_native_witnesses for each row execute function concert_gift_native_witness_guard();

create function concert_gift_receipt_witness_guard() returns trigger language plpgsql as $$
declare
    expected jsonb;
    native jsonb;
    parent_binding jsonb;
    reference jsonb;
begin
    if TG_OP <> 'INSERT' then
        raise exception 'Gift receipt evidence is immutable' using errcode='23000';
    end if;
    select native_identity into native from concert_gift_native_witnesses
        where producer_instance=NEW.producer_instance and scope_id=NEW.scope_id and identity_digest=NEW.identity_digest;
    if native is null or native->>'full_xid' <> pg_current_xact_id()::text
        or (native->>'backend_pid')::int <> pg_backend_pid() then
        raise exception 'Gift receipt transaction mismatch' using errcode='23000';
    end if;
    select binding into parent_binding from concert_product_owner_scopes
        where instance_id=NEW.producer_instance and scope_id=NEW.scope_id;
    -- Decode the exact registered byte-array reference, never a caller-selected
    -- receipt identity. The SDK validates its bounded canonical shape upstream.
    select convert_from(decode(string_agg(lpad(to_hex(value::int),2,'0'),'' order by ordinal),'hex'),'UTF8')::jsonb
        into reference from jsonb_array_elements_text(native->'settlement_reference') with ordinality as bytes(value,ordinal);
    if reference is null or parent_binding is null
        or reference->>'kind' is distinct from 'GIFT_REQUEST_V1'
        or reference->>'actor_id' is distinct from parent_binding->>'actor_id'
        or reference->>'event_id' is distinct from parent_binding->>'event_id'
        or reference->>'request_id' is distinct from parent_binding->>'request_id'
        or reference->>'operation_id' is distinct from parent_binding->>'operation'
        or NEW.receipt_snapshot->>'actor_id' is distinct from reference->>'actor_id'
        or NEW.receipt_snapshot->>'event_id' is distinct from reference->>'event_id'
        or NEW.receipt_snapshot->>'operation_id' is distinct from reference->>'operation_id'
        or NEW.receipt_snapshot->>'idempotency_key' is distinct from reference->>'idempotency_key' then
        raise exception 'Gift receipt parent mismatch' using errcode='23000';
    end if;
    select jsonb_build_object('receipt_id',receipt_id,'actor_id',actor_id,'event_id',event_id,
        'operation_id',operation_id,'idempotency_key',idempotency_key,'target_id',target_id,
        'state',state,'resource_id',resource_id,'resource_revision',resource_revision,
        'result_status',result_status,'failure_reason',failure_reason,
        -- Rust Chrono uses RFC3339; compare timestamps below at DB precision.
        'created_at',NEW.receipt_snapshot->'created_at','expires_at',NEW.receipt_snapshot->'expires_at') into expected
        from concert_command_receipts where receipt_id=NEW.receipt_id
        and created_at=(NEW.receipt_snapshot->>'created_at')::timestamptz
        and expires_at=(NEW.receipt_snapshot->>'expires_at')::timestamptz;
    if expected is null or expected <> NEW.receipt_snapshot then
        raise exception 'Gift receipt snapshot mismatch' using errcode='23000';
    end if;
    return NEW;
end $$;
create trigger concert_gift_receipt_witness_guard before insert or update or delete
    on concert_gift_receipt_witnesses for each row execute function concert_gift_receipt_witness_guard();
