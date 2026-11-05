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


def _metadata_require(ok, reason):
    if not ok: raise ValueError(reason)


def _bounded_metadata_read(path):
    import json
    from pathlib import Path
    path = Path(path)
    info = path.lstat()
    _metadata_require(not path.is_symlink() and info.st_size <= 262144, "unsafe receipt metadata")
    return json.loads(path.read_text())


def decode_receipt_files(files):
    """Pure receipt decoding; no synthetic workload or global census claims."""
    import re
    require, read = _metadata_require, _bounded_metadata_read
    records = {}
    observation_only = []
    for path in files:
        record = read(path)
        owner = record.get("ownerThreadId")
        if owner is None:
            observation = record.get("ordinaryObservation")
            require(isinstance(observation, dict) and set(record) == {"ordinaryObservation"}
                    and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", path.stem), "unrecognized observation-only receipt")
            require(isinstance(observation.get("booking"), dict) and isinstance(observation.get("role"), str)
                    and "ordinary" in observation["role"].lower(), "ordinary observation lacks factual role/booking")
            # This is NOT a synthetic FULL record and never releases a legacy
            # reservation. Filename identifies only this exact observation owner.
            observation_only.append({"observationOwner": path.stem, "ordinaryObservation": observation})
            continue
        require(path.stem == owner and owner not in records, "receipt owner ambiguity")
        require(record.get("state") in {"ACCEPTED", "FULL", "DECLINED"} and "kind" in record, "workload receipt missing canonical state/kind owner=" + str(owner) + " state=" + str(record.get("state")) + " kind=" + str(record.get("kind")))
        if record["state"] == "ACCEPTED" and record["kind"] != "ordinary":
            require(isinstance(record.get("offerId"), str) and bool(record["offerId"])
                    and isinstance(record.get("acceptedAtEpoch"), (int, float))
                    and (record.get("controller") is None or isinstance(record.get("controller"), dict))
                    and isinstance(record.get("guardControllers", []), list), "accepted workload missing actual canonical identity")
        records[owner] = record
    return records, observation_only


def resolve_legacy_receipts(records, membership, releases):
    """Exact current identity or positive literal release, never absence."""
    import hashlib
    from pathlib import Path
    require = _metadata_require
    closed = {"FULL", "DECLINED", "LITERAL_FULL_RECEIVED_RESOURCES_RELEASED"}
    for row in membership["entries"]:
        if row["state"] in closed: continue
        owner = row.get("ownerThreadId", row.get("owner"))
        actual = row.get("acceptedAtEpoch", row.get("actualAccept"))
        record = records.get(owner)
        release = releases.get(row.get("cohort", row.get("offerId", "")), {})
        if release.get("owner") == owner and release.get("actual_accept") == actual and release.get("all_owned_released") is True: continue
        require(record is not None, "accepted legacy owner lacks actual descriptor: " + str(owner))
        # A literal release can close only the same known lease. Unknown legacy
        # clock placeholders require the exact cohort identity as well.
        same_offer = bool(row.get("offerId")) and record.get("offerId") == row.get("offerId")
        same = same_offer and record.get("acceptedAtEpoch") == actual
        if not isinstance(actual, (int, float)):
            same = same_offer and bool(row.get("cohort")) and record.get("cohort") == row.get("cohort")
        if not same:
            for history in record.get("closedHistory", []) + record.get("closedOffers", []):
                history_epoch = history.get("acceptedAtEpoch", history.get("acceptedEpoch"))
                if (history.get("state") == "FULL" and history.get("offerId") == row.get("offerId")
                        and history_epoch == actual and history.get("ownerThreadId", owner) == owner
                        and history.get("resourcesReleased") is True):
                    proof_path = history.get("fullPath")
                    proof_sha = history.get("fullSha256", history.get("sha256"))
                    require(proof_path and proof_sha and hashlib.sha256(Path(proof_path).read_bytes()).hexdigest() == proof_sha,
                            "literal closed-history proof missing/changed")
                    same = True
                    break
        require(same, "legacy/current lease ambiguity: " + str(owner))


def validate_actual_process_descriptor(pid, descriptor, processes, failure_path):
    """Actual PS executable/birth/script; argv0 is not executable identity."""
    import hashlib, json, shlex, subprocess
    from pathlib import Path
    require = _metadata_require
    normalize = lambda value: " ".join(value.split())
    arguments = shlex.split(processes[pid][3])
    actual_executable = subprocess.check_output(["/bin/ps", "-ww", "-p", str(pid), "-o", "comm="], text=True, timeout=2).strip()
    actual_birth = subprocess.check_output(["/bin/ps", "-p", str(pid), "-o", "lstart="], text=True, timeout=2).strip()
    valid = normalize(actual_birth) == processes[pid][2] and actual_executable == descriptor["executable"]
    if not valid:
        evidence = {"sampled_at_epoch": time.time(), "pid": pid, "expected_descriptor": descriptor, "actual_executable": actual_executable, "actual_birth": actual_birth, "command_sha256": hashlib.sha256(processes[pid][3].encode()).hexdigest()}
        failure_path.write_text(json.dumps(evidence, indent=2))
    require(valid, "actual PS executable/birth mismatch pid=" + str(pid) + " expected=" + descriptor["executable"] + " actual=" + actual_executable)
    expected = descriptor["script"]
    if expected in arguments[1:]: return True
    # Relative argv is resolved against the actual process cwd, never a
    # guessed repository or basename. PID birth is checked before and after.
    candidates = [arg for arg in arguments[1:] if not arg.startswith("-") and arg.endswith(".py")]
    if not candidates: return False
    cwd_rows = subprocess.check_output(["/usr/sbin/lsof", "-a", "-p", str(pid), "-d", "cwd", "-Fn"], text=True, timeout=2)
    directories = [line[1:] for line in cwd_rows.splitlines() if line.startswith("n")]
    require(len(directories) == 1 and os.path.isabs(directories[0]), "actual process cwd ambiguous")
    birth = subprocess.check_output(["/bin/ps", "-p", str(pid), "-o", "lstart="], text=True, timeout=2)
    require(normalize(birth) == processes[pid][2], "process changed during cwd resolution")
    return any(os.path.normpath(os.path.join(directories[0], arg)) == expected for arg in candidates)


def sample_global_guard(config, entries=None, parent=None):
    """Independently merge receipt metadata and sample actual workflow trees.

    An unresolved accepted lease remains reserved; only literal releases close
    it. Peer processes are never signaled. Own workflow is counted once.
    """
    import json, subprocess, re, shlex
    from pathlib import Path
    def require(ok, why):
        if not ok: raise ValueError(why)
    def read(path):
        path = Path(path)
        info = path.lstat()
        require(not path.is_symlink() and info.st_size <= 262144, "unsafe receipt metadata")
        return json.loads(path.read_text())
    normalize = lambda value: " ".join(value.split())
    directory = Path(config["receipt_directory"])
    files = list(directory.glob("*.json"))
    require(len(files) <= 64, "receipt directory bound")
    records, observation_only = decode_receipt_files(files)
    membership = read(config["membership_path"])
    require(membership["issuer"] == "01a1a21d-39ec-7912-84cb-910058e7a110", "membership issuer")
    releases = read(config["peer_releases_path"])["releases"]
    import hashlib
    require(hashlib.sha256(Path(config["peer_releases_path"]).read_bytes()).hexdigest() == config["peer_releases_sha256"], "release pins")
    closed = {"FULL", "DECLINED", "LITERAL_FULL_RECEIVED_RESOURCES_RELEASED"}
    resolve_legacy_receipts(records, membership, releases)
    own = records.get(config["owner_thread_id"])
    require(own is not None and own["state"] == "ACCEPTED" and own["acceptedAtEpoch"] == config["actual_accept_epoch"], "own actual receipt absent")
    active = [r for r in records.values() if r["state"] == "ACCEPTED" and r["kind"] != "ordinary"]
    require(all(r["state"] in closed | {"ACCEPTED"} for r in records.values()), "unknown receipt state")
    kinds = {k: sum(r["kind"] == k for r in active) for k in ("source", "browser", "target", "copied-native")}
    require(len(active) <= 4 and kinds["source"] + kinds["copied-native"] <= 3 and kinds["browser"] <= 2 and kinds["target"] <= 1, "workflow cap")
    require(all(r["kind"] in kinds and type(r["processCap"]) is int and r["processCap"] > 0 and type(r["rssCapBytes"]) is int and r["rssCapBytes"] > 0 for r in active), "invalid actual cap")
    require(sum(r["processCap"] for r in active) <= 96 and sum(r["rssCapBytes"] for r in active) <= 18*GIB, "declared aggregate cap")
    # Fresh process sample in EACH monitor, independent from management metadata.
    rows = subprocess.check_output(["/bin/ps", "-ww", "-axo", "pid=,ppid=,rss=,lstart=,command="], text=True, timeout=2)
    processes = {}
    for line in rows.splitlines():
        fields = line.split(maxsplit=8)
        if len(fields) == 9:
            processes[int(fields[0])] = (int(fields[1]), int(fields[2])*1024, normalize(" ".join(fields[3:8])), fields[8])
    def script_matches(pid, descriptor):
        name = "primary" if os.getpid() == config["controller_pid"] else "independent"
        return validate_actual_process_descriptor(pid, descriptor, processes,
                Path(config["output"]) / (name + "-identity-failure.json"))
    union, rss_total, cohorts = set(), 0, []
    retained = config.setdefault("observed_global_identities", {})
    for record in active:
        descriptor = record.get("controller")
        if isinstance(descriptor, dict): descriptor = dict(descriptor, ownerThreadId=record["ownerThreadId"], offerId=record["offerId"])
        if descriptor is None:
            require(not record.get("guardControllers"), "null controller with unresolved actual guardian")
            cohorts.append({"owner": record["ownerThreadId"], "offer": record["offerId"], "actual_pids": [],
                            "rss": 0, "reservation_retained_without_controller": True})
            continue
        require(isinstance(descriptor, dict), "accepted workflow invalid controller")
        pid = descriptor["pid"]
        require(pid in processes, "accepted root absent without literal FULL")
        info = processes[pid]
        require(info[2] == normalize(descriptor["birth"]) and script_matches(pid, descriptor), "actual controller identity mismatch")
        owned = {pid}
        while True:
            more = {p for p,v in processes.items() if v[0] in owned} - owned
            if not more: break
            owned.update(more)
        key = record["ownerThreadId"] + ":" + str(record["acceptedAtEpoch"])
        old = retained.setdefault(key, {})
        owned.update(int(p) for p,birth in old.items() if int(p) in processes and processes[int(p)][2] == birth)
        old.update({str(p): processes[p][2] for p in owned})
        require(not (union & owned), "overlapping workflow ancestry")
        rss = sum(processes[p][1] for p in owned)
        require(len(owned) <= record["processCap"] and rss <= record["rssCapBytes"], "actual workflow cap")
        for guard in record.get("guardControllers", []):
            gp = guard["pid"]
            require(gp in owned and processes[gp][2] == normalize(guard["birth"]) and script_matches(gp, guard), "guardian identity ambiguity")
        union.update(owned); rss_total += rss
        cohorts.append({"owner": record["ownerThreadId"], "offer": record["offerId"], "actual_pids": sorted(owned), "rss": rss})
    ordinary_pids = set()
    for record in list(records.values()) + observation_only:
        controls = []
        if record.get("kind") == "ordinary" and isinstance(record.get("controller"), dict):
            controls.append(record["controller"])
        observation = record.get("ordinaryObservation")
        if observation:
            controls.append({"pid": observation.get("actualpid", observation.get("pid")), "birth": observation.get("birth"), "script": None})
        for descriptor in controls:
            pid = descriptor.get("pid")
            if pid not in processes or not descriptor.get("birth") or processes[pid][2] != normalize(descriptor["birth"]):
                continue  # Never apply a stale observation to a reused PID.
            if pid in union: continue  # Current workload identity wins after exec.
            if descriptor.get("script"):
                require(script_matches(pid, descriptor), "ordinary actual script mismatch")
            tree = {pid}
            while True:
                more = {p for p,v in processes.items() if v[0] in tree} - tree
                if not more: break
                tree.update(more)
            for child in tree:
                command = processes[child][3]
                heavy = bool(re.search(r"\b(?:node|cargo|rustc)(?:\s|$)", command)) or ("Google Chrome --" in command and "--headless" in command)
                script_child = child != pid and bool(re.search(r"\b(?:Python|python3(?:\.\d+)?)\s+(?:-B\s+)?/\S+\.py", command))
                require(not heavy and not script_child, "ordinary role cannot conceal runtime payload")
            ordinary_pids.update(tree)
    union.update(ordinary_pids)
    rss_total = sum(processes[p][1] for p in union)
    require(len(union) <= 96 and rss_total <= 18*GIB, "actual aggregate incl ordinary PID/RSS cap")
    # Scoped recognizer adapted from the executed PI01 producer. This local
    # adapter additionally requires explicit identity for opaque scoped shells.
    for pid, info in processes.items():
        command = info[3]
        scoped = any(part in command for part in ("/mcr-worktrees/", "/mcr-build/", "/private/tmp/mcr-", "/tmp/mcr-", "/T/mcr-", "/mcr-evidence/"))
        browser = "Google Chrome --" in command and "--headless" in command
        runtime = bool(re.search(r"\b(?:node|cargo|rustc)(?:\s|$)", command)) or bool(re.search(r"\b(?:Python|python3(?:\.\d+)?)\s+(?:-B\s+)?/\S+\.py", command)) or browser
        opaque_shell = scoped and command.startswith(("/bin/zsh", "/bin/bash", "/bin/sh"))
        if ((scoped or browser) and runtime) or opaque_shell:
            require(pid in union, "unknown actual scoped runtime/shell pid=" + str(pid))
    result = {"workflow_count": len(active), "source_count": kinds["source"] + kinds["copied-native"], "browser_count": kinds["browser"], "target_count": kinds["target"], "distinct_workflow_pids": len(union), "actual_rss_bytes": rss_total, "ordinary_actual_pids": sorted(ordinary_pids), "cohorts": cohorts, "unknown_peer_policy": "FAIL_CLOSED_SELF_CANCEL_ONLY"}
    result["sampled_at_epoch"] = time.time()
    name = "primary" if os.getpid() == config["controller_pid"] else "independent"
    destination = Path(config["output"]) / (name + "-global-census.json")
    temporary = destination.with_suffix(".tmp-" + str(os.getpid()))
    with temporary.open("x") as stream: json.dump(result, stream)
    os.replace(temporary, destination)
    return result


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
        rss_cap = c.get("rss_cap")
        require(type(rss_cap) is int and 0 < rss_cap <= 8 * GIB,
                "explicit independent RSS cap required")
        result["rss_cap_bytes"] = rss_cap
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
            require(len(owned) <= c["process_cap"] and rss <= rss_cap,
                    "independent owned process/RSS cap")
            global_sample = sample_global_guard(c, entries, parent)
            result["global_sample"] = global_sample
            result["max_global_distinct_pids"] = max(result.get("max_global_distinct_pids", 0), global_sample["distinct_workflow_pids"])
            result["max_global_rss_bytes"] = max(result.get("max_global_rss_bytes", 0), global_sample["actual_rss_bytes"])
            if c["stage"] == "build":
                peak_target = max(peak_target, max(0, allocated(c["target"]) - c["target_baseline"]))
            growth = sum(max(0, allocated(path) - initial)
                         for path, initial in c["source_baseline"].items())
            peak_source = max(peak_source, growth)
            # The separate producer also preserves cumulative copied bytes.
            # Actual physical bytes and original inherited peak are independently
            # measured here; deleting copies never lowers this guardian's peak.
            peak_outside = max(peak_outside, c.get("preparation_upper_bytes", 0) + allocated(c.get("allocation_output_root", output)) + peak_source)
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
        if True:
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
