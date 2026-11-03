-- MCR-owned event-local policy. Auth/Friend remain authorities for their own data.
-- Canonical Auth references are not a second identity database.
CREATE TABLE public.concert_relationship_pairs (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    low_user uuid NOT NULL,
    high_user uuid NOT NULL,
    low_blocks_high boolean NOT NULL DEFAULT false,
    high_blocks_low boolean NOT NULL DEFAULT false,
    revision bigint NOT NULL DEFAULT 1 CHECK (revision BETWEEN 1 AND 9007199254740991),
    created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    PRIMARY KEY(event_id,low_user,high_user),
    CHECK (low_user <= high_user),
    CHECK (low_user <> high_user OR NOT (low_blocks_high OR high_blocks_low)),
    CHECK (updated_at >= created_at)
);
CREATE TRIGGER concert_relationship_pairs_revision_guard BEFORE INSERT OR UPDATE ON public.concert_relationship_pairs
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id','low_user','high_user');
-- Forward recovery: migration is transactional; collision rolls back all of 0006.
-- Preserve rows/revisions during upgrades. Never truncate to "repair" a policy failure.

CREATE TABLE public.concert_social_handles (
 event_id uuid NOT NULL,
 user_id uuid NOT NULL,
 platform text NOT NULL CHECK(platform IN ('INSTAGRAM','X')),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 key_id text CHECK(key_id ~ '^[A-Za-z0-9_-]{1,32}$'),
 nonce bytea CHECK(octet_length(nonce)=12),
 ciphertext bytea CHECK(octet_length(ciphertext) BETWEEN 17 AND 46),
 created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
 updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
 PRIMARY KEY(event_id,user_id,platform),
 FOREIGN KEY(event_id,user_id) REFERENCES public.concert_participants(event_id,user_id) ON DELETE CASCADE,
 CHECK((key_id IS NULL)=(nonce IS NULL) AND (nonce IS NULL)=(ciphertext IS NULL)),
 CHECK(updated_at >= created_at)
);
CREATE TRIGGER concert_social_handles_revision_guard BEFORE INSERT OR UPDATE ON public.concert_social_handles
 FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id','user_id','platform');
CREATE TABLE public.concert_social_requests (
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
 request_id uuid NOT NULL,
 requester_id uuid NOT NULL,
 recipient_id uuid NOT NULL,
 intro_id uuid NOT NULL,
 intro_revision bigint NOT NULL CHECK(intro_revision BETWEEN 1 AND 9007199254740991),
 authority_revision bigint NOT NULL CHECK(authority_revision BETWEEN 1 AND 9007199254740991),
 relationship_revision bigint NOT NULL CHECK(relationship_revision BETWEEN 1 AND 9007199254740991),
 state text NOT NULL CHECK(state IN ('PENDING','APPROVED','REJECTED','CANCELLED','REVOKED','EXPIRED')),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 expires_at timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
 updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
 PRIMARY KEY(event_id,request_id),
 UNIQUE(event_id,request_id,requester_id,recipient_id),
 CHECK(requester_id<>recipient_id),
 CHECK(expires_at>created_at),
 CHECK(updated_at>=created_at)
);
CREATE INDEX concert_social_requests_actor ON public.concert_social_requests(event_id,requester_id,recipient_id,state);
CREATE TRIGGER concert_social_requests_revision_guard BEFORE INSERT OR UPDATE ON public.concert_social_requests
 FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id','request_id','requester_id','recipient_id','intro_id','intro_revision','relationship_revision','authority_revision','expires_at');
CREATE TABLE public.concert_social_bindings (
 event_id uuid NOT NULL,
 request_id uuid NOT NULL,
 platform text NOT NULL CHECK(platform IN ('INSTAGRAM','X')),
 requester_version bigint NOT NULL CHECK(requester_version BETWEEN 1 AND 9007199254740991),
 recipient_version bigint NOT NULL CHECK(recipient_version BETWEEN 1 AND 9007199254740991),
 requester_consents boolean NOT NULL CHECK(requester_consents),
 approved boolean NOT NULL DEFAULT false,
 PRIMARY KEY(event_id,request_id,platform),
 FOREIGN KEY(event_id,request_id) REFERENCES public.concert_social_requests(event_id,request_id) ON DELETE CASCADE
);
CREATE TABLE public.concert_social_grants (
 event_id uuid NOT NULL,
 request_id uuid NOT NULL,
 owner_id uuid NOT NULL,
 viewer_id uuid NOT NULL,
 platform text NOT NULL CHECK(platform IN ('INSTAGRAM','X')),
 handle_version bigint NOT NULL CHECK(handle_version BETWEEN 1 AND 9007199254740991),
 peer_handle_version bigint NOT NULL CHECK(peer_handle_version BETWEEN 1 AND 9007199254740991),
 state text NOT NULL CHECK(state IN ('ACTIVE','REVOKED','EXPIRED')),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
 updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
 expires_at timestamptz NOT NULL,
 PRIMARY KEY(event_id,request_id,owner_id,viewer_id,platform),
 FOREIGN KEY(event_id,request_id,platform) REFERENCES public.concert_social_bindings(event_id,request_id,platform) ON DELETE CASCADE,
 CHECK(owner_id<>viewer_id),
 CHECK(expires_at>created_at),
 CHECK(updated_at>=created_at)
);
CREATE TRIGGER concert_social_grants_revision_guard BEFORE INSERT OR UPDATE ON public.concert_social_grants
 FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id','request_id','owner_id','viewer_id','platform','handle_version','peer_handle_version','expires_at');
