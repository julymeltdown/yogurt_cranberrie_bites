#!/usr/bin/env python3
"""Run only the copied denial tests inside an already accepted finite lease.

The allocator's continuous guard must enclose this process. This script grants
no allocation, starts no database, copies nothing and never accesses a target.
Its result is not actual moderation, World or browser acceptance.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=pathlib.Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--allocation-id", required=True)
    parser.add_argument("--deadline-unix", type=float, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if "target" in binary.parts or binary.is_symlink():
        parser.error("only a retained copied test executable is supported")
    if len(args.source_revision) != 40 or any(c not in "0123456789abcdef" for c in args.source_revision):
        parser.error("an exact source commit is required")
    digest = hashlib.file_digest(binary.open("rb"), "sha256").hexdigest()
    if digest != args.binary_sha256:
        parser.error("copied executable hash mismatch")
    results = []
    for name in (
        "snapshot_role_cannot_start_or_recover_a_social_revocation",
        "unsuspend_or_a_different_step_never_revokes_or_restores_social_authority",
    ):
        remaining = args.deadline_unix - time.time()
        if remaining <= 0:
            parser.error("original allocation deadline expired")
        proc = subprocess.run(
            [str(binary), "--exact", name, "--test-threads=1"],
            capture_output=True, text=True, timeout=min(remaining, 30), check=False,
        )
        passed = proc.returncode == 0 and "1 passed; 0 failed" in proc.stdout
        results.append({"name": name, "exit_code": proc.returncode,
                        "passed": passed, "stdout": proc.stdout, "stderr": proc.stderr})
        if not passed:
            break
    report = {"allocation": args.allocation_id, "source_revision": args.source_revision,
              "binary_sha256": digest, "scope": "DENIAL_COMPONENT_ONLY",
              "actual_moderation": "NOT_RUN", "tests": results}
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    raise SystemExit(0 if len(results) == 2 and all(r["passed"] for r in results) else 1)


if __name__ == "__main__":
    main()
