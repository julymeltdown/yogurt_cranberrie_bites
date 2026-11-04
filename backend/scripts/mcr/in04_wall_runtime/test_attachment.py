"""Synthetic attachment lifecycle tests; no product services/Auth/PostgreSQL."""
from contextlib import ExitStack
import json
import os
from pathlib import Path
import socket
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from in04_wall_runtime import attachment as module


IDS = [f"00000000-0000-0000-0000-{i:012d}" for i in range(1, 7)]


class Controls:
    def __init__(self, **kwargs):
        self.kwargs, self.child, self.env = kwargs, kwargs["concert_child"], kwargs["concert_environment"]
        self.active, self.calls = None, []
        kwargs["validate_deployment"]()
    def binding(self):
        return {"members": self.kwargs["actor_user_ids"]}
    def inspect(self, binding):
        self.calls.append(("inspect", binding))
        return {"state": "OBSERVED"}
    def tick(self):
        self.calls.append("tick")
        self.kwargs["validate_deployment"]()
    def _check(self, **kwargs):
        self.calls.append(("check", kwargs))


class Observations:
    def __init__(self, **kwargs):
        self.kwargs, self.calls = kwargs, []
        kwargs["validate_deployment"]()
    def binding(self):
        return {"members": [s[0] for s in self.kwargs["actor_sessions"][:2]]}
    def observe(self, binding):
        self.calls.append(binding)
        return {"state": "OBSERVED"}


class Server:
    def __init__(self, target, filename):
        self.target, self.requests, self.closed = target, 0, False
        root = target.kwargs["stack"].cluster.root
        self.server_address = str(root / filename)
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            self.socket.bind(self.server_address)
            os.chmod(self.server_address, 0o600)
        except BaseException:
            self.socket.close()
            raise
    def descriptor(self):
        return {"version": 1, "root": str(self.target.kwargs["stack"].cluster.root),
                "owner": "f" * 48, "socket": self.server_address}
    def handle_request(self):
        self.requests += 1
    def server_close(self):
        self.closed = True
        self.socket.close()


