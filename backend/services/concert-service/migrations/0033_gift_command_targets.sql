-- Gift-live intent is installed by the request-bound, canonically guarded
-- adapter. It is consumed only on receipt admission, on the SAME transaction.
-- Existing headless/unwrapped producers are not silently promoted to live.
create table concert_gift_command_targets (
    actor_id uuid not null check (actor_id <> '00000000-0000-0000-0000-000000000000'),
    event_id uuid not null check (event_id <> '00000000-0000-0000-0000-000000000000'),
    operation_id text not null check (operation_id in (
        'createConcertGiveaway','updateConcertGiveaway','setConcertPickupMeeting',
        'reserveConcertGiveaway','cancelConcertReservation','issueConcertPickupCode',
        'claimConcertReservation')),
    idempotency_key uuid not null check (idempotency_key <> '00000000-0000-0000-0000-000000000000'),
    target_id uuid not null check (target_id <> '00000000-0000-0000-0000-000000000000'),
    giveaway_id uuid not null check (giveaway_id <> '00000000-0000-0000-0000-000000000000'),
    receipt_id uuid not null unique references concert_command_receipts(receipt_id)
        deferrable initially deferred,
    created_at timestamptz not null default clock_timestamp(),
    primary key (actor_id,event_id,operation_id,idempotency_key)
);

create function concert_gift_command_target_immutable() returns trigger
language plpgsql as $$ begin
    raise exception 'Gift command target is immutable' using errcode='23000';
end $$;
create trigger concert_gift_command_target_immutable
before update or delete on concert_gift_command_targets
for each row execute function concert_gift_command_target_immutable();

create function concert_capture_gift_command_target() returns trigger
language plpgsql as $$
declare
    raw text;
    intent jsonb;
    root uuid;
    old concert_gift_command_targets%rowtype;
begin
    if new.operation_id not in (
        'createConcertGiveaway','updateConcertGiveaway','setConcertPickupMeeting',
        'reserveConcertGiveaway','cancelConcertReservation','issueConcertPickupCode',
        'claimConcertReservation') then return new; end if;
    raw := nullif(current_setting('mcr.gift_live_command_target',true),'');
    -- Historical retained producers do not have a live intent and receive no
    -- correlation grant. Gift-live always installs one after actual authorization.
    if raw is null then return new; end if;
    intent := raw::jsonb;
    if jsonb_typeof(intent) <> 'object' or
       (select count(*) from jsonb_object_keys(intent)) <> 7 or
       not (intent ?& array['v','actor_id','event_id','operation_id','idempotency_key','target_id','giveaway_id']) or
       intent->'v' <> '1'::jsonb or
       intent->>'actor_id' is distinct from new.actor_id::text or
       intent->>'event_id' is distinct from new.event_id::text or
       intent->>'operation_id' is distinct from new.operation_id or
       intent->>'idempotency_key' is distinct from new.idempotency_key::text or
       intent->>'target_id' is distinct from new.target_id::text then
        raise exception 'Gift command intent mismatch' using errcode='22023';
    end if;
    root := (intent->>'giveaway_id')::uuid;
    if root is null or root='00000000-0000-0000-0000-000000000000' or
       (new.operation_id='createConcertGiveaway' and root<>new.target_id) or
       (new.operation_id<>'createConcertGiveaway' and not exists (
           select 1 from concert_community_giveaways
           where event_id=new.event_id and giveaway_id=root)) then
        raise exception 'Gift command root mismatch' using errcode='22023';
    end if;
    select * into old from concert_gift_command_targets
    where actor_id=new.actor_id and event_id=new.event_id
      and operation_id=new.operation_id and idempotency_key=new.idempotency_key;
    if found then
        if old.target_id<>new.target_id or old.giveaway_id<>root or old.receipt_id<>new.receipt_id then
            raise exception 'Gift command target rebound' using errcode='23000';
        end if;
    else
        insert into concert_gift_command_targets
            (actor_id,event_id,operation_id,idempotency_key,target_id,giveaway_id,receipt_id)
        values (new.actor_id,new.event_id,new.operation_id,new.idempotency_key,new.target_id,root,new.receipt_id);
    end if;
    return new;
end $$;
create trigger concert_capture_gift_command_target before insert on concert_command_receipts
for each row execute function concert_capture_gift_command_target();
