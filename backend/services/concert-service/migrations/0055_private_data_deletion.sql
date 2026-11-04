-- BE42 PrivateData: one external identity, one immutable core receipt, and
-- a permanent writer fence. This table is deliberately separate from the
-- World ticket deletion receipt: target acknowledgement is per owned leaf.
CREATE TABLE public.concert_private_data_deletion_core (
    user_id uuid PRIMARY KEY CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    saga_id uuid NOT NULL CHECK (saga_id <> '00000000-0000-0000-0000-000000000000'),
    actor_id uuid NOT NULL CHECK (actor_id <> '00000000-0000-0000-0000-000000000000'),
    retention_until timestamptz NOT NULL,
    command_binding bytea NOT NULL CHECK (octet_length(command_binding)=32),
    source_binding bytea NOT NULL CHECK (octet_length(source_binding)=32),
    state text NOT NULL DEFAULT 'PENDING' CHECK (state IN ('PENDING','INCOMPLETE','COMPLETE')),
    next_event_id uuid,
    scanned_events bigint NOT NULL DEFAULT 0 CHECK (scanned_events>=0),
    neutralized_participants bigint NOT NULL DEFAULT 0 CHECK (neutralized_participants>=0),
    removed_avatars bigint NOT NULL DEFAULT 0 CHECK (removed_avatars>=0),
    progress_revision bigint NOT NULL DEFAULT 0 CHECK (progress_revision>=0),
    completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(user_id,saga_id)
);

CREATE TABLE public.concert_private_data_participant_fences (
    user_id uuid PRIMARY KEY CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    saga_id uuid NOT NULL CHECK (saga_id <> '00000000-0000-0000-0000-000000000000'),
    fence_revision bigint NOT NULL DEFAULT 1 CHECK (fence_revision=1),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(user_id,saga_id)
);
-- The core receipt establishes a permanent deleted-user identity. PrivateData's
-- own fence records which exact target receipt owns the personal fields; neither
-- table permits UPDATE or DELETE of the receipt/fence identity.
CREATE FUNCTION public.concert_private_data_core_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'PrivateData deletion receipt cannot be removed' USING ERRCODE='23514';
    END IF;
    IF NEW.user_id IS DISTINCT FROM OLD.user_id OR NEW.saga_id IS DISTINCT FROM OLD.saga_id
       OR NEW.actor_id IS DISTINCT FROM OLD.actor_id
       OR NEW.retention_until IS DISTINCT FROM OLD.retention_until
       OR NEW.command_binding IS DISTINCT FROM OLD.command_binding
       OR NEW.source_binding IS DISTINCT FROM OLD.source_binding
       OR OLD.state='COMPLETE'
       OR NEW.state NOT IN ('INCOMPLETE','COMPLETE')
       OR NEW.progress_revision <> OLD.progress_revision+1
       OR NEW.scanned_events < OLD.scanned_events
       OR NEW.neutralized_participants < OLD.neutralized_participants
       OR NEW.removed_avatars < OLD.removed_avatars
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR (OLD.next_event_id IS NOT NULL AND (NEW.next_event_id IS NULL OR NEW.next_event_id < OLD.next_event_id))
    THEN
        RAISE EXCEPTION 'PrivateData deletion receipt is immutable or nonmonotonic' USING ERRCODE='23514';
    END IF;
    NEW.updated_at := clock_timestamp();
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_private_data_core_guard
BEFORE UPDATE ON public.concert_private_data_deletion_core
FOR EACH ROW EXECUTE FUNCTION public.concert_private_data_core_guard();
CREATE FUNCTION public.concert_private_data_core_insert_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state <> 'PENDING' OR NEW.progress_revision <> 0
       OR NEW.completed_at IS NOT NULL OR NEW.neutralized_participants <> 0
       OR NEW.removed_avatars <> 0 OR NEW.scanned_events <> 0 THEN
        RAISE EXCEPTION 'PrivateData receipt must begin empty' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_private_data_core_insert_guard
BEFORE INSERT ON public.concert_private_data_deletion_core
FOR EACH ROW EXECUTE FUNCTION public.concert_private_data_core_insert_guard();
CREATE FUNCTION public.concert_private_data_participant_fence_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'permanent PrivateData participant fence is immutable' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER concert_private_data_participant_fence_immutable
BEFORE UPDATE OR DELETE ON public.concert_private_data_participant_fences
FOR EACH ROW EXECUTE FUNCTION public.concert_private_data_participant_fence_immutable();
CREATE TRIGGER concert_private_data_participant_fence_no_truncate
BEFORE TRUNCATE ON public.concert_private_data_participant_fences
FOR EACH STATEMENT EXECUTE FUNCTION public.concert_private_data_participant_fence_immutable();

-- The durable leaf receipt is bounded: one cursor and bounded counters are
-- enough to resume all-event work after process or database restart.
CREATE INDEX concert_private_data_participant_user
ON public.concert_participants(user_id,event_id);
CREATE INDEX concert_private_data_avatar_user
ON public.concert_avatar_settings(user_id,event_id);

-- A writer may not repopulate personal choices after the permanent fence. The
-- owner-neutral values are canonical existing defaults: PARADE theme, all
-- effects off, and no avatar row. These are not client-supplied assertions.
-- The controlled deletion transaction clears all personal values, then restores
-- only those neutral defaults for the participant key that dependent aggregates
-- still reference.
CREATE FUNCTION public.concert_private_data_participant_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE fence_user uuid;
BEGIN
    fence_user := CASE WHEN TG_OP='DELETE' THEN OLD.user_id ELSE NEW.user_id END;
    IF coalesce(current_setting('mcr.private_data_deletion_user',true),'') <> fence_user::text
       AND EXISTS (SELECT 1 FROM public.concert_private_data_participant_fences WHERE user_id=fence_user) THEN
        RAISE EXCEPTION 'PrivateData recreation is permanently denied' USING ERRCODE='23514';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END;
$$;
CREATE TRIGGER concert_private_data_participant_write_guard
BEFORE INSERT OR UPDATE OR DELETE ON public.concert_participants
FOR EACH ROW EXECUTE FUNCTION public.concert_private_data_participant_write_guard();
CREATE FUNCTION public.concert_private_data_avatar_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE fence_user uuid;
BEGIN
    fence_user := CASE WHEN TG_OP='DELETE' THEN OLD.user_id ELSE NEW.user_id END;
    IF coalesce(current_setting('mcr.private_data_deletion_user',true),'') <> fence_user::text
       AND EXISTS (SELECT 1 FROM public.concert_private_data_participant_fences WHERE user_id=fence_user) THEN
        RAISE EXCEPTION 'PrivateData avatar recreation is permanently denied' USING ERRCODE='23514';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END;
$$;
CREATE TRIGGER concert_private_data_avatar_write_guard
BEFORE INSERT OR UPDATE OR DELETE ON public.concert_avatar_settings
FOR EACH ROW EXECUTE FUNCTION public.concert_private_data_avatar_write_guard();

REVOKE ALL ON FUNCTION public.concert_private_data_core_guard() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.concert_private_data_core_insert_guard() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.concert_private_data_participant_fence_immutable() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.concert_private_data_participant_write_guard() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.concert_private_data_avatar_write_guard() FROM PUBLIC;
