-- Admission by route class with explicit overflow.
--
-- gateway_take_rate_limit (0004) answered only true or false and refused every
-- identity it had not seen once a bucket held 256 live identities. One IPv6
-- network or botnet filling a bucket therefore locked every new client in that
-- bucket out for the whole window, and the caller could not tell a spent quota
-- from a full table.
--
-- gateway_admit keeps the rule that a live identity is never evicted (eviction
-- would reset its remaining quota) and changes what happens when a bucket is
-- full: an unseen identity may be admitted from one shared overflow budget per
-- route class, without getting a row of its own. Churning identities can spend
-- that budget, and only that budget. The answer names the outcome so the
-- Gateway can count quota, capacity, and overflow separately.
--
-- The old function is kept so a rolled-back Gateway keeps working; its keys
-- carry no class prefix, so the two never share a budget row.

create table gateway_rate_limit_overflow (
    route_class text primary key check (octet_length(route_class) between 1 and 32),
    request_count integer not null check (request_count > 0),
    expires_at timestamptz not null
);

create function gateway_admit(
    p_key text,
    p_class text,
    p_limit integer,
    p_window integer,
    p_bucket_capacity integer,
    p_overflow_limit integer
) returns text language plpgsql as $$
declare
    bucket_id integer;
    requests integer;
    live integer;
    v_now timestamptz;
begin
    if p_key is null or p_class is null or p_limit is null or p_window is null
       or p_bucket_capacity is null or p_overflow_limit is null
       or octet_length(p_key) not between 1 and 128
       or octet_length(p_class) not between 1 and 32
       or p_limit < 1 or p_window not between 1 and 86400
       or p_bucket_capacity < 0 or p_overflow_limit < 0 then
        return 'quota';
    end if;
    -- A slow bucket is a store failure the caller handles by route class, not
    -- a wait that holds the request.
    perform set_config('lock_timeout', '150ms', true);
    bucket_id := (hashtextextended(p_key, 0) & 63)::integer;
    perform pg_advisory_xact_lock(724006, bucket_id);
    v_now := clock_timestamp();
    delete from gateway_rate_limit_keys where bucket = bucket_id and expires_at <= v_now;

    select request_count into requests from gateway_rate_limit_keys where client_key = p_key;
    if found then
        if requests >= p_limit then
            return 'quota';
        end if;
        update gateway_rate_limit_keys set request_count = request_count + 1
        where client_key = p_key;
        return 'admitted';
    end if;

    select count(*) into live from gateway_rate_limit_keys where bucket = bucket_id;
    if live < p_bucket_capacity then
        insert into gateway_rate_limit_keys (client_key, bucket, request_count, expires_at)
        values (p_key, bucket_id, 1, v_now + p_window * interval '1 second');
        return 'admitted';
    end if;

    -- The bucket is full. The identity gets no row, so it gains nothing it could
    -- keep; it may only spend the class's shared overflow budget.
    if p_overflow_limit < 1 then
        return 'cardinality';
    end if;
    perform pg_advisory_xact_lock(724007, hashtext(p_class));
    delete from gateway_rate_limit_overflow where route_class = p_class and expires_at <= v_now;
    insert into gateway_rate_limit_overflow as overflow (route_class, request_count, expires_at)
    values (p_class, 1, v_now + p_window * interval '1 second')
    on conflict (route_class) do update
        set request_count = overflow.request_count + 1
        where overflow.request_count < p_overflow_limit
    returning overflow.request_count into requests;
    if found then
        return 'overflow';
    end if;
    return 'cardinality';
end;
$$;
