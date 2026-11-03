-- BE31/32 Concert-to-Post durable intent only. Post owns content and its receipts.
-- Payload is protected internal work data, never a public receipt/outbox payload.
-- No automatic execution/retry or fixtures are installed by this migration.
CREATE UNIQUE INDEX concert_feed_receipt_binding ON public.concert_command_receipts(receipt_id,event_id,target_id,actor_id,operation_id,idempotency_key);
CREATE TABLE public.concert_feed_owner_work (
 receipt_id uuid PRIMARY KEY,
 event_id uuid NOT NULL,
 target_id uuid NOT NULL,
 actor_id uuid NOT NULL,
 operation_id text NOT NULL CHECK(operation_id IN (
  'createConcertPost','editConcertPost','deleteConcertPost','createConcertComment',
  'editConcertComment','deleteConcertComment','likeConcertPost','unlikeConcertPost')),
 idempotency_key uuid NOT NULL,
 session_family_id uuid NOT NULL,
 request_id uuid NOT NULL,
 authority_expires_at timestamptz NOT NULL,
 authority_expires_text text NOT NULL CHECK(char_length(authority_expires_text) BETWEEN 20 AND 40),
 authority_digest bytea NOT NULL CHECK(octet_length(authority_digest)=32),
 parent_id uuid,
 parent_revision bigint CHECK(parent_revision BETWEEN 1 AND 9007199254740991),
 CHECK((parent_id IS NULL)=(parent_revision IS NULL)),
 payload jsonb NOT NULL CHECK(jsonb_typeof(payload)='object' AND octet_length(payload::text)<=65536),
 state text NOT NULL DEFAULT 'QUEUED' CHECK(state IN ('QUEUED','ACKNOWLEDGED')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 acknowledged_at timestamptz,
 FOREIGN KEY(receipt_id,event_id,target_id,actor_id,operation_id,idempotency_key) REFERENCES public.concert_command_receipts(receipt_id,event_id,target_id,actor_id,operation_id,idempotency_key) ON DELETE RESTRICT,
 UNIQUE(actor_id,event_id,operation_id,idempotency_key),
 CHECK((state='QUEUED' AND acknowledged_at IS NULL) OR (state='ACKNOWLEDGED' AND acknowledged_at IS NOT NULL))
);
CREATE INDEX concert_feed_pending_work ON public.concert_feed_owner_work(created_at,receipt_id) WHERE state='QUEUED';