-- Grant/request relationships and current reciprocal authority are checked in the
-- adapter transaction. No SQL SELECT permission on these tables is a reveal grant.

CREATE FUNCTION public.concert_social_state_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='INSERT' THEN
  IF (TG_TABLE_NAME='concert_social_requests' AND NEW.state<>'PENDING')
     OR (TG_TABLE_NAME='concert_social_grants' AND NEW.state<>'ACTIVE') THEN
   RAISE EXCEPTION 'invalid initial social state' USING ERRCODE='23514';
  END IF;
 ELSE
  IF TG_TABLE_NAME='concert_social_requests' THEN
   IF NOT ((OLD.state='PENDING' AND NEW.state IN ('APPROVED','REJECTED','CANCELLED','REVOKED','EXPIRED'))
       OR (OLD.state='APPROVED' AND NEW.state IN ('REVOKED','EXPIRED'))) THEN
    RAISE EXCEPTION 'invalid social request transition' USING ERRCODE='23514';
   END IF;
  ELSIF NOT (OLD.state='ACTIVE' AND NEW.state IN ('REVOKED','EXPIRED')) THEN
   RAISE EXCEPTION 'invalid social grant transition' USING ERRCODE='23514';
  END IF;
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER concert_social_requests_state_guard BEFORE INSERT OR UPDATE ON public.concert_social_requests
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_state_guard();
CREATE TRIGGER concert_social_grants_state_guard BEFORE INSERT OR UPDATE ON public.concert_social_grants
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_state_guard();
CREATE FUNCTION public.concert_social_grant_binding_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE r public.concert_social_requests; b public.concert_social_bindings;
BEGIN
 SELECT * INTO r FROM public.concert_social_requests WHERE event_id=NEW.event_id AND request_id=NEW.request_id;
 SELECT * INTO b FROM public.concert_social_bindings WHERE event_id=NEW.event_id AND request_id=NEW.request_id AND platform=NEW.platform;
 IF r.request_id IS NULL OR b.request_id IS NULL OR NOT b.requester_consents OR NOT b.approved
    OR NEW.expires_at<>r.expires_at
    OR NOT ((NEW.owner_id=r.requester_id AND NEW.viewer_id=r.recipient_id AND NEW.handle_version=b.requester_version AND NEW.peer_handle_version=b.recipient_version)
         OR (NEW.owner_id=r.recipient_id AND NEW.viewer_id=r.requester_id AND NEW.handle_version=b.recipient_version AND NEW.peer_handle_version=b.requester_version)) THEN
  RAISE EXCEPTION 'invalid social grant binding' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER concert_social_grants_binding_guard BEFORE INSERT ON public.concert_social_grants
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_grant_binding_guard();

-- Deferred guards close the gap between apply() checks and the BE15 repository's
-- receipt/outbox commit. Owner proof deadlines cannot be extended by later reads.
CREATE FUNCTION public.concert_relationship_deadline_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE deadline timestamptz;
BEGIN
 deadline := nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz;
 IF deadline IS NULL OR clock_timestamp()>=deadline THEN
  RAISE EXCEPTION 'relationship authority expired' USING ERRCODE='23514';
 END IF;
 IF TG_TABLE_NAME IN ('concert_social_requests','concert_social_grants') THEN
  IF NEW.state IN ('PENDING','APPROVED','ACTIVE') AND NEW.expires_at<=clock_timestamp() THEN
   RAISE EXCEPTION 'social authorization expired' USING ERRCODE='23514';
  END IF;
 END IF;
 RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER concert_relationship_pairs_deadline AFTER INSERT OR UPDATE ON public.concert_relationship_pairs
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_relationship_deadline_guard();
CREATE CONSTRAINT TRIGGER concert_social_handles_deadline AFTER INSERT OR UPDATE ON public.concert_social_handles
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_relationship_deadline_guard();
CREATE CONSTRAINT TRIGGER concert_social_requests_deadline AFTER INSERT OR UPDATE ON public.concert_social_requests
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_relationship_deadline_guard();
CREATE CONSTRAINT TRIGGER concert_social_grants_deadline AFTER INSERT OR UPDATE ON public.concert_social_grants
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_relationship_deadline_guard();
