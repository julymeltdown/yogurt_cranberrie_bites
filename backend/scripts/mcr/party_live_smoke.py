#!/usr/bin/env python3
"""Qualify frozen party_live adapter tests without Cargo or external databases.

The controller proves the exact dedicated test executables and focused C04 parser,
cursor and domain fixture behavior. Native PostgreSQL, canonical Auth/Friend/Profile
holders, Product routing and actual HTTP remain separate required gates.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[3]
REQUIRED = {"party_live_cursor": 3, "party_live_owner": 4, "party_live_wire": 7}


class PreflightError(Exception):
    pass


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def regular(path: Path, executable: bool = False) -> Path:
    if not path.is_absolute() or path.resolve() != path or not path.is_file():
        raise PreflightError("canonical absolute regular artifact required")
    if path.is_symlink() or "target" in path.parts or "mcr-build" in path.parts:
        raise PreflightError("copy artifacts outside build targets first")
    if executable and not os.access(path, os.X_OK):
        raise PreflightError("artifact is not executable")
    return path


def sources() -> dict[str, str]:
    roots = [
        ROOT / "backend/services/concert-service/src/party_live",
        ROOT / "backend/crates/concert-domain/src/community",
        ROOT / "backend/crates/concert-application/src/community",
        ROOT / "backend/services/concert-service/src/community",
    ]
    paths = {
        path
        for root in roots
        for path in root.rglob("*.rs")
        if path.is_file()
    }
    paths.update(
        [
            ROOT / "backend/services/concert-service/src/lib.rs",
            ROOT / "backend/services/concert-service/tests/party_live_cursor.rs",
            ROOT / "backend/services/concert-service/tests/party_live_owner.rs",
            ROOT / "backend/services/concert-service/tests/party_live_wire.rs",
            ROOT / "backend/Cargo.lock",
            ROOT / "contracts/generated/mcr-domain.openapi.json",
            ROOT / "contracts/domain/v1/party.schema.json",
            ROOT / "contracts/domain/v1/operations.json",
            Path(__file__).resolve(),
        ]
    )
    return {
        str(path.relative_to(ROOT)): digest(path)
        for path in sorted(paths)
        if path.is_file()
    }


def artifacts(metadata: Path) -> tuple[dict, dict[str, Path]]:
    regular(metadata)
    if metadata.stat().st_size > 4 * 1024 * 1024:
        raise PreflightError("bounded metadata required")
    data = json.loads(metadata.read_text())
    if (
        not isinstance(data, dict)
        or data.get("scope") != "adapter-only"
        or set(data.get("artifacts", {})) != set(REQUIRED)
    ):
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


def test_result(result: subprocess.CompletedProcess[str], expected: int) -> dict:
    match = re.search(
        r"test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
        result.stdout,
    )
    if result.returncode != 0 or not match or int(match[1]) != expected:
        raise PreflightError("exact unskipped adapter test inventory did not pass")
    return {
        "status": "PASS",
        "exit_code": result.returncode,
        "passed": expected,
        "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binaries-json", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute() or args.report.parent.resolve() != args.report.parent:
        parser.error("new canonical absolute report path required")
    try:
        descriptor = os.open(
            args.report,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL,
            0o600,
        )
    except OSError:
        parser.error("report must be a new writable file")
    report = {
        "status": "FAIL",
        "scope": "adapter-only",
        "native_postgres": "NOT_RUN",
        "canonical_owner_holder": "NOT_RUN",
        "public_profile_holder": "NOT_RUN",
        "product_route": "NOT_RUN",
        "live_http": "NOT_RUN",
        "commands": {},
    }
    try:
        data, binaries = artifacts(args.binaries_json)
        report.update(
            source_commit=data["source_commit"],
            source_state=data["source_state"],
            metadata_sha256=digest(args.binaries_json),
            runner_sha256=digest(Path(__file__).resolve()),
        )
        for name, binary in binaries.items():
            result = subprocess.run(
                [str(binary), "--test-threads=1"],
                cwd=ROOT,
                env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
                text=True,
                capture_output=True,
                timeout=60,
            )
            report["commands"][name] = test_result(result, REQUIRED[name])
        report["status"] = "PASS"
        report["passed"] = sum(REQUIRED.values())
    except (PreflightError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        report["failure"] = (
            str(error)
            if isinstance(error, PreflightError)
            else type(error).__name__
        )
    finally:
        with os.fdopen(descriptor, "w") as output:
            json.dump(report, output, indent=2, sort_keys=True)
            output.write("\n")
    print(
        json.dumps(
            {
                "status": report["status"],
                "scope": "adapter-only",
                "report": str(args.report),
            }
        )
    )
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
