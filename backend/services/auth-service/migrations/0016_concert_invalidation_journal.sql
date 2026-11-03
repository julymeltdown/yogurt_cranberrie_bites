-- Auth-owned replayable invalidation hints. Not authority or a remote fence.
-- SQLx applies this atomically after published 0015; never edit earlier sources.
CREATE TABLE public.auth_concert_invalidation_head (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    owner_epoch uuid NOT NULL DEFAULT gen_random_uuid() CHECK (owner_epoch <> '00000000-0000-0000-0000-000000000000'),
    committed_cursor bigint NOT NULL DEFAULT 0 CHECK (committed_cursor BETWEEN 0 AND 9007199254740991)
);
INSERT INTO public.auth_concert_invalidation_head(singleton) VALUES (true);

CREATE TABLE public.auth_concert_invalidation_journal (
    cursor bigint PRIMARY KEY CHECK (cursor BETWEEN 1 AND 9007199254740991),
    owner_epoch uuid NOT NULL,
    signal_id uuid NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    family_id uuid NULL CHECK (family_id <> '00000000-0000-0000-0000-000000000000'),
    kind text NOT NULL CHECK (kind IN ('SESSION_CHANGED', 'USER_AUTHORITY_CHANGED', 'CREDENTIALS_CHANGED', 'USER_REMOVED')),
    CHECK ((kind = 'SESSION_CHANGED') = (family_id IS NOT NULL)),
    CHECK (signal_id <> '00000000-0000-0000-0000-000000000000')
);

CREATE FUNCTION public.auth_concert_invalidation_head_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'invalidation head is permanent' USING ERRCODE = '23514';
    END IF;
    IF NEW.singleton IS DISTINCT FROM OLD.singleton OR NEW.owner_epoch IS DISTINCT FROM OLD.owner_epoch
       OR NEW.committed_cursor <> OLD.committed_cursor + 1 THEN
        RAISE EXCEPTION 'invalidation head must advance exactly one' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER auth_concert_invalidation_head_guard BEFORE INSERT OR UPDATE OR DELETE
    ON public.auth_concert_invalidation_head FOR EACH ROW EXECUTE FUNCTION public.auth_concert_invalidation_head_guard();
CREATE TRIGGER auth_concert_invalidation_head_no_truncate BEFORE TRUNCATE
    ON public.auth_concert_invalidation_head FOR EACH STATEMENT EXECUTE FUNCTION public.auth_concert_invalidation_head_guard();

CREATE FUNCTION public.auth_concert_invalidation_record_guard() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'invalidation records are immutable' USING ERRCODE = '23514';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM public.auth_concert_invalidation_head
                   WHERE singleton AND owner_epoch = NEW.owner_epoch AND committed_cursor = NEW.cursor) THEN
        RAISE EXCEPTION 'invalidation cursor must match allocated head' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER auth_concert_invalidation_record_guard BEFORE INSERT OR UPDATE OR DELETE
    ON public.auth_concert_invalidation_journal FOR EACH ROW EXECUTE FUNCTION public.auth_concert_invalidation_record_guard();
CREATE TRIGGER auth_concert_invalidation_records_no_truncate BEFORE TRUNCATE
    ON public.auth_concert_invalidation_journal FOR EACH STATEMENT EXECUTE FUNCTION public.auth_concert_invalidation_record_guard();

CREATE FUNCTION public.auth_append_concert_invalidation(subject uuid, family uuid, change_kind text)
RETURNS void LANGUAGE plpgsql SET search_path = pg_catalog AS $$
DECLARE epoch uuid; allocated bigint;
BEGIN
    -- Existing 0014 revision serialization comes FIRST: older statements in
    -- this transaction may already hold it. Never introduce head -> revision.
    PERFORM 1 FROM public.auth_concert_relationship_revision WHERE singleton FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'relationship revision missing' USING ERRCODE = '23514'; END IF;
    -- Transactional row update, not nextval/MAX. This lock is held until commit,
    -- including every entry in a multi-row/multi-statement owning transaction.
    UPDATE public.auth_concert_invalidation_head SET committed_cursor = committed_cursor + 1
      WHERE singleton AND committed_cursor < 9007199254740991
      RETURNING owner_epoch, committed_cursor INTO epoch, allocated;
    IF NOT FOUND THEN RAISE EXCEPTION 'invalidation head unavailable' USING ERRCODE = '23514'; END IF;
    INSERT INTO public.auth_concert_invalidation_journal(cursor, owner_epoch, user_id, family_id, kind)
      VALUES (allocated, epoch, subject, family, change_kind);
