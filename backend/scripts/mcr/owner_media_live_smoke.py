#!/usr/bin/env python3
"""Source-pinned uploader-runtime regressions in a disposable private PostgreSQL16 cluster.

This verifies native fixture behavior, never canonical Auth or HTTP qualification.
It never compiles, downloads dependencies or opens a shared target directory.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_concert_owner_media_live"

def require(value, message):
    if not value:
        raise RuntimeError(message)

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def required_sources(root=ROOT):
    paths = {root / "backend/Cargo.lock", root / "backend/Cargo.toml"}
    # Include full runtime/dependency source trees, not an arbitrary receipt-selected
    # subset. Shared composition changes invalidate the binary as they should.
    for directory in ("services/concert-service", "crates/concert-application",
                      "crates/concert-domain", "crates/platform"):
        base = root / "backend" / directory
        paths.add(base / "Cargo.toml")
        paths.update(base.rglob("*.rs"))
    paths.update((root / "backend/services/concert-service/migrations").glob("*.sql"))
    paths.update((root / "contracts/domain/v1").glob("*.json"))
    paths.add(root / "backend/services/concert-service/build.rs")
    return {str(p.relative_to(root)): digest(p) for p in sorted(paths)}

def validate_artifact(test, receipt_path, root=ROOT):
    """Must complete before any Cluster construction/init/start."""
    test = Path(test).resolve(strict=True)
    require(not Path(receipt_path).is_symlink(), "artifact receipt symlink refused")
    receipt = json.loads(Path(receipt_path).read_text())
    require(receipt.get("exit_code") == 0, "artifact build did not succeed")
    require(Path(receipt.get("private_artifact", "")).resolve() == test, "artifact path differs")
    require(receipt.get("artifact_sha256") == digest(test), "artifact digest differs")
    artifact = receipt.get("cargo_artifact", {})
    require(artifact.get("reason") == "compiler-artifact" and
            artifact.get("target", {}).get("name") == "owner_media_live" and
            artifact.get("profile", {}).get("test") is True and artifact.get("executable"),
            "exact Cargo JSON test artifact missing")
    require(str(receipt.get("rustc_version", "")).startswith("rustc 1.88.0"), "Rust1.88 artifact required")
    require(receipt.get("incremental") == "0" and receipt.get("debug") == "0" and
            receipt.get("jobs") in (1, 2) and receipt.get("locked") is True and receipt.get("offline") is True,
            "artifact build constraints differ")
    pins = receipt.get("source_sha256", {})
    required = required_sources(root)
    require(required.keys() <= pins.keys(), "artifact source pins incomplete")
    for name, expected in pins.items():
        require(not Path(name).is_absolute(), "absolute source pin refused")
        source = (root / name).resolve()
        require(source.is_relative_to(root.resolve()), "source pin escapes checkout")
        require(digest(source) == expected, "artifact source changed")
    revision = receipt.get("source_revision", "")
    require(re.fullmatch(r"[0-9a-f]{40}", revision) is not None, "source revision missing")
    require(subprocess.run(["git", "merge-base", "--is-ancestor", revision, "HEAD"], cwd=root, capture_output=True).returncode == 0,
            "artifact source ancestry differs")
    return receipt

def run(args, report):
    test = migration.executable(args.repository_test)
    receipt = validate_artifact(test, args.artifact_receipt)
    report.update(artifact_receipt_sha256=digest(args.artifact_receipt),
                  artifact_sha256=digest(test), source_revision=receipt["source_revision"],
                  source_pins="PASS")
    tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
    cluster = migration.Cluster(tools)
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        require(report["postgres_version"].startswith("16."), "PostgreSQL16 required")
        cluster.sql("postgres", f"create database {DATABASE} template template0;")
        for path in sorted((ROOT / "backend/services/concert-service/migrations").glob("*.sql")):
            r = subprocess.run([str(tools["psql"]), "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "-1", "-f", str(path)], env=cluster.client_env(DATABASE), cwd=cluster.root, capture_output=True, timeout=30)
            report["commands"]["migration-" + path.name] = r.returncode
            require(r.returncode == 0, "migration failed")
        env = {**cluster.env, "CONCERT_TEST_DATABASE_URL": cluster.url(DATABASE),
               "CONCERT_TEST_OWNER_ROOT": str(cluster.root), "CONCERT_TEST_OWNER_TOKEN": cluster.token,
               "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
        r = subprocess.run([str(test), "--test-threads=1"], cwd=cluster.root, env=env, capture_output=True, timeout=60)
        report["commands"]["owner-media-runtime"] = r.returncode
        output = (r.stdout + r.stderr).decode(errors="replace")
        require(all(v not in output for v in (cluster.password, cluster.url(DATABASE), cluster.token)), "native diagnostic secret leak")
        summary = re.search(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", output)
        require(summary is not None, "native test summary missing")
        report["tests"] = dict(zip(("passed", "failed", "ignored"), map(int, summary.groups())))
        report["native_diagnostic"] = re.sub(r"(?:postgres(?:ql)?|redis)://[^\s]+", "[redacted-url]", output)[-12000:]
        require(r.returncode == 0 and report["tests"]["passed"] >= 6 and
                report["tests"]["failed"] == 0 and report["tests"]["ignored"] == 0, "native tests failed or skipped")
        require(digest(test) == report["artifact_sha256"], "artifact changed during test")
        report["result"] = "PASS"
    finally:
        cluster.cleanup()
        require(not cluster.root.exists(), "owned cluster remains")
        report["cleanup"] = "PASS"

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--repository-test", type=Path, required=True)
    p.add_argument("--artifact-receipt", type=Path, required=True)
    p.add_argument("--pg-bin", type=Path, required=True)
    p.add_argument("--report", type=Path, required=True)
    args = p.parse_args(argv)
    if not all(x.is_absolute() for x in (args.repository_test, args.artifact_receipt, args.pg_bin, args.report)):
        p.error("absolute paths required")
    try:
        fd = os.open(args.report, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except OSError:
        p.error("report must be a new private writable file")
    report = {"scope": "uploader runtime native fixture regressions", "result": "FAIL", "commands": {},
              "canonical_auth": "NOT_RUN", "gateway_concert_http": "NOT_RUN", "cleanup": "NOT_RUN"}
    handlers = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            handlers[sig] = signal.signal(sig, interrupted)
        run(args, report)
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = type(error).__name__
        # Only our fixed errors are safe. Never expose arbitrary SQL/OS diagnostics.
        if type(error) is RuntimeError:
            report["failure_detail"] = str(error)
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({k: report.get(k) for k in ("result", "tests", "cleanup", "failure")}))
    return 0 if report["result"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
