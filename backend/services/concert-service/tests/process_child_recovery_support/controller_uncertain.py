#!/usr/bin/env python3
"""CR02/CR05 source-bound controller. Never builds, starts PostgreSQL, edits a repository, or accepts CR.
Only --execute with an actual complete clean prebuilt checkpoint starts a test executable.
No child output, environment secrets, handoff capabilities, or root views reach reports/stdout.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import signal
import stat
import subprocess
import sys
import time
import uuid
sys.dont_write_bytecode = True
from checkpoint import verify_checkpoint

HERE = Path(__file__).resolve().parent
TEST = "process_root_child_recovery_cr01"
READY = b"MCR_CR01_OLD_READY"
OBSERVED = b"MCR_CR01_RESTART_OBSERVED"
CASES = {
    "CR02": ("process_root_child_recovery_cr02", b"MCR_CR02_OLD_READY", b"MCR_CR02_RESTART_OBSERVED"),
    "CR05": ("process_root_child_recovery_cr05", b"MCR_CR05_OLD_READY", b"MCR_CR05_RESTART_OBSERVED"),
}

class CheckFailed(Exception):
    pass

def require(condition: bool, reason: str) -> None:
    if not condition:
        raise CheckFailed(reason)

def private_file(path: Path, uid: int) -> bytes:
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        meta = os.fstat(fd)
        require(stat.S_ISREG(meta.st_mode) and stat.S_IMODE(meta.st_mode) == 0o600
                and meta.st_uid == uid, "private owned file required")
        require(meta.st_size <= 1_000_000, "private file too large")
        with os.fdopen(os.dup(fd), "rb") as handle:
            return handle.read()
    finally:
        os.close(fd)

def owned_root() -> Path:
    root = Path(os.environ["MCR_CANONICAL_ROOT"])
    meta = root.lstat()
    require(root.is_absolute() and stat.S_ISDIR(meta.st_mode)
            and not stat.S_ISLNK(meta.st_mode) and stat.S_IMODE(meta.st_mode) == 0o700
            and meta.st_uid == os.getuid(), "private owned fixture root required")
    require(f"{meta.st_dev}:{meta.st_ino}" == os.environ["MCR_CANONICAL_ROOT_IDENTITY"],
            "fixture root identity mismatch")
    marker = json.loads(private_file(root / "mcr-owner.json", meta.st_uid))
    require(marker.get("uid") == meta.st_uid
            and marker.get("token") == os.environ["MCR_CANONICAL_ROOT_TOKEN"],
            "fixture root token mismatch")
    return root

def kill_reap(child: subprocess.Popen) -> int:
    # Operate on our exact Popen child only. Never inspect/kill ambient PIDs/groups.
    if child.poll() is None:
        child.kill()  # POSIX SIGKILL: actual abrupt Root-process loss.
    return child.wait(timeout=10)

class Child:
    def __init__(self, binary: Path, env: dict[str, str], log: Path, *, test=TEST, ready=READY, observed=OBSERVED):
        self.ready, self.observed = ready, observed
        self.log = os.fdopen(os.open(log, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb")
        try:
            self.process = subprocess.Popen(
                [str(binary), "--exact", test, "--ignored", "--nocapture", "--quiet", "--test-threads=1"],
                env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, close_fds=True)
        except BaseException:
            self.log.close()
            raise
        assert self.process.stdout is not None
        os.set_blocking(self.process.stdout.fileno(), False)
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.buffer = b""
        self.seen: set[bytes] = set()
        self.eof = False
        self.bytes_logged = 0

    def pump(self, duration: float) -> None:
        for key, _ in self.selector.select(duration):
            block = os.read(key.fileobj.fileno(), 65536)
            if not block:
                self.selector.unregister(key.fileobj)
                self.eof = True
                continue
            self.bytes_logged += len(block)
            require(self.bytes_logged <= 4_000_000, "child output bound exceeded")
            self.log.write(block)
            self.log.flush()
            self.buffer += block
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                if line.rstrip(b"\r") in (self.ready, self.observed):
                    self.seen.add(line.rstrip(b"\r"))
            require(len(self.buffer) <= 65536, "child line bound exceeded")

    def wait_line(self, line: bytes, timeout: float) -> None:
        stop = time.monotonic() + timeout
        while line not in self.seen:
            require(time.monotonic() < stop, "fixed child protocol line timed out")
            self.pump(min(0.2, max(0, stop - time.monotonic())))
            if self.process.poll() is not None and self.eof:
                require(line in self.seen, "child exited before protocol observation")

    def finish(self, timeout: float) -> int:
        stop = time.monotonic() + timeout
        while self.process.poll() is None or not self.eof:
            require(time.monotonic() < stop, "restart test did not exit")
            self.pump(0.1)
        return self.process.wait(timeout=1)

    def close(self) -> None:
        try:
            kill_reap(self.process)
        finally:
            self.selector.close()
            self.process.stdout.close()
            self.log.close()

def execute(args: argparse.Namespace) -> None:
    require(args.checkpoint is not None, "actual complete clean prebuilt checkpoint required")
    binary, binary_sha256, source_commit = verify_checkpoint(args.repo, args.checkpoint)
    checkpoint = json.loads(args.checkpoint.read_text())
    manifest = checkpoint.get("source_manifest", checkpoint.get("source_sha256", {}))
    require(manifest.get("backend/services/concert-service/tests/process_child_recovery_support/controller_uncertain.py")
            == hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), "actual uncertainty controller source mismatch")
    root = owned_root()
    test, ready, observed = CASES[args.case]
    prefix = "process-" + args.case.lower().replace("_", "-")
    run = uuid.uuid4()
    instance = uuid.uuid4()
    env = os.environ.copy()
    env.update(MCR_PROCESS_RUN=str(run), MCR_PROCESS_INSTANCE=str(instance), MCR_PROCESS_CASE=args.case)
    old = restarted = None
    old_pid = new_pid = None
    try:
        env["MCR_PROCESS_MODE"] = "old"
        old = Child(binary, env, root / f"{prefix}-{run.hex}-old.log", test=test, ready=ready, observed=observed)
        old_pid = old.process.pid
        old.wait_line(ready, 90)
        require(old.process.poll() is None, "old process must still await forever at READY")
        state_path = root / f"process-cr01-{run.hex}.json"
        state = json.loads(private_file(state_path, os.getuid()))
        require(state.get("old_pid") == old_pid and state.get("instance") == str(instance)
                and state.get("run") == str(run), "private READY identity mismatch")
        require(kill_reap(old.process) == -9, "old process was not SIGKILLed and reaped")
        # No process can restore an Arc or reuse the old registry across this boundary.
        fresh_binary, fresh_sha256, fresh_commit = verify_checkpoint(args.repo, args.checkpoint)
        require((fresh_binary, fresh_sha256, fresh_commit) == (binary, binary_sha256, source_commit),
                "build checkpoint changed between processes")
        env["MCR_PROCESS_MODE"] = "restart"
        restarted = Child(binary, env, root / f"{prefix}-{run.hex}-restart.log", test=test, ready=ready, observed=observed)
        new_pid = restarted.process.pid
        require(new_pid != old_pid, "distinct physical restart PID required")
        restarted.wait_line(observed, 100)
        require(restarted.finish(10) == 0, "restart integration test failed")
        # Only non-secret runtime-case observations after assertions and test exit zero.
        # This does not infer release or production main acceptance.
        report = {"case": args.case, "runtime_case_status": "PASS", "other_CR_cases": "NOT_RUN",
                  "old_pid": old_pid, "new_pid": new_pid, "old_reaped_after_sigkill": True,
                  "original_Profile_prepared": 1 if args.case == "CR02" else 0,
                  "original_prepared": 4 if args.case == "CR02" else 3,
                  "actual_read_reply_dropped": args.case == "CR02",
                  "actual_SDK_future_explicitly_dropped": args.case == "CR05",
                  "Profile_ReadCurrent_handler_calls": 1 if args.case == "CR02" else 0,
                  "exact_Profile_terminal_receipt": "SETTLED" if args.case == "CR02" else "CANCELLED",
                  "actual_Profile_Complete_proof": True, "immutable_nonce_digest_native_owner": True,
                  "native_prepared_zero_after_recovery": ["auth", "friend", "concert", "profile"],
                  "durable_owner_completed": ["auth", "friend"], "delivery_completed": False,
                  "immutable_intents_and_repeated_ACKs_unchanged": True,
                  "old_process_worker": {"attempted":2,"completed":0,"pending":2,"actual_profile_complete_unavailable":True},
                  "main_activation": "INTEGRATOR_OWNED_EXTERNAL_GATE",
                  "source_commit": source_commit, "binary_sha256": binary_sha256}
        report_path = root / f"{prefix}-{run.hex}-observations.json"
        with os.fdopen(os.open(report_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as handle:
            json.dump(report, handle, indent=2)
            handle.write("\n")
        print(f"{args.case} runtime_case_status=PASS; other_CR_cases=NOT_RUN; release/main acceptance is not inferred.")
    finally:
        try:
            if restarted is not None:
                restarted.close()
        finally:
            if old is not None:
                old.close()
        # No cleanup of PostgreSQL, prepared transactions, owner receipts or handoff.
        # Failed observations remain quarantined for receiving owner 207's owned teardown.

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--case", choices=tuple(CASES), default="CR02")
    parser.add_argument("--checkpoint", type=Path,
                        help="receiving owner's actual clean build source/binary checkpoint")
    parser.add_argument("--repo", type=Path,
                        default=Path(__file__).resolve().parents[5])
    args = parser.parse_args()
    if not args.execute:
        print("DRAFT ONLY: CR02/CR05 runtime acceptance NOT_RUN. No process started.")
        return 0
    def terminate(_signal, _frame):
        raise CheckFailed("owned controller terminated")
    signal.signal(signal.SIGTERM, terminate)
    try:
        execute(args)
        return 0
    except Exception:
        # Never relay arbitrary child output, capability-bearing exceptions or environment.
        print("Selected physical case failed; inspect the private owned logs. Acceptance unchanged.", file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
