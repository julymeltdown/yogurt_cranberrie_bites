#!/usr/bin/env python3
"""Qualify frozen Concert replay clients against both real owners in owned PG16."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import signal
import subprocess
import sys
from auth_invalidation_smoke import frozen_binary
from migration_smoke import BACKEND, Cluster, PG_TOOLS, SmokeError, check_ledger, executable, execute, migration_manifest

ROOT = BACKEND.parent
REQUIRED = {"unit", "invalidation_clients", "relationship_authority"}
DATABASES = {"auth-service": "mcr_invalidation_client_auth", "friend-service": "mcr_invalidation_client_friend"}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources() -> dict[str, str]:
    paths = {BACKEND / "Cargo.toml", BACKEND / "Cargo.lock", Path(__file__).resolve(),
             BACKEND / "scripts/mcr/auth_invalidation_smoke.py", BACKEND / "scripts/mcr/migration_smoke.py",
             BACKEND / "scripts/mcr/test_invalidation_clients_smoke.py"}
    # Pin the actual linked owners/platform and generated contract inputs, not
    # only the client facade. Peer changes cannot reuse an old executable receipt.
    for directory in ["services/auth-service", "services/friend-service", "services/concert-service", "crates/platform", "proto/relationship"]:
        paths.update(p for p in (BACKEND / directory).rglob("*") if p.is_file() and p.suffix in {".rs", ".toml", ".sql", ".proto"})
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}


def binaries(metadata: Path) -> dict[str, Path]:
    if not metadata.is_absolute() or metadata.is_symlink():
        raise SmokeError("preflight", "absolute frozen-artifact metadata required")
    data = json.loads(metadata.read_text())
    if set(data["artifacts"]) != REQUIRED:
        raise SmokeError("preflight", "exact Concert unit/client/authority artifact inventory required")
    current = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()
    if data.get("source_commit") != current or dirty or data.get("source_sha256") != sources():
        raise SmokeError("preflight", "frozen artifact source checkpoint mismatch")
    return {k: frozen_binary(Path(v["path"]), v["sha256"]) for k, v in data["artifacts"].items()}


def test(binary: Path, env: dict, report: dict, phase: str, passed: int,
         exact: str | None = None, prefix: str | None = None) -> None:
    args = [str(binary), "--test-threads=1"]
    if exact:
        args += ["--ignored", "--exact", exact]
    if prefix:
        args.append(prefix)
    result = execute(args, env=env, phase=phase, timeout=60, cwd=BACKEND)
    if f"test result: ok. {passed} passed; 0 failed;" not in result.stdout:
        raise SmokeError(phase, "expected exact passing test inventory did not run")
    report[phase] = {"status": "PASS", "exit_code": result.returncode, "passed": passed,
                     "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest()}


def pki(cluster: Cluster) -> None:
    cluster.assert_owned()
    directory = cluster.root / "pki"
    directory.mkdir(mode=0o700)
    openssl = executable(Path("/usr/bin/openssl"))
    def run(*args):
        execute([str(openssl), *map(str, args)], env=cluster.env, cwd=directory, phase="fixture-pki", timeout=30)
    config = directory / "ca.conf"
    config.write_text("[req]\ndistinguished_name=dn\n[dn]\n[v3_ca]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n")
    run("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "ca.key", "-out", "ca.crt", "-days", "2", "-subj", "/CN=MCR private client qualification", "-config", config, "-extensions", "v3_ca")
    for role in ["auth", "friend", "concert", "gateway"]:
        extension = directory / f"{role}.ext"
        extension.write_text(f"basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:{role}.svc.internal,URI:spiffe://internal/{role}\n")
        run("req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", f"{role}.key", "-out", f"{role}.csr", "-subj", f"/CN={role}.svc.internal")
        run("x509", "-req", "-in", f"{role}.csr", "-CA", "ca.crt", "-CAkey", "ca.key", "-CAcreateserial", "-out", f"{role}.crt", "-days", "2", "-extfile", extension)
    for key in directory.glob("*.key"):
        key.chmod(0o600)


def qualify(artifacts: dict[str, Path], pg: Path, report: dict) -> None:
    tools = {n: executable(pg / n) for n in PG_TOOLS}
    cluster = Cluster(tools)
    report["owned_root"] = str(cluster.root)
    try:
        pki(cluster)
        cluster.init()
        cluster.start()
        version = cluster.sql("postgres", "show server_version_num;")
        if int(version) // 10000 != 16:
            raise SmokeError("preflight", "PG16 required")
        report["postgres_version"] = version
        for db in DATABASES.values():
            execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db], env=cluster.client_env("postgres"), phase="createdb", cwd=cluster.root)
        env = {**cluster.env, "MCR_INVALIDATION_CLIENT_ROOT": str(cluster.root),
               "MCR_INVALIDATION_CLIENT_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
               "MCR_INVALIDATION_CLIENT_TOKEN": cluster.token,
               "MCR_INVALIDATION_CLIENT_AUTH_DATABASE_URL": cluster.url(DATABASES["auth-service"]),
               "MCR_INVALIDATION_CLIENT_FRIEND_DATABASE_URL": cluster.url(DATABASES["friend-service"]),
               "MCR_INVALIDATION_CLIENT_SYSTEM_IDENTIFIER": cluster.sql("postgres", "select system_identifier from pg_control_system();")}
        test(artifacts["unit"], cluster.env, report, "sealed_validation", 4, prefix="adapters::out::relationship_authority::invalidation::validation::tests::")
        test(artifacts["invalidation_clients"], cluster.env, report, "pure_configuration", 1)
        test(artifacts["relationship_authority"], cluster.env, report, "retained_authority_regression", 3)
        for phase, name in [("malicious_transport", "transport::authenticated_malicious_peers_cannot_mint_verified_observations"),
                            ("original_budget", "transport::original_budget_includes_readiness_decode_validation_and_cancellation"),
                            ("real_owner_mutations", "native::both_real_owner_mutations_replay_as_sealed_pages")]:
            test(artifacts["invalidation_clients"], env, report, phase, 1, exact=name)
        report["migration_sha384"] = {}
        report["owner_journals"] = {}
        for service, db in DATABASES.items():
            manifest = migration_manifest(BACKEND, service)
            report["migration_sha384"][service] = manifest
            check_ledger(cluster, db, manifest)
            owner = service.removesuffix("-service")
            report["owner_journals"][owner] = cluster.sql(db, f"select committed_cursor,(select count(*) from {owner}_concert_invalidation_journal) from {owner}_concert_invalidation_head;")
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise SmokeError("restart", "postmaster identity did not change")
        report["restart"] = {"status": "PASS", "before_pid": before, "after_pid": after}
        test(artifacts["invalidation_clients"], env, report, "physical_restart", 1, exact="native::restart_preserves_both_owner_epochs_positions_and_signal_content")
        for service, db in DATABASES.items():
            check_ledger(cluster, db, report["migration_sha384"][service])
    finally:
        cluster.cleanup()
        report["cleanup"] = {"status": "PASS", "owned_root_absent": not cluster.root.exists()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binaries-json", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not args.report.is_absolute() or args.report.exists() or not args.pg_bin_dir.is_absolute():
        parser.error("new absolute report and explicit PG binary directory required")
    report = {"status": "FAIL", "limitations": ["No World effects, durable consumer checkpoint, deployed credentials or release readiness certified.", "Authenticated replay observations only; typed authority rereads remain required.", "Across-page/restart signal deduplication belongs to the World durable ledger."]}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in [signal.SIGINT, signal.SIGTERM]:
            previous[sig] = signal.signal(sig, interrupted)
        artifacts = binaries(args.binaries_json)
        report.update(source_commit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), source_clean=True, source_sha256=sources(), metadata_sha256=digest(args.binaries_json), binaries_sha256={k: digest(p) for k, p in artifacts.items()})
        qualify(artifacts, args.pg_bin_dir, report)
        report["status"] = "PASS"
    except SmokeError as error:
        report["failure"] = error.detail
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = {"phase": "runner", "diagnostic": type(error).__name__}
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with args.report.open("x") as out:
            json.dump(report, out, indent=2, sort_keys=True)
            out.write("\n")
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
