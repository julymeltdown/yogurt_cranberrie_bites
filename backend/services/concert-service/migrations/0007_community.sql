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
