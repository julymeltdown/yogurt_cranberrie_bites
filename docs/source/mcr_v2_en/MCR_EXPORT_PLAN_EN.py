#!/usr/bin/env python3
"""Validate and export the MCR English plan; no third-party dependencies.

Input may be the canonical full specification or PLAN_TABLES_EN.md.
This validates planning structure only. It never executes application tests.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter
from pathlib import Path
from typing import Any

TASK_ID = re.compile(r"(?:C|AR|BE|FE|QA|IN|EX)\d{2}\Z")
IA_ID = re.compile(r"(?:A|W|F|M|G|R|U|X|S|O)\d{2}\Z")
PATH_ALIASES = {
    "CD": "backend/crates/concert-domain/src",
    "CA": "backend/crates/concert-application/src",
    "CS": "backend/services/concert-service/src",
    "GW": "backend/services/api-gateway/src",
    "FS": "frontend/src",
    "CT": "contracts",
    "TEST": "backend/services/concert-service/tests",
}
OPTIONAL_IA = {"M10", "R02", "X02"}
REPOSITORY = "julymeltdown/bigyeting"
BRANCH = "master"


class PlanError(ValueError):
    """The plan cannot safely be exported."""


def split_ids(value: str) -> list[str]:
    return [] if value.strip() == "-" else [s.strip() for s in value.split(",")]


def expand_path(value: str) -> str:
    path = value.strip().strip("`")
    root, slash, tail = path.partition("/")
    return PATH_ALIASES.get(root, root) + (slash + tail if slash else "")


def parse(text: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    tasks: list[dict[str, Any]] = []
    screens: list[dict[str, Any]] = []
    for number, line in enumerate(text.splitlines(), start=1):
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        identifier = cells[0]
        if TASK_ID.fullmatch(identifier):
            if len(cells) != 8:
                raise PlanError(f"Line {number}: task needs 8 columns, got {len(cells)}")
            if not re.fullmatch(r"P[0-5]", cells[1]):
                raise PlanError(f"Line {number}: invalid phase {cells[1]!r}")
            tasks.append({
                "id": identifier,
                "title": cells[4],
                "phase": int(cells[1][1:]),
                "priority": "P1" if identifier.startswith("EX") else "P0",
                "lane": cells[2],
                "depends_on": split_ids(cells[3]),
                "owned_paths": [expand_path(p) for p in cells[5].split(";")],
                "implementation": cells[6],
                "acceptance_evidence_required": cells[7],
                "status": "NOT_STARTED",
                "target_repository": REPOSITORY,
                "target_branch": BRANCH,
                "ia_ids": [],
            })
        elif IA_ID.fullmatch(identifier):
            if len(cells) != 5:
                raise PlanError(f"Line {number}: IA needs 5 columns, got {len(cells)}")
            screens.append({
                "id": identifier,
                "route": cells[1].strip("`"),
                "label": cells[2],
                "behavior_and_states": cells[3],
                "task_ids": split_ids(cells[4]),
                "priority": "P1" if identifier in OPTIONAL_IA else "P0",
            })
    return tasks, screens


def validate(
    text: str,
    tasks: list[dict[str, Any]],
    screens: list[dict[str, Any]],
    expected_tasks: int = 118,
    expected_screens: int = 64,
) -> list[list[str]]:
    if len(tasks) != expected_tasks or len(screens) != expected_screens:
        raise PlanError(
            f"Expected {expected_tasks} tasks/{expected_screens} IA entries; "
            f"found {len(tasks)}/{len(screens)}"
        )
    if re.search(r"[\u1100-\u11ff\u3130-\u318f\uac00-\ud7af]", text):
        raise PlanError("English planning source contains Hangul characters")
    for kind, rows in (("task", tasks), ("IA", screens)):
        repeated = [key for key, count in Counter(r["id"] for r in rows).items() if count > 1]
        if repeated:
            raise PlanError(f"Duplicate {kind} IDs: {repeated}")
    by_id = {task["id"]: task for task in tasks}
    for task in tasks:
        for name in ("title", "lane", "implementation", "acceptance_evidence_required"):
            if not task[name]:
                raise PlanError(f"{task['id']}: missing {name}")
        if not task["owned_paths"] or any(not p for p in task["owned_paths"]):
            raise PlanError(f"{task['id']}: missing file ownership")
        if len(task["depends_on"]) != len(set(task["depends_on"])):
            raise PlanError(f"{task['id']}: duplicate dependency")
        for dependency in task["depends_on"]:
            if dependency not in by_id:
                raise PlanError(f"{task['id']}: unknown dependency {dependency}")
            if dependency == task["id"]:
                raise PlanError(f"{task['id']}: self dependency")
            if task["id"].startswith("FE") and dependency.startswith("BE"):
                raise PlanError(f"{task['id']}: frontend waits on backend {dependency}")
    for screen in screens:
        if not screen["task_ids"]:
            raise PlanError(f"{screen['id']}: no implementation coverage")
        for identifier in screen["task_ids"]:
            if identifier not in by_id:
                raise PlanError(f"{screen['id']}: unknown task {identifier}")
            by_id[identifier]["ia_ids"].append(screen["id"])
        if screen["priority"] == "P0" and all(
            by_id[i]["priority"] != "P0" for i in screen["task_ids"]
        ):
            raise PlanError(f"{screen['id']}: launch screen covered only by optional tasks")
    remaining = {task["id"]: set(task["depends_on"]) for task in tasks}
    waves: list[list[str]] = []
    while remaining:
        ready = sorted(identifier for identifier, deps in remaining.items() if not deps)
        if not ready:
            raise PlanError(f"Dependency cycle among: {', '.join(sorted(remaining))}")
        waves.append(ready)
        for identifier in ready:
            del remaining[identifier]
        for dependencies in remaining.values():
            dependencies.difference_update(ready)
    return waves


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def export(source: Path, output: Path, expected_tasks: int, expected_screens: int) -> dict[str, Any]:
    text = source.read_text(encoding="utf-8")
    tasks, screens = parse(text)
    waves = validate(text, tasks, screens, expected_tasks, expected_screens)
    digest = hashlib.sha256(text.encode("utf-8")).hexdigest()
    output.mkdir(parents=True, exist_ok=True)
    metadata = {
        "schema_version": "2.0",
        "language": "en",
        "repository": REPOSITORY,
        "branch": BRANCH,
        "source_file": source.name,
        "source_sha256": digest,
        "execution_state": "PLANNING_ONLY",
        "priority_note": "P0 is launch-required, P1 is optional; phase is a separate integer.",
        "release_gate_note": (
            "Dependencies are scheduling prerequisites, not the complete release checklist. "
            "IN05 must review every launch-required task and IA entry. Any scope cut must be "
            "explicitly recorded; removing a dependency does not waive acceptance."
        ),
    }
    required = [t["id"] for t in tasks if t["priority"] == "P0" and t["id"] != "IN05"]
    write_json(output / "TASKS_EN.json", {"metadata": metadata, "tasks": tasks})
    write_json(output / "IA_EN.json", {"metadata": metadata, "screens": screens})
    write_json(output / "DEPENDENCIES_EN.json", {
        "metadata": metadata,
        "edges": [{"from": dep, "to": t["id"]} for t in tasks for dep in t["depends_on"]],
        "topological_waves": waves,
        "waves_note": "Do not impose wave barriers; a task starts as soon as its own prerequisites pass.",
        "release_required_tasks_before_IN05": required,
        "release_required_ia": [s["id"] for s in screens if s["priority"] == "P0"],
    })
    cards = [
        "# MCR Developer Task Cards — English", "",
        "These are implementation specifications, not completed work. All task statuses are NOT_STARTED.",
        "Each unit targets julymeltdown/bigyeting on master. All rendering must be newly authored.",
        "Scheduling prerequisites do not replace the full launch acceptance checklist.", "",
    ]
    for task in tasks:
        cards.extend([
            f"## {task['id']} — {task['title']}", "",
            f"**Phase:** {task['phase']} | **Priority:** {task['priority']} | **Lane:** {task['lane']}",
            f"**Status:** {task['status']}",
            f"**Dependencies:** {', '.join(task['depends_on']) or 'None'}",
            f"**IA coverage:** {', '.join(task['ia_ids']) or 'Cross-cutting implementation'}", "",
            "### Owned implementation paths", "",
            *[f"- `{p}`" for p in task["owned_paths"]], "",
            "### Implementation", "", task["implementation"], "",
            "### Required acceptance evidence", "", task["acceptance_evidence_required"], "",
            "### Delivery record", "",
            "Record contract/source revisions, commands and exit codes, evidence paths, "
            "known limitations, and the verified remote commit on master. "
            "A mock-only journey is not real-server acceptance.", "",
        ])
    (output / "DEVELOPER_TASKS_EN.md").write_text("\n".join(cards), encoding="utf-8")
    result = {
        "validation_scope": "Planning structure only; application tests were not run.",
        "result": "PASS",
        "task_count": len(tasks),
        "ia_count": len(screens),
        "task_counts_by_prefix": dict(sorted(Counter(re.sub(r"\d+$", "", t["id"]) for t in tasks).items())),
        "task_counts_by_priority": dict(Counter(t["priority"] for t in tasks)),
        "ia_counts_by_priority": dict(Counter(s["priority"] for s in screens)),
        "dependency_edges": sum(len(t["depends_on"]) for t in tasks),
        "topological_wave_count": len(waves),
        "checks": ["nonempty_expected_counts", "unique_ids", "resolved_dependencies", "acyclic_graph",
                   "all_ia_references_resolved", "all_ia_covered", "frontend_not_blocked_by_backend",
                   "implementation_paths_and_acceptance_present", "english_source_no_hangul"],
        "source_sha256": digest,
    }
    write_json(output / "VALIDATION_EN.json", result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("MCR_IMPLEMENTATION_PLAN_V2_EN.md"))
    parser.add_argument("--output-dir", type=Path, default=Path("mcr-plan-export"))
    parser.add_argument("--expected-tasks", type=int, default=118)
    parser.add_argument("--expected-ia", type=int, default=64)
    args = parser.parse_args()
    try:
        result = export(args.source, args.output_dir, args.expected_tasks, args.expected_ia)
    except (OSError, PlanError) as exc:
        print(f"PLAN VALIDATION FAILED: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
