-- Additive canonical Post-owned MCR event content. No legacy event backfill.
create table post_concert_posts (
    post_id uuid primary key,
    event_id uuid not null,
    author_id uuid not null,
    content jsonb not null,
    revision bigint not null check (revision between 1 and 9007199254740991),
    state text not null check (state in ('PUBLISHED','HIDDEN','DELETED')),
    created_at timestamptz not null,
    edited_at timestamptz,
    comment_count bigint not null default 0 check (comment_count between 0 and 2147483647),
    reaction_count bigint not null default 0 check (reaction_count between 0 and 2147483647),
    unique (event_id, post_id),
    check (jsonb_typeof(content) = 'object')
);
create index post_concert_page on post_concert_posts(event_id,created_at desc,post_id desc)
    where state='PUBLISHED';
create table post_concert_comments (
    comment_id uuid primary key,
    event_id uuid not null,
    post_id uuid not null,
    author_id uuid not null,
    text text not null check (char_length(text) between 1 and 1000),
    revision bigint not null check (revision between 1 and 9007199254740991),
    state text not null check (state in ('PUBLISHED','HIDDEN','DELETED')),
    created_at timestamptz not null,
    edited_at timestamptz,
    foreign key (event_id,post_id) references post_concert_posts(event_id,post_id)
);
create index post_concert_comments_page on post_concert_comments(event_id,post_id,created_at,comment_id)
    where state='PUBLISHED';
create table post_concert_reactions (
    event_id uuid not null,
    post_id uuid not null,
    actor_id uuid not null,
    created_at timestamptz not null,
    primary key(post_id,actor_id),
    foreign key(event_id,post_id) references post_concert_posts(event_id,post_id)
);
-- Tombstones must outlive content. Keys and bodies are never logged or returned.
create table post_concert_receipts (
    operation_id uuid primary key,
    actor_id uuid not null,
    event_id uuid not null,
    post_id uuid not null,
    comment_id uuid,
    key_digest bytea not null check(octet_length(key_digest)=32),
    command_digest bytea not null check(octet_length(command_digest)=32),
    original_authority bytea not null check(octet_length(original_authority)=32),
    original_deadline timestamptz not null,
    outcome jsonb not null,
    committed_at timestamptz not null,
    unique(actor_id,event_id,key_digest)
);
create table post_concert_outbox (
    operation_id uuid primary key references post_concert_receipts(operation_id),
    event_id uuid not null,
    post_id uuid not null,
    post_revision bigint not null,
    kind text not null,
    payload jsonb not null,
    occurred_at timestamptz not null,
    published_at timestamptz,
    unique(post_id,post_revision)
);
-- No mutation may transplant author, event, parent or chronological position.
create function post_concert_identity_immutable() returns trigger language plpgsql as $$
begin
    if new.event_id <> old.event_id or new.author_id <> old.author_id
       or new.created_at <> old.created_at then
        raise exception 'Post concert identity is immutable' using errcode='23514';
    end if;
    if tg_table_name='post_concert_posts' then
        if new.post_id <> old.post_id then
            raise exception 'Post concert object is immutable' using errcode='23514';
        end if;
    else
        if new.post_id <> old.post_id or new.comment_id <> old.comment_id then
            raise exception 'Post concert parent is immutable' using errcode='23514';
        end if;
    end if;
    return new;
end;
$$;
create trigger post_concert_post_identity before update on post_concert_posts
    for each row execute function post_concert_identity_immutable();
create trigger post_concert_comment_identity before update on post_concert_comments
    for each row execute function post_concert_identity_immutable();
