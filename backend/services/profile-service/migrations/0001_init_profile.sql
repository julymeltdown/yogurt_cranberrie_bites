create table if not exists profiles (
    user_id uuid primary key,
    username text not null unique,
    nickname text not null,
    avatar_url text not null,
    bio text not null,
    follower_count bigint not null default 0,
    post_count bigint not null default 0,
    following_count bigint not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists avatars (
    user_id uuid primary key references profiles(user_id) on delete cascade,
    content bytea not null,
    content_type text not null,
    filename text not null default '',
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
