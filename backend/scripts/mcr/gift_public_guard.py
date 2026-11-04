"""Gift-local adapter for explicit accepted allocation bounds; grants no lease.

Reuses the published world_in01_live QualificationGuard output identity, byte,
process, deadline and cleanup accounting without changing that owning helper.
Only the start/continuous floors become required explicit offer arguments.
"""
from __future__ import annotations
import os
import shutil
import signal
import stat
import threading
import time

GIB = 1024**3


def allocated_bytes(path):
    """Count allocated inodes without following links in a live build tree.

    Cargo may remove an enumerated descendant before its stat. Only that
    specific ENOENT race is tolerated; inaccessible/replaced roots and every
    other I/O error fail the guard. Callers retain nonshrinking sampled peaks.
    """
    root = os.fspath(path)
    initial = os.lstat(root)
    if not stat.S_ISDIR(initial.st_mode):
        raise ValueError("allocated-byte root must be a real directory")
    identity = initial.st_dev, initial.st_ino
    seen = {identity}
    total = initial.st_blocks * 512
    deadline = time.monotonic() + 5
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW

    def walk(fd):
        nonlocal total
        with os.scandir(fd) as entries:
            for entry in entries:
                if time.monotonic() >= deadline:
                    raise TimeoutError("allocated-byte sample exceeded five seconds")
                child_fd = None
                try:
                    info = os.stat(entry.name, dir_fd=fd, follow_symlinks=False)
                    if stat.S_ISDIR(info.st_mode):
                        # Pin the actual directory; never traverse a replacement
                        # symlink or resolve a descendant through its pathname.
                        child_fd = os.open(entry.name, flags, dir_fd=fd)
                        info = os.fstat(child_fd)
                except FileNotFoundError:
                    continue
                try:
                    inode = info.st_dev, info.st_ino
                    if inode in seen:
                        continue
                    seen.add(inode)
                    total += info.st_blocks * 512
                    if child_fd is not None:
                        walk(child_fd)
                finally:
                    if child_fd is not None:
                        os.close(child_fd)

    fd = os.open(root, flags)
    try:
        pinned = os.fstat(fd)
        if (pinned.st_dev, pinned.st_ino) != identity:
            raise ValueError("allocated-byte root was replaced")
        walk(fd)
    finally:
        os.close(fd)
    final = os.lstat(root)
    if not stat.S_ISDIR(final.st_mode) or (final.st_dev, final.st_ino) != identity:
        raise ValueError("allocated-byte root was replaced")
    return total


def allocation_arguments(parser):
    for name in ("allocation-start-free-bytes", "allocation-stop-free-bytes"):
        parser.add_argument("--" + name, type=int, required=True)


def qualification_guard(base, args, stack):
    start, stop = args.allocation_start_free_bytes, args.allocation_stop_free_bytes
    if not (start >= 12 * GIB and stop >= 8 * GIB and start >= stop
            and args.whole_byte_limit > args.retained_carry_bytes >= 0):
        raise ValueError("explicit accepted Gift allocation floors required")

    class GiftQualificationGuard(base):
        def check(self):
            super().check()
            if shutil.disk_usage(self.output).free < stop:
                raise ValueError("Gift accepted continuous free-space floor breached")

        def start(self):
            free = shutil.disk_usage(self.output).free
            continuation = getattr(args, "allocation_continuation", False)
            reserved = getattr(args, "allocation_reserved_new_bytes", self.limit - self.carry)
            other = getattr(args, "allocation_other_growth_bytes", 0)
            if not continuation and (free < start or free - other - reserved < 9 * GIB):
                raise ValueError("Gift accepted start or worst-case residual floor breached")
            self.phase("native-startup", args.startup_timeout_seconds)
            self.check()

            def watch():
                while not self.stop_event.wait(.5):
                    try:
                        self.check()
                    except BaseException as error:
                        if self.stop_event.is_set():
                            return
                        self.failure = type(error).__name__ + ": " + str(error)
                        # Same direct-producer cancellation as the existing
                        # helper. Its handler/finally owns child cleanup.
                        os.kill(os.getpid(), signal.SIGTERM)
                        return
            self.thread = threading.Thread(target=watch, daemon=True)
            self.thread.start()

        def close(self):
            receipt = super().close()
            receipt.update(start_free_floor_bytes=start,
                           continuous_stop_floor_bytes=stop,
                           worst_case_residual_floor_bytes=9 * GIB)
            return receipt

    return GiftQualificationGuard(args, stack)


