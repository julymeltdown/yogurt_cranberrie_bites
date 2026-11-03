#!/usr/bin/env python3
"""Provision canonical IN01 inputs in explicitly owned MCR infrastructure; never an IN01 verdict."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import signal
import stat
import subprocess
import time
from uuid import UUID, uuid5

import browser_auth_smoke as auth
import dev_stack as dev

ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_in_concert"
MIGRATIONS = ("0001_concert_core.sql", "0002_command_receipts_outbox.sql",
              "0003_places_map_releases.sql", "0004_bootstrap_admission.sql", "0005_world_admission.sql")


class Failure(Exception):
    """Fixed diagnostic code only; arbitrary server/OS exceptions never enter receipts."""


def need(value, code):
    if not value:
        raise Failure(code)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical_id(value):
    try:
        return isinstance(value, str) and str(UUID(value)) == value and UUID(value).int != 0
    except ValueError:
        return False


def read_regular(path, *, private=False, limit=4 * 1024 * 1024):
    path = Path(path)
    need(path.is_absolute(), "ABSOLUTE_FILE_REQUIRED")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        info = os.fstat(fd)
        need(stat.S_ISREG(info.st_mode) and info.st_size <= limit, "REGULAR_BOUNDED_FILE_REQUIRED")
        if private:
            need(info.st_uid == os.getuid() and not info.st_mode & 0o077, "PRIVATE_OWNED_FILE_REQUIRED")
        with os.fdopen(os.dup(fd), "rb") as stream:
            raw = stream.read(limit + 1)
        need(len(raw) <= limit, "FILE_TOO_LARGE")
        after = os.fstat(fd)
        need((after.st_size, after.st_mtime_ns) == (info.st_size, info.st_mtime_ns), "FILE_CHANGED")
        return raw
    finally:
        os.close(fd)


def pinned(path):
    return {"path": str(path), "sha256": digest(read_regular(path))}


class PrivateRun:
    """Private output namespace, exclusive creation, fd-relative atomic checkpoints."""
    def __init__(self, path, *, existing=False):
        self.path = Path(path)
        need(self.path.is_absolute(), "ABSOLUTE_OUTPUT_REQUIRED")
        for parent in (self.path, *self.path.parents):
            need(not parent.is_symlink(), "OUTPUT_SYMLINK_REFUSED")
            need(not (parent / ".git").exists(), "SECRETS_OUTSIDE_GIT_REQUIRED")
        if not existing:
            self.path.mkdir(mode=0o700, exist_ok=False)
        self.fd = os.open(self.path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        info = os.fstat(self.fd)
        if not (info.st_uid == os.getuid() and not info.st_mode & 0o077):
            os.close(self.fd)
            raise Failure("PRIVATE_OUTPUT_REQUIRED")
        self.identity = (info.st_dev, info.st_ino)
        self.nonce = secrets.token_hex(16)
        try:
            if existing:
                marker = self.read("owner.json")
                need(marker.get("kind") == "mcr-in01-provision-v1" and marker.get("uid") == os.getuid()
                     and marker.get("identity") == list(self.identity)
                     and re.fullmatch(r"[0-9a-f]{32}", marker.get("nonce", "")), "RUN_OWNER_MISMATCH")
                self.nonce = marker["nonce"]
            else:
                self.write("owner.json", {"kind": "mcr-in01-provision-v1", "uid": os.getuid(),
                                         "identity": list(self.identity), "nonce": self.nonce})
        except BaseException:
            os.close(self.fd)
            raise

    def check(self):
        info = self.path.lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and not info.st_mode & 0o077
             and (info.st_dev, info.st_ino) == self.identity, "OUTPUT_REPLACED")

    def read(self, name):
        self.check()
        need(Path(name).name == name, "INVALID_OUTPUT_NAME")
        return json.loads(read_regular(self.path / name, private=True))

    def write(self, name, value):
        self.check()
        need(Path(name).name == name, "INVALID_OUTPUT_NAME")
        raw = (json.dumps(value, indent=2) + "\n").encode()
        try:
            old = os.stat(name, dir_fd=self.fd, follow_symlinks=False)
        except FileNotFoundError:
            old = None
        if old:
            need(stat.S_ISREG(old.st_mode) and old.st_uid == os.getuid() and not old.st_mode & 0o077,
                 "OUTPUT_FILE_REPLACED")
        temporary = ".checkpoint-" + secrets.token_hex(12)
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=self.fd)
        try:
            with os.fdopen(fd, "wb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
            self.check()
            os.replace(temporary, name, src_dir_fd=self.fd, dst_dir_fd=self.fd)
            os.fsync(self.fd)
        finally:
            try:
                os.unlink(temporary, dir_fd=self.fd)
            except FileNotFoundError:
                pass

    def close(self):
        os.close(self.fd)


def owned_descriptor(path):
    raw = read_regular(path, private=True, limit=16384)
    value = json.loads(raw)
    status = dev.control_request(Path(path), "status")
    need(status.get("state") == "READY" and len(status.get("services", {})) == 6, "OWNED_STACK_NOT_READY")
    need(digest(read_regular(path, private=True, limit=16384)) == digest(raw), "STACK_DESCRIPTOR_CHANGED")
    need(status.get("gateway_url") == value.get("gateway_url"), "STACK_ORIGIN_MISMATCH")
    dev.local_origin(value["gateway_url"])
    inbox = Path(value["root"]) / "mail"
    info = inbox.lstat()
    need(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and not info.st_mode & 0o077,
         "PRIVATE_INBOX_REQUIRED")
    return value, digest(raw)


def validate_binaries(bin_dir, receipt_path, *, historical_auth_only=False):
    receipt = json.loads(read_regular(receipt_path))
    need(receipt.get("exit_code") == 0 and re.fullmatch(r"[0-9a-f]{40}", receipt.get("source_revision", "")),
         "SUCCESSFUL_BINARY_SOURCE_RECEIPT_REQUIRED")
    hashes = receipt.get("binary_sha256", {})
    for service in dev.runtime.START_ORDER:
        path = Path(bin_dir) / service
        need(path.is_absolute() and path.is_file() and os.access(path, os.X_OK), "SERVICE_BINARY_REQUIRED")
        need(hashes.get(service) == digest(read_regular(path, limit=512 * 1024 * 1024)), "BINARY_RECEIPT_MISMATCH")
    sources = receipt.get("source_files_sha256", {})
    need(isinstance(sources, dict) and sources, "BINARY_SOURCE_HASHES_REQUIRED")
    drift = []
    for relative, expected in sources.items():
        candidate = Path(relative)
        need(not candidate.is_absolute() and ".." not in candidate.parts and candidate.parts[0] == "backend",
             "BINARY_SOURCE_PATH_INVALID")
        path = ROOT / candidate
        if not path.is_file() or digest(read_regular(path)) != expected:
            drift.append(relative)
    need(not drift or historical_auth_only, "BINARY_SOURCE_DRIFT_REQUIRES_HISTORICAL_AUTH_ONLY")
    return {"source_revision": receipt["source_revision"], "binary_sha256": {s: hashes[s] for s in dev.runtime.START_ORDER},
            "receipt": pinned(receipt_path), "current_source_drift": drift,
            "qualification_scope": "HISTORICAL_AUTH_ONLY_NOT_CURRENT_WORLD" if historical_auth_only else "SOURCE_HASHES_MATCH_BUILD_RECEIPT"}


def wait_mail(inbox, email, *, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        for path in inbox.glob("*.json"):
            value = json.loads(read_regular(path, private=True, limit=16384))
            if value.get("kind") == "VERIFY_EMAIL" and value.get("payload", {}).get("email") == email:
                code = value["payload"].get("verification_code")
                need(isinstance(code, str) and re.fullmatch(r"[0-9]{6}", code), "INVALID_VERIFICATION_DELIVERY")
                return code
        time.sleep(0.1)
    raise Failure("ACTUAL_VERIFICATION_MAIL_TIMEOUT")


def provision_accounts(run, descriptor, report, browser_origin=None):
    browser = auth.Browser(descriptor["gateway_url"], report)
    request = lambda *a, **kw: browser.request(*a, origin=browser_origin or True, **kw)
    accounts = [{"email": f"mcr-in01-{run.nonce}-{i}@example.test", "password": "Mcr!" + secrets.token_hex(24),
                 "stage": "GENERATED"} for i in range(2)]
    state = {"version": 1, "nonce": run.nonce, "gateway": descriptor["gateway_url"],
             "stack_root": descriptor["root"], "accounts": accounts}
    run.write("private.json", state)  # Recover exact created credentials after partial failure.
    for i, account in enumerate(accounts):
        label = f"account-{i}"
        account["stage"] = "SIGNUP_PENDING"
        run.write("private.json", state)
        _, value = request(label + "-signup", "/api/auth/email/signup",
                           {"email": account["email"], "password": account["password"]})
        need(canonical_id(value.get("userId")), "SIGNUP_CANONICAL_ID_REQUIRED")
        account.update(userId=value["userId"], stage="SIGNUP_SUCCEEDED")
        run.write("private.json", state)
        code = wait_mail(Path(descriptor["root"]) / "mail", account["email"])
        _, value = request(label + "-verify", "/api/auth/email/verify", {"email": account["email"], "code": code})
        need(value.get("userId") == account["userId"], "VERIFICATION_IDENTITY_MISMATCH")
        account["stage"] = "VERIFIED"
        run.write("private.json", state)
        headers, value = request(label + "-login", "/api/auth/login",
                                 {"email": account["email"], "password": account["password"]})
        need(value.get("userId") == account["userId"] and value.get("accessToken")
             and value.get("refreshToken") == "", "LOGIN_IDENTITY_OR_TOKEN_MISMATCH")
        account.update(accessToken=value["accessToken"], cookie=auth.refresh_cookie(headers), stage="AUTHENTICATED")
        run.write("private.json", state)
    need(accounts[0]["userId"] != accounts[1]["userId"], "INDEPENDENT_ACCOUNTS_REQUIRED")
    report["canonical_accounts"] = "PASS"
    report["account_count"] = 2
    return state


def resource_pins(map_path, avatar_path, art_path):
    need(map_path and avatar_path and art_path, "REVIEWED_RESOURCE_PINS_REQUIRED")
    documents = [json.loads(read_regular(p)) for p in (map_path, avatar_path)]
    for doc, kind in zip(documents, ("map_manifest", "avatar_manifest")):
        p = doc.get("provenance", {})
        need(doc.get("kind") == kind and p.get("source_kind") in ("ORIGINAL_ART", "PLACE_REFERENCE")
             and p.get("review_state") == "REVIEWED" and p.get("reviewed_by") and p.get("reviewed_at"),
             "REVIEWED_NONFIXTURE_RESOURCE_REQUIRED")
    # Pins are declarations, not art approval; real ProductReleaseBundle must still accept them.
    result = {"map_id": documents[0].get("map_id"), "map_revision": documents[0].get("map_revision"),
              "avatar_manifest_id": documents[1].get("manifest_id"), "avatar_manifest_version": documents[1].get("manifest_version")}
    validate_resource_binding(result)
    selected = json.loads(read_regular(ROOT / "frontend/src/entities/avatar/character-source-pack.json"))
    need(selected.get("production_atlas", {}).get("runtime_manifest_id") == result["avatar_manifest_id"],
         "SELECTED_SOURCE_PACK_RUNTIME_BINDING_REQUIRED")
    return result, {"map": pinned(map_path), "avatar": pinned(avatar_path), "art_review": pinned(art_path)}


def validate_resource_binding(resources):
    need(isinstance(resources, dict) and set(resources) == {"map_id", "map_revision", "avatar_manifest_id", "avatar_manifest_version"}, "INVALID_RESOURCE_BINDING")
    for key, value in resources.items():
        need((isinstance(value, str) and re.fullmatch(r"[a-z][a-z0-9_]{0,63}", value)) if key.endswith("id") else
             (type(value) is int and 1 <= value <= 9007199254740991), "INVALID_RESOURCE_BINDING")


def initialize_event(stack, nonce, report, resources=None, concert_binary=None):
    """Native-fixture initialization allowed only on this invocation's fresh Cluster object."""
    if resources is not None:
        validate_resource_binding(resources)
    cluster = stack.cluster
    need(isinstance(cluster, dev.migration.Cluster), "NEW_CLUSTER_CAPABILITY_REQUIRED")
    cluster.assert_owned()
    cluster.assert_postmaster()
    cluster.sql("postgres", f"create database {DATABASE} template template0;")  # No IF NOT EXISTS/reattach.
    pins = {}
    if concert_binary:
        env = {**cluster.env, "CONCERT_DATABASE_URL": cluster.url(DATABASE)}
        for flag in ("--migrate", "--verify-migrations"):
            dev.migration.execute([str(concert_binary), flag], env=env, cwd=cluster.root, phase="concert-schema")
        expected = dev.migration.migration_manifest(ROOT / "backend", "concert-service")
        dev.migration.check_ledger(cluster, DATABASE, expected)
        pins = {str(k): v for k, v in expected.items()}
    else:
        for name in MIGRATIONS:
            path = ROOT / "backend/services/concert-service/migrations" / name
            raw = read_regular(path)
            cluster.sql(DATABASE, raw.decode())
            pins[name] = digest(raw)
    event = str(uuid5(UUID(nonce), "in01-event"))
    room = str(uuid5(UUID(nonce), "in01-default-room"))
    # All interpolated values are generated UUIDs or strictly validated resource identifiers/integers.
    sql = f"begin; insert into concert_events(event_id,slug,title,phase) values('{event}','in01-{nonce}','Owned IN01 development event','OPEN');"
    if resources:
        sql += ("insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,is_default) values("
                f"'{event}','{room}','Main',50,'{resources['map_id']}',{resources['map_revision']},"
                f"'{resources['avatar_manifest_id']}',{resources['avatar_manifest_version']},true);")
    cluster.sql(DATABASE, sql + "commit;")
    need(cluster.sql(DATABASE, f"select phase||':'||revision from concert_events where event_id='{event}';") == "OPEN:1", "EVENT_FIXTURE_NOT_PERSISTED")
    report["fixture"] = {"event": "PASS", "default_room": "INITIALIZED_NOT_ADMITTED" if resources else "NOT_RUN",
                         "missing": None if resources else "REVIEWED_RESOURCE_PINS_REQUIRED", "migration_hashes": pins,
                         "method": ("qualified Concert migration CLI and exact full SQLx ledger" if concert_binary else
                                    "exact bounded native migration schema; no SQLx runtime ledger claim") + "; new owned DB, generated OPEN event",
                         "roles_admissions_participants_tickets_seeded": False}
    return event


