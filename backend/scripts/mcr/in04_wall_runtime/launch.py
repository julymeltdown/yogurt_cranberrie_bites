"""Root profile for the incumbent seven-service IN04 launcher.

This module starts nothing on import. Enrollment, roles and the restore
checkpoint come from their real owners; no identity or role SQL is inserted.
"""
from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import http.client
import importlib
import json
from pathlib import Path
import re
import secrets
import time
from unittest.mock import patch
from uuid import UUID

import in03_stack_runtime as source


ENROLLMENT_HELPER = "backend/scripts/mcr/session_deletion_runtime/enrollment.py"
ENROLLMENT_SHA = "c4a328cdbb220202ef90502324f9873da5012e045b4f1e8b22d6ba9843760dc9"
OPERATOR_SOURCE = "dee64fd1ac868f4dffbea49c0e75ca8e98727f3a"
ROLES_SHA = "65e238a225b4805ce25eef02526cbfcfea19b2c98adac01ff403f6f7b2b0fa79"
CONCERT_REQUIRED = (
    "src/main.rs", "src/config/operations_runtime.rs",
    "src/config/mural_tools_b.rs",
    "src/operations_live/public_access/protocol.rs",
    "src/operations_live/public_access/settings_product.rs",
    "src/world/runtime.rs", "src/world/replication.rs",
)
GATEWAY_REQUIRED = (
    "src/adapters/in/http/concert_product/feature_routes.rs",
    "src/adapters/in/http/concert_product/mod.rs",
)


def need(value, label="in04-profile-unavailable"):
    source.require(value, label)


def canonical_uuid(value):
    try:
        parsed = UUID(value) if isinstance(value, str) else None
    except ValueError:
        parsed = None
    need(parsed is not None and parsed.int and str(parsed) == value)
    return value


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def timestamp(value):
    need(isinstance(value, str))
    try:
        result = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        raise source.InputRefusal("in04-profile-time") from None
    need(result.utcoffset() is not None and result.utcoffset().total_seconds() == 0)
    return result.timestamp()