class AttachmentCases(unittest.TestCase):
    def setUp(self):
        self.resources = ExitStack()
        self.addCleanup(self.resources.close)
        self.root = Path(self.resources.enter_context(tempfile.TemporaryDirectory(prefix="ma-"))).resolve()
        self.root.chmod(0o700)
        self.cluster = SimpleNamespace(root=self.root, token="owned-cluster", assert_owned=Mock())
        self.child = SimpleNamespace(reservations=("concert-service:world", "concert-service:metrics"))
        self.stack = SimpleNamespace(cluster=self.cluster, children={"concert-service": self.child})
        self.guard = SimpleNamespace(check=Mock())
        self.context = {"env": {"SECRET": "never-serialize-env"}, "event": IDS[0]}
        self.principals = [(IDS[i], self.root / f"credential-{i}.json", f"never-serialize-bearer-{i}")
                           for i in (1, 2, 3)]
        self.deployment = Mock()
        self.expiry = time.time() + 300
        self.inputs = {"environment_id": IDS[4], "window_id": "current-window",
                       "restore_snapshot_id": IDS[5], "expires_at": self.expiry,
                       "actor_sessions": [(p[0], p[2]) for p in self.principals],
                       "validate_deployment": self.deployment}
        self.arguments = dict(stack=self.stack, browser=object(), guard=self.guard, pins={"actual": object()},
            manifests={"actual": object()}, context=self.context, principals=self.principals,
            source_root=self.root, candidate="a" * 40, origin="http://127.0.0.1:3000", expires_at=self.expiry)
        for name, value in (("OwnerControls", Controls), ("Observations", Observations),
            ("ControlServer", lambda target: Server(target, "qa05-control.sock")),
            ("ReadOnlyControl", lambda target: Server(target, "in04-observe.sock"))):
            self.resources.enter_context(patch.object(module, name, value))

    def attach(self, inputs=None):
        value = module.ManagedAttachment(inputs=self.inputs if inputs is None else inputs, **self.arguments)
        self.resources.callback(value.close)
        return value

    def test_actual_objects_sessions_and_observations_precede_descriptors(self):
        value = self.attach()
        self.assertIs(value.controls.kwargs["concert_child"], self.child)
        self.assertIs(value.controls.kwargs["stack"], self.stack)
        self.assertIs(value.observations.kwargs["browser"], self.arguments["browser"])
        self.assertEqual(value.controls.kwargs["actor_user_ids"], [IDS[1], IDS[2]])
        self.assertEqual(value.observations.kwargs["actor_sessions"], tuple(self.inputs["actor_sessions"]))
        self.assertTrue(value.observations.calls and value.controls.calls)
        self.assertGreaterEqual(self.deployment.call_count, 2)

    def test_private_handoff_never_serializes_principal_tokens_or_environment(self):
        value = self.attach()
        handoff = value.handoff()
        self.assertEqual(handoff["environmentId"], IDS[4])
        for name in ("backendDescriptor", "qa05ControlDescriptor"):
            path = Path(handoff[name])
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            raw = path.read_text()
            self.assertNotIn("never-serialize", raw)
            self.assertEqual(json.loads(raw)["version"], 1)
        self.assertNotIn("never-serialize", json.dumps(handoff))

    def test_profile_supplied_metadata_cannot_leak_bearers_in_keys_or_values(self):
        token = self.principals[0][2]
        for value in ({"proof": {"authorization": "Bearer " + token}}, {token: "value"}):
            self.assertTrue(module.contains_access_token(value, self.principals))
        self.assertFalse(module.contains_access_token({"tokens": "MEMORY_ONLY", "user": IDS[1]}, self.principals))

    def test_forged_or_reordered_canonical_sessions_refused(self):
        for sessions in (self.inputs["actor_sessions"][:2], list(reversed(self.inputs["actor_sessions"])),
                         [(IDS[0], "forged-token"), *self.inputs["actor_sessions"][1:]]):
            with self.subTest(sessions=len(sessions)), self.assertRaises(ValueError):
                self.attach({**self.inputs, "actor_sessions": sessions})
        self.assertEqual(list(self.root.iterdir()), [])

    def test_extra_claim_field_and_missing_real_callback_refused(self):
        for inputs in ({**self.inputs, "ready": True}, {**self.inputs, "validate_deployment": True}):
            with self.subTest(), self.assertRaises(ValueError):
                self.attach(inputs)

    def test_expiry_cannot_extend_existing_owner_window(self):
        for expiry in (self.expiry + 1, time.time() - 1, float("nan"), True):
            with self.subTest(), self.assertRaises(ValueError):
                self.attach({**self.inputs, "expires_at": expiry})

    def test_source_validation_failure_does_not_publish_a_descriptor(self):
        self.deployment.side_effect = ValueError("private deployment material")
        with self.assertRaises(ValueError):
            self.attach()
        self.assertEqual(list(self.root.iterdir()), [])

    def test_real_role_observation_failure_cannot_be_configured_ready(self):
        with patch.object(Observations, "observe", side_effect=ValueError("actual operator denied")):
            with self.assertRaises(ValueError):
                self.attach()
        self.assertEqual(list(self.root.iterdir()), [])

    def test_health_relaxation_only_for_current_active_control(self):
        value = self.attach()
        full = Mock()
        self.assertEqual(value.health(full), "READY")
        full.assert_called_once_with()
        value.controls.active = "current-request"
        self.assertEqual(value.health(full), "CONTROLLED_FAULT_IN_PROGRESS")
        full.assert_called_once_with()
        self.assertIn(("check", {"running": False}), value.controls.calls)
        value.controls.active = None
        self.assertEqual(value.health(full), "READY")
        self.assertEqual(full.call_count, 2)

    def test_active_fault_observer_refuses_without_query_or_health(self):
        value = self.attach()
        before = len(value.observations.calls)
        value.controls.active = "actual-request"
        with self.assertRaises(ValueError):
            value.observations.observe(value.observations.binding())
        self.assertEqual(len(value.observations.calls), before)

    def test_current_env_tracks_only_registered_control_replacement(self):
        value = self.attach()
        replacement = SimpleNamespace(reservations=self.child.reservations)
        value.controls.child, value.controls.env = replacement, {"ACTUAL_NEW_OWNER": "observed"}
        # An unregistered caller-provided child cannot rewrite context.
        value.controls.kwargs["validate_deployment"]()
        self.assertNotIn("ACTUAL_NEW_OWNER", self.context["env"])
        self.stack.children["concert-service"] = replacement
        value.poll()
        self.assertIs(self.context["env"], value.controls.env)
        self.assertEqual(value.control_server.requests, 1)
        self.assertEqual(value.observer_server.requests, 1)
        self.assertIn("tick", value.controls.calls)

    def test_existing_descriptor_is_preserved_and_partial_attachment_cleaned(self):
        path = self.root / "in04-owner-observer.json"
        path.write_text("preserve existing owner data")
        with self.assertRaises(FileExistsError):
            self.attach()
        self.assertEqual(path.read_text(), "preserve existing owner data")
        self.assertEqual(list(self.root.iterdir()), [path])

    def test_foreign_descriptor_inode_is_not_deleted_other_resources_close(self):
        value = self.attach()
        foreign = self.root / "replacement"
        foreign.write_text("foreign")
        foreign.replace(value.control_descriptor)
        with self.assertRaises(ValueError):
            value.close()
        self.assertEqual(value.control_descriptor.read_text(), "foreign")
        self.assertTrue(value.control_server.closed and value.observer_server.closed)
        self.assertFalse(value.observer_descriptor.exists())

    def test_cleanup_is_idempotent_and_retires_files_and_sockets(self):
        value = self.attach()
        value.close(); value.close()
        self.assertEqual(list(self.root.iterdir()), [])
        with self.assertRaises(ValueError):
            value.poll()


if __name__ == "__main__":
    unittest.main()
