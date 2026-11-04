#!/usr/bin/env python3
"""Actual public Gateway body qualification on a newly owned real-login stack."""
from __future__ import annotations

import argparse
import http.client
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import time
from unittest.mock import patch
from uuid import UUID

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import browser_auth_smoke as auth
import browser_refresh
import dev_stack as dev
from auth_invalidation_smoke import frozen_binary
from invalidation_clients_smoke import pki
sys.path.insert(0, str(ROOT / "backend/services/concert-service/tests/owner_authority_support"))
import qualify as core

CASE = "http::actual_public_router_retains_real_owners_and_checks_before_bytes"


def source_manifest():
    paths = set(core.source_manifest())
    # The HTTP executable now links the actual public Gateway and all its clients.
    for service in dev.runtime.START_ORDER:
        paths.update(str(p.relative_to(ROOT)) for p in (ROOT / "backend/services" / service).rglob("*")
                     if p.is_file() and p.suffix in {".rs", ".sql", ".toml", ".proto", ".py", ".json"})
    paths.update(str(p.relative_to(ROOT)) for p in (ROOT / "backend/scripts/mcr").glob("*.py"))
    return {name: core.sha(ROOT / name) for name in sorted(paths)}


def principal(stack, browser):
    """A real signup, private outbox delivery, email verification and login."""
    email = f"mcr-owner-http-{secrets.token_hex(12)}@example.test"
    password = secrets.token_urlsafe(24)
    _, created = browser.request("http-signup", "/api/auth/email/signup", {"email": email, "password": password})
    user = str(UUID(created["userId"]))
    auth.require(UUID(user).int != 0 and user == created["userId"], "canonical-signup")
    deadline, code = time.monotonic() + 20, None
    while code is None and time.monotonic() < deadline:
        stack.cluster.assert_owned()
        for path in stack.mail.directory.glob("*.json"):
            auth.require(not path.is_symlink() and path.stat().st_mode & 0o077 == 0, "private-inbox")
            message = json.loads(path.read_bytes())
            if message["kind"] == "VERIFY_EMAIL" and message["payload"]["email"] == email:
                code = message["payload"]["verification_code"]
                break
        if code is None:
            time.sleep(0.1)
    auth.require(code is not None, "real-verification-mail")
    _, verified = browser.request("http-verify", "/api/auth/email/verify", {"email": email, "code": code})
    auth.require(verified["userId"] == user, "canonical-verification")
    headers, logged_in = browser.request("http-login", "/api/auth/login", {"email": email, "password": password})
    auth.require(logged_in["userId"] == user and logged_in["accessToken"]
                 and logged_in["refreshToken"] == "", "canonical-login")
    auth.refresh_cookie(headers)
    # Auth signup does not create the self Profile required by the production
    # ConcertIdentityReader. Use the actual public API, never a fabricated
    # identity observation or SQL-inserted Profile. Auth-only Browser.request
    # assumes response headers that the ordinary Profile route does not promise.
    connection = http.client.HTTPConnection(browser.target.hostname, browser.target.port, timeout=10)
    try:
        connection.request("PUT", "/api/profile/" + user,
            body=json.dumps({"username": "owner_" + secrets.token_hex(6),
                             "nickname": "Owned HTTP participant",
                             "avatarUrl": "/world/avatars/derived-v1/photo01-front-idle-64-e124c88815b22220.png",
                             "bio": "Owned canonical HTTP fixture"}),
            headers={"Authorization": "Bearer " + logged_in["accessToken"],
                     "Content-Type": "application/json", "Origin": browser.origin,
                     "X-MCR-Intent": "browser", "X-Correlation-Id": "owner-http-profile"})
        response = connection.getresponse()
        body = response.read(65537)
        browser.report["statuses"]["http-profile"] = response.status
        auth.require(response.status == 200 and len(body) <= 65536, "actual-self-profile-create")
        auth.require(json.loads(body)["userId"] == user, "actual-self-profile-owner")
    finally:
        connection.close()
    # No token parsing, synthetic sessions, Auth claim SQL or token files.
    return {"user_id": user, "access_token": logged_in["accessToken"]}


