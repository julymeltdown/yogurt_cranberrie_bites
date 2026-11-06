#!/usr/bin/env python3
"""Prove VENU-BE L/M/N migrations and tests on a disposable owned PostgreSQL 16 cluster.

The run is deliberately self-contained:

* a fresh 0700 temp root, marker/inode-owned, is the only cluster this process
  will start; it listens on a private Unix socket and never on TCP;
* the venue ledger is created from an empty database by applying exactly
  ``0001_concert_core.sql`` followed by ``0073``, ``0074`` and ``0075`` in
  order through the service's SQLx migrator;
* the full committed migration set is also applied to a separate database;
* the cluster is physically restarted and the ledger, sentinel venue rows and
  ``--verify-migrations`` are checked again;
* a dropped and a mutated migration are supplied to ``--verify-migrations``
  and the verifier must exit non-zero for both;
* the venue integration tests run against the owned databases.

Nothing outside ``BACKEND`` and the marker-owned temporary root is written.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from migration_smoke import (  # noqa: E402 - intentional sibling import
    BACKEND,
    Cluster,
    SmokeError,
    check_ledger,
    execute,
)

MIGRATIONS = BACKEND / "services" / "concert-service" / "migrations"
VENUE_MIGRATIONS = (
    "0001_concert_core.sql",
    "0073_venue_location.sql",
    "0074_venue_invitations.sql",
    "0075_place_notes.sql",
)
LOCATION_DATABASE = "mcr_concert_venue_location"
MEETING_STAGING = "mcr_concert_meeting"
MEETING_DATABASE = "mcr_meeting"
NOTES_DATABASE = "mcr_concert_venue_notes"
FULL_DATABASE = "mcr_concert_venue_full"
SENTINEL_EVENT = "0b0a3a8e-7c33-4f14-9aa2-2fbd94c80b11"
SENTINEL_OWNER = "1b0d4d3d-8c44-4a25-8bb3-3fce05d91c22"
TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", "/Users/lhs/Desktop/mcr-build/exec-w3-pg"))


def manifest(directory: Path) -> dict[int, str]:
    import hashlib
    import re

    expected: dict[int, str] = {}
    for path in sorted(directory.glob("*.sql")):
        if path.name.endswith(".down.sql"):
            continue
        match = re.fullmatch(r"([0-9]+)_.+\.sql", path.name)
        if not match or int(match[1]) <= 0 or int(match[1]) in expected:
            raise SmokeError("preflight", f"invalid migration set in {directory.name}")
        expected[int(match[1])] = hashlib.sha384(path.read_bytes()).hexdigest()
    if not expected:
        raise SmokeError("preflight", f"empty migration set in {directory.name}")
    return expected


def prepare_venue_directory(root: Path) -> Path:
    destination = root / "venue-migrations"
    destination.mkdir(mode=0o700)
    for name in VENUE_MIGRATIONS:
        source = MIGRATIONS / name
        if source.is_symlink() or not source.is_file():
            raise SmokeError("preflight", f"missing regular migration {name}")
        shutil.copyfile(source, destination / name)
    return destination


def copy_migrations_except(destination: Path, dropped: str) -> None:
    destination.mkdir(mode=0o700)
    for source in sorted(MIGRATIONS.glob("*.sql")):
        if source.name.endswith(".down.sql") or source.name == dropped:
            continue
        shutil.copyfile(source, destination / source.name)


def mutate_copy(destination: Path, name: str) -> None:
    destination.mkdir(mode=0o700)
    for source in sorted(MIGRATIONS.glob("*.sql")):
        if source.name.endswith(".down.sql"):
            continue
        target = destination / source.name
        shutil.copyfile(source, target)
        if source.name == name:
            with target.open("a", encoding="utf-8") as stream:
                stream.write("\n-- tampered after publication\n")


def service_environment(cluster: Cluster, database: str, directory: Path | None) -> dict[str, str]:
    environment = {**cluster.env, "CONCERT_DATABASE_URL": cluster.url(database)}
    if directory is not None:
        environment["CONCERT_MIGRATION_DIRECTORY"] = str(directory)
    return environment


def migrate_and_verify(
    cluster: Cluster,
    binary: Path,
    database: str,
    directory: Path | None,
    label: str,
) -> dict[str, object]:
    execute(
        [str(cluster.tools["createdb"]), "--no-password", "--maintenance-db=postgres", database],
        env=cluster.client_env("postgres"),
        phase=f"{label}:createdb",
        cwd=cluster.root,
    )
    environment = service_environment(cluster, database, directory)
    row: dict[str, object] = {"database": database, "label": label}
    for flag, key in (("--migrate", "migrate"), ("--verify-migrations", "verify")):
        execute([str(binary), flag], env=environment, phase=f"{label}:{flag}", cwd=cluster.root)
        row[key] = "PASS"
    return row


def cargo_environment(cluster: Cluster, target_dir: Path) -> dict[str, str]:
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith("PG") and not key.endswith("DATABASE_URL")
    }
    environment.update(
        {
            "CARGO_TARGET_DIR": str(target_dir),
            "CARGO_BUILD_JOBS": "3",
            "CONCERT_TEST_DATABASE_URL": cluster.url(LOCATION_DATABASE),
            "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
            "MEETING_TEST_OWNER_ROOT": str(cluster.root),
            "MEETING_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
            "MEETING_TEST_OWNER_TOKEN": cluster.token,
            "MEETING_TEST_DATABASE_URL": cluster.url(MEETING_DATABASE),
            "PLACE_NOTES_TEST_DATABASE_URL": cluster.url(NOTES_DATABASE),
        }
    )
    return environment


def build_service(target_dir: Path, cargo: str, toolchain: str) -> Path:
    execute(
        [cargo, toolchain, "build", "-p", "concert-service", "--bin", "concert-service"],
        env={**os.environ, "CARGO_TARGET_DIR": str(target_dir), "CARGO_BUILD_JOBS": "3"},
        phase="cargo-build",
        timeout=1800,
        cwd=BACKEND,
    )
    binary = target_dir / "debug" / "concert-service"
    if binary.is_symlink() or not binary.is_file() or not os.access(binary, os.X_OK):
        raise SmokeError("preflight", "concert-service binary is missing or not executable")
    return binary


def install_sentinel(cluster: Cluster) -> None:
    cluster.sql(
        LOCATION_DATABASE,
        "insert into concert_events(event_id,slug,title,phase) "
        f"values ('{SENTINEL_EVENT}','venue-restart-sentinel','Venue restart sentinel','OPEN'); "
        "insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) "
        f"values ('{SENTINEL_EVENT}','{SENTINEL_OWNER}','PARADE',false,false,true); "
        "insert into concert_venue_location_sharing(event_id,owner_id,state,revision,expires_at) "
        f"values ('{SENTINEL_EVENT}','{SENTINEL_OWNER}','ACTIVE',1,clock_timestamp()+interval '1 hour');",
    )


def sentinel(cluster: Cluster) -> str:
    return cluster.sql(
        LOCATION_DATABASE,
        "select state || '|' || revision from concert_venue_location_sharing "
        f"where event_id='{SENTINEL_EVENT}' and owner_id='{SENTINEL_OWNER}';",
    )


def expect_verify_failure(
    cluster: Cluster, binary: Path, database: str, directory: Path, phase: str
) -> dict[str, object]:
    try:
        execute(
            [str(binary), "--verify-migrations"],
            env=service_environment(cluster, database, directory),
            phase=phase,
            cwd=cluster.root,
        )
    except SmokeError as error:
        if error.detail["exit_code"] in (None, 0):
            raise
        return {
            "phase": phase,
            "exit_code": error.detail["exit_code"],
            "diagnostic": error.detail["diagnostic"],
        }
    raise SmokeError(phase, "mutated migration verifier unexpectedly succeeded")


def run_venue_tests(cluster: Cluster, target_dir: Path, cargo: str, toolchain: str) -> dict[str, object]:
    command = [
        cargo,
        toolchain,
        "test",
        "-p",
        "concert-service",
        "--test",
        "venue_*",
        "--",
        "--test-threads=1",
    ]
    result = execute(
        command,
        env=cargo_environment(cluster, target_dir),
        phase="cargo-venue-tests",
        timeout=1800,
        cwd=BACKEND,
    )
    return {
        "command": " ".join(command),
        "exit_code": 0,
        "output_tail": result.stdout[-4000:],
    }


def qualify(
    cluster: Cluster,
    binary: Path,
    target_dir: Path,
    cargo: str,
    toolchain: str,
    report: dict[str, object],
) -> None:
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    if not str(report["postgres_version"]).startswith("16."):
        raise SmokeError("preflight", "PostgreSQL 16 is required")

    venue_directory = prepare_venue_directory(cluster.root)
    venue_manifest = manifest(venue_directory)
    full_manifest = manifest(MIGRATIONS)
    report["venue_migrations"] = list(venue_manifest)
    report["full_migration_count"] = len(full_manifest)

    location = migrate_and_verify(
        cluster, binary, LOCATION_DATABASE, venue_directory, "location"
    )
    location["ledger_versions"] = list(venue_manifest)
    check_ledger(cluster, LOCATION_DATABASE, venue_manifest)

    meeting = migrate_and_verify(
        cluster, binary, MEETING_STAGING, venue_directory, "meeting"
    )
    check_ledger(cluster, MEETING_STAGING, venue_manifest)
    cluster.sql(
        "postgres", f"alter database {MEETING_STAGING} rename to {MEETING_DATABASE};"
    )
    meeting["database"] = MEETING_DATABASE
    meeting["ledger_versions"] = list(venue_manifest)
    meeting["rename"] = f"{MEETING_STAGING}->{MEETING_DATABASE}"
    check_ledger(cluster, MEETING_DATABASE, venue_manifest)

    full = migrate_and_verify(cluster, binary, FULL_DATABASE, None, "full")
    full["ledger_count"] = check_ledger(cluster, FULL_DATABASE, full_manifest)

    execute(
        [str(cluster.tools["createdb"]), "--no-password", "--maintenance-db=postgres", NOTES_DATABASE],
        env=cluster.client_env("postgres"),
        phase="notes:createdb",
        cwd=cluster.root,
    )
    report["migrations"] = {"location": location, "meeting": meeting, "full": full}

    install_sentinel(cluster)
    before_value = sentinel(cluster)
    before_start = cluster.sql("postgres", "select pg_postmaster_start_time();")
    before_pid = cluster.assert_postmaster()

    tests = run_venue_tests(cluster, target_dir, cargo, toolchain)
    report["tests"] = tests

    notes_ledger = check_ledger(cluster, NOTES_DATABASE, full_manifest)
    execute(
        [str(binary), "--verify-migrations"],
        env=service_environment(cluster, NOTES_DATABASE, None),
        phase="notes:verify",
        cwd=cluster.root,
    )
    report["notes_ledger_count"] = notes_ledger

    cluster.stop()
    cluster.start()
    after_start = cluster.sql("postgres", "select pg_postmaster_start_time();")
    after_pid = cluster.assert_postmaster()
    if before_start == after_start or before_pid == after_pid:
        raise SmokeError("restart", "postmaster did not change")
    if sentinel(cluster) != before_value:
        raise SmokeError("restart", "venue sentinel did not survive restart")
    check_ledger(cluster, LOCATION_DATABASE, venue_manifest)
    check_ledger(cluster, MEETING_DATABASE, venue_manifest)
    check_ledger(cluster, FULL_DATABASE, full_manifest)
    for database, directory, label in (
        (LOCATION_DATABASE, venue_directory, "location"),
        (FULL_DATABASE, None, "full"),
        (NOTES_DATABASE, None, "notes"),
    ):
        execute(
            [str(binary), "--verify-migrations"],
            env=service_environment(cluster, database, directory),
            phase=f"{label}:verify-after-restart",
            cwd=cluster.root,
        )
    report["restart"] = {
        "status": "PASS",
        "before_pid": before_pid,
        "after_pid": after_pid,
        "sentinel": before_value,
    }

    dropped = cluster.root / "migrations-dropped-0075"
    copy_migrations_except(dropped, "0075_place_notes.sql")
    mutated = cluster.root / "migrations-mutated-0075"
    mutate_copy(mutated, "0075_place_notes.sql")
    report["failure_oracle"] = {
        "dropped": expect_verify_failure(
            cluster, binary, FULL_DATABASE, dropped, "oracle:dropped-0075"
        ),
        "mutated": expect_verify_failure(
            cluster, binary, FULL_DATABASE, mutated, "oracle:mutated-0075"
        ),
    }


def run(binary: Path | None, target_dir: Path, pg_bin_dir: Path, cargo: str, toolchain: str) -> dict[str, object]:
    report: dict[str, object] = {
        "scope": "VENU-BE L/M/N venue ledger, restart and route families on private PostgreSQL 16",
        "result": "FAIL",
        "cleanup": "NOT_RUN",
        "normal_service_startup": "NOT_RUN",
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
    }
    cluster = None
    try:
        if not target_dir.is_absolute():
            raise SmokeError("preflight", "CARGO_TARGET_DIR must be absolute")
        tools = {
            name: (pg_bin_dir / name).resolve()
            for name in ("initdb", "pg_ctl", "psql", "createdb")
        }
        for tool in tools.values():
            if not tool.is_file() or not os.access(tool, os.X_OK):
                raise SmokeError("preflight", f"missing executable: {tool.name}")
        version = execute(
            [str(tools["initdb"]), "--version"],
            env={"PATH": "/usr/bin:/bin:/opt/homebrew/bin"},
            phase="preflight",
            cwd=pg_bin_dir,
        ).stdout
        report["postgres_tools"] = {name: str(path) for name, path in tools.items()}
        report["initdb_version"] = version.strip()
        if "PostgreSQL) 16." not in version:
            raise SmokeError("preflight", "PostgreSQL 16 initdb is required")

        revision = execute(
            ["/usr/bin/git", "-C", str(BACKEND), "rev-parse", "HEAD"],
            env={"PATH": "/usr/bin:/bin"},
            phase="source-revision",
            cwd=BACKEND,
        ).stdout.strip()
        report["source_revision"] = revision

        if binary is None:
            binary = build_service(target_dir, cargo, toolchain)
        if binary.is_symlink() or not binary.is_file():
            raise SmokeError("preflight", "explicit service binary must be a regular file")
        report["service_binary_sha256"] = __import__("hashlib").sha256(binary.read_bytes()).hexdigest()

        cluster = Cluster(tools)
        report["cluster_root"] = str(cluster.root)
        qualify(cluster, binary, target_dir, cargo, toolchain, report)
        report["result"] = "PASS"
    except SmokeError as error:
        report["failure"] = error.detail
    except (KeyboardInterrupt, Exception) as error:  # noqa: BLE001 - report closed failure
        report["failure"] = {"phase": "harness", "diagnostic": type(error).__name__}
    finally:
        if cluster is not None:
            try:
                cluster.cleanup()
                if cluster.root.exists():
                    raise SmokeError("cleanup", "owned cluster root remains")
                report["cleanup"] = "PASS"
            except (SmokeError, OSError) as error:
                report["cleanup"] = "FAIL"
                report["retained_owned_directory"] = str(cluster.root)
                report["cleanup_failure"] = (
                    error.detail
                    if isinstance(error, SmokeError)
                    else {"phase": "cleanup", "diagnostic": type(error).__name__}
                )
                report["result"] = "FAIL"
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, help="Concert service binary; built when omitted")
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/bin"))
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--toolchain", default="+1.88.0")
    parser.add_argument("--target-dir", type=Path, default=TARGET_DIR)
    parser.add_argument("--report", required=True, type=Path, help="New JSON report path; never overwritten")
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("--report must be an absolute new file path")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("--report must be a new writable file in an existing directory")
    os.close(fd)
    report = run(args.binary, args.target_dir, args.pg_bin_dir, args.cargo, args.toolchain)
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
