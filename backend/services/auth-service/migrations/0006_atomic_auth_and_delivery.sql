-- Keep historical migrations unchanged. All additions are local to auth_db.
alter table email_verifications add column attempts integer not null default 0 check (attempts >= 0);
create index email_verifications_user_expiry_idx on email_verifications (user_id, expires_at desc);
create index password_reset_tokens_hash_idx on password_reset_tokens (token_hash);
create index refresh_tokens_active_user_idx on refresh_tokens (user_id) where revoked_at is null;

create table auth_user_tombstones (
    user_id uuid primary key,
    retired_at timestamptz not null default now()
);

create function auth_retire_user_id() returns trigger language plpgsql as $$
begin
    perform pg_advisory_xact_lock(hashtextextended(old.id::text, 71917));
    insert into auth_user_tombstones (user_id) values (old.id) on conflict do nothing;
    return old;
end;
$$;
create trigger auth_retire_user_id before delete on users
for each row execute function auth_retire_user_id();

create function auth_reject_retired_user_id() returns trigger language plpgsql as $$
begin
    perform pg_advisory_xact_lock(hashtextextended(new.id::text, 71917));
    if exists (select 1 from auth_user_tombstones where user_id = new.id) then
        raise exception 'User ID permanently retired' using errcode = '23514';
    end if;
    return new;
end;
$$;
create trigger auth_reject_retired_user_id before insert on users
for each row execute function auth_reject_retired_user_id();

create table auth_user_roles (
    user_id uuid not null references users(id) on delete cascade,
    role text not null check (role in ('ROLE_USER', 'ROLE_ADMIN')),
    granted_by uuid not null,
    granted_at timestamptz not null default now(),
    primary key (user_id, role)
);

create function auth_validate_new_session() returns trigger language plpgsql as $$
begin
    perform 1 from users
    where id = new.user_id and status = 'ACTIVE' and deleted_at is null
    for update;
    if not found then
        raise exception 'User not active' using errcode = '23514';
    end if;
    return new;
end;
$$;
create trigger auth_validate_new_session before insert on refresh_tokens
for each row execute function auth_validate_new_session();

create function auth_revoke_changed_credentials() returns trigger language plpgsql as $$
begin
    if new.password_hash is distinct from old.password_hash
       or new.status is distinct from old.status
       or new.deleted_at is distinct from old.deleted_at then
        update refresh_tokens set revoked_at = coalesce(revoked_at, now())
        where user_id = new.id and revoked_at is null;
    end if;
    return new;
end;
$$;
create trigger auth_revoke_changed_credentials after update on users
for each row execute function auth_revoke_changed_credentials();

create function auth_revoke_changed_roles() returns trigger language plpgsql as $$
declare
    subject_id uuid;
begin
    if tg_op = 'DELETE' then subject_id := old.user_id;
    else subject_id := new.user_id;
    end if;
    perform 1 from users where id = subject_id for update;
    update refresh_tokens set revoked_at = coalesce(revoked_at, now())
    where user_id = subject_id and revoked_at is null;
    if tg_op = 'DELETE' then return old; end if;
    return new;
end;
$$;
create trigger auth_revoke_changed_roles after insert or update or delete on auth_user_roles
for each row execute function auth_revoke_changed_roles();

create table auth_mail_outbox (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    kind text not null check (kind in ('VERIFY_EMAIL', 'PASSWORD_RESET')),
    payload_json text,
    created_at timestamptz not null default now(),
    expires_at timestamptz not null,
    next_attempt_at timestamptz not null default now(),
    attempts integer not null default 0 check (attempts >= 0),
    lease_id uuid,
    leased_until timestamptz,
    delivered_at timestamptz,
    dead_lettered_at timestamptz,
    last_error_code text
);
create index auth_mail_outbox_due_idx on auth_mail_outbox (next_attempt_at, created_at)
where delivered_at is null and dead_lettered_at is null;
create index auth_mail_outbox_subject_idx on auth_mail_outbox (user_id, kind, created_at desc);
