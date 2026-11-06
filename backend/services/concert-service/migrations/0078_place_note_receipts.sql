-- VENU-BE-N01: durable keyed receipts for place-note create/reply/withdraw.
-- 0078 is the next free version after 0077_event_service_policy.sql.
CREATE TABLE public.concert_place_note_receipts (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    actor_id uuid NOT NULL,
    command_key uuid NOT NULL,
    operation text NOT NULL
        CONSTRAINT concert_place_note_receipt_operation
        CHECK (operation IN ('createVenuePlaceNote','createVenuePlaceReply','withdrawVenuePlaceNote')),
    request_digest bytea NOT NULL
        CONSTRAINT concert_place_note_receipt_digest
        CHECK (octet_length(request_digest) = 32),
    result jsonb NOT NULL
        CONSTRAINT concert_place_note_receipt_result CHECK (jsonb_typeof(result) = 'object'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    expires_at timestamptz NOT NULL,
    CONSTRAINT concert_place_note_receipt_key PRIMARY KEY(event_id,actor_id,command_key),
    CONSTRAINT concert_place_note_receipt_time CHECK (expires_at >= created_at)
);
CREATE INDEX concert_place_note_receipt_expiry
    ON public.concert_place_note_receipts(expires_at);
