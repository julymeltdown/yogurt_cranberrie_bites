#!/usr/bin/env python3
"""Finite fresh-lease Gift build/copy OR copied pure phase. This file grants no allocation.

Only build stage accesses the named retained target. Pure stage requires an actual
zero-process/handle target-release receipt and never accesses that target.
Native SQL and services never run here.
Source/build receipts are not runtime qualification or original IN03 acceptance.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
import threading
from types import SimpleNamespace

sys.dont_write_bytecode = True
import world_in01_live as world
from gift_public_guard import (IndependentGiftGuardian, allocation_arguments,
                               allocated_bytes, qualification_guard)

ROOT = Path(__file__).resolve().parents[3]
TARGET = Path("/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target")
TOOLCHAIN = Path("/Users/lhs/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin")
INTEGRATION = ("gift_public_admission", "gift_deletion_native", "gift_publicmedia_native",
               "bootstrap_features", "report_evidence_child_contract", "post_media_child_contract",
               "shared_media_dependencies")


def require(value, reason):
    if not value:
        raise RuntimeError(reason)


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def allocated(path):
    return allocated_bytes(path)


def stop_owned_process_group(process, remaining, signal_group=None):
    """Preserve cleanup failures for the terminal report; never mask it."""
    signal_group = signal_group or os.killpg
    errors = []
    for requested_signal, seconds in ((signal.SIGTERM, 5), (signal.SIGKILL, 1)):
        if not remaining():
            break
        try:
            signal_group(process.pid, requested_signal)
        except ProcessLookupError:
            pass
        except OSError as error:
            errors.append(type(error).__name__ + ": " + str(error))
        stop = time.monotonic() + seconds
        while remaining() and time.monotonic() < stop:
            time.sleep(.1)
    try:
        process.wait(timeout=5)
    except (subprocess.TimeoutExpired, OSError) as error:
        errors.append(type(error).__name__ + ": " + str(error))
    return errors


def write_private(path, value):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")



def refresh_frozen_inputs(control_root, allocation_id, revision):
    """Own-checkout byte-identical freshness only; requires an accepted build.

    Shared warm artifacts may originate in another worktree with newer mtimes.
    Verify every selected input against its frozen Git blob before touching any;
    then verify unchanged bytes. This never reads, removes or mutates the target.
    """
    listing = subprocess.check_output(
        ["git", "ls-tree", "-r", "-z", revision, "--", "backend"], cwd=ROOT)
    records = []
    for entry in listing.split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        mode, kind, blob = metadata.split()
        relative = os.fsdecode(raw_path)
        path = ROOT / relative
        if path.suffix not in {".rs", ".toml", ".lock", ".proto", ".sql", ".json"}:
            continue
        require(kind == b"blob" and mode in {b"100644", b"100755"}
                and not path.is_symlink() and path.is_file(), "regular frozen build input required")
        data = path.read_bytes()
        actual = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        require(actual == blob.decode(), "frozen build input differs from committed source")
        records.append({"path": relative, "git_blob": actual,
                        "sha256": hashlib.sha256(data).hexdigest(), "size_bytes": len(data)})
    require(records, "frozen build inputs required")
    refreshed_ns = time.time_ns()
    for record in records:
        path = ROOT / record["path"]
        info = path.stat()
        os.utime(path, ns=(info.st_atime_ns, refreshed_ns), follow_symlinks=False)
        require(digest(path) == record["sha256"], "source bytes changed during freshness update")
    proof = control_root / (allocation_id + "-frozen-input-freshness.json")
    write_private(proof, {"source_revision": revision, "refreshed_unix_ns": refreshed_ns,
                         "input_count": len(records), "inputs": records,
                         "target_access": "NONE; own checkout bytes unchanged"})
    return {"path": str(proof), "sha256": digest(proof), "input_count": len(records)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", choices=("build", "pure"), required=True)
    parser.add_argument("--control-root", type=Path, required=True)
    parser.add_argument("--build-report", type=Path)
    parser.add_argument("--build-report-sha256")
    parser.add_argument("--target-release", type=Path)
    parser.add_argument("--target-release-sha256")
    parser.add_argument("--capacity-snapshot", type=Path, required=True)
    parser.add_argument("--capacity-snapshot-sha256", required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--allocation-id", required=True)
    for name in ("allocation-started-epoch", "allocation-work-deadline-epoch", "allocation-release-deadline-epoch",
                 "whole-timeout-seconds", "startup-timeout-seconds", "build-seconds", "pure-seconds"):
        parser.add_argument("--" + name, type=float, required=True)
    for name in ("whole-byte-limit", "retained-carry-bytes", "max-owned-processes",
                 "target-growth-limit", "output-byte-limit"):
        parser.add_argument("--" + name, type=int, required=True)
    allocation_arguments(parser)
    args = parser.parse_args()
    require(re.fullmatch(r"[A-Za-z0-9_-]{1,100}", args.allocation_id), "accepted allocation identity required")
    require(all(math.isfinite(v) and v > 0 for v in (args.allocation_started_epoch, args.allocation_work_deadline_epoch,
            args.allocation_release_deadline_epoch, args.whole_timeout_seconds,
            args.startup_timeout_seconds, args.build_seconds, args.pure_seconds)), "finite accepted bounds required")
    require(args.allocation_started_epoch <= time.time() < args.allocation_work_deadline_epoch
            and 0 < args.allocation_work_deadline_epoch - args.allocation_started_epoch <= 780
            and args.allocation_release_deadline_epoch - args.allocation_started_epoch <= 1020
            and args.allocation_release_deadline_epoch - args.allocation_work_deadline_epoch >= 240
            and 240 < args.whole_timeout_seconds <= 1020 and 0 < args.max_owned_processes <= 32
            and 0 < args.output_byte_limit <= 1024**3 and 0 < args.target_growth_limit <= 2 * 1024**3
            and args.retained_carry_bytes < args.whole_byte_limit <= 40 * 1024**3
            and args.retained_carry_bytes >= 3669547184
            and args.build_seconds <= 720 and args.pure_seconds <= 60,
            "accepted work/copy/cleanup bounds required")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    require(revision == args.source_revision and not subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=ROOT, text=True), "clean frozen source required")
    require(args.report.is_absolute() and not args.report.exists(), "new explicit private report required")
    require(args.control_root.is_absolute() and args.control_root.is_dir()
            and not args.control_root.is_symlink(), "exact private control root required")
    control_info = args.control_root.stat()
    require(control_info.st_uid == os.getuid() and control_info.st_mode & 0o077 == 0,
            "private control owner/mode required")
    git_root = Path(subprocess.check_output(["git", "rev-parse", "--git-common-dir"], cwd=ROOT, text=True).strip())
    if not git_root.is_absolute():
        git_root = (ROOT / git_root).resolve()
    source_roots = (ROOT, git_root, args.control_root)
    require(all(not args.report.parent.resolve().is_relative_to(root.resolve()) for root in source_roots),
            "build output must be separate from source/control accounting roots")
    bound_names = ("allocation_started_epoch", "allocation_work_deadline_epoch",
                   "allocation_release_deadline_epoch", "whole_byte_limit", "retained_carry_bytes",
                   "max_owned_processes", "target_growth_limit", "output_byte_limit",
                   "allocation_start_free_bytes", "allocation_stop_free_bytes")
    accepted_bounds = {name: getattr(args, name) for name in bound_names}
    snapshot = args.capacity_snapshot
    require(snapshot.is_absolute() and snapshot.is_file() and not snapshot.is_symlink()
            and snapshot.stat().st_uid == os.getuid() and snapshot.stat().st_mode & 0o077 == 0
            and digest(snapshot) == args.capacity_snapshot_sha256, "private actual capacity snapshot required")
    capacity = json.loads(snapshot.read_text())
    require(capacity["allocation_id"] == args.allocation_id
            and capacity["source_revision"] == revision
            and type(capacity["other_growth_bytes"]) is int and capacity["other_growth_bytes"] >= 0
            and type(capacity["other_workflows"]) is int and 0 <= capacity["other_workflows"] <= 3
            and type(capacity["other_source_workflows"]) is int and 0 <= capacity["other_source_workflows"] <= 2
            and type(capacity["other_browser_workflows"]) is int and 0 <= capacity["other_browser_workflows"] <= 2
            and capacity["claims"] and capacity["predecessor_target_release_receipt"],
            "complete actual named capacity claims and predecessor release required")
    args.allocation_continuation = args.stage == "pure"
    args.allocation_reserved_new_bytes = 3 * 1024**3
    args.allocation_other_growth_bytes = capacity["other_growth_bytes"]
    prior = None
    if args.stage == "pure":
        for path, sha in ((args.build_report, args.build_report_sha256),
                          (args.target_release, args.target_release_sha256)):
            require(path is not None and path.is_absolute() and not path.is_symlink()
                    and path.is_file() and digest(path) == sha, "exact build/release receipt pin required")
        prior = json.loads(args.build_report.read_text())
        release = json.loads(args.target_release.read_text())
        require(prior["result"] == "PASS" and prior["stage"] == "build"
                and prior.get("accounting_includes_full_target_baseline") is True
                and prior["accepted_bounds"] == accepted_bounds
                and prior["capacity_snapshot_sha256"] == args.capacity_snapshot_sha256
                and prior["allocation_id"] == args.allocation_id and prior["source_revision"] == revision
                and prior["accepted_work_deadline_epoch"] == args.allocation_work_deadline_epoch
                and prior["accepted_release_deadline_epoch"] == args.allocation_release_deadline_epoch
                and prior["accounting_carry_bytes"] == args.retained_carry_bytes,
                "pure phase must retain original accepted build bounds")
        require(release["kind"] == "GIFT_TARGET_RELEASE" and release["allocation_id"] == args.allocation_id
                and release["source_revision"] == revision and release["build_report_sha256"] == args.build_report_sha256
                and release["target_process_count"] == 0 and release["target_handle_count"] == 0
                and prior["finished_unix"] <= release["released_unix"] <= time.time(),
                "actual target release required before pure")
        # Deliberately do not stat, resolve, measure or enumerate TARGET here.
        baseline = prior["initial_target_allocated_bytes"]
        source_baseline = prior["source_baseline_allocated_bytes"]
    else:
        require(time.time() - 60 <= capacity["observed_unix"] <= time.time()
                and capacity["valid_until_unix"] >= time.time(), "fresh actual capacity observation required")
        free = shutil.disk_usage(args.report.parent).free
        require(free >= args.allocation_start_free_bytes
                and free - capacity["other_growth_bytes"] - 3 * 1024**3 >= 9 * 1024**3,
                "actual after-ALL admission floor required before target inspection")
        # Source/guard checks precede children; allocation acceptance is external.
        require(TARGET.is_dir() and not TARGET.is_symlink(), "exact retained target required")
        baseline = allocated(TARGET)
        source_baseline = {str(root): allocated(root) for root in source_roots}
    require(args.retained_carry_bytes >= baseline + sum(source_baseline.values()),
            "retained carry must include full target and all source roots")
    guard = qualification_guard(world.QualificationGuard, args, SimpleNamespace(cluster=None))
    original_check = guard.check
    peak_growth = prior["max_target_growth_bytes"] if prior else 0
    peak_source_growth = prior["max_source_growth_bytes"] if prior else 0
    peak_outside = prior["max_outside_bytes"] if prior else 0
    copied_bytes = prior["cumulative_copied_bytes"] if prior else 0
    check_lock = threading.RLock()
    rss_peak = prior["max_sampled_rss_bytes"] if prior else 0
    if prior:
        guard.process_peak = prior["bounds"]["max_sampled_owned_processes"]
    process = None
    guardian = None
    groups = set()

    def process_check():
        nonlocal rss_peak
        rows = subprocess.check_output(["/bin/ps", "-axo", "pid=,ppid=,pgid=,rss="], text=True, timeout=2)
        entries = {int(p[0]): (int(p[1]), int(p[2]), int(p[3]) * 1024)
                   for row in rows.splitlines() if len(p := row.split()) == 4}
        owned = {os.getpid()} | {pid for pid, (_, group, _) in entries.items() if group in groups}
        while True:
            more = {pid for pid, (parent, _, _) in entries.items() if parent in owned} - owned
            if not more:
                break
            owned.update(more)
        rss = sum(entries.get(pid, (0, 0, 0))[2] for pid in owned)
        rss_peak = max(rss_peak, rss)
        guard.process_peak = max(guard.process_peak, len(owned))
        require(len(owned) <= args.max_owned_processes and rss <= 8 * 1024**3,
                "accepted all-process/RSS bound exceeded")
    guard.process_check = process_check

    def check():
        nonlocal peak_growth, peak_source_growth, peak_outside
        with check_lock:
            original_check()
            if args.stage == "build":
                peak_growth = max(peak_growth, max(0, allocated(TARGET) - baseline))
            growth = sum(max(0, allocated(root) - source_baseline[str(root)]) for root in source_roots)
            peak_source_growth = max(peak_source_growth, growth)
            peak_outside = max(peak_outside, max(copied_bytes, allocated(args.report.parent)) + peak_source_growth)
            require(peak_growth <= args.target_growth_limit, "accepted target growth exceeded")
            require(peak_source_growth <= 128 * 1024**2, "accepted source/Git/control growth exceeded")
            require(peak_outside <= args.output_byte_limit, "inclusive outside/cumulative copy cap exceeded")
            require(peak_growth + peak_outside <= 3 * 1024**3,
                    "accepted ALL-new-growth cap exceeded")
            require(guard.carry + peak_growth + peak_outside <= args.whole_byte_limit,
                    "accepted nonshrinking whole carry exceeded")
            guard.maximum = max(guard.maximum, guard.carry + peak_growth + peak_outside)

    guard.check = check
    env = {key: value for key, value in os.environ.items()
           if "RUSTFLAGS" not in key and key not in {"RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTC_BOOTSTRAP"}}
    env.update(CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0",
               CARGO_PROFILE_TEST_DEBUG="0", RUSTC=str(TOOLCHAIN / "rustc"))
    if args.stage == "build":
        env["CARGO_TARGET_DIR"] = str(TARGET)
    else:
        env.pop("CARGO_TARGET_DIR", None)
    report = {"scope": "GIFT_PUBLIC_SOURCE_BUILD", "stage": args.stage, "result": "FAIL",
              "source_revision": revision, "allocation_id": args.allocation_id,
              "accepted_bounds": accepted_bounds,
              "capacity_snapshot_sha256": args.capacity_snapshot_sha256,
              "accepted_work_deadline_epoch": args.allocation_work_deadline_epoch,
              "accepted_release_deadline_epoch": args.allocation_release_deadline_epoch,
              "accounting_carry_bytes": args.retained_carry_bytes,
              "runtime": "NOT_RUN", "pure": "NOT_RUN", "commands": [],
              "binaries": dict(prior["binaries"]) if prior else {}, "copies": [],
              "target": str(TARGET), "initial_target_allocated_bytes": baseline,
              "accounting_includes_full_target_baseline": True,
              "source_baseline_allocated_bytes": source_baseline}

    def remaining_group_processes():
        rows = subprocess.check_output(["/bin/ps", "-axo", "pid=,pgid="], text=True, timeout=5)
        return [int(parts[0]) for row in rows.splitlines()
                if len(parts := row.split()) == 2 and int(parts[1]) in groups]

    def interrupt(_sig, _frame):
        raise KeyboardInterrupt("Gift build allocation stopped")

    previous = {sig: signal.signal(sig, interrupt) for sig in (signal.SIGTERM, signal.SIGINT)}

    def execute(argv, label):
        nonlocal process
        out, err = args.report.parent / (label + ".stdout"), args.report.parent / (label + ".stderr")
        check()
        with os.fdopen(os.open(out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as stdout, \
             os.fdopen(os.open(err, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as stderr:
            process = subprocess.Popen(argv, cwd=ROOT / "backend", env=env, stdout=stdout, stderr=stderr, start_new_session=True)
            groups.add(process.pid)
            while process.poll() is None:
                check()
                time.sleep(.5)
            code = process.wait()
        report["commands"].append({"argv": argv, "exit_code": code, "stdout": str(out), "stderr": str(err)})
        check()
        require(code == 0, "Gift build/pure command failed: " + label)
        require(not remaining_group_processes(), "owned build/test group still has processes")
        groups.discard(process.pid)
        process = None
        return out

    def copy(executable, name):
        nonlocal copied_bytes
        require(executable.is_absolute() and not executable.is_symlink()
                and executable.resolve().is_relative_to(TARGET.resolve()), "foreign compiler output")
        info = executable.stat()
        require(stat.S_ISREG(info.st_mode) and os.access(executable, os.X_OK), "regular compiled executable required")
        require(peak_source_growth + max(copied_bytes + info.st_size,
                allocated(args.report.parent) + info.st_size + 1024**2) <= args.output_byte_limit,
                "copy would exceed cumulative copy or physical output cap")
        destination = args.report.parent / name
        require(not destination.exists(), "new copied path required")
        expected = digest(executable)
        shutil.copyfile(executable, destination)
        copied_bytes += info.st_size  # Never reset when a temporary copy is removed.
        destination.chmod(0o700)
        require(digest(destination) == expected == digest(executable), "copied executable drift")
        report["copies"].append({"path": str(destination), "sha256": expected,
                                 "source_revision": revision, "size_bytes": info.st_size})
        check()
        return destination, expected

    try:
        guardian_config = dict(allocation_id=args.allocation_id, stage=args.stage,
            output=str(args.report.parent), carry=guard.carry, work_deadline=args.allocation_work_deadline_epoch,
            full_deadline=args.allocation_release_deadline_epoch,
            start_free=args.allocation_start_free_bytes, stop_free=args.allocation_stop_free_bytes,
            other_growth=capacity["other_growth_bytes"], process_cap=args.max_owned_processes,
            target_peak=peak_growth, source_peak=peak_source_growth, outside_peak=peak_outside,
            source_baseline=source_baseline, target_cap=args.target_growth_limit,
            outside_cap=args.output_byte_limit, whole_cap=args.whole_byte_limit)
        if args.stage == "build":
            guardian_config.update(target=str(TARGET), target_baseline=baseline)
        guardian = IndependentGiftGuardian(guardian_config, args.control_root)
        guardian.start()
        guard.start()
        cases = [(name, "") for name in INTEGRATION if name not in {"gift_deletion_native", "gift_publicmedia_native"}]
        cases += [("api_gateway", "gift_admission_tests"), ("concert_service", "gift_live::product::tests"),
                  ("concert_service", "config::gift_runtime::tests")]
        if args.stage == "build":
            report["frozen_input_freshness"] = refresh_frozen_inputs(args.control_root, args.allocation_id, revision)
            check()
            compile_remaining = args.allocation_started_epoch + args.build_seconds - time.time()
            require(compile_remaining > 0, "original compile deadline already elapsed")
            guard.phase("gift-build", compile_remaining)
            cargo = str(TOOLCHAIN / "cargo")
            common = ["--offline", "--locked", "--message-format=json", "--config",
                      "profile.dev.package.contracts-build.opt-level=2", "--config",
                      "profile.test.package.contracts-build.opt-level=2"]
            commands = [[cargo, "test", "-p", "concert-service", "--lib", *sum((["--test", name] for name in INTEGRATION), []), "--no-run", *common],
                        [cargo, "test", "-p", "api-gateway", "--lib", "--no-run", *common],
                        [cargo, "build", "-p", "concert-service", "-p", "api-gateway", "--bin", "concert-service", "--bin", "api-gateway", *common]]
            artifacts = {}
            for index, argv in enumerate(commands):
                output = execute(argv, "cargo-" + str(index))
                with output.open() as stream:
                    messages = (json.loads(line) for line in stream)
                    for value in messages:
                        if value.get("reason") == "compiler-artifact" and value.get("executable"):
                            key = (value["target"]["name"], value["profile"]["test"])
                            artifacts[key] = Path(value["executable"])
            copy_remaining = args.allocation_work_deadline_epoch - time.time()
            require(copy_remaining > 0, "original copy/work deadline already elapsed")
            guard.phase("gift-copy", copy_remaining)
            keep = {("concert_service", True): "concert-library-tests",
                    ("gift_deletion_native", True): "gift-deletion-tests",
                    ("gift_publicmedia_native", True): "gift-media-tests",
                    ("concert-service", False): "concert-service",
                    ("api-gateway", False): "api-gateway"}
            for key, name in keep.items():
                path, sha = copy(artifacts[key], name)
                report["binaries"][name] = {"path": str(path), "sha256": sha, "source": revision}
                if not key[1]:
                    evidence = path.with_name(name + "-build-provenance.json")
                    write_private(evidence, {"service": name, "source_revision": revision, "binary_sha256": sha, "result": "PASS"})
                    report["binaries"][name].update(qualification="GIFT_PUBLIC_SOURCE_BUILD",
                        evidence={"path": str(evidence), "sha256": digest(evidence)})
            for name, select in cases:
                if name != "concert_service":
                    path, sha = copy(artifacts[(name, True)], "pure-" + name)
                    report["binaries"]["pure-" + name] = {"path": str(path), "sha256": sha, "source": revision}
            report["pure_cases"] = [["concert-library-tests" if name == "concert_service" else "pure-" + name, select]
                                    for name, select in cases]
        else:
            guard.phase("gift-focused-pure", args.pure_seconds)
            expected_cases = [["concert-library-tests" if name == "concert_service" else "pure-" + name, select]
                              for name, select in cases]
            require(prior["pure_cases"] == expected_cases and len(report["binaries"]) == 11,
                    "exact original eight pure selections and eleven copies required")
            for index, (name, select) in enumerate(expected_cases):
                record = report["binaries"][name]
                path = Path(record["path"])
                require(path.parent == args.report.parent and path.is_file() and not path.is_symlink()
                        and record["source"] == revision and digest(path) == record["sha256"],
                        "copied pure executable provenance drift")
                output = execute([str(path), *([select] if select else []), "--test-threads=1"], "pure-" + str(index))
                require(re.search(r"test result: ok\. [1-9][0-9]* passed", output.read_text()),
                        "actual passing pure cases required")
                if name.startswith("pure-"):
                    path.unlink()  # Exact new copy, with cumulative byte counter retained.
            report["pure"] = "PASS"
        require(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == revision
                and not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True), "frozen source drift")
        report["result"] = "PASS"
    except BaseException as error:
        report["result"] = "FAIL"
        report["failure_type"] = type(error).__name__
        report["failure"] = str(error)
        raise
    finally:
        guard.stop_event.set()
        if guardian is not None:
            try:
                guardian.begin_cleanup()
            except Exception as error:
                report["guardian_cleanup_error"] = str(error)
                report["result"] = "FAIL"
        if guard.thread is not None:
            guard.thread.join(timeout=20)
        try:
            report["bounds"] = guard.close()
        except Exception as error:
            report["bounds"] = {"failure": type(error).__name__ + ": " + str(error)}
            report["result"] = "FAIL"
        if process is not None:
            # Keep custody of the current group until it is empty, even if
            # Cargo exited before an owned compiler/test descendant did.
            try:
                errors = stop_owned_process_group(process, remaining_group_processes)
                if errors:
                    report["producer_cleanup_errors"] = errors
                    report["result"] = "FAIL"
            except Exception as error:
                report["cleanup_error"] = type(error).__name__ + ": " + str(error)
                report["result"] = "FAIL"
        try:
            remaining = remaining_group_processes()
        except Exception as error:
            remaining = ["UNKNOWN; process inspection failed"]
            report["process_inspection_error"] = type(error).__name__ + ": " + str(error)
            report["result"] = "FAIL"
        if guardian is not None:
            try:
                report["independent_guardian"] = guardian.stop()
                observed = report["independent_guardian"]
                if observed.get("result") != "PASS":
                    report["result"] = "FAIL"
                peak_growth = max(peak_growth, observed.get("max_target_growth_bytes", 0))
                peak_source_growth = max(peak_source_growth, observed.get("max_source_growth_bytes", 0))
                peak_outside = max(peak_outside, observed.get("max_outside_bytes", 0))
                rss_peak = max(rss_peak, observed.get("max_rss_bytes", 0))
                guard.process_peak = max(guard.process_peak, observed.get("max_owned_processes", 0))
            except Exception as error:
                report["independent_guardian"] = {"result": "FAIL", "reason": str(error)}
                report["result"] = "FAIL"
        def terminal_allocated(path, fallback):
            try:
                return allocated(path)
            except Exception as error:
                report.setdefault("terminal_accounting_errors", []).append(
                    {"path": str(path), "error": type(error).__name__ + ": " + str(error)})
                report["result"] = "FAIL"
                return fallback  # Explicitly unqualified, never a passing measurement.
        final_target = (terminal_allocated(TARGET, baseline + peak_growth) if args.stage == "build"
                        else prior["final_target_allocated_bytes"])
        final_output = terminal_allocated(args.report.parent, peak_outside)
        peak_growth = max(peak_growth, max(0, final_target - baseline))
        peak_source_growth = max(peak_source_growth,
            sum(max(0, terminal_allocated(root, source_baseline[str(root)])
                    - source_baseline[str(root)]) for root in source_roots))
        # Charge the bounded final report/release metadata before writing it;
        # partial failed copies and cleanup writes are included in final_output.
        peak_outside = max(peak_outside, max(copied_bytes, final_output) + peak_source_growth + 65536)
        report["bounds"]["max_sampled_whole_bytes"] = max(report["bounds"].get("max_sampled_whole_bytes", guard.carry),
            guard.carry + peak_growth + peak_outside)
        report["bounds"]["max_sampled_owned_processes"] = guard.process_peak
        if (peak_growth > args.target_growth_limit or peak_outside > args.output_byte_limit
                or peak_source_growth > 128 * 1024**2 or peak_growth + peak_outside > 3 * 1024**3
                or guard.carry + peak_growth + peak_outside > args.whole_byte_limit
                or rss_peak > 8 * 1024**3 or guard.process_peak > args.max_owned_processes):
            report["cleanup_error"] = "final actual accounting exceeded accepted bounds"
            report["result"] = "FAIL"
        report.update(max_target_growth_bytes=peak_growth, cumulative_copied_bytes=copied_bytes,
                      max_source_growth_bytes=peak_source_growth, max_outside_bytes=peak_outside,
                      max_sampled_rss_bytes=rss_peak, rss_limit_bytes=8 * 1024**3,
                      final_target_allocated_bytes=final_target,
                      final_output_allocated_bytes=final_output,
                      owned_group_processes=remaining, target_processes_stopped=not remaining,
                      target_handle_check=("NOT_RUN; owner must verify before releasing the target"
                          if args.stage == "build" else "PINNED_TARGET_RELEASE; no target access"))
        if remaining:
            report["result"] = "FAIL"
        if guard.failure:
            report["result"] = "FAIL"
        report["finished_unix"] = time.time()
        write_private(args.report, report)
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    return int(report["result"] != "PASS")


if __name__ == "__main__":
    raise SystemExit(main())
