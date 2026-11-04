-- BE42: one immutable Auth deletion identity; target completion is separate.
CREATE TABLE public.concert_world_account_deletion_fences (
    user_id uuid PRIMARY KEY CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    saga_id uuid NOT NULL UNIQUE CHECK (saga_id <> '00000000-0000-0000-0000-000000000000'),
    identity_binding bytea NOT NULL CHECK (octet_length(identity_binding)=32),
    fence_revision bigint NOT NULL DEFAULT 1 CHECK (fence_revision=1),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(user_id,saga_id)
);
CREATE FUNCTION public.concert_world_deletion_fence_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'permanent World deletion fence is immutable' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER concert_world_deletion_fence_immutable
BEFORE UPDATE OR DELETE ON public.concert_world_account_deletion_fences
FOR EACH ROW EXECUTE FUNCTION public.concert_world_deletion_fence_immutable();
CREATE TRIGGER concert_world_deletion_fence_no_truncate
BEFORE TRUNCATE ON public.concert_world_account_deletion_fences
FOR EACH STATEMENT EXECUTE FUNCTION public.concert_world_deletion_fence_immutable();

CREATE TABLE public.concert_world_ticket_deletion_progress (
    user_id uuid PRIMARY KEY,
    saga_id uuid NOT NULL UNIQUE,
    command_binding bytea NOT NULL CHECK (octet_length(command_binding)=32),
    source_binding bytea NOT NULL CHECK (octet_length(source_binding)=32),
    cursor_digest bytea CHECK (octet_length(cursor_digest)=32),
    revoked_count bigint NOT NULL DEFAULT 0 CHECK (revoked_count>=0),
    progress_revision bigint NOT NULL DEFAULT 0 CHECK (progress_revision>=0),
    completed_at timestamptz,
    FOREIGN KEY(user_id,saga_id) REFERENCES public.concert_world_account_deletion_fences(user_id,saga_id)
);
CREATE FUNCTION public.concert_world_ticket_deletion_monotonic() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'World ticket deletion receipt cannot be removed' USING ERRCODE='23514';
    END IF;
    IF OLD.completed_at IS NOT NULL OR
       (NEW.user_id,NEW.saga_id,NEW.command_binding,NEW.source_binding) IS DISTINCT FROM
       (OLD.user_id,OLD.saga_id,OLD.command_binding,OLD.source_binding) OR
       NEW.revoked_count < OLD.revoked_count OR
       NEW.progress_revision <> OLD.progress_revision+1 OR
       (OLD.cursor_digest IS NOT NULL AND (NEW.cursor_digest IS NULL OR NEW.cursor_digest < OLD.cursor_digest)) THEN
        RAISE EXCEPTION 'World ticket deletion receipt is immutable or nonmonotonic' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_world_ticket_deletion_monotonic
BEFORE UPDATE OR DELETE ON public.concert_world_ticket_deletion_progress
FOR EACH ROW EXECUTE FUNCTION public.concert_world_ticket_deletion_monotonic();
CREATE TRIGGER concert_world_ticket_deletion_no_truncate
BEFORE TRUNCATE ON public.concert_world_ticket_deletion_progress
FOR EACH STATEMENT EXECUTE FUNCTION public.concert_world_ticket_deletion_monotonic();
CREATE INDEX concert_world_ticket_pending_user
ON public.concert_world_tickets(user_id,ticket_digest) WHERE consumed_at IS NULL;
