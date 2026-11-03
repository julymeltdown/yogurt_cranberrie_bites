-- BE15 internal foundation, pinned C04 REVIEW_CANDIDATE. No raw request/result payloads.
CREATE TABLE public.concert_command_operations (operation_id text PRIMARY KEY CHECK(char_length(operation_id) BETWEEN 1 AND 100));
INSERT INTO public.concert_command_operations(operation_id) VALUES
('createConcertParty'),
('updateConcertParty'),
('cancelConcertParty'),
('transferConcertParty'),
('applyConcertParty'),
('decideConcertApplication'),
('cancelConcertApplication'),
('removeConcertMember'),
('setConcertRally'),
('createConcertBeacon'),
('updateConcertBeacon'),
('revokeConcertBeacon'),
('createConcertGiveaway'),
('updateConcertGiveaway'),
('setConcertPickupMeeting'),
('reserveConcertGiveaway'),
('cancelConcertReservation'),
('issueConcertPickupCode'),
('claimConcertReservation'),
('setConcertIntro'),
('publishConcertIntro'),
('closeConcertIntro'),
('startConcertAgeVerification'),
('completeConcertAgeVerification'),
('setConcertHandle'),
('removeConcertHandle'),
('createConcertSocialRequest'),
('decideConcertSocialRequest'),
('cancelConcertSocialRequest'),
('revokeConcertSocialRequest'),
('createConcertPost'),
('editConcertPost'),
('deleteConcertPost'),
('createConcertComment'),
('editConcertComment'),
('deleteConcertComment'),
('likeConcertPost'),
('unlikeConcertPost'),
('initConcertUpload'),
('putConcertUploadBytes'),
('finalizeConcertUpload'),
('markConcertNotification'),
('markAllConcertNotifications'),
('blockConcertUser'),
('unblockConcertUser'),
('createConcertReport'),
('placeConcertPixel'),
('setConcertSettings'),
('setConcertEventPhase'),
('setConcertPlace'),
('createConcertNotice'),
('updateConcertNotice'),
('decideConcertReport'),
('moderateConcertUser'),
('moderateConcertContent'),
('controlConcertMural'),
('lockConcertMuralRegion'),
('restoreConcertMural'),
('closeConcertEvent');
-- Fixed lifetime admission budget: tombstones are never automatically deleted.
-- Reviewed event/account cleanup must design budget reclamation; exhaustion fails closed.
CREATE TABLE public.concert_command_budget (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
 receipts_used bigint NOT NULL DEFAULT 0 CHECK(receipts_used BETWEEN 0 AND 1000000)
);
INSERT INTO public.concert_command_budget DEFAULT VALUES;
CREATE TABLE public.concert_command_receipts (
 receipt_id uuid PRIMARY KEY,
 actor_id uuid NOT NULL,
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE RESTRICT,
 operation_id text NOT NULL REFERENCES public.concert_command_operations(operation_id),
 idempotency_key uuid NOT NULL,
 request_hash text NOT NULL CHECK(request_hash ~ '^sha256:[0-9a-f]{64}$'),
 target_id uuid NOT NULL,
 state text NOT NULL CHECK(state IN ('PENDING','SUCCEEDED','FAILED','RETIRED')),
 resource_id uuid,
 resource_revision bigint CHECK(resource_revision BETWEEN 1 AND 9007199254740991),
 result_status smallint CHECK(result_status BETWEEN 200 AND 299 AND result_status <> 202),
 failure_reason text CHECK(failure_reason IN ('INVALID_TRANSITION','VERSION_CONFLICT','DEPENDENCY_UNAVAILABLE','INTERNAL_ERROR')),
 created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
 expires_at timestamptz NOT NULL DEFAULT statement_timestamp()+interval '86400 seconds',
 UNIQUE(actor_id,event_id,operation_id,idempotency_key),
 UNIQUE(receipt_id,event_id,target_id),
 CHECK(resource_id IS NULL OR resource_id=target_id),
 CHECK(expires_at > created_at AND expires_at <= created_at+interval '86400 seconds'),
 CHECK((resource_id IS NULL)=(resource_revision IS NULL)),
 CHECK((state='SUCCEEDED' AND resource_id IS NOT NULL AND result_status IS NOT NULL AND failure_reason IS NULL)
    OR (state='FAILED' AND resource_id IS NULL AND result_status IS NULL AND failure_reason IS NOT NULL)
    OR (state IN ('PENDING','RETIRED') AND resource_id IS NULL AND result_status IS NULL AND failure_reason IS NULL))
);
CREATE INDEX concert_command_actor_budget ON public.concert_command_receipts(actor_id,event_id);
CREATE INDEX concert_command_expiry ON public.concert_command_receipts(expires_at) WHERE state <> 'RETIRED';
CREATE TABLE public.concert_outbox (
 outbox_id uuid PRIMARY KEY,
 receipt_id uuid NOT NULL UNIQUE,
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE RESTRICT,
 resource_id uuid NOT NULL,
 resource_revision bigint NOT NULL CHECK(resource_revision BETWEEN 1 AND 9007199254740991),
 kind text NOT NULL DEFAULT 'RESOURCE_INVALIDATED' CHECK(kind='RESOURCE_INVALIDATED'),
 state text NOT NULL DEFAULT 'READY' CHECK(state IN ('READY','LEASED','DELIVERED','DEAD')),
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 8),
 lease_token uuid,
 lease_until timestamptz,
 available_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 terminal_at timestamptz,
 last_error text CHECK(last_error IN ('RETRYABLE','REJECTED','ATTEMPTS_EXHAUSTED')),
 FOREIGN KEY(receipt_id,event_id,resource_id) REFERENCES public.concert_command_receipts(receipt_id,event_id,target_id) ON DELETE RESTRICT,
 CHECK((state='LEASED' AND lease_token IS NOT NULL AND lease_until IS NOT NULL) OR (state<>'LEASED' AND lease_token IS NULL AND lease_until IS NULL)),
 CHECK((state IN ('DELIVERED','DEAD'))=(terminal_at IS NOT NULL))
);
CREATE INDEX concert_outbox_claim ON public.concert_outbox(available_at,created_at,outbox_id) WHERE state IN ('READY','LEASED');
CREATE INDEX concert_outbox_prune ON public.concert_outbox(terminal_at,outbox_id) WHERE state IN ('DELIVERED','DEAD');
CREATE FUNCTION public.concert_receipt_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF (NEW.receipt_id,NEW.actor_id,NEW.event_id,NEW.operation_id,NEW.idempotency_key,NEW.request_hash,NEW.target_id,NEW.created_at,NEW.expires_at)
 IS DISTINCT FROM (OLD.receipt_id,OLD.actor_id,OLD.event_id,OLD.operation_id,OLD.idempotency_key,OLD.request_hash,OLD.target_id,OLD.created_at,OLD.expires_at)
 THEN RAISE EXCEPTION 'immutable command correlation' USING ERRCODE='23514'; END IF;
 IF OLD.state <> 'PENDING' AND NEW.state <> 'RETIRED' AND NEW IS DISTINCT FROM OLD
 THEN RAISE EXCEPTION 'terminal command receipt' USING ERRCODE='23514'; END IF;
 IF OLD.state='RETIRED' AND NEW IS DISTINCT FROM OLD
 THEN RAISE EXCEPTION 'retired command receipt' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_receipt_guard BEFORE UPDATE ON public.concert_command_receipts FOR EACH ROW EXECUTE FUNCTION public.concert_receipt_guard();
