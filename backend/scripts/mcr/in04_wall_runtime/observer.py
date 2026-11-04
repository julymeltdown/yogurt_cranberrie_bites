"""Read-only observations from an already owned, running seven-service stack.

No server, database, actor, role, fixture, restart or allocation is created here.
The assembling launcher owns the real children and calls handle_request inside
its existing guarded loop. Construction/attachment is NOT runtime qualification.
"""
from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import http.client
import json
import os
from pathlib import Path
import secrets
import socketserver
import time
from uuid import UUID

import in03_stack_runtime as source
import migration_smoke as migration


def require(value):
    source.require(value, "in04-managed-runtime-unavailable")


def canonical_uuid(value):
    require(isinstance(value, str) and str(UUID(value)) == value)
    return value


class Observations:
    def __init__(self, *, stack, browser, guard, pins, manifests, source_root,
                 candidate, origin, event_id, environment_id, actor_sessions,
                 restore_snapshot_id, expires_at, validate_deployment):
        self.stack, self.browser, self.guard = stack, browser, guard
        self.pins, self.manifests = pins, manifests
        self.root, self.candidate = Path(source_root), source.source_revision(candidate)
        self.origin = origin
        self.event = canonical_uuid(event_id)
        self.environment = canonical_uuid(environment_id)
        self.snapshot = canonical_uuid(restore_snapshot_id)
        # Exactly three actual authenticated sessions, with access tokens kept
        # only in the owning launcher's memory. No password/token in diagnostics.
        require(len(actor_sessions) == 3)
        self.sessions = tuple(actor_sessions)
        users = [canonical_uuid(s[0]) for s in self.sessions]
        require(len(set(users)) == 3 and all(isinstance(s[1], str) and s[1] for s in self.sessions))
        require(callable(validate_deployment))
        self.validate_deployment = validate_deployment
        self.expires_at = expires_at
        require(time.time() < expires_at <= time.time() + 900)
        require(set(pins) == source.SERVICES == set(manifests))
        self.binary_identity = source.validate_binary_cohort(pins)
        # Candidate means the assembled deployment revision. The assembling
        # owner MUST validate its actual component/source closure here, retaining
        # per-binary compiler revisions separately; it cannot relabel old builds.
        validate_deployment()
        self.hashes = {
            "domainHash": self.contract_hash("contracts/generated/mcr-domain.openapi.json"),
            "realtimeHash": self.contract_hash("contracts/realtime/v1/world.schema.json"),
        }
        self.snapshot_digest = None
        self.pg_identity = None
        self._restart_stage = None

    def arm_restart_baseline(self):
        """Explicit owner preparation; this never requests or performs a fault."""
        require(self._restart_stage is None)
        self.guard.check()
        require(time.time() < self.expires_at)
        self._restart_stage = {"state": "ARMED"}

    def record_restart_baseline(self, binding, baseline):
        require(self._restart_stage == {"state": "ARMED"})
        require(isinstance(baseline, dict) and set(baseline) == {
            "systemIdentifier", "postmasterStartedAt", "concertPid"})
        # observe() performs actual owner/source/PG/children/public-role checks.
        # A caller's supplied baseline alone is never an observation.
        current = self.observe(binding)["generation"]
        require(current == baseline)
        self._restart_stage = {"state": "RECORDED", "generation": dict(current)}
        return {"state": "OBSERVED", "generation": dict(current)}

    def take_restart_baseline(self):
        if self._restart_stage is None or self._restart_stage["state"] != "RECORDED":
            return None
        self.guard.check()
        require(time.time() < self.expires_at)
        generation = dict(self._restart_stage["generation"])
        self._restart_stage = {"state": "CONSUMED"}
        return generation

    def contract_hash(self, path):
        return hashlib.sha256(source.source_bytes(self.root, self.candidate, path)).hexdigest()

    def binding(self):
        return {"candidate": self.candidate, "eventId": self.event,
                "environmentId": self.environment, "origin": self.origin, **self.hashes,
                "members": [s[0] for s in self.sessions[:2]], "operator": self.sessions[2][0],
                "restoreSnapshotId": self.snapshot}

    def observe(self, binding):
        require(binding == self.binding())
        self.guard.check()
        require(time.time() < self.expires_at)
        self.stack.cluster.assert_owned()
        self.validate_deployment()
        require(source.validate_binary_cohort(self.pins) == self.binary_identity)
        require(set(self.stack.children) == source.SERVICES)
        for name, child in self.stack.children.items():
            require(child.process is not None and child.process.poll() is None
                    and Path(child.process.args[0]) == self.pins[name].path)
        self.stack.health()
        cluster = self.stack.cluster
        generation = json.loads(cluster.sql("postgres", "select json_build_object("
            "'systemIdentifier',(select system_identifier::text from pg_control_system()),"
            "'postmasterStartedAt',to_char(pg_postmaster_start_time() at time zone 'UTC',"
            "'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),"
            "'dataDirectory',current_setting('data_directory'))"))
        require(generation.pop("dataDirectory") == str(cluster.data))
        if self.pg_identity is None:
            self.pg_identity = generation["systemIdentifier"]
        require(self.pg_identity == generation["systemIdentifier"])
        databases = json.loads(cluster.sql("postgres", "select coalesce(json_agg(datname order by datname),'[]') "
            "from pg_database where not datistemplate and datname<>'postgres'"))
        expected_databases = sorted(self.database(name) for name in source.SERVICES)
        require(databases == expected_databases)
        for name in sorted(source.SERVICES):
            # Actual source-bound ledger, not cached startup status.
            migration.check_ledger(cluster, self.database(name), self.manifests[name])
        actual_events = cluster.sql("mcr_concert", "select coalesce(string_agg(event_id::text,',' order by event_id),'') from concert_events")
        require(actual_events == self.event)
        # UUIDs above are canonical, not arbitrary SQL. No mutation is permitted.
        snapshot = cluster.sql("mcr_concert", "select row_to_json(s)::text from "
            "(select snapshot_id,event_id,revision,state,encode(pixels,'hex') pixels,palette,created_at "
            f"from concert_mural_snapshots where event_id='{self.event}'::uuid "
            f"and snapshot_id='{self.snapshot}'::uuid) s")
        require(bool(snapshot))
        digest = hashlib.sha256(snapshot.encode()).hexdigest()
        if self.snapshot_digest is None:
            self.snapshot_digest = digest
        require(digest == self.snapshot_digest)
        # Actual public reads enforce current Auth/admission and managed roles.
        # No extra login, refresh, synthetic role row or authority flag is used.
        for index, (user, token) in enumerate(self.sessions):
            participant = self.get(f"/api/concert/events/{self.event}/me", token, 200)
            require(isinstance(participant, dict) and participant.get("profile", {}).get("user_id") == user)
            admin = self.get(f"/api/concert/events/{self.event}/admin/mural", token, 200 if index == 2 else 403)
            if index == 2:
                require(isinstance(admin, dict) and admin.get("event_id") == self.event)
            else:
                require(isinstance(admin, dict) and admin.get("details", {}).get("reason") == "PERMISSION_DENIED")
        require({k: self.contract_hash(p) for k, p in (
            ("domainHash", "contracts/generated/mcr-domain.openapi.json"),
            ("realtimeHash", "contracts/realtime/v1/world.schema.json"))} == self.hashes)
        self.guard.check()
        generation["concertPid"] = self.stack.children["concert-service"].process.pid
        proof = {"environmentId": self.environment, "eventId": self.event,
            "isolated": True, "legacyData": False, "candidate": self.candidate, **self.hashes,
            "expiresAt": datetime.fromtimestamp(self.expires_at, timezone.utc).isoformat().replace("+00:00", "Z"),
            "actors": [{"userId": s[0], "role": "OPERATOR" if i == 2 else "MEMBER"}
                       for i, s in enumerate(self.sessions)],
            "services": [{"name": n, "ready": True, "candidate": self.candidate}
                         for n in ("gateway", "auth", "profile", "concert", "world", "postgres")],
            "ownResultOperation": "getConcertMuralOwnResult", "restoreSnapshotId": self.snapshot,
            "exclusiveEvent": True}
        return {"state": "OBSERVED", "proof": proof, "generation": generation}

    def get(self, path, token, status):
        # Bodyless HTTP on the actual incumbent Gateway. The general Auth
        # Browser helper defaults to JSON {}, which is unsuitable for these
        # closed bodyless product operations.
        target = self.browser.target
        require(target.scheme == "http" and target.hostname in ("127.0.0.1", "localhost", "::1"))
        connection = http.client.HTTPConnection(target.hostname, target.port, timeout=3)
        correlation = "in04-observe-" + secrets.token_hex(12)
        try:
            connection.request("GET", path, headers={"Authorization": "Bearer " + token,
                "Origin": self.origin, "X-MCR-Intent": "browser", "X-Correlation-Id": correlation})
            response = connection.getresponse()
            raw = response.read(65537)
            require(response.status == status and len(raw) <= 65536
                    and response.getheader("Cache-Control") == "no-store"
                    and response.getheader("X-Correlation-Id") == correlation)
            return source.json_object(raw)
        finally:
            connection.close()

    @staticmethod
    def database(service):
        return "mcr_" + service.removesuffix("-service").replace("-", "_")


