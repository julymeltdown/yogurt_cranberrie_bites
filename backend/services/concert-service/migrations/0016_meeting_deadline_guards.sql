-- Forward-only upgrade of published 0008; never rewrite the migration ledger.
-- Old reports cannot be carried into current guidance without reconfirmation.
-- A constant column default preserves old report bytes/revisions without firing
-- the existing UPDATE revision guard. Remove the default for all future writes.
-- Epoch marks legacy reports as unqualified until a new explicit self-report.
ALTER TABLE public.concert_meeting_rally_states ADD COLUMN meeting_deadline timestamptz NOT NULL DEFAULT '1970-01-01T00:00:00Z'::timestamptz;
ALTER TABLE public.concert_meeting_rally_states ALTER COLUMN meeting_deadline DROP DEFAULT;

-- A trusted BE37 adapter pins its earliest owner-proof deadline in this local
-- transaction setting. It is never authentication by itself. A fenced custom
-- authority can use a different qualified protocol; raw SQL does not grant access.
CREATE FUNCTION public.concert_meeting_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE proof_until timestamptz; cutoff timestamptz;
BEGIN
    proof_until:=nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz;
    IF proof_until IS NOT NULL AND clock_timestamp()>=proof_until THEN
        RAISE EXCEPTION 'meeting authority expired before commit' USING ERRCODE='23514';
    END IF;
    IF TG_TABLE_NAME='concert_meeting_beacons' AND NEW.state='REVOKED' THEN RETURN NULL; END IF;
    SELECT event_cutoff INTO cutoff FROM public.concert_meeting_settings WHERE event_id=NEW.event_id;
    IF cutoff IS NULL OR clock_timestamp()>=cutoff THEN
        RAISE EXCEPTION 'meeting cutoff expired before commit' USING ERRCODE='23514';
    END IF;
    IF TG_TABLE_NAME='concert_meeting_beacons' THEN
        IF clock_timestamp()>=NEW.expires_at THEN RAISE EXCEPTION 'beacon expired before commit' USING ERRCODE='23514'; END IF;
    ELSE
        IF clock_timestamp()>=NEW.meeting_deadline THEN RAISE EXCEPTION 'rally expired before commit' USING ERRCODE='23514'; END IF;
    END IF;
    RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER concert_meeting_beacon_commit AFTER INSERT OR UPDATE ON public.concert_meeting_beacons
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_meeting_commit_guard();
CREATE CONSTRAINT TRIGGER concert_meeting_rally_commit AFTER INSERT OR UPDATE ON public.concert_meeting_rally_states
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_meeting_commit_guard();