class IndependentGiftGuardian:
    """Separate supervisor process; no workload starts before its ready receipt.

    All paths are exact private allocation paths. This is a second enforcement
    control, not a reservation broker or evidence of an allocator's ACCEPT.
    """
    def __init__(self, config, root):
        import hashlib
        import json
        import subprocess
        import sys
        import time
        self.path = root / (config["allocation_id"] + "-" + config["stage"] + "-guardian.json")
        self.stop_path = self.path.with_suffix(".stop")
        self.ready_path = self.path.with_suffix(".ready")
        self.result_path = self.path.with_suffix(".result")
        self.cleanup_path = self.path.with_suffix(".cleanup")
        for path in (self.path, self.stop_path, self.ready_path, self.result_path, self.cleanup_path):
            if path.exists() or path.is_symlink():
                raise ValueError("new exact independent guardian paths required")
        config.update(controller_pid=os.getpid(), stop_path=str(self.stop_path),
                      ready_path=str(self.ready_path), result_path=str(self.result_path),
                      cleanup_path=str(self.cleanup_path))
        with os.fdopen(os.open(self.path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as stream:
            json.dump(config, stream)
        sha = hashlib.sha256(self.path.read_bytes()).hexdigest()
        self.sha = sha
        self.process = None

    def start(self):
        import json
        import subprocess
        import sys
        import time
        sha = self.sha
        self.process = subprocess.Popen([sys.executable, "-B", __file__, "--watch", str(self.path), sha],
                                        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                        stderr=subprocess.DEVNULL)
        end = time.monotonic() + 10
        while time.monotonic() < end:
            if self.process.poll() is not None:
                raise ValueError("independent Gift guardian refused startup")
            if self.ready_path.exists():
                ready = json.loads(self.ready_path.read_text())
                if ready != {"config_sha256": sha, "guardian_pid": self.process.pid}:
                    raise ValueError("independent Gift guardian ready mismatch")
                return
            time.sleep(.05)
        self.stop()
        raise ValueError("independent Gift guardian startup deadline")

    def begin_cleanup(self):
        self.cleanup_path.touch(mode=0o600, exist_ok=False)

    def stop(self):
        import json
        import subprocess
        if self.process is None:
            return {"result": "NOT_STARTED"}
        self.stop_path.touch(mode=0o600, exist_ok=False)
        try:
            code = self.process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=3)
            raise ValueError("independent Gift guardian shutdown failed")
        if not self.result_path.is_file():
            raise ValueError("independent Gift guardian failed")
        result = json.loads(self.result_path.read_text())
        if code != 0:
            result["result"] = "FAIL"
            result["guardian_exit_code"] = code
        return result


def _watch(config_path, expected_sha):
    """Private second process: actual bytes, live floors, process/RSS, deadlines.

    Build-only target measurement is absent from the pure configuration. A
    parent PID is accepted only while this guardian is its actual direct child.
    """
    import hashlib
    import json
    from pathlib import Path
    import subprocess
    import time
    def require(condition, reason):
        if not condition:
            raise ValueError(reason)
    def write(path, value):
        with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as stream:
            json.dump(value, stream)
    allocated = allocated_bytes
    config = Path(config_path)
    info = config.lstat()
    require(not config.is_symlink() and info.st_uid == os.getuid() and info.st_mode & 0o077 == 0,
            "private guardian configuration required")
    require(hashlib.sha256(config.read_bytes()).hexdigest() == expected_sha, "guardian pin mismatch")
    c = json.loads(config.read_text())
    parent = c["controller_pid"]
    require(os.getppid() == parent, "actual direct Gift controller required")
    output = Path(c["output"])
    output_info = output.lstat()
    require(not output.is_symlink() and output_info.st_uid == os.getuid()
            and output_info.st_mode & 0o077 == 0, "private guardian output required")
    output_identity = output_info.st_dev, output_info.st_ino
    stop = Path(c["stop_path"])
    result = {"result": "FAIL", "config_sha256": expected_sha, "max_owned_processes": 0,
              "max_rss_bytes": 0, "max_whole_bytes": c["carry"], "minimum_free_bytes": None,
              "target_measured": c["stage"] == "build"}
    peak_target, peak_source, peak_outside = c["target_peak"], c["source_peak"], c["outside_peak"]
    owned_identities = {}
    failure = None
    try:
        free = shutil.disk_usage(output).free
        if c["stage"] == "build" or c.get("admission_required", False):
            require(free >= c["start_free"], "independent start free floor")
            require(free - c["other_growth"] - c.get("reserved_new_bytes", 3 * GIB)
                    >= c.get("after_all_floor", 9 * GIB),
                    "independent after-ALL reservation floor")
        while not stop.exists():
            require(os.getppid() == parent, "Gift controller exited unexpectedly")
            deadline = c["full_deadline"] if Path(c["cleanup_path"]).exists() else c["work_deadline"]
            require(time.time() < deadline, "independent original phase deadline")
            info = output.lstat()
            require(not output.is_symlink() and (info.st_dev, info.st_ino) == output_identity,
                    "independent output replaced")
            free = shutil.disk_usage(output).free
            result["minimum_free_bytes"] = free if result["minimum_free_bytes"] is None else min(free, result["minimum_free_bytes"])
            require(free >= c["stop_free"], "independent live free floor")
            rows = subprocess.check_output(["/bin/ps", "-axo", "pid=,ppid=,rss=,lstart="], text=True, timeout=2)
            entries = {int(p[0]): (int(p[1]), int(p[2]) * 1024, p[3])
                       for row in rows.splitlines() if len(p := row.split(maxsplit=3)) == 4}
            owned = {parent}
            while True:
                more = {pid for pid, (ppid, _, _) in entries.items() if ppid in owned} - owned
                if not more:
                    break
                owned.update(more)
            # Preserve actual previously observed orphan descendants by PID AND
            # process start identity; a reused PID is never inherited custody.
            owned.update(pid for pid, started in owned_identities.items()
                         if pid in entries and entries[pid][2] == started)
            owned_identities.update({pid: entries[pid][2] for pid in owned if pid in entries})
            rss = sum(entries[pid][1] for pid in owned if pid in entries)
            result["max_owned_processes"] = max(result["max_owned_processes"], len(owned))
            result["max_rss_bytes"] = max(result["max_rss_bytes"], rss)
            require(len(owned) <= c["process_cap"] and rss <= 8 * GIB,
                    "independent owned process/RSS cap")
            if c["stage"] == "build":
                peak_target = max(peak_target, max(0, allocated(c["target"]) - c["target_baseline"]))
            growth = sum(max(0, allocated(path) - initial)
                         for path, initial in c["source_baseline"].items())
            peak_source = max(peak_source, growth)
            # The separate producer also preserves cumulative copied bytes.
            # Actual physical bytes and original inherited peak are independently
            # measured here; deleting copies never lowers this guardian's peak.
            peak_outside = max(peak_outside, allocated(output) + peak_source)
            require(peak_source <= 128 * 1024**2 and peak_target <= c["target_cap"]
                    and peak_outside <= c["outside_cap"]
                    and peak_target + peak_outside <= 3 * GIB,
                    "independent inclusive new byte cap")
            whole = c["carry"] + peak_target + peak_outside
            result["max_whole_bytes"] = max(result["max_whole_bytes"], whole)
            require(whole <= c["whole_cap"], "independent nonshrinking whole cap")
            if not Path(c["ready_path"]).exists():
                write(c["ready_path"], {"config_sha256": expected_sha, "guardian_pid": os.getpid()})
            time.sleep(.5)
        result["result"] = "PASS"
    except BaseException as error:
        if stop.exists():
            result["result"] = "PASS"
        else:
            failure = type(error).__name__ + ": " + str(error)
            result["failure"] = failure
            if os.getppid() == parent:
                os.kill(parent, signal.SIGTERM)
            # Parent owns normal group cleanup. If it disappeared, stop ONLY
            # observed exact descendant identities, never peers or guessed PIDs.
            until = time.monotonic() + 5
            while os.getppid() == parent and time.monotonic() < until and not stop.exists():
                time.sleep(.1)
            if os.getppid() != parent:
                rows = subprocess.check_output(["/bin/ps", "-axo", "pid=,lstart="], text=True, timeout=2)
                current = {int(p[0]): p[1] for row in rows.splitlines()
                           if len(p := row.split(maxsplit=1)) == 2}
                for pid, started in owned_identities.items():
                    if pid not in {parent, os.getpid()} and current.get(pid) == started:
                        try:
                            os.kill(pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
    finally:
        result.update(max_target_growth_bytes=peak_target, max_source_growth_bytes=peak_source,
                      max_outside_bytes=peak_outside, finished_unix=time.time())
        write(c["result_path"], result)
    return int(failure is not None)


if __name__ == "__main__":
    import sys
    sys.dont_write_bytecode = True
    if len(sys.argv) != 4 or sys.argv[1] != "--watch":
        raise SystemExit("private --watch CONFIG SHA256 required")
    raise SystemExit(_watch(sys.argv[2], sys.argv[3]))