class ReadOnlyControl(socketserver.UnixStreamServer):
    def __init__(self, observations):
        owner = self
        self.observations = observations
        self.token = secrets.token_hex(24)

        class Handler(socketserver.StreamRequestHandler):
            def handle(self):
                self.connection.settimeout(12)
                try:
                    raw = self.rfile.readline(8193)
                    require(len(raw) <= 8192 and raw.endswith(b"\n"))
                    request = source.json_object(raw)
                    require(isinstance(request, dict) and isinstance(request.get("owner"), str)
                            and secrets.compare_digest(request["owner"], owner.token))
                    if request.get("action") == "inspect":
                        require(set(request) == {"action", "owner", "binding"})
                        result = observations.observe(request["binding"])
                    else:
                        require(request.get("action") == "restart-baseline" and set(request) == {
                            "action", "owner", "binding", "baseline"})
                        result = observations.record_restart_baseline(request["binding"], request["baseline"])
                    self.wfile.write(json.dumps(result, separators=(",", ":")).encode() + b"\n")
                except Exception:
                    # Errors cannot expose SQL, credentials, frames or arbitrary
                    # exception strings into browser/QA artifacts.
                    self.wfile.write(b'{"state":"UNAVAILABLE"}\n')

        super().__init__(str(observations.stack.cluster.root / "in04-observe.sock"), Handler)
        Path(self.server_address).chmod(0o600)
        self.timeout = 0.2

    def descriptor(self):
        cluster = self.observations.stack.cluster
        cluster.assert_owned()
        return {"version": 1, "owner": self.token, "root": str(cluster.root),
                "rootIdentity": list(cluster.identity), "socket": str(self.server_address),
                "expiresAtEpoch": self.observations.expires_at}

    def handle_error(self, *_):
        pass
