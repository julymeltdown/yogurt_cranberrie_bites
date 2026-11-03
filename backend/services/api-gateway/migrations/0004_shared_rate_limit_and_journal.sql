create table gateway_rate_limit_keys (
    client_key text primary key check (octet_length(client_key) between 1 and 128),
    bucket integer not null check (bucket between 0 and 63),
    request_count integer not null check (request_count > 0),
    expires_at timestamptz not null
);
create index gateway_rate_limit_bucket_expiry_idx on gateway_rate_limit_keys (bucket, expires_at);

-- 64 independently serialized buckets, at most 256 live identities each.
-- Never evict a live identity: doing so would reset its remaining quota.
create function gateway_take_rate_limit(p_key text, p_limit integer, p_window integer)
returns boolean language plpgsql as $$
declare
    bucket_id integer;
    requests integer;
    v_now timestamptz;
begin
    if p_key is null or p_limit is null or p_window is null
       or octet_length(p_key) not between 1 and 128 or p_limit < 1
       or p_window not between 1 and 86400 then return false; end if;
    bucket_id := (hashtextextended(p_key, 0) & 63)::integer;
    perform pg_advisory_xact_lock(724006, bucket_id);
    v_now := clock_timestamp();
    delete from gateway_rate_limit_keys where bucket = bucket_id and expires_at <= v_now;
    select request_count into requests from gateway_rate_limit_keys where client_key = p_key;
    if found then
        if requests >= p_limit then return false; end if;
        update gateway_rate_limit_keys set request_count = request_count + 1 where client_key = p_key;
        return true;
    end if;
    if (select count(*) from gateway_rate_limit_keys where bucket = bucket_id) >= 256 then return false; end if;
    insert into gateway_rate_limit_keys (client_key, bucket, request_count, expires_at)
    values (p_key, bucket_id, 1, v_now + p_window * interval '1 second');
    return true;
end;
$$;

-- Commit intent before dispatch. Interrupted operations require reconciliation.
create table gateway_request_journal (
    id uuid primary key,
    actor_id text,
    action text not null,
    resource_path text not null,
    request_id text,
    ip_address text,
    user_agent text,
    idempotency_key text check (octet_length(idempotency_key) between 1 and 128),
    request_fingerprint text,
    state text not null default 'IN_PROGRESS' check (state in ('IN_PROGRESS', 'COMPLETED', 'UNKNOWN')),
    response_status integer,
    response_content_type text,
    response_body bytea,
    created_at timestamptz not null default now(),
    finished_at timestamptz,
    expires_at timestamptz not null default now() + interval '24 hours',
    check (response_body is null or octet_length(response_body) <= 2097152),
    check (idempotency_key is null or actor_id is not null)
);
create unique index gateway_request_idempotency_idx
    on gateway_request_journal (actor_id, idempotency_key) where idempotency_key is not null;
create index gateway_request_journal_expiry_idx on gateway_request_journal (expires_at);
create index gateway_request_journal_pending_idx on gateway_request_journal (created_at)
    where state = 'IN_PROGRESS';
