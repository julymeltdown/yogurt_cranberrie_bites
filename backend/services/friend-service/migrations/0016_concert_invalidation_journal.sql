-- C05 rev1, new Friend-owned replay journal. 0001..0015 remain byte-identical.
-- The observation epoch/revision in 0015 is NOT a replay epoch/cursor.
CREATE TABLE public.friend_world_invalidation_head (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    owner_epoch uuid NOT NULL UNIQUE DEFAULT gen_random_uuid()
        CHECK (owner_epoch <> '00000000-0000-0000-0000-000000000000'),
    sequence bigint NOT NULL DEFAULT 0 CHECK (sequence BETWEEN 0 AND 9007199254740991)
);
INSERT INTO public.friend_world_invalidation_head(singleton) VALUES (true);
CREATE TABLE public.friend_world_invalidation_journal (
    owner_epoch uuid NOT NULL REFERENCES public.friend_world_invalidation_head(owner_epoch),
    sequence bigint NOT NULL CHECK (sequence BETWEEN 1 AND 9007199254740991),
    signal_id uuid NOT NULL UNIQUE DEFAULT gen_random_uuid()
        CHECK (signal_id <> '00000000-0000-0000-0000-000000000000'),
    reason text NOT NULL,
    user_id uuid NOT NULL CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    peer_id uuid CHECK (peer_id <> '00000000-0000-0000-0000-000000000000'),
    PRIMARY KEY(owner_epoch, sequence),
    CHECK ((reason = 'FRIEND_RELATIONSHIP_CHANGED' AND peer_id IS NOT NULL AND user_id < peer_id)
        OR (reason = 'FRIEND_USER_REMOVED' AND peer_id IS NULL))
);
CREATE FUNCTION public.friend_world_head_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- Only the journal's BEFORE INSERT trigger advances the counter. Arbitrary
    -- direct head changes cannot create gaps or reset a durable checkpoint.
    IF TG_OP <> 'UPDATE' OR pg_trigger_depth() < 2 THEN
        RAISE EXCEPTION 'World invalidation identity is permanent' USING ERRCODE='23514';
    END IF;
    IF NEW.singleton IS DISTINCT FROM OLD.singleton OR NEW.owner_epoch IS DISTINCT FROM OLD.owner_epoch
       OR NEW.sequence <> OLD.sequence + 1 THEN
        RAISE EXCEPTION 'World invalidation cursor must advance exactly one' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER friend_world_head_guard BEFORE INSERT OR UPDATE OR DELETE ON public.friend_world_invalidation_head
    FOR EACH ROW EXECUTE FUNCTION public.friend_world_head_guard();
CREATE TRIGGER friend_world_head_no_truncate BEFORE TRUNCATE ON public.friend_world_invalidation_head
    FOR EACH STATEMENT EXECUTE FUNCTION public.friend_world_head_guard();

CREATE FUNCTION public.friend_world_journal_assign() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.owner_epoch IS NOT NULL OR NEW.sequence IS NOT NULL THEN
        RAISE EXCEPTION 'World invalidation cursor is producer-assigned' USING ERRCODE='23514';
    END IF;
    -- Preserve 0015's lock ordering even for a direct owner-local journal insert:
    -- relationship revision FIRST, durable journal head SECOND. Never reversed.
    PERFORM 1 FROM public.friend_concert_relationship_revision WHERE singleton FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'Friend relationship revision missing' USING ERRCODE='23514'; END IF;
    UPDATE public.friend_world_invalidation_head SET sequence=sequence+1 WHERE singleton
        RETURNING owner_epoch,sequence INTO NEW.owner_epoch,NEW.sequence;
    IF NOT FOUND THEN RAISE EXCEPTION 'World invalidation identity missing' USING ERRCODE='23514'; END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER friend_world_journal_assign BEFORE INSERT ON public.friend_world_invalidation_journal
    FOR EACH ROW EXECUTE FUNCTION public.friend_world_journal_assign();
CREATE FUNCTION public.friend_world_journal_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'World invalidation journal is append-only' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER friend_world_journal_immutable BEFORE UPDATE OR DELETE ON public.friend_world_invalidation_journal
    FOR EACH ROW EXECUTE FUNCTION public.friend_world_journal_immutable();
CREATE TRIGGER friend_world_journal_no_truncate BEFORE TRUNCATE ON public.friend_world_invalidation_journal
    FOR EACH STATEMENT EXECUTE FUNCTION public.friend_world_journal_immutable();

-- Every transactional allocation must have a retained row at commit, including
-- owner-local INSERT ... ON CONFLICT DO NOTHING. An index lookup per allocation
-- rejects a suppressed insert without scanning history or using MAX to allocate.
CREATE FUNCTION public.friend_world_head_complete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM public.friend_world_invalidation_journal
        WHERE owner_epoch=NEW.owner_epoch AND sequence=NEW.sequence) THEN
        RAISE EXCEPTION 'World invalidation allocation has no retained signal' USING ERRCODE='23514';
    END IF;
    RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER friend_world_head_complete AFTER UPDATE ON public.friend_world_invalidation_head
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.friend_world_head_complete();

