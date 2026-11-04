#!/usr/bin/env python3
"""Qualify the local seed producer in a new owned PG16, under an accepted lease.

Composes existing migration/marker guards and the actual owning IN01 catalog
producer. Does not attach a prior runtime, create an Ops role or claim IN04.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
from migration_smoke import Cluster, PG_TOOLS, SmokeError, check_ledger, executable, execute, migration_manifest
from auth_invalidation_smoke import frozen_binary
from world_in01_prepare import seed_concert
sys.path.insert(0, str(ROOT / "backend/scripts/mcr/wall_tools_b"))
from resource_guard import NativeBudget

ART_SOURCE = "ca5d4bcad50d6159597309a961a29a8e4d1f3bb9"
ART_MANIFEST = "3f1aa318f0e4b7d8fa7f079b1101f7386619291a8dbb3bb04ef70820952f34ce"
FILES = ("manifest.json", "source.svg", "pixels.bin", "palette.json", "preview.png", "README.md")
CASES = ("actual_pack_and_tamper_rejection", "native_seed_atomic_operator_binding_and_conflicts",
         "native_seed_concurrency_history_and_immutable_provenance", "operator_provision_bootstrap_event")

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def source_manifest():
    # Read the frozen Wall source-closure implementation without changing it;
    # additionally include all Python helpers reachable by the owning bootstrap.
    path = ROOT / "backend/scripts/mcr/wall_tools_b/qualify.py"
    spec = importlib.util.spec_from_file_location("frozen_wall_closure", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    result = module.source_manifest()
    for path in (ROOT / "backend/scripts/mcr").rglob("*.py"):
        result[str(path.relative_to(ROOT))] = sha(path)
    return dict(sorted(result.items()))

def pack_manifest(path):
    if not path.is_absolute() or not path.is_dir() or path.is_symlink():
        raise SmokeError("source", "explicit frozen six-file art readset required")
    if {p.name for p in path.iterdir()} != set(FILES):
        raise SmokeError("source", "exact six art input files required")
    if any(not (path / n).is_file() or (path / n).is_symlink() for n in FILES):
        raise SmokeError("source", "regular frozen pack files required")
    if sha(path / "manifest.json") != ART_MANIFEST:
        raise SmokeError("source", "actual selected art manifest mismatch")
    return {n: sha(path / n) for n in FILES}

def qualify(checkpoint, pg, report, budget, live_owner):
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise SmokeError("source", "clean committed source required")
    if checkpoint["source_commit"] != commit or checkpoint["source_sha256"] != source_manifest():
        raise SmokeError("source", "source checkpoint drift")
    binary = frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])
    concert = frozen_binary(Path(checkpoint["concert_binary"]), checkpoint["concert_binary_sha256"])
    art = Path(checkpoint["art_directory"])
    if checkpoint["art_source"] != ART_SOURCE or checkpoint["art_sha256"] != pack_manifest(art):
        raise SmokeError("source", "frozen actual art readset drift")
    tools = {name: executable(pg / name) for name in PG_TOOLS}
    budget.check()
    cluster = Cluster(tools)
    report.update(source_commit=commit, binary_sha256=checkpoint["binary_sha256"],
                  concert_binary_sha256=checkpoint["concert_binary_sha256"],
                  source_sha256=checkpoint["source_sha256"],art_source=ART_SOURCE,
                  art_sha256=checkpoint["art_sha256"],owned_root=str(cluster.root), phases={})
    try:
        if live_owner:
            with live_owner.open("x") as stream:
                live_owner.chmod(0o600)
                json.dump({"owned_root": str(cluster.root), "owner_identity": list(cluster.identity),
                           "marker_sha256": sha(cluster.marker)}, stream)
        budget.check(cluster)
        cluster.init()
        cluster.start()
        budget.check(cluster)
        version = cluster.sql("postgres", "show server_version_num;")
        if int(version) // 10000 != 16:
            raise SmokeError("preflight", "PG16 required")
        env = {**cluster.env,"MCR_MURAL_SEED_PACK":str(art),"RUST_MIN_STACK":"16777216",
               "MCR_CANONICAL_ROOT":str(cluster.root),"MCR_CANONICAL_ROOT_TOKEN":cluster.token,
               "MCR_CANONICAL_ROOT_IDENTITY":f"{cluster.identity[0]}:{cluster.identity[1]}",
               "MCR_CANONICAL_SYSTEM":cluster.sql("postgres","select system_identifier from pg_control_system();")}
        inventory = budget.execute([str(binary),"--list"],cluster=cluster,env=env,phase="inventory",cwd=ROOT/"backend")
        if set(line.removesuffix(": test") for line in inventory.stdout.splitlines() if line.endswith(": test")) != set(CASES):
            raise SmokeError("inventory","exact four seed cases required")
        for index, case in enumerate(CASES):
            budget.check(cluster)
            if index in (1,2):
                if index==2:
                    cluster.sql("postgres","drop database mcr_concert_wall_b with (force);")
                execute([str(tools["createdb"]),"--no-password","--maintenance-db=postgres","mcr_concert_wall_b"],env=cluster.client_env("postgres"),cwd=cluster.root,phase="create-owned-concert")
                env["MCR_CANONICAL_CONCERT_URL"] = cluster.url("mcr_concert_wall_b")
            if index==3:
                # Exact existing owning producer: real migration commands and
                # actual source-bound --prepare-in01-catalog. No arbitrary UUID.
                seed_concert(SimpleNamespace(cluster=cluster),concert,report,True)
                env["MCR_CANONICAL_CONCERT_URL"] = cluster.url("mcr_concert")
            result=budget.execute([str(binary),"--ignored","--exact",case,"--test-threads=1","--nocapture"],
                                  cluster=cluster,env=env,phase=case,cwd=ROOT/"backend",timeout=30)
            if "test result: ok. 1 passed; 0 failed;" not in result.stdout:
                raise SmokeError("inventory","expected one exact passing seed case")
            report["phases"][case]={"status":"PASS","exit_code":result.returncode,"stdout":result.stdout}
            prepared=int(cluster.sql("postgres","select count(*) from pg_prepared_xacts;"))
            if prepared:
                raise SmokeError("settlement","seed case left prepared transactions")
            report["phases"][case]["prepared_after"]=prepared
            if index:
                db="mcr_concert" if index==3 else "mcr_concert_wall_b"
                report["phases"][case]["migration_count"]=check_ledger(cluster,db,migration_manifest(ROOT/"backend","concert-service"))
            if index==3:
                records=[json.loads(line.split("MURAL_SEED_PROVISION=",1)[1]) for line in result.stdout.splitlines() if "MURAL_SEED_PROVISION=" in line]
                if len(records)!=1:
                    raise SmokeError("provision","exact owning operator readout required")
                report["operator_provision"]=records[0]
        sql="select md5(jsonb_build_object('state',(select jsonb_agg(s order by event_id) from concert_mural_state s),'config',(select jsonb_agg(c order by event_id) from concert_mural_tool_config c),'provenance',(select jsonb_agg(p order by event_id) from concert_mural_seed_provenance p))::text);"
        before=cluster.sql("mcr_concert",sql)
        pid=cluster.assert_postmaster();start=cluster.sql("postgres","select pg_postmaster_start_time();")
        budget.check(cluster)
        cluster.stop();cluster.start()
        budget.check(cluster)
        after=cluster.sql("mcr_concert",sql);after_pid=cluster.assert_postmaster();after_start=cluster.sql("postgres","select pg_postmaster_start_time();")
        if before!=after or pid==after_pid or start==after_start:
            raise SmokeError("restart","actual seed/provenance restart recovery failed")
        report["restart"]={"status":"PASS","before_pid":pid,"after_pid":after_pid,"before_start":start,"after_start":after_start,"durable_digest":after}
        if source_manifest()!=checkpoint["source_sha256"] or pack_manifest(art)!=checkpoint["art_sha256"] or sha(binary)!=checkpoint["binary_sha256"] or sha(concert)!=checkpoint["concert_binary_sha256"]:
            raise SmokeError("source","source, art or executable drift")
    finally:
        report["resource"]=budget.receipt()
        cluster.cleanup()
        report["cleanup"]={"status":"PASS","owned_root_absent":not cluster.root.exists()}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--checkpoint",type=Path,required=True);p.add_argument("--pg-bin-dir",type=Path,required=True)
    p.add_argument("--report",type=Path,required=True);p.add_argument("--live-owner",type=Path,required=True)
    p.add_argument("--lease-id",required=True);p.add_argument("--deadline-unix",type=float,required=True)
    p.add_argument("--min-free-bytes",type=int,required=True);p.add_argument("--max-owned-bytes",type=int,required=True)
    args=p.parse_args()
    if any(not path.is_absolute() for path in (args.checkpoint,args.pg_bin_dir,args.report,args.live_owner)) or args.report.exists() or args.live_owner.exists():
        p.error("explicit absolute inputs and new report/live-owner paths required")
    report={"status":"FAIL","limitations":["Local operator producer and actual source-bound owning catalog only; no invented production Ops role.","Authenticated server C03/snapshot and default-App/two-client IN04 remain NOT_RUN.","Load qualification DEFERRED_BY_USER."]}
    try:
        budget=NativeBudget(args.lease_id,args.deadline_unix,args.min_free_bytes,args.max_owned_bytes)
        qualify(json.loads(args.checkpoint.read_text()),args.pg_bin_dir,report,budget,args.live_owner)
        report["status"]="PASS"
    except SmokeError as error:
        report["failure"]=error.detail
    except Exception as error:
        report["failure"]={"phase":"runner","diagnostic":type(error).__name__,"detail":str(error)}
    args.report.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
    print(json.dumps({"status":report["status"],"report":str(args.report)}))
    return 0 if report["status"]=="PASS" else 1

if __name__=="__main__":
    sys.exit(main())
