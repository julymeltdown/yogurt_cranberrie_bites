-- BE42 PrivateData aggregate coordinator. 0055 remains the immutable core
-- leaf; this migration adds only durable category receipts and their guard.
CREATE TABLE public.concert_private_data_owner_progress (
    user_id uuid PRIMARY KEY CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    saga_id uuid NOT NULL UNIQUE,
    actor_id uuid NOT NULL CHECK (actor_id <> '00000000-0000-0000-0000-000000000000'),
    target text NOT NULL CHECK (target='PRIVATE_DATA'),
    retention_seconds bigint NOT NULL,
    retention_nanos integer NOT NULL CHECK (retention_nanos BETWEEN 0 AND 999999999),
    identity_binding bytea NOT NULL CHECK (octet_length(identity_binding)=32),
    command_binding bytea NOT NULL CHECK (octet_length(command_binding)=32),
    source_binding bytea NOT NULL CHECK (octet_length(source_binding)=32 AND source_binding<>decode(repeat('00',32),'hex')),
    fence_revision bigint NOT NULL CHECK (fence_revision>0),
    next_category integer NOT NULL DEFAULT 0 CHECK (next_category BETWEEN 0 AND 7),
    progress_revision bigint NOT NULL DEFAULT 0 CHECK (progress_revision>=0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    completed_at timestamptz,
    FOREIGN KEY(user_id,saga_id) REFERENCES public.concert_world_account_deletion_fences(user_id,saga_id),
    UNIQUE(user_id,saga_id),
    UNIQUE(user_id,saga_id,command_binding,source_binding,fence_revision)
);
CREATE INDEX concert_private_data_owner_pending
 ON public.concert_private_data_owner_progress(user_id,completed_at,next_category);

CREATE TABLE public.concert_private_data_category_progress (
    user_id uuid NOT NULL,
    saga_id uuid NOT NULL,
    category text NOT NULL CHECK (category IN ('CORE_PREFERENCES_AND_AVATAR','INTRO_TEXT','AGE_CLAIMS','MEDIA_AND_MEETING_PHOTOS','BEACONS_AND_RALLY','PARTY_AND_GIFT_INPUT','MURAL_PRIVATE_METADATA')),
    category_version integer NOT NULL CHECK (category_version=1),
    command_binding bytea NOT NULL CHECK (octet_length(command_binding)=32),
    source_binding bytea NOT NULL CHECK (octet_length(source_binding)=32 AND source_binding<>decode(repeat('00',32),'hex')),
    fence_revision bigint NOT NULL CHECK (fence_revision>0),
    ordinal integer NOT NULL CHECK (ordinal BETWEEN 0 AND 6),
    owner_revision bigint CHECK (owner_revision IS NULL OR owner_revision>0),
    state text NOT NULL CHECK (state IN ('INCOMPLETE','COMPLETE')),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    completed_at timestamptz,
    PRIMARY KEY(user_id,category),
    FOREIGN KEY(user_id,saga_id,command_binding,source_binding,fence_revision)
      REFERENCES public.concert_private_data_owner_progress(user_id,saga_id,command_binding,source_binding,fence_revision)
      ON UPDATE RESTRICT ON DELETE RESTRICT,
    CHECK ((state='COMPLETE' AND owner_revision IS NOT NULL AND completed_at IS NOT NULL)
        OR (state='INCOMPLETE' AND completed_at IS NULL)),
    UNIQUE(user_id,saga_id,ordinal),
    CHECK (ordinal=case category
        when 'CORE_PREFERENCES_AND_AVATAR' then 0
        when 'INTRO_TEXT' then 1
        when 'AGE_CLAIMS' then 2
        when 'MEDIA_AND_MEETING_PHOTOS' then 3
        when 'BEACONS_AND_RALLY' then 4
        when 'PARTY_AND_GIFT_INPUT' then 5
        when 'MURAL_PRIVATE_METADATA' then 6
        else -1 end)
);
CREATE INDEX concert_private_data_category_pending
 ON public.concert_private_data_category_progress(user_id,saga_id,state,ordinal);

CREATE FUNCTION public.concert_private_data_aggregate_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP <> 'UPDATE' THEN RAISE EXCEPTION 'PrivateData aggregate receipt cannot be removed' USING ERRCODE='23514'; END IF;
 IF OLD.completed_at IS NOT NULL OR
    (NEW.user_id,NEW.saga_id,NEW.actor_id,NEW.target,NEW.retention_seconds,NEW.retention_nanos,
     NEW.identity_binding,NEW.command_binding,NEW.source_binding,NEW.fence_revision,NEW.created_at)
    IS DISTINCT FROM
    (OLD.user_id,OLD.saga_id,OLD.actor_id,OLD.target,OLD.retention_seconds,OLD.retention_nanos,
     OLD.identity_binding,OLD.command_binding,OLD.source_binding,OLD.fence_revision,OLD.created_at)
    OR NEW.progress_revision<>OLD.progress_revision+1
    OR NEW.next_category<OLD.next_category
    OR (OLD.completed_at IS NOT NULL AND NEW.completed_at IS DISTINCT FROM OLD.completed_at)
    OR (OLD.completed_at IS NULL AND NEW.completed_at IS NOT NULL
        AND NEW.next_category<>7) THEN
  RAISE EXCEPTION 'PrivateData aggregate receipt is immutable or nonmonotonic' USING ERRCODE='23514';
 END IF;
 IF NEW.completed_at IS NOT NULL AND
    (SELECT count(*) FROM public.concert_private_data_category_progress
      WHERE user_id=NEW.user_id AND saga_id=NEW.saga_id AND command_binding=NEW.command_binding
        AND source_binding=NEW.source_binding AND state='COMPLETE') <> 7 THEN
  RAISE EXCEPTION 'PrivateData whole acknowledgement requires all seven categories' USING ERRCODE='23514';
 END IF;
 NEW.updated_at:=clock_timestamp(); RETURN NEW;
END $$;
CREATE TRIGGER concert_private_data_aggregate_guard BEFORE UPDATE OR DELETE ON public.concert_private_data_owner_progress
 FOR EACH ROW EXECUTE FUNCTION public.concert_private_data_aggregate_guard();
CREATE TRIGGER concert_private_data_aggregate_no_truncate BEFORE TRUNCATE ON public.concert_private_data_owner_progress
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_private_data_aggregate_guard();

CREATE FUNCTION public.concert_private_data_category_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP <> 'UPDATE' THEN RAISE EXCEPTION 'PrivateData category receipt cannot be removed' USING ERRCODE='23514'; END IF;
    IF (OLD.state='COMPLETE' AND (NEW.state<>'COMPLETE' OR
        NEW.owner_revision IS DISTINCT FROM OLD.owner_revision OR
        NEW.completed_at IS DISTINCT FROM OLD.completed_at)) OR
    (NEW.user_id,NEW.saga_id,NEW.category,NEW.category_version,NEW.command_binding,NEW.source_binding,
     NEW.fence_revision,NEW.ordinal,NEW.created_at)
    IS DISTINCT FROM
    (OLD.user_id,OLD.saga_id,OLD.category,OLD.category_version,OLD.command_binding,OLD.source_binding,
     OLD.fence_revision,OLD.ordinal,OLD.created_at)
    OR (NEW.state IS DISTINCT FROM OLD.state AND NEW.state<>'COMPLETE')
    OR (OLD.owner_revision IS NOT NULL AND NEW.owner_revision IS NULL)
    OR (OLD.completed_at IS NOT NULL AND NEW.completed_at IS DISTINCT FROM OLD.completed_at)
    OR (OLD.completed_at IS NULL AND NEW.completed_at IS NOT NULL
        AND NEW.state<>'COMPLETE')
    OR (NEW.state='COMPLETE' AND NEW.owner_revision IS NULL)
    OR (NEW.state='INCOMPLETE' AND OLD.owner_revision IS NOT NULL)
    OR (OLD.owner_revision IS NULL AND NEW.owner_revision IS NOT NULL
        AND NEW.state<>'COMPLETE')
    OR (OLD.owner_revision IS NOT NULL AND NEW.owner_revision<OLD.owner_revision) THEN
  RAISE EXCEPTION 'PrivateData category receipt is immutable or nonmonotonic' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_private_data_category_guard BEFORE UPDATE OR DELETE ON public.concert_private_data_category_progress
 FOR EACH ROW EXECUTE FUNCTION public.concert_private_data_category_guard();
CREATE TRIGGER concert_private_data_category_no_truncate BEFORE TRUNCATE ON public.concert_private_data_category_progress
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_private_data_category_guard();
