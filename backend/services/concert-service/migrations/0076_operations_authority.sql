-- BE38 operator authority and reviewed-notice provenance. No role assignment,
-- reviewer or notice source is seeded here: provisioning is an explicit operator
-- concern performed outside the public product routes.
CREATE TABLE public.concert_event_operator_roles (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
    user_id uuid NOT NULL,
    role text NOT NULL CHECK (role IN ('MODERATOR', 'OPERATOR')),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision BETWEEN 1 AND 9007199254740991),
    granted_by uuid NOT NULL,
    granted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    revoked_at timestamptz,
    PRIMARY KEY (event_id, user_id, role),
    CONSTRAINT concert_operator_role_grant_bounds CHECK (granted_at <= clock_timestamp()),
    CONSTRAINT concert_operator_role_revocation_bounds CHECK (
        revoked_at IS NULL OR revoked_at >= granted_at
    )
);

-- A notice may be published only when this exact content/source/confirmation/
-- state digest was reviewed by this actor for this event. The digest binds the
-- immutable notice bytes; any edit requires a new review row.
CREATE TABLE public.concert_operation_notice_reviews (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
    input_digest bytea NOT NULL CHECK (octet_length(input_digest) = 32),
    source_uri text NOT NULL CHECK (
        source_uri ~ '^https://'
        AND char_length(source_uri) BETWEEN 1 AND 2048
        AND source_uri !~ '[[:cntrl:]]'
    ),
    reviewed_by uuid NOT NULL,
    reviewed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision BETWEEN 1 AND 9007199254740991),
    PRIMARY KEY (event_id, input_digest),
    CONSTRAINT concert_notice_review_time CHECK (reviewed_at <= clock_timestamp())
);

-- Reviews and role grants are owner-operated provenance, not client-editable
-- command output. Their durable revision records the exact reviewed/granted state.
CREATE FUNCTION public.concert_operation_notice_review_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'notice reviews are immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.event_id <> OLD.event_id OR NEW.input_digest <> OLD.input_digest THEN
        RAISE EXCEPTION 'notice review identity is immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.reviewed_by IS DISTINCT FROM OLD.reviewed_by
       OR NEW.source_uri IS DISTINCT FROM OLD.source_uri
       OR NEW.reviewed_at > clock_timestamp() THEN
        RAISE EXCEPTION 'notice review provenance is invalid' USING ERRCODE = '23514';
    END IF;
    NEW.revision := OLD.revision + 1;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_operation_notice_review_revision
    BEFORE UPDATE OR DELETE ON public.concert_operation_notice_reviews
    FOR EACH ROW EXECUTE FUNCTION public.concert_operation_notice_review_guard();

CREATE FUNCTION public.concert_event_operator_role_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'operator role grants are immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.event_id <> OLD.event_id
       OR NEW.user_id <> OLD.user_id
       OR NEW.role <> OLD.role
       OR NEW.granted_by <> OLD.granted_by
       OR NEW.granted_at <> OLD.granted_at THEN
        RAISE EXCEPTION 'operator role grant identity is immutable' USING ERRCODE = '23514';
    END IF;
    IF OLD.revoked_at IS NOT NULL
       OR NEW.revoked_at IS NULL
       OR NEW.revoked_at < OLD.granted_at
       OR NEW.revoked_at > clock_timestamp() THEN
        RAISE EXCEPTION 'operator role revocation is invalid' USING ERRCODE = '23514';
    END IF;
    NEW.revision := OLD.revision + 1;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_event_operator_role_revision
    BEFORE UPDATE OR DELETE ON public.concert_event_operator_roles
    FOR EACH ROW EXECUTE FUNCTION public.concert_event_operator_role_guard();
