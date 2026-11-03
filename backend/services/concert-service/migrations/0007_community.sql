-- Newly authored MCR community persistence. No source identity data or production seeds.
-- Aggregate root row is the serialization point. Private children never enter public projections.
CREATE TABLE public.concert_community_parties (
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
 party_id uuid NOT NULL, owner_id uuid NOT NULL, revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 state text NOT NULL CHECK(state IN ('OPEN','CLOSED','CANCELLED','COMPLETED')),
 member_count integer NOT NULL CHECK(member_count BETWEEN 1 AND 50),
 capacity integer NOT NULL CHECK(capacity BETWEEN 2 AND 50 AND capacity>=member_count),
 meeting_revision bigint NOT NULL CHECK(meeting_revision BETWEEN 1 AND 9007199254740991),
 audience_revision bigint NOT NULL CHECK(audience_revision BETWEEN 1 AND 9007199254740991),
 -- Closed internal aggregate encoding only; no provider data, handles, physical descriptions or tokens.
 aggregate jsonb NOT NULL CHECK(jsonb_typeof(aggregate)='object'),
 PRIMARY KEY(event_id,party_id),
 CHECK((aggregate->>'id')::uuid=party_id AND (aggregate->>'event')::uuid=event_id AND (aggregate->>'owner')::uuid=owner_id),
 CHECK((aggregate->>'revision')::bigint=revision AND aggregate->>'state'=state),
 CHECK((aggregate->'fields'->>'capacity')::integer=capacity),
 CHECK((aggregate->>'meeting_revision')::bigint=meeting_revision AND (aggregate->>'audience_revision')::bigint=audience_revision)
);
CREATE TABLE public.concert_community_applications (
 event_id uuid NOT NULL, party_id uuid NOT NULL, application_id uuid NOT NULL, user_id uuid NOT NULL,
 state text NOT NULL CHECK(state IN ('PENDING','APPROVED','REJECTED','CANCELLED')),
 revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 PRIMARY KEY(event_id,application_id),
 FOREIGN KEY(event_id,party_id) REFERENCES public.concert_community_parties(event_id,party_id)
);
CREATE UNIQUE INDEX concert_community_pending_application ON public.concert_community_applications(event_id,party_id,user_id) WHERE state='PENDING';
CREATE TABLE public.concert_community_members (
 event_id uuid NOT NULL, party_id uuid NOT NULL, user_id uuid NOT NULL, is_owner boolean NOT NULL,
 state text NOT NULL CHECK(state IN ('ACTIVE','REMOVED','LEFT')),
 revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 PRIMARY KEY(event_id,party_id,user_id), FOREIGN KEY(event_id,party_id) REFERENCES public.concert_community_parties(event_id,party_id),
 CHECK(NOT is_owner OR state='ACTIVE')
);
CREATE UNIQUE INDEX concert_community_one_owner ON public.concert_community_members(event_id,party_id) WHERE is_owner;

