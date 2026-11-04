"""One explicitly allocated QA08 workflow; no install or dependency copying.

Invoke only after accepting a fresh named resource offer. Preserve failed logs,
clean only this run's temporary tree/link, and report actual release receipts.
"""
import hashlib
from datetime import datetime
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
OWNED = ROOT / "frontend/e2e/qa08-rendering"
EVIDENCE = ROOT / "docs/mcr/evidence/frontend-performance/qa08"
DOCUMENT = ROOT / "docs/mcr/frontend/qa08-frontend-performance.md"
NODE = Path("/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node")
DEPS = Path("/Users/lhs/Desktop/mcr/frontend/node_modules")
LINK = ROOT / "frontend/node_modules"
MIB, GIB = 1024**2, 1024**3


def size(path):
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file() and not p.is_symlink())


def processes():
    rows = subprocess.check_output(["/bin/ps", "-axo", "pid=,ppid=,uid=,rss=,command="], text=True)
    return [line.strip().split(None, 4) for line in rows.splitlines() if line.strip()]


def owned_processes(parent, temporary):
    rows = processes()
    members = {parent} if parent else set()
    for _ in range(12):
        for pid, ppid, uid, _, command in rows:
            if int(uid) == os.getuid() and (int(ppid) in members or str(temporary) in command):
                members.add(int(pid))
    members.discard(os.getpid())
    return [(int(pid), int(rss) * 1024) for pid, _, _, rss, _ in rows if int(pid) in members]


