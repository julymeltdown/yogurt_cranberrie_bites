-- Gift owns its child progress; the shared category coordinator is a separate owner.
create table concert_gift_account_deletion_progress (
    user_id uuid primary key,
    saga_id uuid not null,
    command_binding bytea not null check (octet_length(command_binding)=32),
    source_binding bytea not null check (octet_length(source_binding)=32),
    identity_binding bytea not null check (octet_length(identity_binding)=32),
    fence_revision bigint not null check (fence_revision>0),
    revision bigint not null check (revision>0),
    complete boolean not null default false
);
create function concert_gift_deletion_progress_guard() returns trigger language plpgsql as $$
begin
    if TG_OP='DELETE' then raise exception 'Gift deletion progress is retained' using errcode='23000'; end if;
    if TG_OP='UPDATE' and (OLD.complete or
        (NEW.user_id,NEW.saga_id,NEW.command_binding,NEW.source_binding,NEW.identity_binding,NEW.fence_revision)
        is distinct from
        (OLD.user_id,OLD.saga_id,OLD.command_binding,OLD.source_binding,OLD.identity_binding,OLD.fence_revision)
        or NEW.revision<>OLD.revision+1) then
        raise exception 'Gift deletion identity or terminal receipt changed' using errcode='23000';
    end if;
    if not exists(select 1 from concert_world_account_deletion_fences f where
        f.user_id=NEW.user_id and f.saga_id=NEW.saga_id and f.identity_binding=NEW.identity_binding
        and f.fence_revision=NEW.fence_revision) then
        raise exception 'Gift deletion common fence mismatch' using errcode='23000';
    end if;
    return NEW;
end $$;
create trigger concert_gift_deletion_progress_guard before insert or update or delete
on concert_gift_account_deletion_progress for each row execute function concert_gift_deletion_progress_guard();

create function concert_gift_deletion_deadline_guard() returns trigger language plpgsql as $$
declare until_time timestamptz;
begin
    until_time:=nullif(current_setting('mcr.gift_deletion_until',true),'')::timestamptz;
    if until_time is null or clock_timestamp()>=until_time then
        raise exception 'Gift deletion deadline expired' using errcode='23514';
    end if;
    return null;
end $$;
create constraint trigger concert_gift_deletion_deadline_guard after insert or update
on concert_gift_account_deletion_progress deferrable initially deferred
for each row execute function concert_gift_deletion_deadline_guard();

-- These contain resource identifiers only, never the withdrawn identity or a
-- replacement profile. Counts and terminal claim/receipt facts remain retained.
create table concert_gift_withdrawn_roots (
    event_id uuid not null,
    giveaway_id uuid not null,
    primary key(event_id,giveaway_id),
    foreign key(event_id,giveaway_id) references concert_community_giveaways(event_id,giveaway_id)
);
create table concert_gift_withdrawn_reservations (
    event_id uuid not null,
    reservation_id uuid not null,
    primary key(event_id,reservation_id),
    foreign key(event_id,reservation_id) references concert_community_reservations(event_id,reservation_id)
);
