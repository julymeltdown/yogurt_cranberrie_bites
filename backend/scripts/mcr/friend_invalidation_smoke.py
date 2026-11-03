#!/usr/bin/env python3
"""C05 Friend journal native upgrade/replay qualification in a new private PG cluster."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_friend_invalidation"
NATIVE = (
    "native_upgrade_0015_to_0016",
    "native_repository_block_unblock_and_deletion",
    "native_direct_sql_scopes_guards_and_rollback",
    "native_concurrent_commit_and_rollback_never_skip_late_commit",
    "native_page_bounds_echo_epoch_and_future_rejected",
    "native_cancel_and_deadline_are_failures_not_empty_success",
    "native_corrupt_retained_gap_fails_closed",
)

class CheckFailure(Exception):
    pass

def require(value, label):
    if not value:
        raise CheckFailure(label)

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def frozen_executable(path):
    require(path.is_absolute(), "explicit-absolute-executable-required")
    resolved = migration.executable(path)
    require("target" not in resolved.parts, "copy-executables-outside-target-first")
    return resolved

def command(report, label, argv, env, cluster):
    cluster.assert_owned()
    result = subprocess.run([str(x) for x in argv], cwd=cluster.root, env=env,
                            capture_output=True, timeout=60)
    report["commands"][label] = result.returncode
    for secret in (cluster.password, cluster.url(DATABASE)):
        require(secret.encode() not in result.stdout + result.stderr, label + ":redaction")
    # Keep bounded sanitized diagnostics for reproducibility, never raw URL/env.
    if result.returncode:
        report["diagnostic"] = migration.redact((result.stdout + result.stderr).decode(errors="replace"),
                                                (cluster.password, cluster.url(DATABASE)))
    require(result.returncode == 0, label + ":exit")
    return result.stdout

def snapshot(cluster):
    # Complete ordered identity/content digest, not merely an mtime or row count.
    return cluster.sql(DATABASE, "SELECT md5(coalesce(string_agg(row_to_json(j)::text,E'\\n' ORDER BY sequence),'')) FROM friend_world_invalidation_journal j;")

def exercise(cluster, binary, test, relationship_test, report):
    cluster.init()
    cluster.start()
    require(cluster.sql("postgres", "show server_version_num;") == "160014", "postgres-16.14-required")
    report["postgres_version"] = "16.14"
    cluster.sql("postgres", "CREATE DATABASE mcr_friend_invalidation TEMPLATE template0;")
    url = cluster.url(DATABASE)
    env = {**cluster.env, "FRIEND_DATABASE_URL": url,
           "MCR_FRIEND_INVALIDATION_URL": url,
           "MCR_FRIEND_INVALIDATION_ROOT": str(cluster.root),
           "MCR_FRIEND_INVALIDATION_TOKEN": cluster.token,
           "MCR_FRIEND_INVALIDATION_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    # The first exact native test applies the embedded original fifteen SQLx
    # migrations, seeds real repository state, and upgrades with unchanged receipts.
    for name in NATIVE:
        output = command(report, name, [test, "--ignored", "--exact", name, "--test-threads=1"], env, cluster)
        require(b"1 passed; 0 failed" in output, name + ":not-skipped")
    report["native_tests"] = len(NATIVE)
    manifest = migration.migration_manifest(ROOT / "backend", "friend-service")
    report["migration_sha384"] = manifest
    require(len(manifest) == 16, "exact-migration-count")
    report["migration_ledger"] = migration.check_ledger(cluster, DATABASE, manifest)
    command(report, "cli-verify", [binary, "--verify-migrations"], env, cluster)
    command(report, "cli-reapply", [binary, "--migrate"], env, cluster)
    require(migration.check_ledger(cluster, DATABASE, manifest) == report["migration_ledger"], "stable-reapply-ledger")
    before = snapshot(cluster)
    before_head = cluster.sql(DATABASE, "SELECT owner_epoch::text || ':' || sequence::text FROM friend_world_invalidation_head;")
    before_revision = cluster.sql(DATABASE, "SELECT owner_epoch::text || ':' || revision::text FROM friend_concert_relationship_revision;")
    cluster.stop()
    cluster.start()
    require(snapshot(cluster) == before, "restart-signal-identities-and-contents")
    require(cluster.sql(DATABASE, "SELECT owner_epoch::text || ':' || sequence::text FROM friend_world_invalidation_head;") == before_head, "restart-head-identity")
    require(cluster.sql(DATABASE, "SELECT owner_epoch::text || ':' || revision::text FROM friend_concert_relationship_revision;") == before_revision, "restart-revision-identity")
    command(report, "cli-verify-after-restart", [binary, "--verify-migrations"], env, cluster)
    output = command(report, "native-restart-replay", [test, "--ignored", "--exact", "native_restart_replay_stability", "--test-threads=1"], env, cluster)
    require(b"1 passed; 0 failed" in output, "restart-test-not-skipped")
    report["native_tests"] += 1
    # Retain the existing 0015 observation/revision behavior on a separately
    # named database in the same owned cluster, using the unchanged native test.
    cluster.sql("postgres", "CREATE DATABASE mcr_friend_relationship TEMPLATE template0;")
    relationship_url = cluster.url("mcr_friend_relationship")
    command(report, "retained-owner-migrate", [binary, "--migrate"], {**cluster.env, "FRIEND_DATABASE_URL": relationship_url}, cluster)
    relationship_env = {**cluster.env, "MCR_RELATIONSHIP_FRIEND_URL": relationship_url,
                        "MCR_RELATIONSHIP_ROOT": str(cluster.root),
                        "MCR_RELATIONSHIP_TOKEN": cluster.token,
                        "MCR_RELATIONSHIP_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    output = command(report, "retained-native-owner", [relationship_test, "--ignored", "--exact", "native_relationship_owner_snapshot_and_revisions", "--test-threads=1"], relationship_env, cluster)
    require(b"1 passed; 0 failed" in output, "retained-native-not-skipped")
    report["retained_native_tests"] = 1
    report["restart_persistence"] = "PASS"
    report["ordered_journal_md5"] = before
    report["result"] = "PASS"

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repository-test", type=Path, required=True)
    parser.add_argument("--relationship-test", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("report must be a new absolute file")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    report = {"scope": "C05-FRIEND-INVALIDATION owner-local journal", "result": "FAIL", "commands": {},
              "rpc_startup": "NOT_EXPOSED", "consumer_fence_checkpoint": "NOT_RUN", "IN01": "NOT_RUN",
              "production": "NOT_RUN", "secrets_recorded": False}
    cluster = None
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        binary, test = frozen_executable(args.binary), frozen_executable(args.repository_test)
        relationship_test = frozen_executable(args.relationship_test)
        require(args.pg_bin_dir.is_absolute(), "explicit-pg-tools-required")
        tools = {name: migration.executable(args.pg_bin_dir / name) for name in migration.PG_TOOLS}
        report["binary_sha256"] = digest(binary)
        report["repository_test_sha256"] = digest(test)
        report["relationship_test_sha256"] = digest(relationship_test)
        report["script_sha256"] = digest(__file__)
        report["migration_helper_sha256"] = digest(migration.__file__)
        prefixes = ("backend/services/friend-service", "backend/crates/friend-application", "backend/crates/friend-domain")
        files = [p for prefix in prefixes for p in (ROOT / prefix).rglob("*") if p.suffix in (".rs", ".sql", ".toml")]
        files += [ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock", ROOT / "backend/proto/relationship/v1/relationship.proto", ROOT / "docs/mcr/world-invalidation-contract.md"]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(files)}
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        cluster = migration.Cluster(tools)
        report["owned_directory"] = str(cluster.root)
        exercise(cluster, binary, test, relationship_test, report)
    except (Exception, KeyboardInterrupt) as error:
        report["result"] = "FAIL"
        report["failure"] = str(error) if isinstance(error, CheckFailure) else type(error).__name__
    finally:
        try:
            if cluster is not None:
                cluster.cleanup()
                require(not cluster.root.exists(), "owned-directory-not-removed")
            report["cleanup"] = "PASS"
        except BaseException:
            report.update(cleanup="FAIL", result="FAIL")
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({k: report.get(k) for k in ("result", "cleanup", "native_tests", "restart_persistence", "failure")}))
    return 0 if report["result"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
