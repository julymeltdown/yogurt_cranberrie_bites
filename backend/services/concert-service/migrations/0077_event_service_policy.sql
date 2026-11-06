-- Operational service policy: immutable per-event publications, no seeded policy.
-- A missing, withdrawn or expired publication can only ever read as UNAVAILABLE.
CREATE TABLE public.concert_event_service_policy (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
    policy_revision bigint NOT NULL CHECK (policy_revision BETWEEN 1 AND 9007199254740991),
    event_revision bigint NOT NULL CHECK (event_revision BETWEEN 1 AND 9007199254740991),
    publication_state text NOT NULL CHECK (publication_state IN ('PUBLISHED','WITHDRAWN')),
    mode text CHECK (mode IN ('NORMAL','MAINTENANCE')),
    minimum_client_version text,
    published_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    expires_at timestamptz,
    reason text NOT NULL CHECK (octet_length(reason) BETWEEN 1 AND 500 AND btrim(reason)=reason),
    PRIMARY KEY (event_id, policy_revision),
    CHECK ((publication_state='WITHDRAWN' AND mode IS NULL AND minimum_client_version IS NULL AND expires_at IS NULL)
        OR (publication_state='PUBLISHED' AND mode IS NOT NULL AND minimum_client_version IS NOT NULL
            AND expires_at IS NOT NULL AND expires_at>published_at
            AND expires_at<=published_at+interval '24 hours')),
    CHECK (minimum_client_version IS NULL OR (
        minimum_client_version ~ '^(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})$'
        AND split_part(minimum_client_version,'.',1)::integer<=65535
        AND split_part(minimum_client_version,'.',2)::integer<=65535
        AND split_part(minimum_client_version,'.',3)::integer<=65535))
);

CREATE FUNCTION public.concert_service_policy_insert_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE current_revision bigint; event_rev bigint;
BEGIN
    SELECT revision INTO STRICT event_rev FROM public.concert_events WHERE event_id=NEW.event_id FOR UPDATE;
    SELECT coalesce(max(policy_revision),0) INTO current_revision
        FROM public.concert_event_service_policy WHERE event_id=NEW.event_id;
    IF NEW.policy_revision<>current_revision+1 OR NEW.event_revision<>event_rev THEN
        RAISE EXCEPTION 'invalid service policy revision' USING ERRCODE='23514';
    END IF;
    NEW.published_at:=clock_timestamp();
    IF NEW.publication_state='PUBLISHED' AND NEW.expires_at<=NEW.published_at THEN
        RAISE EXCEPTION 'service policy expiry already passed' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER concert_service_policy_insert_guard BEFORE INSERT ON public.concert_event_service_policy
    FOR EACH ROW EXECUTE FUNCTION public.concert_service_policy_insert_guard();

CREATE FUNCTION public.concert_service_policy_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'service policy history is immutable' USING ERRCODE='23514';
END $$;
CREATE TRIGGER concert_service_policy_immutable BEFORE UPDATE OR DELETE ON public.concert_event_service_policy
    FOR EACH ROW EXECUTE FUNCTION public.concert_service_policy_immutable();
