#!/usr/bin/env python3
"""Run the QA05 realtime failure suite in a marker-owned socket-only PG16 cluster.

The experiment binary is `tests/qa05_realtime_relay.rs`; it opens actual
loopback World sockets against the production router/runtime and the production
`AdmissionBridge`. External Auth/owner observations are the committed synthetic
fixture, not IN01/IN03 evidence. This runner owns only the cluster and the
`QA05_TEST_*` environment contract; it never starts another service.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration  # noqa: E402

DATABASE = "mcr_world"
EXPECTED = 6
SOURCES = (
    "backend/services/concert-service/tests/qa05_realtime_relay.rs",
    "backend/services/concert-service/src/adapters/in/world_ws.rs",
    "backend/services/concert-service/src/world/admission_bridge.rs",
    "backend/services/concert-service/src/world/runtime.rs",
    "backend/services/concert-service/src/world/replication.rs",
    "backend/services/concert-service/src/world/recovery.rs",
    "backend/services/concert-service/src/world/ingress.rs",
    "backend/scripts/mcr/qa05_realtime_suite.py",
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class OwnedCluster(migration.Cluster):
    """Owned cluster using anonymous mmap shared memory.

    This host's 32 SysV segments can be exhausted by concurrent isolated MCR
    clusters; mmap keeps this suite's ownership and isolation contract while
    avoiding a host-global resource that is not part of the product under test.
    """

    def init(self) -> None:
        self.assert_owned()
        if self.data.exists():
            raise migration.SmokeError("initdb", "refusing existing data directory")
        password_file = self.root / "init-password"
        fd = os.open(password_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as stream:
            stream.write(self.password + "\n")
        try:
            migration.execute(
                [str(self.tools["initdb"]), "-D", str(self.data), "-U", migration.OWNER,
                 "--auth=scram-sha-256", "--encoding=UTF8", "--no-locale",
                 f"--pwfile={password_file}", "-c", "shared_memory_type=mmap"],
                env=self.env, phase="initdb", cwd=self.root,
            )
        finally:
            password_file.unlink(missing_ok=True)


def require(value: bool, label: str) -> None:
    if not value:
        raise RuntimeError(label)


def apply_migrations(cluster: migration.Cluster) -> list[str]:
    paths = sorted(
        path
        for path in (ROOT / "backend/services/concert-service/migrations").glob("*.sql")
        if not path.name.endswith(".down.sql")
    )
    require(len(paths) >= 5, "migration-inventory")
    for path in paths:
        cluster.sql(DATABASE, path.read_text())
    return [path.name for path in paths]


def exercise(cluster: migration.Cluster, binary: Path, report: dict) -> None:
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    cluster.sql("postgres", f"create database {DATABASE} template template0;")
    report["migrations"] = apply_migrations(cluster)
    environment = {
        **cluster.env,
        "QA05_TEST_DATABASE_URL": cluster.url(DATABASE),
        "QA05_TEST_OWNER_ROOT": str(cluster.root),
        "QA05_TEST_OWNER_TOKEN": cluster.token,
        "QA05_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
    }
    result = subprocess.run(
        [str(binary), "--ignored", "qa05_", "--test-threads=1", "--nocapture"],
        env=environment,
        cwd=cluster.root,
        capture_output=True,
        text=True,
        timeout=180,
    )
    output = result.stdout + result.stderr
    report["test_exit"] = result.returncode
    report["test_output_sha256"] = hashlib.sha256(output.encode()).hexdigest()
    passed = [line for line in output.splitlines() if line.startswith("test qa05_") and line.endswith(" ... ok")]
    failed = [line for line in output.splitlines() if line.startswith("test qa05_") and line.endswith(" ... FAILED")]
    report["cases"] = [line.split()[1] for line in passed]
    report["failed_cases"] = [line.split()[1] for line in failed]
    require(
        cluster.password not in output
        and cluster.url(DATABASE) not in output
        and cluster.token not in output,
        "secret-leak",
    )
    if result.returncode != 0:
        report["test_tail"] = output[-6000:]
        raise RuntimeError("qa05-suite")
    require(len(passed) == EXPECTED and not failed, "case-inventory")

    consumed = int(
        cluster.sql(
            DATABASE,
            "select count(*) from concert_world_tickets where consumed_at is not null;",
        )
    )
    issued = int(cluster.sql(DATABASE, "select count(*) from concert_world_tickets;"))
    require(consumed >= 10, "ticket-consumption-evidence")
    report["tickets"] = {"issued": issued, "consumed": consumed}

    active = int(
        cluster.sql(
            DATABASE,
            "select count(*) from concert_world_memberships "
            "where not disconnected and lease_until > clock_timestamp();",
        )
    )
    prepared = int(cluster.sql("postgres", "select count(*) from pg_prepared_xacts;"))
    require(active == 0 and prepared == 0, "post-suite-cleanup")
    report["post_suite_cleanup"] = {"active_memberships": active, "prepared": prepared}

    before = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(cluster.assert_postmaster() != before, "restart-identity")
    persisted = int(
        cluster.sql(
            DATABASE,
            "select count(*) from concert_world_tickets where consumed_at is not null;",
        )
    )
    require(persisted == consumed, "restart-ticket-persistence")
    report["restart_persistence"] = "PASS"


def run(binary: Path, report: dict) -> None:
    cluster = None
    try:
        require(binary.is_absolute() and not binary.is_symlink() and binary.is_file(), "explicit-binary")
        binary = migration.executable(binary)
        tools = {
            name: migration.executable(Path("/opt/homebrew/opt/postgresql@16/bin") / name)
            for name in migration.PG_TOOLS
        }
        cluster = OwnedCluster(tools)
        report["owned_directory"] = str(cluster.root)
        report["binary_sha256"] = digest(binary)
        exercise(cluster, binary, report)
        report["result"] = "PASS"
    finally:
        cleanup = "PASS"
        try:
            if cluster is not None:
                cluster.cleanup()
        except BaseException:
            cleanup = "FAIL"
        report["cleanup"] = cleanup


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args(argv)
    require(args.report.is_absolute(), "absolute-report")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    report = {
        "scope": "QA05 real sockets with marker-owned PostgreSQL and synthetic external authority",
        "result": "FAIL",
        "cleanup": "NOT_RUN",
        "canonical_auth": "SYNTHETIC_FIXTURE_NOT_IN01",
        "browser_app": "NOT_RUN",
        "source_revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "source_sha256": {path: digest(ROOT / path) for path in SOURCES},
    }
    previous = signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
    try:
        run(args.binary, report)
    except BaseException as error:
        if isinstance(error, RuntimeError) and str(error) in {
            "qa05-suite",
            "case-inventory",
            "secret-leak",
        }:
            report["failure"] = str(error)
        else:
            report["failure"] = type(error).__name__
    finally:
        signal.signal(signal.SIGTERM, previous)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2, sort_keys=True)
            stream.write("\n")
    print(
        json.dumps(
            {
                key: report.get(key)
                for key in ("result", "cleanup", "test_exit", "cases", "failed_cases", "restart_persistence")
            }
        )
    )
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