def run():
    allocation = os.environ.get("QA08_ALLOCATION", "")
    name = os.environ.get("QA08_RUN", "")
    assert allocation and name and all(c in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in name)
    work_deadline = datetime.fromisoformat(os.environ["QA08_WORK_DEADLINE_UTC"].replace("Z", "+00:00")).timestamp()
    hard_deadline = datetime.fromisoformat(os.environ["QA08_HARD_DEADLINE_UTC"].replace("Z", "+00:00")).timestamp()
    assert time.time() < work_deadline < hard_deadline
    assert hard_deadline-work_deadline >= 60, "Keep offered absolute cleanup reserve"
    assert not os.path.lexists(LINK), "Do not replace any existing dependency path"
    assert DEPS.is_dir() and NODE.is_file()
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT), "Freeze source before resource acceptance"
    assert int(os.environ["QA08_ACCEPT_FREE_BYTES"]) >= 16 * GIB, "Original actual-acceptance free-space sample"
    assert shutil.disk_usage(ROOT).free >= 8 * GIB, "Continuous floor after acceptance"
    output = EVIDENCE / name
    assert not output.exists(), "Never overwrite a failed or historical run"
    output.mkdir()
    temporary = Path(tempfile.mkdtemp(prefix="mcr-qa08-current-", dir="/private/tmp"))
    os.chmod(temporary, 0o700)
    for child in ["tmp", "cache"]:
        (temporary / child).mkdir()
    environment = {**os.environ, "PATH": f"{NODE.parent}:{os.environ['PATH']}",
                   "QA08_CACHE_ROOT": str(temporary), "QA08_RUN": name + "-native",
                   "TMPDIR": str(temporary / "tmp"), "TMP": str(temporary / "tmp"),
                   "XDG_CACHE_HOME": str(temporary / "cache"), "NO_COLOR": "1"}
    for option in ["QA08_REUSE_BUILD", "QA08_SKIP_COLD", "QA08_WIDTH", "QA08_WINDOW_MS",
                   "VITE_MCR_EVENT_ID", "VITE_MCR_LOCAL_INPUT_CONFIG"]:
        environment.pop(option, None)
    receipt = {"allocation": allocation, "candidate": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
               "status": "IN_PROGRESS", "commands": [], "startedUtc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "caps": {"wholeBytes": 256*MIB, "temporaryBytes": 192*MIB, "durableBytes": 48*MIB,
                        "rssBytes": 8*GIB, "processes": 48, "workSeconds": 350, "cleanupReserveSeconds": 60,
                        "initialFreeBytes": 16*GIB, "continuousFreeBytes": 8*GIB},
               "peaks": {"wholeBytes": 0, "temporaryBytes": 0, "durableBytes": 0, "rssBytes": 0, "processes": 0},
               "freeBeforeBytes": shutil.disk_usage(ROOT).free,
               "offeredWorkDeadlineUtc": os.environ["QA08_WORK_DEADLINE_UTC"],
               "offeredHardDeadlineUtc": os.environ["QA08_HARD_DEADLINE_UTC"],
               "actualAcceptedAtUtc": os.environ["QA08_ACCEPTED_AT_UTC"],
               "actualAcceptanceFreeBytes": int(os.environ["QA08_ACCEPT_FREE_BYTES"]),
               "temporaryPath": str(temporary), "dependencyTarget": str(DEPS), "failure": None}
    started, process = time.monotonic(), None
    commands = [
        [str(NODE), "--check", str(OWNED / "native.mjs")],
        [str(NODE), "--test", str(OWNED / "metrics.node-selftest.mjs")],
        [str(NODE), str(DEPS / "typescript/bin/tsc"), "-p", str(OWNED / "tsconfig.json"), "--incremental", "--tsBuildInfoFile", str(temporary / "fixture.tsbuildinfo")],
        [str(DEPS / ".bin/oxlint"), "--deny-warnings", *[str(OWNED / p) for p in ["fixture.tsx", "current-data.ts", "native.mjs", "metrics.mjs", "metrics.node-selftest.mjs"]]],
        [str(NODE), str(OWNED / "native.mjs")],
    ]
    try:
        LINK.symlink_to(DEPS, target_is_directory=True)
        for index, argv in enumerate(commands):
            stage = {"argv": argv, "startedSeconds": time.monotonic()-started, "exitCode": None}
            receipt["commands"].append(stage)
            with (output / f"{index:02d}.log").open("wb") as log:
                process = subprocess.Popen(argv, cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                while process.poll() is None:
                    members = owned_processes(process.pid, temporary)
                    current = {"temporaryBytes": size(temporary), "durableBytes": size(EVIDENCE),
                               "rssBytes": sum(rss for _, rss in members), "processes": len(members)}
                    current["wholeBytes"] = current["temporaryBytes"] + current["durableBytes"] + size(OWNED) + DOCUMENT.stat().st_size
                    for key, value in current.items():
                        receipt["peaks"][key] = max(value, receipt["peaks"][key])
                        assert value <= receipt["caps"][key], f"Resource cap exceeded: {key}={value}"
                    assert shutil.disk_usage(ROOT).free >= 8*GIB, "Continuous free-space floor"
                    assert time.monotonic()-started <= 350, "Work deadline; cleanup reserve begins"
                    assert time.time() < work_deadline, "Original offered absolute work deadline reached"
                    time.sleep(.5)
                stage["exitCode"] = process.returncode
            stage["elapsedSeconds"] = time.monotonic()-started-stage["startedSeconds"]
            assert process.returncode == 0, f"Stage {index} exited {process.returncode}; raw log preserved"
        receipt["status"] = "PASS"
    except BaseException as error:
        receipt["status"] = "FAIL"
        receipt["failure"] = f"{type(error).__name__}: {error}"
    finally:
        cleanup_started = time.monotonic()
        for sig in [signal.SIGTERM, signal.SIGKILL]:
            for pid, _ in owned_processes(process.pid if process else None, temporary):
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
            if process:
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    pass
        if receipt["commands"] and receipt["commands"][-1]["exitCode"] is None and process:
            receipt["commands"][-1]["exitCode"] = process.poll()
        receipt["remainingProcesses"] = owned_processes(None, temporary)
        if LINK.is_symlink() and LINK.resolve() == DEPS.resolve():
            LINK.unlink()
        receipt["dependencyLinkAbsent"] = not os.path.lexists(LINK)
        shutil.rmtree(temporary)
        receipt["temporaryAbsent"] = not temporary.exists()
        receipt["cleanupSeconds"] = time.monotonic()-cleanup_started
        receipt["elapsedSeconds"] = time.monotonic()-started
        receipt["freeAfterBytes"] = shutil.disk_usage(ROOT).free
        receipt["finishedBeforeOfferedHardDeadline"] = time.time() < hard_deadline
        receipt["released"] = not receipt["remainingProcesses"] and receipt["dependencyLinkAbsent"] and receipt["temporaryAbsent"]
        receipt["logHashes"] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in output.glob("*.log")}
        (output / "release.json").write_text(json.dumps(receipt, indent=2)+"\n")
        print(json.dumps(receipt, indent=2))
    return 0 if receipt["status"] == "PASS" and receipt["released"] and receipt["finishedBeforeOfferedHardDeadline"] else 1


if __name__ == "__main__":
    sys.exit(run())
