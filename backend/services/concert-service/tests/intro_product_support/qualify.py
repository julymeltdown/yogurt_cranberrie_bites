#!/usr/bin/env python3
"""Run the frozen canonical consumer against a newly owned socket-only PG16."""
from __future__ import annotations
import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
from migration_smoke import (Cluster, PG_TOOLS, SmokeError, check_ledger,
                             executable, execute, migration_manifest)
from auth_invalidation_smoke import frozen_binary
from invalidation_clients_smoke import pki

OWNERS = ("auth", "friend", "concert", "profile")
DATABASES = {owner: ("mcr_concert_intro" if owner == "concert" else f"mcr_canonical_{owner}")
             for owner in OWNERS}
# Exact names are required: a filtered or wholly ignored inventory is not proof.
CASES = ("canonical_authoring_discovery_detail_idempotency_and_close",
         "canonical_restricted_age_outage_trust_and_expiry_parity",
         "canonical_full_frame_rebinding_and_missing_parent_fail_closed",
         "canonical_block_departure_suspension_and_profile_outage",
         "canonical_claim_expiry_bounds_root_delivery_after_commit",
         "canonical_intro_edit_revokes_consent_atomically_with_receipt_outbox",
         "canonical_native_witness_requires_exact_registered_transaction")

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def source_manifest() -> dict[str, str]:
    paths = {ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock", Path(__file__).resolve()}
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

def qualify(checkpoint: dict, pg: Path, report: dict) -> None:
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if checkpoint["source_commit"] != commit or checkpoint["source_sha256"] != source_manifest():
        raise SmokeError("preflight", "source checkpoint changed")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise SmokeError("preflight", "clean committed fixture source required")
    binary = frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])
    if shutil.disk_usage(ROOT).free < 8 * 1024**3:
        raise SmokeError("preflight", "8 GiB free required")
    tools = {name: executable(pg / name) for name in PG_TOOLS}
    cluster = Cluster(tools)
    report.update(source_commit=commit, source_sha256=checkpoint["source_sha256"],
                  binary_sha256=checkpoint["binary_sha256"], owned_root=str(cluster.root), phases={})
    try:
        cluster.init()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            # Holds both source-owner guards plus full-page local children.
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        pki(cluster)
        profile_pki(cluster)
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
        result=execute([str(binary), "--test-threads=1", "--nocapture"], env=env, phase="pure", cwd=ROOT/"backend")
        if "test result: ok. 2 passed; 0 failed; 7 ignored;" not in result.stdout:
            raise SmokeError("inventory", "exact two pure and seven native cases required")
        report["pure"]={"exit_code":result.returncode,"stdout":result.stdout}
        for index,case in enumerate(CASES):
            # Every native case receives a fresh four-store fixture. Selected
            # release event IDs stay canonical without cross-test rows.
            for owner in OWNERS:
                db=DATABASES[owner]
                if index:
                    cluster.sql("postgres", f"drop database {db} with (force);")
                execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db], env=cluster.client_env("postgres"),phase=f"create-{owner}",cwd=cluster.root)
                env[f"MCR_CANONICAL_{owner.upper()}_URL"]=cluster.url(db)
            result=execute([str(binary),"--ignored","--exact",case,"--test-threads=1","--nocapture"],env=env,phase=case,cwd=ROOT/"backend",timeout=60)
            if "test result: ok. 1 passed; 0 failed;" not in result.stdout:
                raise SmokeError("inventory","expected one exact native test")
            report["phases"][case]={"status":"PASS","exit_code":result.returncode,"stdout":result.stdout}
            report["migration_receipts"][case]={}
            for owner in OWNERS:
                db=DATABASES[owner]
                report["migration_receipts"][case][owner]=check_ledger(cluster,db,migration_manifest(ROOT/"backend",f"{owner}-service"))
            prepared=int(cluster.sql("postgres","select count(*) from pg_prepared_xacts;"))
            if index != len(CASES)-1 and prepared:
                raise SmokeError("cleanup","settled case left prepared transactions")
            # The final case intentionally commits an effect without a witness.
            # UNKNOWN must preserve actual owner exclusion. Only this disposable
            # owned cluster's final destruction removes that quarantined fixture.
            report["phases"][case]["prepared_after"]=prepared
            if index == len(CASES)-1 and not prepared:
                raise SmokeError("quarantine","unknown native commit released every owner")
        retained=checkpoint.get("retained",{})
        if set(retained)!={"intro"}:
            raise SmokeError("inventory","retained Intro engine artifact required")
        artifact=retained["intro"]
        owned_binary=frozen_binary(Path(artifact["binary"]),artifact["binary_sha256"])
        db="mcr_intro_be"
        execute([str(tools["createdb"]),"--no-password","--maintenance-db=postgres",db],env=cluster.client_env("postgres"),phase="retained-create",cwd=cluster.root)
        # Retained engine regression seam intentionally uses its historical
        # explicit synthetic authority schema. Canonical acceptance is above.
        for name in ("0001_concert_core.sql","0002_command_receipts_outbox.sql","0006_relationship_policy.sql","0008_meeting_beacons.sql","intro_schema_v1.sql"):
            prefix="src/adapters/out/repository" if name=="intro_schema_v1.sql" else "migrations"
            cluster.sql(db,(ROOT/"backend/services/concert-service"/prefix/name).read_text())
        owned_env={**env,"CONCERT_TEST_DATABASE_URL":cluster.url(db),"CONCERT_TEST_OWNER_ROOT":str(cluster.root),"CONCERT_TEST_OWNER_TOKEN":cluster.token,"CONCERT_TEST_OWNER_IDENTITY":f"{cluster.identity[0]}:{cluster.identity[1]}"}
        result=execute([str(owned_binary),"--test-threads=1","--nocapture"],env=owned_env,phase="retained-intro",cwd=ROOT/"backend",timeout=180)
        if "test result: ok. 4 passed; 0 failed;" not in result.stdout:
            raise SmokeError("inventory","exact four retained Intro engine cases required")
        report["retained"]={"intro":{"status":"PASS","exit_code":result.returncode,"stdout":result.stdout,"binary_sha256":artifact["binary_sha256"]}}
        before_row=cluster.sql(db,"select state,revision from concert_intros order by created_at,intro_id limit 1;")
        before_pid = cluster.assert_postmaster()
        before_start = cluster.sql("postgres", "select pg_postmaster_start_time();")
        cluster.stop()
        cluster.start()
        after_pid = cluster.assert_postmaster()
        after_start = cluster.sql("postgres", "select pg_postmaster_start_time();")
        if before_pid == after_pid or before_start == after_start:
            raise SmokeError("restart", "postmaster identity unchanged")
        after_row=cluster.sql("mcr_intro_be","select state,revision from concert_intros order by created_at,intro_id limit 1;")
        if before_row!=after_row:
            raise SmokeError("restart","retained Intro row changed after restart")
        report["restart"]={"status":"PASS","before_pid":before_pid,"after_pid":after_pid,"before_start":before_start,"after_start":after_start,"retained_row":after_row}
        if source_manifest() != checkpoint["source_sha256"] or sha(binary) != checkpoint["binary_sha256"]:
            raise SmokeError("provenance", "source or frozen artifact drifted")
    finally:
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS", "owned_root_absent": not cluster.root.exists()}

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not args.report.is_absolute() or args.report.exists() or not args.pg_bin_dir.is_absolute():
        parser.error("new absolute report and explicit absolute PG directory required")
    report = {"status": "FAIL", "limitations": [
        "Actual canonical Intro facade, Auth/Friend owners, Profile RPC and same-TX Age projection; Root HTTP composition has a separate owner.",
        "Commercial provider activation, Root HTTP composition and full browser/two-client IN03 are NOT_RUN."]}
    try:
        qualify(json.loads(args.checkpoint.read_text()), args.pg_bin_dir, report)
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
