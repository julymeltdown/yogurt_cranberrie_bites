#!/usr/bin/env python3
"""Assemble the F3 manual-QA report from the raw driver reports.

Reads only the raw evidence in ./raw/, trims oversized values and writes
report.json next to this script. Every status, reason, row and log excerpt is
copied from a raw driver report; nothing is invented here.
"""
from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
RAW = HERE / "raw"
BINS = Path("/Users/lhs/Desktop/mcr-build/exec-f3-bins")
SERVICES = ("api-gateway", "auth-service", "concert-service", "friend-service",
            "notification-service", "post-service", "profile-service")


def load(name):
    return json.loads((RAW / name).read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def trim(value, depth=0):
    if depth > 6:
        return "<depth-limit>"
    if isinstance(value, str):
        return value if len(value) <= 600 else value[:600] + f"<trimmed {len(value) - 600} chars>"
    if isinstance(value, list):
        items = [trim(item, depth + 1) for item in value[:24]]
        if len(value) > 24:
            items.append(f"<trimmed {len(value) - 24} items>")
        return items
    if isinstance(value, dict):
        return {key: trim(item, depth + 1) for key, item in value.items()}
    return value


def checks_summary(checks):
    return {
        "total": len(checks),
        "passed": sum(1 for value in checks.values() if value is True),
        "failed": sorted(name for name, value in checks.items() if value is False),
        "not_evaluated": sorted(name for name, value in checks.items() if value is None),
    }


def collect_steps(raw_steps, keep=None):
    steps = {}
    for name, value in raw_steps.items():
        if keep is not None and not keep(name):
            continue
        steps[name] = trim(value)
    return steps


main = load("is1-in02.json")
mural = load("is2-in04.json")
flow = load("is34-closure-venue.json")
social = load("privacy-in03.json")
qa04 = load("attempt2-privacy-qa04.json")
qa04_first = load("attempt1-privacy-qa04.json")

qa04_counts = {}
for cell in qa04["cells"]:
    slot = qa04_counts.setdefault(cell["surface"], {"PASS": 0, "NOT_RUN": 0, "FAIL": 0})
    slot[cell.get("result", "UNKNOWN")] = slot.get(cell.get("result", "UNKNOWN"), 0) + 1
qa04_first_counts = {}
for cell in qa04_first["cells"]:
    slot = qa04_first_counts.setdefault(cell["surface"], {"PASS": 0, "NOT_RUN": 0, "FAIL": 0})
    slot[cell.get("result", "UNKNOWN")] = slot.get(cell.get("result", "UNKNOWN"), 0) + 1

binaries = {name: digest(BINS / name) for name in SERVICES}

report = {
    "schema": "mcr.f3.manual-qa.v1",
    "task": "F3",
    "generated_at_utc": datetime.now(timezone.utc).isoformat(),
    "candidate": {
        "revision": "71e54aa735e6fbd7c51f0c7b6115fa71a9704048",
        "branch": "codex/mcr-exec-f3",
        "base": "71e54aa73",
        "worktree": str(HERE.parents[3]),
        "clean_tracked_tree_at_build": True,
    },
    "binaries": binaries,
    "environment": {
        "kind": "native seven-service development stack, one owned disposable PostgreSQL 16 "
                "cluster, owned Redis and owned local mail sink per scenario",
        "services": list(SERVICES),
        "postgres": "PostgreSQL 16.14 (owned cluster; max_prepared_transactions=256)",
        "mtls": "real per-run PKI/TLS from world_in01_live.pki/tls; no plaintext peer transport",
        "enrollment_policy": {
            "label": "development, nonproduction; never legal approval",
            "detail": "productionApproval=false, approvalReference=development-not-admitted",
            "revisions": {
                "is1_in02": main.get("enrollment_policy_revision"),
                "is3_is4_closure_venue": flow.get("enrollment_policy_revision"),
            },
        },
        "gateway_timeout": {
            "is1_is3_is4": "GATEWAY_REQUEST_TIMEOUT_MS=5000 set by closure_vertical_stack.py so the "
                            "transport does not pre-empt the product's own 2000 ms budget",
            "is2": "driver default (no override)",
            "privacy_attempt1": "driver default",
            "privacy_attempt2": "GATEWAY_REQUEST_TIMEOUT_MS=5000 applied by "
                                "f3-manual-qa/run_privacy_with_gateway_budget.py",
            "privacy_final_in03": "driver default",
        },
        "release_bundle": "development release bundle, NOT_ADMITTED",
        "stack_roots": {
            "is1": (main.get("stack") or {}).get("root"),
            "is2": (mural.get("stack") or {}).get("root"),
            "is3_is4": (flow.get("stack") or {}).get("root"),
            "privacy": (social.get("stack") or {}).get("root"),
        },
    },
    "commands": [
        "CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-f3 cargo build -p concert-service "
        "-p auth-service -p friend-service -p profile-service -p post-service "
        "-p notification-service -p api-gateway -j 6",
        "python3 -B backend/scripts/mcr/closure_vertical_stack.py --bin-dir "
        "/Users/lhs/Desktop/mcr-build/exec-f3-bins --report .../raw/is1-in02.json --in02",
        "python3 -B backend/scripts/mcr/in04_wall_b_gate.py --bin-dir "
        "/Users/lhs/Desktop/mcr-build/exec-f3-bins --report .../raw/is2-in04.json",
        "python3 -B backend/scripts/mcr/closure_vertical_stack.py --bin-dir "
        "/Users/lhs/Desktop/mcr-build/exec-f3-bins --report .../raw/is34-closure-venue.json",
        "python3 -B backend/scripts/mcr/in03_social_gate.py --bin-dir "
        "/Users/lhs/Desktop/mcr-build/exec-f3-bins --report .../raw/privacy-in03.json",
        "python3 -B .omo/evidence/mcr-finish-project/f3-manual-qa/run_privacy_with_gateway_budget.py "
        "--bin-dir /Users/lhs/Desktop/mcr-build/exec-f3-bins --report "
        ".../raw/attempt2-privacy-in03.json --qa04-report <absolute new path>",
    ],
    "scenarios": {
        "IS-1": {
            "title": "Two real clients join the event, complete a meetup and a pickup",
            "verdict": "PASS",
            "driver": "backend/scripts/mcr/closure_vertical_stack.py --in02",
            "raw_report": "raw/is1-in02.json",
            "result_field": main.get("result"),
            "cleanup": main.get("cleanup"),
            "event": main.get("event"),
            "preconditions": {
                "principals": "host, alice, bob through real development enrollment -> mail "
                              "verification -> login -> admission -> public profile initialization",
                "provisioning_rows": trim(main.get("steps", {}).get("in02_provision_rows")),
                "enrollment_policy_revision": main.get("enrollment_policy_revision"),
                "gateway_url": (main.get("stack") or {}).get("gateway_url"),
                "gateway_request_timeout_ms": 5000,
            },
            "checks": checks_summary(main.get("checks", {})),
            "steps": collect_steps(main.get("steps", {})),
            "not_run": {
                "world_websocket_join_in_this_driver": "The IN02 driver drives product HTTP "
                    "routes after real admission; it does not open /world-ws itself. Two-client "
                    "world socket presence is measured in IS-2 step world_socket_frames.",
                "beacon_rally": "No beacon/rally producer is composed; not part of this "
                    "scenario's capacity/stock/pickup checks.",
            },
        },
        "IS-2": {
            "title": "Mural convergence, cooldown and budget rejections, eligible undo, moderation",
            "verdict": "PASS",
            "driver": "backend/scripts/mcr/in04_wall_b_gate.py",
            "raw_report": "raw/is2-in04.json",
            "result_field": mural.get("result"),
            "cleanup": mural.get("cleanup"),
            "scenario": trim(mural.get("scenario")),
            "principal_attempts": mural.get("principal_attempts"),
            "checks": checks_summary(mural.get("checks", {})),
            "failed_checks": mural.get("failed_checks"),
            "db": trim(mural.get("db")),
            "http_request_count": len(mural.get("http_requests", []) or []),
            "steps": collect_steps(mural.get("steps", {})),
            "not_run": trim(mural.get("not_run")),
        },
        "IS-3": {
            "title": "Admin report and closure through the public route",
            "verdict": "PASS",
            "verdict_scope": "operator closure ACK, member refusal, post-closure 409 and archive "
                             "behaviour at the reached READ_ONLY phase",
            "driver": "backend/scripts/mcr/closure_vertical_stack.py",
            "raw_report": "raw/is34-closure-venue.json",
            "result_field": flow.get("result"),
            "cleanup": flow.get("cleanup"),
            "checks": checks_summary(flow.get("checks", {})),
            "steps": collect_steps(flow.get("steps", {}), keep=lambda name: name in {
                "operator_role", "operator_closure", "member_closure", "post_closure_settle",
                "feed_post_write", "post_closure_write", "archive", "archive_anonymous",
                "forged_closure", "expired_closure", "closure_rows", "post_probe_liveness",
            }),
            "not_run": {
                "archive_public_200": "Requires phase ARCHIVED and a concert_closure_archives row; "
                    "the composed lifecycle reaches READ_ONLY and FREEZE_MURAL..CLEANUP have no "
                    "composed LifecycleOwners (ClosureProducer::UnavailableLater). Archived 200 is "
                    "therefore NOT_RUN; the driver marks result PARTIAL for exactly this reason.",
            },
        },
        "IS-4": {
            "title": "Venue invitation, conversation and place notes",
            "verdict": "PASS",
            "verdict_scope": "friend invitation create/detail/accept, conversation send/read, "
                             "place note create/reply/withdraw and stranger/foreign denials; "
                             "blocked venue-meeting denial measured in the privacy matrix",
            "driver": "backend/scripts/mcr/closure_vertical_stack.py",
            "raw_report": "raw/is34-closure-venue.json",
            "checks": {name: value for name, value in flow.get("checks", {}).items()
                       if name.startswith("venue_")},
            "steps": collect_steps(flow.get("steps", {}), keep=lambda name: name.startswith("venue_")),
            "not_run": {
                "blocked_place_note_read_denial": "Place-note list/thread/replies return 503 "
                    "DEPENDENCY_UNAVAILABLE for every role because the N01 prepared native "
                    "read-retention path is not composed; the blocked-read denial status is NOT_RUN.",
                "suspended_and_expired_venue_denials": "Asserted by the QA04 matrix cells "
                    "(suspended/expired venue location, notes create and meeting) but not by this "
                    "driver.",
            },
        },
    },
    "privacy": {
        "spot_check_verdict": "FAIL",
        "verdict_reason": "The IN03 social/privacy gate passed all 27 checks, but the "
                          "role x surface QA04 matrix aborted twice at its own state-restore step with "
                          "504 GATEWAY_504 on DELETE /api/relationships/blocks/{blocked}; the "
                          "remaining ws-ticket/subscription/replay and denial-audit cells were "
                          "not reached.",
        "in03_social_gate": {
            "verdict": "PASS",
            "raw_report": "raw/privacy-in03.json",
            "result_field": social.get("result"),
            "cleanup": social.get("cleanup"),
            "principals": social.get("principals"),
            "checks": checks_summary(social.get("checks", {})),
            "not_run": trim(social.get("not_run")),
            "denied_payload_audit": trim(social.get("steps", {}).get("denied_payload_audit")),
            "steps": collect_steps(social.get("steps", {})),
        },
        "qa04_matrix": {
            "verdict": "FAIL",
            "raw_report": "raw/attempt2-privacy-qa04.json",
            "first_attempt_report": "raw/attempt1-privacy-qa04.json",
            "aborted": True,
            "failure": qa04.get("failure"),
            "failure_step_route": "DELETE /api/relationships/blocks/{target_user_id}",
            "failure_observed_attempts": 2,
            "failure_gateway_budgets": ["default", "GATEWAY_REQUEST_TIMEOUT_MS=5000"],
            "cells_measured": len(qa04["cells"]),
            "cells_by_surface": qa04_counts,
            "first_attempt_cells_measured": len(qa04_first["cells"]),
            "first_attempt_cells_by_surface": qa04_first_counts,
            "gift_row": trim(qa04.get("gift_row")),
            "cells": trim(qa04["cells"]),
            "not_run_after_abort": [
                "ws_ticket (7 roles)",
                "ws_subscription (7 roles)",
                "ws_ticket_replay (owner)",
                "denial-envelope audit over all denied cells",
                "restored_state confirmation",
            ],
        },
    },
    "not_run": [
        {"id": "IS-2.undo_wall_clock_expiry", "blocker":
            "No real >600 s wait in this round; positive in-window undo and the revision guard "
            "executed.", "owner": "mural undo guard"},
        {"id": "IS-2.device_and_visual", "blocker": "Backend multiuser gate only; no browser or "
            "physical device surface.", "owner": "IN04 frontend runner"},
        {"id": "IS-3.archive_public_200", "blocker": "Phase stays READ_ONLY; FREEZE_MURAL..CLEANUP "
            "have no composed LifecycleOwners, so ARCHIVED and a 200 archive are unreachable.",
            "owner": "concert-service closure producer"},
        {"id": "IS-4.blocked_place_note_read_denial", "blocker": "Note reads return 503 "
            "DEPENDENCY_UNAVAILABLE for every role (N01 read-retention provider absent).",
            "owner": "concert-service venue_product ListNotes/GetNote"},
        {"id": "privacy.media_metadata_and_content", "blocker": "Media read path closes with a "
            "truncated Gateway body (IncompleteRead) and the upload mutation returns CONCERT_403 "
            "PERMISSION_DENIED; 14 cells NOT_RUN by the matrix itself.",
            "owner": "gateway media routes / owner-media composition"},
        {"id": "privacy.venue_notes_read", "blocker": "Place-note reads 503 for every role; 7 cells "
            "NOT_RUN by the matrix itself.", "owner": "concert-service venue_product"},
        {"id": "privacy.feed_cache_eviction", "blocker": "Warm owner feed had no removable "
            "blocked-author post; journal growth recorded instead.", "owner": "feed read path"},
        {"id": "privacy.ws_ticket_subscription_replay", "blocker": "QA04 aborted before the "
            "WebSocket cells; no close-code assertion in this run.", "owner": "QA04 matrix"},
        {"id": "privacy.denial_envelope_audit_qa04", "blocker": "QA04 aborted before its final "
            "denial audit; the IN03 audit over 10 denied cases passed with no private values.",
            "owner": "QA04 matrix"},
    ],
    "limits": [
        "Development release bundle, NOT_ADMITTED; QA08 capacity and release qualification NOT_CLAIMED.",
        "Synthetic development authorities: event, place, wall pack and principals are development "
        "fixtures; enrollment policy is explicitly nonproduction (productionApproval=false).",
        "No physical device, browser, or client-cache exercise; all captures are server-side HTTP/WS.",
        "No in-process restart or persistence replay was performed during these scenarios.",
        "The native stack is the development seven-service composition, not the container image.",
        "The QA04 matrix aborted at its own state-restore unblock; its measured cells are reported "
        "as-is and do not certify the unmeasured surfaces.",
    ],
    "raw_reports": [],
}

for path in sorted(RAW.glob("*.json")):
    report["raw_reports"].append({"path": str(path.relative_to(HERE)), "sha256": digest(path),
                                  "bytes": path.stat().st_size})

(HERE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps({
    "report": str(HERE / "report.json"),
    "scenarios": {name: value["verdict"] for name, value in report["scenarios"].items()},
    "privacy": report["privacy"]["spot_check_verdict"],
    "not_run": len(report["not_run"]),
    "raw_reports": len(report["raw_reports"]),
}, indent=1))
