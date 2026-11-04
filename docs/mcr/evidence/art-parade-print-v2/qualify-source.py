#!/usr/bin/env python3
"""Run ONLY after an actual named Resource21d offer and explicit ACCEPT."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = Path(__file__).resolve().parent
ASSETS = ROOT / "frontend/public/reference-parade-print-v2"
ART = "4d297ce650a84d43b67203826c7a10dbfa21696d"
GIB = 1024 ** 3


def stamp():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inventory():
    files = [p for root in [ASSETS, EVIDENCE] for p in root.rglob("*") if p.is_file()]
    files.append(ROOT / "docs/mcr/art/reference-parade-print-v2.md")
    return {str(p.relative_to(ROOT)): {"bytes": p.stat().st_size, "sha256": digest(p)} for p in files}


def size(root):
    return sum(p.stat().st_size for p in root.rglob("*") if p.is_file()) if root.exists() else 0


def owned_processes():
    # Includes this guard, descendants and the short-lived ps sampler itself.
    snapshot = subprocess.check_output(["ps", "-axo", "pid=,ppid=,rss="], text=True, timeout=2)
    rows = [tuple(map(int, line.split())) for line in snapshot.splitlines()]
    owned = {os.getpid()}
    while True:
        more = {pid for pid, ppid, _ in rows if ppid in owned}
        if more <= owned:
            break
        owned |= more
    selected = [(pid, rss * 1024) for pid, _, rss in rows if pid in owned]
    return selected


def stop(child, cleanup_seconds):
    # Every child is launched in a new session; never signal the caller's group.
    if child is None or child.poll() is not None:
        return
    for sig, seconds in [(signal.SIGTERM, min(2, cleanup_seconds / 2)), (signal.SIGKILL, min(5, cleanup_seconds / 2))]:
        try:
            os.killpg(child.pid, sig)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=seconds)
            return
        except subprocess.TimeoutExpired:
            pass
    raise RuntimeError("Owned child failed to stop within cleanup budget")


def interrupted(number, _frame):
    raise InterruptedError("Guard received signal " + str(number))


def main(args):
    assert re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,95}", args.offer), "Unsafe offer name"
    assert args.total_seconds >= args.work_seconds + args.cleanup_seconds
    assert args.cleanup_seconds >= 10
    deadline = dt.datetime.fromisoformat(args.deadline_utc.replace("Z", "+00:00")).timestamp()
    assert deadline > time.time() + args.cleanup_seconds, "Offer has no remaining work window"
    accepted_time = dt.datetime.fromisoformat(args.accepted_at.replace("Z", "+00:00")).timestamp()
    assert accepted_time <= time.time() < deadline
    assert args.accept_free_bytes >= 16 * GIB, "Recorded actual acceptance floor below16GiB"
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    assert head == args.head, "Frozen HEAD mismatch"
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT), "Checkout must be clean"
    assert not (EVIDENCE / ".qualification-work").exists(), "Unreleased previous work directory"
    candidate = (ROOT / args.candidate_source).resolve() if args.candidate_source else None
    candidate_output = (ROOT / args.candidate_output).resolve() if args.candidate_output else None
    assert bool(candidate) == bool(candidate_output), "Candidate source and new output directory must be paired"
    if candidate:
        assert candidate.is_relative_to(EVIDENCE) and candidate.is_file()
        assert candidate_output.is_relative_to(EVIDENCE) and not candidate_output.exists()
    before = inventory()
    manifest = json.loads((ASSETS / "placement.json").read_text())
    references = [ROOT / x["path"] for x in manifest["references"]]
    reference_bytes = sum(p.stat().st_size for p in references)
    for pin in manifest["files"]:
        assert digest(ROOT / pin["path"]) == pin["sha256"]
    for pin, path in zip(manifest["references"], references):
        assert digest(path) == pin["sha256"]
    source_free = shutil.disk_usage(ROOT).free
    assert source_free >= 8 * GIB, "Running floor below8GiB; no new per-child16GiB floor"
    carry = sum(x["bytes"] for x in before.values())
    git_baseline = None
    if args.git_baseline_reserve_bytes:
        git_admin = Path(subprocess.check_output(["git", "rev-parse", "--absolute-git-dir"], cwd=ROOT, text=True).strip())
        assert (git_admin / "commondir").is_file(), "Only private worktree administration, not shared common Git"
        git_bytes = size(git_admin) + (ROOT / ".git").stat().st_size
        assert git_bytes <= args.git_baseline_reserve_bytes, "Private Git baseline exceeds fixed reserve"
        git_baseline = {"path": str(git_admin), "actualBytes": git_bytes, "wholeReserveBytes": args.git_baseline_reserve_bytes,
                        "policy": "Private Git baseline counted only in WHOLE. Shared common Git and foreign checkouts are read-only/excluded; no recursive common-store scan."}
    assert carry + reference_bytes + args.git_baseline_reserve_bytes < args.whole_bytes
    assert carry + args.log_bytes + 65536 < args.durable_bytes, "Reserve bounded log+receipt capacity"
    runs = EVIDENCE / "runs"
    runs.mkdir(exist_ok=True)
    receipt_path = runs / (args.offer + ".json")
    log_path = runs / (args.offer + ".log")
    assert not receipt_path.exists() and not log_path.exists(), "Never overwrite prior receipt"
    temporary_parent = EVIDENCE / ".qualification-work"
    temporary_parent.mkdir()
    temporary = Path(tempfile.mkdtemp(prefix=args.offer + "-", dir=temporary_parent))
    started = time.monotonic()
    receipt = {
        "task": "ART-PARADE-PRINT-02", "offer": args.offer,
        "acceptance": "Caller explicitly ACCEPTed the actual Resource21d offer; not inferred from script invocation",
        "acceptedAt": args.accepted_at, "deadlineUtc": args.deadline_utc,
        "startedAt": stamp(), "head": head, "artCommit": ART,
        "limits": vars(args), "carryBytes": carry, "readonlyReferenceBytes": reference_bytes,
        "accounting": "Whole=exclusive owned tree including temp/logs/receipts + counted readonly reference bytes + explicit fixed private Git baseline reserve; durable=owned tree excluding job temp and existing private Git baseline. Shared common Git and foreign checkouts excluded. RSS=sampled guard+descendants including ps; rusage child high-water separately recorded.",
        "privateGitBaseline": git_baseline,
        "acceptanceFreeBytes": args.accept_free_bytes,
        "sourceFreeBytes": source_free, "minimumFreeBytes": source_free,
        "peaks": {"wholeBytes": carry + reference_bytes + args.git_baseline_reserve_bytes, "tempBytes": 0, "durableBytes": carry, "rssBytes": 0, "processCount": 0},
        "commands": [], "commandFiles": {}, "temporary": str(temporary.relative_to(ROOT)),
        "inputPins": manifest["files"] + manifest["references"],
        "frozenCarryPins": before,
        "native": "NOT_RUN", "status": "RUNNING", "release": "NOT_RELEASED",
    }
    commands = [
        [sys.executable, str(EVIDENCE / "measure-reference.py"), "--report-dir", str(temporary)],
        [sys.executable, str(EVIDENCE / "render-assets.py"), "--output-dir", str(temporary / "exports"), "--report-dir", str(temporary)],
    ]
    if candidate:
        commands[1] += ["--source", str(candidate)]
        receipt["candidateSource"] = {"path": str(candidate.relative_to(ROOT)), "sha256": digest(candidate)}
    receipt["commandFiles"] = {str(p.relative_to(ROOT)): digest(p) for p in [Path(__file__), EVIDENCE / "measure-reference.py", EVIDENCE / "render-assets.py"]}
    child = None
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGHUP, interrupted)
    try:
        environment = dict(os.environ, TMPDIR=str(temporary), PYTHONDONTWRITEBYTECODE="1")
        with log_path.open("wb") as log:
            for command in commands:
                step = {"argv": command, "startedAt": stamp()}
                receipt["commands"].append(step)
                child = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                step["pid"] = child.pid
                while True:
                    processes = owned_processes()
                    owned_bytes = size(ASSETS) + size(EVIDENCE) + (ROOT / "docs/mcr/art/reference-parade-print-v2.md").stat().st_size
                    temp_bytes = size(temporary_parent)
                    measured = {"wholeBytes": owned_bytes + reference_bytes + args.git_baseline_reserve_bytes, "tempBytes": temp_bytes,
                                "durableBytes": owned_bytes - temp_bytes, "rssBytes": sum(rss for _, rss in processes), "processCount": len(processes)}
                    for key, value in measured.items():
                        receipt["peaks"][key] = max(receipt["peaks"][key], value)
                    free = shutil.disk_usage(ROOT).free
                    receipt["minimumFreeBytes"] = min(receipt["minimumFreeBytes"], free)
                    assert free >= 8 * GIB, "Running free-space floor below8GiB"
                    for key, maximum in [("wholeBytes", args.whole_bytes), ("tempBytes", args.temp_bytes), ("durableBytes", args.durable_bytes), ("rssBytes", args.rss_bytes), ("processCount", args.processes)]:
                        assert measured[key] <= maximum, (key, measured[key], maximum)
                    assert log_path.stat().st_size <= args.log_bytes, "Log cap exceeded"
                    assert time.monotonic() - started <= args.work_seconds, "Work deadline exceeded"
                    assert time.time() < deadline - args.cleanup_seconds, "Offer cleanup window reached"
                    code = child.poll()
                    if code is not None:
                        step.update(exitCode=code, finishedAt=stamp())
                        assert code == 0, (command[1], code)
                        break
                    time.sleep(.1)
                child = None
        if not candidate:
            for pin in manifest["files"]:
                if pin["path"].endswith(".png"):
                    assert digest(temporary / "exports" / Path(pin["path"]).name) == pin["sha256"], pin["path"]
            assert digest(temporary / "source-review-paper.png") == digest(EVIDENCE / "source-review-paper.png")
        assert (temporary / "reference-measurements.json").read_bytes() == (EVIDENCE / "reference-measurements.json").read_bytes()
        after = inventory()
        assert all(after.get(path) == value for path, value in before.items()), "Frozen source/evidence changed"
        receipt["freshExportsMatchFrozenPins"] = not bool(candidate)
        receipt["frozenCarryUnchanged"] = True
        if candidate:
            candidate_report = json.loads((temporary / "source-validation.json").read_text())
            assert candidate_report["source"]["sha256"] == digest(candidate)
            selected = [temporary / "exports/parade-print-preview.png", temporary / "exports/parade-print@2x.png",
                        temporary / "source-review-paper.png", temporary / "source-validation.json"]
            addition = sum(p.stat().st_size for p in selected)
            owned_bytes = size(ASSETS) + size(EVIDENCE) + (ROOT / "docs/mcr/art/reference-parade-print-v2.md").stat().st_size
            assert owned_bytes + reference_bytes + args.git_baseline_reserve_bytes + addition + 65536 <= args.whole_bytes
            assert owned_bytes - size(temporary_parent) + addition + 65536 <= args.durable_bytes
            assert time.time() < deadline - args.cleanup_seconds
            assert time.monotonic() - started <= args.work_seconds
            candidate_output.mkdir()
            for path in selected:
                shutil.copyfile(path, candidate_output / path.name)
            receipt["candidateExportPins"] = {
                str((candidate_output / path.name).relative_to(ROOT)): {"bytes": path.stat().st_size, "sha256": digest(path)}
                for path in selected
            }
            assert all(digest(ROOT / path) == value["sha256"] for path, value in receipt["candidateExportPins"].items())
            owned_bytes += addition
            receipt["peaks"]["wholeBytes"] = max(receipt["peaks"]["wholeBytes"], owned_bytes + reference_bytes + args.git_baseline_reserve_bytes)
            receipt["peaks"]["durableBytes"] = max(receipt["peaks"]["durableBytes"], owned_bytes - size(temporary_parent))
            receipt["candidateDisposition"] = "SOURCE_ONLY; retained candidate exports, no public source promotion or native claim"
        receipt["status"] = "PASS_SOURCE_ONLY"
    except BaseException as error:
        receipt["status"] = "FAILED_SOURCE_ONLY"
        receipt["failure"] = {"type": type(error).__name__, "message": str(error)}
    finally:
        cleanup_started = time.monotonic()
        try:
            stop(child, args.cleanup_seconds)
            if child is not None:
                receipt["interruptedChildExit"] = child.poll()
            receipt["temporaryOutputPins"] = {
                str(p.relative_to(temporary)): {"bytes": p.stat().st_size, "sha256": digest(p)}
                for p in temporary.rglob("*") if p.is_file()
            }
            shutil.rmtree(temporary)
            temporary_parent.rmdir()
        except BaseException as error:
            receipt["cleanupFailure"] = str(error)
            receipt["status"] = "FAILED_CLEANUP"
        receipt["cleanupSeconds"] = time.monotonic() - cleanup_started
        receipt["elapsedSeconds"] = time.monotonic() - started
        receipt["acceptanceToFinishedSeconds"] = time.time() - accepted_time
        child_rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        receipt["childMaxRssBytes"] = child_rss if sys.platform == "darwin" else child_rss * 1024
        if receipt["childMaxRssBytes"] > args.rss_bytes or receipt["cleanupSeconds"] > args.cleanup_seconds or receipt["acceptanceToFinishedSeconds"] > args.total_seconds:
            receipt["status"] = "FAILED_FINAL_LIMIT"
        try:
            remaining = [pid for pid, _ in owned_processes() if pid != os.getpid()]
        except BaseException as error:
            remaining = []
            receipt["processInspectionFailure"] = str(error)
            receipt["status"] = "FAILED_PROCESS_INSPECTION"
        # The ps sampler normally appears in its own snapshot and has already exited.
        live = []
        for pid in remaining:
            try:
                os.kill(pid, 0)
                live.append(pid)
            except ProcessLookupError:
                pass
        receipt["liveDescendants"] = live
        receipt["temporaryAbsent"] = not temporary_parent.exists()
        receipt["finishedAt"] = stamp()
        receipt["log"] = {"path": str(log_path.relative_to(ROOT)), "bytes": log_path.stat().st_size, "sha256": digest(log_path)}
        receipt["release"] = "FULL" if not live and receipt["temporaryAbsent"] and "processInspectionFailure" not in receipt else "INCOMPLETE"
        receipt["exitCode"] = 0 if receipt["status"] == "PASS_SOURCE_ONLY" and receipt["release"] == "FULL" else 1
        payload = json.dumps(receipt, indent=2) + "\n"
        assert len(payload.encode()) <= 65536, "Receipt reservation exceeded"
        receipt_path.write_text(payload)
        print(json.dumps({"receipt": str(receipt_path.relative_to(ROOT)), "status": receipt["status"], "release": receipt["release"], "exitCode": receipt["exitCode"], "peaks": receipt["peaks"]}))
    return receipt["exitCode"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ["offer", "head", "deadline-utc", "accepted-at"]:
        parser.add_argument("--" + flag, required=True)
    parser.add_argument("--candidate-source")
    parser.add_argument("--candidate-output")
    parser.add_argument("--git-baseline-reserve-bytes", type=int, default=0)
    for flag in ["total-seconds", "work-seconds", "cleanup-seconds", "whole-bytes", "temp-bytes", "durable-bytes", "rss-bytes", "processes", "log-bytes", "accept-free-bytes"]:
        parser.add_argument("--" + flag, type=int, required=True)
    raise SystemExit(main(parser.parse_args()))
