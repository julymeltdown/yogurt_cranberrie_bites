#!/usr/bin/env python3
"""Qualify copied Auth replay RPC artifacts in one fresh owned PG16 cluster."""
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

REQUIRED = {"auth-service", "concert_invalidation_rpc", "concert_relationship", "unit"}
ROOT = BACKEND.parent


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources() -> dict[str, str]:
    paths = [
        "backend/services/auth-service/src/adapters/in/grpc/concert_invalidation.rs",
        "backend/services/auth-service/src/adapters/in/grpc/mod.rs",
        "backend/services/auth-service/src/main.rs",
        "backend/services/auth-service/src/concert_invalidation/mod.rs",
        "backend/services/auth-service/tests/concert_invalidation_rpc.rs",
        "backend/services/auth-service/tests/concert_invalidation_rpc_support/native.rs",
        "backend/services/auth-service/tests/concert_relationship.rs",
        "backend/services/auth-service/Cargo.toml", "backend/services/auth-service/build.rs",
        "backend/Cargo.toml", "backend/Cargo.lock",
        "backend/proto/relationship/v1/relationship.proto",
        "backend/crates/platform/src/grpc_auth.rs", "backend/crates/platform/src/grpc_method_policy.rs",
        "backend/crates/platform/src/grpc_tls.rs", "backend/crates/platform/src/call_deadline.rs",
        "backend/scripts/mcr/auth_invalidation_rpc_smoke.py",
        "backend/scripts/mcr/auth_invalidation_smoke.py", "backend/scripts/mcr/migration_smoke.py",
    ]
    paths += [str(p.relative_to(ROOT)) for p in sorted((BACKEND / "services/auth-service/migrations").glob("*.sql"))]
    return {p: digest(ROOT / p) for p in paths}


def binaries(metadata: Path) -> dict[str, Path]:
    if not metadata.is_absolute() or metadata.is_symlink():
        raise SmokeError("preflight", "absolute frozen-artifact metadata required")
    data = json.loads(metadata.read_text())
    if set(data["artifacts"]) != REQUIRED:
        raise SmokeError("preflight", "exact RPC, relationship, unit and Auth binary inventory required")
    current = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()
    if data.get("source_commit") != current or dirty or data.get("source_sha256") != sources():
        raise SmokeError("preflight", "frozen artifact source checkpoint mismatch")
    return {k: frozen_binary(Path(v["path"]), v["sha256"]) for k, v in data["artifacts"].items()}


def test(binary: Path, env: dict, report: dict, phase: str, passed: int, exact: str | None = None) -> None:
    args = [str(binary), "--test-threads=1"]
    if exact:
        args += ["--ignored", "--exact", exact]
    result = execute(args, env=env, phase=phase, timeout=60, cwd=BACKEND)
    if f"test result: ok. {passed} passed; 0 failed;" not in result.stdout:
        raise SmokeError(phase, "expected exact passing test inventory did not run")
    report[phase] = {"status": "PASS", "exit_code": result.returncode, "passed": passed,
                     "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest()}


def qualify(artifacts: dict[str, Path], pg: Path, report: dict) -> None:
    tools = {n: executable(pg / n) for n in PG_TOOLS}
    cluster = Cluster(tools)
    report["owned_root"] = str(cluster.root)
    try:
        cluster.init()
        cluster.start()
        version = cluster.sql("postgres", "show server_version_num;")
        if int(version) // 10000 != 16:
            raise SmokeError("preflight", "PG16 required")
        report["postgres_version"] = version
        for db in ["mcr_auth_invalidation_rpc", "mcr_auth_relationship"]:
            execute([str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", db],
                    env=cluster.client_env("postgres"), phase="createdb", cwd=cluster.root)
        common = {**cluster.env, "MCR_INVALIDATION_RPC_ROOT": str(cluster.root),
                  "MCR_INVALIDATION_RPC_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
                  "MCR_INVALIDATION_RPC_TOKEN": cluster.token,
                  "MCR_INVALIDATION_RPC_DATABASE_URL": cluster.url("mcr_auth_invalidation_rpc"),
                  "MCR_INVALIDATION_RPC_SYSTEM_IDENTIFIER": cluster.sql("postgres", "select system_identifier from pg_control_system();")}
        test(artifacts["concert_invalidation_rpc"], cluster.env, report, "pure_rpc", 4)
        test(artifacts["concert_relationship"], cluster.env, report, "relationship_guard_regression", 2)
        test(artifacts["unit"], cluster.env, report, "auth_unit", 124)
        test(artifacts["concert_invalidation_rpc"], common, report, "native_tls_mutations", 1,
             "native::owning_mutations_replay_over_guarded_mutual_tls")
        test(artifacts["concert_invalidation_rpc"], common, report, "native_deadline_cancellation", 1,
             "native::deadline_includes_pool_sql_and_cancellation_releases_work")
        relationship_env = {**cluster.env, "AUTH_DATABASE_URL": cluster.url("mcr_auth_relationship")}
        for command in ["--migrate", "--verify-migrations"]:
            execute([str(artifacts["auth-service"]), command], env=relationship_env,
                    phase="relationship-" + command, cwd=BACKEND)
        relationship_env.update(MCR_RELATIONSHIP_ROOT=str(cluster.root),
                                MCR_RELATIONSHIP_IDENTITY=f"{cluster.identity[0]}:{cluster.identity[1]}",
                                MCR_RELATIONSHIP_TOKEN=cluster.token,
                                MCR_RELATIONSHIP_AUTH_URL=cluster.url("mcr_auth_relationship"))
        test(artifacts["concert_relationship"], relationship_env, report, "native_relationship_regression", 1,
             "native_relationship_owner_snapshot_and_revisions")
        manifest = migration_manifest(BACKEND, "auth-service")
        report["migration_sha384"] = manifest
        for db in ["mcr_auth_invalidation_rpc", "mcr_auth_relationship"]:
            check_ledger(cluster, db, manifest)
        report["committed_cursor_and_count"] = cluster.sql("mcr_auth_invalidation_rpc", "select committed_cursor,(select count(*) from auth_concert_invalidation_journal) from auth_concert_invalidation_head;")
        before = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after = cluster.assert_postmaster()
        if before == after:
            raise SmokeError("restart", "postmaster identity did not change")
        report["restart"] = {"status": "PASS", "before_pid": before, "after_pid": after}
        test(artifacts["concert_invalidation_rpc"], common, report, "native_restart", 1,
             "native::restart_replays_same_epoch_positions_and_signal_identity")
        for db in ["mcr_auth_invalidation_rpc", "mcr_auth_relationship"]:
            check_ledger(cluster, db, manifest)
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
    report = {"status": "FAIL", "limitations": ["No deployed service credentials or production World consumer qualification.",
              "Client-side channel readiness and response validation must use the same original consumer deadline.",
              "SQLx queues rollback on cancellation; in-flight SQL retains its bounded original statement timer."]}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in [signal.SIGINT, signal.SIGTERM]:
            previous[sig] = signal.signal(sig, interrupted)
        artifacts = binaries(args.binaries_json)
        report.update(source_commit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                      source_clean=True, source_sha256=sources(), metadata_sha256=digest(args.binaries_json),
                      binaries_sha256={k: digest(p) for k, p in artifacts.items()})
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