def load_config(path):
    raw = source.private_input(path)
    config = source.closed(source.json_object(raw),
        ("version", "environment_id", "window_id", "operator_packet", "restart_stage", "frontend_candidate"), "in04-profile-schema")
    need(type(config["version"]) is int and config["version"] == 1)
    canonical_uuid(config["environment_id"])
    source.source_revision(config["frontend_candidate"])
    need(config["restart_stage"] in ("NONE", "POSTGRES_CONCERT_AFTER_BASELINE"))
    need(isinstance(config["window_id"], str)
         and re.fullmatch(r"[A-Za-z0-9_-]{1,96}", config["window_id"]))
    pin = source.closed(config["operator_packet"], ("path", "sha256"), "in04-operator-pin")
    need(isinstance(pin["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", pin["sha256"]))
    packet_raw = source.private_input(Path(pin["path"]))
    need(digest(packet_raw) == pin["sha256"])
    packet = source.json_object(packet_raw)
    need(isinstance(packet, dict)
         and packet.get("state") == "COMPILE_QUALIFIED_EXISTING_STACK_EXECUTION_NOT_RUN"
         and packet.get("source_revision") == OPERATOR_SOURCE)
    executable = packet.get("compiled_executable", {})
    need(executable.get("kind") == "test" and executable.get("package") == "auth-service"
         and executable.get("producing_source") == OPERATOR_SOURCE
         and executable.get("cargo_exit") == 0
         and packet.get("helper", {}).get("sha256") == ENROLLMENT_SHA)
    binary = source.BinaryPin(Path(executable["path"]), executable["sha256"], OPERATOR_SOURCE)
    binary.validate()
    return config, raw, packet_raw, binary


def validate_frontend_binding(root, helper_revision, frontend_candidate):
    """Bind the published App independently of the actual backend source.

    Application build identity is produced later by the frontend builder. These
    checks neither invent that identity nor replace its own source validation.
    """
    source.source_revision(helper_revision)
    source.source_revision(frontend_candidate)
    need(source.git(root, "rev-parse", frontend_candidate + "^{commit}").decode().strip()
         == frontend_candidate, "in04-frontend-commit-required")
    source.git(root, "merge-base", "--is-ancestor", frontend_candidate, "refs/remotes/origin/master")
    # A published backend-only history without an App is not an App candidate.
    package = source.json_object(source.source_bytes(root, frontend_candidate, "frontend/package.json"))
    need(isinstance(package, dict) and isinstance(package.get("version"), str),
         "in04-frontend-package-required")
    hashes = {}
    for path in ("contracts/generated/mcr-domain.openapi.json", "contracts/realtime/v1/world.schema.json"):
        actual = source.source_bytes(root, helper_revision, path)
        need(actual == source.source_bytes(root, frontend_candidate, path), "in04-frontend-contract-drift")
        hashes[path] = digest(actual)
    return hashes


def get_json(browser, origin, path, token, *, expected=200, maximum=65536):
    target = browser.target
    need(target.scheme == "http" and target.hostname in ("localhost", "127.0.0.1", "::1"))
    connection = http.client.HTTPConnection(target.hostname, target.port, timeout=3)
    correlation = "in04-profile-" + secrets.token_hex(12)
    try:
        connection.request("GET", path, headers={"Authorization": "Bearer " + token,
            "Origin": origin, "X-MCR-Intent": "browser", "X-Correlation-Id": correlation})
        response = connection.getresponse()
        raw = response.read(maximum + 1)
        need(response.status == expected and len(raw) <= maximum
             and response.getheader("Cache-Control") == "no-store"
             and response.getheader("X-Correlation-Id") == correlation
             and response.getheader("Location") is None)
        value = source.json_object(raw)
        need(isinstance(value, dict))
        return value
    finally:
        connection.close()


class ManagedProfile:
    def __init__(self):
        self.config = self.roles = self.stack = None
        self.restart_request = None

    def attachment_ready(self, *, attachment, context, args):
        need(attachment.stack is self.stack)
        if self.config["restart_stage"] == "POSTGRES_CONCERT_AFTER_BASELINE":
            attachment.observations.arm_restart_baseline()

    def poll(self, *, attachment, context, args):
        """Independent guarded owner decision; never an observer-side command."""
        self.guard.check()
        if self.restart_request is not None:
            receipt = attachment.controls.checkpoint(attachment.controls.binding(), self.restart_request)
            need(receipt["state"] != "FAILED", "in04-owner-restart-failed")
            return
        baseline = attachment.observations.take_restart_baseline()
        if baseline is None:
            return
        need(self.config["restart_stage"] == "POSTGRES_CONCERT_AFTER_BASELINE")
        controls = attachment.controls
        # The notification supplies no fault authority. Reobserve the incumbent
        # controller's actual richer generation and require the same baseline.
        actual = controls.inspect(controls.binding())["generation"]
        need(actual["systemIdentifier"] == baseline["systemIdentifier"]
             and actual["concertPid"] == baseline["concertPid"]
             and timestamp(actual["postmasterStartedAt"]) == timestamp(baseline["postmasterStartedAt"]))
        request_id = "in04-root-" + secrets.token_hex(12)
        receipt = controls.request_postgres_concert_restart(controls.binding(), request_id, actual)
        need(receipt["state"] == "QUEUED")
        self.restart_request = request_id

    def before_start(self, *, args, stack, guard, pins, manifests, context, world, prep, dev, patches):
        need(self.stack is None)
        self.config, self.config_raw, self.packet_raw, self.operator = load_config(args.in04_managed_inputs)
        self.stack, self.guard, self.pins, self.manifests = stack, guard, pins, manifests
        self.args = args
        config = source.json_object(source.private_input(args.inputs))
        self.root = Path(config["helper_source"]["root"])
        self.revision = source.source_revision(config["helper_source"]["revision"])
        self.frontend_candidate = self.config["frontend_candidate"]
        self.frontend_contracts = validate_frontend_binding(self.root, self.revision, self.frontend_candidate)
        need(digest(source.source_bytes(self.root, self.revision, ENROLLMENT_HELPER)) == ENROLLMENT_SHA)
        need(digest(source.source_bytes(self.root, self.revision,
             "backend/scripts/mcr/in04_wall_runtime/roles.py")) == ROLES_SHA)
        for service, paths in (("concert-service", CONCERT_REQUIRED), ("api-gateway", GATEWAY_REQUIRED)):
            for relative in paths:
                path = "backend/services/" + service + "/" + relative
                need(source.source_bytes(self.root, pins[service].source, path)
                     == source.source_bytes(self.root, self.revision, path), "in04-component-source-drift")
        gateway_routes = source.source_bytes(self.root, pins["api-gateway"].source,
            "backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs")
        need(b"getConcertSettings" in gateway_routes
             and b"getOwnConcertOperationsAccess" in gateway_routes, "in04-qualified-gateway-required")
        need(len(manifests["auth-service"]) == 19, "in04-auth-enrollment-migrations")
        enrollment = importlib.import_module("session_deletion_runtime.enrollment")
        need(Path(enrollment.__file__).resolve() == (self.root / ENROLLMENT_HELPER).resolve())
        self.mail, self.actual_principal, self.actual_provenance = enrollment.owned_auth_adapters(
            prep, world, dev, self.operator.path, self.operator.sha256, args.browser_origin)
        patches.enter_context(patch.object(dev, "MailSink", self.mail))
        self.binary_identity = source.validate_binary_cohort(pins)

    def configure_environment(self, *, envs, concert, context, cluster, ports, args):
        from .roles import ManagedRoles
        need(self.stack.cluster is cluster and self.roles is None)
        cluster.assert_owned()
        system = cluster.sql("postgres", "select system_identifier::text from pg_control_system()")
        # Leave a small conversion margin; the issuer independently clamps its
        # microsecond expiry to this actual work deadline.
        now, mono = time.time(), time.monotonic()
        deadline = source.WorkDeadline(self.guard.deadline, self.guard.hard_deadline)
        end = min(args.allocation_work_deadline_epoch, now + deadline.work_monotonic - mono) - .01
        need(end > now)
        self.roles = ManagedRoles(self.stack, deadline,
            expected_system_identifier=system, window_end_epoch=end)
        self.role_environment = self.roles.environment()
        concert.update(self.role_environment)
        concert.update(CONCERT_WALL_TOOLS_B_MODE="live",
            CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON=json.dumps([
                {"id": "in04_ephemeral", "secret_hex": secrets.token_hex(32)}]))
        envs["auth-service"]["AUTH_ENROLLMENT_VERIFICATION_URL"] = args.browser_origin + "/verify-email"
        need(envs["api-gateway"].get("GATEWAY_REFRESH_COOKIE_SECURE") == "false")
        self.event = canonical_uuid(context["event"])
        self.system = system
        self.initial_env = dict(concert)
        self.work_expiry = end

    def principal(self, *, stack, browser, ordinal, deadline, context, args):
        need(stack is self.stack and context["event"] == self.event and ordinal in (1, 2, 3))
        self.guard.check()
        self.operator.validate()
        return self.actual_principal(stack, browser, ordinal, deadline=deadline)

    def auth_provenance(self, *, stack, browser, report, principals, event, revision,
                        auth_source, expires_at, world, args, context):
        need(stack is self.stack and event == self.event and revision == self.revision)
        return self.actual_provenance(stack, browser, report, principals, event,
            revision, auth_source, expires_at, world)

    def after_principals(self, *, stack, browser, guard, pins, manifests, context,
                         principals, source_root, candidate, origin, expires_at, args):
        need(stack is self.stack and guard is self.guard and pins is self.pins
             and manifests is self.manifests and source_root == self.root
             and candidate == self.revision and len(principals) == 3)
        users = [canonical_uuid(p[0]) for p in principals]
        need(len(set(users)) == 3)
        self.roles.publish_operator(self.event,
            [{"user_id": p[0], "credentials_path": p[1]} for p in principals], users[2])
        stop = min(time.monotonic() + 5, guard.deadline)
        while True:
            guard.check()
            access = get_json(browser, origin,
                f"/api/concert/events/{self.event}/me/operations-access", principals[2][2])
            lease = access.get("access")
            if isinstance(lease, dict) and lease.get("role") == "OPERATOR":
                need(lease.get("actor_id") == users[2] and lease.get("event_id") == self.event
                     and time.time() < timestamp(lease["expires_at"]) <= self.work_expiry)
                break
            need(lease is None and time.monotonic() < stop, "in04-role-worker-not-applied")
            time.sleep(.1)
        for principal in principals[:2]:
            member = get_json(browser, origin,
                f"/api/concert/events/{self.event}/me/operations-access", principal[2])
            need(member.get("access", "missing") is None, "in04-member-role-drift")
        settings = get_json(browser, origin,
            f"/api/concert/events/{self.event}/admin/settings", principals[2][2])
        need(settings.get("event", {}).get("event_id") == self.event
             and type(settings.get("revision")) is int and settings["revision"] > 0)
        snapshot = get_json(browser, origin, f"/api/concert/events/{self.event}/mural/snapshot",
            principals[0][2], maximum=262144)
        checkpoint = canonical_uuid(snapshot.get("snapshot_id"))
        need(snapshot.get("event_id") == self.event and snapshot.get("width") == 128
             and snapshot.get("height") == 128 and len(snapshot.get("pixels", [])) == 16384
             and len(snapshot.get("palette", [])) == 16)
        # Public snapshot() creates the immutable checkpoint in its actual
        # repository transaction. This is an observation, never SQL seeding.
        matches = stack.cluster.sql("mcr_concert", "select exists(select 1 from "
            "concert_mural_snapshots s join concert_mural_state m on m.snapshot_id=s.snapshot_id "
            f"where s.snapshot_id='{checkpoint}'::uuid and m.event_id='{self.event}'::uuid "
            "and (s.event_id,s.revision,s.state,s.pixels,s.palette,s.created_at)="
            "(m.event_id,m.revision,m.state,m.pixels,m.palette,m.snapshot_created_at))")
        need(matches == "t", "in04-checkpoint-not-durable")
        owner = stack.cluster.root.lstat()
        owner_identity = owner.st_dev, owner.st_ino
        peer_children = {name: child for name, child in stack.children.items() if name != "concert-service"}
        publication = Path(self.role_environment["CONCERT_OPERATIONS_PUBLICATION_FILE"])
        publication_raw = source.private_input(publication)
        helper_sources = source.validate_sources(self.root, self.revision)

        def validate_deployment():
            guard.check()
            stack.cluster.assert_owned()
            info = stack.cluster.root.lstat()
            need((info.st_dev, info.st_ino) == owner_identity
                 and source.private_input(args.in04_managed_inputs) == self.config_raw)
            need(source.private_input(Path(self.config["operator_packet"]["path"])) == self.packet_raw
                 and source.private_input(publication) == publication_raw)
            need(source.validate_sources(self.root, self.revision) == helper_sources
                 and source.validate_binary_cohort(pins) == self.binary_identity)
            need(validate_frontend_binding(self.root, self.revision, self.frontend_candidate)
                 == self.frontend_contracts)
            need(set(stack.children) == source.SERVICES and all(
                stack.children.get(name) is child for name, child in peer_children.items()))
            child = stack.children["concert-service"]
            # OwnerControls registers its actual replacement Child/environment
            # before calling this closure. A stopped fault child is allowed;
            # readiness and PG generation are checked by the actual observers.
            need(child.cluster is stack.cluster and child.process is not None
                 and child.process.args == [str(pins["concert-service"].path)])
            env = context["env"]
            need(all(env.get(key) == value for key, value in self.initial_env.items()
                     if key not in {"CONCERT_PRODUCER_INSTANCE_ID", "CONCERT_WORLD_OWNER_ID",
                                    "CONCERT_WORLD_INVALIDATION_CONSUMER_ID"})
                 and env.get("CONCERT_WALL_TOOLS_B_MODE") == "live")
            need(canonical_uuid(env.get("CONCERT_WORLD_OWNER_ID"))
                 == canonical_uuid(env.get("CONCERT_WORLD_INVALIDATION_CONSUMER_ID")))
            canonical_uuid(env.get("CONCERT_PRODUCER_INSTANCE_ID"))
            source.verify_migration_package(context["migration_package"], manifests["concert-service"])

        validate_deployment()
        return {"environment_id": self.config["environment_id"], "window_id": self.config["window_id"],
            "frontend_candidate": self.frontend_candidate,
            "restore_snapshot_id": checkpoint, "expires_at": min(expires_at, self.work_expiry),
            "validate_deployment": validate_deployment,
            "actor_sessions": tuple((p[0], p[2]) for p in principals)}


def main(argv=None):
    return source.main(argv, managed=ManagedProfile())


if __name__ == "__main__":
    raise SystemExit(main())
