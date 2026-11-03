#!/usr/bin/env python3
"""Foreground disposable MCR development stack; native services, private mail, owned stop."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import http.server
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import socketserver
import stat
import threading
import time
from urllib.parse import urlsplit
from uuid import UUID

import migration_smoke as migration
import runtime_smoke as runtime

SmokeError = migration.SmokeError
MAX_HOLD = 8 * 60 * 60


def encode(value):
    return (json.dumps(value, indent=2) + "\n").encode()


def write_fd(fd, value):
    with os.fdopen(fd, "wb") as stream:
        stream.write(encode(value))
        stream.flush()
        os.fsync(stream.fileno())


def local_origin(value):
    try:
        parsed = urlsplit(value)
        valid = (parsed.scheme == "http" and parsed.hostname in ("127.0.0.1", "localhost")
                 and parsed.port is not None and 0 < parsed.port < 65536
                 and parsed.netloc == f"{parsed.hostname}:{parsed.port}"
                 and not parsed.path and not parsed.query and not parsed.fragment)
    except ValueError:
        valid = False
    if not valid:
        raise SmokeError("origin", "browser origin must be an explicit plain loopback HTTP origin")
    return value


class MailSink:
    """Actual Auth outbox HTTP delivery; captures are private fixture data, never logs."""
    def __init__(self, cluster, ports, token):
        self.cluster, self.token, self.count = cluster, token, 0
        self.directory = cluster.root / "mail"
        self.directory.mkdir(mode=0o700)
        sink = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                self.connection.settimeout(2)
                status_code = 400
                try:
                    if self.headers.get("Authorization") != "Bearer " + sink.token:
                        status_code = 401
                    else:
                        size = int(self.headers.get("Content-Length", "0"))
                        if not 0 < size <= 16384 or self.headers.get("Content-Type") != "application/json":
                            raise ValueError
                        raw = self.rfile.read(size)
                        if len(raw) != size:
                            raise ValueError
                        status_code = sink.accept(self.path, self.headers.get("Idempotency-Key", ""), json.loads(raw))
                except (ValueError, TypeError, KeyError, OSError, SmokeError):
                    status_code = 400
                self.send_response(status_code)
                self.send_header("Content-Length", "0")
                self.send_header("Connection", "close")
                self.end_headers()
                self.close_connection = True

        class QuietServer(http.server.HTTPServer):
            def handle_error(self, *_args):
                pass

        self.server = QuietServer(("127.0.0.1", ports.ports["mail"]), Handler, bind_and_activate=False)
        self.server.socket.close()
        self.server.socket = ports.held.pop("mail")
        self.server.server_address = self.server.socket.getsockname()
        self.server.server_activate()
        self.thread = threading.Thread(target=self.server.serve_forever, kwargs={"poll_interval": 0.1}, daemon=True)
        try:
            self.thread.start()
        except BaseException:
            self.server.server_close()
            raise

    def accept(self, path, delivery_id, payload):
        self.cluster.assert_owned()
        if self.directory.is_symlink() or not self.directory.is_dir():
            raise SmokeError("mail", "private inbox was replaced")
        delivery_id = str(UUID(delivery_id))
        kind = {"/verify": "VERIFY_EMAIL", "/reset": "PASSWORD_RESET"}.get(path)
        required = {"email", "verification_code", "expires_at"} if kind == "VERIFY_EMAIL" else {"email", "reset_link"}
        if kind is None or not isinstance(payload, dict) or set(payload) != required:
            raise ValueError
        if not all(isinstance(v, str) and 0 < len(v) <= 2048 for v in payload.values()):
            raise ValueError
        if kind == "VERIFY_EMAIL" and (len(payload["verification_code"]) != 6 or not payload["verification_code"].isascii() or not payload["verification_code"].isdigit()):
            raise ValueError
        target = self.directory / (delivery_id + ".json")
        value = {"kind": kind, "delivery_id": delivery_id, "payload": payload}
        if target.exists() or target.is_symlink():
            if target.is_symlink() or target.read_bytes() != encode(value):
                return 409
            return 204
        if self.count >= 1000:
            return 507
        pending = self.directory / ("." + delivery_id + ".pending")
        try:
            runtime.private_file(pending, encode(value))
            os.link(pending, target)  # Publish complete bytes without replacing an existing capture.
        finally:
            pending.unlink(missing_ok=True)
        self.count += 1
        return 204

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=3)
        if self.thread.is_alive():
            raise SmokeError("mail-stop", "owned mail server did not stop")

    def stop(self):
        self.close()
        return {"resource": "in_process_mail_sink", "exit_code": 0, "forced": False}


def development_environments(cluster, ports, keys, credentials, redis_password, mail_token, browser_origin=None):
    envs = runtime.environments(cluster, ports, keys, credentials, redis_password)
    # Qualify the unchanged common configuration BEFORE the narrowly owned local overrides.
    runtime.validate_environments(cluster, ports, envs)
    origin = local_origin(browser_origin or ports.url("api-gateway:http"))
    envs["api-gateway"].update(GATEWAY_REFRESH_COOKIE_SECURE="false", GATEWAY_CORS_ALLOWED_ORIGINS=origin)
    auth = envs["auth-service"]
    auth["AUTH_PASSWORD_RESET_BASE_URL"] = ports.url("api-gateway:http") + "/reset-password"
    auth["AUTH_PASSWORD_RESET_MAILER_MODE"] = "http"
    for label, path in (("VERIFICATION", "/verify"), ("PASSWORD_RESET", "/reset")):
        auth[f"AUTH_{label}_MAILER_ENDPOINT"] = ports.url("mail") + path
        auth[f"AUTH_{label}_MAILER_BEARER_TOKEN"] = mail_token
    return envs


class DevStack:
    """One owner per foreground invocation; never reattaches an existing store."""
    def __init__(self, bin_dir, pg_bin_dir, redis_binary, openssl_binary, browser_origin=None):
        self.bin_dir, self.pg_bin_dir = bin_dir, pg_bin_dir
        self.redis_binary, self.openssl_binary = redis_binary, openssl_binary
        self.browser_origin = browser_origin
        self.cluster, self.mail, self.redis = None, None, None
        self.ports, self.children = runtime.Ports(), {}
        self.report = {"scope": "BE02 foreground native local development", "result": "FAIL", "cleanup": "NOT_RUN",
                       "started_at_utc": datetime.now(timezone.utc).isoformat(), "services": [], "process_stops": [],
                       "script_sha256": runtime.digest(Path(__file__)),
                       "runtime_helper_sha256": runtime.digest(Path(runtime.__file__)),
                       "migration_helper_sha256": runtime.digest(Path(migration.__file__)),
                       "auth_acceptance": "NOT_RUN", "image_build": "NOT_RUN", "production": "NOT_RUN"}

    def start(self):
        binaries, tools, manifests = migration.preflight(self.bin_dir, self.pg_bin_dir)
        for path in (self.redis_binary, self.openssl_binary):
            if not path.is_absolute():
                raise SmokeError("preflight", "native dependency paths must be absolute")
        redis_binary = migration.executable(self.redis_binary)
        openssl_binary = migration.executable(self.openssl_binary)
        migration.executable(runtime.LSOF)
        if self.browser_origin:
            local_origin(self.browser_origin)
        self.report["source_revision"] = migration.execute(["/usr/bin/git", "rev-parse", "HEAD"],
            env={"PATH": "/usr/bin:/bin"}, cwd=migration.BACKEND, phase="revision").stdout.strip()
        with runtime.protected_signals():
            self.cluster = migration.Cluster(tools)
            self.report["owned_directory"] = str(self.cluster.root)
            migration.qualify(self.cluster, binaries, manifests, self.report)
        self.rows = {row["service"]: row for row in self.report["services"]}
        for name in runtime.START_ORDER:
            for kind in ("http", "grpc", "metrics"):
                self.ports.reserve(f"{name}:{kind}")
        for name in ("redis", "disabled-egress", "mail"):
            self.ports.reserve(name)
        keys = runtime.generated_keys(self.cluster, openssl_binary)
        credentials = {prefix + "_SERVICE_CREDENTIAL": secrets.token_hex(32) for prefix in runtime.PREFIX.values()}
        redis_password, mail_token = secrets.token_hex(32), secrets.token_hex(32)
        envs = development_environments(self.cluster, self.ports, keys, credentials, redis_password, mail_token, self.browser_origin)
        secret_values = (self.cluster.password, redis_password, mail_token, *keys.values(), *credentials.values(),
                         *(v for env in envs.values() for k, v in env.items() if "HMAC_KEY" in k))
        config = self.cluster.root / "redis.conf"
        runtime.private_file(config, (f"bind 127.0.0.1\nport {self.ports.ports['redis']}\nprotected-mode yes\n"
            f"requirepass {redis_password}\ndir {self.cluster.root}\ndaemonize no\nsave \"\"\nappendonly no\n").encode())
        with runtime.protected_signals():
            self.mail = MailSink(self.cluster, self.ports, mail_token)
            self.redis = runtime.Child(self.cluster, "redis", [str(redis_binary), str(config)], self.cluster.env,
                                       self.ports, ("redis",), secret_values)
        self.redis.wait_listeners()
        runtime.redis_ping(self.redis, redis_password)
        for name in runtime.START_ORDER:
            with runtime.protected_signals():
                child = runtime.Child(self.cluster, name, [str(binaries[name])], envs[name], self.ports,
                                      tuple(f"{name}:{kind}" for kind in ("http", "metrics", "grpc")), secret_values)
                self.children[name] = child
                self.rows[name]["pid"] = child.process.pid
            child.wait_listeners()
        self.health()
        self.report["configuration"] = {"mode": "parity", "seed": False, "migration_mode": "external",
            "home": "OMITTED", "cookie_secure": False, "cookie_scope": "explicit local HTTP fixture only",
            "browser_origin": envs["api-gateway"]["GATEWAY_CORS_ALLOWED_ORIGINS"], "mail": "private local HTTP capture",
            "kafka": "DISABLED", "oauth": "NOT_EXERCISED; inherited parity routes are not C03 provider qualification"}
        self.report["result"] = "PASS"
        return self.descriptor()

    def descriptor(self):
        info = self.cluster.root.stat()
        return {"kind": "mcr-owned-dev-stack-v1", "pid": os.getpid(), "uid": os.getuid(),
                "root": str(self.cluster.root), "root_identity": [info.st_dev, info.st_ino],
                "owner": {"token": self.cluster.token, "uid": os.getuid()},
                "socket": str(self.cluster.root / "control.sock"),
                "gateway_url": self.ports.url("api-gateway:http"),
                "services": {name: {kind: self.ports.url(f"{name}:{kind}") for kind in ("http", "grpc", "metrics")}
                             for name in runtime.START_ORDER},
                "mail_directory": str(self.mail.directory), "auth_http": "Gateway /auth/*; no /api ingress adapter yet"}

    def health(self):
        runtime.wait_ready(self.children, self.rows, "readiness", timeout=10)
        for name, child in self.children.items():
            status, _ = runtime.http_probe(child, "metrics", "/livez")
            self.rows[name]["livez_status"] = status
            if status != 200:
                raise SmokeError("liveness", "owned service failed liveness")
        if self.redis is not None:
            if not self.redis.listeners():
                raise SmokeError("redis", "owned Redis listener disappeared")
        if self.mail is not None and not self.mail.thread.is_alive():
            raise SmokeError("mail", "owned local mail server exited")

    def close(self):
        with runtime.protected_signals(cleanup=True):
            resources = {**self.children, **({"local-mail": self.mail} if self.mail else {})}
            runtime.cleanup_resources(self.cluster, self.ports, resources, self.redis, self.report)
            if self.mail is not None:
                self.report["captured_mail_count"] = self.mail.count
            for row in self.report["services"]:
                row.pop("_token", None)
            self.report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()


class Control(socketserver.UnixStreamServer):
    def __init__(self, stack):
        self.stack, self.stopping = stack, False
        owner = self
        class Handler(socketserver.StreamRequestHandler):
            def handle(self):
                self.connection.settimeout(1)
                try:
                    request = json.loads(self.rfile.readline(2049))
                    if not isinstance(request, dict) or set(request) != {"action"}:
                        raise ValueError
                    if request["action"] == "stop":
                        owner.stopping = True
                        response = {"state": "STOPPING"}
                    elif request["action"] == "status":
                        stack.health()
                        response = {"state": "READY", "gateway_url": stack.ports.url("api-gateway:http"),
                                    "services": {name: row["readiness"] for name, row in stack.rows.items()}}
                    else:
                        raise ValueError
                    self.wfile.write(encode(response))
                except (ValueError, TypeError, OSError, SmokeError):
                    self.wfile.write(b'{"state":"REJECTED"}\n')
        super().__init__(str(stack.cluster.root / "control.sock"), Handler)
        Path(self.server_address).chmod(0o600)
        self.timeout = 0.2

    def handle_error(self, *_args):
        pass


def control_request(ready_file, action):
    """Read only a private descriptor; stop is a protocol request, never PID signalling."""
    info = ready_file.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise SmokeError("control", "ready file must be an owned private regular file")
    value = json.loads(ready_file.read_bytes())
    root, control = Path(value["root"]), Path(value["socket"])
    root_info, sock_info = root.lstat(), control.lstat()
    marker = root / "mcr-owner.json"
    if (value["kind"] != "mcr-owned-dev-stack-v1" or value["uid"] != os.getuid()
            or not stat.S_ISDIR(root_info.st_mode) or root_info.st_uid != os.getuid() or root_info.st_mode & 0o077
            or [root_info.st_dev, root_info.st_ino] != value["root_identity"]
            or marker.is_symlink() or json.loads(marker.read_bytes()) != value["owner"]
            or control != root / "control.sock" or not stat.S_ISSOCK(sock_info.st_mode)
            or sock_info.st_uid != os.getuid() or sock_info.st_mode & 0o077):
        raise SmokeError("control", "refusing stale, foreign or replaced stack descriptor")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(3)
        connection.connect(str(control))
        connection.sendall((json.dumps({"action": action}) + "\n").encode())
        with connection.makefile("rb") as stream:
            response = stream.read(16385)
        if len(response) > 16384:
            raise SmokeError("control", "oversized control response")
        return json.loads(response)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    start = commands.add_parser("start")
    start.add_argument("--bin-dir", type=Path, required=True)
    start.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/bin"))
    start.add_argument("--redis-binary", type=Path, default=Path("/opt/homebrew/bin/redis-server"))
    start.add_argument("--openssl-binary", type=Path, default=Path("/opt/homebrew/bin/openssl"))
    start.add_argument("--browser-origin")
    start.add_argument("--duration-seconds", type=int, default=3600)
    start.add_argument("--ready-file", type=Path, required=True)
    start.add_argument("--report", type=Path, required=True)
    for name in ("status", "stop"):
        commands.add_parser(name).add_argument("--ready-file", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.command != "start":
        try:
            response = control_request(args.ready_file, args.command)
            print(json.dumps(response))
            expected = "READY" if args.command == "status" else "STOPPING"
            return 0 if response.get("state") == expected else 1
        except (SmokeError, OSError, ValueError, KeyError, TypeError):
            print(json.dumps({"state": "UNAVAILABLE", "diagnostic": "owned stack control is unavailable"}))
            return 1
    if not (1 <= args.duration_seconds <= MAX_HOLD):
        parser.error("duration must be 1..28800 seconds after readiness")
    if not args.report.is_absolute() or not args.ready_file.is_absolute() or args.report == args.ready_file:
        parser.error("report and ready file must be distinct absolute new paths")
    fd = ready_fd = None
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        ready_fd = os.open(args.ready_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        if fd is not None:
            write_fd(fd, {"result": "FAIL", "cleanup": "NOT_RUN", "failure": "new ready-file reservation failed"})
        parser.error("report and ready file must be new writable files; no existing file is overwritten")
    def interrupted(*_args):
        raise KeyboardInterrupt
    previous = {sig: signal.signal(sig, interrupted) for sig in (signal.SIGINT, signal.SIGTERM)}
    stack = DevStack(args.bin_dir, args.pg_bin_dir, args.redis_binary, args.openssl_binary, args.browser_origin)
    control, ready = None, False
    try:
        descriptor = stack.start()
        with runtime.protected_signals():
            control = Control(stack)
            write_fd(ready_fd, descriptor)
            ready_fd = None
            ready = True
        print(json.dumps({"state": "READY", "ready_file": str(args.ready_file), "gateway_url": descriptor["gateway_url"]}), flush=True)
        deadline, next_health = time.monotonic() + args.duration_seconds, time.monotonic() + 2
        while time.monotonic() < deadline and not control.stopping:
            control.handle_request()
            if time.monotonic() >= next_health:
                stack.health()
                next_health = time.monotonic() + 2
        stack.report["end_reason"] = "control_stop" if control.stopping else "duration_elapsed"
    except KeyboardInterrupt:
        stack.report["end_reason"] = "signal"
        if not ready:
            stack.report.update(result="FAIL", failure={"phase": "startup", "diagnostic": "KeyboardInterrupt"})
    except Exception as error:
        # Never include HTTP payloads, cookies, mail bodies or arbitrary exception messages.
        stack.report.update(result="FAIL", failure={"phase": "development", "diagnostic": type(error).__name__})
    finally:
        with runtime.protected_signals(cleanup=True):
            if control is not None:
                try:
                    control.server_close()
                except Exception as error:
                    stack.report.update(result="FAIL", control_cleanup_failure=type(error).__name__)
            stack.close()
            write_fd(fd, stack.report)
            if ready_fd is not None:
                os.close(ready_fd)
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    print(json.dumps({"result": stack.report["result"], "cleanup": stack.report["cleanup"], "report": str(args.report)}))
    return 0 if stack.report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