END;
$$;
REVOKE EXECUTE ON FUNCTION public.auth_append_concert_invalidation(uuid, uuid, text) FROM PUBLIC;

CREATE FUNCTION public.auth_capture_concert_session_invalidation() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        PERFORM public.auth_append_concert_invalidation(OLD.user_id, OLD.family_id, 'SESSION_CHANGED');
    END IF;
    IF TG_OP = 'INSERT' THEN
        PERFORM public.auth_append_concert_invalidation(NEW.user_id, NEW.family_id, 'SESSION_CHANGED');
    ELSIF TG_OP = 'UPDATE' AND (NEW.user_id IS DISTINCT FROM OLD.user_id OR NEW.family_id IS DISTINCT FROM OLD.family_id) THEN
        PERFORM public.auth_append_concert_invalidation(NEW.user_id, NEW.family_id, 'SESSION_CHANGED');
    END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER auth_capture_concert_session_invalidation AFTER INSERT OR UPDATE OR DELETE
    ON public.refresh_tokens FOR EACH ROW EXECUTE FUNCTION public.auth_capture_concert_session_invalidation();

CREATE FUNCTION public.auth_capture_concert_user_invalidation() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM public.auth_append_concert_invalidation(OLD.id, NULL, 'USER_REMOVED');
    ELSE
        IF NEW.id IS DISTINCT FROM OLD.id OR NEW.status IS DISTINCT FROM OLD.status OR NEW.deleted_at IS DISTINCT FROM OLD.deleted_at THEN
            PERFORM public.auth_append_concert_invalidation(OLD.id, NULL, 'USER_AUTHORITY_CHANGED');
            IF NEW.id IS DISTINCT FROM OLD.id THEN
                PERFORM public.auth_append_concert_invalidation(NEW.id, NULL, 'USER_AUTHORITY_CHANGED');
            END IF;
        END IF;
        IF NEW.password_hash IS DISTINCT FROM OLD.password_hash THEN
            PERFORM public.auth_append_concert_invalidation(OLD.id, NULL, 'CREDENTIALS_CHANGED');
        END IF;
    END IF;
    RETURN NULL;
END;
$$;
-- PostgreSQL orders same-event triggers by name. Preserve the inherited
-- credentials trigger's user -> refresh-row lock prefix before user-level hint.
CREATE TRIGGER zz_auth_capture_concert_user_invalidation AFTER UPDATE OR DELETE
    ON public.users FOR EACH ROW EXECUTE FUNCTION public.auth_capture_concert_user_invalidation();

-- Preserve TRUNCATE semantics too: enumerate safe bindings BEFORE rows vanish,
-- under PostgreSQL's existing exclusive source-table lock. No pruning of hints.
CREATE FUNCTION public.auth_capture_concert_source_truncate() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog AS $$
DECLARE subject record;
BEGIN
    IF TG_TABLE_NAME = 'users' THEN
        FOR subject IN SELECT id FROM public.users ORDER BY id LOOP
            PERFORM public.auth_append_concert_invalidation(subject.id, NULL, 'USER_REMOVED');
        END LOOP;
    ELSE
        FOR subject IN SELECT DISTINCT user_id, family_id FROM public.refresh_tokens ORDER BY user_id, family_id LOOP
            PERFORM public.auth_append_concert_invalidation(subject.user_id, subject.family_id, 'SESSION_CHANGED');
        END LOOP;
    END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER auth_concert_users_truncate_capture BEFORE TRUNCATE ON public.users
    FOR EACH STATEMENT EXECUTE FUNCTION public.auth_capture_concert_source_truncate();
CREATE TRIGGER auth_concert_sessions_truncate_capture BEFORE TRUNCATE ON public.refresh_tokens
    FOR EACH STATEMENT EXECUTE FUNCTION public.auth_capture_concert_source_truncate();
