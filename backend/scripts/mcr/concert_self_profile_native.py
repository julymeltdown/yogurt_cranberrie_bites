#!/usr/bin/env python3
"""Source-bound isolated Profile SQL/prepare mechanics; not guard/RPC acceptance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess

import migration_smoke as migration
from concert_core_smoke import command, digest, require

ROOT = Path(__file__).resolve().parents[3]
PREFIX = "concert_self_profile::native::native_"
DB = "mcr_profile_self"


def artifacts(receipt, frozen_receipt):
    frozen = json.loads(frozen_receipt.read_text())
    require(frozen["cargo_json_sha256"] == digest(receipt), "frozen-build-receipt-mismatch")
    selected = {}
    finished = 0
    manifest = ROOT / "backend/services/profile-service/Cargo.toml"
    for line in receipt.read_text().splitlines():
        row = json.loads(line)
        if row.get("reason") == "build-finished":
            require(row.get("success") is True, "failed-build-receipt")
            finished += 1
        if row.get("reason") != "compiler-artifact" or not row.get("executable"):
            continue
        target = row["target"]
        key = None
        if target["name"] == "concert_self_profile" and target["kind"] == ["test"] and row["profile"]["test"]:
            key = "test"
        if target["name"] == "profile-service" and target["kind"] == ["bin"] and not row["profile"]["test"]:
            key = "profile-bin"
        if key is None:
            continue
        require(Path(row["manifest_path"]).resolve() == manifest, "foreign-source-artifact")
        record = frozen["artifacts"][key]
        require(record["original"] == row["executable"], "frozen-artifact-origin-mismatch")
        path = migration.executable(Path(record["path"]))
        require(digest(path) == record["sha256"], "frozen-artifact-digest-mismatch")
        require(key not in selected or selected[key] == path, "ambiguous-artifact")
        selected[key] = path
    require(finished >= 2 and set(selected) == {"test", "profile-bin"}, "successful-artifacts-required")
    return selected


def exercise(cluster, pinned, report):
    listing = command(report, "list-native", [pinned["test"], "--list"], cluster.env, cluster.root)
    names = re.findall(r"^(" + re.escape(PREFIX) + r"[a-z0-9_]+): test$", listing.stdout.decode(), re.MULTILINE)
    require(len(names) == 3, "exact-native-tests-required")
    missing = next(name for name in names if name.endswith("native_versions_cover_retained_updates_and_tombstone_fences"))
    command(report, "missing-owner-refused", [pinned["test"], "--ignored", "--exact", missing], cluster.env, cluster.root, success=False)
    cluster.init()
    cluster.assert_owned()
    with (cluster.data / "postgresql.conf").open("a") as stream:
        stream.write("\nmax_prepared_transactions = 16\n")
    cluster.start()
    require(cluster.sql("postgres", "show server_version_num;") == "160014", "postgres-16.14-required")
    cluster.sql("postgres", f"create database {DB} template template0;")
    url = cluster.url(DB)
    secrets = (cluster.password, cluster.token, url)
    service_env = {**cluster.env, "PROFILE_DATABASE_URL": url}
    for label, args, success, env in [
        ("missing-explicit-db", ["--migrate"], False, cluster.env),
        ("verify-empty-refused", ["--verify-migrations"], False, service_env),
        ("migrate", ["--migrate"], True, service_env),
        ("verify", ["--verify-migrations"], True, service_env),
        ("reapply", ["--migrate"], True, service_env),
    ]:
        command(report, label, [pinned["profile-bin"], *args], env, cluster.root, success=success, secrets=secrets)
    manifest = migration.migration_manifest(ROOT / "backend", "profile-service")
    require(len(manifest) == 8 and migration.check_ledger(cluster, DB, manifest) == 8, "exact-eight-migrations-required")
    report["migration_sha384"] = manifest
    env = {**cluster.env, "MCR_SELF_PROFILE_ROOT": str(cluster.root), "MCR_SELF_PROFILE_TOKEN": cluster.token,
           "MCR_SELF_PROFILE_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}", "MCR_SELF_PROFILE_URL": url}
    def run(suffix):
        name = next(name for name in names if name.endswith(suffix))
        result = subprocess.run([str(pinned["test"]), "--ignored", "--exact", name, "--test-threads=1"], env=env, cwd=cluster.root, capture_output=True, timeout=30)
        report["commands"][suffix] = result.returncode
        raw = result.stdout + result.stderr
        require(all(secret.encode() not in raw for secret in secrets), "native-redaction")
        report.setdefault("native_diagnostics", {})[suffix] = migration.redact(raw.decode(errors="replace"), secrets)
        require(result.returncode == 0, "native-case-failed:" + suffix)
        require(re.findall(rb"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", result.stdout, re.MULTILINE) == [(b"1", b"0", b"0")], "native-case-missing-or-skipped")
    run("native_versions_cover_retained_updates_and_tombstone_fences")
    run("native_prepare_preserves_visibility_and_blocks_legacy_writes")
    require(cluster.sql(DB, "select count(*) from pg_prepared_xacts where database=current_database();") == "2", "two-prepared-effects-required")
    require(cluster.sql(DB, "select count(*) from concert_self_profile_receipts;") == "0", "uncommitted-receipts-must-be-invisible")
    old = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(cluster.assert_postmaster() != old, "new-postmaster-required")
    require(cluster.sql(DB, "select count(*) from pg_prepared_xacts where database=current_database();") == "2", "prepared-effects-must-survive-restart")
    run("native_after_restart_resolves_actual_commit_and_abort_receipts")
    require(cluster.sql(DB, "select count(*) from pg_prepared_xacts where database=current_database();") == "0", "prepared-effects-must-resolve")
    require(cluster.sql(DB, "select count(*) from concert_self_profile_receipts;") == "1", "only-actual-commit-has-receipt")
    migration.check_ledger(cluster, DB, manifest)
    command(report, "verify-after-restart", [pinned["profile-bin"], "--verify-migrations"], service_env, cluster.root, secrets=secrets)
    report.update(native_tests=3, restart_persistence="PASS", result="PASS")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cargo-json", "frozen-artifacts", "pg-bin", "artifact-dir", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    require(all(p.is_absolute() for p in vars(args).values()), "absolute-inputs-required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"scope": "Profile SELF local owning SQL/native transaction mechanics", "result": "FAIL", "commands": {},
              "actual_auth_friend_guard_and_cross_store_coordinator_rpc": "NOT_RUN"}
    cluster = None
    previous = {}
    def interrupt(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupt)
        require(shutil.disk_usage(ROOT).free >= 8 * 1024 ** 3, "eight-gibibyte-floor-required")
        selected = artifacts(args.cargo_json, args.frozen_artifacts)
        report["frozen_artifacts_sha256"] = digest(args.frozen_artifacts)
        args.artifact_dir.mkdir(mode=0o700, exist_ok=False)
        pinned = {}
        report["artifact_sha256"] = {key: digest(path) for key, path in selected.items()}
        for key, path in selected.items():
            dest = args.artifact_dir / key
            shutil.copyfile(path, dest)
            dest.chmod(0o700)
            require(digest(dest) == report["artifact_sha256"][key], "copied-artifact-mismatch")
            pinned[key] = dest
        report["cargo_json_sha256"] = digest(args.cargo_json)
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["dirty_files"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()
        sources = [p for p in (ROOT / "backend").rglob("*") if p.is_file() and p.suffix in (".rs", ".sql", ".toml", ".proto")]
        sources += [ROOT / "backend/Cargo.lock", Path(__file__), Path(migration.__file__), ROOT / "backend/scripts/mcr/concert_core_smoke.py"]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sources}
        cluster = migration.Cluster({name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS})
        report["owned_directory"] = str(cluster.root)
        exercise(cluster, pinned, report)
    except (Exception, KeyboardInterrupt) as error:
        report.update(result="FAIL", failure=str(error) if type(error).__name__ == "CheckFailure" else type(error).__name__)
    finally:
        try:
            if cluster is not None:
                cluster.cleanup()
                require(not cluster.root.exists(), "owned-root-remains")
            report["cleanup"] = "PASS"
        except BaseException:
            report.update(result="FAIL", cleanup="FAIL")
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({key: report.get(key) for key in ("result", "failure", "native_tests", "restart_persistence", "cleanup")}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
