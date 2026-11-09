#!/usr/bin/env python3
"""Shared private-cluster harness for the work-package test scripts.

Every work package that needs a real PostgreSQL cluster wrote its own copy of the
same four steps:

1. resolve the PostgreSQL tools from an explicit `--pg-bin-dir`;
2. create a marker-owned `mcr-pg-*` cluster in `/tmp` with no TCP listener and no
   inherited credentials (`migration_smoke.Cluster`);
3. create one database and apply a service's forward migrations in order;
4. write a JSON report, run the caller's commands against the cluster URL, and
   remove the cluster afterwards.

Those copies drifted: one refused a duplicate migration number, another applied
both and warned, a third silently applied both, and the report keys differed
between them. This module is the one implementation. Each script keeps its own
CLI and its own report scope; only the mechanics are shared.

Nothing here decides *what* to run. A caller supplies the database name, the
service whose migrations to apply, and a function that does the work while the
cluster is up.
"""
from __future__ import annotations

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import traceback
from typing import Callable, Iterable, Sequence

ROOT = Path(__file__).resolve().parents[3]
BACKEND = ROOT / "backend"
sys.path.insert(0, str(BACKEND / "scripts/mcr"))
import migration_smoke as migration  # noqa: E402

DEFAULT_PG_BIN_DIR = Path("/opt/homebrew/opt/postgresql@16/bin")
# The PostgreSQL 17 Homebrew prefix. A caller whose default is this one keeps it.
LEGACY_PG_BIN_DIR = Path("/opt/homebrew/bin")


def resolve_tools(pg_bin_dir: Path) -> dict[str, Path]:
    """Resolve every PostgreSQL tool the harness uses, or refuse."""
    return {name: migration.executable(pg_bin_dir / name) for name in migration.PG_TOOLS}


def forward_migrations(service: str) -> list[Path]:
    """Every forward migration of `service`, in file-name order.

    The duplicate-number policy is the caller's: `on_duplicate` decides whether a
    repeated number is refused or applied in order with a warning. The two
    packages that met a genuine collision chose differently, and that choice is
    part of each script's contract, not of this helper.
    """
    directory = BACKEND / "services" / service / "migrations"
    files = [
        path
        for path in sorted(directory.glob("*.sql"))
        if not path.name.endswith(".down.sql")
    ]
    if not files:
        raise SystemExit(f"empty {service} migration set")
    return files


def migration_number(path: Path) -> int:
    return int(path.name.split("_", 1)[0])


def refuse_duplicate_numbers(files: Sequence[Path], service: str) -> None:
    """Refuse a repeated migration number. The strict policy."""
    numbers = [migration_number(path) for path in files]
    if len(numbers) != len(set(numbers)):
        raise SystemExit(f"duplicate {service} migration number")


def warn_duplicate_numbers(files: Sequence[Path]) -> None:
    """Apply a repeated migration number in order, with a warning on stderr.

    Used by the packages that met the `0086` collision while it existed: the two
    files touched disjoint tables, so one ordered pass reached the same head
    schema. The warning keeps the collision visible in the report.
    """
    numbers = [migration_number(path) for path in files]
    duplicates = sorted({number for number in numbers if numbers.count(number) > 1})
    if duplicates:
        print(
            json.dumps({"warning": "duplicate migration numbers applied in order",
                        "numbers": duplicates}),
            file=sys.stderr,
        )


def create_database(cluster: migration.Cluster, database: str) -> None:
    """Create one empty database on the owned cluster."""
    migration.execute(
        [str(cluster.tools["createdb"]), "--no-password",
         "--maintenance-db=postgres", database],
        env=cluster.client_env("postgres"), phase="createdb", cwd=cluster.root,
    )


