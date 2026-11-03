-- BE10/BE11 authoritative, durable world admission state. Credentials enter this schema only as
-- SHA-256 digests. Event restrictions and canonical identity remain owner dependencies.
CREATE TABLE public.concert_world_rooms (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    room_id uuid NOT NULL,
    label text NOT NULL CONSTRAINT concert_world_room_label CHECK (
        char_length(label) BETWEEN 1 AND 48
        AND label ~ '[^[:space:]]'
        AND label !~ '[[:cntrl:]]'
    ),
    capacity integer NOT NULL CONSTRAINT concert_world_room_capacity CHECK (capacity BETWEEN 1 AND 50),
    availability text NOT NULL DEFAULT 'AVAILABLE' CONSTRAINT concert_world_room_availability CHECK (
        availability IN ('AVAILABLE','UNAVAILABLE')
    ),
    map_id text NOT NULL CONSTRAINT concert_world_room_map_id CHECK (map_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    map_revision bigint NOT NULL CONSTRAINT concert_world_room_map_revision CHECK (map_revision BETWEEN 1 AND 9007199254740991),
    avatar_manifest_id text NOT NULL CONSTRAINT concert_world_room_avatar_id CHECK (avatar_manifest_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    avatar_manifest_version bigint NOT NULL CONSTRAINT concert_world_room_avatar_version CHECK (avatar_manifest_version BETWEEN 1 AND 9007199254740991),
    is_default boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(event_id,room_id)
);
CREATE UNIQUE INDEX concert_world_one_default_room
    ON public.concert_world_rooms(event_id) WHERE is_default;
CREATE INDEX concert_world_rooms_catalog ON public.concert_world_rooms(event_id,room_id);

CREATE FUNCTION public.concert_guard_world_room_catalog() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE event_key uuid; room_count bigint;
BEGIN
    event_key := CASE WHEN TG_OP='DELETE' THEN OLD.event_id ELSE NEW.event_id END;
    SELECT count(*) INTO room_count FROM public.concert_world_rooms WHERE event_id=event_key;
    IF room_count > 16 THEN
        RAISE EXCEPTION 'world room catalog exceeds 16 rooms' USING ERRCODE='23514';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END;
$$;
CREATE TRIGGER concert_world_room_catalog_guard
    AFTER INSERT OR UPDATE OF event_id ON public.concert_world_rooms
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_world_room_catalog();

CREATE TABLE public.concert_world_owner_leases (
    event_id uuid NOT NULL,
    room_id uuid NOT NULL,
    owner_process_id text NOT NULL CONSTRAINT concert_world_owner_process CHECK (
        char_length(owner_process_id) BETWEEN 1 AND 128
        AND owner_process_id !~ '[[:cntrl:]]'
    ),
    owner_epoch uuid NOT NULL,
    leased_at timestamptz NOT NULL,
    lease_until timestamptz NOT NULL,
    CONSTRAINT concert_world_owner_room FOREIGN KEY(event_id,room_id)
        REFERENCES public.concert_world_rooms(event_id,room_id) ON DELETE CASCADE,
    CONSTRAINT concert_world_owner_key PRIMARY KEY(event_id,room_id),
    CONSTRAINT concert_world_owner_lease_time CHECK (lease_until > leased_at)
);
CREATE INDEX concert_world_owner_expiry ON public.concert_world_owner_leases(lease_until);

CREATE TABLE public.concert_world_tickets (
    ticket_digest bytea PRIMARY KEY CONSTRAINT concert_world_ticket_digest_shape CHECK (octet_length(ticket_digest)=32),
    user_id uuid NOT NULL,
    session_id uuid NOT NULL,
    event_id uuid NOT NULL,
    room_id uuid NOT NULL,
    map_id text NOT NULL CONSTRAINT concert_world_ticket_map_id CHECK (map_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    map_revision bigint NOT NULL CONSTRAINT concert_world_ticket_map_revision CHECK (map_revision BETWEEN 1 AND 9007199254740991),
    avatar_manifest_id text NOT NULL CONSTRAINT concert_world_ticket_avatar_id CHECK (avatar_manifest_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    avatar_manifest_version bigint NOT NULL CONSTRAINT concert_world_ticket_avatar_version CHECK (avatar_manifest_version BETWEEN 1 AND 9007199254740991),
    issued_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    access_expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    consumed_connection_id uuid,
    FOREIGN KEY(event_id,room_id) REFERENCES public.concert_world_rooms(event_id,room_id) ON DELETE CASCADE,
    CONSTRAINT concert_world_ticket_ttl CHECK (
        expires_at > issued_at
        AND expires_at <= access_expires_at
        AND expires_at <= issued_at + interval '30 seconds'
    ),
    CONSTRAINT concert_world_ticket_consumption CHECK (
        (consumed_at IS NULL AND consumed_connection_id IS NULL)
        OR (consumed_at IS NOT NULL AND consumed_connection_id IS NOT NULL AND consumed_at >= issued_at)
    )
);
CREATE INDEX concert_world_ticket_expiry ON public.concert_world_tickets(expires_at);
CREATE INDEX concert_world_ticket_consumed ON public.concert_world_tickets(consumed_at) WHERE consumed_at IS NOT NULL;

CREATE TABLE public.concert_world_memberships (
    event_id uuid NOT NULL,
    user_id uuid NOT NULL,
    room_id uuid NOT NULL,
    session_id uuid NOT NULL,
    connection_id uuid NOT NULL,
    owner_process_id text NOT NULL,
    owner_epoch uuid NOT NULL,
    map_id text NOT NULL CONSTRAINT concert_world_membership_map_id CHECK (map_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    map_revision bigint NOT NULL CONSTRAINT concert_world_membership_map_revision CHECK (map_revision BETWEEN 1 AND 9007199254740991),
    avatar_manifest_id text NOT NULL CONSTRAINT concert_world_membership_avatar_id CHECK (avatar_manifest_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    avatar_manifest_version bigint NOT NULL CONSTRAINT concert_world_membership_avatar_version CHECK (avatar_manifest_version BETWEEN 1 AND 9007199254740991),
    admitted_at timestamptz NOT NULL,
    lease_until timestamptz NOT NULL,
    access_expires_at timestamptz NOT NULL,
    disconnected boolean NOT NULL DEFAULT false,
    PRIMARY KEY(event_id,user_id),
    FOREIGN KEY(event_id,room_id) REFERENCES public.concert_world_rooms(event_id,room_id),
    CONSTRAINT concert_world_membership_lease CHECK (
        lease_until > admitted_at AND lease_until <= access_expires_at
    )
);
CREATE INDEX concert_world_membership_room ON public.concert_world_memberships(event_id,room_id,lease_until);
CREATE INDEX concert_world_membership_expiry ON public.concert_world_memberships(lease_until);
CREATE UNIQUE INDEX concert_world_membership_connection
    ON public.concert_world_memberships(connection_id);

-- Direct SQL cannot create a second active row for a user/event because that is the primary key.
-- Ghost rows lose admission at lease_until; room capacity queries count only unexpired rows.