def qualify(args, report):
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    auth.require(not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip(), "clean-source")
    auth.require(shutil.disk_usage(ROOT).free >= 8 * 1024**3, "eight-gib-free")
    if args.prepare_only:
        checkpoint = {"source_commit": commit, "source_sha256": source_manifest(), "binary_sha256": "NOT_RUN"}
        binary = None
    else:
        checkpoint = json.loads(args.checkpoint.read_bytes())
        auth.require(checkpoint["source_commit"] == commit and checkpoint["source_sha256"] == source_manifest(), "source-pins")
        auth.require(checkpoint["exit_code"] == 0 and checkpoint["target_name"] == "product_owner_cancellation", "exact-compiled-test")
        binary = frozen_binary(Path(checkpoint["binary"]), checkpoint["binary_sha256"])
    metadata = json.loads(args.stack_artifacts.read_bytes())
    auth.require(set(metadata["binary_sha256"]) == set(dev.runtime.START_ORDER), "six-service-inventory")
    binaries = {}
    for service, expected in metadata["binary_sha256"].items():
        path = frozen_binary(args.bin_dir / service, expected["sha256"])
        auth.require(path.stat().st_size == expected["size"], "frozen-service-size")
        expected_migrations = {str(k): v for k, v in dev.migration.migration_manifest(ROOT / "backend", service).items()}
        auth.require(metadata["migrations"][service] == expected_migrations, "service-migration-pins")
        binaries[service] = path
    report.update(source_commit=commit, source_sha256=checkpoint["source_sha256"],
                  binary_sha256=checkpoint["binary_sha256"], stack_source_revision=metadata["source_revision"],
                  stack_artifacts_sha256=core.sha(args.stack_artifacts), stack_binary_sha256=metadata["binary_sha256"])
    stack = dev.DevStack(args.bin_dir, args.pg_bin_dir, args.redis_binary, args.openssl_binary)
    original_init, original_envs = dev.migration.Cluster.init, dev.development_environments
    gateway_env = {}

    def init_owned(cluster):
        original_init(cluster)
        with (cluster.data / "postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=256\n")

    def environments(cluster, *params, **kwargs):
        # Derive the actual generated keys, credentials and six concrete peer
        # endpoints from DevStack's existing checked environment builder.
        envs = original_envs(cluster, *params, **kwargs)
        pki(cluster)
        for owner in core.OWNERS:
            database = f"mcr_canonical_{owner}"
            dev.migration.execute([str(cluster.tools["createdb"]), "--no-password", "--maintenance-db=postgres", database],
                                  env=cluster.client_env("postgres"), cwd=cluster.root, phase=f"canonical-{owner}-create")
            if owner in ("auth", "friend"):
                service = owner + "-service"
                variable = owner.upper() + "_DATABASE_URL"
                # Actual main processes serve the same fresh Auth/Friend DBs
                # that the canonical production owner-store libraries fence.
                envs[service][variable] = cluster.url(database)
                for flag in ("--migrate", "--verify-migrations"):
                    dev.migration.execute([str(binaries[service]), flag],
                                          env={**cluster.env, variable: cluster.url(database)},
                                          cwd=cluster.root, phase=f"canonical-{owner}-{flag}")
        gateway_env.update(envs["api-gateway"])
        gateway_env.update(CONCERT_SERVICE_CREDENTIAL="concert-fixture",
                           MCR_CANONICAL_ROOT=str(cluster.root),
                           MCR_CANONICAL_ROOT_IDENTITY=f"{cluster.identity[0]}:{cluster.identity[1]}",
                           MCR_CANONICAL_ROOT_TOKEN=cluster.token,
                           MCR_CANONICAL_SYSTEM=cluster.sql("postgres", "select system_identifier from pg_control_system();"))
        for owner in core.OWNERS:
            gateway_env[f"MCR_CANONICAL_{owner.upper()}_URL"] = cluster.url(f"mcr_canonical_{owner}")
        return envs

    try:
        with patch.object(dev.migration.Cluster, "init", init_owned), \
             patch.object(dev, "development_environments", environments), \
             patch.object(dev.runtime, "readiness_value", browser_refresh.current_readiness):
            descriptor = stack.start()
        version = stack.cluster.sql("postgres", "show server_version_num;")
        auth.require(int(version) // 10000 == 16, "actual-pg16")
        report["postgres_version"] = stack.cluster.sql("postgres", "show server_version;")
        credentials = principal(stack, auth.Browser(descriptor["gateway_url"], report))
        if args.prepare_only:
            # Observe real service-created identity in the exact canonical DB;
            # preparation never constructs a successful Product/scope result.
            user = str(UUID(credentials["user_id"]))
            observed = stack.cluster.sql("mcr_canonical_auth", "select count(*) from users where id='" + user + "' and status='ACTIVE';")
            auth.require(observed == "1", "real-main-uses-canonical-auth-database")
            sessions = stack.cluster.sql("mcr_canonical_auth", "select count(*) from refresh_tokens where user_id='" + user + "' and expires_at>clock_timestamp();")
            auth.require(int(sessions) > 0, "real-login-session-in-canonical-auth-database")
            credentials.clear()
            report.update(status="PARTIAL", gateway_canonical_http="NOT_RUN",
                          preparation="Real signup/verification/login and canonical Auth session observed; no compiled HTTP fixture")
            return
        result = subprocess.run([str(binary), "--ignored", "--exact", CASE, "--test-threads=1", "--nocapture"],
                                input=json.dumps(credentials), env=gateway_env, cwd=ROOT / "backend",
                                capture_output=True, text=True, timeout=50)
        hidden = (credentials["access_token"], *(v for k, v in gateway_env.items()
                  if any(word in k for word in ("TOKEN", "KEY", "SEED", "CREDENTIAL", "DATABASE_URL"))))
        report["test"] = {"case": CASE, "exit_code": result.returncode,
                          "stdout": dev.migration.redact(result.stdout, hidden),
                          "stderr": dev.migration.redact(result.stderr, hidden)}
        credentials.clear()
        auth.require(result.returncode == 0 and "test result: ok. 1 passed; 0 failed;" in result.stdout, "exact-http-native-case")
        counts = {owner: int(stack.cluster.sql(f"mcr_canonical_{owner}",
                  "select count(*) from pg_prepared_xacts where database=current_database();")) for owner in core.OWNERS}
        auth.require(counts == {"auth": 2, "friend": 2, "concert": 0}, "reset-and-expiry-remain-quarantined")
        report["expected_reset_and_expired_quarantine"] = counts
        report["migration_receipts"] = {owner: dev.migration.check_ledger(stack.cluster, f"mcr_canonical_{owner}",
            dev.migration.migration_manifest(ROOT / "backend", owner + "-service")) for owner in core.OWNERS}
        report["status"] = "PASS"
    finally:
        gateway_env.clear()
        stack.close()
        report["owned_stack"] = stack.report
        report["cleanup"] = {"status": stack.report.get("cleanup"),
                             "owned_root_absent": stack.cluster is not None and not stack.cluster.root.exists(),
                             "unsettled_scopes": "Owned fixture teardown only; no fabricated successful terminal delivery"}
        if report["cleanup"]["status"] != "PASS" or not report["cleanup"]["owned_root_absent"]:
            report["status"] = "FAIL"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("stack-artifacts", "bin-dir", "pg-bin-dir", "redis-binary", "openssl-binary", "report"):
        parser.add_argument("--" + flag, type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--prepare-only", action="store_true",
                        help="Run real owned identity preparation only; result remains PARTIAL and HTTP NOT_RUN")
    args = parser.parse_args()
    if (any(not value.is_absolute() for value in vars(args).values() if isinstance(value, Path))
            or (not args.prepare_only and args.checkpoint is None) or args.report.exists()):
        parser.error("explicit absolute inputs and new report required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"status": "FAIL", "statuses": {}, "limitations": [
        "Six-service identity mains use actual parity configuration, not full internal TLS startup.",
        "The producer deliberately refuses; no positive product, Gift, child-join or consumer restart proof.",
        "Reset/expired-response owner exclusions remain quarantined until owned fixture teardown."]}
    try:
        qualify(args, report)
    except BaseException as error:
        report["failure"] = (str(error) if isinstance(error, auth.CheckFailure)
                             else error.detail if isinstance(error, dev.SmokeError) else type(error).__name__)
        report["status"] = "FAIL"
    finally:
        dev.write_fd(fd, report)
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return int(report["status"] != ("PARTIAL" if args.prepare_only else "PASS"))


if __name__ == "__main__":
    raise SystemExit(main())
