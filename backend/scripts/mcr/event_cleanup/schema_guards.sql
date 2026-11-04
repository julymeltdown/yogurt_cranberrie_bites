-- Database-constraint qualification ONLY. The marked owned fixture uses synthetic
-- job/receipt metadata, not production operator grants or actual owner effects.
SELECT owner_token = :'fixture_owner' AS fixture_owned FROM event_cleanup_fixture_marker \gset
\if :fixture_owned
\else
\quit 9
\endif
CREATE FUNCTION pg_temp.expect_cleanup_rejection(label text, statement text, expected text DEFAULT '23514') RETURNS text LANGUAGE plpgsql AS $$
BEGIN
 BEGIN
  EXECUTE statement;
 EXCEPTION WHEN OTHERS THEN
  IF SQLSTATE=expected THEN RETURN 'SQL_GUARD_PASS:'||label; END IF;
  RAISE;
 END;
 RAISE EXCEPTION 'expected rejection: %',label;
END; $$;
BEGIN;
SELECT set_config('mcr.lifecycle_authority_until',(clock_timestamp()+interval '30 seconds')::text,true);
INSERT INTO concert_events(event_id,slug,title,phase) VALUES('46464646-4646-4646-8646-464646464601','event-cleanup-sql-guard','Synthetic metadata guard fixture','READ_ONLY');
INSERT INTO concert_closure_jobs(job_id,event_id,actor_id,start_key,original_event_revision,retention_revision,closed_at,cleanup_after,progress)
VALUES('46464646-4646-4646-8646-464646464603','46464646-4646-4646-8646-464646464601','46464646-4646-4646-8646-464646464602','46464646-4646-4646-8646-464646464604',1,1,clock_timestamp(),clock_timestamp()+interval '1 hour',
'{"job_id":"46464646-4646-4646-8646-464646464603","event_id":"46464646-4646-4646-8646-464646464601","revision":1,"state":"PENDING","steps":[{"name":"READ_ONLY","state":"SUCCEEDED"},{"name":"FREEZE_MURAL","state":"PENDING"},{"name":"SNAPSHOT","state":"PENDING"},{"name":"EXPIRE_GRANTS","state":"PENDING"},{"name":"EXPIRE_BEACONS","state":"PENDING"},{"name":"EXPORT","state":"PENDING"},{"name":"CLEANUP","state":"PENDING"}]}');
INSERT INTO concert_closure_effects(event_id,job_id,step,owner_receipt,owner_revision) VALUES('46464646-4646-4646-8646-464646464601','46464646-4646-4646-8646-464646464603','READ_ONLY','46464646-4646-4646-8646-464646464605',1);
INSERT INTO concert_event_cleanup_progress(job_id,event_id,actor_id,step,owner,batch_limit) VALUES('46464646-4646-4646-8646-464646464603','46464646-4646-4646-8646-464646464601','46464646-4646-4646-8646-464646464602','FREEZE_MURAL','MURAL',2);
SELECT pg_temp.expect_cleanup_rejection('direct_completion',$q$UPDATE concert_event_cleanup_progress SET state='COMPLETE'$q$);
SELECT pg_temp.expect_cleanup_rejection('direct_cursor',$q$UPDATE concert_event_cleanup_progress SET ordinal=1,cursor=decode('01','hex'),processed=1$q$);
SELECT pg_temp.expect_cleanup_rejection('scope_rebinding',$q$UPDATE concert_event_cleanup_progress SET actor_id='46464646-4646-4646-8646-464646464699'$q$);
SELECT pg_temp.expect_cleanup_rejection('batch_limit',$q$UPDATE concert_event_cleanup_progress SET batch_limit=3$q$);
INSERT INTO concert_event_cleanup_batches(operation_id,job_id,event_id,actor_id,step,owner,ordinal,binding,cursor_in,cursor_out,outcome,receipt_id,owner_revision,processed,cumulative)
VALUES('46464646-4646-4646-8646-464646464610','46464646-4646-4646-8646-464646464603','46464646-4646-4646-8646-464646464601','46464646-4646-4646-8646-464646464602','FREEZE_MURAL','MURAL',0,decode(repeat('01',32),'hex'),NULL,decode('0000000000000002','hex'),'PAGE','46464646-4646-4646-8646-464646464611',1,2,2);
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM concert_event_cleanup_progress WHERE ordinal=1 AND encode(cursor,'hex')='0000000000000002' AND processed=2 AND state='PENDING') THEN RAISE EXCEPTION 'cursor did not advance atomically'; END IF;
END; $$;
SELECT 'SQL_GUARD_PASS:atomic_cursor';
SELECT pg_temp.expect_cleanup_rejection('stale_page',$q$INSERT INTO concert_event_cleanup_batches SELECT '46464646-4646-4646-8646-464646464612',job_id,event_id,actor_id,step,owner,ordinal,binding,cursor_in,cursor_out,outcome,receipt_id,owner_revision,processed,cumulative,created_at FROM concert_event_cleanup_batches$q$);
SELECT pg_temp.expect_cleanup_rejection('immutable_batch',$q$UPDATE concert_event_cleanup_batches SET processed=1$q$);
SELECT pg_temp.expect_cleanup_rejection('delete_batch',$q$DELETE FROM concert_event_cleanup_batches$q$);
SET CONSTRAINTS ALL IMMEDIATE;
SELECT pg_temp.expect_cleanup_rejection('truncate_batches',$q$TRUNCATE concert_event_cleanup_batches CASCADE$q$);
SELECT pg_temp.expect_cleanup_rejection('truncate_heads',$q$TRUNCATE concert_event_cleanup_progress CASCADE$q$);
SAVEPOINT page_rollback;
INSERT INTO concert_event_cleanup_batches(operation_id,job_id,event_id,actor_id,step,owner,ordinal,binding,cursor_in,cursor_out,outcome,receipt_id,owner_revision,processed,cumulative)
VALUES('46464646-4646-4646-8646-464646464620','46464646-4646-4646-8646-464646464603','46464646-4646-4646-8646-464646464601','46464646-4646-4646-8646-464646464602','FREEZE_MURAL','MURAL',1,decode(repeat('02',32),'hex'),decode('0000000000000002','hex'),NULL,'CONVERGED','46464646-4646-4646-8646-464646464621',1,1,3);
ROLLBACK TO SAVEPOINT page_rollback;
RELEASE SAVEPOINT page_rollback;
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM concert_event_cleanup_progress WHERE ordinal=1 AND processed=2 AND state='PENDING') OR (SELECT count(*) FROM concert_event_cleanup_batches)<>1 THEN RAISE EXCEPTION 'page rollback lost cursor or evidence'; END IF;
END; $$;
SELECT 'SQL_GUARD_PASS:rollback_cursor';
COMMIT;
