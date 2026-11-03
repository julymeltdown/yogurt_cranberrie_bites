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
    if test:
        require(args.artifact_receipt is not None, "compiled artifact receipt required")
        receipt = json.loads(args.artifact_receipt.read_text())
        require(receipt.get("exit_code") == 0, "artifact build did not succeed")
        require(Path(receipt["private_artifact"]).resolve() == test, "artifact receipt path mismatch")
        require(receipt["artifact_sha256"] == digest(test), "artifact receipt digest mismatch")
        require(receipt["source_sha256"], "artifact source pins missing")
        for name, expected in receipt["source_sha256"].items():
            source = (ROOT / name).resolve()
            source.relative_to(ROOT)
            require(digest(source) == expected, f"artifact source changed: {name}")
        require(subprocess.run(["git", "merge-base", "--is-ancestor", receipt["source_revision"], "HEAD"], cwd=ROOT, capture_output=True).returncode == 0, "artifact source ancestry differs")
        report["artifact_receipt_sha256"] = digest(args.artifact_receipt)
        report["artifact_source_revision"] = receipt["source_revision"]
        report["artifact_source_pins"] = "PASS"
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
        # The expiry clauses are the exact producer SQL, not a second implementation.
        event = "a0240000-0000-0000-0000-00000000ee01"
        actor = "a0240000-0000-0000-0000-00000000ee02"
        cluster.sql(DATABASE, f"insert into concert_events(event_id,slug,title,phase) values('{event}','be24-sql-expiry','Owned SQL expiry','OPEN');"
                    f"insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values('{event}','{actor}','REVENGE',false,false,false);")
        updates = re.findall(r'sqlx::query\("(update public\.concert_media set [^"\n]+)"\)', media_source)
        for suffix, state, marker, extra in (
                ("1", "PENDING", "set staged=$2", ",staged"),
                ("2", "UPLOADED", "set staged=null,sanitized=$2", ",staged"),
                ("3", "READY", "set target_id=$2", ",sanitized,width,height")):
            media = "a0240000-0000-0000-0000-00000000ff0" + suffix
            payload_columns = extra if state != "PENDING" else ""
            payload_values = ",decode('00','hex'),1,1" if state == "READY" else ",decode('00','hex')" if state == "UPLOADED" else ""
            cluster.sql(DATABASE, f"insert into concert_media(media_id,event_id,owner_id,purpose,content_type,declared_bytes,input_sha256,policy_event_revision,policy_relationship_revision,policy_authority_revision,state,created_at,expires_at,orphan_at{payload_columns}) values('{media}','{event}','{actor}','POST','image/png',1,decode(repeat('00',32),'hex'),1,1,1,'{state}',clock_timestamp()-interval '2 days',clock_timestamp()-interval '1 day',clock_timestamp()-interval '1 hour'{payload_values});")
            candidates = [q for q in updates if marker in q]
            require(len(candidates) == 1, "expiry source query missing or ambiguous")
            query = candidates[0].replace("$1", f"'{media}'::uuid")
            query = query.replace("$2", f"'{actor}'::uuid" if state == "READY" else "decode('00','hex')").replace("$3", "1").replace("$4", "1")
            require("UPDATE 0" in cluster.sql(DATABASE, query).splitlines(), f"expired source mutation applied: {state}")
            report["commands"][f"expiry-source-{state}"] = 0
        reads = re.findall(r'"(select (?:sanitized,width,height|expires_at) from public\.concert_media [^"\n]+)"', media_source)
        require(len(reads) == 2, "expiry read source missing or ambiguous")
        for query in reads:
            read_id = "a0240000-0000-0000-0000-00000000ff03" if "sanitized" in query else "a0240000-0000-0000-0000-00000000ff01"
            require(cluster.sql(DATABASE, query.replace("$1", f"'{read_id}'::uuid")) == "", "expired source projection returned bytes or intent")
        report["commands"]["expiry-source-private-read-and-intent"] = 0
        cluster.sql(DATABASE, f"delete from concert_events where event_id='{event}';")
        report["expiry_source_guards"] = "PASS"
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
                # Keep enough diagnostics to distinguish assertion failures from fixture
                # cleanup failures. Never truncate before checking exact secret values.
                report[label] = re.sub(r"(?:postgres(?:ql)?|redis)://[^\s]+", "[redacted-url]", text)[-64000:]
                summary = re.search(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", text)
                if summary:
                    report[label + "-summary"] = dict(zip(("passed", "failed", "ignored"), map(int, summary.groups())))
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
    parser.add_argument("--artifact-receipt", type=Path)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute() or not args.pg_bin.is_absolute():
        parser.error("absolute report and PostgreSQL directory required")
    try:
        fd = os.open(args.report, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    report = {"scope": "BE24 owned native producer and raster codec only", "result": "FAIL", "commands": {}, "real_authority": "NOT_COMPOSED", "http_e2e": "NOT_RUN", "sqlx_runtime_migration_ledger": "NOT_RUN", "restart_persistence": "NOT_RUN", "cleanup": "NOT_RUN"}
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
        if type(error) is RuntimeError:
            report["failure_detail"] = str(error)
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
