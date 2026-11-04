"""Private IN04 issuer input for the real Operations publication worker.

No service, PostgreSQL cluster, role SQL or identity is created here. The caller
supplies its actual existing owned stack, deadline and sanitized real principals.
"""
from __future__ import annotations

from datetime import datetime, timedelta, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import subprocess
import time
from uuid import UUID, uuid4

import dev_stack
import migration_smoke
import in03_stack_runtime

PREFIX = b"MCR-OPERATIONS-ASSIGNMENT-V1\0"
FIELDS = ("version", "statement_id", "issuer", "event_id", "actor_id",
          "expected_revision", "role", "issued_at", "expires_at")
SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")
PRIVATE_PREFIX = bytes.fromhex("302e020100300506032b657004220420")
CLOCK_SQL = """SELECT json_build_object(
 'system_identifier',(SELECT system_identifier::text FROM pg_control_system()),
 'data_directory',current_setting('data_directory'),
 'socket',current_setting('unix_socket_directories'),
 'tcp',inet_server_addr() IS NOT NULL,'database',current_database(),
 'owner',current_user,
 'clock',to_char(clock_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'))::text"""


class RoleRefusal(ValueError):
    """Fixed labels only; never include a path, bearer, key or command output."""


def require(value, label):
    if not value:
        raise RoleRefusal(label)


def canonical_uuid(value):
    try:
        parsed = UUID(value) if isinstance(value, str) else None
    except ValueError:
        parsed = None
    require(parsed is not None and parsed.int != 0 and str(parsed) == value,
            "roles-canonical-identity")
    return value


def timestamp(value):
    require(isinstance(value, str) and re.fullmatch(
        r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,6})?Z", value),
            "roles-server-clock")
    try:
        result = datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        raise RoleRefusal("roles-server-clock") from None
    require(result.utcoffset().total_seconds() == 0, "roles-server-clock")
    return result


def bounded_expiry(epoch):
    # datetime rounds to microseconds. Clamp AFTER conversion: multiplication
    # by1e6 before floor can itself round upward at contemporary epoch sizes.
    expires = datetime.fromtimestamp(epoch, timezone.utc)
    if expires.timestamp() > epoch:
        expires -= timedelta(microseconds=1)
    require(expires.timestamp() <= epoch, "roles-window-expiry")
    return expires


def statement_bytes(statement):
    require(isinstance(statement, dict) and tuple(statement) == FIELDS,
            "roles-statement-fields")
    require(type(statement["version"]) is int and statement["version"] == 1
            and type(statement["expected_revision"]) is int
            and statement["expected_revision"] == 0
            and statement["role"] == "OPERATOR", "roles-statement-policy")
    for name in ("statement_id", "event_id", "actor_id"):
        canonical_uuid(statement[name])
    require(isinstance(statement["issuer"], str)
            and re.fullmatch(r"[A-Za-z0-9_-]{1,64}", statement["issuer"]),
            "roles-statement-issuer")
    issued, expires = timestamp(statement["issued_at"]), timestamp(statement["expires_at"])
    require(issued < expires, "roles-statement-expiry")
    # All strings are restricted ASCII, so this exactly matches Rust struct
    # serde_json::to_vec in declaration order, not sort_keys or pretty JSON.
    return PREFIX + json.dumps(statement, ensure_ascii=False, separators=(",", ":"),
                               allow_nan=False).encode("utf-8")


def file_identity(info):
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns


def private_read(path, limit=262144):
    try:
        before = path.lstat()
        with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as stream:
            opened = os.fstat(stream.fileno())
            require(stat.S_ISREG(opened.st_mode) and opened.st_uid == os.getuid()
                    and opened.st_mode & 0o077 == 0 and opened.st_nlink == 1,
                    "roles-private-file")
            require(opened.st_size <= limit, "roles-private-file-size")
            raw = stream.read(limit + 1)
            require(len(raw) <= limit and file_identity(before) == file_identity(opened)
                    == file_identity(os.fstat(stream.fileno())) == file_identity(path.lstat()),
                    "roles-file-replaced")
        return raw
    except OSError:
        raise RoleRefusal("roles-private-file") from None


def create_private(path, data):
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        require(private_read(path) == data, "roles-file-write")
    except OSError:
        raise RoleRefusal("roles-file-write") from None


