#!/usr/bin/env python3
"""Qualify copied Friend replay RPC artifacts in a new owned socket-only PG16.14 cluster."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration
from friend_invalidation_smoke import frozen_executable

ROOT = Path(__file__).resolve().parents[3]
REQUIRED = {"friend-service", "concert_invalidation_rpc", "concert_relationship", "concert_invalidation", "unit"}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def sources():
    prefixes = ("backend/services/friend-service", "backend/crates/friend-domain",
                "backend/crates/friend-application", "backend/crates/platform")
    paths = [p for prefix in prefixes for p in (ROOT / prefix).rglob("*")
             if p.suffix in (".rs", ".sql", ".toml")]
    paths += [ROOT / p for p in ("backend/Cargo.toml", "backend/Cargo.lock",
              "backend/proto/relationship/v1/relationship.proto", "docs/mcr/world-invalidation-contract.md",
              "backend/scripts/mcr/friend_invalidation_rpc_smoke.py", "backend/scripts/mcr/friend_invalidation_smoke.py",
              "backend/scripts/mcr/migration_smoke.py")]
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}


def binaries(metadata):
    if not metadata.is_absolute() or metadata.is_symlink():
        raise migration.SmokeError("preflight", "absolute regular frozen metadata required")
    data = json.loads(metadata.read_text())
    if set(data["artifacts"]) != REQUIRED:
        raise migration.SmokeError("preflight", "exact Friend RPC/journal/relationship/unit/service artifacts required")
    current = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()
    if data.get("source_commit") != current or dirty or data.get("source_sha256") != sources():
        raise migration.SmokeError("preflight", "frozen source checkpoint mismatch")
    result = {}
    for name, item in data["artifacts"].items():
        path = frozen_executable(Path(item["path"]))
        if digest(path) != item["sha256"]:
            raise migration.SmokeError("preflight", "frozen binary hash mismatch")
        result[name] = path
    return result


def test(binary, env, report, phase, passed, exact=None):
    args = [str(binary), "--test-threads=1"]
    if exact:
        args += ["--ignored", "--exact", exact]
    result = migration.execute(args, env=env, phase=phase, timeout=60, cwd=ROOT / "backend")
    if f"test result: ok. {passed} passed; 0 failed;" not in result.stdout:
        raise migration.SmokeError(phase, "exact test inventory did not pass")
    report[phase] = {"status": "PASS", "exit_code": result.returncode, "passed": passed,
                     "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest()}


def qualify(artifacts, pg, report):
    tools = {name: migration.executable(pg / name) for name in migration.PG_TOOLS}
    cluster = migration.Cluster(tools)
    report["owned_root"] = str(cluster.root)
    try:
        cluster.init()
        cluster.start()
        version = cluster.sql("postgres", "show server_version_num;")
        if version != "160014":
            raise migration.SmokeError("preflight", "PostgreSQL 16.14 required")
        report["postgres_version"] = version
        for db in ("mcr_friend_invalidation_rpc", "mcr_friend_relationship"):
            migration.execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db],
                              env=cluster.client_env("postgres"), phase="createdb", cwd=cluster.root)
        common = {**cluster.env, "MCR_INVALIDATION_RPC_ROOT": str(cluster.root),
                  "MCR_INVALIDATION_RPC_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
                  "MCR_INVALIDATION_RPC_TOKEN": cluster.token,
                  "MCR_INVALIDATION_RPC_DATABASE_URL": cluster.url("mcr_friend_invalidation_rpc"),
                  "MCR_INVALIDATION_RPC_SYSTEM_IDENTIFIER": cluster.sql("postgres", "select system_identifier from pg_control_system();")}
        test(artifacts["concert_invalidation_rpc"], cluster.env, report, "pure_rpc", 4)
        test(artifacts["concert_relationship"], cluster.env, report, "relationship_guard_regression", 2)
        test(artifacts["concert_invalidation"], cluster.env, report, "journal_shape_regression", 1)
        test(artifacts["unit"], cluster.env, report, "friend_units", 87)
        test(artifacts["concert_invalidation_rpc"], common, report, "native_tls_mutations", 1,
             "native::owning_mutations_replay_over_guarded_mutual_tls")
        test(artifacts["concert_invalidation_rpc"], common, report, "native_deadline_cancellation", 1,
             "native::deadline_includes_pool_sql_and_cancellation_releases_work")
        env = {**cluster.env, "FRIEND_DATABASE_URL": cluster.url("mcr_friend_relationship")}
        for command in ("--migrate", "--verify-migrations"):
            migration.execute([str(artifacts["friend-service"]), command], env=env,
                              phase="relationship-" + command, cwd=cluster.root)
        env.update(MCR_RELATIONSHIP_ROOT=str(cluster.root),
                   MCR_RELATIONSHIP_IDENTITY=f"{cluster.identity[0]}:{cluster.identity[1]}",
                   MCR_RELATIONSHIP_TOKEN=cluster.token,
                   MCR_RELATIONSHIP_FRIEND_URL=cluster.url("mcr_friend_relationship"))
        test(artifacts["concert_relationship"], env, report, "native_relationship_regression", 1,
             "native_relationship_owner_snapshot_and_revisions")
        manifest = migration.migration_manifest(ROOT / "backend", "friend-service")
        report["migration_sha384"] = manifest
        for db in ("mcr_friend_invalidation_rpc", "mcr_friend_relationship"):
            migration.check_ledger(cluster, db, manifest)
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise migration.SmokeError("restart", "postmaster identity did not change")
        report["restart"] = {"status": "PASS", "before_pid": before, "after_pid": after}
        test(artifacts["concert_invalidation_rpc"], common, report, "native_restart", 1,
             "native::restart_replays_same_epoch_positions_and_signal_identity")
        for db in ("mcr_friend_invalidation_rpc", "mcr_friend_relationship"):
            migration.check_ledger(cluster, db, manifest)
    finally:
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS", "owned_root_absent": not cluster.root.exists()}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binaries-json", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute() or not args.pg_bin_dir.is_absolute():
        parser.error("new absolute report and explicit PG binary directory required")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    report = {"status": "FAIL", "limitations": ["No deployed credentials or production World consumer acceptance.",
              "No IN01 or consumer checkpoint/fence qualification.",
              "Retained Docker-backed contract cases remain NOT_RUN; no build lease is implied."]}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        artifacts = binaries(args.binaries_json)
        report.update(source_commit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                      source_clean=True, source_sha256=sources(), metadata_sha256=digest(args.binaries_json),
                      binaries_sha256={name: digest(path) for name, path in artifacts.items()})
        qualify(artifacts, args.pg_bin_dir, report)
        report["status"] = "PASS"
    except migration.SmokeError as error:
        report["failure"] = error.detail
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = {"phase": "runner", "diagnostic": type(error).__name__}
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as out:
            json.dump(report, out, indent=2, sort_keys=True)
            out.write("\n")
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
