create table if not exists users (
    id uuid primary key,
    primary_email text not null unique,
    password_hash text not null,
    status text not null,
    last_login_at timestamptz
);

create table if not exists user_identities (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    provider_type text not null,
    email text not null,
    email_verified boolean not null default false,
    linked_at timestamptz not null default now(),
    unique (user_id, provider_type, email)
);

create table if not exists email_verifications (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    email text not null,
    code text not null,
    expires_at timestamptz not null
);

create table if not exists refresh_tokens (
    jti text primary key,
    token_hash text not null,
    user_id uuid not null references users(id) on delete cascade,
    expires_at timestamptz not null,
    revoked_at timestamptz
);

create table if not exists login_audit (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    ip_address text,
    user_agent text,
    success boolean not null,
    occurred_at timestamptz not null
);

create table if not exists password_reset_tokens (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    token_hash text not null,
    expires_at timestamptz not null
);
