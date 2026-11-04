"""Attach observations and fault controls to the incumbent managed stack.

The profile supplies actual Enrollment principals, signed roles and a durable
checkpoint. This module creates no stack, identity, database, role or checkpoint.
"""
from __future__ import annotations

import json
import math
import os
from pathlib import Path
import stat
import time

import in03_stack_runtime as source
from .observer import Observations, ReadOnlyControl
from .owner_control import OwnerControls
from .owner_control import ControlServer, uuid


def need(value):
    source.require(value, "managed-attachment-unavailable")


def contains_access_token(value, principals):
    raw = json.dumps(value, separators=(",", ":"))
    return any(p[2] in raw for p in principals)


class ManagedAttachment:
    def __init__(self, *, inputs, stack, browser, guard, pins, manifests, context,
                 principals, source_root, candidate, origin, expires_at):
        self.stack, self.guard, self.context = stack, guard, context
        self.controls = self.observations = None
        self.control_server = self.observer_server = None
        self.files, self.sockets, self.closed = {}, {}, False
        need(isinstance(inputs, dict) and set(inputs) == {
            "environment_id", "window_id", "restore_snapshot_id", "expires_at",
            "validate_deployment", "actor_sessions"})
        need(len(principals) == 3 and all(isinstance(p, tuple) and len(p) == 3
             and isinstance(p[1], Path) and isinstance(p[2], str) and p[2] for p in principals))
        sessions = tuple((uuid(p[0]), p[2]) for p in principals)
        need(len({s[0] for s in sessions}) == 3 and tuple(tuple(s) for s in inputs["actor_sessions"]) == sessions)
        need(callable(inputs["validate_deployment"]))
        actual_expiry = inputs["expires_at"]
        need(type(actual_expiry) in (int, float) and math.isfinite(actual_expiry)
             and time.time() < actual_expiry <= expires_at)
        self.expires_at, self.environment = actual_expiry, uuid(inputs["environment_id"])
        self.window, self.snapshot = inputs["window_id"], uuid(inputs["restore_snapshot_id"])
        stack.cluster.assert_owned()
        self.root, self.cluster_token = stack.cluster.root, stack.cluster.token
        info = self.root.lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and not info.st_mode & 0o077)
        self.root_identity = info.st_dev, info.st_ino

        def validate():
            # Follow only a replacement already registered by our retained
            # controls. The profile sees the actual current RAM environment.
            if self.controls is not None and stack.children.get("concert-service") is self.controls.child:
                context["env"] = self.controls.env
            inputs["validate_deployment"]()

        class FaultAwareObservations(Observations):
            def observe(observer, binding):
                need(not self.fault_active)
                return super().observe(binding)

        common = dict(stack=stack, guard=guard, pins=pins, manifests=manifests,
                      source_root=source_root, candidate=candidate, origin=origin,
                      event_id=context["event"], environment_id=self.environment,
                      expires_at=actual_expiry, validate_deployment=validate)
        try:
            child = stack.children["concert-service"]
            self.controls = OwnerControls(**common, concert_child=child,
                concert_environment=context["env"], listener_names=tuple(child.reservations),
                window_id=self.window, actor_user_ids=[s[0] for s in sessions[:2]])
            self.observations = FaultAwareObservations(**common, browser=browser,
                actor_sessions=sessions, restore_snapshot_id=self.snapshot)
            # Real SQL, role/public reads, current children and source closure;
            # construction/configuration alone is never an observed producer.
            self.observations.observe(self.observations.binding())
            self.controls.inspect(self.controls.binding())
            self.control_server = ControlServer(self.controls)
            self._remember_socket(self.control_server)
            self.observer_server = ReadOnlyControl(self.observations)
            self._remember_socket(self.observer_server)
            self.control_descriptor = self._publish("qa05-owner-control.json", self.control_server.descriptor())
            self.observer_descriptor = self._publish("in04-owner-observer.json", self.observer_server.descriptor())
        except BaseException:
            try:
                self.close()
            except BaseException:
                pass  # Outer stack cleanup still runs and owns all children.
            raise

    @property
    def fault_active(self):
        return self.controls is not None and self.controls.active is not None

    def _owned(self):
        self.stack.cluster.assert_owned()
        info = self.root.lstat()
        need(self.stack.cluster.token == self.cluster_token and stat.S_ISDIR(info.st_mode)
             and info.st_uid == os.getuid() and not info.st_mode & 0o077
             and (info.st_dev, info.st_ino) == self.root_identity)

    def _remember_socket(self, server):
        self._owned()
        path = Path(server.server_address)
        info = path.lstat()
        need(path.parent == self.root and stat.S_ISSOCK(info.st_mode)
             and info.st_uid == os.getuid() and not info.st_mode & 0o077)
        self.sockets[path] = info.st_dev, info.st_ino

    def _publish(self, filename, value):
        self._owned()
        path = self.root / filename
        raw = json.dumps(value, separators=(",", ":")).encode() + b"\n"
        need(len(raw) <= 8192)
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "wb") as stream:
            info = os.fstat(stream.fileno())
            self.files[path] = info.st_dev, info.st_ino
            stream.write(raw)
        need((path.lstat().st_dev, path.lstat().st_ino) == self.files[path])
        return path

    def handoff(self):
        self._owned()
        return {"version": 1, "environmentId": self.environment, "windowId": self.window,
                "restoreSnapshotId": self.snapshot, "expiresAtEpoch": self.expires_at,
                "backendDescriptor": str(self.observer_descriptor),
                "qa05ControlDescriptor": str(self.control_descriptor)}

    def poll(self):
        need(not self.closed)
        self._owned()
        self.guard.check()
        self.control_server.handle_request()
        self.controls.tick()
        self.observer_server.handle_request()
        self.guard.check()

    def health(self, check_all_services):
        need(not self.closed)
        self.guard.check()
        if self.fault_active:
            # A live owned fault is not healthy readiness. Keep direct child,
            # unchanged peer, source, owner and resource checks active instead.
            self.controls._check(running=False)
            return "CONTROLLED_FAULT_IN_PROGRESS"
        check_all_services()
        return "READY"

    def close(self):
        if self.closed:
            return
        self.closed = True
        failed = False
        for server in (self.control_server, self.observer_server):
            if server is not None:
                try:
                    server.server_close()
                except BaseException:
                    failed = True
        for path, identity in {**self.files, **self.sockets}.items():
            try:
                self._owned()
                info = path.lstat()
                need((info.st_dev, info.st_ino) == identity and info.st_uid == os.getuid())
                path.unlink()
            except FileNotFoundError:
                # Removal by the caller is not silent cleanup success.
                failed = True
            except BaseException:
                failed = True
        need(not failed)
