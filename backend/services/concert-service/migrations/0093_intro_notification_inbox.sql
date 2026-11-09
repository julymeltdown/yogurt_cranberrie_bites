-- The notification inbox: the columns `feed.Notification` requires.
--
-- Migration 0087 created `intro_like_notifications` as a *dedupe fact*: it
-- answers "has this actor already been told about this intro", and its primary
-- key `(recipient, intro, actor)` is exactly that question. It was never meant to
-- be a list, and it carries none of the fields a list needs.
--
-- `feed.Notification` requires `notification_id`, `event_id`, `read_at` and
-- `revision`, and `markNotificationRead` needs somewhere to record that a
-- recipient has opened one. This migration adds them, so the inbox can be served
-- from the same rows the like command already writes rather than from a second
-- table that could disagree with it.
--
-- ## Why these shapes
--
-- * `notification_id` is a stable identity the client can address. It is separate
--   from the dedupe key because the contract addresses a notification by id, and
--   a composite key is not a wire identity.
-- * `event_id` is required by the contract and is `NOT NULL`. A profile
--   notification is not about a concert event, so the row stores the nil UUID:
--   the contract requires the field, and inventing a real event id would be a
--   lie about what the notification is about. The value is documented here so a
--   reader does not mistake nil for a missing value.
-- * `read_at` is nullable: absent means unread. There is deliberately no
--   `unread` boolean, because two columns that can disagree is one column too
--   many.
-- * `revision` follows the project's revision discipline so a later mutation can
--   carry `expected_revision`. It starts at 1 and is never zero.
--
-- ## Backfill
--
-- Existing rows predate the inbox. They get a fresh `notification_id`, the nil
-- `event_id`, no `read_at` (they are unread, which is true — nobody has opened
-- them) and revision 1. The dedupe primary key is untouched, so the "two PUTs
-- produce one notification" rule still holds.

-- The defaults are what make the like command need no change: it inserts only
-- `(recipient, intro, actor, created_at)`, and the identity, the event id and the
-- revision are supplied here. A writer that had to remember to set them could
-- forget, and a notification without an identity cannot be addressed.
alter table intro_like_notifications
    add column notification_id uuid not null default gen_random_uuid(),
    add column event_id uuid not null
        default '00000000-0000-0000-0000-000000000000'::uuid,
    add column read_at timestamptz,
    add column revision bigint not null default 1;

-- The wire identity is unique, and a notification is addressed by it.
alter table intro_like_notifications
    add constraint intro_like_notifications_notification_id_key
        unique (notification_id),
    add constraint intro_like_notifications_notification_id_not_nil
        check (notification_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    add constraint intro_like_notifications_revision
        check (revision between 1 and 9007199254740991);

-- The recipient's inbox order is already indexed by 0087
-- (`intro_like_notifications_recipient`), so the list query needs no new index.
--
-- The unread filter is a partial index: an inbox read with `unread_only=true`
-- touches only unread rows, and a recipient's unread set is small compared with
-- their history.
create index intro_like_notifications_unread
    on intro_like_notifications (recipient_profile_id, created_at desc, actor_profile_id desc)
    where read_at is null;
