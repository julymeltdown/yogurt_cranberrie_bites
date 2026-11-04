"""QA05 controls attached by the incumbent launcher to its actual owned stack.

No CLI, discovered PID, second stack, principal creation, or database mutation.
The launcher calls server.handle_request(), then controls.tick() in its existing
guarded loop. A request queues work; a checkpoint never performs that work.
"""
from __future__ import annotations

import hashlib
import json
import math
import os
from pathlib import Path
import re
import secrets
import socketserver
import stat
import subprocess
import time
from uuid import UUID, uuid4
from urllib.parse import urlsplit

import in03_stack_runtime as source
import migration_smoke as migration
import world_in01_live as world


def need(value):
    source.require(value, "qa05-owned-control-unavailable")


def uuid(value):
    need(isinstance(value, str) and str(UUID(value)) == value)
    return value


def private(path, kind):
    path = Path(path)
    info = path.lstat()
    need(path.is_absolute() and info.st_uid == os.getuid() and not info.st_mode & 0o077
         and not stat.S_ISLNK(info.st_mode)
         and (stat.S_ISDIR(info.st_mode) if kind == "directory" else stat.S_ISSOCK(info.st_mode)))
    return info.st_dev, info.st_ino


class OwnerControls:
    """Retained direct Child + RAM environment, never descriptor/PID authority.

    validate_deployment checks the launcher's actual candidate/component closure.
    actor_user_ids are the exact two already authenticated App principals; no
    credential is written to this descriptor, checkpoint, or exception text.
    """
    def __init__(self, *, stack, concert_child, concert_environment, listener_names,
                 guard, pins, manifests, source_root, candidate, origin, event_id,
                 environment_id, window_id, actor_user_ids, expires_at, validate_deployment):
        self.stack, self.child, self.guard = stack, concert_child, guard
        self.env, self.names = dict(concert_environment), tuple(listener_names)
        self.pins, self.manifests = pins, manifests
        self.source_root, self.candidate = Path(source_root), source.source_revision(candidate)
        self.event, self.environment = uuid(event_id), uuid(environment_id)
        need(isinstance(window_id, str) and re.fullmatch(r"[A-Za-z0-9_-]{1,96}", window_id))
        self.window = window_id
        need(isinstance(origin, str) and len(origin) < 256)
        target = urlsplit(origin)
        need(target.scheme == "http" and target.hostname in ("127.0.0.1", "localhost", "::1")
             and target.port is not None and not target.username and not target.password
             and not target.path and not target.query and not target.fragment)
        need(len(actor_user_ids) == 2)
        self.actors = tuple(uuid(x) for x in actor_user_ids)
        need(len(set(self.actors)) == 2 and callable(validate_deployment))
        self.origin, self.validate_deployment = origin, validate_deployment
        now = time.time()
        need(type(expires_at) in (int, float) and math.isfinite(expires_at)
             and now < expires_at <= now + 900)
        self.expires_at, self.deadline = expires_at, time.monotonic() + expires_at - now
        self.root = stack.cluster.root
        self.root_identity = private(self.root, "directory")
        self.cluster_token = stack.cluster.token
        need(set(pins) == source.SERVICES == set(manifests))
        self.binary_identity = source.validate_binary_cohort(pins)
        self.hashes = {k: hashlib.sha256(source.source_bytes(self.source_root, self.candidate, p)).hexdigest()
                       for k, p in (("domainHash", "contracts/generated/mcr-domain.openapi.json"),
                                    ("realtimeHash", "contracts/realtime/v1/world.schema.json"))}
        self.pg_identity = None
        self.requests, self.active = {}, None
        need(set(stack.children) == source.SERVICES)
        self.peers = {n: c for n, c in stack.children.items() if n != "concert-service"}
        self._check(running=True)

    def binding(self):
        return {"candidate": self.candidate, "origin": self.origin,
                "eventId": self.event, "environmentId": self.environment,
                "windowId": self.window, "members": list(self.actors), **self.hashes}

    def _check(self, *, running):
        self.guard.check()
        need(time.time() < self.expires_at and time.monotonic() < self.deadline)
        self.stack.cluster.assert_owned()
        need(self.stack.cluster.token == self.cluster_token
             and private(self.root, "directory") == self.root_identity)
        self.validate_deployment()
        need(source.validate_binary_cohort(self.pins) == self.binary_identity)
        need(set(self.stack.children) == source.SERVICES)
        for name, child in self.peers.items():
            need(self.stack.children[name] is child and child.cluster is self.stack.cluster
                 and isinstance(child, world.dev.runtime.Child)
                 and isinstance(child.process, subprocess.Popen) and child.process.poll() is None
                 and child.process.args == [str(self.pins[name].path)])
        need(self.stack.children.get("concert-service") is self.child
             and isinstance(self.child, world.dev.runtime.Child)
             and self.child.cluster is self.stack.cluster
             and isinstance(self.child.process, subprocess.Popen)
             and self.child.process.args == [str(self.pins["concert-service"].path)]
             and tuple(self.child.reservations) == self.names and bool(self.names)
             and all(n.startswith("concert-service:") for n in self.names))
        if running:
            need(self.child.process.poll() is None)
        self.guard.check()

    def _pg(self):
        cluster = self.stack.cluster
        for name in sorted(source.SERVICES):
            database = "mcr_" + name.removesuffix("-service").replace("-", "_")
            migration.check_ledger(cluster, database, self.manifests[name])
        pg = source.json_object(cluster.sql("postgres", "select json_build_object("
            "'systemIdentifier',(select system_identifier::text from pg_control_system()),"
            "'postmasterStartedAt',pg_postmaster_start_time()::text,"
            "'dataDirectory',current_setting('data_directory'))"))
        need(pg.pop("dataDirectory") == str(cluster.data))
        if self.pg_identity is None:
            self.pg_identity = pg["systemIdentifier"]
        need(pg["systemIdentifier"] == self.pg_identity)
        return pg

    def _observe(self):
        self._check(running=True)
        cluster = self.stack.cluster
        pg = self._pg()
        # Canonical UUIDs only. These are observations, never a cleanup/reset.
        rooms = source.json_object(cluster.sql("mcr_concert",
            "select coalesce(json_agg(json_build_object('roomId',room_id,'ownerEpoch',owner_epoch,"
            "'ownerProcessId',owner_process_id,'leaseUntil',lease_until) order by room_id),'[]') "
            f"from concert_world_owner_leases where event_id='{self.event}'::uuid"))
        memberships = source.json_object(cluster.sql("mcr_concert",
            "select coalesce(json_agg(json_build_object('userId',user_id,'roomId',room_id,"
            "'ownerEpoch',owner_epoch,'disconnected',disconnected) order by user_id),'[]') "
            f"from concert_world_memberships where event_id='{self.event}'::uuid"))
        need(isinstance(rooms, list) and 0 < len(rooms) <= 16 and isinstance(memberships, list)
             and len(memberships) <= 2 and all(m["userId"] in self.actors for m in memberships))
        started = migration.execute(["/bin/ps", "-p", str(self.child.process.pid), "-o", "lstart="],
            env=cluster.env, cwd=cluster.root, phase="qa05-child-start", timeout=2).stdout.strip()
        need(bool(started) and self.child.process.poll() is None)
        generation = {**pg, "concertPid": self.child.process.pid, "concertStartedAt": started,
                      "executableSha256": self.pins["concert-service"].sha256,
                      "ownerEpochs": [{"roomId": r["roomId"], "ownerEpoch": r["ownerEpoch"]} for r in rooms]}
        self.guard.check()
        return {"state": "OBSERVED", "binding": self.binding(), "generation": generation,
                "rooms": rooms, "memberships": memberships}

    def inspect(self, binding):
        need(binding == self.binding())
        need(self.active is None or self.requests[self.active]["state"] in ("COMPLETE", "FAILED"))
        return self._observe()

    def request_world_restart(self, binding, request_id, before_generation):
        return self._request(binding, request_id, before_generation, "WORLD_RESTART")

    def request_postgres_concert_restart(self, binding, request_id, before_generation):
        return self._request(binding, request_id, before_generation, "POSTGRES_CONCERT_RESTART")

    def _request(self, binding, request_id, before_generation, action):
        need(binding == self.binding() and isinstance(request_id, str)
             and re.fullmatch(r"[A-Za-z0-9:_-]{1,128}", request_id) is not None)
        if request_id in self.requests:
            old = self.requests[request_id]
            need(old["action"] == action and old["before"]["generation"] == before_generation)
            return self.checkpoint(binding, request_id)
        need(len(self.requests) < 16 and self.active is None)
        reserve = 65 if action == "WORLD_RESTART" else 165
        need(min(self.expires_at - time.time(), self.deadline - time.monotonic()) >= reserve)
        observation = self._observe()
        need(before_generation == observation["generation"])
        self.requests[request_id] = {"requestId": request_id, "action": action,
            "state": "QUEUED", "before": observation, "requestedAtEpoch": time.time()}
        self.request_deadline = time.monotonic() + reserve - 5
        self.active = request_id
        return self.checkpoint(binding, request_id)

    def checkpoint(self, binding, request_id):
        need(binding == self.binding() and request_id in self.requests)
        self._check(running=False)
        # Detached values: an in-process caller cannot rewrite retained proof.
        return json.loads(json.dumps({"binding": self.binding(), **self.requests[request_id]}))

    def tick(self):
        """Only the incumbent launcher calls this; never called by checkpoint."""
        if self.active is None:
            return
        record = self.requests[self.active]
        try:
            self._check(running=record["state"] == "QUEUED")
            need(time.monotonic() < self.request_deadline)
            if record["state"] == "QUEUED":
                # Capture exact current generation again before any side effect.
                need(self._observe()["generation"] == record["before"]["generation"])
                self.child.process.terminate()  # Retained unreaped Popen, never os.kill(PID).
                record.update(state="STOPPING", stopRequestedAtEpoch=time.time())
                self.stop_deadline = time.monotonic() + 12
            elif record["state"] == "STOPPING":
                result = self.child.process.poll()
                if result is None:
                    need(time.monotonic() < self.stop_deadline)
                    return
                need(result == 0)
                record.update(state="LEASE_WAIT", stoppedAtEpoch=time.time(),
                              stoppedAtMonotonic=time.monotonic())
            elif record["state"] == "LEASE_WAIT":
                elapsed = time.monotonic() - record["stoppedAtMonotonic"]
                if elapsed < 5.1:
                    return
                record["naturalOwnerLeaseWaitSeconds"] = elapsed
                if record["action"] == "POSTGRES_CONCERT_RESTART":
                    # Explicit separate mode. The same cluster object/data/ledger
                    # is retained; no initdb, fixture reset, or new services.
                    self.stack.cluster.stop()
                    self._check(running=False)
                    self.stack.cluster.start()
                    self._check(running=False)
                    need(self._pg()["postmasterStartedAt"] != record["before"]["generation"]["postmasterStartedAt"])
                self._replace(record)
                record.update(state="REPLACEMENT_STARTING", replacementStartedAtEpoch=time.time())
            elif record["state"] == "REPLACEMENT_STARTING":
                need(self.child.process.poll() is None)
                if not self.child.listeners():
                    return
                status, _ = world.dev.runtime.http_probe(self.child, "metrics", "/readyz")
                if status != 200:
                    return
                after = self._observe()
                before = record["before"]["generation"]
                need(after["generation"]["systemIdentifier"] == before["systemIdentifier"]
                     and (after["generation"]["postmasterStartedAt"] == before["postmasterStartedAt"])
                     == (record["action"] == "WORLD_RESTART")
                     and after["generation"]["concertPid"] != before["concertPid"])
                old_epochs = {r["ownerEpoch"] for r in before["ownerEpochs"]}
                need({r["roomId"] for r in before["ownerEpochs"]} == {r["roomId"] for r in after["rooms"]}
                     and all(r["ownerEpoch"] not in old_epochs for r in after["rooms"])
                     and all(m["ownerEpoch"] not in old_epochs for m in after["memberships"]))
                record.update(state="COMPLETE", after=after, completedAtEpoch=time.time())
                self.active = None
        except Exception:
            # Fixed diagnosis only; the launcher's existing cleanup still owns
            # every actual child, including a failed replacement.
            record.update(state="FAILED", failure="QA05_OWNER_CONTROL_FAILED")
            self.active = None
            raise source.InputRefusal("qa05-owned-control-failed") from None

    def _replace(self, record):
        ports = world.RestartPorts()
        fresh = {**self.env, "CONCERT_WORLD_OWNER_ID": str(uuid4()),
                 "CONCERT_PRODUCER_INSTANCE_ID": str(uuid4())}
        fresh["CONCERT_WORLD_INVALIDATION_CONSUMER_ID"] = fresh["CONCERT_WORLD_OWNER_ID"]
        try:
            self._check(running=False)
            need(self.child.process.poll() is not None)
            for name in self.names:
                ports.reserve(name, self.stack.ports.ports[name])
            self._check(running=False)
            with world.dev.runtime.protected_signals():
                child = world.dev.runtime.Child(self.stack.cluster, "concert-service",
                    [str(self.pins["concert-service"].path)], fresh, ports, self.names, tuple(fresh.values()))
                # Immediately transfer to existing stack cleanup, before readiness.
                self.stack.children["concert-service"] = self.child = child
                self.env = fresh
        finally:
            ports.close()


