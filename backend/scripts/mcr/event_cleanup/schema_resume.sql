-- Run only after the same marked server was actually stopped and restarted.
SELECT owner_token = :'fixture_owner' AS fixture_owned FROM event_cleanup_fixture_marker \gset
\if :fixture_owned
\else
\quit 9
\endif
BEGIN;
SELECT set_config('mcr.lifecycle_authority_until',(clock_timestamp()+interval '30 seconds')::text,true);
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM concert_event_cleanup_progress WHERE ordinal=1 AND encode(cursor,'hex')='0000000000000002' AND processed=2 AND state='PENDING') THEN RAISE EXCEPTION 'restart lost committed cursor'; END IF;
END; $$;
INSERT INTO concert_event_cleanup_batches(operation_id,job_id,event_id,actor_id,step,owner,ordinal,binding,cursor_in,cursor_out,outcome,receipt_id,owner_revision,processed,cumulative)
VALUES('46464646-4646-4646-8646-464646464620','46464646-4646-4646-8646-464646464603','46464646-4646-4646-8646-464646464601','46464646-4646-4646-8646-464646464602','FREEZE_MURAL','MURAL',1,decode(repeat('02',32),'hex'),decode('0000000000000002','hex'),NULL,'CONVERGED','46464646-4646-4646-8646-464646464621',1,1,3);
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM concert_event_cleanup_progress WHERE ordinal=1 AND processed=3 AND state='COMPLETE') OR (SELECT count(*) FROM concert_event_cleanup_batches)<>2 THEN RAISE EXCEPTION 'resumed batch state invalid'; END IF;
 IF EXISTS(SELECT 1 FROM concert_closure_effects WHERE step='FREEZE_MURAL') OR EXISTS(SELECT 1 FROM concert_closure_audit WHERE outcome='SUCCEEDED') THEN RAISE EXCEPTION 'batch progress fabricated parent success'; END IF;
END; $$;
COMMIT;
SELECT 'SQL_GUARD_PASS:restart_resume';
SELECT 'SQL_GUARD_PASS:parent_still_pending';
