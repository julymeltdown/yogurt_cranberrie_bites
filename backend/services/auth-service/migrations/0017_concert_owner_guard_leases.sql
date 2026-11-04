-- New owner-local authority ledger. Retained source revision and invalidation
-- triggers are intentionally untouched. No cross-service table access.
CREATE TABLE public.concert_owner_guard_leases (
    guard_id uuid PRIMARY KEY,
    binding_digest bytea NOT NULL CHECK (octet_length(binding_digest)=32),
    binding jsonb NOT NULL CHECK (octet_length(binding::text)<=32768),
    enrolled_subjects uuid[] NOT NULL CHECK (cardinality(enrolled_subjects) BETWEEN 1 AND 101 AND array_position(enrolled_subjects,'00000000-0000-0000-0000-000000000000'::uuid) IS NULL),
    response_scope jsonb NOT NULL CHECK (octet_length(response_scope::text)<=4096),
    scope_settlement jsonb CHECK (scope_settlement IS NULL OR octet_length(scope_settlement::text)<=32768),
    view_hash bytea NOT NULL CHECK (octet_length(view_hash)=32),
    completion_hash bytea NOT NULL CHECK (octet_length(completion_hash)=32),
    prepared_gid text NOT NULL UNIQUE CHECK (prepared_gid ~ '^mcr_oa_[af]_[0-9a-f]{32}$'),
    state text NOT NULL CHECK (state IN ('PREPARING','ACTIVE','SEALED','COMPLETED','ABANDONED')),
    preparer_pid integer CHECK (preparer_pid>0),
    preparer_started timestamptz,
    preparer_xid text CHECK (preparer_xid ~ '^[1-9][0-9]*$'),
    owner_epoch uuid,
    owner_revision bigint CHECK (owner_revision BETWEEN 1 AND 9007199254740991),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    expires_at timestamptz NOT NULL CHECK (expires_at>created_at),
    sealed_at timestamptz,
    completed_at timestamptz,
    CHECK ((preparer_pid IS NULL)=(preparer_started IS NULL) AND (preparer_pid IS NULL)=(preparer_xid IS NULL)),
    CHECK ((owner_epoch IS NULL)=(owner_revision IS NULL)),
    CHECK (state IN ('PREPARING','ABANDONED') OR owner_epoch IS NOT NULL),
    CHECK ((state IN ('SEALED','COMPLETED'))=(sealed_at IS NOT NULL)),
    CHECK ((state IN ('COMPLETED','ABANDONED'))=(completed_at IS NOT NULL)),
    CHECK (state<>'COMPLETED' OR scope_settlement IS NOT NULL)
);
CREATE INDEX concert_owner_guard_pending ON public.concert_owner_guard_leases(state,created_at)
    WHERE state NOT IN ('COMPLETED','ABANDONED');
