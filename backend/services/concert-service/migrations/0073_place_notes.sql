CREATE TABLE public.concert_place_notes (
    note_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    landmark_id text NOT NULL
        CONSTRAINT concert_place_note_landmark_shape
        CHECK (landmark_id ~ '^[A-Za-z0-9._~-]{1,128}$' AND landmark_id NOT IN ('.', '..')),
    catalog_revision bigint NOT NULL
        CONSTRAINT concert_place_note_catalog_revision
        CHECK (catalog_revision BETWEEN 1 AND 9007199254740991),
    landmark_revision bigint NOT NULL
        CONSTRAINT concert_place_note_landmark_revision
        CHECK (landmark_revision BETWEEN 1 AND 9007199254740991),
    author_id uuid NOT NULL,
    parent_id uuid REFERENCES public.concert_place_notes(note_id) ON DELETE CASCADE,
    audience text NOT NULL
        CONSTRAINT concert_place_note_audience CHECK (audience IN ('FRIENDS','EVERYONE')),
    expiry_kind text NOT NULL
        CONSTRAINT concert_place_note_expiry CHECK (expiry_kind IN ('ONE_HOUR','VENUE_END_DAY')),
    body text NOT NULL
        CONSTRAINT concert_place_note_body_bounds
        CHECK (char_length(body) BETWEEN 0 AND 1000 AND octet_length(body) <= 4000),
    revision bigint NOT NULL DEFAULT 1
        CONSTRAINT concert_place_note_revision CHECK (revision BETWEEN 1 AND 9007199254740991),
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    withdrawn_at timestamptz,
    hidden_at timestamptz,
    hidden_by uuid,
    CONSTRAINT concert_place_note_times CHECK (expires_at > created_at),
    CONSTRAINT concert_place_note_reply_same_event
        CHECK (parent_id IS NULL OR parent_id <> note_id),
    CONSTRAINT concert_place_note_hidden_pair
        CHECK ((hidden_at IS NULL) = (hidden_by IS NULL))
);

CREATE INDEX concert_place_notes_landmark_order
    ON public.concert_place_notes (event_id, landmark_id, created_at DESC, note_id DESC)
    WHERE parent_id IS NULL;
CREATE INDEX concert_place_notes_reply_order
    ON public.concert_place_notes (parent_id, created_at DESC, note_id DESC)
    WHERE parent_id IS NOT NULL;

CREATE FUNCTION public.concert_place_note_inherit() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE parent public.concert_place_notes%ROWTYPE;
BEGIN
    IF NEW.parent_id IS NULL THEN
        RETURN NEW;
    END IF;
    SELECT * INTO parent FROM public.concert_place_notes
        WHERE note_id = NEW.parent_id FOR SHARE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'note parent is unavailable' USING ERRCODE = '23503';
    END IF;
    IF parent.parent_id IS NOT NULL THEN
        RAISE EXCEPTION 'place replies are one level only' USING ERRCODE = '23514';
    END IF;
    IF parent.event_id <> NEW.event_id
        OR parent.landmark_id <> NEW.landmark_id
        OR parent.catalog_revision <> NEW.catalog_revision
        OR parent.landmark_revision <> NEW.landmark_revision THEN
        RAISE EXCEPTION 'reply must stay on the parent place' USING ERRCODE = '23514';
    END IF;
    NEW.audience := parent.audience;
    NEW.expiry_kind := parent.expiry_kind;
    NEW.expires_at := parent.expires_at;
    IF NEW.expires_at <= NEW.created_at THEN
        RAISE EXCEPTION 'reply cannot outlive its parent' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_place_notes_inherit_guard
    BEFORE INSERT ON public.concert_place_notes
    FOR EACH ROW EXECUTE FUNCTION public.concert_place_note_inherit();

CREATE FUNCTION public.concert_place_note_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.note_id <> OLD.note_id
        OR NEW.event_id <> OLD.event_id
        OR NEW.landmark_id <> OLD.landmark_id
        OR NEW.catalog_revision <> OLD.catalog_revision
        OR NEW.landmark_revision <> OLD.landmark_revision
        OR NEW.author_id <> OLD.author_id
        OR NEW.parent_id IS DISTINCT FROM OLD.parent_id
        OR NEW.audience <> OLD.audience
        OR NEW.expiry_kind <> OLD.expiry_kind
        OR NEW.created_at <> OLD.created_at
        OR NEW.expires_at <> OLD.expires_at THEN
        RAISE EXCEPTION 'place note identity is immutable' USING ERRCODE = '23514';
    END IF;
    IF OLD.withdrawn_at IS NOT NULL AND NEW.withdrawn_at IS NULL THEN
        RAISE EXCEPTION 'withdrawal is terminal' USING ERRCODE = '23514';
    END IF;
    IF OLD.hidden_at IS NOT NULL AND NEW.hidden_at IS NULL THEN
        RAISE EXCEPTION 'moderation hide is terminal for this note' USING ERRCODE = '23514';
    END IF;
    IF NEW.body IS DISTINCT FROM OLD.body
        AND NOT (NEW.body = '' AND (NEW.withdrawn_at IS NOT NULL OR NEW.hidden_at IS NOT NULL)) THEN
        RAISE EXCEPTION 'place note body is immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.revision NOT IN (OLD.revision, OLD.revision + 1) THEN
        RAISE EXCEPTION 'place note revision must advance by at most one' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_place_notes_guard
    BEFORE UPDATE ON public.concert_place_notes
    FOR EACH ROW EXECUTE FUNCTION public.concert_place_note_guard();