class ControlServer(socketserver.UnixStreamServer):
    """Private socket in the already owned root; no background server thread."""
    def __init__(self, controls):
        self.controls, self.token = controls, secrets.token_hex(24)
        controls._check(running=True)
        owner = self

        class Handler(socketserver.StreamRequestHandler):
            def handle(self):
                self.connection.settimeout(2)
                try:
                    raw = self.rfile.readline(8193)
                    need(len(raw) <= 8192 and raw.endswith(b"\n"))
                    request = source.json_object(raw)
                    need(isinstance(request, dict) and isinstance(request.get("owner"), str)
                         and secrets.compare_digest(request["owner"], owner.token))
                    common = {"action", "owner", "binding"}
                    action = request.get("action")
                    if action == "inspect":
                        need(set(request) == common)
                        result = controls.inspect(request["binding"])
                    elif action in ("requestWorldRestart", "requestPostgresConcertRestart"):
                        need(set(request) == common | {"requestId", "beforeGeneration"})
                        method = controls.request_world_restart if action == "requestWorldRestart" else controls.request_postgres_concert_restart
                        result = method(request["binding"], request["requestId"], request["beforeGeneration"])
                    elif action == "checkpoint":
                        need(set(request) == common | {"requestId"})
                        result = controls.checkpoint(request["binding"], request["requestId"])
                    else:
                        need(False)
                    response = json.dumps(result, separators=(",", ":")).encode() + b"\n"
                    need(len(response) <= 32768)
                    self.wfile.write(response)
                except Exception:
                    self.wfile.write(b'{"state":"UNAVAILABLE"}\n')

        super().__init__(str(controls.root / "qa05-control.sock"), Handler)
        Path(self.server_address).chmod(0o600)
        self.socket_identity = private(self.server_address, "socket")
        self.timeout = 0.1

    def descriptor(self):
        self.controls._check(running=False)
        need(private(self.server_address, "socket") == self.socket_identity)
        return {"version": 1, "owner": self.token, "root": str(self.controls.root),
                "rootIdentity": list(self.controls.root_identity),
                "socket": str(self.server_address), "socketIdentity": list(self.socket_identity),
                "binding": self.controls.binding(), "expiresAtEpoch": self.controls.expires_at}

    def handle_error(self, *_):
        pass