CREATE TABLE public.concert_owner_guard_participants (
    registration_id uuid PRIMARY KEY,
    guard_id uuid NOT NULL REFERENCES public.concert_owner_guard_leases(guard_id),
    producer text NOT NULL CHECK (producer IN ('CONCERT','POST','PROFILE')),
    identity_digest bytea NOT NULL CHECK (octet_length(identity_digest)=32),
    identity jsonb NOT NULL CHECK (octet_length(identity::text)<=32768),
    state text NOT NULL DEFAULT 'PENDING' CHECK (state IN ('PENDING','COMMITTED','ABORTED')),
    settlement jsonb CHECK (settlement IS NULL OR octet_length(settlement::text)<=131072),
    registered_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    settled_at timestamptz,
    UNIQUE (guard_id,producer,identity_digest),
    CHECK ((state='PENDING')=(settled_at IS NULL)),
    CHECK ((state='PENDING')=(settlement IS NULL))
);
-- Readers take this exact lease row lock BEFORE registering; Complete takes it
-- BEFORE sealing. The source fence is a separate prepared SHARE transaction.
-- Expiry is not a DELETE/release trigger. Uncertain producer outcomes stay fenced.
CREATE FUNCTION public.concert_owner_guard_identity_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' OR NEW.guard_id<>OLD.guard_id OR NEW.binding_digest<>OLD.binding_digest
       OR NEW.binding<>OLD.binding OR NEW.response_scope<>OLD.response_scope
       OR NEW.view_hash<>OLD.view_hash OR NEW.completion_hash<>OLD.completion_hash
       OR NEW.prepared_gid<>OLD.prepared_gid OR NEW.created_at<>OLD.created_at OR NEW.expires_at<>OLD.expires_at THEN
        RAISE EXCEPTION 'owner guard identity is immutable' USING ERRCODE='23514';
    END IF;
    IF NOT ((OLD.state='ACTIVE' AND NEW.state='ACTIVE' AND OLD.enrolled_subjects <@ NEW.enrolled_subjects)
       OR (OLD.state='PREPARING' AND NEW.state IN ('PREPARING','ACTIVE','ABANDONED'))
       OR (OLD.state='ACTIVE' AND NEW.state='SEALED')
       OR (OLD.state='SEALED' AND NEW.state IN ('SEALED','COMPLETED'))) THEN
        RAISE EXCEPTION 'invalid owner guard transition' USING ERRCODE='23514';
    END IF;
    IF OLD.preparer_pid IS NOT NULL AND (NEW.preparer_pid,NEW.preparer_started,NEW.preparer_xid)
        IS DISTINCT FROM (OLD.preparer_pid,OLD.preparer_started,OLD.preparer_xid) THEN
        RAISE EXCEPTION 'owner preparer identity is immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.state='ACTIVE' AND NEW.preparer_pid IS NULL THEN
        RAISE EXCEPTION 'owner preparer identity required' USING ERRCODE='23514';
    END IF;
    IF NEW.enrolled_subjects IS DISTINCT FROM OLD.enrolled_subjects
       AND NOT (OLD.state='ACTIVE' AND NEW.state='ACTIVE' AND OLD.enrolled_subjects <@ NEW.enrolled_subjects) THEN
        RAISE EXCEPTION 'sealed enrollment is immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.scope_settlement IS NOT NULL AND NEW.state NOT IN ('SEALED','COMPLETED') THEN
        RAISE EXCEPTION 'scope settlement requires sealing' USING ERRCODE='23514';
    END IF;
    IF OLD.owner_epoch IS NOT NULL AND (NEW.owner_epoch,NEW.owner_revision) IS DISTINCT FROM (OLD.owner_epoch,OLD.owner_revision) THEN
        RAISE EXCEPTION 'owner guard source revision is immutable' USING ERRCODE='23514';
    END IF;
    IF OLD.scope_settlement IS NOT NULL AND NEW.scope_settlement IS DISTINCT FROM OLD.scope_settlement THEN
        RAISE EXCEPTION 'owner scope settlement is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER concert_owner_guard_identity_guard BEFORE UPDATE OR DELETE ON public.concert_owner_guard_leases
    FOR EACH ROW EXECUTE FUNCTION public.concert_owner_guard_identity_guard();

CREATE FUNCTION public.concert_owner_participant_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP<>'UPDATE' OR (NEW.registration_id,NEW.guard_id,NEW.producer,NEW.identity_digest,NEW.identity,NEW.registered_at)
        IS DISTINCT FROM (OLD.registration_id,OLD.guard_id,OLD.producer,OLD.identity_digest,OLD.identity,OLD.registered_at)
        OR OLD.state<>'PENDING' OR NEW.state NOT IN ('COMMITTED','ABORTED') THEN
        RAISE EXCEPTION 'invalid owner participant transition' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER concert_owner_participant_guard BEFORE UPDATE OR DELETE ON public.concert_owner_guard_participants
    FOR EACH ROW EXECUTE FUNCTION public.concert_owner_participant_guard();


CREATE TRIGGER concert_owner_guard_no_truncate BEFORE TRUNCATE ON public.concert_owner_guard_leases
    FOR EACH STATEMENT EXECUTE FUNCTION public.concert_owner_guard_identity_guard();
CREATE TRIGGER concert_owner_participant_no_truncate BEFORE TRUNCATE ON public.concert_owner_guard_participants
    FOR EACH STATEMENT EXECUTE FUNCTION public.concert_owner_participant_guard();

-- Per-user security provenance, independent of the deliberately coarse source
-- exclusion revision. Keep tombstones after deletion; UUID reuse never resets it.
CREATE TABLE public.auth_concert_user_security_generation (
    user_id uuid PRIMARY KEY CHECK (user_id<>'00000000-0000-0000-0000-000000000000'),
    generation bigint NOT NULL CHECK (generation BETWEEN 1 AND 9007199254740991)
);
INSERT INTO public.auth_concert_user_security_generation(user_id,generation) SELECT id,1 FROM public.users;
CREATE FUNCTION public.auth_concert_security_generation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='INSERT' THEN
        IF NEW.generation<>1 THEN RAISE EXCEPTION 'invalid initial security generation' USING ERRCODE='23514'; END IF;
    ELSIF TG_OP<>'UPDATE' OR NEW.user_id IS DISTINCT FROM OLD.user_id OR NEW.generation<>OLD.generation+1 THEN
        RAISE EXCEPTION 'security generation is permanent and monotonic' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER auth_concert_security_generation_guard BEFORE INSERT OR UPDATE OR DELETE
    ON public.auth_concert_user_security_generation FOR EACH ROW EXECUTE FUNCTION public.auth_concert_security_generation_guard();
