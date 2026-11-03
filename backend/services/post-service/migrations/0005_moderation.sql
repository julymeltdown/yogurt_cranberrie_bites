alter table posts
    add column if not exists moderation_hidden_at timestamptz null,
    add column if not exists moderation_report_id uuid null,
    add column if not exists moderation_actor_id uuid null;

alter table comments
    add column if not exists moderation_hidden_at timestamptz null,
    add column if not exists moderation_report_id uuid null,
    add column if not exists moderation_actor_id uuid null;

create table if not exists post_moderation_actions (
    report_id uuid not null,
    action text not null,
    target_type text not null,
    target_id uuid not null,
    actor_id uuid not null,
    occurred_at timestamptz not null default now(),
    primary key (report_id, action)
);

create index if not exists posts_visible_created_idx
    on posts (created_at desc, id desc)
    where deleted_at is null and moderation_hidden_at is null;

create index if not exists comments_visible_post_created_idx
    on comments (post_id, created_at asc, id asc)
    where deleted_at is null and moderation_hidden_at is null;

create index if not exists post_moderation_actions_target_idx
    on post_moderation_actions (target_type, target_id, occurred_at desc);
