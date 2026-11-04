-- BE42 Age owner progress. Audit receipts and runtime witnesses remain intact.
CREATE TABLE public.concert_age_deletion_progress (
    user_id uuid PRIMARY KEY,
    saga_id uuid NOT NULL UNIQUE,
    command_binding bytea NOT NULL CHECK (octet_length(command_binding)=32),
    source_binding bytea NOT NULL CHECK (octet_length(source_binding)=32),
    cursor_verification uuid,
    erased_claims bigint NOT NULL DEFAULT 0 CHECK (erased_claims>=0),
    erased_verifications bigint NOT NULL DEFAULT 0 CHECK (erased_verifications>=0),
    progress_revision bigint NOT NULL DEFAULT 0 CHECK (progress_revision>=0),
    completed_at timestamptz,
    FOREIGN KEY(user_id,saga_id) REFERENCES public.concert_world_account_deletion_fences(user_id,saga_id),
    CHECK (completed_at IS NULL OR progress_revision>0)
);
CREATE FUNCTION public.concert_age_deletion_progress_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='INSERT' THEN
        IF NEW.progress_revision<>0 OR NEW.erased_claims<>0 OR NEW.erased_verifications<>0 OR NEW.completed_at IS NOT NULL OR NEW.cursor_verification IS NOT NULL THEN
            RAISE EXCEPTION 'Age deletion must begin without a terminal receipt' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'Age deletion progress cannot be removed' USING ERRCODE='23514';
    END IF;
    IF OLD.completed_at IS NOT NULL OR
       (NEW.user_id,NEW.saga_id,NEW.command_binding,NEW.source_binding) IS DISTINCT FROM
       (OLD.user_id,OLD.saga_id,OLD.command_binding,OLD.source_binding) OR
       NEW.erased_claims<OLD.erased_claims OR NEW.erased_verifications<OLD.erased_verifications OR
       NEW.progress_revision<>OLD.progress_revision+1 OR
       (OLD.cursor_verification IS NOT NULL AND (NEW.cursor_verification IS NULL OR NEW.cursor_verification<OLD.cursor_verification)) THEN
        RAISE EXCEPTION 'Age deletion progress must be exact and monotonic' USING ERRCODE='23514';
    END IF;
    IF NEW.completed_at IS NOT NULL AND
       (EXISTS(SELECT 1 FROM public.concert_age_verifications WHERE user_id=NEW.user_id) OR
        EXISTS(SELECT 1 FROM public.concert_age_claims WHERE user_id=NEW.user_id) OR
        EXISTS(SELECT 1 FROM public.concert_age_owners WHERE user_id=NEW.user_id)) THEN
        RAISE EXCEPTION 'Age deletion cannot finish with retained private rows' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_age_deletion_progress_guard
BEFORE INSERT OR UPDATE OR DELETE ON public.concert_age_deletion_progress
FOR EACH ROW EXECUTE FUNCTION public.concert_age_deletion_progress_guard();
CREATE TRIGGER concert_age_deletion_progress_no_truncate
BEFORE TRUNCATE ON public.concert_age_deletion_progress
FOR EACH STATEMENT EXECUTE FUNCTION public.concert_age_deletion_progress_guard();

-- Defense against a late callback or any recreated source after canonical
-- deletion. Same advisory key as lock_user_in; this is not a second fence.
CREATE FUNCTION public.concert_age_account_writer_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='UPDATE' AND NEW.user_id IS DISTINCT FROM OLD.user_id THEN
        RAISE EXCEPTION 'Age owner identity cannot change' USING ERRCODE='23514';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:' || NEW.user_id::text,0));
    IF EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences WHERE user_id=NEW.user_id) THEN
        RAISE EXCEPTION 'Age source is permanently fenced by account deletion' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_age_verification_account_writer_guard
BEFORE INSERT OR UPDATE ON public.concert_age_verifications
FOR EACH ROW EXECUTE FUNCTION public.concert_age_account_writer_guard();
CREATE TRIGGER concert_age_claim_account_writer_guard
BEFORE INSERT OR UPDATE ON public.concert_age_claims
FOR EACH ROW EXECUTE FUNCTION public.concert_age_account_writer_guard();
CREATE TRIGGER concert_age_owner_account_writer_guard
BEFORE INSERT OR UPDATE ON public.concert_age_owners
FOR EACH ROW EXECUTE FUNCTION public.concert_age_account_writer_guard();
