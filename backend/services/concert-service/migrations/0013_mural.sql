-- BE34/35. No deployment settings/palette defaults: provisioning is an owning operator concern.
CREATE TABLE public.concert_mural_state (
 event_id uuid PRIMARY KEY REFERENCES public.concert_events(event_id) ON DELETE RESTRICT,
 revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 snapshot_id uuid NOT NULL,
 state text NOT NULL CHECK(state IN ('EDITABLE','PAUSED','FROZEN')),
 pixels bytea NOT NULL CHECK(octet_length(pixels)=16384),
 palette text[] NOT NULL CHECK(cardinality(palette)=16 AND array_ndims(palette)=1),
 cooldown_ms bigint NOT NULL CHECK(cooldown_ms BETWEEN 0 AND 3600000),
 retained_from_revision bigint NOT NULL DEFAULT 1,
 reset_revision bigint NOT NULL DEFAULT 1,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 snapshot_created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 CHECK(retained_from_revision BETWEEN 1 AND revision),
 CHECK(reset_revision BETWEEN 1 AND revision)
);
CREATE TABLE public.concert_mural_limits (
 event_id uuid NOT NULL REFERENCES public.concert_mural_state(event_id),
 actor_id uuid NOT NULL,
 next_allowed_at timestamptz NOT NULL,
 PRIMARY KEY(event_id,actor_id)
);
-- Private author/cooldown records must never be selected into public projections.
CREATE TABLE public.concert_mural_operations (
 event_id uuid NOT NULL REFERENCES public.concert_mural_state(event_id),
 revision bigint NOT NULL CHECK(revision BETWEEN 2 AND 9007199254740991),
 operation_id uuid NOT NULL UNIQUE,
 actor_id uuid NOT NULL,
 x integer NOT NULL CHECK(x BETWEEN 0 AND 127),
 y integer NOT NULL CHECK(y BETWEEN 0 AND 127),
 color_index integer NOT NULL CHECK(color_index BETWEEN 0 AND 15),
 committed_at timestamptz NOT NULL,
 next_allowed_at timestamptz NOT NULL CHECK(next_allowed_at>=committed_at),
 PRIMARY KEY(event_id,revision)
);
CREATE TABLE public.concert_mural_locks (
 event_id uuid NOT NULL REFERENCES public.concert_mural_state(event_id),
 lock_id uuid NOT NULL,
 x_min integer NOT NULL CHECK(x_min BETWEEN 0 AND 127),
 y_min integer NOT NULL CHECK(y_min BETWEEN 0 AND 127),
 x_max_exclusive integer NOT NULL CHECK(x_max_exclusive BETWEEN 1 AND 128 AND x_max_exclusive>x_min),
 y_max_exclusive integer NOT NULL CHECK(y_max_exclusive BETWEEN 1 AND 128 AND y_max_exclusive>y_min),
 PRIMARY KEY(event_id,lock_id)
);
CREATE FUNCTION public.concert_mural_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p text; i integer;
BEGIN
 IF TG_OP='UPDATE' THEN
  IF NEW.event_id<>OLD.event_id OR NEW.created_at<>OLD.created_at OR NEW.revision<>OLD.revision+1
     OR NEW.snapshot_id=OLD.snapshot_id
     OR OLD.state='FROZEN' OR NEW.reset_revision<OLD.reset_revision
     OR NEW.retained_from_revision<OLD.retained_from_revision THEN
    RAISE EXCEPTION 'invalid mural revision transition' USING ERRCODE='23514';
  END IF;
  -- BE40 restore/control must lock this row, increment revision, and advance reset_revision
  -- to NEW.revision whenever the revision has no single public pixel operation.
 END IF;
 FOREACH p IN ARRAY NEW.palette LOOP
  IF p IS NULL OR p !~ '^#[0-9A-F]{6}$' THEN RAISE EXCEPTION 'invalid palette' USING ERRCODE='23514'; END IF;
 END LOOP;
 FOR i IN 0..16383 LOOP
  IF get_byte(NEW.pixels,i)>15 THEN RAISE EXCEPTION 'invalid pixels' USING ERRCODE='23514'; END IF;
 END LOOP;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_mural_guard BEFORE INSERT OR UPDATE ON public.concert_mural_state
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_guard();

-- Immutable public checkpoints selected from one locked aggregate revision. Caller
-- cooldown, author and moderation metadata never enter a recovery checkpoint.
CREATE TABLE public.concert_mural_snapshots (
 snapshot_id uuid PRIMARY KEY,
 event_id uuid NOT NULL REFERENCES public.concert_mural_state(event_id),
 revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 state text NOT NULL CHECK(state IN ('EDITABLE','PAUSED','FROZEN')),
 pixels bytea NOT NULL CHECK(octet_length(pixels)=16384),
 palette text[] NOT NULL CHECK(cardinality(palette)=16 AND array_ndims(palette)=1),
 created_at timestamptz NOT NULL,
 UNIQUE(event_id,revision)
);
CREATE FUNCTION public.concert_mural_checkpoint_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 RAISE EXCEPTION 'immutable mural recovery checkpoint' USING ERRCODE='23514';
END $$;
CREATE TRIGGER concert_mural_checkpoint_immutable BEFORE UPDATE ON public.concert_mural_snapshots
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_checkpoint_immutable();
