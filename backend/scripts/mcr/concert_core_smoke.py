#!/usr/bin/env python3
"""Qualify BE05 only in a new marker/inode-owned PostgreSQL cluster; no product API."""
from __future__ import annotations
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time
from uuid import uuid4
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_concert"

class CheckFailure(Exception):
    pass

def require(value, label):
    if not value:
        raise CheckFailure(label)

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def health(address, path):
    require(re.fullmatch(r"127\.0\.0\.1:[0-9]{1,5}", address) is not None, "foreign-health-target")
    port = int(address.rsplit(":", 1)[1])
    require(0 < port < 65536, "invalid-health-port")
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
    try:
        connection.request("GET", path)
        response = connection.getresponse()
        raw = response.read(65537)
        require(len(raw) <= 65536, "health-body-bound")
        if path in ("/readyz", "/livez"):
            require(response.getheader("cache-control") == "no-store", "health-no-store")
        return response.status, json.loads(raw) if path == "/readyz" else None
    finally:
        connection.close()

class OwnedService:
    def __init__(self, cluster, binary, env):
        self.cluster, self.binary, self.env = cluster, binary, env
        self.child = None
        self.log = None
        self.address = None

    def start(self):
        self.cluster.assert_owned()
        path = self.cluster.root / "concert-service.log"
        self.log = os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb")
        self.child = subprocess.Popen([str(self.binary)], cwd=self.cluster.root, env=self.env,
                                      stdout=self.log, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            self.cluster.assert_owned()
            require(self.child.poll() is None, "service-startup-exited")
            require(not path.is_symlink(), "replaced-service-log")
            for line in path.read_text().splitlines():
                if line.startswith("CONCERT_HEALTH_LISTENER="):
                    self.address = line.split("=", 1)[1]
                    status, _ = health(self.address, "/readyz")
                    require(status == 200, "initial-readiness")
                    return
            time.sleep(0.05)
        raise CheckFailure("service-startup-timeout")

    def stop(self):
        if self.child is None:
            if self.log is not None:
                self.log.close()
            return True
        self.cluster.assert_owned()
        graceful = True
        if self.child.poll() is None:
            self.child.terminate()
            try:
                self.child.wait(timeout=8)
            except subprocess.TimeoutExpired:
                graceful = False
                self.child.kill()  # Only this live, owned Popen child; never a PID from a file.
                self.child.wait(timeout=5)
        graceful = graceful and self.child.returncode == 0
        if self.log is not None:
            self.log.close()
        return graceful

def command(report, label, argv, env, root, *, success=True, secrets=()):
    result = subprocess.run([str(arg) for arg in argv], env=env, cwd=root, capture_output=True, timeout=30)
    report["commands"][label] = result.returncode
    require(all(value.encode() not in result.stdout + result.stderr for value in secrets if value), label + ":redaction")
    require((result.returncode == 0) == success, label + ":exit")
    return result

def exercise(cluster, binary, repository_test, report):
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    cluster.sql("postgres", "create database mcr_concert template template0;")
    url = cluster.url(DATABASE)
    env = {**cluster.env, "CONCERT_DATABASE_URL": url}
    secret_values = (cluster.password, url)
    def cli(label, *args, success=True, override=None):
        return command(report, label, [binary, *args], override or env, cluster.root,
                       success=success, secrets=secret_values)
    cli("help-without-database", "--help", override=cluster.env)
    cli("missing-explicit-database", "--migrate", success=False, override=cluster.env)
    cli("foreign-database-refused", "--migrate", success=False,
        override={**cluster.env, "CONCERT_DATABASE_URL": "postgresql://untrusted@127.0.0.1/postgres"})
    cli("verify-empty-refused", "--verify-migrations", success=False)
    serving = {**env, "CONCERT_RUNTIME_MODE": "local", "CONCERT_METRICS_ADDR": "127.0.0.1:0"}
    cli("external-empty-startup-refused", success=False, override=serving)
    require(cluster.sql(DATABASE, "select count(*) from pg_tables where schemaname='public';") == "0", "no-startup-migration")
    cli("migrate", "--migrate")
    cli("verify", "--verify-migrations")
    manifests = migration.migration_manifest(ROOT / "backend", "concert-service")
    report["migration_sha384"] = manifests
    report["migration_manifest_sha256"] = hashlib.sha256(json.dumps(manifests, sort_keys=True).encode()).hexdigest()
    report["migration_ledger"] = migration.check_ledger(cluster, DATABASE, manifests)
    cli("migrate-reapply", "--migrate")
    cli("verify-reapply", "--verify-migrations")
    require(migration.check_ledger(cluster, DATABASE, manifests) == report["migration_ledger"], "reapply-ledger-stable")

    cluster.assert_owned()
    test_env = {**env, "CONCERT_TEST_DATABASE_URL": url,
                "CONCERT_TEST_OWNER_ROOT": str(cluster.root),
                "CONCERT_TEST_OWNER_TOKEN": cluster.token,
                "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
    tested = command(report, "real-repository-tests", [repository_test, "--test-threads=1"], test_env,
                     cluster.root, secrets=secret_values)
    require(b"3 passed; 0 failed" in tested.stdout, "repository-tests-not-skipped")
    report["real_repository_tests"] = 3

    # Receipts alone do not prove physical constraints. Startup's metadata probe must reject this.
    cluster.sql(DATABASE, "alter table concert_avatar_settings drop constraint concert_avatar_participant;")
    try:
        cli("receipt-only-verify-after-damage", "--verify-migrations")
        cli("external-schema-damage-refused", success=False, override=serving)
    finally:
        cluster.sql(DATABASE, "alter table concert_avatar_settings add constraint concert_avatar_participant foreign key(event_id,user_id) references concert_participants(event_id,user_id) on delete cascade;")
    report["physical_schema_negative"] = "PASS"

    event = str(uuid4())
    cluster.sql(DATABASE, f"insert into concert_events(event_id,slug,title,phase) values('{event}','restart-proof','Owned restart proof','OPEN');")
    service = OwnedService(cluster, binary, serving)
    report["service"] = service  # Removed before serializing; enables cleanup even if a later check fails.
    service.start()
    report["service_pid"] = service.child.pid
    status, value = health(service.address, "/readyz")
    require(status == 200 and value["ready"] and value["checks"]["database"]["status"] == "up", "healthy-readiness")
    report["readiness"] = status
    require(health(service.address, "/api/concert/events")[0] == 404, "no-alternate-product-api")
    before = cluster.assert_postmaster()
    cluster.stop()
    time.sleep(1.1)  # Expire the inherited one-second readiness cache.
    status, value = health(service.address, "/readyz")
    require(status == 503 and not value["ready"], "database-outage-readiness")
    require(health(service.address, "/livez")[0] == 200, "database-outage-liveness")
    report["outage"] = {"readiness": status, "liveness": 200}
    cluster.start()
    require(cluster.assert_postmaster() != before, "postgres-did-not-restart")
    require(cluster.sql(DATABASE, f"select title from concert_events where event_id='{event}';") == "Owned restart proof", "restart-persistence")
    require(migration.check_ledger(cluster, DATABASE, manifests) == report["migration_ledger"], "restart-ledger")
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if health(service.address, "/readyz")[0] == 200:
            break
        time.sleep(0.1)
    else:
        raise CheckFailure("readiness-recovery-timeout")
    report["restart_persistence"] = "PASS"
    report["readiness_recovered"] = True
    require(service.stop(), "graceful-service-stop")
    report["service_exit_code"] = service.child.returncode
    report.pop("service")
    cli("verify-after-restart", "--verify-migrations")
    report["result"] = "PASS"

def run(binary, repository_test, report):
    cluster = None
    try:
        require(binary.is_absolute() and repository_test.is_absolute(), "explicit-binary-paths")
        binary, repository_test = migration.executable(binary), migration.executable(repository_test)
        report["binary_sha256"] = digest(binary)
        report["repository_test_sha256"] = digest(repository_test)
        tools = {name: migration.executable(Path("/opt/homebrew/bin") / name) for name in migration.PG_TOOLS}
        cluster = migration.Cluster(tools)
        report["owned_directory"] = str(cluster.root)
        exercise(cluster, binary, repository_test, report)
    except (Exception, KeyboardInterrupt) as error:
        report["result"] = "FAIL"
        report["failure"] = str(error) if isinstance(error, CheckFailure) else type(error).__name__
    finally:
        service = report.pop("service", None)
        stopped = True
        if service is not None:
            try:
                stopped = service.stop()
            except BaseException:
                stopped = False
        try:
            if cluster is not None:
                cluster.cleanup()
            report["cleanup"] = "PASS" if stopped else "FAIL"
        except BaseException:
            report["cleanup"] = "FAIL"
        if report["cleanup"] != "PASS":
            report["result"] = "FAIL"

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repository-test", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("report must be a new absolute path")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    report = {"scope": "BE05 owned core database and operational service", "result": "FAIL", "commands": {},
              "product_rpc": "NOT_EXPOSED", "auth_policy": "UNAVAILABLE_UNTIL_BE04",
              "production_avatar_catalog": "NOT_RUN", "production": "NOT_RUN", "secrets_recorded": False}
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        report["script_sha256"] = digest(__file__)
        report["migration_helper_sha256"] = digest(migration.__file__)
        files = [path for prefix in ("backend/crates/concert-domain", "backend/crates/concert-application", "backend/services/concert-service")
                 for path in (ROOT / prefix).rglob("*") if path.suffix in (".rs", ".sql", ".toml")]
        files += [ROOT / "backend/Cargo.toml", ROOT / "backend/Cargo.lock"]
        report["source_sha256"] = {str(p.relative_to(ROOT)): digest(p) for p in sorted(files)}
        report["contract_sha256"] = {name: digest(ROOT / name) for name in ("contracts/core/v1/core.schema.json",
             "contracts/assets/v1/avatar-config.schema.json", "contracts/assets/v1/common.schema.json", "contracts/realtime/v1/world.schema.json")}
        report["source_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        run(args.binary, args.repository_test, report)
    except (Exception, KeyboardInterrupt) as error:
        report.update(result="FAIL", failure=type(error).__name__, cleanup="NOT_RUN")
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({key: report.get(key) for key in ("result", "cleanup", "real_repository_tests", "restart_persistence")}))
    return 0 if report["result"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