def check_world(descriptor, state, event, report, browser_origin=None):
    browser = auth.Browser(descriptor["gateway_url"], report)
    default = None
    for i, account in enumerate(state["accounts"]):
        for suffix in ("bootstrap", "rooms"):
            label = f"account-{i}-{suffix}"
            try:
                _, value = browser.request(label, f"/api/concert/events/{event}/{suffix}", method="GET",
                                           bearer=account["accessToken"], origin=browser_origin or True)
            except auth.CheckFailure:
                need(report["statuses"].get(label) != 200, "REAL_WORLD_RESPONSE_POLICY_FAILED")
                report["world_prerequisites"] = {"status": "NOT_RUN", "code": "REAL_BOOTSTRAP_OR_ROOMS_UNAVAILABLE",
                                                 "failed_operation": suffix, "observed_status": report["statuses"].get(label)}
                return False
            if suffix == "bootstrap":
                need(value.get("event", {}).get("event_id") == event and value.get("participant", {}).get("user_id") == account["userId"], "BOOTSTRAP_SCOPE_MISMATCH")
                if value.get("capabilities", {}).get("world", {}).get("available") is not True:
                    report["world_prerequisites"] = {"status": "NOT_RUN", "code": "WORLD_CAPABILITY_UNAVAILABLE"}
                    return False
            else:
                room = value.get("default_room_id")
                items = value.get("items", [])
                need(value.get("event_id") == event and canonical_id(room) and isinstance(items, list)
                     and len({r.get("room_id") for r in items}) == len(items)
                     and any(r.get("room_id") == room and r.get("availability") == "AVAILABLE" for r in items), "DEFAULT_ROOM_NOT_AVAILABLE")
                need(default is None or default == room, "ACCOUNTS_DEFAULT_ROOM_MISMATCH")
                default = room
    report["world_prerequisites"] = {"status": "PASS", "scope": "real bootstrap and same available default-room reads only; ticket/admission/movement NOT_RUN"}
    return True


