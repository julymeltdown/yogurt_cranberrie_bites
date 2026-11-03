-- An accepted asynchronous mutation must have an encrypted, durable command
-- payload and a restart-recoverable lease. Only the Gateway's short-lived
-- application keyring can decrypt payload bytes; the journal remains the
-- source of actor, idempotency, and operation identity.
create table gateway_async_operation_queue (
    operation_id uuid primary key references gateway_request_journal(id) on delete restrict,
    request_uri text not null check (octet_length(request_uri) between 1 and 2048),
    content_type text null check (content_type is null or octet_length(content_type) <= 256),
    encryption_key_id smallint not null check (encryption_key_id > 0),
    payload_nonce bytea null check (payload_nonce is null or octet_length(payload_nonce) = 12),
    payload_ciphertext bytea null check (
        payload_ciphertext is null or octet_length(payload_ciphertext) between 16 and 2097168
    ),
    state text not null default 'READY' check (state in ('READY', 'LEASED', 'DONE', 'DEAD')),
    available_at timestamptz not null,
    attempt_count integer not null default 0 check (attempt_count >= 0),
    lease_owner uuid null,
    lease_expires_at timestamptz null,
    last_error_code text null check (last_error_code is null or octet_length(last_error_code) <= 64),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (
        (state in ('READY', 'LEASED') and payload_nonce is not null and payload_ciphertext is not null)
        or (state in ('DONE', 'DEAD') and payload_nonce is null and payload_ciphertext is null)
    ),
    check (
        (state = 'LEASED' and lease_owner is not null and lease_expires_at is not null)
        or (state <> 'LEASED' and lease_owner is null and lease_expires_at is null)
    )
);

create index gateway_async_operation_queue_ready_idx
    on gateway_async_operation_queue (available_at, operation_id)
    where state = 'READY';

create index gateway_async_operation_queue_lease_idx
    on gateway_async_operation_queue (lease_expires_at, operation_id)
    where state = 'LEASED';