CREATE TABLE public.concert_community_giveaways (
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id), giveaway_id uuid NOT NULL,
 owner_id uuid NOT NULL, revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 state text NOT NULL CHECK(state IN ('DRAFT','OPEN','PAUSED','CLOSED','CANCELLED')),
 quantity_total integer NOT NULL CHECK(quantity_total BETWEEN 1 AND 10000),
 reserved_unclaimed integer NOT NULL CHECK(reserved_unclaimed>=0),
 quantity_claimed integer NOT NULL CHECK(quantity_claimed>=0),
 next_action_at timestamptz, aggregate jsonb NOT NULL CHECK(jsonb_typeof(aggregate)='object'),
 PRIMARY KEY(event_id,giveaway_id), CHECK(reserved_unclaimed+quantity_claimed<=quantity_total),
 CHECK((aggregate->>'id')::uuid=giveaway_id AND (aggregate->>'event')::uuid=event_id AND (aggregate->>'owner')::uuid=owner_id),
 CHECK((aggregate->>'revision')::bigint=revision AND aggregate->>'state'=state),
 CHECK((aggregate->'fields'->>'quantity_total')::integer=quantity_total),
 CHECK((aggregate->>'reserved_unclaimed')::integer=reserved_unclaimed AND (aggregate->>'quantity_claimed')::integer=quantity_claimed)
);
CREATE INDEX concert_community_expiry_scan ON public.concert_community_giveaways(next_action_at,event_id,giveaway_id) WHERE next_action_at IS NOT NULL;
CREATE TABLE public.concert_community_reservations (
 event_id uuid NOT NULL, giveaway_id uuid NOT NULL,reservation_id uuid NOT NULL,recipient_id uuid NOT NULL,
 state text NOT NULL CHECK(state IN ('HELD','READY','CLAIMED','CANCELLED','EXPIRED')),
 revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),expires_at timestamptz NOT NULL,
 PRIMARY KEY(event_id,reservation_id),FOREIGN KEY(event_id,giveaway_id) REFERENCES public.concert_community_giveaways(event_id,giveaway_id)
);
CREATE UNIQUE INDEX concert_community_active_reservation ON public.concert_community_reservations(event_id,giveaway_id,recipient_id) WHERE state IN ('HELD','READY');
CREATE TABLE public.concert_community_pickup_secrets (
 event_id uuid NOT NULL,reservation_id uuid NOT NULL,generation bigint NOT NULL CHECK(generation>0),
 key_id text NOT NULL CHECK(key_id ~ '^[A-Za-z0-9_-]{1,32}$'),
 keyed_hash bytea NOT NULL CHECK(octet_length(keyed_hash)=32),expires_at timestamptz NOT NULL,
 PRIMARY KEY(event_id,reservation_id), FOREIGN KEY(event_id,reservation_id) REFERENCES public.concert_community_reservations(event_id,reservation_id)
);
-- Shared durable guess budgets: reservation, owner, recipient and trusted keyed IP bucket.
-- Counts are retained across code rotation/restart. No raw IP or code is stored.
CREATE TABLE public.concert_community_pickup_attempts (
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
 bucket bytea NOT NULL CHECK(octet_length(bucket)=32),window_started_at timestamptz NOT NULL,
 attempts integer NOT NULL CHECK(attempts BETWEEN 1 AND 100),PRIMARY KEY(event_id,bucket)
);

-- Check the earliest verified proof/effect deadline at actual commit, after BE15 receipt/outbox SQL.
CREATE FUNCTION public.concert_community_deadline_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE deadline timestamptz;
BEGIN
 deadline:=nullif(current_setting('mcr.community_authority_until',true),'')::timestamptz;
 IF deadline IS NOT NULL AND clock_timestamp()>=deadline THEN
  RAISE EXCEPTION 'community authority expired' USING ERRCODE='23514';
 END IF;
 deadline:=nullif(current_setting('mcr.community_effect_until',true),'')::timestamptz;
 IF deadline IS NOT NULL AND clock_timestamp()>=deadline THEN
  RAISE EXCEPTION 'community effect expired' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_community_party_deadline AFTER INSERT OR UPDATE ON public.concert_community_parties
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_community_deadline_guard();
CREATE CONSTRAINT TRIGGER concert_community_gift_deadline AFTER INSERT OR UPDATE ON public.concert_community_giveaways
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_community_deadline_guard();

-- Guess admission is idempotent even if a process dies before its BE15 result receipt.
-- Correlation and keyed request MAC only; never plaintext code, IP or actor claims.
CREATE TABLE public.concert_community_pickup_admissions (
 actor_id uuid NOT NULL,event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
 idempotency_key uuid NOT NULL,reservation_id uuid NOT NULL,
 request_hash text NOT NULL CHECK(request_hash ~ '^hmac-sha256:v1:[A-Za-z0-9_-]{1,32}:[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL,expires_at timestamptz NOT NULL,
 PRIMARY KEY(actor_id,event_id,idempotency_key),
 FOREIGN KEY(event_id,reservation_id) REFERENCES public.concert_community_reservations(event_id,reservation_id),
 CHECK(expires_at=created_at+interval '86400 seconds')
);
