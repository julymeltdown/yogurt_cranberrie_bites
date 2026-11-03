-- Owning-service observation revision only. This is not a remote transaction fence.
CREATE TABLE public.friend_concert_relationship_revision (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    owner_epoch uuid NOT NULL DEFAULT gen_random_uuid(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision BETWEEN 1 AND 9007199254740991)
);
INSERT INTO public.friend_concert_relationship_revision(singleton) VALUES(true);
CREATE FUNCTION public.friend_concert_relationship_revision_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'relationship revision identity is permanent' USING ERRCODE='23514';
    END IF;
    IF NEW.singleton IS DISTINCT FROM OLD.singleton OR NEW.owner_epoch IS DISTINCT FROM OLD.owner_epoch
       OR NEW.revision <> OLD.revision + 1 THEN
        RAISE EXCEPTION 'relationship revision must advance exactly one' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER friend_concert_relationship_revision_guard BEFORE INSERT OR UPDATE OR DELETE
    ON public.friend_concert_relationship_revision FOR EACH ROW EXECUTE FUNCTION public.friend_concert_relationship_revision_guard();
CREATE TRIGGER friend_concert_relationship_revision_no_truncate BEFORE TRUNCATE
    ON public.friend_concert_relationship_revision FOR EACH STATEMENT EXECUTE FUNCTION public.friend_concert_relationship_revision_guard();
CREATE FUNCTION public.friend_concert_relationship_revision_bump() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE public.friend_concert_relationship_revision SET revision=revision+1 WHERE singleton;
    IF NOT FOUND THEN RAISE EXCEPTION 'relationship revision missing' USING ERRCODE='23514'; END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER friend_relationship_user_blocks_revision
    AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.user_blocks
    FOR EACH STATEMENT EXECUTE FUNCTION public.friend_concert_relationship_revision_bump();
CREATE TRIGGER friend_relationship_friend_user_deletion_tombstones_revision
    AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.friend_user_deletion_tombstones
    FOR EACH STATEMENT EXECUTE FUNCTION public.friend_concert_relationship_revision_bump();
