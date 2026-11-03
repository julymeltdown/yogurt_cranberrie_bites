#!/usr/bin/env python3
"""Qualify migration receipts and restart persistence in a disposable owned PG cluster."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import stat
import subprocess
import tempfile
from urllib.parse import parse_qs, quote, urlencode, urlsplit


BACKEND = Path(__file__).resolve().parents[2]
SERVICES = {
    "api-gateway": "GATEWAY_DATABASE_URL",
    "auth-service": "AUTH_DATABASE_URL",
    "profile-service": "PROFILE_DATABASE_URL",
    "friend-service": "FRIEND_DATABASE_URL",
    "post-service": "POST_DATABASE_URL",
    "notification-service": "NOTIFICATION_DATABASE_URL",
}
PG_TOOLS = ("initdb", "pg_ctl", "psql", "createdb")
OWNER = "mcr_owner"
PORT = 5432  # Unix sockets are namespaced by our new private directory; no TCP bind.


class SmokeError(Exception):
    def __init__(self, phase: str, diagnostic: str, exit_code: int | None = None):
        self.detail = {"phase": phase, "diagnostic": diagnostic, "exit_code": exit_code}
        super().__init__(phase)


def redact(text: str, passwords: tuple[str, ...] = ()) -> str:
    for value in passwords:
        text = text.replace(value, "[redacted]")
    text = re.sub(r"(?:postgres(?:ql)?|redis)://[^\s]+", "[redacted-url]", text)
    return text[-1200:]


def execute(argv: list[str], *, env: dict[str, str], phase: str,
            timeout: int = 60, allowed: tuple[int, ...] = (0,),
            cwd: Path | None = None) -> subprocess.CompletedProcess:
    try:
        result = subprocess.run(argv, env=env, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise SmokeError(phase, "command timed out") from None
    except OSError as error:
        raise SmokeError(phase, f"cannot execute command (errno {error.errno})") from None
    if result.returncode not in allowed:
        password = env.get("PGPASSWORD", "")
        # A service environment deliberately contains only its own generated URL.
        passwords = tuple(filter(None, [password, *[
            urlsplit(v).password or "" for k, v in env.items() if k.endswith("DATABASE_URL")
        ]]))
        raise SmokeError(phase, redact(result.stderr or result.stdout, passwords), result.returncode)
    return result


def executable(path: Path) -> Path:
    if not path.is_file() or not os.access(path, os.X_OK):
        raise SmokeError("preflight", f"missing executable: {path.name}")
    return path.resolve()


def migration_manifest(backend: Path, service: str) -> dict[int, str]:
    expected = {}
    for path in sorted((backend / "services" / service / "migrations").glob("*.sql")):
        if path.name.endswith(".down.sql"):
            continue
        match = re.fullmatch(r"([0-9]+)_.+\.sql", path.name)
        if not match or int(match[1]) <= 0 or int(match[1]) in expected:
            raise SmokeError("preflight", f"invalid migration set: {service}")
        expected[int(match[1])] = hashlib.sha384(path.read_bytes()).hexdigest()
    if not expected:
        raise SmokeError("preflight", f"empty migration set: {service}")
    return expected


def preflight(bin_dir: Path, pg_bin_dir: Path) -> tuple[dict, dict, dict]:
    if not bin_dir.is_absolute() or not pg_bin_dir.is_absolute():
        raise SmokeError("preflight", "binary directories must be explicit absolute paths")
    binaries = {service: executable(bin_dir / service) for service in SERVICES}
    tools = {name: executable(pg_bin_dir / name) for name in PG_TOOLS}
    manifests = {service: migration_manifest(BACKEND, service) for service in SERVICES}
    return binaries, tools, manifests


class Cluster:
    """Never attaches an existing root; stop/remove require this instance's marker/inode."""

    def __init__(self, tools: dict[str, Path], root: Path | None = None):
        self.tools = tools
        if root is None:
            root = Path(tempfile.mkdtemp(prefix="mcr-pg-", dir="/tmp"))
        else:
            # Testable refusal path: even an empty existing directory is foreign.
            root.mkdir(mode=0o700, exist_ok=False)
        self.root = root.resolve()
        self.data = self.root / "data"
        self.socket = self.root / "socket"
        self.marker = self.root / "mcr-owner.json"
        self.identity = self.root.stat().st_dev, self.root.stat().st_ino
        self.token = secrets.token_hex(24)
        self.password = secrets.token_hex(32)
        try:
            with self.marker.open("x") as stream:
                stream.write(json.dumps({"token": self.token, "uid": os.getuid()}))
            self.marker.chmod(0o600)
            self.socket.mkdir(mode=0o700)
        except OSError:
            # Construction cannot have started a process. Remove only the root we made.
            info = self.root.lstat()
            if (info.st_dev, info.st_ino) == self.identity and info.st_uid == os.getuid():
                shutil.rmtree(self.root)
            raise
        self.started = False
        # No HOME/PG*/DATABASE_URL/provider credentials are inherited.
        self.env = {"PATH": "/usr/bin:/bin:/opt/homebrew/bin", "LC_ALL": "C",
                    "TMPDIR": str(self.root),
                    "PGPASSFILE": "/dev/null", "PGCONNECT_TIMEOUT": "5"}

    def assert_owned(self) -> None:
        try:
            info = self.root.lstat()
            marker_info = self.marker.lstat()
            valid = (stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
                     and (info.st_dev, info.st_ino) == self.identity
                     and stat.S_ISREG(marker_info.st_mode)
                     and marker_info.st_uid == os.getuid()
                     and json.loads(self.marker.read_text()) == {"token": self.token, "uid": os.getuid()})
            for path in (self.data, self.socket):
                if path.is_symlink() or (path.exists() and not path.is_dir()):
                    valid = False
        except (OSError, ValueError):
            valid = False
        if not valid:
            raise SmokeError("ownership", "refusing foreign/replaced cluster directory")

    def init(self) -> None:
        self.assert_owned()
        if self.data.exists():
            raise SmokeError("initdb", "refusing existing data directory")
        password_file = self.root / "init-password"
        fd = os.open(password_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as stream:
            stream.write(self.password + "\n")
        try:
            execute([str(self.tools["initdb"]), "-D", str(self.data), "-U", OWNER,
                     "--auth=scram-sha-256", "--encoding=UTF8", "--no-locale",
                     f"--pwfile={password_file}"], env=self.env, phase="initdb", cwd=self.root)
        finally:
            password_file.unlink(missing_ok=True)

    def assert_postmaster(self) -> int:
        self.assert_owned()
        pidfile = self.data / "postmaster.pid"
        if pidfile.is_symlink():
            raise SmokeError("ownership", "refusing replaced postmaster pid file")
        try:
            lines = pidfile.read_text().splitlines()
            valid = (int(lines[0]) > 1 and lines[1] == str(self.data)
                     and lines[3] == str(PORT) and lines[4] == str(self.socket))
        except (OSError, ValueError, IndexError):
            valid = False
        if not valid:
            raise SmokeError("ownership", "postmaster identity does not match owned cluster")
        return int(lines[0])

    def start(self) -> None:
        self.assert_owned()
        if (self.data / "postmaster.pid").exists():
            raise SmokeError("start", "refusing existing postmaster")
        options = (f"-c listen_addresses='' -c unix_socket_directories='{self.socket}' "
                   f"-c unix_socket_permissions=0700 -c fsync=on -p {PORT}")
        execute([str(self.tools["pg_ctl"]), "-D", str(self.data), "-l", str(self.root / "postgres.log"),
                 "-o", options, "-w", "-t", "20", "start"], env=self.env, phase="start", timeout=30,
                cwd=self.root)
        self.started = True
        self.assert_postmaster()
        if self.sql("postgres", "show data_directory;") != str(self.data):
            raise SmokeError("ownership", "server data directory mismatch")
        if self.sql("postgres", "show listen_addresses;"):
            raise SmokeError("ownership", "TCP was not disabled")

    def stop(self) -> None:
        self.assert_owned()
        if not (self.data / "postmaster.pid").exists():
            if self.started:
                raise SmokeError("stop", "owned postmaster unexpectedly disappeared")
            return
        self.assert_postmaster()
        for mode in ("fast", "immediate"):
            try:
                execute([str(self.tools["pg_ctl"]), "-D", str(self.data), "-w", "-t", "20",
                         "-m", mode, "stop"], env=self.env, phase="stop", timeout=30, cwd=self.root)
                if (self.data / "postmaster.pid").exists():
                    raise SmokeError("stop", "postmaster pid file remains")
                self.started = False
                return
            except SmokeError:
                if mode == "immediate":
                    raise
                self.assert_postmaster()

    def cleanup(self) -> None:
        self.assert_owned()
        self.stop()
        self.assert_owned()
        shutil.rmtree(self.root)  # Only our marker/inode-validated root, after owned stop.

    def client_env(self, database: str) -> dict[str, str]:
        return {**self.env, "PGHOST": str(self.socket), "PGPORT": str(PORT), "PGUSER": OWNER,
                "PGPASSWORD": self.password, "PGDATABASE": database}

    def sql(self, database: str, query: str) -> str:
        self.assert_postmaster()
        return execute([str(self.tools["psql"]), "-X", "--no-password", "-A", "-t",
                        "-v", "ON_ERROR_STOP=1", "-c", query], env=self.client_env(database),
                       phase="sql", cwd=self.root).stdout.strip()

    def url(self, database: str) -> str:
        if not re.fullmatch(r"mcr_[a-z_]+", database):
            raise SmokeError("url", "database name must use the private mcr_ namespace")
        query = urlencode({"host": str(self.socket), "port": PORT, "sslmode": "disable"})
        value = f"postgresql://{OWNER}:{quote(self.password)}@localhost/{database}?{query}"
        self.validate_url(value, database)
        return value

    def validate_url(self, value: str, database: str) -> None:
        self.assert_owned()
        parsed = urlsplit(value)
        params = parse_qs(parsed.query)
        if not (parsed.scheme == "postgresql" and parsed.hostname == "localhost"
                and parsed.port is None and parsed.username == OWNER
                and parsed.password == self.password and parsed.path == "/" + database
                and re.fullmatch(r"mcr_[a-z_]+", database)
                and params == {"host": [str(self.socket)], "port": [str(PORT)], "sslmode": ["disable"]}):
            raise SmokeError("url", "refusing default/foreign database URL")


def check_ledger(cluster: Cluster, database: str, expected: dict[int, str]) -> int:
    rows = cluster.sql(database, "select version, success, encode(checksum, 'hex') "
                       "from public._sqlx_migrations order by version;")
    actual = {}
    try:
        for line in rows.splitlines():
            version, success, checksum = line.split("|")
            if success != "t" or int(version) in actual:
                raise ValueError
            actual[int(version)] = checksum
    except ValueError:
        raise SmokeError("ledger", "invalid or unsuccessful migration receipt") from None
    if actual != expected:
        raise SmokeError("ledger", "migration versions/checksums/count differ from imported sources")
    return len(actual)


def qualify(cluster: Cluster, binaries: dict, manifests: dict, report: dict) -> None:
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    before = cluster.sql("postgres", "select pg_postmaster_start_time();")
    report["restart"] = {"status": "NOT_RUN", "before_pid": cluster.assert_postmaster()}
    for service, variable in SERVICES.items():
        database = "mcr_" + service.removesuffix("-service").replace("-", "_")
        row = {"service": service, "database": database, "expected_migrations": len(manifests[service]),
               "binary_sha256": hashlib.sha256(binaries[service].read_bytes()).hexdigest(),
               "migration_manifest_sha256": hashlib.sha256(
                   json.dumps(manifests[service], sort_keys=True).encode()).hexdigest(),
               "migrate": "NOT_RUN", "verify": "NOT_RUN", "restart_persistence": "NOT_RUN",
               "verify_after_restart": "NOT_RUN"}
        report["services"].append(row)
        execute([str(cluster.tools["createdb"]), "--no-password", "--maintenance-db=postgres", database],
                env=cluster.client_env("postgres"), phase=f"{service}:createdb", cwd=cluster.root)
        url = cluster.url(database)
        env = {**cluster.env, variable: url}
        for flag, key in [("--migrate", "migrate"), ("--verify-migrations", "verify")]:
            try:
                execute([str(binaries[service]), flag], env=env, phase=f"{service}:{flag}",
                        cwd=cluster.root)
            except SmokeError as error:
                row[key] = "FAIL"
                row[key + "_exit_code"] = error.detail["exit_code"]
                raise
            row[key] = "PASS"
            row[key + "_exit_code"] = 0
        row["ledger_before"] = check_ledger(cluster, database, manifests[service])
        token = secrets.token_hex(24)
        # Independent synthetic table; no inherited user records or seed data are needed.
        cluster.sql(database, "create schema mcr_smoke; create table mcr_smoke.records "
                    f"(token text primary key); insert into mcr_smoke.records values ('{token}');")
        row["_token"] = token
    cluster.stop()
    cluster.start()
    after = cluster.sql("postgres", "select pg_postmaster_start_time();")
    new_pid = cluster.assert_postmaster()
    if before == after or report["restart"]["before_pid"] == new_pid:
        raise SmokeError("restart", "postmaster did not change")
    report["restart"].update(status="PASS", after_pid=new_pid)
    for row in report["services"]:
        service, database = row["service"], row["database"]
        row["restart_persistence"] = "FAIL"
        if cluster.sql(database, "select token from mcr_smoke.records;") != row["_token"]:
            raise SmokeError("persistence", f"smoke record missing or changed: {service}")
        row["ledger_after"] = check_ledger(cluster, database, manifests[service])
        try:
            execute([str(binaries[service]), "--verify-migrations"],
                    env={**cluster.env, SERVICES[service]: cluster.url(database)},
                    phase=f"{service}:verify-after-restart", cwd=cluster.root)
        except SmokeError as error:
            row["verify_after_restart"] = "FAIL"
            row["verify_after_restart_exit_code"] = error.detail["exit_code"]
            raise
        row["restart_persistence"] = "PASS"
        row["verify_after_restart"] = "PASS"
        row["verify_after_restart_exit_code"] = 0


def run(bin_dir: Path, pg_bin_dir: Path) -> dict:
    report = {"scope": "BE02 six inherited migration sets and native PG restart only",
              "result": "FAIL", "services": [], "cleanup": "NOT_RUN",
              "started_at_utc": datetime.now(timezone.utc).isoformat(),
              "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "normal_service_startup": "NOT_RUN", "packaging": "NOT_RUN", "BE44": "NOT_RUN"}
    cluster = None
    try:
        binaries, tools, manifests = preflight(bin_dir, pg_bin_dir)
        revision = execute(["/usr/bin/git", "-C", str(BACKEND), "rev-parse", "HEAD"],
                           env={"PATH": "/usr/bin:/bin"}, phase="source-revision", cwd=BACKEND).stdout.strip()
        report["source_revision"] = revision
        report["binary_directory"] = str(bin_dir)
        report["postgres_tools"] = {name: str(path) for name, path in tools.items()}
        cluster = Cluster(tools)
        qualify(cluster, binaries, manifests, report)
        report["result"] = "PASS"
    except SmokeError as error:
        report["failure"] = error.detail
    except (KeyboardInterrupt, Exception) as error:
        report["failure"] = {"phase": "harness", "diagnostic": type(error).__name__}
    finally:
        if cluster is not None:
            try:
                cluster.cleanup()
                report["cleanup"] = "PASS"
            except (SmokeError, OSError) as error:
                report["cleanup"] = "FAIL"
                report["cleanup_failure"] = (error.detail if isinstance(error, SmokeError)
                                             else {"phase": "cleanup", "diagnostic": type(error).__name__})
                report["retained_owned_directory"] = str(cluster.root)
                report["result"] = "FAIL"
        for row in report["services"]:
            row.pop("_token", None)
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", required=True, type=Path, help="Explicit prebuilt service directory")
    parser.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/bin"))
    parser.add_argument("--report", required=True, type=Path, help="New JSON report path; never overwritten")
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("--report must be an absolute new file path")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("--report must be a new writable file in an existing directory")
    def interrupted(*_):
        raise KeyboardInterrupt

    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        report = run(args.bin_dir, args.pg_bin_dir)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
        print(json.dumps({"result": report["result"], "report": str(args.report),
                          "cleanup": report["cleanup"]}))
        return 0 if report["result"] == "PASS" else 1
    finally:
        signal.signal(signal.SIGTERM, previous)


if __name__ == "__main__":
    raise SystemExit(main())
