#!/usr/bin/env python3
"""Run the Post owner SQL regression in a newly owned loopback-only cluster.

Requires an explicitly allocated shared Cargo lease; this runner never builds or
creates a second target. Pass the exact already-built post_service unit binary.
No URLs, payloads or credentials are written to evidence output. Owned cluster
and all test databases are stopped and removed even when the regression fails.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import uuid


def run(argv, **kwargs):
    return subprocess.run(argv, check=True, text=True, capture_output=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--test-binary", type=Path)
    mode.add_argument("--sql-only", action="store_true", help="apply all owned Post migrations without Rust build")
    parser.add_argument("--test-sha256", help="exact compiled/copied unit executable digest")
    args = parser.parse_args()
    binary = None
    if args.test_binary:
        binary = args.test_binary.resolve(strict=True)
        if not binary.name.startswith("post_service-") or not args.test_sha256:
            parser.error("provide the exact Post unit-test executable and --test-sha256")
        with binary.open("rb") as stream:
            actual = hashlib.file_digest(stream, "sha256").hexdigest()
        if actual != args.test_sha256:
            parser.error("Post test executable digest differs from the pinned build")
    tools = {t: shutil.which(t) for t in ("initdb", "pg_ctl", "psql")}
    if not all(tools.values()):
        parser.error("native PostgreSQL tools unavailable")
    with tempfile.TemporaryDirectory(prefix="mcr-post-concert-owned-") as temp:
        root = Path(temp)
        data = root / "data"
        user = "mcr_post_concert_fixture"
        database = "mcr_post_concert_fixture_" + uuid.uuid4().hex
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        run([tools["initdb"], "-D", str(data), "-A", "trust", "-U", user, "--encoding=UTF8", "--locale=C"])
        started = False
        try:
            # Disable Unix sockets to avoid exposure beyond the owned loopback
            # listener. PostgreSQL log stays in this ephemeral private directory.
            run([tools["pg_ctl"], "-D", str(data), "-l", str(root / "postgres.log"), "-o", f"-h 127.0.0.1 -p {port} -k ''", "-w", "start"])
            started = True
            base = [tools["psql"], "-h", "127.0.0.1", "-p", str(port), "-U", user, "-d", "postgres", "-v", "ON_ERROR_STOP=1", "-At"]
            run(base + ["-c", f'create database "{database}" template template0'])
            lineage = run(base + ["-c", "select system_identifier::text from pg_control_system()"]).stdout.strip()
            server_version = run(base + ["-c", "show server_version"]).stdout.strip()
            if args.sql_only:
                migrations = Path(__file__).resolve().parents[2] / "services/post-service/migrations"
                files = sorted(migrations.glob("*.sql"))
                assert files[-1].name == "0031_concert_feed_owner.sql"
                fixture = base.copy()
                fixture[fixture.index("postgres")] = database
                sql_args = ["--single-transaction"]
                for migration in files:
                    sql_args.extend(["-f", str(migration)])
                run(fixture + sql_args)
                checks = "select count(*) from pg_tables where schemaname='public' and tablename like 'post_concert_%'"
                assert run(fixture + ["-c", checks]).stdout.strip() == "5"
                print(json.dumps({"scope":"empty isolated Post database; all31 migrations", "result":"PASS", "exit_code":0, "production_authority":"NOT_QUALIFIED", "postgres":server_version}))
                return 0
            env = dict(os.environ, POST_CONCERT_TEST_DATABASE_URL=f"postgresql://{user}@127.0.0.1:{port}/{database}", POST_CONCERT_TEST_LINEAGE=lineage)
            result = subprocess.run([str(binary), "concert_feed::tests::postgres_owner_authorization_persistence_and_races", "--ignored", "--exact", "--nocapture"], env=env, text=True, capture_output=True, timeout=60)
            # Rust assertions never print content/URLs. Do not echo subprocess
            # PostgreSQL diagnostics that could include a private connection URL.
            if result.returncode == 0 and ("running 1 test" not in result.stdout or "1 passed; 0 failed" not in result.stdout):
                raise RuntimeError("owner fixture test was not actually executed")
            print(result.stdout, end="")
            if result.returncode:
                print(result.stderr, end="")
            print(json.dumps({"scope": "isolated Post owner PostgreSQL; test authority only", "exit_code": result.returncode, "production_authority": "NOT_QUALIFIED", "postgres": server_version, "cleanup": "owned ephemeral cluster"}))
            return result.returncode
        finally:
            if started or (data / "postmaster.pid").exists():
                run([tools["pg_ctl"], "-D", str(data), "-m", "immediate", "-w", "stop"])


if __name__ == "__main__":
    raise SystemExit(main())
