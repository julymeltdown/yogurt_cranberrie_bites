-- Ephemeral membership observations for admitted request stages. No actor
-- tokens, post content or provider credentials are stored. This migration does
-- not activate forwarding or change any existing operational credentials.
create table post_authority_resource_witnesses (
    stage_id uuid primary key check (stage_id <> '00000000-0000-0000-0000-000000000000'),
    parent_digest bytea not null check (octet_length(parent_digest) = 32),
    request_digest bytea not null check (octet_length(request_digest) = 32),
    actor_id uuid not null check (actor_id <> '00000000-0000-0000-0000-000000000000'),
    root_kind text not null check (root_kind in (
        'FEED','SEARCH','TRENDING','POST','CREATE_COMMENT','LIKE','UNLIKE',
        'DELETE_POST','NOTIFICATION_POSTS','REPLAY_POST_MUTATION')),
    root_resources uuid[] not null,
    purpose text not null check (purpose in (
        'RANKED_CANDIDATES','CHRONOLOGICAL_PAGE','SEARCH_BATCH','TRENDING_PAGE','SNAPSHOT_PAGE','FIXED_POSTS')),
    query_digest bytea not null check (octet_length(query_digest) = 32),
    batch_sequence bigint not null check (batch_sequence between 0 and 4294967295),
    post_ids uuid[] not null,
    author_ids uuid[] not null,
    missing_post_ids uuid[] not null,
    created_at timestamptz not null,
    parent_expires_at timestamptz not null check (parent_expires_at > created_at
        and parent_expires_at <= created_at + interval '65 seconds'),
    check (cardinality(root_resources) <= 100),
    check (cardinality(post_ids) = cardinality(author_ids)),
    check (cardinality(post_ids) + cardinality(missing_post_ids) <= 4096),
    check (array_position(root_resources, null) is null
        and array_position(post_ids, null) is null
        and array_position(author_ids, null) is null
        and array_position(missing_post_ids, null) is null),
    check ((root_kind='FEED' and purpose in ('RANKED_CANDIDATES','CHRONOLOGICAL_PAGE','SNAPSHOT_PAGE'))
        or (root_kind='SEARCH' and purpose='SEARCH_BATCH')
        or (root_kind='TRENDING' and purpose='TRENDING_PAGE')
        or (root_kind in ('POST','CREATE_COMMENT','LIKE','UNLIKE','DELETE_POST','NOTIFICATION_POSTS','REPLAY_POST_MUTATION')
            and purpose='FIXED_POSTS'))
);
create index post_authority_resource_witnesses_expiry_idx
    on post_authority_resource_witnesses(parent_expires_at, stage_id);

-- A stage is never updated or extended. Expired rows can be removed by the
-- bounded cleanup capability; lookup always checks expiry independently.
create function post_authority_resource_witness_immutable() returns trigger
language plpgsql as $$
begin
    raise exception 'Authority witness stages are immutable'
        using errcode='23514', constraint='post_authority_resource_witness_immutable';
end;
$$;
create trigger post_authority_resource_witness_immutable
    before update on post_authority_resource_witnesses
    for each row execute function post_authority_resource_witness_immutable();
