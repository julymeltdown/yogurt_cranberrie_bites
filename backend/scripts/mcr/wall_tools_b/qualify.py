#!/usr/bin/env python3
"""Run committed Wall B source and frozen binary in a newly owned socket-only PG16.
Adapted from intro_product_support/qualify.py at7d5d91bd; preserves marker and source checks.
A separate accepted resource offer is mandatory before invoking native qualification.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
from migration_smoke import (Cluster, PG_TOOLS, SmokeError, check_ledger,
                             executable, execute, migration_manifest)
from auth_invalidation_smoke import frozen_binary
from invalidation_clients_smoke import pki
sys.path.insert(0, str(Path(__file__).resolve().parent))
from resource_guard import NativeBudget

OWNERS = ("auth", "friend", "concert", "profile")
DATABASES = {owner: ("mcr_concert_wall_b" if owner == "concert" else f"mcr_canonical_{owner}")
             for owner in OWNERS}
# Exact names are required: a filtered or wholly ignored inventory is not proof.
CASES = ("canonical_fill_undo_budget_recovery_and_restart", "canonical_fill_257_noop_region_lock_and_failures_are_atomic", "canonical_pencil_shares_undo_budget_and_storage_failure_rolls_back", "canonical_tabs_revision_cooldown_pause_and_admission_are_server_owned", "canonical_activation_cannot_forgive_recent_unaccounted_pencil", "canonical_actor_budget_and_private_recovery_are_isolated", "canonical_receipt_expiry_and_history_retirement_never_refund_budget")

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def source_manifest() -> dict[str, str]:
    paths = {Path(__file__).resolve().with_name("resource_guard.py"),
             ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock",
             ROOT / "scripts/mcr/validate-domain.py", Path(__file__).resolve()}
    for prefix in ("backend/crates", "backend/proto", "backend/services/auth-service",
                   "backend/services/friend-service", "backend/services/concert-service",
                   "backend/services/profile-service", "contracts"):
        paths.update(p for p in (ROOT / prefix).rglob("*")
                     if p.is_file() and p.suffix in {".rs", ".sql", ".toml", ".proto", ".py", ".json"})
    for config in (".cargo/config.toml", "backend/.cargo/config.toml", "rust-toolchain.toml", "backend/rust-toolchain.toml"):
        if (ROOT / config).is_file():
            paths.add(ROOT / config)
    for name in ("migration_smoke.py", "auth_invalidation_smoke.py", "invalidation_clients_smoke.py"):
        paths.add(ROOT / "backend/scripts/mcr" / name)
    # Actual release construction also reads selected source-bound static bytes.
    for prefix in ("backend/releases", "frontend/public"):
        paths.update(p for p in (ROOT / prefix).rglob("*") if p.is_file())
    return {str(p.relative_to(ROOT)): sha(p) for p in sorted(paths)}

def validate_responses(stdout: str) -> int:
    import importlib.util
    spec=importlib.util.spec_from_file_location("wall_contract",ROOT/"scripts/mcr/validate-domain.py")
    domain=importlib.util.module_from_spec(spec);spec.loader.exec_module(domain)
    contract=domain.Contract();count=0
    for line in stdout.splitlines():
        if "WALL_B_RESPONSE=" not in line: continue
        row=json.loads(line.split("WALL_B_RESPONSE=",1)[1]);req=row["request"];body=row["body"]
        path,_,query=req["target"].partition("?")
        context={"actor_id":req["actor_id"],"event_id":req["event_id"],"request_path":path,"request_query":query}
        if req["idempotency_key"]:
            context.update(idempotency_key=req["idempotency_key"],original_command={"operation_id":req["operation_id"],"body":req["original_body"]})
        elif req["operation_id"]=="getOwnConcertMuralAction" and row["status"]==200:
            # Native case separately asserts equality to the retained original POST.
            context.update(idempotency_key=path.rsplit("/",1)[1],original_command=body["command"])
        contract.validate_http(body,req["operation_id"],row["status"],**context);count+=1
    if count==0:raise SmokeError("contract","no actual producer responses captured")
    return count

def profile_pki(cluster: Cluster) -> None:
    """Additional actual workload certificate; retained helper covers four peers."""
    cluster.assert_owned()
    directory=cluster.root/"pki"
    extension=directory/"profile.ext"
    extension.write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:profile.svc.internal,URI:spiffe://internal/profile\n")
    openssl=executable(Path("/usr/bin/openssl"))
    for args in (("req","-new","-newkey","rsa:2048","-nodes","-keyout","profile.key","-out","profile.csr","-subj","/CN=profile.svc.internal"),
                 ("x509","-req","-in","profile.csr","-CA","ca.crt","-CAkey","ca.key","-CAcreateserial","-out","profile.crt","-days","2","-extfile",str(extension))):
        execute([str(openssl),*args],env=cluster.env,cwd=directory,phase="fixture-profile-pki",timeout=30)
    (directory/"profile.key").chmod(0o600)

def qualify(checkpoint: dict, pg: Path, report: dict, budget: NativeBudget) -> None:
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if checkpoint["source_commit"] != commit or checkpoint["source_sha256"] != source_manifest():
        raise SmokeError("preflight", "source checkpoint changed")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise SmokeError("preflight", "clean committed fixture source required")
    binary = frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])
    budget.check()
    tools = {name: executable(pg / name) for name in PG_TOOLS}
    cluster = Cluster(tools)
    report.update(source_commit=commit, source_sha256=checkpoint["source_sha256"],
                  binary_sha256=checkpoint["binary_sha256"], owned_root=str(cluster.root), phases={})
    try:
        budget.check(cluster)
        cluster.init()
        budget.check(cluster)
        with (cluster.data / "postgresql.conf").open("a") as stream:
            # Holds both source-owner guards plus full-page local children.
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        pki(cluster)
        profile_pki(cluster)
        budget.check(cluster)
        version = cluster.sql("postgres", "show server_version_num;")
        if int(version) // 10000 != 16:
            raise SmokeError("preflight", "PG16 required")
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        env = {**cluster.env,
               "CONCERT_SERVICE_CREDENTIAL": "concert-fixture",
               "RUST_MIN_STACK": "16777216", # unoptimized in-process four-service fixture
               "MCR_CANONICAL_ROOT": str(cluster.root),
               "MCR_CANONICAL_ROOT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
               "MCR_CANONICAL_ROOT_TOKEN": cluster.token,
               "MCR_CANONICAL_SYSTEM": cluster.sql("postgres", "select system_identifier from pg_control_system();")}
        report["migration_receipts"] = {}
        report["pure"] = {}
        result=budget.execute([str(binary), "--test-threads=1", "--nocapture"], cluster=cluster, env=env, phase="pure", cwd=ROOT/"backend")
        if "test result: ok. 1 passed; 0 failed; 7 ignored;" not in result.stdout:
            raise SmokeError("inventory", "exact one pure and seven native cases required")
        report["pure"]={"exit_code":result.returncode,"stdout":result.stdout}
        for index,case in enumerate(CASES):
            budget.check(cluster)
            # Every native case receives a fresh four-store fixture. Selected
            # release event IDs stay canonical without cross-test rows.
            for owner in OWNERS:
                db=DATABASES[owner]
                if index:
                    cluster.sql("postgres", f"drop database {db} with (force);")
                execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db], env=cluster.client_env("postgres"),phase=f"create-{owner}",cwd=cluster.root)
                env[f"MCR_CANONICAL_{owner.upper()}_URL"]=cluster.url(db)
            result=budget.execute([str(binary),"--ignored","--exact",case,"--test-threads=1","--nocapture"],cluster=cluster,env=env,phase=case,cwd=ROOT/"backend",timeout=60)
            if "test result: ok. 1 passed; 0 failed;" not in result.stdout:
                raise SmokeError("inventory","expected one exact native test")
            report["phases"][case]={"status":"PASS","exit_code":result.returncode,"stdout":result.stdout,"contract_responses":validate_responses(result.stdout)}
            report["migration_receipts"][case]={}
            for owner in OWNERS:
                db=DATABASES[owner]
                report["migration_receipts"][case][owner]=check_ledger(cluster,db,migration_manifest(ROOT/"backend",f"{owner}-service"))
            prepared=int(cluster.sql("postgres","select count(*) from pg_prepared_xacts;"))
            if index != len(CASES)-1 and prepared:
                raise SmokeError("cleanup","settled case left prepared transactions")
            report["phases"][case]["prepared_after"]=prepared
            if prepared:
                raise SmokeError("cleanup","native Wall case left prepared transactions")
        before=cluster.sql(DATABASES["concert"],"select md5(string_agg(to_jsonb(s)::text,',' order by event_id)) from concert_mural_state s;")
        before_pid=cluster.assert_postmaster()
        before_start=cluster.sql("postgres","select pg_postmaster_start_time();")
        budget.check(cluster)
        cluster.stop(); cluster.start()
        budget.check(cluster)
        after_pid=cluster.assert_postmaster()
        after_start=cluster.sql("postgres","select pg_postmaster_start_time();")
        after=cluster.sql(DATABASES["concert"],"select md5(string_agg(to_jsonb(s)::text,',' order by event_id)) from concert_mural_state s;")
        if before!=after or before_pid==after_pid or before_start==after_start:
            raise SmokeError("restart","Wall durable state changed or postmaster did not restart")
        report["postgres_restart"]={"status":"PASS","before_pid":before_pid,"after_pid":after_pid,"before_start":before_start,"after_start":after_start,"durable_state_digest":after}
        if source_manifest() != checkpoint["source_sha256"] or sha(binary) != checkpoint["binary_sha256"]:
            raise SmokeError("provenance", "source or frozen artifact drifted")
    finally:
        report["resource"] = budget.receipt()
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS", "owned_root_absent": not cluster.root.exists()}

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--lease-id", required=True, help="exact already accepted native allocation ID")
    parser.add_argument("--deadline-unix", type=float, required=True)
    parser.add_argument("--min-free-bytes", type=int, required=True)
    parser.add_argument("--max-owned-bytes", type=int, required=True)
    args = parser.parse_args()
    if not args.report.is_absolute() or args.report.exists() or not args.pg_bin_dir.is_absolute():
        parser.error("new absolute report and explicit absolute PG directory required")
    report = {"status": "FAIL", "limitations": [
        "Canonical Wall Product facade and actual Auth/Friend native transaction coverage; no fabricated MuralAuthority success.",
        "Actual startup/Gateway HTTP, persistent Phaser, two-client world and complete IN04 remain NOT_RUN.",
        "Performance/load qualification is DEFERRED_BY_USER."]}
    try:
        budget=NativeBudget(args.lease_id,args.deadline_unix,args.min_free_bytes,args.max_owned_bytes)
        qualify(json.loads(args.checkpoint.read_text()), args.pg_bin_dir, report, budget)
        report["status"] = "PASS"
    except SmokeError as error:
        report["failure"] = error.detail
    except Exception as error:
        report["failure"] = {"phase": "runner", "diagnostic": type(error).__name__, "detail": str(error)}
    finally:
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1

if __name__ == "__main__":
    sys.exit(main())
