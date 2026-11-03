-- Owning-service observation revision only. This is not a remote transaction fence.
CREATE TABLE public.auth_concert_relationship_revision (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    owner_epoch uuid NOT NULL DEFAULT gen_random_uuid(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision BETWEEN 1 AND 9007199254740991)
);
INSERT INTO public.auth_concert_relationship_revision(singleton) VALUES(true);
CREATE FUNCTION public.auth_concert_relationship_revision_guard() RETURNS trigger LANGUAGE plpgsql AS $$
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
CREATE TRIGGER auth_concert_relationship_revision_guard BEFORE INSERT OR UPDATE OR DELETE
    ON public.auth_concert_relationship_revision FOR EACH ROW EXECUTE FUNCTION public.auth_concert_relationship_revision_guard();
CREATE TRIGGER auth_concert_relationship_revision_no_truncate BEFORE TRUNCATE
    ON public.auth_concert_relationship_revision FOR EACH STATEMENT EXECUTE FUNCTION public.auth_concert_relationship_revision_guard();
CREATE FUNCTION public.auth_concert_relationship_revision_bump() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE public.auth_concert_relationship_revision SET revision=revision+1 WHERE singleton;
    IF NOT FOUND THEN RAISE EXCEPTION 'relationship revision missing' USING ERRCODE='23514'; END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER auth_relationship_users_revision
    AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.users
    FOR EACH STATEMENT EXECUTE FUNCTION public.auth_concert_relationship_revision_bump();
CREATE TRIGGER auth_relationship_refresh_tokens_revision
    AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON public.refresh_tokens
    FOR EACH STATEMENT EXECUTE FUNCTION public.auth_concert_relationship_revision_bump();
