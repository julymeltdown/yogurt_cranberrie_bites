#!/usr/bin/env python3
"""Bounded Auth/Friend observations and Concert clients in owned PG16.14 only."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import signal
import shutil
import tempfile
import subprocess
import migration_smoke as migration
from concert_core_smoke import command, digest, require, ROOT

DATABASES = {owner: f"mcr_{owner}_relationship" for owner in ("auth", "friend")}
ARTIFACTS = {"auth-bin", "friend-bin", "auth-test", "friend-test", "concert-test"}
NATIVE = {"auth": "native_relationship_owner_snapshot_and_revisions",
          "friend": "native_relationship_owner_snapshot_and_revisions",
          "concert": "native_two_owner_client_rechecks_revocation_and_block_generations"}


def build_artifacts(metadata):
    require(metadata.is_absolute(), "absolute-cargo-json-required")
    selected = {}
    finished = 0
    for line in metadata.read_text().splitlines():
        row = json.loads(line)
        if row.get("reason") == "build-finished":
            require(row.get("success") is True, "successful-cargo-build-required")
            finished += 1
        if row.get("reason") != "compiler-artifact" or not row.get("executable"):
            continue
        manifest = Path(row["manifest_path"]).resolve()
        target = row["target"]
        key = None
        for owner in ("auth", "friend", "concert"):
            if manifest != ROOT / f"backend/services/{owner}-service/Cargo.toml":
                continue
            if owner != "concert" and target["name"] == f"{owner}-service" and "bin" in target["kind"] and not row["profile"]["test"]:
                key = owner + "-bin"
            if "test" in target["kind"] and target["name"] == ("relationship_authority" if owner == "concert" else "concert_relationship"):
                key = owner + "-test"
        if key:
            path = Path(row["executable"])
            require(path.is_absolute(), "absolute-executable-required")
            require(key not in selected or selected[key] == path, "ambiguous-cargo-artifact")
            selected[key] = migration.executable(path)
    require(finished > 0 and set(selected) == ARTIFACTS, "exact-successful-cargo-artifacts-required")
    return selected


def exercise(cluster, artifacts, report):
    cluster.init()
    cluster.start()
    require(cluster.sql("postgres", "show server_version_num;") == "160014", "postgres-16.14-required")
    urls = {owner: cluster.url(db) for owner, db in DATABASES.items()}
    secrets = (cluster.password, cluster.token, *urls.values())
    manifests = {}
    for owner, db in DATABASES.items():
        cluster.sql("postgres", f"create database {db} template template0;")
        env = {**cluster.env, owner.upper() + "_DATABASE_URL": urls[owner]}
        binary = artifacts[owner + "-bin"]
        for label, args, success, override in [
            ("missing-explicit-database", ["--migrate"], False, cluster.env),
            ("verify-empty-refused", ["--verify-migrations"], False, env),
            ("migrate", ["--migrate"], True, env),
            ("verify", ["--verify-migrations"], True, env),
            ("migrate-reapply", ["--migrate"], True, env),
            ("verify-reapply", ["--verify-migrations"], True, env),
        ]:
            command(report, owner + "-" + label, [binary, *args], override, cluster.root, success=success, secrets=secrets)
        manifests[owner] = migration.migration_manifest(ROOT / "backend", owner + "-service")
        count = 14 if owner == "auth" else 15
        require(set(manifests[owner]) == set(range(1, count + 1)), "exact-owner-migration-set")
        require(migration.check_ledger(cluster, db, manifests[owner]) == count, "exact-owner-ledger")
    report["migration_sha384"] = manifests
    env = {**cluster.env, "MCR_RELATIONSHIP_ROOT": str(cluster.root), "MCR_RELATIONSHIP_TOKEN": cluster.token,
           "MCR_RELATIONSHIP_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
           **{f"MCR_RELATIONSHIP_{owner.upper()}_URL": url for owner, url in urls.items()}}
    for owner, name in NATIVE.items():
        argv = [artifacts[owner + "-test"], "--ignored", "--exact", name, "--test-threads=1"]
        command(report, owner + "-missing-ownership-refused", argv, cluster.env, cluster.root, success=False, secrets=secrets)
        result = command(report, owner + "-native", argv, env, cluster.root, secrets=secrets)
        require(re.search(rb"test result: ok\. 1 passed; 0 failed; 0 ignored;", result.stdout) is not None,
                owner + "-native-test-missing-or-skipped")
    report["native_tests"] = 3
    state_sql = {
        "auth": "select owner_epoch::text||':'||revision::text||':'||(select count(*) from users where status='SUSPENDED')::text||':'||(select count(*) from refresh_tokens where revoked_at is not null)::text from auth_concert_relationship_revision;",
        "friend": "select owner_epoch::text||':'||revision::text||':'||(select count(*) from user_blocks)::text||':'||(select count(*) from friend_user_deletion_tombstones)::text from friend_concert_relationship_revision;",
    }
    before = {owner: cluster.sql(DATABASES[owner], sql) for owner, sql in state_sql.items()}
    for owner, row in before.items():
        fields = row.split(":")
        require(len(fields) == 4 and int(fields[1]) > 1 and int(fields[2]) > 0 and int(fields[3]) > 0, owner + "-restart-fixtures-required")
    postmaster = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(cluster.assert_postmaster() != postmaster, "new-postmaster-required")
    for owner, sql in state_sql.items():
        require(cluster.sql(DATABASES[owner], sql) == before[owner], owner + "-restart-state-preserved")
        migration.check_ledger(cluster, DATABASES[owner], manifests[owner])
        command(report, owner + "-verify-after-restart", [artifacts[owner + "-bin"], "--verify-migrations"],
                {**cluster.env, owner.upper() + "_DATABASE_URL": urls[owner]}, cluster.root, secrets=secrets)
    report.update(restart_persistence="PASS", result="PASS")


def copy_artifacts(artifacts, directory, hashes):
    pinned = {}
    for key, source in artifacts.items():
        target = directory / key
        shutil.copyfile(source, target)
        target.chmod(0o700)
        require(digest(target) == hashes[key], "pinned-binary-hash-mismatch")
        pinned[key] = target
    return pinned


def run(metadata, pg_bin, report):
    cluster = None
    try:
        require(metadata.is_absolute() and pg_bin.is_absolute(), "explicit-absolute-inputs")
        artifacts = build_artifacts(metadata)
        report["binary_sha256"] = {key: digest(path) for key, path in artifacts.items()}
        report["cargo_json_sha256"] = digest(metadata)
        tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        report["owned_directory"] = str(cluster.root)
        with tempfile.TemporaryDirectory(prefix="mcr-owner-relationship-binaries-") as folder:
            pinned = copy_artifacts(artifacts, Path(folder), report["binary_sha256"])
            print("OWNER_RELATIONSHIP_ARTIFACTS_PINNED", flush=True)
            exercise(cluster, pinned, report)
    except (Exception, KeyboardInterrupt) as error:
        report.update(result="FAIL", failure=str(error) if type(error).__name__ == "CheckFailure" else type(error).__name__)
    finally:
        try:
            if cluster is not None:
                cluster.cleanup()
                require(not cluster.root.exists(), "owned-directory-remains")
            report["cleanup"] = "PASS"
        except BaseException:
            report.update(result="FAIL", cleanup="FAIL")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-json", type=Path, required=True)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("report must be a new absolute file")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be new and writable")
    report = {"scope": "BE37 bounded owner observations; no distributed fence", "result": "FAIL", "commands": {},
              "private_reveal_fence": "NOT_RUN", "production_tls_deployment": "NOT_RUN", "multiuser_flow": "NOT_RUN"}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        report["script_sha256"] = digest(__file__)
        report["helper_sha256"] = {p.name: digest(p) for p in (Path(migration.__file__), ROOT / "backend/scripts/mcr/concert_core_smoke.py")}
        paths = [p for prefix in ("backend/crates/auth-application", "backend/crates/friend-application", "backend/services/auth-service", "backend/services/friend-service", "backend/services/concert-service", "backend/proto/relationship") for p in (ROOT / prefix).rglob("*") if p.suffix in (".rs", ".sql", ".toml", ".proto")]
        paths += [ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock"]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["dirty_files"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()
        run(args.cargo_json, args.pg_bin, report)
    except (Exception, KeyboardInterrupt) as error:
        report.update(result="FAIL", failure=type(error).__name__, cleanup="NOT_RUN")
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({k: report.get(k) for k in ("result", "failure", "cleanup", "native_tests", "restart_persistence")}))
    return 0 if report["result"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