def apply_migrations(
    cluster: migration.Cluster,
    database: str,
    files: Iterable[Path],
    *,
    with_ledger: bool = False,
) -> list[int]:
    """Apply `files` to `database` in order and return the applied numbers.

    `with_ledger` also writes the SQLx receipt row the service migrator would
    write, so a test that reads `_sqlx_migrations` sees a real ledger.
    """
    applied: list[int] = []
    for path in files:
        number = migration_number(path)
        cluster.sql(database, f"begin; {path.read_text()} commit;")
        if with_ledger:
            import hashlib

            checksum = hashlib.sha384(path.read_bytes()).hexdigest()
            cluster.sql(
                database,
                "insert into public._sqlx_migrations(version, description, installed_on, "
                f"success, checksum, execution_time) values({number}, '{path.stem}', "
                f"clock_timestamp(), true, decode('{checksum}','hex'), 0)",
            )
        applied.append(number)
    return applied


def create_ledger_table(cluster: migration.Cluster, database: str) -> None:
    """Create the `_sqlx_migrations` table exactly as SQLx creates it."""
    cluster.sql(
        database,
        "create table public._sqlx_migrations(version bigint primary key, "
        "description text not null, installed_on timestamptz not null default now(), "
        "success boolean not null, checksum bytea not null, execution_time bigint not null)",
    )


def run_command(command: str, url: str, env_name: str = "CONCERT_TEST_DATABASE_URL") -> int:
    """Run one shell command with the cluster URL in `env_name`; return its exit code."""
    env = {**os.environ, env_name: url}
    completed = subprocess.run(command, shell=True, env=env, cwd=str(BACKEND))
    return completed.returncode


def new_report(scope: str, **extra: object) -> dict:
    """A report with the keys every harness writes."""
    return {
        "scope": scope,
        **extra,
        "result": "FAIL",
        "cleanup": "NOT_RUN",
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
    }


def write_report(report: dict, path: Path) -> None:
    report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    path.write_text(json.dumps(report, indent=2) + "\n")


def finish(
    report: dict,
    path: Path,
    *,
    keep: bool = False,
    cluster: migration.Cluster | None = None,
) -> None:
    """Clean up the cluster and write the report. Never raises."""
    if cluster is not None and not keep:
        try:
            cluster.cleanup()
            report["cleanup"] = "PASS"
        except BaseException as error:  # noqa: BLE001 - the report must always be written
            report["cleanup"] = "FAIL"
            report["cleanup_failure"] = type(error).__name__
            report["result"] = "FAIL"
    elif cluster is not None:
        report["cleanup"] = "KEPT"
    write_report(report, path)


def record_failure(report: dict, error: BaseException) -> None:
    """Record an exception in the report without leaking a credential."""
    if isinstance(error, migration.SmokeError):
        report["failure"] = error.detail
    else:
        report["failure"] = {"phase": "harness", "diagnostic": type(error).__name__}
        traceback.print_exc()


def cluster_main(
    *,
    scope: str,
    database: str,
    service: str,
    report_path: Path,
    pg_bin_dir: Path,
    body: Callable[[migration.Cluster, str, dict], int],
    keep: bool = False,
    duplicate_policy: Callable[[Sequence[Path]], None] | None = None,
    extra: dict | None = None,
) -> int:
    """The whole four-step harness, parameterised by `body`.

    `body(cluster, url, report)` runs while the cluster is up and returns the
    script's exit code. `duplicate_policy` is the caller's duplicate-number rule;
    the default refuses a repeat.
    """
    tools = resolve_tools(pg_bin_dir)
    report = new_report(scope, **(extra or {}))
    cluster = migration.Cluster(tools)
    exit_code = 1
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        report["cluster_root"] = str(cluster.root)
        create_database(cluster, database)
        files = forward_migrations(service)
        (duplicate_policy or (lambda paths: refuse_duplicate_numbers(paths, service)))(files)
        report["migrations_applied"] = apply_migrations(cluster, database, files)
        url = cluster.url(database)
        report["database_url"] = url
        report["result"] = "PASS"
        exit_code = body(cluster, url, report)
    except BaseException as error:  # noqa: BLE001 - the report must always be written
        record_failure(report, error)
        exit_code = 1
    finally:
        finish(report, report_path, keep=keep, cluster=cluster)
    print(json.dumps({"result": report["result"], "cleanup": report["cleanup"]}))
    return exit_code
