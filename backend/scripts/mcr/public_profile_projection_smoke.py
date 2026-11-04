#!/usr/bin/env python3
"""Freeze exact Cargo test artifacts, then qualify an isolated held Profile read.

This program never invokes Cargo or Docker and never uses a service-default URL.
Optional --joined qualifies the real owner/reader core, separately from product release and HTTP delivery.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
TARGETS = {"concert_public_profile": "services/profile-service",
           "public_profile_projection": "services/concert-service"}

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def source_pins():
    paths = subprocess.check_output(["git", "ls-files", "backend", "contracts"], cwd=ROOT, text=True).splitlines()
    paths += subprocess.check_output(["git", "ls-files", "--others", "--exclude-standard",
        "backend/services/profile-service/src/concert_public_profile",
        "backend/services/profile-service/src/adapters/in/grpc/concert_public_profile.rs",
        "backend/services/profile-service/tests/concert_public_profile.rs",
        "backend/services/profile-service/tests/support/public_profile",
        "backend/services/profile-service/migrations",
        "backend/services/concert-service/src/adapters/out/public_profile",
        "backend/services/concert-service/tests/public_profile_projection.rs",
        "backend/scripts/mcr/public_profile_projection_smoke.py"], cwd=ROOT, text=True).splitlines()
    return {p: sha(ROOT/p) for p in sorted(set(paths)) if Path(p).suffix in {".rs", ".sql", ".proto", ".json", ".toml", ".lock", ".py"} and (ROOT/p).is_file()}

def select_artifacts(items, root=ROOT, profile_only=False, joined=False):
    result = {}
    targets = {**TARGETS, **({"concert_service": "services/concert-service"} if joined else {})}
    if joined and profile_only: raise ValueError("joined and profile-only are incompatible")
    for name, package in targets.items():
        if profile_only and name != "concert_public_profile": continue
        manifest = str(root/"backend"/package/"Cargo.toml")
        candidates = [a for a in items if a.get("reason") == "compiler-artifact"
            and a.get("executable") and a.get("target", {}).get("name") == name
            and a.get("manifest_path") == manifest and a.get("profile", {}).get("test")
            and (name != "concert_service" or a.get("target", {}).get("kind") == ["lib"])]
        if len(candidates) != 1:
            raise ValueError("one exact test artifact required: " + name)
        if candidates[0]["profile"].get("debuginfo") != 0:
            raise ValueError("debug zero artifact required")
        result[name] = candidates[0]
    return result

def freeze(json_paths, profile_only=False, joined=False):
    items = [json.loads(line) for path in json_paths for line in path.read_text().splitlines() if line.startswith("{")]
    artifacts = select_artifacts(items, profile_only=profile_only, joined=joined)
    out = Path(tempfile.mkdtemp(prefix="mcr-public-profile-frozen-", dir="/tmp")).resolve()
    report = {"head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "profile_only": profile_only, "joined": joined, "source_sha256": source_pins(), "cargo_json_sha256": [sha(p) for p in json_paths], "artifacts": {}}
    for name, artifact in artifacts.items():
        src = Path(artifact["executable"]).resolve(strict=True)
        if src.is_symlink() or not src.is_file():
            raise ValueError("regular executable required")
        dst = out/name
        shutil.copy2(src, dst)
        assert sha(src) == sha(dst)
        report["artifacts"][name] = {"binary": str(dst), "sha256": sha(dst), "artifact": artifact}
    path = out/"pin.json"
    path.write_text(json.dumps(report, indent=2)+"\n")
    return path

def verify_pin(path):
    out = path.resolve(strict=True).parent
    if out.stat().st_mode & 0o077 or not out.name.startswith("mcr-public-profile-frozen-"):
        raise ValueError("private frozen directory required")
    report = json.loads(path.read_text())
    expected = {"concert_public_profile"} if report.get("profile_only") is True else set(TARGETS)
    if report.get("joined"):
        if report.get("profile_only"): raise ValueError("incompatible artifact mode")
        expected.add("concert_service")
    if set(report["artifacts"]) != expected:
        raise ValueError("exact test artifact set required")
    for name, artifact in report["artifacts"].items():
        binary = Path(artifact["binary"])
        if binary.is_symlink() or binary.resolve().parent != out or binary.name != name or sha(binary) != artifact["sha256"]:
            raise ValueError("frozen executable mismatch")
    if report["source_sha256"] != source_pins():
        raise ValueError("source inventory or bytes changed after artifact pin")
    return out, report

def qualify(path, pg_bin):
    out, report = verify_pin(path)
    report.update(result="IN_PROGRESS", commands={}, limits=["controlled loopback RPC; production TLS NOT_RUN",
        "canonical Auth/Friend guard plus complete Feed response joined gate NOT_RUN by this controller",
        "no production public-profile capability claim"])
    cluster = None
    def save(): (out/"native-report.json").write_text(json.dumps(report, indent=2)+"\n")
    def run(name, binary, options, env):
        if cluster: cluster.assert_owned()
        item = report["artifacts"][binary]
        assert sha(item["binary"]) == item["sha256"]
        log = out/(name+".log")
        command = [item["binary"], *options, "--test-threads=1"]
        started = time.monotonic()
        try:
            with log.open("w") as stream:
                code = subprocess.run(command, cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT, timeout=60).returncode
        except subprocess.TimeoutExpired:
            code = "TIMEOUT"
        text = log.read_text()
        counts = [int(n) for n in re.findall(r"test result: ok\. (\d+) passed;", text)]
        report["commands"][name] = {"args": command, "exit_code": code, "passed": sum(counts),
            "seconds": time.monotonic()-started, "log_sha256": sha(log)}
        save()
        assert code == 0 and sum(counts) > 0, "test did not pass with actual execution"
        if cluster:
            assert cluster.password not in text and cluster.url("mcr_profile_self_rpc") not in text
    save()
    try:
        env = {k: v for k, v in os.environ.items() if not k.startswith(("PG", "MCR_PROFILE_RPC_", "MCR_PUBLIC_PROFILE_")) and not k.endswith("DATABASE_URL")}
        run("signed-profile", "concert_public_profile", [], env)
        if not report.get("profile_only"):
            run("closed-c03-union", "public_profile_projection", [], env)
        if report.get("joined"):
            run("authority-denial-regression", "concert_service", ["--exact", "adapters::out::public_profile::tests::guard_or_transport_failure_cannot_become_a_skippable_author"], env)
        sys.path.insert(0, str(ROOT/"backend/scripts/mcr"))
        import migration_smoke as native
        cluster = native.Cluster({name: pg_bin/name for name in native.PG_TOOLS})
        report["owned_root"] = str(cluster.root)
        report["owner_identity"] = list(cluster.identity)
        save()
        cluster.init()
        with (cluster.data/"postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions = 256\n")
        cluster.start()
        database = "mcr_profile_self_rpc"  # retained fixture helper's exact isolated name
        cluster.sql("postgres", "create database mcr_profile_self_rpc template template0;")
        migrations = sorted((ROOT/"backend/services/profile-service/migrations").glob("*.sql"))
        numbers = [int(p.name.split("_",1)[0]) for p in migrations]
        assert len(migrations) >= 7 and numbers == list(range(1,len(migrations)+1))
        report["migration_sha256"] = {p.name:sha(p) for p in migrations}
        for migration in migrations: cluster.sql(database, migration.read_text())
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        report["max_prepared_transactions"] = cluster.sql("postgres", "show max_prepared_transactions;")
        fixture = {**cluster.env, "MCR_PROFILE_RPC_OWNER_ROOT": str(cluster.root),
            "MCR_PROFILE_RPC_OWNER_IDENTITY": ":".join(map(str, cluster.identity)),
            "MCR_PROFILE_RPC_OWNER_TOKEN": cluster.token,
            "MCR_PROFILE_RPC_DATABASE_URL": cluster.url(database),
            "MCR_PUBLIC_PROFILE_RESTART_FILE": str(out/"private-restart-child.json")}
        run("native-privacy", "concert_public_profile", ["--exact", "native_current_public_privacy_tombstones_and_minimal_wire", "--ignored"], fixture)
        run("native-held-exclusion", "concert_public_profile", ["--exact", "native_prepared_child_excludes_privacy_and_direct_tombstone_until_settled", "--ignored"], fixture)
        run("native-original-budget", "concert_public_profile", ["--exact", "native_original_budget_bounds_owner_lock_wait_and_recovers", "--ignored"], fixture)
        run("native-capacity", "concert_public_profile", ["--exact", "native_full_hundred_child_capacity_and_fail_closed_overflow", "--ignored"], fixture)
        run("native-pre-dispatch-cancel", "concert_public_profile", ["--exact", "native_pre_dispatch_cancellation_prevents_late_read", "--ignored"], fixture)
        run("native-lost-ack", "concert_public_profile", ["--exact", "native_lost_read_and_completion_replies_recover_from_actual_receipt", "--ignored"], fixture)
        run("native-absence-not-proof", "concert_public_profile", ["--exact", "native_gid_absence_never_settles_active_committed_or_misbound_receipt", "--ignored"], fixture)
        run("native-owner-before-restart", "concert_public_profile", ["--exact", "native_prepare_restart_quarantine", "--ignored"], fixture)
        assert cluster.sql(database, "select count(*) from pg_prepared_xacts where database=current_database();") == "1"
        cluster.stop()
        time.sleep(2)  # expire the original read; never releases its prepared child
        cluster.start()
        assert cluster.sql(database, "select count(*) from pg_prepared_xacts where database=current_database();") == "1"
        run("native-settlement-after-restart", "concert_public_profile", ["--exact", "native_settle_after_database_restart_and_original_expiry", "--ignored"], fixture)
        assert cluster.sql(database, "select count(*) from pg_prepared_xacts where database=current_database();") == "0"
        if report.get("joined"):
            from invalidation_clients_smoke import pki
            pki(cluster)
            directory = cluster.root/"pki"
            # Add only this fixture's Profile identity to the private CA.
            extension = directory/"profile.ext"
            extension.write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:profile.svc.internal,URI:spiffe://internal/profile\n")
            for options in [
                ["req","-new","-newkey","rsa:2048","-nodes","-keyout","profile.key","-out","profile.csr","-subj","/CN=profile.svc.internal"],
                ["x509","-req","-in","profile.csr","-CA","ca.crt","-CAkey","ca.key","-CAcreateserial","-out","profile.crt","-days","2","-extfile",str(extension)]]:
                native.execute(["/usr/bin/openssl",*options], env=cluster.env, cwd=directory, phase="public-fixture-pki", timeout=30)
            (directory/"profile.key").chmod(0o600)
            canonical = {**cluster.env,
                "CONCERT_SERVICE_CREDENTIAL":"concert-fixture",
                "MCR_CANONICAL_ROOT":str(cluster.root),
                "MCR_CANONICAL_ROOT_IDENTITY":":".join(map(str,cluster.identity)),
                "MCR_CANONICAL_ROOT_TOKEN":cluster.token,
                "MCR_CANONICAL_SYSTEM":cluster.sql("postgres","select system_identifier from pg_control_system();")}
            for owner in ("auth","friend","concert","profile"):
                db = "mcr_canonical_"+owner
                native.execute([str(pg_bin/"createdb"),"--no-password","--maintenance-db=postgres",db], env=cluster.client_env("postgres"), cwd=cluster.root, phase="create-"+owner)
                canonical["MCR_CANONICAL_"+owner.upper()+"_URL"] = cluster.url(db)
            for phase, case in [
                ("joined-owner-reader-hold","actual_owner_public_read_reuses_local_hold_through_serialization"),
                ("joined-local-denial","actual_local_denials_and_failed_profile_intent_settle_without_fallback")]:
                run(phase,"concert_service",["--ignored","--exact","adapters::out::public_profile::native::"+case],canonical)
            report["joined_migrations"] = {}
            for owner in ("auth","friend","concert","profile"):
                report["joined_migrations"][owner] = native.check_ledger(cluster,"mcr_canonical_"+owner,native.migration_manifest(ROOT/"backend",owner+"-service"))
            assert cluster.sql("postgres","select count(*) from pg_prepared_xacts;") == "0"
            report["limits"] = ["Real owner/reader core with selected avatar metadata; private cfg(test) constructor bypasses product release/art construction only", "Actual Gateway HTTP terminal callback and complete Feed/C04 route NOT_RUN", "No production capability or art fidelity claim"]
        report["result"] = "PASS"
    except BaseException as exc:
        report.update(result="FAIL", failure_type=type(exc).__name__)
    finally:
        if cluster:
            root = cluster.root
            try:
                if not cluster.started and (cluster.data/"PG_VERSION").is_file():
                    cluster.start()
                if cluster.started:
                    # Only this controller's private cluster and known read-only
                    # prepared children can be settled for failed-test cleanup.
                    rows = cluster.sql("postgres", "select database||E'\\t'||gid from pg_prepared_xacts;").splitlines()
                    report["cleanup_forced_private_rollbacks"] = len(rows)
                    # Exact private fixture teardown, never an owner success reply
                    # or a passing test. Unknown DB/GID retains quarantine.
                    for row in rows:
                        db, gid = row.split("\t")
                        allowed = (
                            db in {"mcr_profile_self_rpc", "mcr_canonical_profile"}
                            and re.fullmatch(r"pp:[A-Za-z0-9_-]{107}:[0-9a-f]{32}:[A-Za-z0-9_-]{43}", gid)) or (
                            report.get("joined") and db == "mcr_canonical_concert"
                            and re.fullmatch(r"mcr-public-avatar:[0-9a-f]{32}", gid)) or (
                            report.get("joined") and db in {"mcr_canonical_auth", "mcr_canonical_friend"}
                            and re.fullmatch("mcr_oa_"+("a_" if db.endswith("auth") else "f_")+r"[0-9a-f]{32}", gid))
                        assert allowed, "unknown prepared quarantine"
                        cluster.sql(db, "rollback prepared '"+gid+"';")
                    assert cluster.sql("postgres", "select count(*) from pg_prepared_xacts;") == "0"
                cluster.cleanup()
                report["cleanup"] = {"prepared_quarantine_resolved": True, "exact_owned_root_absent": not root.exists(), "owned_server_stopped": True}
            except BaseException as exc:
                report.update(result="FAIL", cleanup={"failure_type": type(exc).__name__, "owned_root_retained": True})
        else:
            report["cleanup"] = {"no_cluster_started": True}
        if not report.get("cleanup",{}).get("owned_root_retained"):
            (out/"private-restart-child.json").unlink(missing_ok=True)
        save()
    return report, out/"native-report.json"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--cargo-json", action="append", type=Path)
    group.add_argument("--pinned", type=Path)
    parser.add_argument("--joined", action="store_true", help="also freeze Concert lib tests for actual owner/reader native qualification")
    parser.add_argument("--profile-only", action="store_true", help="qualify only the owning Profile producer, never the full projection")
    parser.add_argument("--pg-bin", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    args = parser.parse_args()
    if args.cargo_json:
        print(json.dumps({"pinned": str(freeze(args.cargo_json, args.profile_only, args.joined))}))
        return 0
    report, path = qualify(args.pinned, args.pg_bin)
    print(json.dumps({"result": report["result"], "report": str(path), "cleanup": report["cleanup"]}))
    return 0 if report["result"] == "PASS" else 1
if __name__ == "__main__": raise SystemExit(main())
