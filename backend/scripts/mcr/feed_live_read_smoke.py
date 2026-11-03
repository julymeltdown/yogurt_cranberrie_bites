#!/usr/bin/env python3
"""Reproduce frozen feed adapter checks; never claim live owner/HTTP acceptance."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[3]
REQUIRED = {"feed_live_read": 8, "concert_feed_live_read": 8, "feed_wire": 3}

class PreflightError(Exception):
    pass

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def regular(path, executable=False):
    if not path.is_absolute() or path.resolve() != path or not path.is_file():
        raise PreflightError("canonical absolute regular artifact required")
    if "target" in path.parts or "mcr-build" in path.parts:
        raise PreflightError("copy artifacts outside build targets first")
    if executable and not os.access(path, os.X_OK):
        raise PreflightError("artifact is not executable")
    return path

def sources():
    paths = [p for p in (ROOT / "backend").rglob("*") if p.is_file()
             and p.suffix in (".rs", ".toml", ".sql", ".proto") and "target" not in p.parts]
    paths += [ROOT / "backend/Cargo.lock", ROOT / "contracts/generated/mcr-domain.openapi.json",
              ROOT / "contracts/fixtures/domain/public-post.json"]
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(set(paths))}

def artifacts(metadata):
    regular(metadata)
    if metadata.stat().st_size > 4 * 1024 * 1024:
        raise PreflightError("bounded metadata required")
    data = json.loads(metadata.read_text())
    if not isinstance(data, dict):
        raise PreflightError("object metadata required")
    if data.get("scope") != "adapter-only" or set(data.get("artifacts", {})) != set(REQUIRED):
        raise PreflightError("exact adapter-only artifact inventory required")
    if data.get("source_sha256") != sources():
        raise PreflightError("frozen source checkpoint mismatch")
    result = {}
    for name in REQUIRED:
        item = data["artifacts"][name]
        path = regular(Path(item["path"]), executable=True)
        if digest(path) != item["sha256"]:
            raise PreflightError("frozen binary hash mismatch")
        result[name] = path
    return data, result

def test_result(result, expected):
    match = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;", result.stdout)
    if result.returncode != 0 or not match or int(match[1]) != expected:
        raise PreflightError("exact unskipped adapter test inventory did not pass")
    return {"status": "PASS", "exit_code": result.returncode, "passed": expected,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest()}

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binaries-json", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--adapters-only", action="store_true", required=True,
                        help="live native/HTTP qualification is a separate unfinished obligation")
    args = parser.parse_args(argv)
    if not args.report.is_absolute() or args.report.parent.resolve() != args.report.parent:
        parser.error("new canonical absolute report path required")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    report = {"status": "FAIL", "scope": "adapter-only", "native_postgres": "NOT_RUN",
              "live_http": "NOT_RUN", "qualified_guard_integration": "NOT_RUN", "commands": {}}
    try:
        data, binaries = artifacts(args.binaries_json)
        report.update(source_commit=data["source_commit"], source_state=data["source_state"],
                      metadata_sha256=digest(args.binaries_json), runner_sha256=digest(Path(__file__).resolve()))
        for name, binary in binaries.items():
            result = subprocess.run([str(binary), "--test-threads=1"], cwd=ROOT,
                                    env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
                                    text=True, capture_output=True, timeout=60)
            report["commands"][name] = test_result(result, REQUIRED[name])
        report["status"] = "PASS"
        report["passed"] = sum(REQUIRED.values())
    except (PreflightError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        report["failure"] = str(error) if isinstance(error, PreflightError) else type(error).__name__
    finally:
        with os.fdopen(fd, "w") as out:
            json.dump(report, out, indent=2, sort_keys=True)
            out.write("\n")
    print(json.dumps({"status": report["status"], "scope": "adapter-only", "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