def validate_origin(origin):
    from urllib.parse import urlsplit
    try:
        u = urlsplit(origin)
        port = u.port
        host = u.hostname or ""
        normalized = ("[" + host + "]") if ":" in host else host
        if port is not None and port != (443 if u.scheme == "https" else 80):
            normalized += ":" + str(port)
        valid = (not u.username and not u.password and not u.path and not u.query and not u.fragment
                 and host and u.scheme + "://" + normalized == origin
                 and (u.scheme == "https" or (u.scheme == "http" and host in ("localhost", "127.0.0.1", "::1"))))
    except (ValueError, TypeError):
        valid = False
    need(valid, "INVALID_GATE_ORIGIN")
    return origin


def gate_config(state, origin, event, revision, service, art):
    need(canonical_id(event) and re.fullmatch(r"[0-9a-f]{40}", revision), "INVALID_RELEASE_DECLARATION")
    validate_origin(origin)
    accounts = [{k: a[k] for k in ("email", "password", "userId")} for a in state["accounts"]]
    need(len(accounts) == 2 and accounts[0]["userId"] != accounts[1]["userId"]
         and accounts[0]["email"].lower() != accounts[1]["email"].lower(), "INDEPENDENT_ACCOUNTS_REQUIRED")
    return {"version": 1, "isolated": True, "origin": origin, "eventId": event, "releaseRevision": revision,
            "accounts": accounts, "serviceReceipt": pinned(service), "artReview": pinned(art)}


