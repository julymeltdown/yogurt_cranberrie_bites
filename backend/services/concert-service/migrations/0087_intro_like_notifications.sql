-- WP-05 T-411: the like notification, written by the command.
--
-- WP-02 deliberately put **no** trigger on `intro_likes`: a like is a row, and
-- nothing in the database reacts to it. The notification is therefore written by
-- the like command, inside the same transaction, so it carries the command's
-- authority and its idempotency.
--
-- The dedupe key is `(recipient, intro, actor)`. That is what makes "two PUT
-- likes produce one row and one notification" true even if the command is
-- replayed or the row insert is repeated: the second insert conflicts and does
-- nothing.
--
-- There is deliberately no `state`, `pending`, `accepted` or `read` column on
-- this table. A like is not a request, so there is nothing to accept and
-- nothing to be pending. The recipient's activity feed marks an entry read in
-- its own table; the notification itself is a fact, not a workflow.
create table intro_like_notifications (
    recipient_profile_id uuid not null
        check (recipient_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    intro_id uuid not null references intro_posts(intro_id) on delete cascade,
    actor_profile_id uuid not null
        check (actor_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    created_at timestamptz not null default clock_timestamp(),
    primary key (recipient_profile_id, intro_id, actor_profile_id),
    -- The author cannot like their own post, so the recipient is never the actor.
    constraint intro_like_notifications_distinct check (recipient_profile_id <> actor_profile_id)
);

-- The recipient's own inbox order.
create index intro_like_notifications_recipient
    on intro_like_notifications (recipient_profile_id, created_at desc, actor_profile_id desc);

-- A notification never outlives the like it describes. When the actor withdraws
-- the heart, the notification goes with it: the author should not keep a
-- "someone liked you" entry for a like that no longer exists.
create function intro_like_notifications_follow_like() returns trigger language plpgsql as $$
begin
    if tg_op = 'DELETE' then
        delete from intro_like_notifications
        where intro_id = old.intro_id and actor_profile_id = old.actor_profile_id;
        return old;
    end if;
    return new;
end $$;

create trigger intro_like_notifications_follow_like
    after delete on intro_likes
    for each row execute function intro_like_notifications_follow_like();
