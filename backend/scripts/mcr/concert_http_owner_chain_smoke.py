#!/usr/bin/env python3
"""Run the HTTP -> Gateway -> guarded owners -> isolated PostgreSQL gate."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import signal
import subprocess
from datetime import datetime, timezone

from migration_smoke import Cluster, SmokeError, execute


BACKEND = Path(__file__).resolve().parents[2]
REPOSITORY = BACKEND.parent
TEST_NAME = (
    "native_http_real_owner_chain_success_negatives_and_safe_failures"
)
SERVICES = {
    "auth": ("auth-service", "mcr_http_owner_auth"),
    "profile": ("profile-service", "mcr_http_owner_profile"),
    "gateway": ("api-gateway", "mcr_http_owner_gateway"),
}
PG_TOOLS = ("initdb", "pg_ctl", "psql", "createdb")
REQUIRED_COMMITS = (
    "775d5f1",
    "16a37fa",
    "4d66ce4",
    "1f6dceb",
)
CANARIES = (
    "owner-chain-private@example.invalid",
    "OwnerChain-H3a-Private-Password!",
    "Private owner nickname",
    "private-owner-bio-canary",
    "private-owner-username",
    "https://private-owner-avatar.invalid/secret",
    "private-owner-delete-reason-canary",
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def redact(text: str, password: str) -> str:
    text = text.replace(password, "[redacted]")
    text = re.sub(r"(?:postgres(?:ql)?|redis)://[^\s]+", "[redacted-url]", text)
    for canary in CANARIES:
        text = text.replace(canary, "[private-canary]")
    return text[-16_000:]


def new_attempt(output: Path) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    attempt = output / f"attempt-{stamp}-{secrets.token_hex(4)}"
    attempt.mkdir(mode=0o700, parents=True, exist_ok=False)
    return attempt


def write_new(path: Path, data: str, mode: int = 0o600) -> None:
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(fd, "w") as stream:
        stream.write(data)


def command(
    argv: list[str], *, env: dict[str, str], phase: str, attempt: Path,
    password: str, output_name: str, timeout: int = 120,
    allowed: tuple[int, ...] = (0,),
) -> dict:
    try:
        result = subprocess.run(
            argv, env=env, capture_output=True, text=True, timeout=timeout,
            cwd=REPOSITORY,
        )
    except subprocess.TimeoutExpired:
        raise SmokeError(phase, "command timed out") from None
    except OSError as error:
        raise SmokeError(phase, f"cannot execute command (errno {error.errno})") from None
    diagnostic = redact((result.stdout or "") + (result.stderr or ""), password)
    log = attempt / output_name
    write_new(log, diagnostic)
    if result.returncode not in allowed:
        raise SmokeError(phase, redact(result.stderr or result.stdout, password), result.returncode)
    return {
        "phase": phase,
        "argv": argv,
        "exit_code": result.returncode,
        "log": str(log),
        "log_sha256": sha256(log),
    }


def executable(path: Path) -> Path:
    resolved = path.resolve()
    if not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise SmokeError("preflight", f"missing executable: {path.name}")
    return resolved


def binary_from_metadata(path: Path, target: Path) -> tuple[Path, dict]:
    metadata = path.resolve()
    if not metadata.is_file() or metadata.is_symlink():
        raise SmokeError("preflight", "build metadata must be a regular file")
    executables = []
    reason_counts = {"compiler-artifact": 0, "build-finished": 0}
    for line in metadata.read_text().splitlines():
        item = json.loads(line)
        reason = item.get("reason")
        if reason in reason_counts:
            reason_counts[reason] += 1
        if reason == "build-finished" and item.get("success") is not True:
            raise SmokeError("preflight", "build did not succeed")
        if reason == "compiler-artifact" and item.get("target", {}).get("name") == "concert_http_owner_chain":
            source = Path(item["target"].get("src_path", "")).resolve()
            if source != BACKEND / "services/auth-service/tests/concert_http_owner_chain.rs":
                raise SmokeError("preflight", "artifact source is not this checkout")
            if item.get("profile", {}).get("debuginfo") != 0:
                raise SmokeError("preflight", "test artifact must have debuginfo 0")
            executable_value = item.get("executable")
            if executable_value:
                executables.append(Path(executable_value))
    if reason_counts["build-finished"] != 1:
        raise SmokeError("preflight", "build metadata must contain one completion record")
    if len(executables) != 1:
        raise SmokeError("preflight", "build metadata must identify one test executable")
    binary = executables[0].resolve()
    if target not in binary.parents or not binary.is_file() or not os.access(binary, os.X_OK):
        raise SmokeError("preflight", "test executable is outside the explicit target")
    return binary, {
        "path": str(metadata),
        "sha256": sha256(metadata),
        "records": reason_counts,
    }


def ledger(cluster: Cluster, database: str) -> int:
    rows = cluster.sql(
        database,
        "select version,success from public._sqlx_migrations order by version;",
    )
    versions = []
    for row in rows.splitlines():
        version, success = row.split("|")
        if success != "t":
            raise SmokeError("migration", f"unsuccessful migration receipt: {database}")
        versions.append(int(version))
    if not versions or versions != sorted(set(versions)):
        raise SmokeError("migration", f"invalid migration receipt sequence: {database}")
    return len(versions)


def run(args: argparse.Namespace) -> dict:
    output = args.output.resolve()
    output.mkdir(mode=0o700, parents=True, exist_ok=True)
    attempt = new_attempt(output)
    report = {
        "task": "BE04 real HTTP identity -> Auth/Profile owner chain",
        "result": "FAIL",
        "scope": "test-only HTTP Router; no public endpoint, TLS, event authority or grant",
        "branch": None,
        "head": None,
        "required_commits": {},
        "started_at_utc": utc_now(),
        "commands": [],
        "database": "NOT_RUN",
        "test": {"distinct": 1, "expected_passed": 1, "passed": 0},
    }
    cluster: Cluster | None = None
    jwt_key = subprocess.run(
        ["/usr/bin/openssl", "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048"],
        env={"PATH": "/usr/bin:/bin"}, capture_output=True, text=True, timeout=60,
        check=False,
    )
    if jwt_key.returncode != 0 or "BEGIN PRIVATE KEY" not in jwt_key.stdout:
        raise SmokeError("preflight", "cannot generate isolated signing key")
    try:
        if not args.output.is_absolute() or not args.pg_bin.is_absolute():
            raise SmokeError("preflight", "output and pg-bin must be absolute")
        binary, metadata = binary_from_metadata(args.build_metadata, args.target_dir.resolve())
        tools = {name: executable(args.pg_bin / name) for name in PG_TOOLS}
        head = execute(
            ["/usr/bin/git", "rev-parse", "HEAD"],
            env={"PATH": "/usr/bin:/bin"}, phase="git-head", cwd=REPOSITORY,
        ).stdout.strip()
        branch = execute(
            ["/usr/bin/git", "rev-parse", "--abbrev-ref", "HEAD"],
            env={"PATH": "/usr/bin:/bin"}, phase="git-branch", cwd=REPOSITORY,
        ).stdout.strip()
        report["head"], report["branch"] = head, branch
        report["sources"] = {
            str(path.relative_to(REPOSITORY)): sha256(path)
            for path in (
                Path(__file__).resolve(),
                BACKEND / "services/auth-service/tests/concert_http_owner_chain.rs",
            )
        }
        for commit in REQUIRED_COMMITS:
            execute(
                ["/usr/bin/git", "merge-base", "--is-ancestor", commit, head],
                env={"PATH": "/usr/bin:/bin"}, phase=f"ancestry:{commit}", cwd=REPOSITORY,
            )
            report["required_commits"][commit] = "ANCESTOR"
        report["binary"] = {"path": str(binary), "sha256": sha256(binary)}
        report["build_metadata"] = metadata
        cluster = Cluster(tools)
        cluster.init()
        cluster.start()
        report["database"] = {
            "result": "IN_PROGRESS",
            "postgres_version": cluster.sql("postgres", "show server_version;"),
            "data_directory": cluster.sql("postgres", "show data_directory;"),
            "listen_addresses": cluster.sql("postgres", "show listen_addresses;"),
            "services": {},
        }
        if report["database"]["listen_addresses"]:
            raise SmokeError("database", "owned PG unexpectedly listens on TCP")
        env = {**cluster.env, "RUST_BACKTRACE": "0"}
        urls = {}
        for lane, (_, database) in SERVICES.items():
            execute(
                [str(tools["createdb"]), "--no-password", "--maintenance-db=postgres", database],
                env=cluster.client_env("postgres"), phase=f"createdb:{lane}", cwd=cluster.root,
            )
            urls[lane] = cluster.url(database)
        # Service startup applies the shipped SQLx migrations and schema probes.
        internal_credential = secrets.token_hex(24)
        for lane in ("GATEWAY", "AUTH", "PROFILE"):
            env[f"{lane}_RUNTIME_MODE"] = "parity"
            env[f"{lane}_DATABASE_MIGRATION_MODE"] = "startup"
        env.update({
            "GATEWAY_OUTBOUND_IDENTITY": internal_credential,
            "AUTH_OUTBOUND_IDENTITY": internal_credential,
            "PROFILE_OUTBOUND_IDENTITY": internal_credential,
            "GATEWAY_SERVICE_CREDENTIAL": internal_credential,
            "AUTH_SERVICE_CREDENTIAL": internal_credential,
            "PROFILE_SERVICE_CREDENTIAL": internal_credential,
        })
        env.update({
            "MCR_HTTP_OWNER_JWT_PRIVATE_KEY_PEM": jwt_key.stdout,
            "GATEWAY_JWT_ISSUER": "owner-chain-auth",
            "GATEWAY_JWT_AUDIENCE": "owner-chain-api",
            "AUTH_ISSUER": "owner-chain-auth",
            "AUTH_ACCESS_AUDIENCE": "owner-chain-api",
        })
        env.update({
            "MCR_HTTP_OWNER_AUTH_URL": urls["auth"],
            "MCR_HTTP_OWNER_PROFILE_URL": urls["profile"],
            "MCR_HTTP_OWNER_GATEWAY_URL": urls["gateway"],
        })
        result = command(
            [str(binary), "--ignored", "--exact", TEST_NAME, "--test-threads=1", "--nocapture"],
            env=env, phase="native-owner-chain", attempt=attempt,
            password=cluster.password, output_name="native-owner-chain.log",
            timeout=180,
        )
        report["commands"].append(result)
        for lane, (_, database) in SERVICES.items():
            report["database"]["services"][lane] = {
                "database": database,
                "migration_receipts": ledger(cluster, database),
                "schema_probe": cluster.sql(
                    database,
                    "select current_database()=datname from pg_database where datname=current_database();",
                ) == "t",
            }
            if not report["database"]["services"][lane]["schema_probe"]:
                raise SmokeError("database", f"database identity probe failed: {lane}")
        log_text = (attempt / "native-owner-chain.log").read_text()
        if "1 passed" not in log_text or "0 failed" not in log_text:
            raise SmokeError("test", "native test did not report exactly one passing case")
        report["test"]["passed"] = 1
        report["database"]["result"] = "PASS"
        report["result"] = "PASS"
    except (SmokeError, OSError, ValueError, json.JSONDecodeError) as error:
        detail = error.detail if isinstance(error, SmokeError) else {
            "phase": "harness", "diagnostic": type(error).__name__
        }
        report["failure"] = detail
    finally:
        if cluster is not None:
            try:
                assert cluster is not None
                cluster_identity = list(cluster.identity)
                root = str(cluster.root)
                cluster.cleanup()
                report["cleanup"] = {
                    "result": "PASS" if not Path(root).exists() else "FAIL",
                    "root": root,
                    "identity": cluster_identity,
                    "root_absent": not Path(root).exists(),
                    "postmaster_stopped": not (Path(root) / "data" / "postmaster.pid").exists(),
                }
                if not report["cleanup"]["root_absent"]:
                    report["result"] = "FAIL"
            except (SmokeError, OSError) as error:
                report["cleanup"] = {
                    "result": "FAIL",
                    "diagnostic": error.detail if isinstance(error, SmokeError) else type(error).__name__,
                    "root": str(cluster.root),
                }
                report["result"] = "FAIL"
        else:
            report["cleanup"] = {"result": "NOT_RUN", "root_absent": True}
        report["finished_at_utc"] = utc_now()
        receipt = attempt / "receipt.json"
        receipt.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        receipt.chmod(0o600)
        print(json.dumps({
            "result": report["result"], "receipt": str(receipt),
            "test_passed": report["test"]["passed"],
            "cleanup": report["cleanup"]["result"],
        }))
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-metadata", required=True, type=Path)
    parser.add_argument("--target-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--pg-bin", required=True, type=Path)
    args = parser.parse_args()
    if not args.output.is_absolute():
        parser.error("--output must be an absolute path")
    previous = signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
    try:
        report = run(args)
        return 0 if report["result"] == "PASS" else 1
    finally:
        signal.signal(signal.SIGTERM, previous)


if __name__ == "__main__":
    raise SystemExit(main())
