"""Same-candidate composition over original catalogs, never an acceptance ledger."""
import re
import subprocess
from pathlib import PurePosixPath

import check as source
from .reports import CATEGORIES, closed, evidence, names

SCHEMA = "mcr.qa10.release-composition.v1"
AUTHORITY = "docs/mcr/functional-delivery-plan.md"
GATES = {
    "BE43": {"native", "live"}, "QA01": {"unit", "native"},
    "QA02": {"device", "operator"}, "QA03": {"native"}, "QA04": {"live"},
    "QA05": {"live"}, "QA06": {"device"}, "QA07": {"live", "device"},
    "QA09": {"native", "live"}, "IN01": {"live"}, "IN02": {"live"},
    "IN03": {"live"}, "IN04": {"live"},
}
STATES = {"ACCEPTED", "NOT_RUN", "BLOCKED", "IN_PROGRESS", "INTEGRATED", "DEFERRED_BY_USER"}


def catalog(repo, expected, path, key, count):
    data = repo.git("show", expected + ":" + path)
    rows = source.decode(data)[key]
    source.require(isinstance(rows, list), "catalog.shape")
    all_ids = [r["id"] for r in rows]
    source.require(len(set(all_ids)) == len(all_ids), "catalog.duplicates")
    required = {r["id"]: r for r in rows if r["priority"] == "P0"}
    source.require(len(required) == count, "catalog.original-scope-mismatch")
    return required, {"path": path, "sha256": source.digest(data)}


def coverage_records(values, allowed, rule):
    source.require(isinstance(values, list) and len(values) <= len(allowed), rule + ".shape")
    result = {}
    for record in values:
        closed(record, ("id", "status", "owner", "reason", "commit", "paths", "receipt_ids"), rule + ".record")
        identity = record["id"]
        source.require(isinstance(identity, str) and identity in allowed and identity not in result,
                       rule + ".unknown-or-duplicate")
        source.require(record["status"] in STATES and isinstance(record["owner"], str)
                       and record["owner"] and isinstance(record["reason"], str) and record["reason"],
                       rule + ".status-or-owner")
        source.require(record["status"] != "DEFERRED_BY_USER" or identity == "QA08",
                       "deferral.unauthorized-scope-cut")
        result[identity] = record
    return result


def owned(path, patterns):
    for pattern in patterns:
        first, separator, tail = pattern.partition("/")
        prefix = source.ALIASES.get(first, first) + (separator + tail if separator else "")
        if path == prefix or prefix.endswith("/") and path.startswith(prefix):
            return True
    return False


def binding(repo, record, expected, receipts, coverage_kind):
    commit = record["commit"]
    source.require(isinstance(commit, str) and source.HEX40.fullmatch(commit), "binding.commit")
    repo.git("merge-base", "--is-ancestor", commit, expected)
    paths = names(record["paths"], "binding.paths")
    checks = names(record["receipt_ids"], "binding.receipts")
    source.require(all(c in receipts for c in checks), "binding.receipt-not-qualified")
    source.require(any(record["id"] in receipts[c]["coverage"][coverage_kind] for c in checks),
                   "binding.criteria-not-qualified")
    for path in paths:
        repo.path(path)
        source.require(any(path in receipts[c]["pins"] for c in checks), "binding.source-not-qualified")
        source.require(repo.git("rev-parse", commit + ":" + path)
                       == repo.git("rev-parse", expected + ":" + path), "binding.stale-source")


def deferral(repo, document, expected, tasks):
    values = document["deferrals"]
    source.require(isinstance(values, list) and len(values) <= 1, "deferral.shape")
    if not values:
        return False
    value = values[0]
    closed(value, ("task_id", "status", "blocking", "authority"), "deferral.record")
    source.require(value["task_id"] == "QA08" and value["status"] == "DEFERRED_BY_USER"
                   and value["blocking"] is False, "deferral.unauthorized-scope-cut")
    pin = value["authority"]
    closed(pin, ("path", "git_blob", "sha256"), "deferral.authority-shape")
    source.require(pin["path"] == AUTHORITY, "deferral.authority-path")
    repo.pins({AUTHORITY: {k: pin[k] for k in ("git_blob", "sha256")}}, expected)
    text = repo.source_bytes[AUTHORITY].decode("utf-8")
    source.require(re.search(r"\| QA08[^\n]*DEFERRED_BY_USER", text)
                   and "Defer QA08" in text and "111 active" in text, "deferral.authority-missing")
    source.require("QA08" in tasks, "deferral.original-task-missing")
    return True


