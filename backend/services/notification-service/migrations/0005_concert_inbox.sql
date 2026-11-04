-- BE33 extends the inherited Notification owner. It does not create a second inbox.
CREATE TABLE concert_notification_deliveries (
 outbox_id uuid PRIMARY KEY,
 receipt_id uuid NOT NULL UNIQUE,
 event_id uuid NOT NULL,
 resource_id uuid NOT NULL,
 resource_revision bigint NOT NULL CHECK(resource_revision BETWEEN 1 AND 9007199254740991),
 committed_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
-- Tombstones above survive content retention/purge; late replay cannot recreate it.
CREATE TABLE concert_notification_items (
 notification_id uuid PRIMARY KEY REFERENCES notifications(id) ON DELETE CASCADE,
 outbox_id uuid NOT NULL REFERENCES concert_notification_deliveries(outbox_id),
 event_id uuid NOT NULL,
 kind text NOT NULL CHECK(kind IN ('PARTY','GIVEAWAY','SOCIAL','MODERATION','NOTICE','FEED')),
 target_type text CHECK(target_type IN ('PARTY','APPLICATION','GIVEAWAY','RESERVATION','SOCIAL_REQUEST','POST','NOTICE','REPORT')),
 target_id uuid,
 CHECK((target_type IS NULL) = (target_id IS NULL))
);
CREATE INDEX concert_notification_event ON concert_notification_items(event_id,notification_id);
CREATE TABLE concert_notification_inbox_states (
 actor_id uuid NOT NULL,
 event_id uuid NOT NULL,
 PRIMARY KEY(actor_id,event_id)
);
CREATE TABLE concert_notification_cursors (
 token uuid PRIMARY KEY,
 actor_id uuid NOT NULL,
 event_id uuid NOT NULL,
 unread_only boolean NOT NULL,
 upper_at timestamptz NOT NULL,
 after_at timestamptz NOT NULL,
 after_id uuid NOT NULL,
 expires_at timestamptz NOT NULL
);
CREATE INDEX concert_notification_cursor_expiry ON concert_notification_cursors(expires_at);
CREATE TABLE concert_notification_read_receipts (
 actor_id uuid NOT NULL,
 event_id uuid NOT NULL,
 operation text NOT NULL CHECK(operation IN ('SINGLE','ALL')),
 idempotency_key uuid NOT NULL,
 notification_id uuid,
 expected_revision bigint,
 read_count integer NOT NULL CHECK(read_count>=0),
 read_at timestamptz NOT NULL,
 expires_at timestamptz NOT NULL,
 PRIMARY KEY(actor_id,event_id,operation,idempotency_key),
 CHECK((operation='SINGLE' AND notification_id IS NOT NULL AND expected_revision BETWEEN 1 AND 9007199254740991)
    OR (operation='ALL' AND notification_id IS NULL AND expected_revision IS NULL))
);
-- Inherited mark-read/all-read also participate: revision derives from immutable
-- creation/read state (1 unread, 2 read), so old owner RPCs cannot desynchronize it.
-- Commit guard prevents a delayed write/receipt/cursor escaping its authority bound.
CREATE FUNCTION concert_notification_authority_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE deadline timestamptz;
BEGIN
 deadline := nullif(current_setting('mcr.notification_authority_until',true),'')::timestamptz;
 IF deadline IS NULL OR deadline<=clock_timestamp() THEN
   RAISE EXCEPTION 'notification authority expired' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_notification_receipt_guard AFTER INSERT ON concert_notification_read_receipts
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION concert_notification_authority_guard();
CREATE CONSTRAINT TRIGGER concert_notification_cursor_guard AFTER INSERT ON concert_notification_cursors
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION concert_notification_authority_guard();

CREATE FUNCTION concert_notification_read_monotone() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.read_at IS NOT NULL AND NEW.read_at IS DISTINCT FROM OLD.read_at
    AND EXISTS(SELECT 1 FROM concert_notification_items WHERE notification_id=OLD.id) THEN
   RAISE EXCEPTION 'concert notification read state is monotone' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_notification_read_monotone BEFORE UPDATE OF read_at ON notifications
 FOR EACH ROW EXECUTE FUNCTION concert_notification_read_monotone();
