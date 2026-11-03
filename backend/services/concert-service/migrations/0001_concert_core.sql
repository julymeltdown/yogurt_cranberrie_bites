-- New MCR-owned schema; no joins, seed rows, or identity copy from other services.
CREATE TABLE public.concert_events (
    event_id uuid PRIMARY KEY,
    slug text NOT NULL CONSTRAINT concert_event_slug_shape CHECK (slug ~ '^[a-z0-9][a-z0-9-]{0,63}$'),
    title text NOT NULL CONSTRAINT concert_event_title_bounds CHECK (char_length(title) BETWEEN 1 AND 120 AND title ~ '[^[:space:]]' AND title !~ '[[:cntrl:]]'),
    phase text NOT NULL CONSTRAINT concert_event_phase CHECK (phase IN ('DRAFT','OPEN','LIVE','READ_ONLY','ARCHIVED')),
    timezone text NOT NULL DEFAULT 'Asia/Seoul' CONSTRAINT concert_event_timezone CHECK (timezone='Asia/Seoul'),
    revision bigint NOT NULL DEFAULT 1 CONSTRAINT concert_event_revision CHECK (revision BETWEEN 1 AND 9007199254740991),
    created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    CONSTRAINT concert_event_times CHECK (updated_at >= created_at),
    CONSTRAINT concert_event_slug_unique UNIQUE (slug)
);
CREATE TABLE public.concert_participants (
    event_id uuid NOT NULL,
    user_id uuid NOT NULL, -- canonical Auth reference; authority is resolved through BE04 policy, never copied claims.
    revision bigint NOT NULL DEFAULT 1 CONSTRAINT concert_participant_revision CHECK (revision BETWEEN 1 AND 9007199254740991),
    theme text NOT NULL CONSTRAINT concert_participant_theme CHECK (theme IN ('REVENGE','PARADE','DANGER')),
    low_power boolean NOT NULL,
    reduced_motion boolean NOT NULL,
    sound_enabled boolean NOT NULL,
    created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    CONSTRAINT concert_participant_key PRIMARY KEY(event_id,user_id),
    CONSTRAINT concert_participant_event FOREIGN KEY(event_id) REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    CONSTRAINT concert_participant_times CHECK (updated_at >= created_at)
);
CREATE TABLE public.concert_avatar_settings (
    event_id uuid NOT NULL,
    user_id uuid NOT NULL,
    revision bigint NOT NULL DEFAULT 1 CONSTRAINT concert_avatar_revision CHECK (revision BETWEEN 1 AND 9007199254740991),
    contract_version text NOT NULL CONSTRAINT concert_avatar_contract CHECK (contract_version='1.0.0'),
    manifest_id text NOT NULL CONSTRAINT concert_avatar_manifest_id CHECK (manifest_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    manifest_version bigint NOT NULL CONSTRAINT concert_avatar_manifest_version CHECK (manifest_version BETWEEN 1 AND 9007199254740991),
    body_id text NOT NULL CONSTRAINT concert_avatar_body CHECK (body_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    eyes_id text NOT NULL CONSTRAINT concert_avatar_eyes CHECK (eyes_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    hair_id text NOT NULL CONSTRAINT concert_avatar_hair CHECK (hair_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    outfit_id text NOT NULL CONSTRAINT concert_avatar_outfit CHECK (outfit_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    accessory_id text CONSTRAINT concert_avatar_accessory CHECK (accessory_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    skin_tone_id text NOT NULL CONSTRAINT concert_avatar_skin_tone CHECK (skin_tone_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    CONSTRAINT concert_avatar_key PRIMARY KEY(event_id,user_id),
    CONSTRAINT concert_avatar_participant FOREIGN KEY(event_id,user_id) REFERENCES public.concert_participants(event_id,user_id) ON DELETE CASCADE,
    CONSTRAINT concert_avatar_times CHECK (updated_at >= created_at)
);

-- All write paths, including direct SQL, advance exactly one revision and retain scope.
CREATE FUNCTION public.concert_guard_revision() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE key_name text;
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.revision <> 1 THEN RAISE EXCEPTION 'initial Concert revision must be one' USING ERRCODE='23514'; END IF;
    ELSE
        FOREACH key_name IN ARRAY TG_ARGV LOOP
            IF to_jsonb(NEW)->key_name IS DISTINCT FROM to_jsonb(OLD)->key_name THEN
                RAISE EXCEPTION 'Concert scope is immutable' USING ERRCODE='23514';
            END IF;
        END LOOP;
        IF NEW.created_at IS DISTINCT FROM OLD.created_at OR NEW.revision <> OLD.revision + 1 THEN
            RAISE EXCEPTION 'Concert revision or creation time is invalid' USING ERRCODE='23514';
        END IF;
        NEW.updated_at := greatest(OLD.created_at, clock_timestamp());
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_events_revision_guard BEFORE INSERT OR UPDATE ON public.concert_events
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id');
CREATE TRIGGER concert_participants_revision_guard BEFORE INSERT OR UPDATE ON public.concert_participants
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id','user_id');
CREATE TRIGGER concert_avatars_revision_guard BEFORE INSERT OR UPDATE ON public.concert_avatar_settings
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id','user_id');
