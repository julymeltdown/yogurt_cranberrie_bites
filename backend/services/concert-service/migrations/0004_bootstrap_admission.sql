-- Event-local participation state. Absence means no bootstrap has occurred;
-- only an explicit ACTIVE row authorizes creation of participant defaults.
CREATE TABLE public.concert_event_admissions (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    user_id uuid NOT NULL,
    state text NOT NULL CONSTRAINT concert_admission_state CHECK (state IN ('ACTIVE','SUSPENDED')),
    restriction_revision bigint NOT NULL DEFAULT 1 CONSTRAINT concert_admission_revision CHECK (restriction_revision BETWEEN 1 AND 9007199254740991),
    created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    CONSTRAINT concert_admission_key PRIMARY KEY(event_id,user_id),
    CONSTRAINT concert_admission_times CHECK (updated_at >= created_at)
);
CREATE FUNCTION public.concert_guard_admission() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.state <> 'ACTIVE' OR NEW.restriction_revision <> 1 THEN
            RAISE EXCEPTION 'initial admission must be active revision one' USING ERRCODE='23514';
        END IF;
    ELSE
        IF NEW.event_id IS DISTINCT FROM OLD.event_id
           OR NEW.user_id IS DISTINCT FROM OLD.user_id
           OR NEW.created_at IS DISTINCT FROM OLD.created_at
           OR NEW.restriction_revision <> OLD.restriction_revision + 1 THEN
            RAISE EXCEPTION 'admission scope or revision is invalid' USING ERRCODE='23514';
        END IF;
        NEW.updated_at := greatest(OLD.created_at, clock_timestamp());
        IF OLD.state = 'ACTIVE' AND NEW.state = 'SUSPENDED'
           AND NEW.restriction_revision <= OLD.restriction_revision THEN
            RAISE EXCEPTION 'restriction must advance its revision' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_admissions_revision_guard BEFORE INSERT OR UPDATE
    ON public.concert_event_admissions FOR EACH ROW
    EXECUTE FUNCTION public.concert_guard_admission();
