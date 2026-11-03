#!/usr/bin/env python3
"""BE24 exact owned PostgreSQL16 schema/producer/restart qualification, never a live DB."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import signal
import subprocess
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_concert_be_media"

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def require(value, message):
    if not value:
        raise RuntimeError(message)

def run(args, report):
    tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
    test = migration.executable(args.repository_test) if args.repository_test else None
    cluster = migration.Cluster(tools)
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        require(report["postgres_version"].startswith("16."), "PostgreSQL16 required")
        cluster.sql("postgres", f"create database {DATABASE} template template0;")
        paths = sorted((ROOT / "backend/services/concert-service/migrations").glob("*.sql"))
        report["migrations"] = {p.name: digest(p) for p in paths}
        for path in paths:
            result = subprocess.run([str(tools["psql"]), "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "-1", "-f", str(path)], env=cluster.client_env(DATABASE), cwd=cluster.root, capture_output=True, timeout=30)
            report["commands"][f"migrate-{path.name}"] = result.returncode
            require(result.returncode == 0, f"migration failed: {path.name}")
        require(cluster.sql(DATABASE, "select count(*) from information_schema.columns where table_name='concert_media';") == "21", "media columns differ")
        report["schema"] = "PASS"
        # Exercise the exact SQL string from the owning Rust source, independently of
        # cached artifacts, including the largest PostgreSQL millisecond setting.
        media_source = (ROOT / "backend/services/concert-service/src/adapters/out/media/mod.rs").read_text()
        clamp_source = media_source[media_source.index("async fn limit_sql_in"):]
        clamp = re.search(r'sqlx::query\("([^"\n]+)"\)', clamp_source)
        require(clamp is not None, "timeout clamp source missing")
        for label, lock, statement, expected in (
                ("unlimited", "0", "0", "1500,5000"),
                ("shorter-caller", "100ms", "200ms", "100,200"),
                ("max-integer", "2147483647ms", "2147483647ms", "1500,5000")):
            query = (f"begin;set local lock_timeout='{lock}';set local statement_timeout='{statement}';"
                     + clamp[1] + ";select (extract(epoch from current_setting('lock_timeout')::interval)*1000)::bigint::text||','||"
                     "(extract(epoch from current_setting('statement_timeout')::interval)*1000)::bigint::text;rollback;")
            require(expected in cluster.sql(DATABASE, query).splitlines(), f"timeout clamp differs: {label}")
            report["commands"][f"timeout-clamp-{label}"] = 0
        report["timeout_clamp"] = "PASS"
        if test:
            report["test_binary_sha256"] = digest(test)
            env = {**cluster.env, "CONCERT_TEST_DATABASE_URL": cluster.url(DATABASE), "CONCERT_TEST_OWNER_ROOT": str(cluster.root), "CONCERT_TEST_OWNER_TOKEN": cluster.token, "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
            def tests(label, extra):
                result = subprocess.run([str(test), *extra], env=env, cwd=cluster.root, capture_output=True, timeout=60)
                report["commands"][label] = result.returncode
                # Test outputs are checked before any diagnostic is written to the report.
                secrets = (cluster.password, cluster.url(DATABASE), cluster.token)
                text = (result.stdout + result.stderr).decode(errors="replace")
                require(all(v not in text for v in secrets), "test diagnostic secret leak")
                report[label] = migration.redact(text, secrets)
                require(result.returncode == 0, f"tests failed: {label}")
                require(digest(test) == report["test_binary_sha256"], "test binary changed")
            tests("producer-tests", ["--test-threads=1"])
            cluster.stop()
            cluster.start()
            tests("restart-tests", ["--ignored", "--exact", "durable_restart_verify", "--test-threads=1"])
            report["restart_persistence"] = "PASS"
        else:
            report["producer_tests"] = report["restart_persistence"] = "NOT_RUN"
        report["result"] = "PASS"
    finally:
        cluster.cleanup()
        require(not cluster.root.exists(), "owned cluster remains")
        report["cleanup"] = "PASS"

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-test", type=Path)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute() or not args.pg_bin.is_absolute():
        parser.error("absolute report and PostgreSQL directory required")
    try:
        fd = os.open(args.report, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    report = {"scope": "BE24 owned native producer and raster codec only", "result": "FAIL", "commands": {}, "real_authority": "NOT_COMPOSED", "http_e2e": "NOT_RUN", "sqlx_runtime_migration_ledger": "NOT_RUN", "cleanup": "NOT_RUN"}
    handlers = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            handlers[sig] = signal.signal(sig, interrupted)
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["dirty_paths"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines()
        report["script_sha256"] = digest(__file__)
        report["helper_sha256"] = digest(migration.__file__)
        contracts = ROOT / "contracts/domain/v1"
        report["contracts"] = {name: digest(contracts / name) for name in ("feed.schema.json", "operations.json", "policy.json")}
        source = [ROOT / "backend/Cargo.lock", ROOT / "backend/services/concert-service/Cargo.toml"]
        for directory in ("backend/crates/concert-application/src/media", "backend/services/concert-service/src/adapters/out/media", "backend/services/concert-service/tests/media_support"):
            source.extend((ROOT / directory).glob("*.rs"))
        source.append(ROOT / "backend/services/concert-service/tests/media.rs")
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(source)}
        run(args, report)
    except (Exception, KeyboardInterrupt) as error:
        # Do not publish raw database diagnostics, DSNs or fixture credentials.
        report.update(result="FAIL", failure=type(error).__name__)
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({k: report.get(k) for k in ("result", "failure", "schema", "restart_persistence", "cleanup")}))
    return 0 if report["result"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
