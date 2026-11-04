"""Enforce an already ACCEPTED native lease; this file grants no allocation.

Only the marker-owned cluster and this supervisor's live child process group
are measured/stopped. No Cargo target, peer process or ambient database is used.
"""
from __future__ import annotations
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import time
from migration_smoke import SmokeError


class NativeBudget:
    def __init__(self, lease_id: str, deadline: float, minimum_free: int, maximum_owned: int):
        if (not re.fullmatch(r"[A-Za-z0-9_.:-]{1,160}", lease_id)
                or not math.isfinite(deadline) or deadline <= time.time()
                or minimum_free < 16 * 1024**3 or maximum_owned < 64 * 1024**2):
            raise SmokeError("resource", "fresh accepted native limits required")
        self.lease_id = lease_id
        self.deadline = deadline
        self.minimum_free = minimum_free
        self.maximum_owned = maximum_owned
        self.peak_owned = 0
        self.lowest_free = None
        self.commands = []

    def check(self, cluster=None):
        if time.time() >= self.deadline:
            raise SmokeError("resource", "accepted native deadline expired")
        volume = cluster.root if cluster is not None else Path(__file__).resolve()
        free = shutil.disk_usage(volume).free
        self.lowest_free = free if self.lowest_free is None else min(self.lowest_free, free)
        if free < self.minimum_free:
            raise SmokeError("resource", "accepted free-space floor breached")
        if cluster is not None:
            cluster.assert_owned()
            # Physical allocation, including open named test logs. Never follow
            # links, attach a peer root, or traverse any Cargo target.
            total = 0
            for directory, dirs, files in os.walk(cluster.root, followlinks=False):
                dirs[:] = [name for name in dirs if not (Path(directory) / name).is_symlink()]
                for name in files:
                    path = Path(directory) / name
                    if not path.is_symlink():
                        try:
                            total += path.stat().st_blocks * 512
                        except FileNotFoundError:
                            pass  # PG may remove its own transient file between observations.
            self.peak_owned = max(self.peak_owned, total)
            if total > self.maximum_owned:
                raise SmokeError("resource", "accepted owned-byte ceiling breached")

    def receipt(self):
        return dict(lease_id=self.lease_id, deadline_unix=self.deadline,
                    minimum_free_bytes=self.minimum_free, maximum_owned_bytes=self.maximum_owned,
                    peak_owned_bytes=self.peak_owned, lowest_free_bytes=self.lowest_free,
                    commands=self.commands)

    def execute(self, argv, *, cluster, env, phase, cwd, timeout=60):
        self.check(cluster)
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,120}", phase):
            raise SmokeError("resource", "invalid owned command phase")
        stdout_path = cluster.root / f"wall-{phase}.stdout"
        stderr_path = cluster.root / f"wall-{phase}.stderr"
        start = time.monotonic()
        process = None
        failure = None
        try:
            with stdout_path.open("xb") as out, stderr_path.open("xb") as err:
                process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=out, stderr=err,
                                           start_new_session=True)
                while process.poll() is None:
                    self.check(cluster)
                    if time.monotonic() - start >= timeout:
                        raise SmokeError(phase, "native test timeout")
                    time.sleep(0.25)
                self.check(cluster)
        except Exception as error:
            failure = error
        finally:
            if process is not None and process.poll() is None:
                # This PID is our un-reaped direct child and its own session
                # leader. It cannot identify another owner's process group.
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    process.wait(timeout=2)
                except ProcessLookupError:
                    process.wait(timeout=2)
        stdout = stdout_path.read_text(errors="replace") if stdout_path.exists() else ""
        stderr = stderr_path.read_text(errors="replace") if stderr_path.exists() else ""
        result = subprocess.CompletedProcess(argv, process.returncode if process else None, stdout, stderr)
        def private_diagnostic(text):
            text = re.sub(r"(?:postgres(?:ql)?|redis)://[^\s]+", "[redacted-url]", text)
            for key, value in env.items():
                if value and any(part in key for part in ("PASSWORD", "TOKEN", "SECRET")):
                    text = text.replace(value, "[redacted]")
            return text
        self.commands.append(dict(phase=phase, argv=argv, exit_code=result.returncode,
                                  duration_seconds=time.monotonic()-start,
                                  stdout=private_diagnostic(stdout), stderr=private_diagnostic(stderr),
                                  guardian_failure=getattr(failure, "detail", type(failure).__name__) if failure else None))
        if failure is not None:
            raise failure
        if result.returncode != 0:
            # Fixture diagnostic remains private; remove generated connection URLs.
            diagnostic = private_diagnostic(stderr or stdout)
            raise SmokeError(phase, diagnostic[-1200:], result.returncode)
        return result
