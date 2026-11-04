#!/usr/bin/env python3
"""Bounded original-IN03 seven-service stack; existing FE209 owns the journey.

Native startup is never a fallback for missing qualified binaries or feature
configuration. This module's pure input guards do not start a service on import.
"""
from __future__ import annotations

from dataclasses import dataclass
import argparse
from contextlib import ExitStack
from datetime import datetime, timezone
import hashlib
import importlib
import json
import math
import os
from pathlib import Path
import re
import secrets
import signal
import stat
import subprocess
import sys
import time
from typing import Mapping
from unittest.mock import patch
from uuid import uuid4


SERVICES = frozenset({"api-gateway", "concert-service", "post-service",
                      "notification-service", "auth-service", "friend-service",
                      "profile-service"})
ADVANCED_SERVICES = frozenset({"api-gateway", "concert-service", "post-service",
                               "notification-service"})
ROLES = ("requester", "recipient", "restricted")


class InputRefusal(ValueError):
    """Public diagnostics contain only fixed guard labels, never input values."""


def require(condition: bool, label: str) -> None:
    if not condition:
        raise InputRefusal(label)


def source_revision(value: str) -> str:
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value) is not None,
            "source-revision-required")
    return value


def identity(info):
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns


def read_owned(path: Path, *, private=False, executable=False, limit=None):
    """No leaf-link following, open/stat identity and bounded regular-file reads."""
    require(path.is_absolute() and not path.is_symlink(), "explicit-file-path-required")
    try:
        with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as stream:
            info = os.fstat(stream.fileno())
            require(stat.S_ISREG(info.st_mode), "regular-file-required")
            if private:
                require(info.st_uid == os.getuid() and info.st_mode & 0o077 == 0,
                        "private-input-custody-required")
            if executable:
                require(os.access(path, os.X_OK), "binary-not-executable")
                require(info.st_mode & 0o022 == 0, "writable-binary-refused")
            require(limit is None or info.st_size <= limit, "private-input-too-large")
            data = stream.read() if limit is None else stream.read(limit + 1)
            require(limit is None or len(data) <= limit, "private-input-too-large")
            require(identity(info) == identity(os.fstat(stream.fileno()))
                    == identity(path.lstat()), "file-changed-during-read")
            return data, info
    except OSError:
        raise InputRefusal("input-unavailable") from None


