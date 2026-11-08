#!/usr/bin/env python3
"""WP-03: run the favorites/discovery backend tests against a private cluster.

This reuses the WP-02 harness pattern in `migration_smoke.py`: a marker-owned
`mcr-pg-*` cluster in `/tmp`, no TCP listener, no inherited credentials. It
creates the two fixture databases, applies each service's migrations through the
same SQLx-style ledger the service migrator writes, seeds the catalog from the
import manifest, and then runs the test binaries with the URLs they require.

Usage:
    python3 backend/scripts/mcr/wp03_discovery_tests.py \
        --profile-test catalog_search \
        --concert-test discovery --concert-test discovery_routes
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration  # noqa: E402

PG_BIN = Path("/opt/homebrew/opt/postgresql@16/bin")
# The harness namespaces databases as `mcr_[a-z_]+`, so the names are spelled
# with letters only.
DATABASES = {"profile-service": "mcr_wp03_profile", "concert-service": "mcr_wp03_concert"}
DATABASES = {"profile-service": "mcr_wp_three_profile", "concert-service": "mcr_wp_three_concert"}
SEED = ROOT / "backend/services/profile-service/catalog/catalog-seed.v1.json"


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
    return files


def apply(cluster: migration.Cluster, database: str, service: str) -> list[int]:
    applied: list[int] = []
    for number, path in sorted(manifest(service).items()):
        body = path.read_text()
        cluster.sql(database, f"begin; {body} commit;")
        checksum = hashlib.sha384(path.read_bytes()).hexdigest()
        cluster.sql(
            database,
            "insert into public._sqlx_migrations(version, description, installed_on, "
            f"success, checksum, execution_time) values({number}, '{path.stem}', "
            f"clock_timestamp(), true, decode('{checksum}','hex'), 0)",
        )
        applied.append(number)
    return applied


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile-test", action="append", default=[])
    parser.add_argument("--concert-test", action="append", default=[])
    parser.add_argument("--no-seed", action="store_true")
    args = parser.parse_args(argv)
    tools = {name: migration.executable(PG_BIN / name) for name in migration.PG_TOOLS}
    cluster = migration.Cluster(tools)
    code = 1
    try:
        cluster.init()
        cluster.start()
        for service, database in DATABASES.items():
            migration.execute(
                [str(cluster.tools["createdb"]), "--no-password",
                 "--maintenance-db=postgres", database],
                env=cluster.client_env("postgres"), phase="createdb", cwd=cluster.root,
            )
            # The ledger table SQLx itself creates.
            cluster.sql(
                database,
                "create table public._sqlx_migrations(version bigint primary key, "
                "description text not null, installed_on timestamptz not null default now(), "
                "success boolean not null, checksum bytea not null, execution_time bigint not null)",
            )
            applied = apply(cluster, database, service)
            print(f"{service}: applied {applied[0]}..{applied[-1]} ({len(applied)} migrations)")
        if not args.no_seed:
            seed = subprocess.run(
                [sys.executable, str(ROOT / "backend/scripts/mcr/catalog_import.py"),
                 "--database-url", cluster.url(DATABASES["profile-service"])],
                capture_output=True, text=True, timeout=180,
            )
            print(f"catalog seed: {seed.stdout.strip() or seed.stderr.strip()}")
            if seed.returncode != 0:
                raise SystemExit("catalog seed failed")
        env = {
            "PATH": f"{Path.home()}/.cargo/bin:/usr/bin:/bin:/opt/homebrew/bin",
            "HOME": str(Path.home()),
            "PROFILE_TEST_DATABASE_URL": cluster.url(DATABASES["profile-service"]),
            "CONCERT_TEST_DATABASE_URL": cluster.url(DATABASES["concert-service"]),
        }
        failures = 0
        for package, tests in (("profile-service", args.profile_test),
                               ("concert-service", args.concert_test)):
            for test in tests:
                command = ["cargo", "+1.88.0", "test", "--locked", "-p", package,
                           "--test", test, "--", "--test-threads=1"]
                print(f"$ {' '.join(command)}")
                result = subprocess.run(command, cwd=ROOT / "backend", env=env,
                                        timeout=1800)
                print(f"{package} --test {test}: exit {result.returncode}")
                failures += result.returncode != 0
        code = 0 if failures == 0 else 1
    finally:
        cluster.cleanup()
    return code


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
