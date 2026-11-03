#!/usr/bin/env python3
"""Verify the imported planning baseline; never certify application behavior."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]
ARCHIVE = ROOT / "docs/source/mcr_v2_en"
EXPORT_FILES = (
    "TASKS_EN.json", "IA_EN.json", "DEPENDENCIES_EN.json",
    "VALIDATION_EN.json", "DEVELOPER_TASKS_EN.md",
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*args: str, cwd: Path) -> None:
    result = subprocess.run(
        [sys.executable, *args], cwd=cwd, capture_output=True, text=True,
    )
    require(result.returncode == 0, result.stdout + result.stderr)


def check() -> dict:
    checks: list[str] = []
    checksums = json.loads((ARCHIVE / "SHA256SUMS_EN.json").read_text())
    require(len(checksums) == 13, "Unexpected archive checksum inventory")
    for name, expected in checksums.items():
        require(digest(ARCHIVE / name) == expected, f"Source changed: {name}")
    checks.append("13 original archive checksums match")

    provenance = json.loads((ROOT / "docs/mcr/source-provenance.json").read_text())
    narrative = ROOT / provenance["narrative"]["preserved_path"]
    require(digest(narrative) == provenance["narrative"]["sha256"], "Upstream narrative changed")
    checks.append("pinned upstream narrative checksum matches")

    spec = importlib.util.spec_from_file_location("mcr_plan", ROOT / "MCR_EXPORT_PLAN_EN.py")
    require(spec is not None and spec.loader is not None, "Exporter cannot load")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    baseline = None
    for source in (ROOT / "MCR_IMPLEMENTATION_PLAN_V2_EN.md", ROOT / "PLAN_TABLES_EN.md", narrative, ARCHIVE / "PLAN_TABLES_EN.md"):
        text = source.read_text(encoding="utf-8")
        tasks, screens = module.parse(text)
        module.validate(text, tasks, screens)
        parsed = (tasks, screens)
        if baseline is None:
            baseline = parsed
        require(parsed == baseline, f"Task/IA content differs: {source}")
    checks.append("active and original narrative/table task and IA content match")

    with tempfile.TemporaryDirectory(prefix="mcr-plan-check-") as directory:
        temp = Path(directory)
        for label, source_dir in (("original", ARCHIVE), ("active", ROOT)):
            work = temp / label
            work.mkdir()
            (work / "generated").mkdir()
            for name in ("MCR_EXPORT_PLAN_EN.py", "TEST_PLAN_EXPORT_EN.py", "PLAN_TABLES_EN.md"):
                shutil.copyfile(source_dir / name, work / name)
            run("TEST_PLAN_EXPORT_EN.py", cwd=work)
            report = json.loads((work / "generated/EXPORTER_TESTS_EN.json").read_text())
            require(report["result"] == "PASS" and report["test_count"] == 9, f"{label} regression probes failed")
            require(all(t["result"] == "PASS" for t in report["tests"]), f"{label} has failing probes")
            require((work / "generated/EXPORTER_TESTS_EN.json").read_bytes() == (source_dir / "generated/EXPORTER_TESTS_EN.json").read_bytes(), f"Stale {label} regression report")
            source = source_dir / ("PLAN_TABLES_EN.md" if label == "original" else "MCR_IMPLEMENTATION_PLAN_V2_EN.md")
            run("MCR_EXPORT_PLAN_EN.py", "--source", str(source), "--output-dir", "generated", cwd=work)
            for name in EXPORT_FILES:
                require((work / "generated" / name).read_bytes() == (source_dir / "generated" / name).read_bytes(), f"Stale {label} export: {name}")
            checks.append(f"{label}: nine regression probes pass and all committed exports reproduce")

    tasks = json.loads((ROOT / "generated/TASKS_EN.json").read_text())
    for name in ("TASKS_EN.json", "IA_EN.json", "DEPENDENCIES_EN.json"):
        metadata = json.loads((ROOT / "generated" / name).read_text())["metadata"]
        require(metadata["repository"] == "julymeltdown/mcr" and metadata["branch"] == "master", f"Wrong delivery destination: {name}")
    require(all(t["target_repository"] == "julymeltdown/mcr" and t["target_branch"] == "master" for t in tasks["tasks"]), "Wrong task delivery destination")
    require(all(t["status"] == "NOT_STARTED" for t in tasks["tasks"]), "Planning baseline falsely claims implementation")
    checks.append("all active metadata and 118 task destinations use mcr/master; baseline remains NOT_STARTED")

    dependencies = json.loads((ROOT / "generated/DEPENDENCIES_EN.json").read_text())
    required = {t["id"] for t in tasks["tasks"] if t["priority"] == "P0"}
    require(len(required) == 112, "Wrong required task count")
    require(set(dependencies["release_required_tasks_before_IN05"]) == required - {"IN05"}, "Incomplete release task checklist")
    require(len(dependencies["release_required_ia"]) == 61, "Incomplete required IA checklist")
    checks.append("release checklist contains 111 prerequisites plus IN05 and 61 required IA entries")
    return {
        "scope": "Planning provenance, structure, exporter regression, and delivery routing only",
        "result": "PASS", "python_version": sys.version.split()[0], "checks": checks,
        "task_count": 118, "ia_count": 64,
        "application_build": "NOT_RUN", "application_tests": "NOT_RUN", "deployment": "NOT_RUN",
        "input_sha256": {str(p.relative_to(ROOT)): digest(p) for p in (
            ROOT / "MCR_IMPLEMENTATION_PLAN_V2_EN.md", ROOT / "PLAN_TABLES_EN.md",
            ROOT / "MCR_EXPORT_PLAN_EN.py", ROOT / "TEST_PLAN_EXPORT_EN.py",
            ROOT / "scripts/mcr/check-plan.py", ROOT / "docs/mcr/source-provenance.json",
        )},
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    try:
        report = check()
    except (OSError, ValueError, KeyError) as error:
        print(f"PLANNING CHECK FAILED: {error}", file=sys.stderr)
        return 1
    output = json.dumps(report, indent=2) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(output, encoding="utf-8")
    print(output, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
