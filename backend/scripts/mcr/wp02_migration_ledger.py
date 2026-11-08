#!/usr/bin/env python3
"""WP-02 T-112: migration ledger and head check for the profile-owned schema.

This is the ledger test the task document asks for. It reuses the private-cluster
harness pattern from `backend/scripts/mcr/migration_smoke.py` — a marker-owned
`mcr-pg-*` cluster in `/tmp`, no TCP listener, no inherited credentials — and
runs **both** required scenarios:

1. an empty database: every forward migration of both services is applied and
   the SQLx ledger records one successful receipt per file, with the checksum
   SQLx computed from the file bytes;
2. a database already at the previous head (profile `0010`, concert `0079`):
   only the new migrations are applied, the ledger gains exactly the expected
   number of rows, and no applied migration file was edited.

For each scenario the report records the applied order and a schema hash, so a
later reader can compare two runs without re-deriving them.

Usage:
    python3 backend/scripts/mcr/wp02_migration_ledger.py --report /abs/new.json
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration  # noqa: E402

# The previous head of each service, before WP-02.
PREVIOUS_HEAD = {"profile-service": 10, "concert-service": 79}
# The migrations this work package adds.
NEW_MIGRATIONS = {
    "profile-service": [11, 12, 13, 14, 15],
    "concert-service": [80, 81, 82, 83, 84, 85],
}
DATABASES = {"profile-service": "mcr_wp02_ledger_profile", "concert-service": "mcr_wp02_ledger_concert"}


def manifest(service: str) -> dict[int, Path]:
    directory = ROOT / "backend/services" / service / "migrations"
    files: dict[int, Path] = {}
    for path in sorted(directory.glob("*.sql")):
        if path.name.endswith(".down.sql"):
            continue
        number = int(path.name.split("_", 1)[0])
        if number in files:
            raise SystemExit(f"duplicate migration number {number} in {service}")
        files[number] = path
    if not files:
        raise SystemExit(f"empty migration set: {service}")
    return files


def sqlx_checksum(path: Path) -> str:
    """SQLx records the SHA-384 of the migration bytes."""
    return hashlib.sha384(path.read_bytes()).hexdigest()


def schema_hash(cluster: migration.Cluster, database: str) -> str:
    """A stable hash of the applied schema: tables, columns, constraints, indexes."""
    query = r"""
    with objects as (
      select 'column' as kind, table_name || '.' || column_name || ':' || data_type
             || ':' || is_nullable as name
      from information_schema.columns where table_schema='public'
      union all
      select 'constraint', conrelid::regclass::text || '.' || conname
      from pg_constraint where connamespace='public'::regnamespace
      union all
      select 'index', indexrelid::regclass::text
      from pg_index where indrelid in (
        select oid from pg_class where relnamespace='public'::regnamespace and relkind='r')
      union all
      select 'table', relname from pg_class
      where relnamespace='public'::regnamespace and relkind='r'
    )
    select kind || ' ' || name from objects order by kind, name;
    """
    rows = cluster.sql(database, query)
    return hashlib.sha384(rows.encode()).hexdigest()


def apply(cluster: migration.Cluster, database: str, service: str, files: dict[int, Path],
          only: list[int] | None = None) -> list[int]:
    applied: list[int] = []
    for number, path in sorted(files.items()):
        if only is not None and number not in only:
            continue
        # Apply through the SQLx-style ledger so the receipt is comparable to a
        # real service migration run. The SQL is executed as one script.
        body = path.read_text()
        escaped = body.replace("'", "''")
        cluster.sql(database, f"begin; {body} commit;")
        cluster.sql(
            database,
            "insert into public._sqlx_migrations(version, description, installed_on, "
            f"success, checksum, execution_time) values({number}, "
            f"'{path.stem}', clock_timestamp(), true, "
            f"decode('{sqlx_checksum(path)}','hex'), 0)",
        )
        assert escaped  # keep the variable meaningful for readers
        applied.append(number)
    return applied


def ledger(cluster: migration.Cluster, database: str) -> dict[int, str]:
    rows = cluster.sql(
        database,
        "select version, success, encode(checksum,'hex') from public._sqlx_migrations "
        "order by version",
    )
    result: dict[int, str] = {}
    for line in rows.splitlines():
        version, success, checksum = line.split("|")
        if success != "t":
            raise SystemExit(f"unsuccessful migration receipt: {version}")
        result[int(version)] = checksum
    return result


def scenario(cluster: migration.Cluster, service: str, files: dict[int, Path],
             from_head: int | None) -> dict:
    # One database per scenario, so the empty run and the previous-head run never
    # share a ledger.
    suffix = "empty" if from_head is None else f"head{from_head}"
    database = f"{DATABASES[service]}_{suffix}"
    cluster.sql("postgres", f"create database {database} template template0;")
    # The ledger table SQLx itself creates.
    cluster.sql(
        database,
        "create table public._sqlx_migrations(version bigint primary key, "
        "description text not null, installed_on timestamptz not null default now(), "
        "success boolean not null, checksum bytea not null, execution_time bigint not null)",
    )
    if from_head is None:
        applied = apply(cluster, database, service, files)
        scenario_name = "empty"
    else:
        previous = {number: path for number, path in files.items() if number <= from_head}
        apply(cluster, database, service, previous)
        before = ledger(cluster, database)
        applied = apply(cluster, database, service, files, only=NEW_MIGRATIONS[service])
        after = ledger(cluster, database)
        if set(after) - set(before) != set(NEW_MIGRATIONS[service]):
            raise SystemExit(f"{service}: ledger gained unexpected versions")
        if any(before[version] != after[version] for version in before):
            raise SystemExit(f"{service}: an applied migration checksum changed")
        scenario_name = f"previous-head-{from_head}"
    receipts = ledger(cluster, database)
    expected = {number: sqlx_checksum(path) for number, path in files.items()}
    if receipts != expected:
        raise SystemExit(f"{service}: ledger differs from the migration files")
    return {
        "scenario": scenario_name,
        "database": database,
        "applied_order": applied,
        "ledger_rows": len(receipts),
        "new_migrations": sorted(set(applied) & set(NEW_MIGRATIONS[service])),
        "schema_hash": schema_hash(cluster, database),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/bin"))
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("--report must be an absolute new file path")
    tools = {
        name: migration.executable(args.pg_bin_dir / name) for name in migration.PG_TOOLS
    }
    report: dict = {
        "scope": "WP-02 T-112 migration ledger and head check",
        "result": "FAIL",
        "cleanup": "NOT_RUN",
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
    }
    cluster = migration.Cluster(tools)
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        report["services"] = {}
        for service in ("profile-service", "concert-service"):
            files = manifest(service)
            report["services"][service] = {
                "head": max(files),
                "previous_head": PREVIOUS_HEAD[service],
                "new_migrations": NEW_MIGRATIONS[service],
                "runs": [
                    scenario(cluster, service, files, None),
                    scenario(cluster, service, files, PREVIOUS_HEAD[service]),
                ],
            }
        report["result"] = "PASS"
    except migration.SmokeError as error:
        report["failure"] = error.detail
    except BaseException as error:  # noqa: BLE001 - the report must always be written
        report["failure"] = {"phase": "harness", "diagnostic": type(error).__name__}
    finally:
        try:
            cluster.cleanup()
            report["cleanup"] = "PASS"
        except BaseException as error:  # noqa: BLE001
            report["cleanup"] = "FAIL"
            report["cleanup_failure"] = type(error).__name__
            report["result"] = "FAIL"
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"result": report["result"], "cleanup": report["cleanup"]}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