CREATE TRIGGER auth_concert_security_generation_no_truncate BEFORE TRUNCATE
    ON public.auth_concert_user_security_generation FOR EACH STATEMENT EXECUTE FUNCTION public.auth_concert_security_generation_guard();
CREATE FUNCTION public.auth_concert_bump_security(subject uuid) RETURNS void
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
BEGIN
    -- Same existing source singleton first; never generation -> revision.
    PERFORM 1 FROM public.auth_concert_relationship_revision WHERE singleton FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'security source unavailable' USING ERRCODE='23514'; END IF;
    INSERT INTO public.auth_concert_user_security_generation(user_id,generation) VALUES(subject,1)
    ON CONFLICT(user_id) DO UPDATE SET generation=public.auth_concert_user_security_generation.generation+1;
END $$;
REVOKE EXECUTE ON FUNCTION public.auth_concert_bump_security(uuid) FROM PUBLIC;
CREATE FUNCTION public.auth_concert_user_security_capture() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP IN ('INSERT','DELETE') THEN
        PERFORM public.auth_concert_bump_security(CASE WHEN TG_OP='DELETE' THEN OLD.id ELSE NEW.id END);
    ELSIF (NEW.id,NEW.password_hash,NEW.status,NEW.deleted_at) IS DISTINCT FROM (OLD.id,OLD.password_hash,OLD.status,OLD.deleted_at) THEN
        PERFORM public.auth_concert_bump_security(OLD.id);
        IF NEW.id<>OLD.id THEN PERFORM public.auth_concert_bump_security(NEW.id); END IF;
    END IF;
    RETURN NULL;
END $$;
-- Existing credentials and journal triggers keep their original order.
CREATE TRIGGER zzzz_auth_concert_user_security_capture AFTER INSERT OR UPDATE OR DELETE ON public.users
    FOR EACH ROW EXECUTE FUNCTION public.auth_concert_user_security_capture();
CREATE FUNCTION public.auth_concert_session_security_capture() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.revoked_at IS NOT NULL OR OLD.expires_at<=transaction_timestamp() THEN RETURN NULL; END IF;
    IF TG_OP='UPDATE' THEN
        IF (NEW.user_id,NEW.family_id,NEW.token_hash,NEW.expires_at,NEW.revoked_at)
            IS NOT DISTINCT FROM (OLD.user_id,OLD.family_id,OLD.token_hash,OLD.expires_at,OLD.revoked_at) THEN RETURN NULL; END IF;
        -- Rotation inserts the successor after updating its predecessor. This
        -- deferred trigger checks the actual committed candidate, not the label.
        IF NEW.revoked_reason='ROTATED' AND NEW.rotated_at=NEW.revoked_at AND NEW.successor_jti IS NOT NULL
           AND (NEW.user_id,NEW.family_id,NEW.token_hash,NEW.expires_at) IS NOT DISTINCT FROM (OLD.user_id,OLD.family_id,OLD.token_hash,OLD.expires_at)
           AND EXISTS(SELECT 1 FROM public.refresh_tokens s WHERE s.jti=NEW.successor_jti
               AND s.jti<>OLD.jti AND s.user_id=OLD.user_id AND s.family_id=OLD.family_id
               AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp()) THEN RETURN NULL; END IF;
    END IF;
    PERFORM public.auth_concert_bump_security(OLD.user_id);
    IF TG_OP='UPDATE' AND NEW.user_id<>OLD.user_id THEN PERFORM public.auth_concert_bump_security(NEW.user_id); END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER zzzz_auth_concert_session_security_capture AFTER UPDATE OR DELETE ON public.refresh_tokens
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.auth_concert_session_security_capture();
CREATE FUNCTION public.auth_concert_security_truncate_capture() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE subject uuid;
BEGIN
    IF TG_TABLE_NAME='users' THEN
        FOR subject IN SELECT id FROM public.users ORDER BY id LOOP PERFORM public.auth_concert_bump_security(subject); END LOOP;
    ELSE
        FOR subject IN SELECT DISTINCT user_id FROM public.refresh_tokens WHERE revoked_at IS NULL AND expires_at>clock_timestamp() ORDER BY user_id
        LOOP PERFORM public.auth_concert_bump_security(subject); END LOOP;
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER zzzz_auth_concert_users_security_truncate BEFORE TRUNCATE ON public.users
    FOR EACH STATEMENT EXECUTE FUNCTION public.auth_concert_security_truncate_capture();
CREATE TRIGGER zzzz_auth_concert_sessions_security_truncate BEFORE TRUNCATE ON public.refresh_tokens
    FOR EACH STATEMENT EXECUTE FUNCTION public.auth_concert_security_truncate_capture();