CREATE FUNCTION public.friend_capture_world_invalidation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE item record; source_sql text;
BEGIN
    IF TG_TABLE_NAME = 'user_blocks' THEN
        source_sql := CASE TG_OP
            WHEN 'INSERT' THEN 'SELECT DISTINCT least(blocker_id,blocked_id) AS user_id,greatest(blocker_id,blocked_id) AS peer_id FROM journal_new ORDER BY 1,2'
            WHEN 'DELETE' THEN 'SELECT DISTINCT least(blocker_id,blocked_id) AS user_id,greatest(blocker_id,blocked_id) AS peer_id FROM journal_old ORDER BY 1,2'
            WHEN 'UPDATE' THEN 'SELECT least(blocker_id,blocked_id) AS user_id,greatest(blocker_id,blocked_id) AS peer_id FROM journal_old UNION SELECT least(blocker_id,blocked_id),greatest(blocker_id,blocked_id) FROM journal_new ORDER BY 1,2'
        END;
        FOR item IN EXECUTE source_sql LOOP
            INSERT INTO public.friend_world_invalidation_journal(reason,user_id,peer_id)
                VALUES ('FRIEND_RELATIONSHIP_CHANGED',item.user_id,item.peer_id);
        END LOOP;
    ELSIF TG_TABLE_NAME = 'friend_user_deletion_tombstones' THEN
        source_sql := CASE TG_OP
            WHEN 'INSERT' THEN 'SELECT DISTINCT user_id FROM journal_new ORDER BY 1'
            WHEN 'DELETE' THEN 'SELECT DISTINCT user_id FROM journal_old ORDER BY 1'
            WHEN 'UPDATE' THEN 'SELECT user_id FROM journal_old UNION SELECT user_id FROM journal_new ORDER BY 1'
        END;
        FOR item IN EXECUTE source_sql LOOP
            INSERT INTO public.friend_world_invalidation_journal(reason,user_id)
                VALUES ('FRIEND_USER_REMOVED',item.user_id);
        END LOOP;
    ELSE
        RAISE EXCEPTION 'Unsupported World invalidation source' USING ERRCODE='23514';
    END IF;
    RETURN NULL;
END;
$$;
-- PostgreSQL executes equal-kind triggers by name. These AFTER STATEMENT names
-- sort AFTER friend_relationship_* from 0015, including empty/upsert statements.
CREATE TRIGGER friend_world_blocks_insert AFTER INSERT ON public.user_blocks
    REFERENCING NEW TABLE AS journal_new FOR EACH STATEMENT EXECUTE FUNCTION public.friend_capture_world_invalidation();
CREATE TRIGGER friend_world_blocks_update AFTER UPDATE ON public.user_blocks
    REFERENCING OLD TABLE AS journal_old NEW TABLE AS journal_new FOR EACH STATEMENT EXECUTE FUNCTION public.friend_capture_world_invalidation();
CREATE TRIGGER friend_world_blocks_delete AFTER DELETE ON public.user_blocks
    REFERENCING OLD TABLE AS journal_old FOR EACH STATEMENT EXECUTE FUNCTION public.friend_capture_world_invalidation();
CREATE TRIGGER friend_world_tombstones_insert AFTER INSERT ON public.friend_user_deletion_tombstones
    REFERENCING NEW TABLE AS journal_new FOR EACH STATEMENT EXECUTE FUNCTION public.friend_capture_world_invalidation();
CREATE TRIGGER friend_world_tombstones_update AFTER UPDATE ON public.friend_user_deletion_tombstones
    REFERENCING OLD TABLE AS journal_old NEW TABLE AS journal_new FOR EACH STATEMENT EXECUTE FUNCTION public.friend_capture_world_invalidation();
CREATE TRIGGER friend_world_tombstones_delete AFTER DELETE ON public.friend_user_deletion_tombstones
    REFERENCING OLD TABLE AS journal_old FOR EACH STATEMENT EXECUTE FUNCTION public.friend_capture_world_invalidation();

-- TRUNCATE has no row transition table. The contract has no owner-wide scope;
-- require DELETE rather than silently erase facts without scoped invalidations.
CREATE FUNCTION public.friend_world_source_no_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'World invalidation sources require row-scoped deletion' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER friend_world_source_no_truncate BEFORE TRUNCATE ON public.user_blocks
    FOR EACH STATEMENT EXECUTE FUNCTION public.friend_world_source_no_truncate();
CREATE TRIGGER friend_world_source_no_truncate BEFORE TRUNCATE ON public.friend_user_deletion_tombstones
    FOR EACH STATEMENT EXECUTE FUNCTION public.friend_world_source_no_truncate();
