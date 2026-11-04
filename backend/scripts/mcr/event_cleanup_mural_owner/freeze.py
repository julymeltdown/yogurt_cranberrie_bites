#!/usr/bin/env python3
"""Capture one exact Cargo test artifact under an accepted named target lease.

Adapted from event_cleanup/freeze.py at 0bd35b223d2590fb2b343171d6d8ac30d37ff857.
No Cargo invocation, target scan or PostgreSQL launch is performed here.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[4]
PIN_PATHS = [
    "backend/services/concert-service/src/event_cleanup_mural_owner/mod.rs",
    "backend/services/concert-service/tests/event_cleanup_mural_owner.rs",
    "backend/services/concert-service/tests/event_cleanup_mural_owner_support/mod.rs",
]


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def snapshot():
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "backend", "contracts", "AGENTS.md"], cwd=ROOT
    ).decode().split("\0")
    files = {p: sha(ROOT / p) for p in sorted(set(paths)) if p and (ROOT / p).is_file()}
    pin = hashlib.sha256()
    for path in PIN_PATHS:
        pin.update((ROOT / path).read_bytes())
        pin.update(b"\0")
    return {
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "files": files,
        "owned_source_pin": pin.hexdigest(),
    }


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--snapshot", type=Path)
    p.add_argument("--before", type=Path)
    p.add_argument("--cargo-json", type=Path)
    p.add_argument("--out", type=Path)
    p.add_argument("--accepted-offer")
    a = p.parse_args()
    if a.snapshot:
        a.snapshot.write_text(json.dumps(snapshot(), indent=2) + "\n")
        return
    if not (a.before and a.cargo_json and a.out and a.accepted_offer):
        p.error("freeze requires --before, --cargo-json, --out and --accepted-offer")
    source = json.loads(a.before.read_text())
    assert source == snapshot(), "source changed during build"
    candidate = None
    success = False
    for line in a.cargo_json.read_text().splitlines():
        row = json.loads(line)
        if row.get("reason") == "build-finished":
            success = row["success"]
        if row.get("reason") == "compiler-artifact" and row.get("target", {}).get("name") == "event_cleanup_mural_owner" and row.get("profile", {}).get("test") and row.get("executable"):
            assert Path(row["target"]["src_path"]).resolve() == (ROOT / PIN_PATHS[1]).resolve()
            assert row["profile"].get("debuginfo") in (None, 0)
            candidate = Path(row["executable"]).resolve()
    assert success and candidate and candidate.is_file(), "missing successful exact test artifact"
    a.out.mkdir(mode=0o700)
    (a.out / "bin").mkdir()
    dest = a.out / "bin" / "event_cleanup_mural_owner"
    shutil.copy2(candidate, dest)
    st = dest.stat()
    shutil.copy2(a.cargo_json, a.out / "cargo.jsonl")
    (a.out / "migrations").mkdir()
    migrations = {}
    for src in sorted((ROOT / "backend/services/concert-service/migrations").glob("*.sql")):
        out = a.out / "migrations" / src.name
        shutil.copy2(src, out)
        migrations[src.name] = sha(out)
    receipt = {
        "accepted_offer": a.accepted_offer, "source": source,
        "binary": {"sha256": sha(dest), "size": st.st_size, "device": st.st_dev, "inode": st.st_ino},
        "migrations": migrations, "cargo_json_sha256": sha(a.out / "cargo.jsonl"),
        "build_environment": {k: os.environ.get(k) for k in ["CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "CARGO_INCREMENTAL", "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_TEST_DEBUG", "RUSTFLAGS"]},
    }
    (a.out / "freeze.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"frozen": str(a.out), "binary": receipt["binary"], "owned_source_pin": source["owned_source_pin"]}))


if __name__ == "__main__":
    main()
