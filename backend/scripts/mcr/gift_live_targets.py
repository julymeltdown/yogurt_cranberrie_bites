#!/usr/bin/env python3
"""Qualify 0033 only on a new marker-owned private PG. Never builds/uses Cargo.

Synthetic SQL rows test correlation constraints, not canonical Gift authorization.
Uses migration_smoke.py's inode/marker ownership and socket-only lifecycle.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_gift_targets"

def require(value, reason):
    if not value:
        raise RuntimeError(reason)

def literal(value):
    return "'" + str(value).replace("'", "''") + "'"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pg-bin", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.is_absolute(), "absolute report required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    paths = sorted((ROOT / "backend/services/concert-service/migrations").glob("*.sql"))
    pins = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
    report = {"result":"NOT_RUN", "scope":"0033 correlation SQL only",
              "canonical_authority":"NOT_RUN", "runtime_rust":"NOT_RUN", "http":"NOT_RUN",
              "source_revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
              "migration_sha256":pins,"cases":[]}
    cluster = None
    try:
        cluster = migration.Cluster({n:migration.executable(args.pg_bin/n) for n in migration.PG_TOOLS})
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres","show server_version;")
        cluster.sql("postgres",f"create database {DATABASE} template template0;")
        for path in paths:
            cluster.sql(DATABASE,path.read_text())
        actor,event,root,other = [str(uuid.uuid4()) for _ in range(4)]
        aggregate = {"id":root,"event":event,"owner":actor,"revision":1,"state":"OPEN",
                     "fields":{"quantity_total":2},"reserved_unclaimed":0,"quantity_claimed":0}
        cluster.sql(DATABASE,f"""begin;
            select set_config('mcr.community_authority_until',(clock_timestamp()+interval '10 seconds')::text,true);
            insert into concert_events(event_id,slug,title,phase) values('{event}','gift-target-{event}','Synthetic correlation fixture','OPEN');
            insert into concert_community_giveaways(event_id,giveaway_id,owner_id,revision,state,quantity_total,reserved_unclaimed,quantity_claimed,aggregate)
                values('{event}','{root}','{actor}',1,'OPEN',2,0,0,{literal(json.dumps(aggregate))}::jsonb);
            commit;""")

        def intent(key,target,giveaway=root,**changes):
            value={"v":1,"actor_id":actor,"event_id":event,"operation_id":"reserveConcertGiveaway",
                   "idempotency_key":key,"target_id":target,"giveaway_id":giveaway}
            value.update(changes)
            return f"select set_config('mcr.gift_live_command_target',{literal(json.dumps(value))},true);"

        def receipt(key,target,receipt_id):
            return f"""insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state)
                values('{receipt_id}','{actor}','{event}','reserveConcertGiveaway','{key}',
                'hmac-sha256:v1:sql-fixture:{'0'*64}','{target}','PENDING');"""

        def check(sql):
            return f"do $$ begin if not ({sql}) then raise exception 'SQL assertion failed'; end if; end $$;"

        def run(name,sql):
            output=cluster.sql(DATABASE,sql+" select 'GIFT_TARGET_OK';")
            require("GIFT_TARGET_OK" in output.splitlines(),name+" incomplete")
            report["cases"].append({"name":name,"exit_code":0,"result":"PASS"})

        key,target,rid=[str(uuid.uuid4()) for _ in range(3)]
        run("rollback_removes_mapping_and_pending_receipt", "begin;"+intent(key,target)+receipt(key,target,rid)
            +check(f"exists(select 1 from concert_gift_command_targets where receipt_id='{rid}')")
            +"rollback;"+check(f"not exists(select 1 from concert_gift_command_targets where receipt_id='{rid}')")
            +check(f"not exists(select 1 from concert_command_receipts where receipt_id='{rid}')"))
        failed_key,failed_target,failed_id=[str(uuid.uuid4()) for _ in range(3)]
        run("failed_reserve_preserves_root_without_reservation", "begin;"+intent(failed_key,failed_target)
            +receipt(failed_key,failed_target,failed_id)+"savepoint command_effects;"
            +"rollback to savepoint command_effects;"
            +f"update concert_command_receipts set state='FAILED',failure_reason='GIVEAWAY_SOLD_OUT' where receipt_id='{failed_id}';commit;"
            +check(f"exists(select 1 from concert_gift_command_targets where receipt_id='{failed_id}' and giveaway_id='{root}')")
            +check(f"not exists(select 1 from concert_community_reservations where reservation_id='{failed_target}')"))
        pending_key,pending_target,pending_id=[str(uuid.uuid4()) for _ in range(3)]
        run("pending_reserve_commits_exact_root_before_effects", "begin;"+intent(pending_key,pending_target)
            +receipt(pending_key,pending_target,pending_id)+"commit;"
            +check("nullif(current_setting('mcr.gift_live_command_target',true),'') is null"))
        cluster.stop()
        cluster.start()
        run("lost_response_restart_keeps_failed_and_pending_exact_correlation",
            check(f"(select count(*) from concert_gift_command_targets t join concert_command_receipts r using(receipt_id) where t.actor_id='{actor}' and t.event_id='{event}' and t.giveaway_id='{root}' and r.state in ('FAILED','PENDING'))=2"))
        run("exact_duplicate_is_idempotent", "begin;"+intent(pending_key,pending_target)
            +receipt(pending_key,pending_target,pending_id).rstrip(";")+" on conflict do nothing;commit;"
            +check(f"(select count(*) from concert_gift_command_targets where receipt_id='{pending_id}' and giveaway_id='{root}')=1"))
        for field in ["actor_id","event_id","idempotency_key","target_id"]:
            k,t,r=[str(uuid.uuid4()) for _ in range(3)]
            body=intent(k,t,**{field:other}).replace("select set_config", "perform set_config")+receipt(k,t,r)
            run("altered_"+field+"_denied",f"do $$ begin begin {body} raise exception 'correlation mismatch accepted'; exception when sqlstate '22023' then null; end; end $$;"
                +check(f"not exists(select 1 from concert_command_receipts where receipt_id='{r}')"))
        other_aggregate = {**aggregate,"id":other}
        cluster.sql(DATABASE,f"""insert into concert_community_giveaways(event_id,giveaway_id,owner_id,revision,state,quantity_total,reserved_unclaimed,quantity_claimed,aggregate)
            values('{event}','{other}','{actor}',1,'OPEN',2,0,0,{literal(json.dumps(other_aggregate))}::jsonb);""")
        rebound = intent(pending_key,pending_target,giveaway=other).replace("select set_config","perform set_config")+receipt(pending_key,pending_target,str(uuid.uuid4()))
        run("root_rebind_and_mapping_mutation_denied",f"""do $$ begin
            begin {rebound} raise exception 'root rebind accepted'; exception when sqlstate '23000' then null; end;
            begin update concert_gift_command_targets set giveaway_id='{other}' where receipt_id='{pending_id}';
                raise exception 'mapping rewrite accepted'; exception when sqlstate '23000' then null; end;
            begin delete from concert_gift_command_targets where receipt_id='{pending_id}';
                raise exception 'mapping deletion accepted'; exception when sqlstate '23000' then null; end;
            end $$;"""+check(f"(select giveaway_id='{root}' from concert_gift_command_targets where receipt_id='{pending_id}')"))
        k,t,r=[str(uuid.uuid4()) for _ in range(3)]
        run("historical_untagged_receipt_is_not_promoted_to_live", "begin;"+receipt(k,t,r)+"commit;"
            +check(f"not exists(select 1 from concert_gift_command_targets where receipt_id='{r}')"))
        require(pins=={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},"migration source changed")
        report["result"]="PASS"
    except BaseException as error:
        report["result"]="FAIL"
        report["failure"]=str(error) if isinstance(error,RuntimeError) else type(error).__name__
        if isinstance(error,migration.SmokeError):
            report["diagnostic"]=error.detail
    finally:
        try:
            if cluster is not None:
                root_path=cluster.root
                cluster.cleanup()
                require(not root_path.exists(),"owned PG root remains")
            report["cleanup"]="PASS"
        except BaseException:
            report["cleanup"]="FAIL"
            report["result"]="FAIL"
        with os.fdopen(fd,"w") as stream:
            json.dump(report,stream,indent=2)
            stream.write("\n")
    print(json.dumps({"result":report["result"],"cases":len(report["cases"]),"cleanup":report["cleanup"]}))
    return 0 if report["result"]=="PASS" else 1

if __name__=="__main__":
    raise SystemExit(main())
