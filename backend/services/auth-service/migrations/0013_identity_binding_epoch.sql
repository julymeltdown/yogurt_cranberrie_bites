-- Exact provider references need an owner generation: linked_at uses now(),
-- which is transaction-stable and cannot distinguish repeated relinks.
-- This migration is applied only by the owning Auth SQLx migration lifecycle.
CREATE SEQUENCE public.auth_identity_binding_epoch
    AS bigint MINVALUE 1 MAXVALUE 9007199254740991 NO CYCLE CACHE 1;

ALTER TABLE public.user_identities ADD COLUMN binding_epoch bigint;
UPDATE public.user_identities
    SET binding_epoch = nextval('public.auth_identity_binding_epoch'::regclass);
ALTER TABLE public.user_identities
    ALTER COLUMN binding_epoch SET NOT NULL,
    ADD CONSTRAINT user_identities_binding_epoch_range
        CHECK (binding_epoch BETWEEN 1 AND 9007199254740991);
ALTER SEQUENCE public.auth_identity_binding_epoch
    OWNED BY public.user_identities.binding_epoch;

-- Ignore caller-supplied generations on INSERT and UPDATE. Conservative
-- invalidation on every update also covers relink, verification and soft delete,
-- including multiple updates with the same linked_at in one transaction.
-- Sequence allocation is intentionally nontransactional: rollback leaves gaps,
-- not reusable generations. Exhaustion refuses the write; never wrap/reset it.
CREATE FUNCTION public.assign_auth_identity_binding_epoch()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $$
BEGIN
    NEW.binding_epoch := nextval('public.auth_identity_binding_epoch'::regclass);
    RETURN NEW;
END;
$$;

CREATE TRIGGER auth_identity_binding_epoch_on_write
    BEFORE INSERT OR UPDATE ON public.user_identities
    FOR EACH ROW EXECUTE FUNCTION public.assign_auth_identity_binding_epoch();