def evaluate(root, expected, manifest=None):
    result = {"schema": SCHEMA, "candidate": expected, "status": "INCOMPLETE",
              "composition_checks": "NOT_RUN", "release_decision": "NOT_ASSESSED",
              "functional_evidence": "INCOMPLETE", "findings": [], "receipts": [], "gates": [],
              "task_coverage": {}, "ia_coverage": {}, "qa08": "NOT_RUN",
              "limits": ["Offline owner-evidence consistency, not signed execution attestation.",
                         "No receipt command is executed. Owner acceptance is recorded, not independently decided.",
                         "This output cannot grant IN05, qualify performance, or replace physical/live work."]}
    try:
        repo = source.Repository(root)
        source.require(isinstance(expected, str) and source.HEX40.fullmatch(expected), "candidate.full-sha-required")
        source.require(repo.git("rev-parse", "HEAD").decode().strip() == expected, "candidate.not-current-head")
        source.require(not repo.git("status", "--porcelain", "--untracked-files=no").strip(), "candidate.worktree-dirty")
        tree = repo.git("rev-parse", expected + "^{tree}").decode().strip()
        tasks, task_pin = catalog(repo, expected, "generated/TASKS_EN.json", "tasks", 112)
        screens, ia_pin = catalog(repo, expected, "generated/IA_EN.json", "screens", 61)
        result["catalogs"] = [task_pin, ia_pin]
        result["task_coverage"] = {"required": 112, "bound": 0, "missing": sorted(tasks)}
        result["ia_coverage"] = {"required": 61, "bound": 0, "missing": sorted(screens)}
        if manifest is None:
            result["findings"].append("manifest.NOT_RUN")
            return result
        raw = repo.read(manifest)
        document = source.decode(raw)
        closed(document, ("schema", "candidate", "source_manifest", "receipts", "tasks", "ia", "gates", "deferrals"),
               "composition.manifest-shape")
        source.require(document["schema"] == SCHEMA, "composition.schema")
        repo.candidate(document["candidate"], expected)
        result["input_manifest"] = {"path": manifest, "sha256": source.digest(raw)}
        gaps = []
        if document["source_manifest"] is None:
            gaps.append("source.NOT_RUN")
        else:
            repo.artifact(document["source_manifest"])
            bounded = source.evaluate(root, expected, document["source_manifest"]["path"])
            source.require(bounded["status"] != "FAIL", "source.failed")
            result["source_checks"] = bounded["source_checks"]
            if bounded["source_checks"] != "PASS":
                gaps.append("source.INCOMPLETE")
        receipts = {}
        declared = set()
        source.require(isinstance(document["receipts"], list) and len(document["receipts"]) <= 512,
                       "composition.receipt-limit")
        for ref in document["receipts"]:
            report = source.decode(repo.artifact(ref))
            identity = report.get("id", "frontend" if report.get("schema") == "mcr.qa10.frontend.v1" else None)
            source.require(isinstance(identity, str) and re.fullmatch(r"[A-Za-z0-9_-]{1,100}", identity)
                           and identity not in declared, "evidence.duplicate-or-invalid-id")
            declared.add(identity)
            repo.candidate(report.get("candidate"), expected)
            status = report.get("status")
            if status in ("NOT_RUN", "BLOCKED"):
                closed(report, ("schema", "id", "candidate", "status", "owner", "reason"), "evidence.pending-shape")
                source.require(report["schema"] == "mcr.qa10.composition-evidence.v1"
                               and isinstance(report["owner"], str) and report["owner"]
                               and isinstance(report["reason"], str) and report["reason"], "evidence.pending-owner")
                gaps.append("evidence." + identity + "." + status)
                result["receipts"].append({"id": identity, "status": status})
                continue
            checked = evidence(repo, report, expected, str(PurePosixPath(ref["path"]).parent))
            source.require(set(checked["coverage"]["task_ids"]) <= set(tasks) - {"QA08"}
                           and set(checked["coverage"]["ia_ids"]) <= set(screens)
                           and set(checked["coverage"]["gate_ids"]) <= set(GATES), "evidence.unknown-coverage")
            if checked["counts"]["skipped"]:
                gaps.append("evidence." + identity + ".skipped")
            else:
                receipts[identity] = checked
            result["receipts"].append({"id": identity, "status": "INCOMPLETE" if checked["counts"]["skipped"] else "PASS",
                                       "category": checked["category"], "level": checked["level"],
                                       "counts": checked["counts"]})
        missing_categories = CATEGORIES - {r["category"] for r in receipts.values()}
        gaps.extend("category." + c + ".NOT_RUN" for c in sorted(missing_categories))
        if not any(r["producer"] == "frontend-v1" for r in receipts.values()):
            gaps.append("frontend.v1.NOT_RUN")
        deferred = deferral(repo, document, expected, tasks)
        result["qa08"] = "DEFERRED_BY_USER" if deferred else "NOT_RUN"
        if not deferred:
            gaps.append("QA08.deferral.NOT_RUN")
        task_records = coverage_records(document["tasks"], tasks, "tasks")
        ia_records = coverage_records(document["ia"], screens, "ia")
        missing_tasks = []
        for identity, task in tasks.items():
            record = task_records.get(identity)
            if identity == "QA08":
                source.require(not record or record["status"] == "DEFERRED_BY_USER", "QA08.cannot-pass")
                if not deferred or not record:
                    missing_tasks.append(identity)
                continue
            if not record or record["status"] != "ACCEPTED":
                missing_tasks.append(identity)
                continue
            binding(repo, record, expected, receipts, "task_ids")
            source.require(all(owned(p, task["owned_paths"]) for p in record["paths"]), "tasks.path-outside-original-task")
        missing_ia = []
        for identity, screen in screens.items():
            record = ia_records.get(identity)
            if not record or record["status"] != "ACCEPTED":
                missing_ia.append(identity)
                continue
            binding(repo, record, expected, receipts, "ia_ids")
            source.require(any(receipts[c]["level"] in ("live", "device") for c in record["receipt_ids"]),
                           "ia.unit-as-product")
            source.require(all(t in tasks for t in screen["task_ids"]), "ia.unknown-task")
            source.require(all(any(owned(p, tasks[t]["owned_paths"]) for t in screen["task_ids"])
                               for p in record["paths"]), "ia.unrelated-source")
            if any(t in missing_tasks for t in screen["task_ids"]):
                missing_ia.append(identity)
        if missing_tasks:
            gaps.append("tasks.NOT_RUN-or-BLOCKED")
        if missing_ia:
            gaps.append("ia.NOT_RUN-or-BLOCKED")
        result["task_coverage"] = {"required": 112, "bound": 112 - len(missing_tasks) - int(deferred and "QA08" not in missing_tasks),
                                   "deferred": ["QA08"] if deferred else [], "missing": sorted(missing_tasks)}
        result["ia_coverage"] = {"required": 61, "bound": 61 - len(missing_ia), "missing": sorted(missing_ia)}
        gate_records = coverage_records(document["gates"], set(GATES) | {"QA08"}, "gates")
        for identity, levels in GATES.items():
            record = gate_records.get(identity)
            if not record or record["status"] != "ACCEPTED":
                state = "BLOCKED" if record and record["status"] == "BLOCKED" else "NOT_RUN"
                gaps.append("gate." + identity + "." + state)
                result["gates"].append({"id": identity, "status": state})
                continue
            binding(repo, record, expected, receipts, "gate_ids")
            selected = [receipts[c] for c in record["receipt_ids"]]
            source.require(any(r["level"] in levels for r in selected), "gates.unit-as-live")
            if identity == "QA06":
                platforms = {r["context"]["device"]["platform"] for r in selected if r["level"] == "device"}
                source.require(platforms == {"ios", "android"}, "gates.both-physical-platforms-required")
            result["gates"].append({"id": identity, "status": "OWNER_RECORDED", "receipt_ids": record["receipt_ids"]})
        qa08 = gate_records.get("QA08")
        source.require(not qa08 or qa08["status"] == "DEFERRED_BY_USER", "QA08.cannot-pass")
        if not qa08:
            gaps.append("gate.QA08.deferral.NOT_RUN")
        result["gates"].append({"id": "QA08", "status": result["qa08"], "blocking": not deferred})
        source.require(repo.git("rev-parse", "HEAD").decode().strip() == expected
                       and repo.git("rev-parse", expected + "^{tree}").decode().strip() == tree
                       and not repo.git("status", "--porcelain", "--untracked-files=no").strip(), "candidate.end-changed")
        result["findings"] = sorted(set(gaps))
        result["composition_checks"] = "INCOMPLETE" if gaps else "PASS"
        result["status"] = "INCOMPLETE" if gaps else "PASS"
        result["functional_evidence"] = "INCOMPLETE" if gaps else "OWNER_RECORDED"
    except source.Rejected as error:
        result["status"] = result["composition_checks"] = "FAIL"
        result["findings"].append(str(error))
    except (OSError, KeyError, TypeError, ValueError, AttributeError, IndexError, RecursionError, subprocess.TimeoutExpired):
        result["status"] = result["composition_checks"] = "FAIL"
        result["findings"].append("composition.malformed-or-unavailable")
    return result
