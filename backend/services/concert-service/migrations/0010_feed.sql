-- BE31/32 Concert-to-Post durable intent only. Post owns content and its receipts.
-- Payload is protected internal work data, never a public receipt/outbox payload.
-- No automatic execution/retry or fixtures are installed by this migration.
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
 payload jsonb NOT NULL CHECK(jsonb_typeof(payload)='object' AND octet_length(payload::text)<=65536),
 state text NOT NULL DEFAULT 'QUEUED' CHECK(state IN ('QUEUED','ACKNOWLEDGED')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 acknowledged_at timestamptz,
 FOREIGN KEY(receipt_id,event_id,target_id) REFERENCES public.concert_command_receipts(receipt_id,event_id,target_id) ON DELETE RESTRICT,
 UNIQUE(actor_id,event_id,operation_id,idempotency_key),
 CHECK((state='QUEUED' AND acknowledged_at IS NULL) OR (state='ACKNOWLEDGED' AND acknowledged_at IS NOT NULL))
);
CREATE INDEX concert_feed_pending_work ON public.concert_feed_owner_work(created_at,receipt_id) WHERE state='QUEUED';