def source_receipt():
    paths = [Path(__file__), ROOT / "frontend/e2e/real/world-gate/config.ts",
             ROOT / "frontend/src/entities/avatar/character-source-pack.json", ROOT / "contracts/core/v1/operations.json",
             ROOT / "contracts/core/v1/core.schema.json", ROOT / "contracts/realtime/v1/operations.json",
             Path(auth.__file__), Path(dev.__file__), Path(dev.runtime.__file__), Path(dev.migration.__file__)]
    return {"source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "source_sha256": {str(p.relative_to(ROOT)): digest(read_regular(p)) for p in paths}}


def save_receipt(run, report):
    """Defense in depth for inherited lifecycle diagnostics; no private canary in receipts."""
    try:
        accounts = run.read("private.json")["accounts"]
    except FileNotFoundError:
        accounts = []
    private = [a[k] for a in accounts for k in ("email", "password", "accessToken", "cookie") if a.get(k)]
    def scrub(value):
        if isinstance(value, str):
            for secret in private:
                value = value.replace(secret, "[private]")
            return value
        if isinstance(value, dict):
            return {k: scrub(v) for k, v in value.items()}
        if isinstance(value, list):
            return [scrub(v) for v in value]
        return value
    sanitized = scrub(report)
    run.write("receipt.json", sanitized)
    return sanitized


def execute(args, run, stack=None):
    report = {"kind": "mcr-in01-provision-receipt-v1", "result": "FAIL", "IN01": "NOT_RUN", "statuses": {},
              "canonical_accounts": "NOT_RUN", "world_prerequisites": {"status": "NOT_RUN"},
              "cleanup": "NOT_RUN", "secrets_recorded": False, **source_receipt()}
    control = None
    try:
        need(canonical_id(args.event_id) if args.event_id else True, "INVALID_EVENT_ID")
        need(not (stack and args.event_id), "NEW_STACK_GENERATES_OWN_EVENT")
        resources = None
        if args.map_manifest or args.avatar_manifest:
            resources, report["resource_declarations"] = resource_pins(args.map_manifest, args.avatar_manifest, args.art_review)
        if args.art_review:
            pinned(args.art_review)
        concert_binary = getattr(args, "concert_binary", None)
        concert_receipt = getattr(args, "concert_build_receipt", None)
        need(bool(concert_binary) == bool(concert_receipt), "CONCERT_BINARY_RECEIPT_PAIR_REQUIRED")
        if concert_binary:
            need(stack is not None, "CONCERT_INITIALIZATION_REQUIRES_NEW_OWNED_STACK")
            binding = json.loads(read_regular(concert_receipt))
            need(binding.get("exit_code") == 0 and re.fullmatch(r"[0-9a-f]{40}", binding.get("source_revision", ""))
                 and os.access(concert_binary, os.X_OK)
                 and binding.get("binary_sha256", {}).get("concert-service") == digest(read_regular(concert_binary, limit=512 * 1024 * 1024)), "CONCERT_BINARY_RECEIPT_MISMATCH")
            report["concert_binary_binding"] = {"source_revision": binding["source_revision"], "receipt": pinned(concert_receipt),
                                                "binary_sha256": binding["binary_sha256"]["concert-service"]}
        if stack:
            report["binary_bindings"] = validate_binaries(args.bin_dir, args.binary_receipt,
                                                           historical_auth_only=getattr(args, "historical_auth_only", False))
            descriptor = stack.start()
            control = dev.Control(stack)
            run.write("ready.json", descriptor)
            descriptor_hash = digest(dev.encode(descriptor))
        else:
            descriptor, descriptor_hash = owned_descriptor(args.ready_file)
        report["owned_stack_descriptor_sha256"] = descriptor_hash
        run.write("binding.json", {"ready_file": str(args.ready_file) if not stack else str(run.path / "ready.json"),
                                   "descriptor_sha256": descriptor_hash})
        state = provision_accounts(run, descriptor, report, args.browser_origin)
        event = args.event_id
        if stack:
            event = initialize_event(stack, run.nonce, report, resources, getattr(args, "concert_binary", None))
            state["concert_database_url"] = stack.cluster.url(DATABASE)
        if not event:
            report["world_prerequisites"] = {"status": "NOT_RUN", "code": "OWNED_EVENT_ID_REQUIRED"}
        else:
            need(canonical_id(event), "INVALID_EVENT_ID")
            state["eventId"] = event
            run.write("private.json", state)
            ready = check_world(descriptor, state, event, report, args.browser_origin)
            if ready and args.art_review and args.browser_origin and not getattr(args, "historical_auth_only", False):
                run.write("services.json", {"kind": "mcr-in01-services-v1", "observed_at": datetime.now(timezone.utc).isoformat(),
                                           "release_revision_declared": args.release_revision, "observations": report})
                run.write("config.json", gate_config(state, args.browser_origin, event, args.release_revision,
                                                     run.path / "services.json", args.art_review))
                report["gate_config"] = "WRITTEN_WITH_SUPPLIED_ART_RECEIPT_NOT_VISUAL_ACCEPTANCE"
            else:
                report["gate_config"] = "NOT_WRITTEN_REAL_WORLD_AND_ART_PREREQUISITES_REQUIRED"
        report["result"] = "PASS"  # Bounded canonical provisioning, never the world gate.
        report["lifetime"] = "OWNED_FOREGROUND_STACK" if stack else "EXTERNAL_OWNER_CONTROLS_STACK"
        save_receipt(run, report)
        print(json.dumps({"result": "PASS", "canonical_accounts": "PASS", "IN01": "NOT_RUN", "output": str(run.path)}), flush=True)
        deadline = time.monotonic() + args.hold_seconds
        next_health = 0
        while stack and time.monotonic() < deadline and not control.stopping:
            control.handle_request()
            if time.monotonic() >= next_health:
                stack.health()
                next_health = time.monotonic() + 2
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = str(error) if isinstance(error, Failure) else type(error).__name__
        report["result"] = "FAIL"
    finally:
        if stack:
            try:
                try:
                    if control is not None:
                        control.server_close()
                finally:
                    stack.close()
                report["owned_stack"] = stack.report
                report["cleanup"] = stack.report.get("cleanup", "FAIL")
                report["lifetime"] = "ENDED_ACCOUNTS_AND_EVENT_REMOVED_WITH_OWNED_CLUSTER"
                if report["cleanup"] != "PASS":
                    report["result"] = "FAIL"
            except BaseException:
                report.update(result="FAIL", cleanup="FAIL")
        else:
            report["cleanup"] = "NOT_REQUESTED_EXISTING_OWNER_STACK_RETAINED"
        save_receipt(run, report)
    return report


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="command", required=True)
    for kind in ("owned", "attach"):
        c = sub.add_parser(kind)
        c.add_argument("--output-dir", type=Path, required=True)
        c.add_argument("--browser-origin")
        c.add_argument("--event-id")
        c.add_argument("--art-review", type=Path)
        c.add_argument("--map-manifest", type=Path)
        c.add_argument("--avatar-manifest", type=Path)
        c.add_argument("--release-revision", required=True)
        c.add_argument("--hold-seconds", type=int, default=0)
        if kind == "owned":
            c.add_argument("--bin-dir", type=Path, required=True)
            c.add_argument("--binary-receipt", type=Path, required=True)
            c.add_argument("--historical-auth-only", action="store_true")
            c.add_argument("--concert-binary", type=Path)
            c.add_argument("--concert-build-receipt", type=Path)
            c.add_argument("--pg-bin-dir", type=Path, default=Path("/opt/homebrew/opt/postgresql@16/bin"))
            c.add_argument("--redis-binary", type=Path, default=Path("/opt/homebrew/bin/redis-server"))
            c.add_argument("--openssl-binary", type=Path, default=Path("/opt/homebrew/bin/openssl"))
        else:
            c.add_argument("--ready-file", type=Path, required=True)
    args = p.parse_args(argv)
    need(re.fullmatch(r"[0-9a-f]{40}", args.release_revision) and 0 <= args.hold_seconds <= dev.MAX_HOLD,
         "INVALID_DECLARED_REVISION_OR_DURATION")
    if args.browser_origin:
        validate_origin(args.browser_origin)
        if args.command == "owned":
            dev.local_origin(args.browser_origin)
    stack = (dev.DevStack(args.bin_dir, args.pg_bin_dir, args.redis_binary, args.openssl_binary, args.browser_origin)
             if args.command == "owned" else None)
    run = None
    previous = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupted)
        run = PrivateRun(args.output_dir)
        report = execute(args, run, stack)
        print(json.dumps({"result": report["result"], "IN01": report["IN01"], "cleanup": report["cleanup"],
                          "failure": report.get("failure")}), flush=True)
        return 0 if report["result"] == "PASS" else 1
    except (Exception, KeyboardInterrupt):
        print(json.dumps({"result": "FAIL", "failure": "PRIVATE_PREFLIGHT_FAILED", "IN01": "NOT_RUN"}))
        return 1
    finally:
        if run:
            run.close()
        for sig, handler in previous.items():
            signal.signal(sig, handler)


if __name__ == "__main__":
    raise SystemExit(main())
