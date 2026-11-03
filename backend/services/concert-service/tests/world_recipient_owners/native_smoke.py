#!/usr/bin/env python3
# Adapted from world_auth_read/native_smoke.py at source revision 1b7caec.
"""Source-bound World recipient read against marker/inode-owned native PostgreSQL."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration
from concert_core_smoke import command, digest, require

PREFIX = "world::recipient_owners::tests::native_"
DATABASES = {owner: f"mcr_{owner}_recipient" for owner in ("auth", "friend", "concert")}


def artifacts(receipt: Path):
    selected = {}
    finished = 0
    for line in receipt.read_text().splitlines():
        row = json.loads(line)
        if row.get("reason") == "build-finished":
            require(row.get("success") is True, "failed-build-receipt")
            finished += 1
        if row.get("reason") != "compiler-artifact" or not row.get("executable"):
            continue
        target = row["target"]
        key = None
        if target["name"] == "concert_service" and target["kind"] == ["lib"] and row["profile"]["test"]:
            key, owner = "test", "concert"
        elif target["name"] in ("auth-service", "friend-service", "concert-service") and target["kind"] == ["bin"] and not row["profile"]["test"]:
            owner = target["name"].removesuffix("-service")
            key = owner + "-bin"
        if key is None:
            continue
        manifest = ROOT / f"backend/services/{owner}-service/Cargo.toml"
        require(Path(row["manifest_path"]).resolve() == manifest, "foreign-source-artifact")
        path = migration.executable(Path(row["executable"]))
        require(key not in selected or selected[key] == path, "ambiguous-cargo-artifact")
        selected[key] = path
    require(finished >= 2 and set(selected) == {"test", "auth-bin", "friend-bin", "concert-bin"}, "successful-artifacts-required")
    return selected


def summary(raw: bytes, expected: int):
    found = re.findall(rb"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", raw, re.MULTILINE)
    require(found == [(str(expected).encode(), b"0", b"0")], "native-tests-missing-or-skipped")


def exercise(cluster, pinned, report):
    env = cluster.env
    listing = command(report, "list-native", [pinned["test"], "--list"], env, cluster.root)
    names = re.findall(r"^(" + re.escape(PREFIX) + r"[a-z0-9_]+): test$", listing.stdout.decode(), re.MULTILINE)
    require(len(names) == 1 and len(set(names)) == 1, "exact-native-tests-required")
    command(report, "missing-owner-refused", [pinned["test"], "--ignored", "--exact", names[0]], env, cluster.root, success=False)
    cluster.init()
    cluster.start()
    require(cluster.sql("postgres", "show server_version_num;") == "160014", "postgres-16.14-required")
    urls = {owner: cluster.url(db) for owner, db in DATABASES.items()}
    secrets = (cluster.password, cluster.token, *urls.values())
    manifests = {}
    for owner, db in DATABASES.items():
        cluster.sql("postgres", f"create database {db} template template0;")
        service_env = {**env, owner.upper() + "_DATABASE_URL": urls[owner]}
        for label, args, success, configured in [
            ("missing-explicit-database", ["--migrate"], False, env),
            ("verify-empty-refused", ["--verify-migrations"], False, service_env),
            ("migrate", ["--migrate"], True, service_env),
            ("verify", ["--verify-migrations"], True, service_env),
            ("reapply", ["--migrate"], True, service_env),
        ]:
            command(report, owner + "-" + label, [pinned[owner + "-bin"], *args], configured, cluster.root, success=success, secrets=secrets)
        manifests[owner] = migration.migration_manifest(ROOT / "backend", owner + "-service")
        require(migration.check_ledger(cluster, db, manifests[owner]) == len(manifests[owner]), "exact-migration-ledger")
    report["migration_sha384"] = manifests
    fixture_env = {**env, "MCR_RECIPIENT_ROOT": str(cluster.root), "MCR_RECIPIENT_TOKEN": cluster.token,
                   "MCR_RECIPIENT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
                   **{f"MCR_RECIPIENT_{owner.upper()}_URL": url for owner, url in urls.items()}}
    # Preserve redacted native failure evidence before asserting success; the
    # shared command helper intentionally records only exit codes.
    argv = [str(pinned["test"]), PREFIX, "--ignored", "--test-threads=1"]
    result = subprocess.run(argv, env=fixture_env, cwd=cluster.root, capture_output=True, timeout=30)
    report["commands"]["native-recipient"] = result.returncode
    raw = result.stdout + result.stderr
    require(all(value.encode() not in raw for value in secrets if value), "native-output-redaction")
    report["native_diagnostics"] = migration.redact(raw.decode(errors="replace"), secrets)
    require(result.returncode == 0, "native-recipient:exit")
    summary(result.stdout, len(names))
    report.update(native_test_names=names, native_tests=len(names))
    state_sql = {
        "friend": "select owner_epoch::text||':'||revision::text from friend_concert_relationship_revision;",
        "auth": "select owner_epoch::text||':'||revision::text||':'||(select count(*) from refresh_tokens where revoked_at is not null)::text from auth_concert_relationship_revision;",
        "concert": "select (select count(*) from concert_world_tickets where consumed_at is not null)::text||':'||(select count(*) from concert_world_memberships)::text;",
    }
    before = {owner: cluster.sql(DATABASES[owner], sql) for owner, sql in state_sql.items()}
    require(int(before["auth"].split(":")[-1]) > 0, "persisted-session-revocation-required")
    require(before["concert"] == "2:2", "two-consumes-current-memberships-required")
    old = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(cluster.assert_postmaster() != old, "new-postmaster-required")
    for owner, sql in state_sql.items():
        require(cluster.sql(DATABASES[owner], sql) == before[owner], "restart-owner-state-changed")
        migration.check_ledger(cluster, DATABASES[owner], manifests[owner])
        command(report, owner + "-verify-after-restart", [pinned[owner + "-bin"], "--verify-migrations"],
                {**env, owner.upper() + "_DATABASE_URL": urls[owner]}, cluster.root, secrets=secrets)
    report.update(restart_persistence="PASS", result="PASS")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-json", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    require(all(p.is_absolute() for p in vars(args).values()), "absolute-inputs-required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"scope": "BE13/BE14 sealed Presence read, independent owners and local policy", "result": "FAIL", "commands": {},
              "distributed_fence_multiuser_product_composition": "NOT_RUN"}
    cluster = None
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        require(shutil.disk_usage(ROOT).free >= 8 * 1024 ** 3, "eight-gibibyte-floor-required")
        selected = artifacts(args.cargo_json)
        args.artifact_dir.mkdir(mode=0o700, exist_ok=False)
        pinned = {}
        report["artifact_sha256"] = {key: digest(path) for key, path in selected.items()}
        for key, source in selected.items():
            dest = args.artifact_dir / key
            shutil.copyfile(source, dest)
            dest.chmod(0o700)
            require(digest(dest) == report["artifact_sha256"][key], "copied-artifact-mismatch")
            pinned[key] = dest
        report["pinned_artifacts"] = {key: str(path) for key, path in pinned.items()}
        report["cargo_json_sha256"] = digest(args.cargo_json)
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["dirty_files"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()
        sources = [p for p in (ROOT / "backend").rglob("*") if p.is_file() and p.suffix in (".rs", ".sql", ".toml", ".proto")]
        sources += [ROOT / "backend/Cargo.lock", Path(__file__), Path(migration.__file__), ROOT / "backend/scripts/mcr/concert_core_smoke.py"]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sources}
        tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        report["owned_directory"] = str(cluster.root)
        print("WORLD_RECIPIENT_ARTIFACTS_PINNED", flush=True)
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
