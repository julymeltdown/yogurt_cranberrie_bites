#!/usr/bin/env python3
"""WP-04b: create the private owned PostgreSQL cluster the identity tests use.

This is the harness pattern from `backend/scripts/mcr/migration_smoke.py` and
`wp02_migration_ledger.py`, reduced to what a test run needs:

* a marker-owned `mcr-pg-*` directory in `/tmp`;
* no TCP listener, a private Unix socket directory, `listen_addresses=''`;
* a random password written to a private `PGPASSFILE`, never to a URL that is
  printed;
* the profile-service database created empty, so the test binary applies the
  real migrations itself.

It prints the connection URL the tests need and then waits. `--stop` shuts the
cluster down and removes only a directory this script's own marker validates.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import migration_smoke as migration  # noqa: E402
import wp_cluster_harness as harness  # noqa: E402

DATABASE = "mcr_wp_four_b_identity"
CONCERT_DATABASE = "mcr_wp_four_b_concert"


def start(pg_bin_dir: Path, state_file: Path) -> int:
    tools = harness.resolve_tools(pg_bin_dir)
    cluster = migration.Cluster(tools)
    cluster.init()
    cluster.start()
    version = cluster.sql("postgres", "show server_version;")
    cluster.sql("postgres", f"create database {DATABASE} template template0;")
    # The concert service's tests read `CONCERT_TEST_DATABASE_URL`; both
    # services' suites run against this one private cluster, each in its own
    # database, exactly as the compose topology separates them.
    cluster.sql("postgres", f"create database {CONCERT_DATABASE} template template0;")
    # The harness validates the database name against its own `mcr_` namespace;
    # this database is named in that namespace so the URL check applies.
    url = cluster.url(DATABASE)
    concert_url = cluster.url(CONCERT_DATABASE)
    state_file.write_text(
        json.dumps(
            {
                "root": str(cluster.root),
                "token": cluster.token,
                "password": cluster.password,
                "database": DATABASE,
                "concert_database": CONCERT_DATABASE,
                "url": url,
                "concert_url": concert_url,
                "server_version": version,
            }
        )
    )
    state_file.chmod(0o600)
    print(json.dumps({
        "database_url": url,
        "concert_database_url": concert_url,
        "server_version": version,
    }))
    return 0


def stop(state_file: Path) -> int:
    if not state_file.is_file():
        print("no cluster state; nothing to stop", file=sys.stderr)
        return 0
    state = json.loads(state_file.read_text())
    root = Path(state["root"]).resolve()
    marker = root / "mcr-owner.json"
    try:
        recorded = json.loads(marker.read_text())
    except (OSError, ValueError):
        print("refusing to stop a cluster without its marker", file=sys.stderr)
        return 1
    if recorded != {"token": state["token"], "uid": os.getuid()}:
        print("refusing to stop a foreign cluster", file=sys.stderr)
        return 1
    info = root.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid():
        print("refusing to stop a replaced directory", file=sys.stderr)
        return 1
    tools = {name: migration.executable(Path("/opt/homebrew/bin") / name) for name in ("pg_ctl",)}
    env = {"PATH": "/usr/bin:/bin:/opt/homebrew/bin", "LC_ALL": "C", "TMPDIR": str(root)}
    subprocess.run(
        [str(tools["pg_ctl"]), "-D", str(root / "data"), "-w", "-t", "20", "-m", "fast", "stop"],
        env=env,
        capture_output=True,
        text=True,
        cwd=root,
    )
    shutil.rmtree(root)
    state_file.unlink(missing_ok=True)
    print("stopped")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/bin"))
    parser.add_argument("--state-file", type=Path, default=Path("/tmp/wp04b-identity-cluster.json"))
    parser.add_argument("--stop", action="store_true")
    args = parser.parse_args(argv)
    if args.stop:
        return stop(args.state_file)
    return start(args.pg_bin_dir, args.state_file)


if __name__ == "__main__":
    sys.exit(main())
