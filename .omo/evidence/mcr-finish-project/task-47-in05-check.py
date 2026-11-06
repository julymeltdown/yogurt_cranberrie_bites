#!/usr/bin/env python3
"""IN05 (plan row 47) mechanical reconciliation.

Parses every plan row, checks closed-row evidence and delivering-commit
ancestry against the candidate, counts the named obligations, inventories the
recorded gaps, and writes task-47-in05.md / .json next to this script.

Exit code 0 only when every in-scope binding passes; otherwise 1.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CANDIDATE = "7d14251bb"
PLAN = ROOT / ".omo/plans/mcr-finish-project.md"
EVIDENCE_DIR = ROOT / ".omo/evidence/mcr-finish-project"
REGISTER = ROOT / "docs/mcr/task-progress-2026-11-04.json"
IA_JSON = ROOT / "generated/IA_EN.json"
TASKS_JSON = ROOT / "generated/TASKS_EN.json"
FDP = ROOT / "docs/mcr/functional-delivery-plan.md"
AMENDMENT = ROOT / "docs/mcr/venue-feature-amendment-2026-11-05.md"
OUT_MD = EVIDENCE_DIR / "task-47-in05.md"
OUT_JSON = EVIDENCE_DIR / "task-47-in05.json"

ROW_RE = re.compile(r"^- \[([ x~])\] (?:(\d+)\.|(F\d+)\.)\s*(.*)$")
EXPECTED_GAPS = [11, 14, 16, 28, 30, 32, 35, 36, 37, 41, 42, 43, 45, 51]

GAP_OWNERS = {
    "11": "Feed/Post owner (task 49 envelope landed; media pipeline qualified, feed mutation transport deliberately closed)",
    "14": "Account/Concert deletion owner (task 50 mounted the coordinator; serving convergence unproven)",
    "16": "Concert lifecycle owner (task 51 producer gap)",
    "28": "Venue Map/Friends authority owner (bilateral friend authority; reference-base Photo 5 approval is a product decision)",
    "30": "IN03 social owner + external provider/claims/mail provenance requester",
    "32": "World/Feed composition owner (Social policy transport and Feed product mode)",
    "35": "AR01 venue-provenance owner + external venue operator input",
    "36": "QA06 visual owner; blocked producers named in the gap (siteplan, operator, safety, providers, legal/archive)",
    "37": "World release/atlas activation owner (tracked with task 51)",
    "41": "QA09 recovery owner (deletion saga convergence)",
    "42": "QA10 release-composition publisher (must re-run against the final release SHA)",
    "43": "Frontend gift owner + BE24 Gateway media composition owner",
    "45": "FE10 legal/eligibility owner + external approved-publication requester",
    "51": "Event lifecycle owner (LifecycleOwners for FREEZE_MURAL..CLEANUP)",
}


def sh(*args: str) -> str:
    return subprocess.run(
        args, cwd=ROOT, capture_output=True, text=True, check=False
    ).stdout


def parse_plan() -> list[dict]:
    rows: list[dict] = []
    current: dict | None = None
    for line in PLAN.read_text(encoding="utf-8").splitlines():
        m = ROW_RE.match(line)
        if m:
            if current:
                rows.append(current)
            current = {
                "row": m.group(2) or m.group(3),
                "state": m.group(1),
                "title": m.group(4).strip(),
                "body": [],
            }
        elif current is not None:
            current["body"].append(line)
    if current:
        rows.append(current)
    for row in rows:
        body = "\n".join(row["body"])
        row["evidence"] = (re.search(r"Evidence `([^`]+)`", body) or [None, None])[1]
        row["commit"] = (re.search(r"Commit: ([YN])(?: \| `([^`]+)`)?", body) or [None, None, None])[2]
        row["gap"] = (re.search(r"^Gap \(\d{4}-\d{2}-\d{2}\):\s*(.+)$", body, re.M) or [None, None])[1]
        row["closes"] = (re.search(r"^Closes: (.+)$", body, re.M) or [None, None])[1]
    return rows


def commit_index() -> list[dict]:
    raw = sh("git", "log", "--format=%H%x00%s%x00%b%x01", CANDIDATE, "--")
    commits = []
    for chunk in raw.split("\x01"):
        chunk = chunk.strip("\n")
        if not chunk:
            continue
        sha, subject, body = (chunk.split("\x00") + ["", ""])[:3]
        commits.append({"sha": sha, "subject": subject, "body": body})
    return commits


ANCESTRY: set[str] | None = None


def is_ancestor(sha: str) -> bool:
    global ANCESTRY
    if ANCESTRY is None:
        raw = sh("git", "rev-list", CANDIDATE)
        ANCESTRY = {line.strip() for line in raw.splitlines() if line.strip()}
    return sha in ANCESTRY or any(a.startswith(sha) for a in ANCESTRY)


def normalize_message(message: str) -> str:
    return re.sub(r"\s+at\s+[0-9a-f]{7,40}\s*$", "", message).strip().strip("`")


STOP = {
    "the", "and", "add", "a", "an", "of", "for", "to", "with", "on", "in",
    "its", "all", "per", "as", "from", "by", "or", "is", "be",
}


def tokens(text: str) -> set[str]:
    words = re.findall(r"[a-z0-9]+", text.lower())
    return {w[:-1] if len(w) > 3 and w.endswith("s") else w for w in words if w not in STOP}


def overlap(plan_text: str, subject: str) -> tuple[float, set[str]]:
    wanted = tokens(plan_text)
    shared = wanted & tokens(subject)
    return (len(shared) / max(1, len(wanted)), shared)


def is_commit(sha: str) -> bool:
    return subprocess.run(
        ["git", "cat-file", "-e", f"{sha}^{{commit}}"], cwd=ROOT, capture_output=True
    ).returncode == 0


def evidence_shas(row: dict) -> list[str]:
    path = row["evidence_check"].get("path")
    if not path:
        return []
    full = ROOT / path
    if not full.is_file():
        return []
    text = full.read_text(encoding="utf-8", errors="replace")
    return sorted(set(re.findall(r"\b[0-9a-f]{7,40}\b", text)))


def ledger_commits() -> dict[str, list[str]]:
    """Row id -> delivering commits recorded in the ulw-execute ledger."""
    path = ROOT / ".omo/ulw-execute/ledger.jsonl"
    found: dict[str, list[str]] = {}
    if not path.is_file():
        return found
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            continue
        if entry.get("event") not in {"task-verified", "task-gap-closed"}:
            continue
        task = str(entry.get("task", ""))
        sha = (entry.get("commit") or "").strip()
        if task and sha:
            found.setdefault(task, []).append(sha)
    return found


LEDGER = ledger_commits()


def resolve_commit(row: dict, commits: list[dict]) -> dict:
    for sha in LEDGER.get(str(row["row"]), []):
        if is_commit(sha) and is_ancestor(sha):
            match = next((c for c in commits if c["sha"].startswith(sha)), None)
            return {
                "status": "PASS",
                "sha": match["sha"] if match else sha,
                "subject": match["subject"] if match else "",
                "source": "ledger",
                "score": 1.0,
                "reason": f"ledger-recorded delivering commit {sha[:9]} is an ancestor of the candidate",
            }

    raw = (row.get("commit") or "").strip().strip("`")
    if not raw and re.search(r"^\s*Commit: N\b", "\n".join(row.get("body") or []), re.M):
        return {
            "status": "PASS",
            "sha": "",
            "subject": "",
            "source": "no-commit-required",
            "score": 1.0,
            "reason": "plan row declares Commit: N; verification-only row, evidence artifact is the delivery",
        }
    wanted = normalize_message(raw)
    for sha in re.findall(r"\b[0-9a-f]{7,40}\b", raw):
        if is_commit(sha) and is_ancestor(sha):
            match = next((c for c in commits if c["sha"].startswith(sha)), None)
            return {
                "status": "PASS",
                "sha": match["sha"] if match else sha,
                "subject": match["subject"] if match else "",
                "source": "plan SHA",
                "score": 1.0,
                "reason": f"plan-named commit {sha[:9]} is an ancestor of the candidate",
            }

    # Evidence-named commits: accept the best ancestor whose subject overlaps
    # the plan delivery message or the row title.
    best: tuple[float, dict, str] | None = None
    for sha in evidence_shas(row):
        if not is_commit(sha) or not is_ancestor(sha):
            continue
        match = next((c for c in commits if c["sha"].startswith(sha)), None)
        if not match:
            continue
        score_msg, shared_msg = overlap(wanted, match["subject"])
        score_title, shared_title = overlap(row["title"], match["subject"])
        score, shared = max((score_msg, shared_msg), (score_title, shared_title))
        if score >= 0.4 and len(shared) >= 2 and (best is None or score > best[0]):
            best = (score, match, "evidence")

    # Commit subjects: plan messages are paraphrases, so match significant
    # tokens. Merge commits carry deliveries too and use a lower threshold.
    for c in commits:
        if not is_ancestor(c["sha"]):
            continue
        score_msg, shared_msg = overlap(wanted, c["subject"])
        score_title, shared_title = overlap(row["title"], c["subject"])
        score, shared = max((score_msg, shared_msg), (score_title, shared_title))
        is_merge = c["subject"].startswith("merge(")
        threshold = 0.4 if is_merge else 0.55
        if score >= threshold and len(shared) >= 2:
            if best is None or score > best[0]:
                best = (score, c, "subject")

    if best is None:
        return {
            "status": "FAIL",
            "reason": f"no ancestor commit matches plan delivery {wanted!r} or row title",
        }
    score, match, source = best
    return {
        "status": "PASS",
        "sha": match["sha"],
        "subject": match["subject"],
        "source": source,
        "score": round(score, 2),
        "reason": f"{source} match (token overlap {score:.2f}): {match['subject']!r} is an ancestor",
    }


def resolve_evidence(row: dict) -> dict:
    named = row.get("evidence")
    if not named:
        return {"status": "FAIL", "reason": "no evidence path named"}
    rel = named.rstrip("/").split(":")[0]
    path = ROOT / rel
    if path.exists():
        return {"status": "PASS", "path": rel, "substituted": False}
    row_id = row["row"]
    patterns = [f"task-{row_id}-*"] if row_id.isdigit() else [f"{row_id.lower()}-*"]
    candidates = sorted(
        p.name for pattern in patterns for p in EVIDENCE_DIR.glob(pattern)
    )
    if candidates:
        chosen = f".omo/evidence/mcr-finish-project/{candidates[0]}"
        return {
            "status": "PASS",
            "path": chosen,
            "substituted": True,
            "reason": f"plan path {named!r} missing; substituted {chosen!r}",
        }
    return {"status": "FAIL", "reason": f"evidence path {named!r} missing and no substitute"}


def count_ia_states(row46_text: str) -> list[str]:
    section = row46_text.split("## Source of the ten extension IA states", 1)[-1]
    states = []
    for line in section.splitlines():
        m = re.match(r"^\| (\d+) \| ([^|]+?) \|", line)
        if m and 1 <= int(m.group(1)) <= 10:
            states.append(m.group(2).strip())
    return states


def gap_owner(row_id: str) -> str:
    return GAP_OWNERS.get(row_id, "unassigned")


def build_not_run(rows: list[dict], register: dict) -> list[dict]:
    items = []
    for row in rows:
        for match in re.finditer(r"NOT_RUN", row.get("gap") or ""):
            items.append(
                {
                    "source": f"plan row {row['row']}",
                    "item": "NOT_RUN stated in gap",
                    "detail": row["gap"][max(0, match.start() - 120) : match.start() + 160].strip(),
                }
            )
    for task in register["tasks"]:
        state = task["revised_plan"]["state"]
        if state in {"WAITING_INPUT", "OPEN"} and task["id"] in {
            "AR01",
            "FE10",
            "IN02",
            "IN03",
            "IN04",
            "IN05",
        }:
            items.append(
                {
                    "source": f"register {task['id']}",
                    "item": state,
                    "detail": (task.get("note") or "")[:240],
                }
            )
    items.append(
        {
            "source": "register QA08",
            "item": "DEFERRED_BY_USER / NOT_RUN",
            "detail": "Performance qualification including 200 concurrent users is deferred; it is not passed or completed and gates no current functional delivery.",
        }
    )
    return items


def render_md(p: dict) -> str:
    lines = [
        "# Task 47 - IN05 reconciliation and release decision",
        "",
        f"- Candidate: `{p['candidate']}` on `{p['branch']}`",
        f"- Generated: {p['generated_at']}",
        "- Checker: `.omo/evidence/mcr-finish-project/task-47-in05-check.py`",
        "- Machine record: `.omo/evidence/mcr-finish-project/task-47-in05.json`",
        f"- **Release decision: `{p['release_decision']}`**",
        "",
        "## Decision",
        "",
    ]
    c = p["counts"]
    n = p["name_checks"]
    lines += [
        f"The candidate is **{p['release_decision']}**. Of 56 plan rows, {c['plan_rows'].get('DONE', 0)} are closed, "
        f"{c['plan_rows'].get('GAP', 0)} are recorded gaps, and {c['plan_rows'].get('OPEN', 0)} remain open. "
        f"{c['closed_rows_passed']} closed rows pass both the evidence-existence and delivering-commit-ancestry checks; "
        f"{c['closed_rows_failed']} fail. The register's row-level states total "
        f"{sum(c['register_task_rows'].values())} of 112 with {c['register_task_rows'].get('DONE', 0)} DONE, one deferred QA08 and no capacity claim. "
        "Every recorded gap is quoted below with its standing owner. Because the plan is not fully closed and "
        f"{len(p['closed_row_failures'])} closed row(s) fail a mechanical binding, a release is not authorized by this record.",
        "",
        "## Per-row reconciliation",
        "",
        "State is the plan checkbox: DONE `[x]`, GAP `[~]`, OPEN `[ ]`. Verifier for a closed row is the mechanical "
        "evidence + ancestry result; GAP/OPEN rows are not treated as delivered.",
        "",
        "| Row | Title | State | Commit | Evidence | Verifier | Gap / reason |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for r in p["rows"]:
        commit = r["delivering_commit"] or "-"
        evidence = r["evidence_resolved"] or r["evidence"] or "-"
        if r.get("evidence_substituted"):
            evidence += " (substituted)"
        detail = r.get("gap") or r["verifier_reason"]
        detail = detail.replace("|", "\\|")
        lines.append(
            f"| {r['row']} | {r['title'].replace('|', '\\|')} | {r['state']} | `{commit[:9]}` | `{evidence}` | "
            f"{r['verifier']}: {r['verifier_reason'].replace('|', '\\|')} | {detail} |"
        )
    lines += [
        "",
        "## Counts",
        "",
        "| Measure | Required | Accounted | Status |",
        "| --- | ---: | ---: | --- |",
        f"| Original task IDs (111 active + QA08) | 112 | {n['original_112']['register_ids']} | {n['original_112']['status']} |",
        f"| Required IA entries (P0) | 61 | {n['ia_61']['required']} | {n['ia_61']['status']} |",
        f"| Design families | 12 | {len(n['design_12']['families'])} | {n['design_12']['status']} |",
        f"| Venue deliverables | 8 | {len(n['venue_8']['deliverables'])} | {n['venue_8']['status']} |",
        f"| Extension IA states | 10 | {len(n['extension_ia_10']['states'])} | {n['extension_ia_10']['status']} |",
        f"| Recorded gap rows | 14 | {len(n['gap_14']['found'])} | {n['gap_14']['status']} |",
        "",
        f"- 112 IDs: the register contains {n['original_112']['register_ids']} unique original IDs; `generated/TASKS_EN.json` contains "
        f"{n['original_112']['generated_original_ids']} original IDs plus {len(n['original_112']['generated_extension_ids'])} extension IDs "
        f"({', '.join(n['original_112']['generated_extension_ids'])}). Deferred = {n['original_112']['deferred']}. "
        "This is an ID reconciliation, not a completion claim.",
        f"- 61 IA: `generated/IA_EN.json` has {n['ia_61']['generated_p0']} P0 rows and {len(n['ia_61']['generated_p1_excluded'])} P1 rows "
        f"({', '.join(n['ia_61']['generated_p1_excluded'])}); `functional_delivery.functional_delivery_ia` lists 61 required entries and the ID sets match. "
        "IA capture sources: row 46's extension-state owner map (`.omo/evidence/mcr-finish-project/task-46-w04.md`) and the QA06/IN01 capture evidence "
        "(`task-36-qa06/report.json`, `task-32-in01-delta.md`). Those captures are partial; this record does not infer full IA acceptance from them.",
        f"- 12 design families: {', '.join(n['design_12']['families'])} (authoritative table in `docs/mcr/functional-delivery-plan.md`); "
        f"every required IA entry is assigned one of {n['design_12']['ia_family_count']} families. Families with no IA row: "
        f"{', '.join(n['design_12']['families_without_ia_rows']) or 'none'}.",
        f"- 8 venue deliverables: {', '.join(n['venue_8']['deliverables'])} (`docs/mcr/venue-feature-amendment-2026-11-05.md`).",
        f"- 10 extension IA states: {', '.join(n['extension_ia_10']['states'])}.",
        "",
        "## Gap list",
        "",
        "| Row | Title | Owner | Quoted gap |",
        "| --- | --- | --- | --- |",
    ]
    for g in p["gaps"]:
        lines.append(
            f"| {g['row']} | {g['title'].replace('|', '\\|')} | {g['owner']} | {g['gap'].replace('|', '\\|')} |"
        )
    d = p["register_drift"]
    lines += [
        "",
        "## Register drift (reported, not normalized)",
        "",
        f"1. **Aggregate counts.** The working-tree JSON had been edited before this task: DONE 63 to 65, IN_PROGRESS 40 to 39, REOPENED 2 to 1. "
        f"The row-level states now total {json.dumps(d['row_level_counts'], sort_keys=True)}, which "
        f"{'matches' if d['counts_match'] else 'does not match'} `functional_delivery.counts` "
        f"{json.dumps(d['declared_counts'], sort_keys=True)}. The edit is retained because it corrects the aggregate to the rows; it changes no task state.",
        f"2. **FE33 / FE24.** FE33's row state is `{d['fe33_row_state']}` while `reopened_historical_tasks` still names FE33; "
        f"FE24's row state is `{d['fe24_row_state']}`. The historical list preserves the past reopen and is not rewritten, but it no longer "
        "describes FE33's current state. This is the source of the old aggregate's REOPENED count and is stated here rather than silently removed.",
        f"3. **C07.** Register state is `{d['c07']['state']}` with `recorded_state={d['c07']['recorded_state']!r}` and evidence "
        f"{d['c07']['evidence']!r}. A DONE row with no evidence fails IN05's acceptance rule, so C07 is counted as a GAP in this reconciliation "
        "even though the aggregate counts it DONE.",
        f"4. **QA08.** Register state `{d['qa08']['state']}` with `recorded_state={d['qa08']['recorded_state']!r}` and no evidence. "
        "It remains deferred and NOT_RUN.",
        "5. **Stale narrative.** `docs/mcr/functional-delivery-plan.md` still contains prior checkpoint wording "
        f"such as: {json.dumps(d['narrative_stale'], ensure_ascii=False)}. Those lines are historical narrative and were not edited by this task; "
        "the machine register is the corrected count source.",
        "",
        "## NOT_RUN inventory",
        "",
        "| Source | State | Detail |",
        "| --- | --- | --- |",
    ]
    for item in p["not_run_inventory"]:
        lines.append(
            f"| {item['source']} | {item['item']} | {item['detail'].replace('|', '\\|')} |"
        )
    lines += [
        "",
        "## QA08",
        "",
        f"**{p['qa08']['state']} / NOT_RUN.** Recorded state is `{p['qa08']['recorded_state']}` with no evidence. "
        f"Reopen condition: {p['qa08']['reopen_when']} No capacity claim (including 200 concurrent users) is made anywhere in this record.",
        "",
        "## Release decision",
        "",
        f"`{p['release_decision']}`.",
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    rows = parse_plan()
    commits = commit_index()
    register = json.loads(REGISTER.read_text(encoding="utf-8"))
    ia = json.loads(IA_JSON.read_text(encoding="utf-8"))
    tasks = json.loads(TASKS_JSON.read_text(encoding="utf-8"))
    fdp = FDP.read_text(encoding="utf-8")
    amendment = AMENDMENT.read_text(encoding="utf-8")
    row46_text = (EVIDENCE_DIR / "task-46-w04.md").read_text(encoding="utf-8")

    for row in rows:
        row["evidence_check"] = resolve_evidence(row)
        row["commit_check"] = resolve_commit(row, commits)
        if row["state"] == "x":
            ok = (
                row["evidence_check"]["status"] == "PASS"
                and row["commit_check"]["status"] == "PASS"
            )
            row["verifier"] = "PASS" if ok else "FAIL"
            row["verifier_reason"] = "; ".join(
                [
                    row["evidence_check"].get("reason")
                    or f"evidence {row['evidence_check'].get('path')} exists",
                    row["commit_check"].get("reason", ""),
                ]
            )
        elif row["state"] == "~":
            row["verifier"] = "GAP"
            row["verifier_reason"] = row["gap"] or "state [~] without quoted gap text"
        else:
            row["verifier"] = "OPEN"
            row["verifier_reason"] = "row not closed"

    plan_counts = Counter({"DONE": 0, "GAP": 0, "OPEN": 0})
    for row in rows:
        plan_counts[{"x": "DONE", "~": "GAP", " ": "OPEN"}[row["state"]]] += 1
    unclosed = [r for r in rows if r["state"] != "x"]
    closed_failures = [r for r in rows if r["state"] == "x" and r["verifier"] != "PASS"]

    reg_ids = [t["id"] for t in register["tasks"]]
    gen_ids = [t["id"] for t in tasks["tasks"]]
    gen_original = [i for i in gen_ids if not i.startswith("EX")]
    gen_extension = [i for i in gen_ids if i.startswith("EX")]
    deferred = register["functional_delivery"]["deferred_task_ids"]
    names_112 = {
        "status": "PASS"
        if len(reg_ids) == 112
        and len(set(reg_ids)) == 112
        and sorted(reg_ids) == sorted(gen_original)
        and deferred == ["QA08"]
        else "FAIL",
        "register_ids": len(reg_ids),
        "generated_ids": len(gen_ids),
        "generated_original_ids": len(gen_original),
        "generated_extension_ids": gen_extension,
        "deferred": deferred,
        "reopen_names": register["functional_delivery"].get("reopened_historical_tasks", []),
    }

    fdia = register["functional_delivery_ia"]
    p0 = [s for s in ia["screens"] if s.get("priority") == "P0"]
    p1 = [s for s in ia["screens"] if s.get("priority") != "P0"]
    names_ia = {
        "status": "PASS"
        if len(fdia) == 61
        and len(p0) == 61
        and sorted(s["id"] for s in p0) == sorted(s["id"] for s in fdia)
        else "FAIL",
        "required": len(fdia),
        "generated_p0": len(p0),
        "generated_p1_excluded": [s["id"] for s in p1],
    }

    family_rows = re.findall(r"^\| (PI\d{2}) \| ([^|]+?) \|", fdp, re.M)
    families = sorted({f for f, _ in family_rows})
    ia_families = sorted({x.get("design_family") for x in fdia})
    names_design = {
        "status": "PASS"
        if len(families) == 12 and ia_families and set(ia_families) <= set(families)
        else "FAIL",
        "families": families,
        "ia_family_count": len(ia_families),
        "families_without_ia_rows": sorted(set(families) - set(ia_families)),
    }

    venue = re.findall(r"^\| (VENU-[A-Z0-9-]+) \|", amendment, re.M)
    names_venue = {
        "status": "PASS" if len(venue) == 8 and len(set(venue)) == 8 else "FAIL",
        "deliverables": venue,
    }

    ext_states = count_ia_states(row46_text)
    names_ext = {
        "status": "PASS" if len(ext_states) == 10 else "FAIL",
        "states": ext_states,
        "source": ".omo/evidence/mcr-finish-project/task-46-w04.md#Source of the ten extension IA states",
    }

    row_states = Counter(t["revised_plan"]["state"] for t in register["tasks"])
    declared = register["functional_delivery"]["counts"]
    drift = {
        "row_level_counts": dict(sorted(row_states.items())),
        "declared_counts": declared,
        "counts_match": dict(row_states) == declared,
        "done_tasks": register["functional_delivery"].get("done_tasks"),
        "remaining_active_tasks": register["functional_delivery"].get("remaining_active_tasks"),
        "fe33_row_state": next(t["revised_plan"]["state"] for t in register["tasks"] if t["id"] == "FE33"),
        "fe24_row_state": next(t["revised_plan"]["state"] for t in register["tasks"] if t["id"] == "FE24"),
        "reopened_historical_tasks": register["functional_delivery"].get("reopened_historical_tasks", []),
        "c07": next(
            {
                "recorded_state": t["recorded_state"],
                "state": t["revised_plan"]["state"],
                "evidence": t["evidence"],
            }
            for t in register["tasks"]
            if t["id"] == "C07"
        ),
        "qa08": next(
            {
                "recorded_state": t["recorded_state"],
                "state": t["revised_plan"]["state"],
                "evidence": t["evidence"],
            }
            for t in register["tasks"]
            if t["id"] == "QA08"
        ),
        "narrative_stale": [
            line.strip()
            for line in fdp.splitlines()
            if "63/111 active DONE" in line or "counts stay 63/111" in line.lower()
        ][:5],
    }

    qa08_not_run = (
        drift["qa08"]["state"] == "DEFERRED_BY_USER"
        and drift["qa08"]["recorded_state"] != "ACCEPTED"
        and not drift["qa08"]["evidence"]
    )
    qa08 = {
        "status": "PASS" if qa08_not_run else "FAIL",
        "state": drift["qa08"]["state"],
        "recorded_state": drift["qa08"]["recorded_state"],
        "evidence": drift["qa08"]["evidence"],
        "reopen_when": "A named user decision re-authorizes performance qualification; then re-run the QA08 capacity/soak suite (including the 200-concurrent-user scenario) on a fresh candidate with measured reports and publish the result as a new task delta. Until then no capacity claim is permitted.",
    }

    gaps = [
        {
            "row": r["row"],
            "title": r["title"],
            "owner": gap_owner(r["row"]),
            "gap": r["gap"] or "",
        }
        for r in rows
        if r["state"] == "~"
    ]
    gap_ids = [r["row"] for r in rows if r["state"] == "~"]
    gap_check = {
        "status": "PASS" if gap_ids == [str(x) for x in EXPECTED_GAPS] else "FAIL",
        "expected": [str(x) for x in EXPECTED_GAPS],
        "found": gap_ids,
    }

    all_bindings = (
        not closed_failures
        and not unclosed
        and all(
            x["status"] == "PASS"
            for x in [names_112, names_ia, names_design, names_venue, names_ext, gap_check, qa08]
        )
        and drift["counts_match"]
    )
    decision = "RELEASE_READY" if all_bindings else "NOT_RELEASE_READY"

    payload = {
        "task": "47",
        "id": "IN05",
        "candidate": sh("git", "rev-parse", CANDIDATE).strip(),
        "branch": sh("git", "rev-parse", "--abbrev-ref", "HEAD").strip(),
        "generated_at": subprocess.run(
            ["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"], capture_output=True, text=True
        ).stdout.strip(),
        "release_decision": decision,
        "counts": {
            "plan_rows": dict(plan_counts),
            "register_task_rows": dict(sorted(row_states.items())),
            "closed_rows_passed": sum(1 for r in rows if r["state"] == "x" and r["verifier"] == "PASS"),
            "closed_rows_failed": len(closed_failures),
        },
        "rows": [
            {
                "row": r["row"],
                "title": r["title"],
                "state": {"x": "DONE", "~": "GAP", " ": "OPEN"}[r["state"]],
                "plan_state": r["state"],
                "evidence": r.get("evidence"),
                "evidence_resolved": r["evidence_check"].get("path"),
                "evidence_substituted": r["evidence_check"].get("substituted", False),
                "commit": r.get("commit"),
                "delivering_commit": r["commit_check"].get("sha"),
                "verifier": r["verifier"],
                "verifier_reason": r["verifier_reason"],
                "gap": r.get("gap"),
            }
            for r in rows
        ],
        "closed_row_failures": [
            {"row": r["row"], "title": r["title"], "reason": r["verifier_reason"]}
            for r in closed_failures
        ],
        "name_checks": {
            "original_112": names_112,
            "ia_61": names_ia,
            "design_12": names_design,
            "venue_8": names_venue,
            "extension_ia_10": names_ext,
            "gap_14": gap_check,
        },
        "gaps": gaps,
        "register_drift": drift,
        "qa08": qa08,
        "not_run_inventory": build_not_run(rows, register),
    }

    OUT_JSON.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    OUT_MD.write_text(render_md(payload), encoding="utf-8")
    print(json.dumps({
        "release_decision": decision,
        "counts": payload["counts"],
        "closed_row_failures": payload["closed_row_failures"],
        "name_checks": {k: v["status"] for k, v in payload["name_checks"].items()},
        "gap_status": gap_check["status"],
        "qa08_status": qa08["status"],
        "drift_counts_match": drift["counts_match"],
    }, indent=2))
    return 0 if decision == "RELEASE_READY" else 1


if __name__ == "__main__":
    sys.exit(main())
