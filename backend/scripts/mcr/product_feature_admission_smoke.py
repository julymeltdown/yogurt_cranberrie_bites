#!/usr/bin/env python3
"""Run one frozen admission artifact with fresh test PKI; never Cargo or PG.

Transport only: the recorder always refuses before canonical owner policy.
The caller must first release the shared target and retain its resource bounds.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import time
import uuid


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--output", type=Path, required=True, help="new absolute private directory")
    parser.add_argument("--time-cap-seconds", type=int, default=180)
    parser.add_argument("--free-floor-bytes", type=int, default=8 * 1024**3)
    parser.add_argument("--growth-cap-bytes", type=int, default=128 * 1024**2)
    args = parser.parse_args()
    binary, out = args.artifact, args.output
    assert binary.is_absolute() and binary.is_file() and not binary.is_symlink()
    assert "target" not in binary.parts, "only a copied artifact outside shared targets"
    assert sha256(binary) == args.sha256
    assert out.is_absolute() and not out.exists() and not out.is_symlink()
    assert 1 <= args.time_cap_seconds <= 300
    assert args.free_floor_bytes > 0 and args.growth_cap_bytes > 0
    out.mkdir(mode=0o700)
    identity = (out.stat().st_dev, out.stat().st_ino)
    pki = out / "pki"
    pki.mkdir(mode=0o700)
    pki_identity = (pki.stat().st_dev, pki.stat().st_ino)
    token = str(uuid.uuid4())
    marker = pki / "owner.json"
    marker.write_text(json.dumps({"uid": os.getuid(), "token": token}) + "\n")
    marker.chmod(0o600)
    started = time.monotonic()
    report = {"scope": "actual signed guarded Product RPC admission only; no canonical owner, PG or public HTTP",
              "artifact_sha256": args.sha256, "artifact": str(binary), "phases": [],
              "minimum_free_bytes": shutil.disk_usage(out).free, "peak_output_bytes": 0,
              "status": "IN_PROGRESS", "owned_pki_removed": False}
    process = None

    def check():
        assert (out.stat().st_dev, out.stat().st_ino) == identity
        free = shutil.disk_usage(out).free
        size = sum(p.stat().st_size for p in out.rglob("*") if p.is_file())
        report["minimum_free_bytes"] = min(report["minimum_free_bytes"], free)
        report["peak_output_bytes"] = max(report["peak_output_bytes"], size)
        assert free >= args.free_floor_bytes, "free-space floor"
        assert size <= args.growth_cap_bytes, "output growth cap"
        assert time.monotonic() - started < args.time_cap_seconds, "qualification time cap"

    def execute(name, command, env=None):
        nonlocal process
        check()
        phase = {"name": name, "command": command, "exit_code": None}
        report["phases"].append(phase)
        with (out / (name + ".stdout")).open("wb") as stdout, (out / (name + ".stderr")).open("wb") as stderr:
            process = subprocess.Popen(command, stdout=stdout, stderr=stderr,
                                       env=env, start_new_session=True)
            while process.poll() is None:
                check()
                time.sleep(0.1)
            phase["exit_code"] = process.returncode
        assert process.returncode == 0, name + " failed"
        process = None
        check()

    try:
        # Isolated CA and dual server/client workload leaves follow the platform
        # grpc_tls SAN contract. Keys are never returned in the evidence.
        ssl = "/usr/bin/openssl"
        execute("ca", [ssl, "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                       "-subj", "/CN=CF03-admission-test-only", "-keyout", str(pki / "ca.key"),
                       "-out", str(pki / "ca.crt")])
        for role in ("gateway", "concert", "friend"):
            config = pki / (role + ".ext")
            config.write_text("basicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\n"
                              "extendedKeyUsage=serverAuth,clientAuth\n"
                              f"subjectAltName=DNS:{role}.svc.internal,URI:spiffe://internal/{role}\n")
            execute(role + "-key", [ssl, "req", "-new", "-newkey", "rsa:2048", "-nodes",
                                    "-subj", f"/CN={role}.svc.internal", "-keyout", str(pki / (role + ".key")),
                                    "-out", str(pki / (role + ".csr"))])
            execute(role + "-cert", [ssl, "x509", "-req", "-in", str(pki / (role + ".csr")),
                                     "-CA", str(pki / "ca.crt"), "-CAkey", str(pki / "ca.key"),
                                     "-set_serial", str(100 + len(report["phases"])), "-days", "1",
                                     "-extfile", str(config), "-out", str(pki / (role + ".crt"))])
        for path in pki.iterdir():
            path.chmod(0o600)
        report["public_certificate_sha256"] = {p.name: sha256(p) for p in pki.glob("*.crt")}
        env = {"PATH": "/usr/bin:/bin", "RUST_BACKTRACE": "0", "MCR_FEATURE_PKI": str(pki),
               "MCR_FEATURE_PKI_IDENTITY": f"{pki_identity[0]}:{pki_identity[1]}", "MCR_FEATURE_PKI_TOKEN": token}
        execute("guarded", [str(binary), "--test-threads=1", "--nocapture"], env)
        execute("mtls", [str(binary), "signed_gateway_mtls_admits_all_ten_and_refuses_wrong_certificate_role",
                          "--exact", "--ignored", "--nocapture", "--test-threads=1"], env)
        assert sha256(binary) == args.sha256, "copied artifact drift"
        report["status"] = "PASS"
    except BaseException as error:
        report["status"], report["failure"] = "FAIL", str(error)
    finally:
        if process is not None and process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=2)
        assert (pki.stat().st_dev, pki.stat().st_ino) == pki_identity
        assert stat.S_ISDIR(pki.lstat().st_mode) and not pki.is_symlink()
        assert json.loads(marker.read_text())["token"] == token
        shutil.rmtree(pki)
        report["owned_pki_removed"] = not pki.exists()
        report["elapsed_seconds"] = time.monotonic() - started
        report["resource_bounds"] = {"time_cap_seconds": args.time_cap_seconds,
                                     "free_floor_bytes": args.free_floor_bytes,
                                     "growth_cap_bytes": args.growth_cap_bytes}
        (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report))
    return 0 if report["status"] == "PASS" and report["owned_pki_removed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