class ManagedRoles:
    def __init__(self, stack, deadline, *, expected_system_identifier, window_end_epoch):
        require(type(stack) is dev_stack.DevStack and type(stack.cluster) is migration_smoke.Cluster
                and type(deadline) is in03_stack_runtime.WorkDeadline,
                "roles-owned-stack-required")
        require(isinstance(expected_system_identifier, str)
                and re.fullmatch(r"[1-9][0-9]{0,19}", expected_system_identifier)
                and int(expected_system_identifier) < 2**64, "roles-system-identifier")
        require(isinstance(window_end_epoch, (int, float)) and not isinstance(window_end_epoch, bool)
                and math.isfinite(window_end_epoch), "roles-window-expiry")
        self._stack, self._deadline, self._cluster = stack, deadline, stack.cluster
        self._root, self._root_identity = self._cluster.root, tuple(self._cluster.identity)
        self._system = expected_system_identifier
        require(time.time() < window_end_epoch <= time.time() + deadline.work_monotonic - time.monotonic(),
                "roles-window-expiry")
        self._expires = bounded_expiry(window_end_epoch)
        self._window_end = self._expires.timestamp()
        self._published = None
        self._directory_identity = None
        self._postmaster = None
        self._assert_owner()
        self._postmaster = self._cluster.assert_postmaster()
        self._work_end_epoch = time.time() + deadline.work_monotonic - time.monotonic()
        require("concert-service" not in stack.children, "roles-must-initialize-before-concert")
        now = self._clock()
        expires = self._expires
        require(now < expires and self._window_end <= self._work_end_epoch,
                "roles-window-expiry")
        tool = Path(stack.openssl_binary)
        require(tool.is_absolute() and tool.is_file() and not tool.is_symlink()
                and os.access(tool, os.X_OK) and tool.stat().st_mode & 0o022 == 0,
                "roles-openssl-tool")
        self._openssl, self._tool_identity = tool, file_identity(tool.stat())
        self._directory = self._root / "in04-managed-roles"
        try:
            self._directory.mkdir(mode=0o700, exist_ok=False)
        except OSError:
            raise RoleRefusal("roles-new-private-directory") from None
        info = self._directory.lstat()
        self._directory_identity = info.st_dev, info.st_ino
        self._issuer = "in04_" + uuid4().hex
        self._key = self._directory / "issuer.der"
        self._public = self._directory / "issuer-public.der"
        self._publication = self._directory / "statements.json"
        raw = self._run(["genpkey", "-algorithm", "ED25519", "-outform", "DER"])
        require(len(raw) == 48 and raw.startswith(PRIVATE_PREFIX), "roles-private-key-format")
        create_private(self._key, raw)
        self._key_digest = hashlib.sha256(raw).digest()
        del raw
        public = self._run(["pkey", "-inform", "DER", "-in", str(self._key), "-pubout", "-outform", "DER"])
        require(len(public) == 44 and public.startswith(SPKI_PREFIX)
                and public[len(SPKI_PREFIX):] != bytes(32), "roles-public-key-format")
        create_private(self._public, public)
        self._public_der = public
        self._public_hex = public[len(SPKI_PREFIX):].hex()
        self._replace(b"[]")

    def __repr__(self):
        return "ManagedRoles(<private-owned>)"

    def _remaining(self):
        self._deadline.check()
        remaining = min(self._deadline.work_monotonic - time.monotonic(),
                        self._window_end - time.time())
        require(remaining > 0, "roles-window-expired")
        return min(2.0, remaining)

    def _assert_owner(self):
        require(self._stack.cluster is self._cluster,
                "roles-owner-rebound")
        self._remaining()
        try:
            self._cluster.assert_owned()
            pid = self._cluster.assert_postmaster()
            require(self._postmaster is None or self._postmaster == pid, "roles-postmaster-replaced")
            info = self._root.lstat()
            require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
                    and info.st_mode & 0o077 == 0
                    and (info.st_dev, info.st_ino) == self._root_identity,
                    "roles-root-custody")
            marker = json.loads(private_read(self._cluster.marker, 1024))
            require(marker == {"token": self._cluster.token, "uid": os.getuid()},
                    "roles-owner-marker")
            if self._directory_identity is not None:
                info = self._directory.lstat()
                require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
                        and info.st_mode & 0o077 == 0
                        and (info.st_dev, info.st_ino) == self._directory_identity,
                        "roles-directory-custody")
        except (OSError, ValueError, migration_smoke.SmokeError):
            raise RoleRefusal("roles-owner-custody") from None

    def _query(self, database, query):
        self._assert_owner()
        try:
            result = subprocess.run(
                [str(self._cluster.tools["psql"]), "-X", "--no-password", "-A", "-t", "-v",
                 "ON_ERROR_STOP=1", "-c", query],
                env=self._cluster.client_env(database), cwd=self._root,
                capture_output=True, timeout=self._remaining(), check=False)
            require(result.returncode == 0 and len(result.stdout) <= 4096, "roles-owned-clock-query")
            self._assert_owner()
            return result.stdout.strip()
        except (OSError, subprocess.SubprocessError):
            raise RoleRefusal("roles-owned-query-unavailable") from None

    def _clock(self):
        try:
            row = json.loads(self._query("postgres", CLOCK_SQL))
            require(set(row) == {"system_identifier", "data_directory", "socket", "tcp", "database", "owner", "clock"}
                    and row["system_identifier"] == self._system
                    and row["data_directory"] == str(self._cluster.data)
                    and row["socket"] == str(self._cluster.socket)
                    and row["tcp"] is False and row["database"] == "postgres"
                    and row["owner"] == migration_smoke.OWNER, "roles-owned-clock-identity")
            return timestamp(row["clock"])
        except (ValueError, TypeError):
            raise RoleRefusal("roles-owned-clock-unavailable") from None

    def _run(self, arguments, *, verify=False):
        self._assert_owner()
        try:
            require(not self._openssl.is_symlink()
                    and file_identity(self._openssl.stat()) == self._tool_identity,
                    "roles-openssl-changed")
            result = subprocess.run([str(self._openssl), *arguments], cwd=self._directory,
                env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"}, capture_output=True,
                timeout=self._remaining(), check=False)
            require(len(result.stdout) <= 4096 and result.returncode == 0,
                    "roles-signature-invalid" if verify else "roles-openssl-failed")
            return result.stdout
        except (OSError, subprocess.SubprocessError):
            raise RoleRefusal("roles-openssl-failed") from None

    def _replace(self, value):
        self._assert_owner()
        if self._publication.exists() or self._publication.is_symlink():
            private_read(self._publication)
        temporary = self._directory / ("publication-" + uuid4().hex)
        try:
            create_private(temporary, value)
            self._assert_owner()
            os.replace(temporary, self._publication)
            directory = os.open(self._directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
            require(private_read(self._publication) == value, "roles-publication-write")
        except OSError:
            raise RoleRefusal("roles-publication-write") from None
        finally:
            if temporary.exists():
                self._assert_owner()
                temporary.unlink()

    def _check_keys(self):
        require(hashlib.sha256(private_read(self._key, 256)).digest() == self._key_digest
                and private_read(self._public, 256) == self._public_der, "roles-key-replaced")

    def environment(self):
        require(self._clock().timestamp() < self._window_end, "roles-window-expiry")
        self._check_keys()
        require(private_read(self._publication) == (self._published[1] if self._published else b"[]"),
                "roles-publication-replaced")
        return {"CONCERT_OPERATIONS_MODE": "live",
                "CONCERT_OPERATIONS_ISSUER_KEYS_JSON": json.dumps(
                    [{"issuer": self._issuer, "public_key_hex": self._public_hex}], separators=(",", ":")),
                "CONCERT_OPERATIONS_PUBLICATION_FILE": str(self._publication)}

    def publish_operator(self, event_id, principal_records, operator_id):
        """Only the third existing real HTTP principal receives the one signed grant.

        Root derives exactly three `{user_id, credentials_path}` records in RAM
        from its real HTTP principal tuples; no bearer is passed to this helper.
        """
        canonical_uuid(event_id)
        canonical_uuid(operator_id)
        self._assert_owner()
        require(isinstance(principal_records, (list, tuple)) and len(principal_records) == 3,
                "roles-three-real-principals")
        users = []
        for ordinal, principal in enumerate(principal_records, 1):
            require(isinstance(principal, dict)
                    and set(principal) == {"user_id", "credentials_path"},
                    "roles-principal-record")
            user, path = principal["user_id"], principal["credentials_path"]
            canonical_uuid(user)
            require(isinstance(path, (str, Path)), "roles-principal-file-binding")
            require(Path(path) == self._root / f"in01-principal-{ordinal}.json",
                    "roles-principal-file-binding")
            try:
                login = json.loads(private_read(Path(path), 8192))
            except (ValueError, TypeError):
                raise RoleRefusal("roles-principal-record") from None
            require(isinstance(login, dict) and set(login) == {"user_id", "email", "password", "login_path", "access_token"}
                    and login.get("user_id") == user and login.get("access_token") == "MEMORY_ONLY"
                    and login.get("login_path") == "/api/auth/login", "roles-principal-file-binding")
            email = login.get("email")
            require(isinstance(email, str) and 1 <= len(email) <= 320
                    and len(email.encode("utf-8")) <= 1024, "roles-principal-email")
            # Only validated canonical UUIDs and hexadecimal UTF-8 are inserted
            # into these fixed read-only probes. No secret/claim/role SQL write.
            encoded_email = email.encode("utf-8").hex()
            auth = ("SELECT EXISTS(SELECT 1 FROM public.users u "
                "JOIN public.user_identities i ON i.user_id=u.id "
                f"WHERE u.id='{user}'::uuid AND u.status='ACTIVE' AND u.deleted_at IS NULL "
                f"AND u.primary_email=convert_from(decode('{encoded_email}','hex'),'UTF8') "
                "AND i.email=u.primary_email AND i.email_verified AND i.deleted_at IS NULL "
                "AND EXISTS(SELECT 1 FROM public.login_audit a WHERE a.user_id=u.id AND a.success))")
            require(self._query("mcr_auth", auth) == b"t", "roles-actual-auth-principal")
            admission = ("SELECT EXISTS(SELECT 1 FROM public.concert_event_admissions "
                f"WHERE event_id='{event_id}'::uuid AND user_id='{user}'::uuid "
                "AND state='ACTIVE' AND restriction_revision>0)")
            require(self._query("mcr_concert", admission) == b"t", "roles-actual-event-admission")
            users.append(user)
        require(len(set(users)) == 3, "roles-distinct-principals")
        require(operator_id == users[2], "roles-exact-third-operator")
        binding = (event_id, tuple(users))
        now = self._clock()
        expires = self._expires
        require(now < expires and expires.timestamp() <= self._work_end_epoch,
                "roles-window-expiry")
        if self._published is not None:
            require(self._published[0] == binding and private_read(self._publication) == self._published[1],
                    "roles-publication-replay-conflict")
            return dict(self._published[2])
        require(private_read(self._publication) == b"[]", "roles-initial-publication-required")
        statement = dict(zip(FIELDS, (1, str(uuid4()), self._issuer, event_id, users[2], 0,
            "OPERATOR", now.isoformat(timespec="microseconds").replace("+00:00", "Z"),
            expires.isoformat(timespec="microseconds").replace("+00:00", "Z"))))
        payload = statement_bytes(statement)
        self._check_keys()
        message, signature = self._directory / "statement.bin", self._directory / "signature.bin"
        create_private(message, payload)
        signed = self._run(["pkeyutl", "-sign", "-rawin", "-keyform", "DER", "-inkey",
                            str(self._key), "-in", str(message)])
        require(len(signed) == 64 and signed != bytes(64), "roles-signature-format")
        create_private(signature, signed)
        self._run(["pkeyutl", "-verify", "-rawin", "-pubin", "-keyform", "DER", "-inkey",
                   str(self._public), "-sigfile", str(signature), "-in", str(message)], verify=True)
        # Recheck the database clock/custody after signing, not merely host time.
        require(self._clock() < expires, "roles-window-expiry")
        value = json.dumps([{"statement": statement, "signature_hex": signed.hex()}],
                           ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode()
        self._replace(value)
        receipt = {"statement_id": statement["statement_id"], "event_id": event_id,
                   "actor_id": users[2], "role": "OPERATOR", "expected_revision": 0,
                   "expires_at": statement["expires_at"], "applied": "NOT_OBSERVED"}
        self._published = binding, value, receipt
        return dict(receipt)
