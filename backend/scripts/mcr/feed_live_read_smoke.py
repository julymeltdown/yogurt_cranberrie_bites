#!/usr/bin/env python3
"""Reproduce frozen adapter or Post storage checks; never claim joined HTTP acceptance."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from urllib.parse import parse_qs, urlsplit

ROOT = Path(__file__).resolve().parents[3]
REQUIRED = {"feed_live_read": 9, "concert_feed_live_read": 8, "feed_wire": 3}
NATIVE_CASES = {
    "concert_feed::read_tracking::tests::native_lost_track_ack_uses_actual_aborted_transaction_and_exact_owner_binding",
    "concert_feed::read_tracking::tests::native_committed_transaction_without_exact_same_transaction_witness_stays_unknown",
    "concert_feed::read_tracking::tests::native_both_durable_acknowledgements_and_exact_committed_witness_are_required",
    "concert_feed::read_tracking::tests::native_expiry_reclaims_only_proven_terminal_work_never_a_live_or_unknown_transaction",
    "concert_feed::read_tracking::tests::native_comment_subject_budget_and_filtered_boundary_do_not_skip_or_duplicate_records",
}

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

def artifacts(metadata, native=False):
    regular(metadata)
    if metadata.stat().st_size > 4 * 1024 * 1024:
        raise PreflightError("bounded metadata required")
    data = json.loads(metadata.read_text())
    if not isinstance(data, dict):
        raise PreflightError("object metadata required")
    expected = {"post_service_lib"} if native else set(REQUIRED)
    scope = "post-native-storage" if native else "adapter-only"
    if data.get("scope") != scope or set(data.get("artifacts", {})) != expected:
        raise PreflightError("exact scoped artifact inventory required")
    if data.get("source_sha256") != sources():
        raise PreflightError("frozen source checkpoint mismatch")
    result = {}
    for name in expected:
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

def native_environment(path):
    regular(path)
    if path.stat().st_size > 4096 or path.stat().st_mode & 0o077 or path.stat().st_uid != os.getuid():
        raise PreflightError("bounded owner-only native configuration required")
    try:
        data = json.loads(path.read_text())
        if set(data) != {"url", "root", "identity", "token"}:
            raise ValueError()
        root = Path(data["root"])
        info = root.lstat()
        marker = regular(root / "mcr-owner.json")
        socket = root / "socket"
        if (not root.is_absolute() or root.resolve() != root or not root.is_dir()
                or info.st_uid != os.getuid() or [info.st_dev, info.st_ino] != data["identity"]
                or json.loads(marker.read_text()) != {"token": data["token"], "uid": os.getuid()}
                or not socket.is_dir() or socket.resolve() != socket):
            raise ValueError()
        url = urlsplit(data["url"])
        query = parse_qs(url.query, keep_blank_values=True)
        if (url.scheme != "postgresql" or url.hostname != "localhost" or url.port is not None
                or url.username != "mcr_owner" or not url.password or url.path != "/mcr_feed_native"
                or url.fragment or query != {"host": [str(socket)], "port": ["5432"], "sslmode": ["disable"]}):
            raise ValueError()
    except (ValueError, TypeError, KeyError):
        raise PreflightError("exact owned socket-only native controller required") from None
    return {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "MCR_FEED_NATIVE_DATABASE_URL": data["url"],
            "MCR_FEED_NATIVE_OWNER_ROOT": str(root), "MCR_FEED_NATIVE_OWNER_TOKEN": data["token"],
            "MCR_FEED_NATIVE_OWNER_IDENTITY": ":".join(map(str, data["identity"]))}

def native_checks(binary, environment):
    inventory = subprocess.run([str(binary), "--list", "--ignored", "concert_feed::read_tracking::tests::"],
                               cwd=ROOT, env=environment, text=True, capture_output=True, timeout=10)
    names = {line.removesuffix(": test") for line in inventory.stdout.splitlines() if line.endswith(": test")}
    if inventory.returncode or names != NATIVE_CASES:
        raise PreflightError("exact native storage inventory required")
    results = {}
    for name in sorted(NATIVE_CASES):
        r = subprocess.run([str(binary), "--ignored", "--exact", name, "--nocapture", "--test-threads=1"],
                           cwd=ROOT, env=environment, text=True, capture_output=True, timeout=60)
        match = re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; \d+ filtered out;", r.stdout)
        if r.returncode or not match or f"test {name} ..." not in r.stdout:
            raise PreflightError("native storage case did not execute successfully")
        results[name] = {"status": "PASS", "passed": 1, "stdout_sha256": hashlib.sha256(r.stdout.encode()).hexdigest()}
    return results

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binaries-json", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--adapters-only", action="store_true",
                        help="live native/HTTP qualification is a separate unfinished obligation")
    mode.add_argument("--post-native-storage", action="store_true",
                      help="actual leased PostgreSQL storage only; fixture acknowledgements are not live owner qualification")
    parser.add_argument("--native-config", type=Path,
                        help="private JSON binding the owned controller marker, inode, socket and database URL")
    args = parser.parse_args(argv)
    if not args.report.is_absolute() or args.report.parent.resolve() != args.report.parent:
        parser.error("new canonical absolute report path required")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    scope = "post-native-storage" if args.post_native_storage else "adapter-only"
    report = {"status": "FAIL", "scope": scope, "native_postgres": "NOT_RUN",
              "live_http": "NOT_RUN", "qualified_guard_integration": "NOT_RUN", "commands": {}}
    try:
        data, binaries = artifacts(args.binaries_json, native=args.post_native_storage)
        report.update(source_commit=data["source_commit"], source_state=data["source_state"],
                      metadata_sha256=digest(args.binaries_json), runner_sha256=digest(Path(__file__).resolve()))
        if args.post_native_storage:
            if args.native_config is None:
                raise PreflightError("explicit leased native configuration required")
            report["commands"] = native_checks(binaries["post_service_lib"], native_environment(args.native_config))
            report["native_postgres"] = "PASS_STORAGE_ONLY"
            report["passed"] = len(NATIVE_CASES)
        else:
            if args.native_config is not None:
                raise PreflightError("native configuration is not an adapter input")
            for name, binary in binaries.items():
                result = subprocess.run([str(binary), "--test-threads=1"], cwd=ROOT,
                                        env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
                                        text=True, capture_output=True, timeout=60)
                report["commands"][name] = test_result(result, REQUIRED[name])
            report["passed"] = sum(REQUIRED.values())
        report["status"] = "PASS"
    except (PreflightError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        report["failure"] = str(error) if isinstance(error, PreflightError) else type(error).__name__
    finally:
        with os.fdopen(fd, "w") as out:
            json.dump(report, out, indent=2, sort_keys=True)
            out.write("\n")
    print(json.dumps({"status": report["status"], "scope": scope, "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
