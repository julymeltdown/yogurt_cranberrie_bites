#!/usr/bin/env python3
"""BE02 native runtime qualification; only fresh owned resources, never a deployment."""
from __future__ import annotations

import argparse
import base64
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import http.client
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import time
from urllib.parse import urlsplit

import migration_smoke as migration

SmokeError = migration.SmokeError
START_ORDER = ("profile-service", "notification-service", "friend-service", "post-service",
               "auth-service", "api-gateway")
PREFIX = {name: variable.removesuffix("_DATABASE_URL")
          for name, variable in migration.SERVICES.items()}
PEERS = {"api-gateway": ("auth-service", "profile-service", "friend-service", "post-service",
                         "notification-service"),
         "auth-service": ("profile-service", "friend-service", "post-service", "notification-service"),
         "friend-service": ("profile-service",),
         "post-service": ("profile-service", "friend-service", "notification-service", "api-gateway"),
         "profile-service": (), "notification-service": ()}
DEFAULT_PORTS = {3000, 5173, 5174, 5432, 6379, 9092, *range(8080, 8091),
                 *range(9080, 9090), *range(50050, 50056)}
LSOF = Path("/usr/sbin/lsof")


@contextmanager
def protected_signals(*, cleanup=False):
    """Finish ownership registration (or cleanup) before handling termination."""
    pending = []
    def defer(signum, _frame):
        pending.append(signum)
    previous = {sig: signal.signal(sig, signal.SIG_IGN if cleanup else defer)
                for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        yield
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    if pending:
        raise KeyboardInterrupt


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def private_file(path: Path, data: bytes) -> None:
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as stream:
        stream.write(data)


class Ports:
    """Hold exclusive reservations until each child binds; verify its PID before probing."""
    def __init__(self):
        self.ports: dict[str, int] = {}
        self.held: dict[str, socket.socket] = {}
        self.used: set[int] = set()

    def reserve(self, name: str, port: int = 0) -> int:
        if name in self.held or port in DEFAULT_PORTS:
            raise SmokeError("ports", "refusing duplicate reservation or source-default port")
        for _ in range(100):
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            try:
                sock.bind(("127.0.0.1", port))
                chosen = sock.getsockname()[1]
                if chosen in DEFAULT_PORTS or chosen in self.used:
                    sock.close()
                    continue
                self.ports[name], self.held[name] = chosen, sock
                self.used.add(chosen)
                return chosen
            except OSError:
                sock.close()
                raise SmokeError("ports", "loopback reservation collision; no attach or reuse") from None
        raise SmokeError("ports", "cannot allocate nondefault port")

    def release(self, name: str) -> None:
        self.held.pop(name).close()

    def close(self) -> None:
        for sock in self.held.values():
            sock.close()
        self.held.clear()

    def address(self, name: str) -> str:
        return f"127.0.0.1:{self.ports[name]}"

    def url(self, name: str) -> str:
        return "http://" + self.address(name)


def listener_addresses(output: str, pid: int) -> set[str]:
    """Parse lsof's explicit PID-filtered field output, never a global process scan."""
    seen_pid = None
    addresses = set()
    for line in output.splitlines():
        if line.startswith("p"):
            seen_pid = int(line[1:])
            if seen_pid != pid:
                raise SmokeError("ownership", "listener PID does not match child")
        elif line.startswith("n"):
            if seen_pid != pid:
                raise SmokeError("ownership", "listener has no owned PID")
            address = line[1:]
            if not address.startswith("127.0.0.1:"):
                raise SmokeError("ownership", "child bound a non-loopback listener")
            addresses.add(address)
    return addresses


class Child:
    def __init__(self, cluster, name, argv, env, ports, reservations, secrets_to_redact=()):
        self.cluster, self.name, self.ports = cluster, name, ports
        self.reservations = reservations
        self.log = cluster.root / f"{name}-{secrets.token_hex(4)}.log"
        self.secrets = tuple(secrets_to_redact)
        self.process = None
        cluster.assert_owned()
        for key in reservations:
            if key not in ports.held:
                raise SmokeError("ports", "child launch requires owned port reservations")
        # No shell, daemon, inherited environment, user cwd, or process-group kill.
        with os.fdopen(os.open(self.log, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as log:
            for key in reservations:
                ports.release(key)
            self.process = subprocess.Popen(argv, cwd=cluster.root, env=env, stdin=subprocess.DEVNULL,
                                            stdout=log, stderr=subprocess.STDOUT, close_fds=True)

    def alive(self):
        if self.process.poll() is not None:
            raise SmokeError(f"{self.name}:startup", self.diagnostic(), self.process.returncode)

    def diagnostic(self):
        self.cluster.assert_owned()
        if self.log.is_symlink():
            return "refusing replaced child log"
        with self.log.open("rb") as stream:
            stream.seek(max(0, self.log.stat().st_size - 8192))
            content = stream.read().decode("utf-8", errors="replace")
        return migration.redact(content, self.secrets)

    def listeners(self) -> bool:
        self.cluster.assert_owned()
        self.alive()
        result = migration.execute([str(LSOF), "-nP", "-a", "-p", str(self.process.pid),
                                    "-iTCP", "-sTCP:LISTEN", "-FpFn"], env=self.cluster.env,
                                   cwd=self.cluster.root, phase="listener-ownership", timeout=3,
                                   allowed=(0, 1))
        actual = listener_addresses(result.stdout, self.process.pid)
        expected = {self.ports.address(key) for key in self.reservations}
        if actual - expected:
            raise SmokeError("ownership", "child bound an unexpected listener")
        self.alive()
        return actual == expected

    def wait_listeners(self, timeout=20):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.listeners():
                return
            time.sleep(0.1)
        raise SmokeError(f"{self.name}:listeners", "owned listeners did not become available: " + self.diagnostic())

    def stop(self) -> dict:
        self.cluster.assert_owned()
        result = {"pid": self.process.pid, "forced": False}
        if self.process.poll() is not None:
            result.update(exit_code=self.process.returncode, unexpected_exit=True)
            return result
        self.process.terminate()  # Direct unreaped child handle; never a discovered/reused PID.
        try:
            self.process.wait(timeout=12)
        except subprocess.TimeoutExpired:
            result["forced"] = True
            self.cluster.assert_owned()
            self.process.kill()
            self.process.wait(timeout=5)
        result["exit_code"] = self.process.returncode
        return result


def generated_keys(cluster, openssl):
    keys = {}
    rsa_path = cluster.root / "jwt.pem"
    private_file(rsa_path, b"")
    migration.execute([str(openssl), "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048",
                       "-out", str(rsa_path)], env={**cluster.env, "OPENSSL_CONF": "/dev/null"},
                      cwd=cluster.root, phase="jwt-key")
    keys["AUTH_JWT_PRIVATE_KEY_PEM"] = rsa_path.read_text()
    rsa_path.unlink()
    for label in ("ACTOR_DELEGATION", "RESOURCE_EVIDENCE"):
        seed = secrets.token_bytes(32)
        private_path, public_path = cluster.root / f"{label}.der", cluster.root / f"{label}.pub"
        # RFC 8410 PKCS#8 Ed25519 seed / SubjectPublicKeyInfo, generated for this run.
        private_file(private_path, bytes.fromhex("302e020100300506032b657004220420") + seed)
        private_file(public_path, b"")
        migration.execute([str(openssl), "pkey", "-inform", "DER", "-in", str(private_path),
                           "-pubout", "-outform", "DER", "-out", str(public_path)],
                          env={**cluster.env, "OPENSSL_CONF": "/dev/null"},
                          cwd=cluster.root, phase="delegation-key")
        public = public_path.read_bytes()
        if len(public) != 44 or public[:12] != bytes.fromhex("302a300506032b6570032100"):
            raise SmokeError("keys", "unexpected Ed25519 public encoding")
        keys[label + "_SIGNING_SEED"] = base64.b64encode(seed).decode()
        keys[label + "_VERIFYING_KEYS"] = base64.b64encode(public[12:]).decode()
        private_path.unlink()
        public_path.unlink()
    return keys


def environments(cluster, ports, keys, credentials, redis_password):
    result = {}
    for service, prefix in PREFIX.items():
        db = "mcr_" + service.removesuffix("-service").replace("-", "_")
        env = {**cluster.env, **credentials,
               "ACTOR_DELEGATION_VERIFYING_KEYS": keys["ACTOR_DELEGATION_VERIFYING_KEYS"],
               f"{prefix}_RUNTIME_MODE": "parity", f"{prefix}_DATABASE_URL": cluster.url(db),
               f"{prefix}_DATABASE_MIGRATION_MODE": "external", f"{prefix}_SEED_ENABLED": "false",
               "RUST_LOG": "warn", "TOKIO_WORKER_THREADS": "2"}
        for kind in ("http", "metrics", "grpc"):
            env[f"{prefix}_{kind.upper()}_ADDR"] = ports.address(f"{service}:{kind}")
        for peer in PEERS[service]:
            env[f"{prefix}_{PREFIX[peer]}_GRPC_ENDPOINT"] = ports.url(f"{peer}:grpc")
        result[service] = env
    result["auth-service"].update({
        "AUTH_JWT_PRIVATE_KEY_PEM": keys["AUTH_JWT_PRIVATE_KEY_PEM"], "AUTH_JWT_KEY_ID": "mcr-runtime-jwt",
        "AUTH_ISSUER": "mcr-runtime", "AUTH_ACCESS_AUDIENCE": "mcr-runtime-api",
        "AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED": "false", "AUTH_PASSWORD_RESET_MAILER_MODE": "logging",
        "AUTH_PASSWORD_RESET_BASE_URL": ports.url("disabled-egress") + "/reset"})
    for provider in ("GOOGLE", "APPLE", "X"):
        result["auth-service"].update({f"AUTH_{provider}_{suffix}": ports.url("disabled-egress") + "/disabled"
            for suffix in ("AUTHORIZE_BASE_URI", "TOKEN_URI", "USERINFO_URI", "JWKS_URI", "ISSUER")})
        result["auth-service"][f"AUTH_{provider}_REDIRECT_URI"] = ports.url("api-gateway:http") + "/unused-callback"
    result["post-service"].update({
        "POST_REDIS_URL": f"redis://:{redis_password}@{ports.address('redis')}/0",
        "RESOURCE_EVIDENCE_SIGNING_SEED": keys["RESOURCE_EVIDENCE_SIGNING_SEED"],
        "POST_EXTERNAL_PUBLISHER": "disabled", "POST_KAFKA_BOOTSTRAP_BROKERS": ""})
    result["api-gateway"].update({
        "ACTOR_DELEGATION_SIGNING_SEED": keys["ACTOR_DELEGATION_SIGNING_SEED"],
        "RESOURCE_EVIDENCE_VERIFYING_KEYS": keys["RESOURCE_EVIDENCE_VERIFYING_KEYS"],
        "GATEWAY_JWT_ISSUER": "mcr-runtime", "GATEWAY_JWT_AUDIENCE": "mcr-runtime-api",
        "GATEWAY_JWT_KEYS_REFRESH_SECONDS": "5", "GATEWAY_CORS_ALLOWED_ORIGINS": ports.url("disabled-egress"),
        "GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS": "5",
        "GATEWAY_ADMISSION_HMAC_KEY": secrets.token_hex(32),
        "GATEWAY_SEARCH_CURSOR_HMAC_KEY": secrets.token_hex(32),
        "GATEWAY_REFRESH_COOKIE_SECURE": "true"})
    # Keep inherited background workers enabled: fresh databases have no user workload.
    # Mail delivery destinations/credentials, OTLP exporters and provider credentials are absent.
    return result


def validate_environments(cluster, ports, envs):
    for service, env in envs.items():
        prefix = PREFIX[service]
        database = "mcr_" + service.removesuffix("-service").replace("-", "_")
        cluster.validate_url(env[migration.SERVICES[service]], database)
        if "HOME" in env or env.get("TMPDIR") != str(cluster.root):
            raise SmokeError("environment", "ambient home or foreign temporary directory")
        if env.get(prefix + "_RUNTIME_MODE") != "parity" or env.get(prefix + "_SEED_ENABLED") != "false":
            raise SmokeError("environment", "runtime mode/seeding does not match bounded smoke")
        if env.get(prefix + "_DATABASE_MIGRATION_MODE") != "external":
            raise SmokeError("environment", "normal service must verify owner-applied schema")
        for kind in ("http", "metrics", "grpc"):
            if env.get(f"{prefix}_{kind.upper()}_ADDR") != ports.address(f"{service}:{kind}"):
                raise SmokeError("environment", "foreign/default listener")
        for peer in PEERS[service]:
            if env.get(f"{prefix}_{PREFIX[peer]}_GRPC_ENDPOINT") != ports.url(f"{peer}:grpc"):
                raise SmokeError("environment", "foreign/default dependency endpoint")
        for name, value in env.items():
            if name.endswith("DATABASE_URL") and name != migration.SERVICES[service]:
                raise SmokeError("environment", "unexpected database URL")
            if name.startswith(("OTEL_", "AWS_")) or name.upper().endswith("PROXY") or "MAILER_ENDPOINT" in name or "MAILER_BEARER" in name:
                raise SmokeError("environment", "ambient/external connector setting")
            if value.startswith(("http://", "https://")):
                parsed = urlsplit(value)
                if parsed.scheme != "http" or parsed.hostname != "127.0.0.1" or parsed.port not in ports.ports.values():
                    raise SmokeError("environment", "non-owned HTTP endpoint")
    redis = urlsplit(envs["post-service"]["POST_REDIS_URL"])
    if redis.scheme != "redis" or redis.hostname != "127.0.0.1" or redis.port != ports.ports["redis"] or not redis.password or redis.path != "/0":
        raise SmokeError("environment", "foreign/default Redis endpoint")
    if envs["post-service"]["POST_EXTERNAL_PUBLISHER"] != "disabled" or envs["post-service"]["POST_KAFKA_BOOTSTRAP_BROKERS"]:
        raise SmokeError("environment", "external publisher must stay disabled")


def http_probe(child, kind, path):
    if not child.listeners():
        raise SmokeError("probe", "cannot probe a listener without child ownership")
    connection = http.client.HTTPConnection("127.0.0.1", child.ports.ports[f"{child.name}:{kind}"], timeout=2)
    try:
        connection.request("GET", path, headers={"Connection": "close"})
        response = connection.getresponse()
        body = response.read(16385)
        if len(body) > 16384:
            raise SmokeError("probe", "oversized health response")
        return response.status, body
    finally:
        connection.close()


def readiness_value(status, body, service):
    try:
        value = json.loads(body)
        expected = {"database": True, **{
            f"{PREFIX[service].lower()}->{PREFIX[peer].lower()}":
            service != "auth-service" and not (service == "post-service" and peer in ("profile-service", "notification-service"))
            for peer in PEERS[service]}}
        if service == "api-gateway":
            expected["signing-keys"] = True
        if not all(type(value[k]) is bool for k in ("ready", "degraded", "draining")):
            raise ValueError
        if not isinstance(value["checks"], dict) or set(value["checks"]) != set(expected):
            raise ValueError
        for name, check in value["checks"].items():
            if (type(check["required"]) is not bool or check["required"] != expected[name]
                    or check["status"] not in ("up", "down", "timed_out", "not_checked")):
                raise ValueError
        computed_ready = not value["draining"] and all(not c["required"] or c["status"] == "up" for c in value["checks"].values())
        if (value["ready"] != computed_ready
                or value["degraded"] != any(c["status"] != "up" for c in value["checks"].values())
                or status != (200 if value["ready"] else 503)):
            raise ValueError
        # Copy only the known safe shape, never arbitrary server response fields.
        return {"http_status": status, **{k: value[k] for k in ("ready", "degraded", "draining")},
                "checks": {name: {"required": c["required"], "status": c["status"]} for name, c in value["checks"].items()}}
    except (ValueError, KeyError, TypeError):
        raise SmokeError("readiness", "malformed or inconsistent readiness report") from None


def wait_ready(children, rows, phase, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        complete = True
        for service, child in children.items():
            status, body = http_probe(child, "metrics", "/readyz")
            value = readiness_value(status, body, service)
            rows[service][phase] = value
            complete &= value["ready"] and not value["degraded"]
        if complete:
            return
        time.sleep(0.25)
    raise SmokeError("readiness", "not every service/dependency reached non-degraded readiness")


def redis_ping(child, password):
    if not child.listeners():
        raise SmokeError("redis", "Redis listener not owned")
    def command(*parts):
        return (f"*{len(parts)}\r\n" + "".join(f"${len(p)}\r\n{p}\r\n" for p in parts)).encode()
    with socket.create_connection(("127.0.0.1", child.ports.ports["redis"]), timeout=2) as connection:
        connection.sendall(command("AUTH", password) + command("PING"))
        with connection.makefile("rb") as stream:
            if stream.readline(512) != b"+OK\r\n" or stream.readline(512) != b"+PONG\r\n":
                raise SmokeError("redis", "owned authenticated Redis PING failed")


def run(bin_dir, pg_bin_dir, redis_binary, openssl_binary):
    report = {"scope": "BE02 native six-service startup/health/restart only", "result": "FAIL",
              "started_at_utc": datetime.now(timezone.utc).isoformat(), "cleanup": "NOT_RUN",
              "script_sha256": digest(Path(__file__)), "migration_helper_sha256": digest(Path(migration.__file__)),
              "services": [], "process_stops": [], "auth_journeys": "NOT_RUN", "providers": "NOT_RUN",
              "packaging": "NOT_RUN", "BE44": "NOT_RUN"}
    cluster, ports, children, redis_child = None, Ports(), {}, None
    secret_values = ()
    try:
        binaries, pg_tools, manifests = migration.preflight(bin_dir, pg_bin_dir)
        if not redis_binary.is_absolute() or not openssl_binary.is_absolute():
            raise SmokeError("preflight", "dependency executables must be absolute")
        redis_binary, openssl_binary = migration.executable(redis_binary), migration.executable(openssl_binary)
        migration.executable(LSOF)
        report["source_revision"] = migration.execute(["/usr/bin/git", "rev-parse", "HEAD"],
            env={"PATH": "/usr/bin:/bin"}, cwd=migration.BACKEND, phase="revision").stdout.strip()
        report["binary_directory"] = str(bin_dir)
        report["tools"] = {**{k: str(v) for k, v in pg_tools.items()}, "redis": str(redis_binary), "openssl": str(openssl_binary)}
        with protected_signals():
            cluster = migration.Cluster(pg_tools)
            report["owned_directory"] = str(cluster.root)
        # Reuse the qualified exact-ledger and PG restart flow, without changing the helper.
        # pg_ctl forks a postmaster. Defer interruption until this bounded helper
        # returns, so cleanup cannot race an in-flight start or stop.
        with protected_signals():
            migration.qualify(cluster, binaries, manifests, report)
        report["migration_pg_restart"] = report.pop("restart")
        rows = {row["service"]: row for row in report["services"]}
        for service in START_ORDER:
            for kind in ("http", "metrics", "grpc"):
                ports.reserve(f"{service}:{kind}")
        ports.reserve("redis")
        ports.reserve("disabled-egress")  # Bound but not listening; never an external destination.
        keys = generated_keys(cluster, openssl_binary)
        credentials = {prefix + "_SERVICE_CREDENTIAL": secrets.token_hex(32) for prefix in PREFIX.values()}
        redis_password = secrets.token_hex(32)
        envs = environments(cluster, ports, keys, credentials, redis_password)
        validate_environments(cluster, ports, envs)
        secret_values = (cluster.password, redis_password, *credentials.values(), *keys.values(),
                         *(v for env in envs.values() for k, v in env.items() if "HMAC_KEY" in k))
        report["configuration"] = {"runtime_mode": "parity", "migration_mode": "external", "home": "OMITTED",
            "cwd": "owned temporary cluster root", "seed_enabled": False, "kafka": "DISABLED",
            "mail_destinations": "UNSET", "otlp": "UNSET", "oauth_adapter": "inherited parity; NOT_EXERCISED",
            "ports": ports.ports.copy()}
        redis_config = cluster.root / "redis.conf"
        private_file(redis_config, (f"bind 127.0.0.1\nport {ports.ports['redis']}\nprotected-mode yes\n"
            f"requirepass {redis_password}\ndir {cluster.root}\ndaemonize no\nsave \"\"\nappendonly no\n").encode())
        with protected_signals():
            redis_child = Child(cluster, "redis", [str(redis_binary), str(redis_config)], cluster.env, ports, ("redis",), secret_values)
        redis_child.wait_listeners()
        redis_ping(redis_child, redis_password)
        report["redis"] = {"pid": redis_child.process.pid, "authenticated_ping": "PASS", "binary_sha256": digest(redis_binary)}

        def start_services(phase):
            for service in START_ORDER:
                # Verify every running dependency's listener ownership before adding another caller.
                if not redis_child.listeners():
                    raise SmokeError("redis", "owned listener disappeared")
                for prior in children.values():
                    if not prior.listeners():
                        raise SmokeError("ownership", "prior service lost a listener")
                with protected_signals():
                    child = Child(cluster, service, [str(binaries[service])], envs[service], ports,
                                  tuple(f"{service}:{kind}" for kind in ("http", "metrics", "grpc")), secret_values)
                    children[service] = child
                rows[service][phase + "_pid"] = child.process.pid
                child.wait_listeners()
                rows[service][phase + "_listeners"] = "PASS_OWNED"
                live_status, _ = http_probe(child, "metrics", "/livez")
                rows[service][phase + "_livez_status"] = live_status
                if live_status != 200:
                    raise SmokeError("liveness", "metrics liveness failed")
                public_status, _ = http_probe(child, "http", "/livez")
                rows[service][phase + "_public_livez"] = {"http_status": public_status,
                    "qualification": "listener response only; no readiness inference"}
            wait_ready(children, rows, phase + "_readiness")

        def stop_services():
            for service in reversed(START_ORDER):
                if service in children:
                    with protected_signals():
                        stopped = children[service].stop()
                        report["process_stops"].append({"service": service, **stopped})
                        del children[service]
                    if stopped["forced"] or stopped.get("unexpected_exit") or stopped["exit_code"] != 0:
                        raise SmokeError("service-stop", "service did not exit cleanly")

        start_services("initial")
        # A new record after actual services are running, separate from migration-only evidence.
        for service, row in rows.items():
            token = secrets.token_hex(24)
            cluster.sql(row["database"], f"create table mcr_smoke.runtime_records (token text primary key); "
                        f"insert into mcr_smoke.runtime_records values ('{token}');")
            row["_runtime_token"] = token
        # Real outage: liveness must remain 200 while the required database probe fails.
        before_pid = cluster.assert_postmaster()
        with protected_signals():
            cluster.stop()
        time.sleep(1.1)  # Expire platform's one-second readiness cache.
        for service, child in children.items():
            live_status, _ = http_probe(child, "metrics", "/livez")
            status, body = http_probe(child, "metrics", "/readyz")
            value = readiness_value(status, body, service)
            rows[service]["database_outage"] = {"livez_status": live_status, "readiness": value}
            if live_status != 200 or value["ready"] or value["checks"]["database"]["status"] == "up":
                raise SmokeError("database-outage", "outage did not separate liveness and database readiness")
        with protected_signals():
            cluster.start()
        after_pid = cluster.assert_postmaster()
        if before_pid == after_pid:
            raise SmokeError("restart", "PostgreSQL PID did not change")
        report["runtime_pg_restart"] = {"before_pid": before_pid, "after_pid": after_pid, "status": "PASS"}
        wait_ready(children, rows, "database_recovered_readiness")
        # Stop one real peer with DBs available: required edges must fail closed,
        # while optional profile enrichment/deletion edges report degradation.
        with protected_signals():
            stopped = children["profile-service"].stop()
            report["process_stops"].append({"service": "profile-service", **stopped})
            del children["profile-service"]
        if stopped["forced"] or stopped.get("unexpected_exit") or stopped["exit_code"] != 0:
            raise SmokeError("service-stop", "profile outage did not stop cleanly")
        time.sleep(1.1)
        for service, child in children.items():
            if "profile-service" not in PEERS[service]:
                continue
            live_status, _ = http_probe(child, "metrics", "/livez")
            status, body = http_probe(child, "metrics", "/readyz")
            value = readiness_value(status, body, service)
            rows[service]["profile_outage"] = {"livez_status": live_status, "readiness": value}
            edge = value["checks"][PREFIX[service].lower() + "->profile"]
            if (live_status != 200 or edge["status"] == "up" or not value["degraded"]
                    or value["checks"]["database"]["status"] != "up"
                    or value["ready"] != (not edge["required"])):
                raise SmokeError("profile-outage", "peer outage did not respect required/optional readiness")
        stop_services()
        # Fresh topology avoids rebinding sockets in TIME_WAIT without relaxing
        # exclusive reservation. The restarted peers receive new explicit URLs.
        for service in START_ORDER:
            for kind in ("http", "metrics", "grpc"):
                name = f"{service}:{kind}"
                ports.reserve(name)
        envs = environments(cluster, ports, keys, credentials, redis_password)
        validate_environments(cluster, ports, envs)
        secret_values = (*secret_values, *(v for env in envs.values() for k, v in env.items() if "HMAC_KEY" in k))
        report["configuration"]["restarted_ports"] = ports.ports.copy()
        start_services("restarted")
        for service, row in rows.items():
            if row["initial_pid"] == row["restarted_pid"]:
                raise SmokeError("restart", "service PID did not change")
            if cluster.sql(row["database"], "select token from mcr_smoke.runtime_records;") != row["_runtime_token"]:
                raise SmokeError("persistence", "runtime smoke record lost or changed")
            row["runtime_ledger_after"] = migration.check_ledger(cluster, row["database"], manifests[service])
            row["runtime_record_persistence"] = "PASS"
        redis_ping(redis_child, redis_password)
        stop_services()
        report["result"] = "PASS"
    except SmokeError as error:
        report["failure"] = {**error.detail, "diagnostic": migration.redact(error.detail["diagnostic"], secret_values)}
    except (KeyboardInterrupt, Exception) as error:
        report["failure"] = {"phase": "harness", "diagnostic": type(error).__name__}
    finally:
        with protected_signals(cleanup=True):
            cleanup_resources(cluster, ports, children, redis_child, report)
        for row in report["services"]:
            row.pop("_token", None)
            row.pop("_runtime_token", None)
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    return report


def cleanup_resources(cluster, ports, children, redis_child, report):
    cleanup_errors = []
    # Attempt every child, even if one fails; never remove data under an unconfirmed process.
    for name, child in [*reversed(list(children.items())), *(([("redis", redis_child)]) if redis_child else [])]:
        try:
            stopped = child.stop()
            report["process_stops"].append({"service": name, **stopped})
            if stopped["forced"] or stopped.get("unexpected_exit") or stopped["exit_code"] != 0:
                report["result"] = "FAIL"
        except Exception as error:
            cleanup_errors.append({"service": name, "error": type(error).__name__})
    ports.close()
    if cluster:
        if cleanup_errors:
            # Still stop the owned database, but retain files if any child may
            # be running. A stop failure must not prevent other stop attempts.
            try:
                cluster.stop()
            except Exception as error:
                cleanup_errors.append({"service": "postgres", "error": type(error).__name__})
        else:
            try:
                cluster.cleanup()
                report["cleanup"] = "PASS"
            except Exception as error:
                cleanup_errors.append({"service": "postgres", "error": type(error).__name__})
        if cleanup_errors:
            report.update(cleanup="FAIL", result="FAIL", cleanup_errors=cleanup_errors,
                          retained_owned_directory=str(cluster.root))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", required=True, type=Path)
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/bin"))
    parser.add_argument("--redis-binary", type=Path, default=Path("/opt/homebrew/bin/redis-server"))
    parser.add_argument("--openssl-binary", type=Path, default=Path("/opt/homebrew/bin/openssl"))
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("--report requires an absolute new file path")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("--report must be a new writable file in an existing directory")
    def interrupted(*_):
        raise KeyboardInterrupt
    previous = {sig: signal.signal(sig, interrupted) for sig in (signal.SIGTERM, signal.SIGINT)}
    try:
        with os.fdopen(fd, "w") as stream:
            report = run(args.bin_dir, args.pg_bin_dir, args.redis_binary, args.openssl_binary)
            with protected_signals(cleanup=True):
                json.dump(report, stream, indent=2)
                stream.write("\n")
                stream.flush()
                os.fsync(stream.fileno())
        print(json.dumps({"result": report["result"], "cleanup": report["cleanup"], "report": str(args.report)}))
        return 0 if report["result"] == "PASS" else 1
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


if __name__ == "__main__":
    raise SystemExit(main())
