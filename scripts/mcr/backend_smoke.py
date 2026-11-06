#!/usr/bin/env python3
"""BE44 real-stack smoke: compose first, native seven-service fallback second.

Both modes prove the same contract: fresh databases, every service live, an
all-required-peers-ready gateway, one public route, a peer outage that removes
readiness, a restart that preserves a real row, and complete teardown.
"""
from __future__ import annotations

import argparse
import base64
import http.client
import json
import os
from pathlib import Path
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
BACKEND = ROOT / "backend"
sys.path.insert(0, str(BACKEND / "scripts" / "mcr"))

import migration_smoke as migration  # noqa: E402
import runtime_smoke as runtime  # noqa: E402

SERVICES = ("api-gateway", "auth-service", "profile-service", "friend-service",
            "post-service", "notification-service", "concert-service")
START_ORDER = ("profile-service", "notification-service", "friend-service", "post-service",
               "auth-service", "concert-service", "api-gateway")
COMPOSE_FILE = ROOT / "infra" / "mcr" / "compose.yml"
SMOKE_DB = "mcr_post"  # owned by the stack; only a private mcr_smoke schema is added


def log(message):
    print(message, flush=True)


def run(argv, *, env=None, cwd=ROOT, check=True, timeout=1800):
    result = subprocess.run(argv, env=env, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    if check and result.returncode != 0:
        raise SmokeFailure("command", f"{' '.join(argv[:3])} exited {result.returncode}: "
                           f"{(result.stderr or result.stdout)[-800:]}")
    return result


class SmokeFailure(Exception):
    def __init__(self, phase, diagnostic):
        self.phase, self.diagnostic = phase, diagnostic
        super().__init__(phase)


def probe(port, path, *, method="GET", body=None, headers=None, timeout=5):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
    try:
        connection.request(method, path, body=body, headers=headers or {"Connection": "close"})
        response = connection.getresponse()
        return response.status, response.read(65536)
    finally:
        connection.close()


def wait_status(port, path, expected, *, timeout=180, label=""):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            status, body = probe(port, path, timeout=2)
            last = (status, body)
            if status == expected:
                return status, body
        except OSError:
            pass
        time.sleep(0.25)
    raise SmokeFailure("wait", f"{label} {path} on {port} never returned {expected}: {last}")


def json_or_fail(body, phase):
    try:
        return json.loads(body)
    except ValueError:
        raise SmokeFailure(phase, "response is not JSON") from None


def all_required_up(report):
    return (report.get("ready") is True and report.get("degraded") is False
            and isinstance(report.get("checks"), dict)
            and all(c.get("status") == "up" for c in report["checks"].values() if c.get("required")))


CONCERT_AUTHORITY_SERVICES = ("auth-service", "friend-service")


def expected_checks(service):
    """Required readiness probes owned by each service at the smoke's source revision."""
    expected = {"database": True}
    for peer in runtime.PEERS[service]:
        edge = f"{runtime.PREFIX[service].lower()}->{runtime.PREFIX[peer].lower()}"
        expected[edge] = service != "auth-service" and not (
            service == "post-service" and peer in ("profile-service", "notification-service"))
    if service == "api-gateway":
        expected["signing-keys"] = True
    if service in CONCERT_AUTHORITY_SERVICES:
        expected["concert-owner-authority"] = True
    return expected


def readiness_value(status, body, service):
    """Validate the full readiness contract, not just that the report looks green."""
    try:
        value = json.loads(body)
        if not all(type(value[k]) is bool for k in ("ready", "degraded", "draining")):
            raise ValueError
        expected = expected_checks(service)
        if not isinstance(value["checks"], dict) or set(value["checks"]) != set(expected):
            raise ValueError
        for name, check in value["checks"].items():
            if (type(check["required"]) is not bool or check["required"] != expected[name]
                    or check["status"] not in ("up", "down", "timed_out", "not_checked")):
                raise ValueError
        computed_ready = (not value["draining"]
                          and all(not c["required"] or c["status"] == "up" for c in value["checks"].values()))
        if (value["ready"] != computed_ready
                or value["degraded"] != any(c["status"] != "up" for c in value["checks"].values())
                or status != (200 if value["ready"] else 503)):
            raise ValueError
        return {"http_status": status, **{k: value[k] for k in ("ready", "degraded", "draining")},
                "checks": {name: {"required": c["required"], "status": c["status"]}
                           for name, c in value["checks"].items()}}
    except (ValueError, KeyError, TypeError):
        raise SmokeFailure("readiness", f"{service} readiness report is malformed or inconsistent") from None


def wait_ready(children, rows, phase, *, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        complete = True
        for service, child in children.items():
            status, body = runtime.http_probe(child, "metrics", "/readyz")
            value = readiness_value(status, body, service)
            rows[service][phase] = value
            complete &= value["ready"] and not value["degraded"]
        if complete:
            return
        time.sleep(0.25)
    raise SmokeFailure("readiness", "not every service/dependency reached non-degraded readiness")


def public_route(port, report):
    event = str(uuid.uuid4())
    status, body = probe(port, f"/api/concert/events/{event}/bootstrap")
    value = json_or_fail(body, "public-route")
    if status != 401 or value.get("code") != "GATEWAY_401":
        raise SmokeFailure("public-route", f"expected unauthenticated GATEWAY_401, got {status} {body[:300]!r}")
    report["public_route"] = {"method": "GET", "path": "/api/concert/events/{event}/bootstrap",
                              "http_status": status, "body": value,
                              "scope": "unauthenticated 401 envelope; account provisioning is out of scope"}
    return value


def free_ports(count):
    sockets, ports = [], []
    for _ in range(count):
        sock = socket.socket()
        sock.bind(("127.0.0.1", 0))
        sockets.append(sock)
        ports.append(sock.getsockname()[1])
    for sock in sockets:
        sock.close()
    return ports


# ---------------------------------------------------------------- compose mode

def openssl_keys(directory):
    pem = directory / "jwt.pem"
    run(["openssl", "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048",
         "-out", str(pem)], env={**os.environ, "OPENSSL_CONF": "/dev/null"}, cwd=directory)
    keys = {"AUTH_JWT_PRIVATE_KEY_PEM": pem.read_text()}
    for label in ("ACTOR_DELEGATION", "RESOURCE_EVIDENCE"):
        seed = secrets.token_bytes(32)
        der, pub = directory / f"{label}.der", directory / f"{label}.pub"
        der.write_bytes(bytes.fromhex("302e020100300506032b657004220420") + seed)
        run(["openssl", "pkey", "-inform", "DER", "-in", str(der), "-pubout", "-outform", "DER",
             "-out", str(pub)], env={**os.environ, "OPENSSL_CONF": "/dev/null"}, cwd=directory)
        keys[label + "_SIGNING_SEED"] = base64.b64encode(seed).decode()
        keys[label + "_VERIFYING_KEYS"] = base64.b64encode(pub.read_bytes()[12:]).decode()
        der.unlink()
        pub.unlink()
    pem.unlink()
    return keys


def compose_env(directory, gateway_port):
    keys = openssl_keys(directory)
    credentials = {prefix + "_SERVICE_CREDENTIAL": secrets.token_hex(32) for prefix in runtime.PREFIX.values()}
    credentials["CONCERT_SERVICE_CREDENTIAL"] = secrets.token_hex(32)
    db_password, redis_password = secrets.token_hex(24), secrets.token_hex(24)
    databases = {prefix: "mcr_" + name.removesuffix("-service").replace("-", "_")
                 for name, prefix in {**runtime.PREFIX, "concert-service": "CONCERT"}.items()}
    # The shared migration runner reads every owned URL from this one private file.
    urls = {f"{prefix}_DATABASE_URL":
            f"postgresql://mcr_owner:{db_password}@postgres:5432/{database}?sslmode=disable"
            for prefix, database in databases.items()}
    urls["POST_REDIS_URL"] = f"redis://:{redis_password}@redis:6379/0"
    values = {
        "MCR_DB_PASSWORD": db_password,
        "MCR_REDIS_PASSWORD": redis_password,
        **urls,
        "AUTH_JWT_PRIVATE_KEY_PEM_B64": base64.b64encode(keys["AUTH_JWT_PRIVATE_KEY_PEM"].encode()).decode(),
        "AUTH_JWT_KEY_ID": "mcr-compose-jwt",
        "AUTH_ISSUER": "mcr-compose",
        "AUTH_ACCESS_AUDIENCE": "mcr-compose-api",
        "AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED": "false",
        "AUTH_PASSWORD_RESET_MAILER_MODE": "logging",
        "AUTH_PASSWORD_RESET_BASE_URL": "http://127.0.0.1:9/reset",
        "AUTH_GOOGLE_REDIRECT_URI": f"http://127.0.0.1:{gateway_port}/unused-callback",
        "AUTH_APPLE_REDIRECT_URI": f"http://127.0.0.1:{gateway_port}/unused-callback",
        "AUTH_X_REDIRECT_URI": f"http://127.0.0.1:{gateway_port}/unused-callback",
        # Required by the shared migration runner before the service-specific env applies.
        "CONCERT_METRICS_ADDR": "0.0.0.0:9096",
        "ACTOR_DELEGATION_SIGNING_SEED": keys["ACTOR_DELEGATION_SIGNING_SEED"],
        "ACTOR_DELEGATION_VERIFYING_KEYS": keys["ACTOR_DELEGATION_VERIFYING_KEYS"],
        "RESOURCE_EVIDENCE_SIGNING_SEED": keys["RESOURCE_EVIDENCE_SIGNING_SEED"],
        "RESOURCE_EVIDENCE_VERIFYING_KEYS": keys["RESOURCE_EVIDENCE_VERIFYING_KEYS"],
        "GATEWAY_ADMISSION_HMAC_KEY": secrets.token_hex(32),
        "GATEWAY_SEARCH_CURSOR_HMAC_KEY": secrets.token_hex(32),
        "GATEWAY_JWT_ISSUER": "mcr-compose",
        "GATEWAY_JWT_AUDIENCE": "mcr-compose-api",
        "GATEWAY_JWT_KEYS_REFRESH_SECONDS": "5",
        "GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS": "5",
        "GATEWAY_CORS_ALLOWED_ORIGINS": f"http://127.0.0.1:{gateway_port}",
        "GATEWAY_REFRESH_COOKIE_SECURE": "false",
        "RUST_LOG": "warn",
        "TOKIO_WORKER_THREADS": "2",
        **credentials,
    }
    for provider in ("GOOGLE", "APPLE", "X"):
        for suffix in ("AUTHORIZE_BASE_URI", "TOKEN_URI", "USERINFO_URI", "JWKS_URI", "ISSUER"):
            values[f"AUTH_{provider}_{suffix}"] = "http://127.0.0.1:9/disabled"
    env_file = directory / "stack.env"
    env_file.write_text("".join(f"{key}={value}\n" for key, value in sorted(values.items())))
    env_file.chmod(0o600)
    return env_file, values


def compose_project():
    return "mcr-smoke-" + uuid.uuid4().hex[:12]


def compose_cmd(project, args, extra_env):
    return ["docker", "compose", "-p", project, "-f", str(COMPOSE_FILE), *args]


def compose_leftovers(project):
    kinds = {
        "containers": ["docker", "ps", "-aq", "--filter", f"label=com.docker.compose.project={project}"],
        "networks": ["docker", "network", "ls", "-q", "--filter", f"label=com.docker.compose.project={project}"],
        "volumes": ["docker", "volume", "ls", "-q", "--filter", f"label=com.docker.compose.project={project}"],
    }
    found = {}
    for name, argv in kinds.items():
        result = run(argv, check=False)
        found[name] = [line for line in result.stdout.split() if line]
    return found


def compose_mode(args):
    if run(["docker", "info"], check=False).returncode != 0:
        raise SmokeFailure("docker", "docker daemon is unavailable; compose execution is NOT_RUN")
    directory = Path(tempfile.mkdtemp(prefix="mcr-compose-", dir="/tmp"))
    project = compose_project()
    report = {"mode": "compose", "result": "FAIL", "project": project, "cleanup": "NOT_RUN",
              "services": list(SERVICES), "started_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    loopback = [8080, 9090, 9091, 9092, 9093, 9094, 9095, 9096, 5432, 6379]
    try:
        gateway_port, *health_ports = free_ports(7)
        env_file, values = compose_env(directory, gateway_port)
        extra_env = {**os.environ,
                     "MCR_ENV_FILE": str(env_file),
                     "MCR_DB_PASSWORD": values["MCR_DB_PASSWORD"],
                     "MCR_REDIS_PASSWORD": values["MCR_REDIS_PASSWORD"],
                     "MCR_GATEWAY_HOST_PORT": str(gateway_port),
                     "MCR_GATEWAY_METRICS_HOST_PORT": str(health_ports[0]),
                     "MCR_AUTH_METRICS_HOST_PORT": str(health_ports[1]),
                     "MCR_PROFILE_METRICS_HOST_PORT": str(health_ports[2]),
                     "MCR_FRIEND_METRICS_HOST_PORT": str(health_ports[3]),
                     "MCR_POST_METRICS_HOST_PORT": str(health_ports[4]),
                     "MCR_NOTIFICATION_METRICS_HOST_PORT": str(health_ports[5]),
                     "MCR_CONCERT_METRICS_HOST_PORT": str(health_ports[6])}
        run(compose_cmd(project, ["down", "-v", "--remove-orphans"], extra_env), check=False)
        log("compose: building and starting a fresh volume set")
        run(compose_cmd(project, ["up", "-d", "--build"], extra_env))
        report["services_health"] = {}
        metrics = {"api-gateway": health_ports[0], "auth-service": health_ports[1],
                   "profile-service": health_ports[2], "friend-service": health_ports[3],
                   "post-service": health_ports[4], "notification-service": health_ports[5],
                   "concert-service": health_ports[6]}
        for service, port in metrics.items():
            status, _ = wait_status(port, "/livez", 200, label=service)
            report["services_health"][service] = {"path": "/livez", "http_status": status}
        status, body = wait_status(health_ports[0], "/readyz", 200, label="api-gateway")
        ready = json_or_fail(body, "gateway-readyz")
        if not all_required_up(ready):
            raise SmokeFailure("gateway-readyz", "gateway reported a required peer down")
        report["gateway_readyz"] = {"http_status": status, "body": ready}
        public_route(gateway_port, report)
        # Real row written through psql, then both PostgreSQL and the gateway restart.
        token = secrets.token_hex(16)
        run(compose_cmd(project, ["exec", "-T", "postgres", "psql", "-U", "mcr_owner", "-d", SMOKE_DB,
                                  "-v", "ON_ERROR_STOP=1", "-c",
                                  "create schema if not exists mcr_smoke; "
                                  "create table if not exists mcr_smoke.records (token text primary key); "
                                  f"insert into mcr_smoke.records values ('{token}');"], extra_env))
        report["restart"] = {"before": "inserted mcr_smoke.records row"}
        run(compose_cmd(project, ["restart", "postgres"], extra_env))
        run(compose_cmd(project, ["restart", "api-gateway"], extra_env))
        wait_status(health_ports[0], "/readyz", 200, label="api-gateway-restarted")
        row = run(compose_cmd(project, ["exec", "-T", "postgres", "psql", "-U", "mcr_owner", "-d", SMOKE_DB,
                                        "-A", "-t", "-c", "select token from mcr_smoke.records;"], extra_env))
        if token not in row.stdout:
            raise SmokeFailure("restart-persistence", "row did not survive the PostgreSQL restart")
        report["restart"].update({"after": "row survived PostgreSQL restart", "status": "PASS"})
        report["result"] = "PASS"
    except SmokeFailure as error:
        report["failure"] = {"phase": error.phase, "diagnostic": error.diagnostic}
    finally:
        log("compose: tearing down project")
        run(compose_cmd(project, ["down", "-v", "--remove-orphans"], extra_env if "extra_env" in locals() else os.environ),
            check=False)
        leftovers = compose_leftovers(project)
        report["teardown"] = leftovers
        report["cleanup"] = "PASS" if not any(leftovers.values()) else "FAIL"
        if report["cleanup"] != "PASS":
            report["result"] = "FAIL"
        shutil.rmtree(directory, ignore_errors=True)
    return report


# ----------------------------------------------------------------- native mode

def native_build(args):
    target = os.environ.get("CARGO_TARGET_DIR", str(BACKEND / "target"))
    env = {**os.environ, "CARGO_TARGET_DIR": target,
           "CARGO_BUILD_JOBS": os.environ.get("CARGO_BUILD_JOBS", "3")}
    log(f"native: building seven services into {target}")
    run(["cargo", "+1.88.0", "build", "--locked", "--bins",
         *sum((["-p", service] for service in SERVICES), [])], env=env, cwd=BACKEND)
    return Path(target) / "debug"


def concert_environment(cluster, ports, keys, credentials):
    return {**cluster.env, **credentials,
            "CONCERT_DATABASE_URL": cluster.url("mcr_concert"),
            "CONCERT_METRICS_ADDR": ports.address("concert-service:metrics"),
            "CONCERT_RUNTIME_MODE": "local",
            "CONCERT_MIGRATION_MODE": "external",
            "ACTOR_DELEGATION_VERIFYING_KEYS": keys["ACTOR_DELEGATION_VERIFYING_KEYS"],
            "RUST_LOG": "warn", "TOKIO_WORKER_THREADS": "2"}


def native_mode(args):
    binaries_dir = args.bin_dir or Path(os.environ.get("CARGO_TARGET_DIR", str(BACKEND / "target"))) / "debug"
    if args.build:
        binaries_dir = native_build(args)
    pg_bin = Path(args.pg_bin_dir)
    redis_binary = Path(args.redis_binary)
    openssl_binary = Path(args.openssl_binary)
    binaries = {service: migration.executable(binaries_dir / service) for service in SERVICES}
    tools = {name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS}
    migration.executable(runtime.LSOF)
    redis_binary, openssl_binary = migration.executable(redis_binary), migration.executable(openssl_binary)
    migration.SERVICES["concert-service"] = "CONCERT_DATABASE_URL"
    manifests = {service: migration.migration_manifest(BACKEND, service) for service in SERVICES}
    report = {"mode": "native", "result": "FAIL", "cleanup": "NOT_RUN", "services": [],
              "started_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
              "source_revision": run(["/usr/bin/git", "rev-parse", "HEAD"], env={"PATH": "/usr/bin:/bin"}).stdout.strip(),
              "binaries": {service: str(binaries[service]) for service in SERVICES}}
    cluster, ports, children, redis_child = None, None, {}, None
    secret_values = ()
    paused = []

    def defer(signum, _frame):
        paused.append(signum)
    previous = {sig: signal.signal(sig, defer) for sig in (signal.SIGINT, signal.SIGTERM)}

    def start_stack(label, keys, credentials, redis_password):
        nonlocal ports, redis_child, secret_values
        ports = runtime.Ports()
        for service in runtime.START_ORDER:
            for kind in ("http", "metrics", "grpc"):
                ports.reserve(f"{service}:{kind}")
        ports.reserve("concert-service:metrics")
        ports.reserve("redis")
        ports.reserve("disabled-egress")
        envs = runtime.environments(cluster, ports, keys, credentials, redis_password)
        runtime.validate_environments(cluster, ports, envs)
        envs["concert-service"] = concert_environment(cluster, ports, keys, credentials)
        secret_values = (cluster.password, redis_password, *credentials.values(), *keys.values(),
                         *(value for env in envs.values() for key, value in env.items() if "HMAC_KEY" in key))
        config = cluster.root / f"redis-{label}.conf"
        runtime.private_file(config, (f"bind 127.0.0.1\nport {ports.ports['redis']}\nprotected-mode yes\n"
            f"requirepass {redis_password}\ndir {cluster.root}\ndaemonize no\nsave \"\"\nappendonly no\n").encode())
        redis_child = runtime.Child(cluster, "redis", [str(redis_binary), str(config)],
                                    cluster.env, ports, ("redis",), secret_values)
        redis_child.wait_listeners()
        runtime.redis_ping(redis_child, redis_password)
        for service in START_ORDER:
            if not redis_child.listeners():
                raise SmokeFailure("redis", "owned Redis listener disappeared")
            for prior in children.values():
                if not prior.listeners():
                    raise SmokeFailure("ownership", f"{prior.name} lost a listener")
            reservations = (("concert-service:metrics",) if service == "concert-service"
                            else tuple(f"{service}:{kind}" for kind in ("http", "metrics", "grpc")))
            child = runtime.Child(cluster, service, [str(binaries[service])],
                                  envs[service], ports, reservations, secret_values)
            children[service] = child
            child.wait_listeners()
        report["configuration"] = {"runtime_mode": "parity", "migration_mode": "external",
                                   "concert": "health-only operational mode", "ports": ports.ports.copy()}
        return ports, envs

    def stop_children():
        for service in reversed(START_ORDER):
            child = children.pop(service, None)
            if child is None:
                continue
            stopped = child.stop()
            report.setdefault("process_stops", []).append({"service": service, **stopped})
            if stopped["forced"] or stopped.get("unexpected_exit") or stopped["exit_code"] != 0:
                raise SmokeFailure("service-stop", f"{service} did not exit cleanly")

    def stop_redis():
        nonlocal redis_child
        if redis_child is not None:
            stopped = redis_child.stop()
            report.setdefault("process_stops", []).append({"service": "redis", **stopped})
            redis_child = None
            if stopped["forced"] or stopped.get("unexpected_exit") or stopped["exit_code"] != 0:
                raise SmokeFailure("redis-stop", "redis did not exit cleanly")

    def assert_stack(label):
        rows = {row["service"]: row for row in report["services"]}
        for service in runtime.START_ORDER:
            child = children[service]
            status, _ = runtime.http_probe(child, "metrics", "/livez")
            rows[service][label + "_livez"] = status
            if status != 200:
                raise SmokeFailure("liveness", f"{service} livez is {status}")
        status, _ = runtime.http_probe(children["concert-service"], "metrics", "/livez")
        rows["concert-service"][label + "_livez"] = status
        if status != 200:
            raise SmokeFailure("liveness", f"concert-service livez is {status}")
        bodies = {}
        for name in runtime.START_ORDER:
            status, body = runtime.http_probe(children[name], "metrics", "/readyz")
            bodies[name] = {"status": status, "body": json.loads(body)}
        report.setdefault("readiness_probe", {})[label] = bodies
        wait_ready({name: children[name] for name in runtime.START_ORDER}, rows, label + "_readiness")
        status, body = runtime.http_probe(children["concert-service"], "metrics", "/readyz")
        value = json_or_fail(body, "concert-readyz")
        rows["concert-service"][label + "_readyz"] = {"http_status": status, "body": value}
        if status != 200 or not value.get("ready") or value.get("checks", {}).get("database", {}).get("status") != "up":
            raise SmokeFailure("concert-readyz", f"concert readiness is not honest: {status} {body[:300]!r}")
        status, body = runtime.http_probe(children["api-gateway"], "metrics", "/readyz")
        value = json_or_fail(body, "gateway-readyz")
        rows["api-gateway"][label + "_gateway_readyz"] = {"http_status": status, "body": value}
        if status != 200 or not all_required_up(value):
            raise SmokeFailure("gateway-readyz", "gateway reported a required peer down")
        report[label + "_gateway_readyz"] = {"http_status": status, "body": value}
        gateway_http_port = ports.ports["api-gateway:http"]
        public_route(gateway_http_port, report)
        return value

    try:
        with runtime.protected_signals():
            cluster = migration.Cluster(tools)
            report["owned_directory"] = str(cluster.root)
        log("native: migrating and verifying seven databases (with PostgreSQL restart proof)")
        with runtime.protected_signals():
            migration.qualify(cluster, binaries, manifests, report)
        rows = {row["service"]: row for row in report["services"]}
        credentials = {prefix + "_SERVICE_CREDENTIAL": secrets.token_hex(32) for prefix in runtime.PREFIX.values()}
        credentials["CONCERT_SERVICE_CREDENTIAL"] = secrets.token_hex(32)
        keys = runtime.generated_keys(cluster, openssl_binary)
        redis_password = secrets.token_hex(32)
        log("native: starting seven services")
        with runtime.protected_signals():
            start_stack("initial", keys, credentials, redis_password)
        gateway_ready = assert_stack("initial")
        # One required peer leaves: the gateway must stop reporting ready.
        victim = children["friend-service"]
        stopped = victim.stop()
        report.setdefault("process_stops", []).append({"service": "friend-service", **stopped})
        del children["friend-service"]
        time.sleep(1.1)
        status, body = runtime.http_probe(children["api-gateway"], "metrics", "/readyz")
        value = json_or_fail(body, "gateway-outage-readyz")
        edge = value.get("checks", {}).get("gateway->friend") or {}
        report["peer_outage"] = {"stopped": "friend-service", "http_status": status,
                                 "ready": value.get("ready"), "friend_check": edge}
        if status != 503 or value.get("ready") is not False or edge.get("status") == "up":
            raise SmokeFailure("peer-outage", "gateway readiness did not report the stopped peer")
        # Record a real row per owned database, restart the whole stack, and prove it.
        tokens = {}
        for service in SERVICES:
            database = "mcr_" + service.removesuffix("-service").replace("-", "_")
            token = secrets.token_hex(16)
            cluster.sql(database, "create schema if not exists mcr_smoke; "
                        "create table if not exists mcr_smoke.runtime_records (token text primary key); "
                        f"insert into mcr_smoke.runtime_records values ('{token}');")
            tokens[service] = token
        before_pid = cluster.assert_postmaster()
        with runtime.protected_signals():
            stop_children()
            stop_redis()
            cluster.stop()
        with runtime.protected_signals():
            cluster.start()
        after_pid = cluster.assert_postmaster()
        if before_pid == after_pid:
            raise SmokeFailure("restart", "PostgreSQL PID did not change")
        log("native: restarting seven services on a fresh topology")
        with runtime.protected_signals():
            start_stack("restarted", keys, credentials, redis_password)
        assert_stack("restarted")
        for service, token in tokens.items():
            database = "mcr_" + service.removesuffix("-service").replace("-", "_")
            if cluster.sql(database, "select token from mcr_smoke.runtime_records;") != token:
                raise SmokeFailure("restart-persistence", f"{service} smoke row did not survive")
            rows[service]["restart_persistence"] = "PASS"
        report["restart"] = {"before_pid": before_pid, "after_pid": after_pid,
                             "row_survived": {service: True for service in SERVICES}, "status": "PASS"}
        with runtime.protected_signals():
            stop_children()
            stop_redis()
        report["result"] = "PASS"
    except SmokeFailure as error:
        report["failure"] = {"phase": error.phase, "diagnostic": error.diagnostic}
    except (OSError, ValueError, KeyError, migration.SmokeError) as error:
        if isinstance(error, migration.SmokeError):
            report["failure"] = {"phase": error.detail.get("phase"), "diagnostic": error.detail.get("diagnostic")}
        else:
            import traceback
            report["failure"] = {"phase": "harness", "diagnostic": f"{type(error).__name__}: {error}",
                                 "traceback": traceback.format_exc()[-4000:]}
    finally:
        with runtime.protected_signals(cleanup=True):
            for service in reversed(list(children)):
                try:
                    children.pop(service).stop()
                except Exception:
                    report["result"] = "FAIL"
            if redis_child is not None:
                try:
                    redis_child.stop()
                except Exception:
                    report["result"] = "FAIL"
            if ports is not None:
                ports.close()
            if cluster is not None:
                try:
                    cluster.cleanup()
                    report["cleanup"] = "PASS" if not cluster.root.exists() else "FAIL"
                except Exception as error:
                    report["cleanup"] = "FAIL"
                    report["cleanup_error"] = type(error).__name__
                    report["result"] = "FAIL"
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        if paused:
            raise KeyboardInterrupt
    return report


def docker_available():
    """Honor the active Docker context or DOCKER_HOST, not one hard-coded socket path."""
    if shutil.which("docker") is None:
        return False
    return run(["docker", "info"], check=False).returncode == 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("auto", "compose", "native"), default="auto")
    parser.add_argument("--bin-dir", type=Path)
    parser.add_argument("--build", action="store_true", help="build native binaries first")
    parser.add_argument("--pg-bin-dir", default="/opt/homebrew/bin")
    parser.add_argument("--redis-binary", default="/opt/homebrew/bin/redis-server")
    parser.add_argument("--openssl-binary", default="/opt/homebrew/bin/openssl")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    mode = args.mode
    if mode == "auto":
        mode = "compose" if docker_available() else "native"
        log(f"auto: docker daemon {'available' if mode == 'compose' else 'unavailable -> native fallback'}")
    if mode == "compose" and not docker_available():
        probe_result = run(["docker", "info"], check=False)
        reason = (probe_result.stderr or probe_result.stdout).strip().splitlines()
        report = {"mode": "compose", "result": "NOT_RUN", "cleanup": "NOT_RUN",
                  "reason": "docker daemon is unavailable",
                  "diagnostic": reason[0][:300] if reason else "docker info produced no diagnostic"}
    elif mode == "compose":
        report = compose_mode(args)
    else:
        report = native_mode(args)
    if args.report:
        if not args.report.is_absolute():
            parser.error("--report requires an absolute path")
        args.report.write_text(json.dumps(report, indent=2) + "\n")
        args.report.chmod(0o600)
    print(json.dumps({"mode": report["mode"], "result": report["result"], "cleanup": report["cleanup"],
                      "failure": report.get("failure")}, sort_keys=True))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
