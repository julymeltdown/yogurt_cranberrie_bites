-- Expose PostgreSQL advisory-lock wait separately from pool/query latency.
-- Keep the original text-returning function as a wrapper so an older Gateway
-- binary can still run after this migration during a rolling deploy/rollback.

create function gateway_admit_with_lock_wait(
    p_key text,
    p_class text,
    p_limit integer,
    p_window integer,
    p_bucket_capacity integer,
    p_overflow_limit integer
) returns table (outcome text, lock_wait_seconds double precision)
language plpgsql as $$
declare
    bucket_id integer;
    requests integer;
    live integer;
    v_now timestamptz;
    v_lock_started timestamptz;
    v_lock_wait_seconds double precision := 0;
begin
    if p_key is null or p_class is null or p_limit is null or p_window is null
       or p_bucket_capacity is null or p_overflow_limit is null
       or octet_length(p_key) not between 1 and 128
       or octet_length(p_class) not between 1 and 32
       or p_limit < 1 or p_window not between 1 and 86400
       or p_bucket_capacity < 0 or p_overflow_limit < 0 then
        outcome := 'quota';
        lock_wait_seconds := v_lock_wait_seconds;
        return next;
        return;
    end if;
    perform set_config('lock_timeout', '150ms', true);
    bucket_id := (hashtextextended(p_key, 0) & 63)::integer;

    v_lock_started := clock_timestamp();
    perform pg_advisory_xact_lock(724006, bucket_id);
    v_lock_wait_seconds := v_lock_wait_seconds
        + extract(epoch from clock_timestamp() - v_lock_started)::double precision;
    v_now := clock_timestamp();
    delete from gateway_rate_limit_keys where bucket = bucket_id and expires_at <= v_now;

    select request_count into requests from gateway_rate_limit_keys where client_key = p_key;
    if found then
        if requests >= p_limit then
            outcome := 'quota';
            lock_wait_seconds := v_lock_wait_seconds;
            return next;
            return;
        end if;
        update gateway_rate_limit_keys set request_count = request_count + 1
        where client_key = p_key;
        outcome := 'admitted';
        lock_wait_seconds := v_lock_wait_seconds;
        return next;
        return;
    end if;

    select count(*) into live from gateway_rate_limit_keys where bucket = bucket_id;
    if live < p_bucket_capacity then
        insert into gateway_rate_limit_keys (client_key, bucket, request_count, expires_at)
        values (p_key, bucket_id, 1, v_now + p_window * interval '1 second');
        outcome := 'admitted';
        lock_wait_seconds := v_lock_wait_seconds;
        return next;
        return;
    end if;

    if p_overflow_limit < 1 then
        outcome := 'cardinality';
        lock_wait_seconds := v_lock_wait_seconds;
        return next;
        return;
    end if;

    v_lock_started := clock_timestamp();
    perform pg_advisory_xact_lock(724007, hashtext(p_class));
    v_lock_wait_seconds := v_lock_wait_seconds
        + extract(epoch from clock_timestamp() - v_lock_started)::double precision;
    delete from gateway_rate_limit_overflow where route_class = p_class and expires_at <= v_now;
    insert into gateway_rate_limit_overflow as overflow (route_class, request_count, expires_at)
    values (p_class, 1, v_now + p_window * interval '1 second')
    on conflict (route_class) do update
        set request_count = overflow.request_count + 1
        where overflow.request_count < p_overflow_limit
    returning overflow.request_count into requests;
    if found then
        outcome := 'overflow';
    else
        outcome := 'cardinality';
    end if;
    lock_wait_seconds := v_lock_wait_seconds;
    return next;
end;
$$;

create or replace function gateway_admit(
    p_key text,
    p_class text,
    p_limit integer,
    p_window integer,
    p_bucket_capacity integer,
    p_overflow_limit integer
) returns text language plpgsql as $$
declare
    v_outcome text;
begin
    select timed.outcome into v_outcome
    from gateway_admit_with_lock_wait(
        p_key, p_class, p_limit, p_window, p_bucket_capacity, p_overflow_limit
    ) as timed;
    return v_outcome;
end;
$$;
