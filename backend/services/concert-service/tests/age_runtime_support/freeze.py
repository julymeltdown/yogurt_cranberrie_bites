#!/usr/bin/env python3
"""Freeze exactly four successful Cargo JSON test artifacts before native work."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
from qualify import ROOT, source_manifest
NAMES = {"age_runtime", "age_claims", "age_claim_reader", "age_verification"}
def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()
def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-json", type=Path, action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.output.is_absolute() or args.output.exists():
        parser.error("new absolute private output directory required")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        parser.error("clean committed source required")
    artifacts = {}
    logs = {}
    for log in args.cargo_json:
        if not log.is_absolute() or not log.is_file() or log.is_symlink():
            parser.error("explicit regular Cargo JSON log required")
        finished = False
        for line in log.read_text().splitlines():
            row = json.loads(line)
            if row.get("reason") == "build-finished":
                finished = row.get("success") is True
            if row.get("reason") == "compiler-artifact" and row["target"]["name"] in NAMES and row.get("executable"):
                name = row["target"]["name"]
                if name in artifacts or "test" not in row["target"]["kind"] or row.get("fresh") is not False:
                    parser.error("fresh unambiguous test artifacts required")
                artifacts[name] = row
        if not finished:
            parser.error("successful completed Cargo log required")
        logs[str(log)] = sha(log)
    if set(artifacts) != NAMES:
        parser.error("exact four test artifacts required")
    args.output.mkdir(mode=0o700)
    sources = source_manifest()
    frozen = {}
    for name, row in artifacts.items():
        source = Path(row["executable"])
        if source.is_symlink() or not source.is_file():
            parser.error("regular emitted executable required")
        before = sha(source)
        copy = args.output / name
        shutil.copyfile(source, copy)
        copy.chmod(0o500)
        if sha(copy) != before or sha(source) != before:
            parser.error("artifact changed during freeze")
        frozen[name] = {"binary":str(copy), "binary_sha256":before,
                        "emitted_executable":str(source), "fresh":row["fresh"], "package_id":row["package_id"]}
    if sources != source_manifest():
        parser.error("source changed during freeze")
    checkpoint = {"source_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
                  "source_sha256":sources,"cargo_logs":logs, **frozen["age_runtime"],
                  "retained":{k:v for k,v in frozen.items() if k!="age_runtime"},
                  "limitations":["Build artifacts only. Release the allocated target before native qualification."]}
    (args.output / "checkpoint.json").write_text(json.dumps(checkpoint,indent=2,sort_keys=True)+"\n")
    print(json.dumps({"checkpoint":str(args.output/"checkpoint.json"),"source_commit":checkpoint["source_commit"],
                      "artifacts":{k:v["binary_sha256"] for k,v in frozen.items()}}))
if __name__ == "__main__":
    main()