@dataclass(frozen=True)
class BinaryPin:
    path: Path
    sha256: str
    source: str

    def validate(self) -> dict:
        source_revision(self.source)
        require(isinstance(self.sha256, str) and re.fullmatch(r"[0-9a-f]{64}", self.sha256) is not None,
                "binary-hash-required")
        # Stream the binary: no open handle leak or whole executable allocation.
        require(self.path.is_absolute() and not self.path.is_symlink(), "explicit-binary-path-required")
        try:
            with os.fdopen(os.open(self.path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as stream:
                info = os.fstat(stream.fileno())
                require(stat.S_ISREG(info.st_mode) and os.access(self.path, os.X_OK), "binary-not-executable")
                require(info.st_mode & 0o022 == 0, "writable-binary-refused")
                digest = hashlib.sha256()
                for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(chunk)
                digest = digest.hexdigest()
                require(identity(info) == identity(os.fstat(stream.fileno()))
                        == identity(self.path.lstat()), "binary-changed-during-read")
        except OSError:
            raise InputRefusal("binary-unavailable") from None
        require(digest == self.sha256, "binary-hash-mismatch")
        return {"sha256": digest, "source": self.source,
                "identity": list(identity(info))}


def validate_binary_cohort(pins: Mapping[str, BinaryPin]) -> dict:
    require(set(pins) == SERVICES, "exact-seven-service-cohort-required")
    require(len({pin.path for pin in pins.values()}) == len(SERVICES), "distinct-service-binaries-required")
    values = {name: pins[name].validate() for name in sorted(SERVICES)}
    require(len({tuple(v["identity"][:2]) for v in values.values()}) == len(SERVICES),
            "distinct-service-inodes-required")
    return values


def private_input(path: Path) -> bytes:
    return read_owned(path, private=True, limit=1024 * 1024)[0]


@dataclass(frozen=True)
class WorkDeadline:
    work_monotonic: float
    release_monotonic: float

    def __post_init__(self):
        require(all(isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)
                    for x in (self.work_monotonic, self.release_monotonic))
                and self.work_monotonic < self.release_monotonic, "allocation-deadline-invalid")

    def check(self, *, cleanup: bool = False) -> None:
        require(time.monotonic() < (self.release_monotonic if cleanup else self.work_monotonic),
                "allocation-release-expired" if cleanup else "allocation-work-expired")


def closed(value, keys, label):
    require(isinstance(value, dict) and set(value) == set(keys), label)
    return value


def json_object(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate-json-key")
            result[key] = value
        return result
    try:
        return json.loads(raw, object_pairs_hook=pairs,
                          parse_constant=lambda _: require(False, "nonfinite-json-number"))
    except (ValueError, UnicodeError):
        raise InputRefusal("invalid-json-input") from None


def git(root, *args):
    try:
        return subprocess.run(["/usr/bin/git", "-C", str(root), *args],
                              env={"PATH": "/usr/bin:/bin", "GIT_OPTIONAL_LOCKS": "0"},
                              capture_output=True, check=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        raise InputRefusal("pinned-source-unavailable") from None


def git_paths(root, revision, directory):
    source_revision(revision)
    return git(root, "ls-tree", "-r", "--name-only", revision, "--", directory).decode().splitlines()


def source_bytes(root, revision, name):
    require(not Path(name).is_absolute() and ".." not in Path(name).parts and ":" not in name,
            "source-path-invalid")
    return git(root, "show", source_revision(revision) + ":" + name)


def validate_sources(root, revision):
    """Pin the existing helper closure and release files without changing a peer tree."""
    require(root.is_absolute() and root.is_dir(), "explicit-source-root-required")
    directory = "backend/scripts/mcr"
    names = [p for p in git_paths(root, revision, directory) if p.endswith(".py")]
    actual = {str(p.relative_to(root)) for p in (root / directory).rglob("*.py")}
    require(set(names) == actual, "helper-source-inventory-changed")
    names += git_paths(root, revision, "backend/releases/operator-bundle/quality-v6")
    names += ["backend/services/concert-service/src/config/schema-probe.sql"]
    hashes = {}
    for name in names:
        raw, _ = read_owned(root / name)
        require(raw == source_bytes(root, revision, name), "pinned-source-bytes-changed")
        hashes[name] = hashlib.sha256(raw).hexdigest()
    return hashes


def migration_inputs(root, pins):
    """SQLx SHA-384 manifests from each executable's own immutable source, not HEAD."""
    result = {}
    for name, pin in pins.items():
        directory = f"backend/services/{name}/migrations"
        paths = git_paths(root, pin.source, directory)
        require(bool(paths), "migration-source-missing")
        manifest = {}
        for path in paths:
            match = re.fullmatch(r"([0-9]+)_[^/]+\.sql", Path(path).name)
            require(match is not None, "migration-source-name-invalid")
            version = int(match[1])
            require(version not in manifest, "migration-source-version-duplicate")
            manifest[version] = hashlib.sha384(source_bytes(root, pin.source, path)).hexdigest()
        result[name] = manifest
    return result


def admitted_cohort(records):
    require(isinstance(records, dict) and set(records) == SERVICES, "exact-seven-service-cohort-required")
    pins, evidence = {}, {}
    for name, record in records.items():
        closed(record, ("path", "sha256", "source", "qualification", "evidence"), "binary-record-invalid")
        expected = "IN03_ADVANCED_COMBINED" if name in ADVANCED_SERVICES else "WORLD_AUTH_FOUNDATION"
        require(record["qualification"] == expected, "binary-qualification-mismatch")
        pin = BinaryPin(Path(record["path"]), record["sha256"], record["source"])
        receipt = closed(record["evidence"], ("path", "sha256"), "binary-evidence-invalid")
        raw = private_input(Path(receipt["path"]))
        # Evidence is producer-owned source/build provenance, never authenticated C03.
        value = closed(json_object(raw), ("service", "binary_sha256", "source_revision", "result"),
                       "binary-evidence-schema")
        require(value.get("service") == name
                and value.get("binary_sha256") == pin.sha256 and value.get("source_revision") == pin.source
                and value.get("result") == "PASS", "binary-evidence-binding-mismatch")
        require(hashlib.sha256(raw).hexdigest() == receipt["sha256"], "binary-evidence-hash-mismatch")
        pins[name], evidence[name] = pin, receipt["sha256"]
    return pins, {"binaries": validate_binary_cohort(pins), "evidence_sha256": evidence}


def advanced_sources(root, pins):
    """Reject pre-composition World sources even if a caller relabels their receipt.

    These source guards are necessary, not proof a binary was built from them;
    the named publisher retains that build provenance and runtime qualification.
    """
    requirements = {
        "concert-service": {"src/main.rs": ("SocialStartupSettings::load", "FeedRuntimeSettings::load",
            "IntroRuntimeSettings::load", "NotificationTargetRuntimeSettings::load", "SocialDeliveryRuntime")},
        "post-service": {"src/main.rs": ("ReadStartup::load",)},
        "notification-service": {"src/main.rs": ("ConcertSettings::load",)},
        "api-gateway": {"src/adapters/in/http/concert_product/mod.rs":
            ("createConcertSocialRequest", "feature_routes::router()")},
    }
    for name, files in requirements.items():
        for path, symbols in files.items():
            raw = source_bytes(root, pins[name].source, f"backend/services/{name}/{path}")
            require(all(symbol.encode() in raw for symbol in symbols), "advanced-composition-source-required")


def managed_features(envs, concert, urls, signing_pairs):
    """Exact production parser names; ephemeral local keys, no inherited overrides."""
    post_read, post_callback = signing_pairs
    concert.update(CONCERT_INTRO_MODE="live", CONCERT_SOCIAL_MODE="live",
                   CONCERT_NOTIFICATION_INBOX_MODE="live", CONCERT_NOTIFICATION_TARGET_MODE="live",
                   CONCERT_PUBLIC_FEED_MODE="read", CONCERT_OWNER_MEDIA_MODE="off",
                   CONCERT_NOTIFICATION_GRPC_ENDPOINT=urls["notification"],
                   CONCERT_POST_GRPC_ENDPOINT=urls["post"],
                   CONCERT_POST_READ_SIGNING_SEED=post_read[0],
                   POST_CONCERT_ROOT_READ_VERIFYING_KEYS=post_callback[1],
                   CONCERT_SOCIAL_REQUEST_TTL_SECONDS="300")
    for key in ("CONCERT_INTRO_CURSOR_KEY_HEX", "CONCERT_SOCIAL_CURSOR_KEY_HEX", "CONCERT_FEED_CURSOR_KEY_HEX"):
        concert[key] = secrets.token_hex(32)
    for key in ("CONCERT_INTRO_RECEIPT_KEYS_JSON", "CONCERT_PICKUP_KEYS_JSON",
                "CONCERT_SOCIAL_HANDLE_KEYS_JSON", "CONCERT_SOCIAL_COMMAND_KEYS_JSON"):
        concert[key] = json.dumps([{"id": "in03_ephemeral", "secret_hex": secrets.token_hex(32)}])
    envs["post-service"].update(POST_CONCERT_READ_MODE="read", POST_DB_POOL_MAX_CONNECTIONS="5",
        POST_CONCERT_READ_AUTH_GRPC_ENDPOINT=urls["auth"], POST_FRIEND_GRPC_ENDPOINT=urls["friend"],
        POST_CONCERT_ROOT_GRPC_ENDPOINT=urls["concert"], CONCERT_POST_READ_VERIFYING_KEYS=post_read[1],
        POST_CONCERT_ROOT_READ_SIGNING_SEED=post_callback[0])
    envs["notification-service"].update(NOTIFICATION_CONCERT_MODE="live",
        NOTIFICATION_CONCERT_AUTH_GRPC_ENDPOINT=urls["auth"],
        NOTIFICATION_CONCERT_FRIEND_GRPC_ENDPOINT=urls["friend"],
        NOTIFICATION_CONCERT_ROOT_GRPC_ENDPOINT=urls["concert"])
    validate_modes(envs, concert)


def validate_modes(envs, concert):
    expected = {"CONCERT_INTRO_MODE": "live", "CONCERT_SOCIAL_MODE": "live",
                "CONCERT_NOTIFICATION_INBOX_MODE": "live", "CONCERT_NOTIFICATION_TARGET_MODE": "live",
                "CONCERT_PUBLIC_FEED_MODE": "read", "CONCERT_OWNER_MEDIA_MODE": "off"}
    require(all(concert.get(k) == v for k, v in expected.items())
            and envs["post-service"].get("POST_CONCERT_READ_MODE") == "read"
            and envs["notification-service"].get("NOTIFICATION_CONCERT_MODE") == "live",
            "actual-managed-mode-mismatch")
    forbidden = ("ACCEPTED_CLAIM", "AGE_PROVIDER", "OPERATOR_GRANT", "CLOCK_OVERRIDE")
    require(not any(any(word in key for word in forbidden) for key in concert), "unqualified-provider-override")
    for env in [*envs.values(), concert]:
        require(all(v.startswith("https://127.0.0.1:") for k, v in env.items()
                    if k.endswith("_GRPC_ENDPOINT")), "managed-mtls-endpoint-required")


def seven_readiness(status, body, service, foundation_readiness):
    if service != "concert-service":
        return foundation_readiness(status, body, service)
    value = closed(json_object(body), ("ready", "degraded", "draining", "checks"), "concert-ready-schema")
    require(all(type(value[k]) is bool for k in ("ready", "degraded", "draining")), "concert-ready-flags")
    check = closed(closed(value["checks"], ("database",), "concert-ready-checks")["database"],
                   ("required", "status"), "concert-ready-database")
    require(check["required"] is True and check["status"] in ("up", "down", "timed_out", "not_checked"),
            "concert-ready-database")
    ready = not value["draining"] and check["status"] == "up"
    require(value["ready"] == ready and value["degraded"] == (check["status"] != "up")
            and status == (200 if ready else 503), "concert-ready-consistency")
    return {"http_status": status, "ready": ready, "degraded": value["degraded"], "draining": value["draining"],
            "checks": {"database": {"required": True, "status": check["status"]}}}


def load_helpers(root):
    # CLI only, after source validation. The import itself starts no services.
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(root / "backend/scripts/mcr"))
    modules = [importlib.import_module(name) for name in ("world_in01_live", "world_in01_prepare", "dev_stack")]
    require(all(Path(m.__file__).resolve().is_relative_to(root.resolve()) for m in modules),
            "helper-import-source-mismatch")
    return modules


def cleanup_owned(stack, control, guard):
    """Every cleanup attempt runs even if a control close or SQL observation fails."""
    result = {}
    if control is not None:
        try:
            control.server_close()
        except BaseException:
            result["control_cleanup"] = "FAIL"
    result["resource_bounds"] = guard.close()
    try:
        if stack.cluster is not None and stack.cluster.started:
            result["prepared_before_cleanup"] = stack.cluster.sql("postgres", "SELECT count(*) FROM pg_prepared_xacts")
    except BaseException:
        result["prepared_before_cleanup"] = "UNAVAILABLE"
    try:
        stack.close()
    except BaseException:
        stack.report["cleanup"] = "FAIL"
    result["cleanup"] = stack.report.get("cleanup", "FAIL")
    result["process_stops"] = stack.report.get("process_stops", [])
    if result.get("control_cleanup") == "FAIL":
        result["cleanup"] = "FAIL"
    return result


def auth_provenance(stack, browser, report, principals, event, revision, auth_source, expires_at, world):
    """Private producer receipt. Canonical C03 is validated in RAM and discarded."""
    from world_capacity_support.transport import Contracts
    from world_capacity_support.report import json_value
    actors = {}
    for ordinal, (role, (user, path, token)) in enumerate(zip(ROLES, principals), 1):
        raw = world.public_json(browser, report, f"principal-{ordinal}-canonical-bootstrap",
            f"/api/concert/events/{event}/bootstrap", token, response_bytes=True)
        require(Contracts().bootstrap(json_value(raw), event) == user, "actual-bootstrap-owner")
        # Read only the existing real signup credential file; never copy a bearer.
        login = json_object(private_input(path))
        require(login["user_id"] == user and login.get("access_token") == "MEMORY_ONLY",
                "canonical-login-file-mismatch")
        statuses = {label: report["statuses"].get(f"principal-{ordinal}-{suffix}")
                    for label, suffix in (("signupStatus", "signup"), ("verifyStatus", "verify"),
                                          ("loginStatus", "login"))}
        require(all(value == 200 for value in statuses.values()), "actual-auth-http-status")
        actors[role] = {"userId": user, "email": login["email"], "eventId": event,
            "method": "GATEWAY_SIGNUP_HTTP_MAIL_VERIFY_LOGIN", **statuses, "bootstrapStatus": 200}
        del raw
    return {"version": "mcr-local-real-auth/v1", "kind": "CANONICAL_GATEWAY_PRIVATE_HTTP_MAIL",
        "sourceRevision": revision, "binaryRevision": auth_source, "tool": "world_in01_prepare.py",
        "mailOwner": stack.cluster.token, "controlOwner": stack.cluster.token,
        "origin": browser.origin, "eventId": event, "issuedAt": datetime.now(timezone.utc).isoformat(),
        "expiresAt": datetime.fromtimestamp(expires_at, timezone.utc).isoformat(),
        "custody": "PRIVATE_MODE_0600", "actors": actors}


def run(args):
    config = closed(json_object(private_input(args.inputs)),
        ("version", "helper_source", "binaries", "pg_bin_dir", "redis_binary", "openssl_binary"), "stack-input-schema")
    require(config["version"] == 1, "stack-input-version")
    source = closed(config["helper_source"], ("root", "revision"), "helper-source-schema")
    root, revision = Path(source["root"]), source_revision(source["revision"])
    sources = validate_sources(root, revision)
    pins, provenance = admitted_cohort(config["binaries"])
    advanced_sources(root, pins)
    manifests = migration_inputs(root, pins)
    world, prep, dev = load_helpers(root)
    dev.local_origin(args.browser_origin)
    require(args.report.is_absolute() and args.report.parent.is_dir(), "explicit-report-required")
    stack = dev.DevStack(Path("/unused-explicit-pins"), Path(config["pg_bin_dir"]),
                         Path(config["redis_binary"]), Path(config["openssl_binary"]), args.browser_origin)
    guard = world.QualificationGuard(args, stack)
    require(guard.deadline > time.monotonic(), "allocation-work-expired")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"scope": "IN03-STACK-01 actual seven-service launcher", "result": "FAIL", "statuses": {},
              "original_in03_journey": "NOT_RUN", "acceptance": "NOT_CLAIMED",
              "source_revision": revision, "helper_sha256": sources, "cohort": provenance,
              "allocation_id": args.allocation_id, "full_c03_retained_or_hashed": False,
              "providers": "NO_VERIFIED_AGE_OR_OPERATOR_PROVIDER; missing providers remain unavailable"}
    context, principals, control = {}, [], None
    original_envs = dev.development_environments
    runtime = dev.runtime
    readiness = lambda status, body, name: seven_readiness(status, body, name, prep.current_readiness)

    def check_inputs():
        require(admitted_cohort(config["binaries"])[1] == provenance, "binary-cohort-changed")
        require(validate_sources(root, revision) == sources, "helper-source-changed")
        guard.check()

    def preflight(_bin_dir, pg_dir):
        check_inputs()
        require(pg_dir.is_absolute(), "explicit-pg-tools-required")
        tools = {name: dev.migration.executable(pg_dir / name) for name in dev.migration.PG_TOOLS}
        return ({name: pin.path for name, pin in pins.items() if name != "concert-service"}, tools,
                {name: manifest for name, manifest in manifests.items() if name != "concert-service"})

    def environments(cluster, ports, keys, credentials, *rest):
        credentials["CONCERT_SERVICE_CREDENTIAL"] = secrets.token_hex(32)
        envs = original_envs(cluster, ports, keys, credentials, *rest)
        cluster.assert_owned()
        cluster.stop()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=128\n")
        cluster.start()
        require(cluster.sql("postgres", "SHOW max_prepared_transactions") == "128", "owned-prepared-config")
        world.pki(cluster)
        relationship, self_profile, decision, post_read, post_callback = world.joined.keys(cluster, 5)
        for kind in ("grpc", "metrics", "world"):
            ports.reserve("concert-service:" + kind)
        urls = {role: "https://" + ports.address(role + "-service:grpc")
                for role in ("auth", "friend", "profile", "post", "notification", "concert")}
        for name, env in envs.items():
            for key, value in list(env.items()):
                if key.endswith("_GRPC_ENDPOINT"):
                    require(value.startswith("http://127.0.0.1:"), "owned-pre-tls-endpoint")
                    env[key] = value.replace("http://", "https://", 1)
            env.update(world.tls(cluster, runtime.PREFIX[name].lower()))
            env["CONCERT_RELATIONSHIP_VERIFYING_KEYS"] = relationship[1]
        for role in ("auth", "friend"):
            envs[role + "-service"].update(CONCERT_OWNER_AUTHORITY_ENABLED="true",
                AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT=urls["concert"],
                AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT=urls["profile"])
        envs["profile-service"].update(CONCERT_PROFILE_SELF_VERIFYING_KEYS=self_profile[1],
            PROFILE_PARTICIPANT_DECISION_SIGNING_SEED=decision[0], PROFILE_SELF_AUTH_GRPC_ENDPOINT=urls["auth"],
            PROFILE_SELF_FRIEND_GRPC_ENDPOINT=urls["friend"], PROFILE_SELF_CONCERT_GRPC_ENDPOINT=urls["concert"])
        # Reuse the existing two-room owned catalog profile (2+2 seats) for three
        # real actors. This launches no QA05 probe and makes no QA05 claim.
        path = prep.seed_concert(stack, pins["concert-service"].path, report, True,
                                 profile="qa05", runtime_manifest=manifests["concert-service"])
        inputs = json_object(private_input(path))
        release = inputs.pop("release_inputs")
        inputs.pop("world_activation")
        event, owner, rooms = (inputs.pop(k) for k in
            ("CONCERT_WORLD_EVENT_ID", "CONCERT_WORLD_OWNER_ID", "CONCERT_WORLD_ROOMS_JSON"))
        concert = {**cluster.env, **credentials, **inputs, **world.tls(cluster, "concert"),
            **{k: v for k, v in release.items() if k.startswith("CONCERT_")},
            "ACTOR_DELEGATION_VERIFYING_KEYS": keys["ACTOR_DELEGATION_VERIFYING_KEYS"],
            "CONCERT_GRPC_ADDR": ports.address("concert-service:grpc"),
            "CONCERT_METRICS_ADDR": ports.address("concert-service:metrics"),
            "CONCERT_RELATIONSHIP_SIGNING_SEED": relationship[0], "CONCERT_PROFILE_SELF_SIGNING_SEED": self_profile[0],
            "PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS": decision[1],
            "CONCERT_PARTICIPANT_MODE": "live", "CONCERT_PRODUCER_INSTANCE_ID": str(uuid4()),
            "CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON": json.dumps([
                {"id": "in03_ephemeral", "secret_hex": secrets.token_hex(32)}]),
            "CONCERT_WORLD_LISTEN_ADDR": ports.address("concert-service:world"),
            "CONCERT_WORLD_ALLOWED_ORIGINS": json.dumps([args.browser_origin]),
            "CONCERT_WORLD_OWNER_ID": owner, "CONCERT_WORLD_INVALIDATION_CONSUMER_ID": owner,
            "CONCERT_WORLD_EVENT_ID": event, "CONCERT_WORLD_ROOMS_JSON": rooms,
            "CONCERT_WORLD_RESOURCE_ROOT": release["asset_root"],
            "CONCERT_WORLD_SOURCE_BINDING_FILE": release["source_binding"],
            "TOKIO_WORKER_THREADS": "2", "RUST_LOG": "warn"}
        for role in ("auth", "friend", "profile"):
            concert["CONCERT_PARTICIPANT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
            concert["CONCERT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
        managed_features(envs, concert, urls, (post_read, post_callback))
        inspected = dev.migration.execute([str(pins["concert-service"].path), "--inspect-product-release"],
            env=concert, cwd=cluster.root, phase="in03-active-release-inspection")
        match = re.fullmatch(r"CONCERT_PRODUCT_RELEASE_DIGEST=([0-9a-f]{64})\n?", inspected.stdout)
        require(match is not None, "actual-release-inspection")
        concert["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = match[1]
        envs["api-gateway"].update(GATEWAY_CONCERT_GRPC_ENDPOINT=urls["concert"], GATEWAY_CONCERT_RELEASE_DIGEST=match[1])
        validate_modes(envs, concert)
        context.update(env=concert, event=event, rooms=json.loads(rooms), digest=match[1], release=release)
        return envs

    previous = {sig: signal.signal(sig, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
                for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        guard.start()
        guard.phase("seven-service-native-startup", args.startup_timeout_seconds)
        with ExitStack() as patches:
            patches.enter_context(patch.object(runtime, "readiness_value", readiness))
            patches.enter_context(patch.object(dev.migration, "preflight", preflight))
            patches.enter_context(patch.object(dev, "development_environments", environments))
            descriptor = stack.start()
        # Register Concert in the same cleanup owner BEFORE listener qualification.
        env = context["env"]
        stack.children["concert-service"] = runtime.Child(stack.cluster, "concert-service",
            [str(pins["concert-service"].path)], env, stack.ports,
            tuple("concert-service:" + kind for kind in ("grpc", "metrics", "world")), tuple(env.values()))
        stack.children["concert-service"].wait_listeners()
        stack.rows["concert-service"] = {"service": "concert-service"}
        stack.report["services"].append(stack.rows["concert-service"])
        with patch.object(runtime, "readiness_value", readiness):
            stack.health()
        guard.phase("real-auth-and-public-c03", args.provision_timeout_seconds)
        browser = prep.auth.Browser(descriptor["gateway_url"], report)
        browser.origin = args.browser_origin
        provision_deadline = min(guard.deadline, time.monotonic() + args.provision_timeout_seconds)
        for ordinal in range(1, 4):
            guard.check()
            item = prep.principal(stack, browser, ordinal, deadline=provision_deadline)
            require(item[0] not in {p[0] for p in principals}, "distinct-canonical-principals")
            principals.append(item)
        world.initialize_profiles(browser, report, principals)
        expires_at = min(time.time() + args.hold_seconds, args.allocation_work_deadline_epoch)
        producer = auth_provenance(stack, browser, report, principals, context["event"], revision,
                                  pins["auth-service"].source, expires_at, world)
        report["public_projection"] = world.retain_public_projection(browser, report, context["event"],
            principals[0], args.report.parent, {"helper_source": revision, "binary_sources":
                {name: pin.source for name, pin in pins.items()}, "original_in03_journey": "NOT_RUN"})
        # Full C03 exists only in retain_public_projection's local memory. No full-body copy or digest here.
        check_inputs()
        control = dev.Control(stack)
        private = {**stack.descriptor(), "browser_origin": args.browser_origin, "event_id": context["event"],
            "auth_http": "Gateway /api/auth/*; canonical private local HTTP mail fixture",
            "rooms": context["rooms"], "release_digest": context["digest"], "release_inputs": context["release"],
            "principals": {role: str(item[1]) for role, item in zip(ROLES, principals)},
            "authProducer": producer,
            "principal_provenance": {"kind": "CANONICAL_AUTH_PRIVATE_LOCAL_MAIL_SINK",
                "supplier_revision": revision, "supplier_path": "backend/scripts/mcr/world_in01_prepare.py",
                "auth_binary_source": pins["auth-service"].source,
                "auth_binary_sha256": pins["auth-service"].sha256,
                "steps": ["Gateway signup", "Auth private mail delivery", "Gateway verification", "Gateway login"],
                "users": {role: item[0] for role, item in zip(ROLES, principals)}, "tokens": "MEMORY_ONLY"},
            "world_ws_url": "ws://" + stack.ports.address("concert-service:world") + "/world-ws",
            "lifetime": "REMOVED_ON_OWNED_SHUTDOWN", "fault_control": "OWNER_STOP_ONLY",
            "fidelity": "NOT_BYTE_EXACT_FULL_C03", "original_in03_journey": "NOT_RUN",
            "source": {"helperRevision": revision, "backendRevisions": {n: p.source for n, p in pins.items()},
                "authProducerRevision": revision, "authBinaryRevision": pins["auth-service"].source},
            "runtime": {"controlOwner": stack.cluster.token, "origin": args.browser_origin,
                "eventId": context["event"], "expiresAt": producer["expiresAt"], "private": True},
            "missing_original_prerequisites": ["same-origin frontend TS server", "production availability module",
                "provider-qualified Intro/Post/server-expired request resources", "actual contract denial dispositions"],
            "expires_at_epoch": expires_at,
            "allocation_id": args.allocation_id}
        private["services"]["concert-service"] = {
            kind: stack.ports.url("concert-service:" + kind) for kind in ("grpc", "metrics", "world")}
        handoff = stack.cluster.root / "in03-live-inputs.json"
        prep.private_json(handoff, private)
        prep.private_json(args.report.with_suffix(".ready.json"), {"state": "READY", "private_inputs": str(handoff),
            "full_c03_retained_or_hashed": False, "original_in03_journey": "NOT_RUN"})
        print(json.dumps({"state": "READY", "private_inputs": str(handoff), "original_in03_journey": "NOT_RUN"}), flush=True)
        report.update(result="PARTIAL", private_inputs=str(handoff), actual_seven_service_readiness="PASS")
        guard.phase("existing-fe209-original-in03-consumer", args.hold_seconds)
        end = min(guard.deadline, time.monotonic() + args.hold_seconds)
        while time.monotonic() < end and not control.stopping:
            guard.check()
            with patch.object(runtime, "readiness_value", readiness):
                stack.health()
                control.handle_request()
    except BaseException as error:
        report.update(result="FAIL", failure=error.args[0] if isinstance(error, InputRefusal)
                      else "native-stack-operation-failed", failure_type=type(error).__name__)
    finally:
        report.update(cleanup_owned(stack, control, guard))
        if guard.failure:
            report["result"] = "FAIL"
        # Do not serialize DevStack's diagnostics, URLs, credentials or migration witness tokens.
        if (report["cleanup"] != "PASS" or stack.report.get("result") == "FAIL"
                or report.get("prepared_before_cleanup", "0") != "0" or time.monotonic() >= guard.hard_deadline):
            report["result"] = "FAIL"
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        dev.write_fd(fd, report)
    return int(report["result"] == "FAIL")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--browser-origin", required=True)
    parser.add_argument("--allocation-id", required=True)
    for name in ("allocation-work-deadline-epoch", "allocation-release-deadline-epoch",
                 "whole-timeout-seconds", "startup-timeout-seconds", "provision-timeout-seconds", "hold-seconds"):
        parser.add_argument("--" + name, type=float, required=True)
    for name in ("whole-byte-limit", "retained-carry-bytes", "max-owned-processes"):
        parser.add_argument("--" + name, type=int, required=True)
    args = parser.parse_args(argv)
    try:
        now, mono = time.time(), time.monotonic()
        WorkDeadline(mono + args.allocation_work_deadline_epoch - now,
                     mono + args.allocation_release_deadline_epoch - now).check()
        numbers = (args.whole_timeout_seconds, args.startup_timeout_seconds,
                   args.provision_timeout_seconds, args.hold_seconds)
        require(all(math.isfinite(v) and v > 0 for v in numbers), "phase-budget-invalid")
        require(args.whole_timeout_seconds > 240 and args.retained_carry_bytes >= 0
                and args.whole_byte_limit > args.retained_carry_bytes and args.max_owned_processes > 0,
                "allocation-bounds-invalid")
        require(args.allocation_release_deadline_epoch - args.allocation_work_deadline_epoch >= 240,
                "cleanup-reserve-required")
        require(re.fullmatch(r"[A-Za-z0-9_-]{1,100}", args.allocation_id) is not None, "allocation-id-invalid")
        return run(args)
    except (InputRefusal, OSError, ValueError, TypeError, KeyError):
        print('{"result":"REFUSED","reason":"invalid-or-unavailable-stack-input"}')
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

