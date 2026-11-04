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

