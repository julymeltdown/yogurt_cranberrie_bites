#!/usr/bin/env python3
"""Frozen actual-mural tests on owned socket-only PG16; never touches a target.

Cluster lifetime/ownership pattern adapted from event_cleanup/run.py at
0bd35b223d2590fb2b343171d6d8ac30d37ff857. This is component qualification;
its explicit authority fixtures do not activate production BE41.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import uuid
from freeze import sha

PG = Path("/opt/homebrew/opt/postgresql@16/bin")
FLOOR = 8 * 1024**3


def preflight(frozen):
    receipt = json.loads((frozen / "freeze.json").read_text())
    exe = frozen / "bin/event_cleanup_mural_owner"
    assert not exe.is_symlink() and exe.is_file()
    st = exe.stat()
    expected = receipt["binary"]
    assert (st.st_dev, st.st_ino, st.st_size) == (expected["device"], expected["inode"], expected["size"]), "frozen executable identity changed"
    assert sha(exe) == expected["sha256"], "frozen executable hash changed"
    actual = {p.name: sha(p) for p in (frozen / "migrations").glob("*.sql") if p.is_file() and not p.is_symlink()}
    assert actual == receipt["migrations"], "frozen migrations changed"
    assert sha(frozen / "cargo.jsonl") == receipt["cargo_json_sha256"]
    return receipt, exe


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--frozen", type=Path, required=True)
    parser.add_argument("--accepted-offer", required=True)
    a = parser.parse_args()
    frozen = a.frozen.resolve()
    receipt, exe = preflight(frozen)  # Reject before creating any PG resource.
    assert a.accepted_offer == receipt["accepted_offer"]
    assert shutil.disk_usage("/private/tmp").free >= FLOOR
    logs = frozen / "native"
    logs.mkdir(mode=0o700)
    root = Path(tempfile.mkdtemp(prefix="mcr-mural-owner-pg-", dir="/private/tmp")).resolve()
    owned = root.stat()
    token = uuid.uuid4().hex
    (root / "owner").write_text(token)
    socket, data = root / "socket", root / "data"
    socket.mkdir(mode=0o700)
    started = False
    start = time.monotonic()
    record = {"status": "IN_PROGRESS", "cleanup": False, "cluster": str(root), "source": receipt["source"], "binary": receipt["binary"], "checks": [], "peak_cluster_bytes": 0, "minimum_free_bytes": shutil.disk_usage(root).free}
    env = {k: v for k, v in os.environ.items() if not k.startswith(("PG", "DATABASE", "CONCERT_DATABASE", "MCR_"))}
    env.update(MCR_MURAL_OWNER_SOCKET=str(socket), MCR_MURAL_OWNER_TOKEN=token, MCR_MURAL_OWNER_MIGRATIONS=str(frozen / "migrations"), RUST_BACKTRACE="1")

    def sample():
        record["peak_cluster_bytes"] = max(record["peak_cluster_bytes"], sum(p.stat().st_size for p in root.rglob("*") if p.is_file()))
        free = shutil.disk_usage(root).free
        record["minimum_free_bytes"] = min(record["minimum_free_bytes"], free)
        assert free >= FLOOR, "disk floor violated"
        assert time.monotonic() - start < 240, "native time cap exceeded"

    def run(name, cmd, timeout=60):
        sample()
        proc = subprocess.run([str(v) for v in cmd], env=env, capture_output=True, text=True, timeout=timeout)
        (logs / f"{name}.stdout").write_text(proc.stdout)
        (logs / f"{name}.stderr").write_text(proc.stderr)
        record["checks"].append({"name": name, "exit_code": proc.returncode})
        if proc.returncode:
            raise RuntimeError(f"{name} exited {proc.returncode}; inspect retained logs")
        sample()
        return proc.stdout

    def native(name, filter_name, ignored=True):
        out = run(name, [exe, filter_name, *(["--ignored"] if ignored else []), "--nocapture", "--test-threads=1"], timeout=120)
        match = re.search(r"test result: ok\. (\d+) passed; 0 failed;", out)
        assert match and int(match.group(1)) > 0, "no test executed"
        record["checks"][-1]["passed"] = int(match.group(1))
        return out

    def sql(name, statement):
        return run(name, [PG / "psql", "-X", "-v", "ON_ERROR_STOP=1", "-At", "-h", socket, "-p", "55447", "-U", "mcr_mural_owner_fixture", "-d", "mcr_mural_owner_fixture", "-c", statement])

    try:
        out = native("source-pin", "mural_owner_source_pin", False)
        assert f"MURAL_OWNER_SOURCE_PIN={receipt['source']['owned_source_pin']}" in out
        assert "16." in run("pg-version", [PG / "postgres", "--version"])
        run("initdb", [PG / "initdb", "-D", data, "-A", "trust", "-U", "mcr_mural_owner_fixture", "--no-locale", "-E", "UTF8"])
        with (data / "postgresql.conf").open("a") as f:
            f.write(f"\nlisten_addresses = ''\nport = 55447\nunix_socket_directories = '{socket}'\nunix_socket_permissions = 0700\nfsync = on\nsynchronous_commit = on\nmax_connections = 16\n")
        run("pg-start", [PG / "pg_ctl", "-D", data, "-l", root / "postgres.log", "-w", "start"])
        started = True
        run("database", [PG / "createdb", "-h", socket, "-p", "55447", "-U", "mcr_mural_owner_fixture", "mcr_mural_owner_fixture"])
        sql("marker", f"create table mural_owner_fixture_marker(owner_token text not null);insert into mural_owner_fixture_marker values('{token}');")
        native("migrations", "mural_owner_prepare")
        native("mural-effects", "mural_owner_native_")
        native("restart-prepare", "mural_owner_restart_prepare")
        before = sql("before-restart", "select pg_postmaster_start_time();").strip()
        pid_before = int((data / "postmaster.pid").read_text().splitlines()[0])
        run("pg-stop-restart", [PG / "pg_ctl", "-D", data, "-m", "fast", "-w", "stop"])
        started = False
        run("pg-start-restart", [PG / "pg_ctl", "-D", data, "-l", root / "postgres.log", "-w", "start"])
        started = True
        after = sql("after-restart", "select pg_postmaster_start_time();").strip()
        pid_after = int((data / "postmaster.pid").read_text().splitlines()[0])
        assert before != after and pid_before != pid_after
        record["restart"] = {"before": before, "after": after, "pid_before": pid_before, "pid_after": pid_after}
        native("restart-verify", "mural_owner_restart_verify")
        assert sql("prepared", "select count(*) from pg_prepared_xacts;").strip() == "0"
        record["status"] = "PASS"
    except BaseException as error:
        record["status"], record["failure"] = "FAIL", str(error)
        raise
    finally:
        try:
            actual = root.stat()
            assert (actual.st_dev, actual.st_ino, actual.st_uid) == (owned.st_dev, owned.st_ino, os.getuid()) and (root / "owner").read_text() == token
            if started or (data / "postmaster.pid").exists():
                assert Path((data / "postmaster.pid").read_text().splitlines()[1]).resolve() == data
                # Cleanup remains unconditional even if a resource cap failed.
                proc = subprocess.run([str(PG / "pg_ctl"), "-D", str(data), "-m", "fast", "-w", "stop"], env=env, capture_output=True, text=True, timeout=60)
                (logs / "pg-final-stop.stdout").write_text(proc.stdout)
                (logs / "pg-final-stop.stderr").write_text(proc.stderr)
                assert proc.returncode == 0
            assert not (data / "postmaster.pid").exists()
            if (root / "postgres.log").exists():
                shutil.copy2(root / "postgres.log", logs / "postgres.log")
            shutil.rmtree(root)
            record["cleanup"] = not root.exists()
        finally:
            record["elapsed_seconds"] = round(time.monotonic() - start, 3)
            record["log_hashes"] = {p.name: sha(p) for p in sorted(logs.iterdir()) if p.is_file()}
            (frozen / "native.json").write_text(json.dumps(record, indent=2) + "\n")
            print(json.dumps({"status": record["status"], "cleanup": record["cleanup"], "receipt": str(frozen / "native.json")}), flush=True)


if __name__ == "__main__":
    main()
