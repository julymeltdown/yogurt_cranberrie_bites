#!/usr/bin/env python3
"""Compose same-candidate QA10 release inputs and the task-to-commit trace.

Reads the plan, the progress ledger and the candidate Git objects only. It writes
task-trace.json plus the composition manifest (input.json and the per-category
pending envelopes). It never executes commands recorded in evidence and never
copies another candidate's artifacts: every referenced file is generated here,
and every referenced commit is resolved and ancestor-checked against --candidate.

Usage:
    python3 .omo/evidence/mcr-finish-project/task-42-qa10/compose.py \
        --root . --candidate 25b2d356f...
"""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

PLAN = ".omo/plans/mcr-finish-project.md"
LEDGER = "docs/mcr/task-progress-2026-11-04.json"
TASKS = "generated/TASKS_EN.json"
IA = "generated/IA_EN.json"
AUTHORITY = "docs/mcr/functional-delivery-plan.md"
OUT = ".omo/evidence/mcr-finish-project/task-42-qa10"
COMPOSITION = "mcr.qa10.release-composition.v1"
EVIDENCE = "mcr.qa10.composition-evidence.v1"
GATES = ("BE43", "QA01", "QA02", "QA03", "QA04", "QA05", "QA06", "QA07", "QA09",
         "IN01", "IN02", "IN03", "IN04")
CATEGORIES = ("backend", "frontend", "service", "device", "operator")
OWNER_BY_CATEGORY = {
    "backend": "Backend implementation owner (concert-service / gateway)",
    "frontend": "Frontend implementation owner (world shell and screens)",
    "service": "Real-stack service owner (compose, gateway, PostgreSQL, Redis)",
    "device": "Mobile device evidence owner (physical iOS Safari / Android Chrome)",
    "operator": "Operator/moderation owner (deployment, retention, provenance)",
}
STATUS_BY_LEDGER = {
    "DONE": "NOT_RUN",
    "IN_PROGRESS": "IN_PROGRESS",
    "REOPENED": "IN_PROGRESS",
    "WAITING_INPUT": "BLOCKED",
    "OPEN": "NOT_RUN",
    "DEFERRED_BY_USER": "DEFERRED_BY_USER",
}


def sh(root, *args, check=True):
    done = subprocess.run(["git", *args], cwd=root, capture_output=True, timeout=120)
    if check and done.returncode != 0:
        raise SystemExit("git %s failed: %s" % (" ".join(args), done.stderr.decode().strip()))
    return done.stdout


def blob(root, candidate, path):
    return sh(root, "show", candidate + ":" + path)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def resolve(root, candidate, sha):
    out = sh(root, "rev-parse", "--verify", "--quiet", sha + "^{commit}", check=False).decode().strip()
    if not out or not out.startswith(sha):
        return None
    return out


def ancestor(root, candidate, commit):
    return subprocess.run(["git", "merge-base", "--is-ancestor", commit, candidate],
                          cwd=root, capture_output=True).returncode == 0


def exists_at(root, commit, path):
    return subprocess.run(["git", "cat-file", "-e", commit + ":" + path], cwd=root,
                          capture_output=True).returncode == 0


def introducing(root, candidate, path):
    chain = sh(root, "log", "--first-parent", "--format=%H", candidate, "--", path).decode().split()
    for commit in reversed(chain):
        if exists_at(root, commit, path.rstrip("/")):
            return [commit]
    return []


def entry(commit, method, log):
    return {"commit": commit, "method": method, "subject": log.get(commit)}


STOPWORDS = {"the", "and", "for", "with", "its", "from", "into", "then", "that", "this", "a", "an",
             "of", "on", "in", "to", "is", "be", "or", "as", "at", "by", "not"}


