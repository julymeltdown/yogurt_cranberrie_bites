alter table profiles
    add column if not exists account_visibility text not null default 'PUBLIC';

alter table profiles
    drop constraint if exists profiles_account_visibility_check;
alter table profiles
    add constraint profiles_account_visibility_check
    check (account_visibility in ('PUBLIC', 'PRIVATE'));

create index if not exists profiles_visibility_active_idx
    on profiles (account_visibility, username)
    where deleted_at is null;
