create table if not exists oauth_authorizations (
    state text primary key,
    provider_type text not null,
    redirect_uri text not null,
    code_verifier text not null,
    code_challenge text not null,
    nonce text not null,
    expires_at timestamptz not null,
    consumed_at timestamptz null,
    created_at timestamptz not null default now()
);

create index if not exists oauth_authorizations_provider_state_idx
    on oauth_authorizations (provider_type, state)
    where consumed_at is null;
create index if not exists oauth_authorizations_expires_idx
    on oauth_authorizations (expires_at)
    where consumed_at is null;