def resolve_delivery(root, candidate, row, log, path, evidence_text):
    declared = row["declared_commit_subject"]
    if declared:
        exact = [c for c, s in log.items() if s == declared]
        if exact:
            return [entry(c, "plan-commit-subject", log) for c in exact], "plan-commit-subject"
    labeled = []
    for line in evidence_text.splitlines():
        if not re.search(r"(?i)\b(delivered|delivering|implementation|final correction|handoff|"
                         r"published as|merged as)\b", line):
            continue
        for sha in re.findall(r"(?<![0-9a-f])[0-9a-f]{7,40}(?![0-9a-f])", line):
            commit = resolve(root, candidate, sha)
            if commit in log and commit not in labeled:
                labeled.append(commit)
    if labeled:
        return [entry(c, "evidence-labeled-commit", log) for c in labeled], "evidence-labeled-commit"
    tokens = re.findall(r"(?<![A-Za-z0-9])(?:[A-Z]{2,5}-[A-Z0-9]+(?:-[A-Z0-9]+)*|[A-Z]{2,3}\d{2,4})"
                        r"(?![A-Za-z0-9])", row["title"])
    hits = []
    for token in tokens:
        pattern = re.compile(r"(?<![A-Za-z0-9])" + re.escape(token) + r"(?![A-Za-z0-9])", re.I)
        for commit in sh(root, "log", "--format=%H", "--grep=" + token, "-i", candidate).decode().split():
            if (commit in log and pattern.search(log[commit])
                    and not log[commit].lower().startswith("revert") and commit not in hits):
                hits.append(commit)
    if hits:
        return [entry(c, "identifier-subject", log) for c in hits], "identifier-subject"
    words = [w for w in re.findall(r"[a-z0-9]+", row["title"].lower())
             if w not in STOPWORDS and len(w) > 2]
    scored = []
    for commit, subject in log.items():
        low = subject.lower()
        score = sum(1 for w in words if w in low)
        if words and score >= max(2, (len(words) + 1) // 2):
            scored.append((score, commit))
    if scored:
        best = max(s for s, _ in scored)
        return ([entry(c, "title-phrase", log) for s, c in scored if s == best], "title-phrase")
    for commit in introducing(root, candidate, path):
        if commit in log:
            return [entry(commit, "evidence-bundle-merge", log)], "evidence-bundle-merge"
    return [], "unresolved"


def plan_rows(text, catalog_ids):
    rows = []
    for block in re.split(r"\n(?=- \[.\] \d+\. )", text):
        head = re.match(r"^- \[(.)\] (\d+)\. (.*)$", block, re.M)
        if not head:
            continue
        commit = re.search(r"^  Commit: ([YN])(?: \| `?([^`\n]+?)`?)?$", block, re.M)
        evidence = re.search(r"Evidence:?\s*`([^`]+)`", block)
        title = head.group(3).strip()
        linked = sorted({t for t in catalog_ids
                         if re.search(r"(?<![A-Za-z0-9])" + re.escape(t) + r"(?![A-Za-z0-9])", title)})
        rows.append({"row": int(head.group(2)), "title": title, "box": head.group(1),
                     "declared_commit_subject": commit.group(2).strip() if commit and commit.group(2) else None,
                     "commit_flag": commit.group(1) if commit else None,
                     "evidence_path": evidence.group(1) if evidence else None,
                     "catalog_task_ids": linked})
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=str(Path(__file__).resolve().parents[4]))
    parser.add_argument("--candidate", required=True)
    args = parser.parse_args()
    root = str(Path(args.root).resolve())
    candidate = args.candidate

    head = sh(root, "rev-parse", "HEAD").decode().strip()
    if head != candidate:
        raise SystemExit("candidate.not-current-head: HEAD=%s" % head)
    if sh(root, "status", "--porcelain", "--untracked-files=no").strip():
        raise SystemExit("candidate.worktree-dirty")
    tree = sh(root, "rev-parse", candidate + "^{tree}").decode().strip()
    pinned = {"commit": candidate, "tree": tree, "dirty": False}

    plan_path = Path(root, OUT)
    plan_path.mkdir(parents=True, exist_ok=True)

    plan_bytes = blob(root, candidate, PLAN)
    ledger_bytes = blob(root, candidate, LEDGER)
    tasks_catalog = json.loads(blob(root, candidate, TASKS).decode())["tasks"]
    ia_catalog = json.loads(blob(root, candidate, IA).decode())["screens"]
    required = [t for t in tasks_catalog if t["priority"] == "P0"]
    screens = [s for s in ia_catalog if s["priority"] == "P0"]
    if len(required) != 112 or len(screens) != 61:
        raise SystemExit("catalog.scope-mismatch: %d/%d" % (len(required), len(screens)))
    ledger = json.loads(ledger_bytes.decode())
    ledger_rows = {t["id"]: t for t in ledger["tasks"]}

    log = {}
    for line in sh(root, "log", "--format=%H\t%s", candidate).decode().splitlines():
        commit, _, subject = line.partition("\t")
        log[commit] = subject

    rows = plan_rows(plan_bytes.decode(), [t["id"] for t in required])
    failures = []
    for row in rows:
        row["delivering_commits"] = []
        row["named_commits"] = []
        row["evidence_present"] = False
        row["evidence_tracked"] = False
        path = row["evidence_path"]
        if path:
            row["evidence_present"] = Path(root, path).exists()
            ls = sh(root, "ls-files", "--error-unmatch", path, check=False).decode().strip()
            row["evidence_tracked"] = bool(ls)
        text = sh(root, "show", candidate + ":" + path).decode("utf-8", "replace") if row["evidence_tracked"] else ""
        if row["commit_flag"] == "N":
            row["delivering_commits"], row["resolution"] = [], "no-commit-required"
        elif row["box"] == " ":
            row["delivering_commits"], row["resolution"] = [], "open"
            row["context_commits"] = resolve_delivery(root, candidate, row, log, path, text)[0]
        else:
            row["delivering_commits"], row["resolution"] = resolve_delivery(
                root, candidate, row, log, path, text)
        if row["declared_commit_subject"] and row["resolution"] == "plan-commit-subject":
            row["subject_drift"] = False
        else:
            row["subject_drift"] = bool(row["declared_commit_subject"])
        if row["evidence_tracked"]:
            tokens = sorted(set(re.findall(r"(?<![0-9a-f])[0-9a-f]{7,40}(?![0-9a-f])", text)),
                            key=lambda v: (-len(v), v))
            for token in tokens:
                commit = resolve(root, candidate, token)
                if not commit or any(c["commit"] == commit for c in row["named_commits"]):
                    continue
                row["named_commits"].append({"commit": commit, "mentioned_as": token,
                                             "subject": log.get(commit),
                                             "ancestor": ancestor(root, candidate, commit),
                                             "delivering": any(c["commit"] == commit
                                                               for c in row["delivering_commits"])})
        for item in row["delivering_commits"]:
            item["ancestor"] = ancestor(root, candidate, item["commit"])
            if not item["ancestor"]:
                failures.append({"row": row["row"], "test": "ancestor",
                                 "commit": item["commit"], "subject": item["subject"]})
        if row["box"] == "x":
            row["plan_status"] = "CLOSED"
            if row["commit_flag"] == "N":
                row["verdict"] = "CLOSED_NO_COMMIT_REQUIRED"
            elif not row["delivering_commits"]:
                row["verdict"] = "CLOSED_COMMIT_UNRESOLVED"
                failures.append({"row": row["row"], "test": "commit-map",
                                 "declared": row["declared_commit_subject"]})
            else:
                row["verdict"] = "CLOSED_COMMIT_ANCESTOR"
            if path and not row["evidence_present"]:
                failures.append({"row": row["row"], "test": "evidence-path", "path": path})
        elif row["box"] == "~":
            row["plan_status"] = "PARTIAL"
            row["verdict"] = "PARTIAL_WITH_COMMIT" if row["delivering_commits"] else "PARTIAL"
        else:
            row["plan_status"] = "OPEN"
            row["verdict"] = "OPEN"

    row_counts = {}
    for record in ledger["tasks"]:
        state = record["revised_plan"]["state"]
        row_counts[state] = row_counts.get(state, 0) + 1
    aggregate = ledger["functional_delivery"]["counts"]

    trace = {
        "schema": "mcr.qa10.task-trace.v1",
        "candidate": pinned,
        "plan": {"path": PLAN, "sha256": digest(plan_bytes)},
        "ledger": {"path": LEDGER, "sha256": digest(ledger_bytes),
                   "row_level_counts": row_counts, "aggregate_counts": aggregate,
                   "count_drift": {k: [row_counts.get(k, 0), aggregate.get(k, 0)]
                                   for k in sorted(set(row_counts) | set(aggregate))
                                   if row_counts.get(k, 0) != aggregate.get(k, 0)}},
        "rows": rows,
        "summary": {"rows": len(rows),
                    "closed": sum(r["plan_status"] == "CLOSED" for r in rows),
                    "partial": sum(r["plan_status"] == "PARTIAL" for r in rows),
                    "open": sum(r["plan_status"] == "OPEN" for r in rows),
                    "closed_with_delivering_commit": sum(
                        r["verdict"] in ("CLOSED_COMMIT_ANCESTOR", "CLOSED_NO_COMMIT_REQUIRED")
                        for r in rows)},
        "failures": failures,
    }
    (plan_path / "task-trace.json").write_text(json.dumps(trace, indent=2, sort_keys=True) + "\n")

    def owner(lane):
        return "%s implementation owner" % lane

    def task_record(task):
        record = ledger_rows.get(task["id"])
        state = record["revised_plan"]["state"] if record else "OPEN"
        status = STATUS_BY_LEDGER.get(state)
        if status is None:
            raise SystemExit("ledger.state-unknown: %s %s" % (task["id"], state))
        return {"id": task["id"], "status": status, "owner": owner(task["lane"]),
                "reason": "LEDGER_STATE=%s; CURRENT_CANDIDATE_OWNER_RECEIPT_NOT_SUPPLIED" % state,
                "commit": "", "paths": [], "receipt_ids": []}

    def ia_owner(screen):
        lanes = sorted({t["lane"] for t in required if t["id"] in screen["task_ids"]})
        return owner(lanes[0]) if lanes else "Frontend implementation owner"

    authority_blob = sh(root, "rev-parse", candidate + ":" + AUTHORITY).decode().strip()
    authority_sha = digest(sh(root, "cat-file", "blob", authority_blob))

    receipts = []
    for category in CATEGORIES:
        name = "%s-not-run.json" % category
        envelope = {"schema": EVIDENCE, "id": category + "-candidate", "candidate": pinned,
                    "status": "NOT_RUN", "owner": OWNER_BY_CATEGORY[category],
                    "reason": "CURRENT_CANDIDATE_OWNER_RECEIPT_NOT_SUPPLIED"}
        raw = (json.dumps(envelope, indent=2, sort_keys=True) + "\n").encode()
        (plan_path / name).write_bytes(raw)
        receipts.append({"path": OUT + "/" + name, "sha256": digest(raw)})

    manifest = {
        "schema": COMPOSITION,
        "candidate": pinned,
        "source_manifest": None,
        "receipts": receipts,
        "tasks": [task_record(t) for t in required],
        "ia": [{"id": s["id"], "status": "NOT_RUN", "owner": ia_owner(s),
                "reason": "LEDGER_TASK_BINDINGS_PENDING; CURRENT_CANDIDATE_OWNER_RECEIPT_NOT_SUPPLIED",
                "commit": "", "paths": [], "receipt_ids": []} for s in screens],
        "gates": [{"id": gate,
                   "status": ("DEFERRED_BY_USER" if gate == "QA08" else
                              "BLOCKED" if ledger_rows.get(gate) and
                              ledger_rows[gate]["revised_plan"]["state"] == "WAITING_INPUT"
                              else "NOT_RUN"),
                   "owner": owner(ledger_rows[gate]["lane"] if gate in ledger_rows else "INTEGRATOR"),
                   "reason": "CURRENT_CANDIDATE_OWNER_RECEIPT_NOT_SUPPLIED",
                   "commit": "", "paths": [], "receipt_ids": []} for gate in GATES + ("QA08",)],
        "deferrals": [{"task_id": "QA08", "status": "DEFERRED_BY_USER", "blocking": False,
                       "authority": {"path": AUTHORITY, "git_blob": authority_blob,
                                     "sha256": authority_sha}}],
    }
    manifest_bytes = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
    (plan_path / "input.json").write_bytes(manifest_bytes)

    print(json.dumps({"trace": {"sha256": digest((plan_path / "task-trace.json").read_bytes()),
                                "summary": trace["summary"],
                                "failures": failures},
                      "manifest": {"path": OUT + "/input.json",
                                   "sha256": digest(manifest_bytes),
                                   "receipts": len(receipts), "tasks": len(manifest["tasks"]),
                                   "ia": len(manifest["ia"]), "gates": len(manifest["gates"])}},
                     indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
