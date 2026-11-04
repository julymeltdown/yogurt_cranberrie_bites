#!/usr/bin/env python3
"""Run a frozen real five-owner Feed component join; Gateway HTTP remains NOT_RUN.

Adapted from canonical ea7ccaf4 qualify.py, SHA-256 ba47f1fbc79d633a5c69009e986c5035e10728ec25e903a23ee4079ab96e234a.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
from migration_smoke import (Cluster, PG_TOOLS, SmokeError, check_ledger,
                             executable, execute, migration_manifest)
from auth_invalidation_smoke import frozen_binary
from invalidation_clients_smoke import pki as canonical_pki

OWNERS = ("auth", "friend", "concert", "profile", "post")
CASES = (
    "feed::native_join::actual_post_reads_use_current_public_author_and_same_transaction_owner_tracking",
    "feed::native_join::read_cases::actual_detail_reads_post_and_comments_under_held_guards",
    "feed::native_join::read_cases::actual_comments_read_current_parent_and_public_author",
    "feed::native_join::read_cases::actual_private_author_is_filtered_after_real_post_scan",
    "feed::native_join::read_cases::actual_post_outage_cannot_become_an_empty_page",
    "feed::native_join::read_cases::actual_sealed_root_cannot_authorize_a_later_read",
)

def pki(cluster):
    canonical_pki(cluster)
    directory = cluster.root / "pki"
    openssl = executable(Path("/usr/bin/openssl"))
    def run(*args):
        execute([str(openssl), *map(str, args)], env=cluster.env, cwd=directory, phase="feed-pki", timeout=30)
    for role in ("profile", "post"):
        extension = directory / f"{role}.ext"
        extension.write_text(f"basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:{role}.svc.internal,URI:spiffe://internal/{role}\n")
        run("req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", f"{role}.key", "-out", f"{role}.csr", "-subj", f"/CN={role}.svc.internal")
        run("x509", "-req", "-in", f"{role}.csr", "-CA", "ca.crt", "-CAkey", "ca.key", "-CAcreateserial", "-out", f"{role}.crt", "-days", "2", "-extfile", extension)
        (directory / f"{role}.key").chmod(0o600)

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def resource_inputs(public: Path, binding: Path) -> set[Path]:
    document = json.loads(binding.read_text())
    pins = [document["geometry_source"], document["tilemap_source"]]
    for scene in document["scenes"]:
        pins.extend([scene["scenery_source"], *scene["scene_sources"], *scene["runtime_resources"]])
    if len(pins) > 1024:
        raise SmokeError("preflight", "bounded source binding required")
    paths = set()
    for pin in pins:
        relative = Path(pin["path"])
        path = public / relative
        if relative.is_absolute() or ".." in relative.parts or not path.resolve().is_relative_to(public.resolve()):
            raise SmokeError("preflight", "source binding escapes resource root")
        data = path.read_bytes()
        if len(data) != pin["bytes"] or "sha256:" + hashlib.sha256(data).hexdigest() != pin["sha256"]:
            raise SmokeError("preflight", "source-bound resource changed")
        paths.add(path)
    return paths

def source_manifest() -> dict[str, str]:
    paths = {ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock", Path(__file__).resolve(),
             ROOT / "frontend/src/entities/avatar/character-source-pack.json"}
    for prefix in ("backend/crates", "backend/proto", "backend/services/auth-service",
                   "backend/services/friend-service", "backend/services/concert-service",
                   "backend/services/profile-service", "backend/services/post-service", "contracts",
                   "backend/releases/operator-bundle/quality-v6", "backend/releases/reference-scene-v2/quality-v6",
                   "frontend/public"):
        paths.update(p for p in (ROOT / prefix).rglob("*")
                     if p.is_file() and p.suffix in {".rs", ".sql", ".toml", ".proto", ".py", ".json", ".png", ".jpg", ".webp", ".svg"})
    paths.update(resource_inputs(ROOT / "frontend/public", ROOT / "backend/releases/reference-scene-v2/quality-v6/source-binding.json"))
    for config in (".cargo/config.toml", "backend/.cargo/config.toml", "rust-toolchain.toml", "backend/rust-toolchain.toml"):
        if (ROOT / config).is_file():
            paths.add(ROOT / config)
    for name in ("migration_smoke.py", "auth_invalidation_smoke.py", "invalidation_clients_smoke.py"):
        paths.add(ROOT / "backend/scripts/mcr" / name)
    return {str(p.relative_to(ROOT)): sha(p) for p in sorted(paths)}

def qualify(checkpoint: dict, pg: Path, report: dict) -> None:
    commit = checkpoint["source_commit"]
    if (checkpoint.get("scope") != "feed-native-owner-join" or checkpoint.get("source_state") != "clean"
            or len(commit) != 40 or any(c not in "0123456789abcdef" for c in commit)):
        raise SmokeError("preflight", "exact clean Feed native source checkpoint required")
    if checkpoint["source_sha256"] != source_manifest():
        raise SmokeError("preflight", "source checkpoint changed")
    binary = frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])
    if shutil.disk_usage(ROOT).free < 8 * 1024**3:
        raise SmokeError("preflight", "8 GiB free required")
    tools = {name: executable(pg / name) for name in PG_TOOLS}
    report.update(source_commit=commit, binary_source_commit=checkpoint.get("binary_source_commit", commit),
                  source_sha256=checkpoint["source_sha256"], binary_sha256=checkpoint["binary_sha256"],
                  phases={}, runs={})
    try:
        for case in CASES:
            run = {}
            report["runs"][case] = run
            qualify_case(binary, tools, case, run)
            report["phases"][case] = run["phase"]
    finally:
        clean = bool(report["runs"]) and all(
            run.get("cleanup", {}).get("status") == "PASS"
            and run.get("cleanup", {}).get("owned_root_absent")
            for run in report["runs"].values())
        report["cleanup"] = {"status": "PASS" if clean else "FAIL", "owned_root_absent": clean}


def qualify_case(binary: Path, tools: dict, case: str, report: dict) -> None:
    if case not in CASES:
        raise SmokeError("inventory", "exact native case required")
    # Every case gets fresh databases, PKI, marker and private socket. No case
    # inherits another's event rows, child guards or settlement registrations.
    cluster = Cluster(tools)
    report["owned_root"] = str(cluster.root)
    try:
        cluster.init()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            # Holds both source-owner guards plus full-page local children.
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        pki(cluster)
        version = cluster.sql("postgres", "show server_version_num;")
        if int(version) // 10000 != 16:
            raise SmokeError("preflight", "PG16 required")
        report["postgres_version"] = cluster.sql("postgres", "show server_version;")
        env = {**cluster.env,
               "CONCERT_SERVICE_CREDENTIAL": "concert-fixture",
               "POST_SERVICE_CREDENTIAL": "post-fixture",
               "MCR_FEED_NATIVE_SOURCE_ROOT": str(ROOT),
               "MCR_CANONICAL_ROOT": str(cluster.root),
               "MCR_CANONICAL_ROOT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
               "MCR_CANONICAL_ROOT_TOKEN": cluster.token,
               "MCR_CANONICAL_SYSTEM": cluster.sql("postgres", "select system_identifier from pg_control_system();")}
        for owner in OWNERS:
            db = f"mcr_canonical_{owner}"
            execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db],
                    env=cluster.client_env("postgres"), phase=f"create-{owner}", cwd=cluster.root)
            env[f"MCR_CANONICAL_{owner.upper()}_URL"] = cluster.url(db)
        result = execute([str(binary), "--ignored", "--exact", case,
                          "--test-threads=1", "--nocapture"],
                         env=env, phase=case, cwd=ROOT / "backend", timeout=60)
        if "test result: ok. 1 passed; 0 failed; 0 ignored;" not in result.stdout:
            raise SmokeError("inventory", "expected one exact native test")
        report["phase"] = {"status": "PASS", "exit_code": result.returncode,
                           "stdout": result.stdout}
        report["migration_receipts"] = {}
        for owner in OWNERS:
            db = f"mcr_canonical_{owner}"
            report["migration_receipts"][owner] = check_ledger(
                cluster, db, migration_manifest(ROOT / "backend", f"{owner}-service"))
            if cluster.sql(db, "select count(*) from pg_prepared_xacts where database=current_database();") != "0":
                raise SmokeError("cleanup", "prepared transactions remain")
    finally:
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS", "owned_root_absent": not cluster.root.exists()}

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if (not args.report.is_absolute() or args.report.parent.resolve() != args.report.parent
            or args.report.exists() or not args.pg_bin_dir.is_absolute()):
        parser.error("new absolute report and explicit absolute PG directory required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"scope": "feed-native-owner-join", "live_http": "NOT_RUN", "status": "FAIL", "limitations": [
        "Real Feed owner component only; actual Gateway HTTP body completion remains unqualified.",
        "Media, links, additional negative paths and process/PostgreSQL restart remain separate obligations."]}
    try:
        qualify(json.loads(args.checkpoint.read_text()), args.pg_bin_dir, report)
        report["status"] = "PASS"
    except SmokeError as error:
        report["failure"] = error.detail
    except Exception as error:
        report["failure"] = {"phase": "runner", "diagnostic": type(error).__name__}
    finally:
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2, sort_keys=True)
            stream.write("\n")
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1

if __name__ == "__main__":
    sys.exit(main())
