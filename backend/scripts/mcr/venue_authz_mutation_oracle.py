#!/usr/bin/env python3
"""Removes one venue authorization guard at a time in a throwaway worktree and
requires the real-PostgreSQL matrix to flip at least three cells per mutant."""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

MUTANTS = {
    "location_visibility": (
        "backend/services/concert-service/src/venue_location/mod.rs",
        """        if !sharing.viewers.contains(&viewer.user_id.uuid()) {
            return Err(VenueLocationError::Denied);
        }
        if !self
            .authority
            .friend_in(&mut tx, viewer.event_id, viewer.user_id, owner, now)
            .await?
        {
            return Err(VenueLocationError::Denied);
        }
""",
        "",
    ),
    "notes_audience": (
        "backend/services/concert-service/src/venue_notes/mod.rs",
        """        if note.audience == NoteAudience::Everyone || note.author_id == viewer {
            return Ok(true);
        }
""",
        """        if note.audience == NoteAudience::Everyone || note.author_id == viewer {
            return Ok(true);
        }
        return Ok(true);
""",
    ),
    "meeting_friendship": (
        "backend/crates/concert-application/src/venue_meeting/mod.rs",
        "    authority.friend(event, actor, peer).await\n",
        """    let _ = (authority, event);
    Ok(PairFacts {
        relationship_revision: 1,
    })
""",
    ),
}


def run(cmd, cwd, env):
    result = subprocess.run(cmd, cwd=cwd, env=env, text=True, capture_output=True)
    return result.returncode, result.stdout + result.stderr


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--worktree", default=str(Path(__file__).resolve().parents[3]))
    parser.add_argument("--target-dir", required=True)
    parser.add_argument("--mutant", action="append", choices=sorted(MUTANTS))
    parser.add_argument("--minimum-flips", type=int, default=3)
    args = parser.parse_args()
    root = Path(args.worktree).resolve()
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = args.target_dir
    env["CARGO_BUILD_JOBS"] = "3"
    for name in ("CONCERT_TEST_DATABASE_URL", "CONCERT_TEST_OWNER_ROOT"):
        if name not in env:
            print(json.dumps({"result": "BLOCKED", "missing": name}))
            return 2
    selected = args.mutant or sorted(MUTANTS)
    temporary = Path(tempfile.mkdtemp(prefix="mcr-venue-authz-oracle-"))
    created = False
    report = {"result": "FAIL", "mutants": []}
    try:
        code, output = run(
            ["git", "worktree", "add", "--detach", str(temporary), "HEAD"],
            cwd=root,
            env=env,
        )
        if code != 0:
            report["error"] = output
            print(json.dumps(report, indent=2))
            return 1
        created = True
        for relative in [
            "backend/services/concert-service/tests/venue_authz_matrix.rs",
        ]:
            destination = temporary / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(root / relative, destination)
        for name in selected:
            relative, old, new = MUTANTS[name]
            path = temporary / relative
            source = path.read_text()
            if source.count(old) != 1:
                report["error"] = f"{name}: mutation anchor count {source.count(old)}"
                print(json.dumps(report, indent=2))
                return 1
            path.write_text(source.replace(old, new))
            code, output = run(
                [
                    "cargo",
                    "+1.88.0",
                    "test",
                    "-p",
                    "concert-service",
                    "--test",
                    "venue_authz_matrix",
                    "--",
                    "--test-threads=1",
                ],
                cwd=temporary / "backend",
                env=env,
            )
            failed = re.findall(r"^test (\S+) \.\.\. FAILED$", output, re.MULTILINE)
            summary = re.findall(
                r"test result: (?:FAILED|ok)\. (\d+) passed; (\d+) failed", output
            )
            passed, failures = (int(summary[-1][0]), int(summary[-1][1])) if summary else (0, 0)
            flipped = sorted(set(failed))
            report["mutants"].append(
                {
                    "mutant": name,
                    "exit": code,
                    "passed": passed,
                    "failed": failures,
                    "flipped": flipped,
                }
            )
            run(["git", "checkout", "--", "."], cwd=temporary, env=env)
            if failures < args.minimum_flips:
                report["error"] = f"{name} flipped {failures} < {args.minimum_flips}"
                print(json.dumps(report, indent=2))
                return 1
        report["result"] = "PASS"
        print(json.dumps(report, indent=2))
        return 0
    finally:
        if created:
            run(["git", "worktree", "remove", "--force", str(temporary)], cwd=root, env=env)


if __name__ == "__main__":
    raise SystemExit(main())
