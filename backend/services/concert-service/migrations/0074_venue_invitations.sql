-- VENU-BE-M01 contextual meeting persistence.
-- Version 0074 is the next free ledger version; the plan's "0054" and the
-- contract's provisional "0102" both collide with already-published migrations
-- (0054_social_account_deletion.sql) or reserve an unusable gap.
-- No source venue data is imported. All effect rows are event-scoped and
-- carry absolute reviewed deadlines supplied by the meeting policy.
CREATE TABLE public.concert_venue_meeting_contexts (
    id uuid PRIMARY KEY,
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    context_id text NOT NULL CHECK(char_length(context_id) BETWEEN 1 AND 128),
    member_low uuid NOT NULL,
    member_high uuid NOT NULL,
    landmark_id text,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK(member_low < member_high),
    CHECK(landmark_id IS NULL OR char_length(landmark_id) BETWEEN 1 AND 128),
    UNIQUE(event_id, context_id),
    UNIQUE(event_id, id)
);

CREATE TABLE public.concert_venue_meeting_conversations (
    id uuid PRIMARY KEY,
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    context_id text NOT NULL,
    member_low uuid NOT NULL,
    member_high uuid NOT NULL,
    landmark_id text,
    next_sequence bigint NOT NULL DEFAULT 0 CHECK(next_sequence BETWEEN 0 AND 9007199254740991),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK(member_low < member_high),
    UNIQUE(event_id, context_id),
    UNIQUE(event_id, id),
    FOREIGN KEY(event_id, context_id)
        REFERENCES public.concert_venue_meeting_contexts(event_id, context_id) ON DELETE CASCADE
);

CREATE TABLE public.concert_venue_meeting_invitations (
    id uuid PRIMARY KEY,
    ordinal bigserial UNIQUE NOT NULL CHECK(ordinal BETWEEN 1 AND 9007199254740991),
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    context_id text NOT NULL,
    sender_id uuid NOT NULL,
    recipient_id uuid NOT NULL,
    selection jsonb NOT NULL,
    state text NOT NULL CHECK(state IN ('PENDING','ACCEPTED','DECLINED','CANCELLED','EXPIRED')),
    revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    purge_at timestamptz NOT NULL,
    policy_revision bigint NOT NULL CHECK(policy_revision BETWEEN 1 AND 9007199254740991),
    CHECK(sender_id <> recipient_id),
    CHECK(created_at < expires_at),
    CHECK(purge_at >= expires_at),
    UNIQUE(event_id, id),
    FOREIGN KEY(event_id, context_id)
        REFERENCES public.concert_venue_meeting_contexts(event_id, context_id) ON DELETE CASCADE
);
CREATE INDEX concert_venue_meeting_incoming
    ON public.concert_venue_meeting_invitations(event_id, recipient_id, ordinal);
CREATE INDEX concert_venue_meeting_outgoing
    ON public.concert_venue_meeting_invitations(event_id, sender_id, ordinal);

CREATE TABLE public.concert_venue_meeting_messages (
    id uuid PRIMARY KEY,
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    conversation_id uuid NOT NULL,
    sequence bigint NOT NULL CHECK(sequence BETWEEN 1 AND 9007199254740991),
    sender_id uuid NOT NULL,
    body text NOT NULL CHECK(
        octet_length(body) <= 8000
        AND char_length(body) BETWEEN 1 AND 2000
        AND body ~ '[^[:space:]]'
        AND body !~ '[[:cntrl:]&&[^\n\t]]'
    ),
    invitation_id uuid,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    purge_at timestamptz NOT NULL,
    policy_revision bigint NOT NULL CHECK(policy_revision BETWEEN 1 AND 9007199254740991),
    CHECK(created_at < expires_at),
    CHECK(purge_at >= expires_at),
    UNIQUE(conversation_id, sequence),
    FOREIGN KEY(event_id, conversation_id)
        REFERENCES public.concert_venue_meeting_conversations(event_id, id) ON DELETE CASCADE
);
-- invitation_id is a validated historical association, not an FK: the
-- invitation keeps its own independent reviewed purge deadline.
CREATE INDEX concert_venue_meeting_send_rate
    ON public.concert_venue_meeting_messages(event_id, sender_id, created_at);

CREATE TABLE public.concert_venue_meeting_receipts (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    actor_id uuid NOT NULL,
    command_key uuid NOT NULL,
    operation text NOT NULL CHECK(char_length(operation) BETWEEN 1 AND 64),
    request_digest bytea NOT NULL CHECK(octet_length(request_digest) = 32),
    context_id text NOT NULL,
    result jsonb NOT NULL,
    expires_at timestamptz NOT NULL,
    purge_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK(purge_at >= expires_at),
    PRIMARY KEY(event_id, actor_id, command_key)
);

CREATE FUNCTION public.concert_venue_meeting_effect_deadline() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE ceiling timestamptz;
BEGIN
    ceiling := nullif(current_setting('mcr.venue_meeting_authority_until', true), '')::timestamptz;
    IF ceiling IS NULL OR clock_timestamp() >= ceiling THEN
        RAISE EXCEPTION 'Venue meeting effect authority expired';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_venue_meeting_invitation_deadline
    AFTER INSERT OR UPDATE ON public.concert_venue_meeting_invitations
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
    EXECUTE FUNCTION public.concert_venue_meeting_effect_deadline();
CREATE CONSTRAINT TRIGGER concert_venue_meeting_message_deadline
    AFTER INSERT ON public.concert_venue_meeting_messages
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
    EXECUTE FUNCTION public.concert_venue_meeting_effect_deadline();
CREATE CONSTRAINT TRIGGER concert_venue_meeting_context_deadline
    AFTER INSERT OR UPDATE ON public.concert_venue_meeting_contexts
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
    EXECUTE FUNCTION public.concert_venue_meeting_effect_deadline();
CREATE CONSTRAINT TRIGGER concert_venue_meeting_receipt_deadline
    AFTER INSERT ON public.concert_venue_meeting_receipts
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
    EXECUTE FUNCTION public.concert_venue_meeting_effect_deadline();
