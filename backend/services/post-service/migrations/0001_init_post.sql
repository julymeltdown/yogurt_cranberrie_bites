create table if not exists posts (
    id uuid primary key,
    author_id uuid not null,
    content text not null,
    visibility text not null,
    created_at timestamptz not null,
    comment_count bigint not null default 0,
    like_count bigint not null default 0
);

create table if not exists comments (
    id uuid primary key,
    post_id uuid not null references posts(id) on delete cascade,
    author_id uuid not null,
    content text not null,
    created_at timestamptz not null
);

create table if not exists post_likes (
    post_id uuid not null references posts(id) on delete cascade,
    user_id uuid not null,
    created_at timestamptz not null,
    primary key (post_id, user_id)
);

create table if not exists post_event_outbox (
    event_id uuid primary key,
    event_type text not null,
    aggregate_id uuid not null,
    actor_id uuid null,
    attributes text[] not null default '{}',
    payload text not null,
    occurred_at timestamptz not null,
    created_at timestamptz not null,
    published boolean not null default false,
    published_at timestamptz null
);

create table if not exists event_publications (
    id bigserial primary key,
    event_id uuid not null,
    target text not null,
    published_at timestamptz not null,
    payload text not null
);

create index if not exists posts_author_created_idx on posts (author_id, created_at desc, id desc);
create index if not exists posts_trending_idx on posts (like_count desc, created_at desc, id desc);
create index if not exists comments_post_created_idx on comments (post_id, created_at asc, id asc);
create index if not exists outbox_created_idx on post_event_outbox (created_at asc, event_id asc);
