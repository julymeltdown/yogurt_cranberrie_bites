create table if not exists route_contracts (
    id text primary key,
    route_key text not null unique,
    owner text not null,
    sla_target text,
    status text not null,
    description text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists route_contract_versions (
    contract_id text not null references route_contracts(id) on delete cascade,
    version text not null,
    status text not null,
    deprecation_date timestamptz,
    response_schema_ref text not null,
    primary key (contract_id, version)
);

create table if not exists policy_sets (
    id text primary key,
    name text not null,
    rate_limit text,
    quota text,
    timeout_ms integer,
    retry_policy text,
    circuit_breaker_policy text,
    cache_policy text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists client_profiles (
    id text primary key,
    client_type text not null,
    allowed_contracts text not null,
    policy_set_id text references policy_sets(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists aggregation_recipes (
    id text primary key,
    route_key text not null unique,
    downstream_calls text not null,
    join_strategy text not null,
    timeout_ms integer,
    cache_policy text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists rollouts (
    id text primary key,
    contract_id text not null references route_contracts(id) on delete cascade,
    version text not null,
    strategy text not null,
    status text not null,
    percent integer,
    created_at timestamptz not null default now()
);

create table if not exists telemetry_records (
    id text primary key,
    correlation_id text not null,
    route_key text not null,
    client_profile_id text,
    outcome text not null,
    latency_ms bigint not null,
    error_code text,
    downstream_latencies text not null,
    recorded_at timestamptz not null
);
