"""Synthetic protocol/failure checks; never a real Rust/Auth/PG qualification."""
from contextlib import ExitStack, nullcontext
import copy
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
spec = importlib.util.spec_from_file_location("qa05_owner_control", Path(__file__).with_name("owner_control.py"))
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)

EVENT, ENVIRONMENT, A, B = [f"00000000-0000-0000-0000-{n:012d}" for n in range(1, 5)]


class Clock:
    now = 1000.0
    def advance(self, seconds):
        self.now += seconds


class Process:
    def __init__(self, pid, binary):
        self.pid, self.args, self.code, self.terminations = pid, [str(binary)], None, 0
    def poll(self):
        return self.code
    def terminate(self):
        self.terminations += 1


class Child:
    next_pid = 200
    def __init__(self, cluster, name, argv, env, ports, reservations, secrets=()):
        Child.next_pid += 1
        self.cluster, self.name, self.ports, self.reservations = cluster, name, ports, reservations
        self.process = Process(Child.next_pid, argv[0])
        self.ready = True
        if name == "concert-service":
            cluster.epoch = "epoch-" + str(self.process.pid)
    def listeners(self):
        return self.ready


class Ports:
    def __init__(self):
        self.ports, self.closed = {"concert-service:metrics": 43219}, False
    def reserve(self, name, port):
        self.ports[name] = port
    def close(self):
        self.closed = True


class Cluster:
    def __init__(self, root):
        self.root, self.data, self.token, self.env = root, root / "data", "private-token", {}
        self.epoch, self.postmaster, self.system, self.foreign = "epoch-1", "start-1", "cluster-1", False
        self.memberships, self.stops, self.starts = [], 0, 0
        self.events = EVENT
    def assert_owned(self):
        if self.foreign:
            raise ValueError("private raw identity must never be exposed")
    def sql(self, database, query):
        if "from concert_events" in query:
            return self.events
        if "pg_control_system" in query:
            return json.dumps({"systemIdentifier": self.system, "postmasterStartedAt": self.postmaster,
                               "dataDirectory": str(self.data)})
        if "concert_world_owner_leases" in query:
            return json.dumps([{"roomId": EVENT, "ownerEpoch": self.epoch,
                                "ownerProcessId": "owner", "leaseUntil": "observed-db-time"}])
        if "concert_world_memberships" in query:
            return json.dumps(self.memberships)
        raise AssertionError("unexpected SQL")
    def stop(self):
        self.stops += 1
    def start(self):
        self.starts += 1
        self.postmaster = "start-2"


class OwnerCases(unittest.TestCase):
    def setUp(self):
        self.resources = ExitStack()
        self.addCleanup(self.resources.close)
        self.root = Path(self.resources.enter_context(tempfile.TemporaryDirectory(prefix="qa05-control-test-"))).resolve()
        self.root.chmod(0o700)
        self.clock = Clock()
        self.cluster = Cluster(self.root)
        self.guard = SimpleNamespace(check=lambda: None)
        self.pins = {n: SimpleNamespace(path=self.root / n, sha256="a" * 64) for n in control.source.SERVICES}
        self.stack = SimpleNamespace(cluster=self.cluster, ports=Ports(), children={})
        self.child = Child(self.cluster, "concert-service", [self.pins["concert-service"].path], {},
                           self.stack.ports, ("concert-service:metrics",))
        self.stack.children["concert-service"] = self.child
        for name in control.source.SERVICES - {"concert-service"}:
            self.stack.children[name] = Child(self.cluster, name, [self.pins[name].path], {}, self.stack.ports, ())
        for target, replacement in (
            ("time.time", lambda: self.clock.now), ("time.monotonic", lambda: self.clock.now),
            ("source.validate_binary_cohort", lambda _: {"pinned": True}),
            ("source.source_bytes", lambda *_: b"synthetic-source"),
            ("migration.check_ledger", lambda *_: 1),
            ("migration.execute", lambda argv, **_: SimpleNamespace(stdout="start " + argv[2])),
            ("subprocess.Popen", Process), ("world.dev.runtime.Child", Child),
            ("world.RestartPorts", Ports),
            ("world.dev.runtime.protected_signals", nullcontext),
            ("world.dev.runtime.http_probe", lambda *_: (200, b"")),
        ):
            parent, name = target.rsplit(".", 1)
            obj = control
            for part in parent.split("."):
                obj = getattr(obj, part)
            self.resources.enter_context(patch.object(obj, name, replacement))
        self.owner = control.OwnerControls(stack=self.stack, concert_child=self.child,
            concert_environment={"CONCERT_WORLD_OWNER_ID": "owner"}, listener_names=self.child.reservations,
            guard=self.guard, pins=self.pins, manifests={n: {} for n in self.pins}, source_root=self.root,
            candidate="b" * 40, origin="http://127.0.0.1:12345", event_id=EVENT,
            environment_id=ENVIRONMENT, window_id="window1", actor_user_ids=[A, B],
            expires_at=1600, validate_deployment=lambda: None)
        self.binding = self.owner.binding()

    def queue(self, postgres=False):
        before = self.owner.inspect(self.binding)["generation"]
        method = self.owner.request_postgres_concert_restart if postgres else self.owner.request_world_restart
        return method(self.binding, "window1:world-restart", before)

    def stopped(self):
        self.owner.tick()
        self.assertEqual(self.owner.checkpoint(self.binding, "window1:world-restart")["state"], "STOPPING")
        self.child.process.code = 0
        self.owner.tick()

    def complete(self, postgres=False):
        self.queue(postgres)
        self.stopped()
        self.clock.advance(5.1)
        self.owner.tick()
        self.assertEqual(self.owner.checkpoint(self.binding, "window1:world-restart")["state"], "REPLACEMENT_STARTING")
        self.owner.tick()
        return self.owner.checkpoint(self.binding, "window1:world-restart")

    def test_world_natural_expiry_changes_owner_keeps_pg(self):
        result = self.complete()
        self.assertEqual(result["state"], "COMPLETE")
        self.assertGreaterEqual(result["naturalOwnerLeaseWaitSeconds"], 5.1)
        self.assertEqual(self.cluster.starts + self.cluster.stops, 0)
        self.assertNotEqual(result["before"]["generation"]["concertPid"], result["after"]["generation"]["concertPid"])
        self.assertIs(self.stack.children["concert-service"], self.owner.child)

    def test_no_early_lease_replacement_or_poll_side_effect(self):
        self.queue()
        self.stopped()
        self.clock.advance(5)
        self.owner.tick()
        for _ in range(3):
            self.assertEqual(self.owner.checkpoint(self.binding, "window1:world-restart")["state"], "LEASE_WAIT")
        self.assertIs(self.owner.child, self.child)
        self.assertEqual(self.child.process.terminations, 1)

    def test_duplicate_request_id_same_effect_conflict_refused(self):
        first = copy.deepcopy(self.queue())
        self.owner.tick()
        current = self.owner.request_world_restart(self.binding, first["requestId"], first["before"]["generation"])
        self.assertEqual(current["state"], "STOPPING")
        self.assertEqual(self.child.process.terminations, 1)
        with self.assertRaises(control.source.InputRefusal):
            self.owner.request_world_restart(self.binding, first["requestId"], {})

    def test_stale_generation_refused_before_stop(self):
        with self.assertRaises(control.source.InputRefusal):
            self.owner.request_world_restart(self.binding, "stale", {"concertPid": self.child.process.pid})
        self.assertEqual(self.child.process.terminations, 0)

    def test_returned_checkpoint_cannot_modify_retained_generation(self):
        first = self.queue()
        first["before"]["generation"]["concertPid"] = -1
        self.owner.tick()
        self.assertEqual(self.child.process.terminations, 1)

    def test_foreign_binding_refused(self):
        for key, value in (("eventId", A), ("environmentId", B), ("candidate", "c" * 40),
                           ("members", [B, A]), ("windowId", "other")):
            with self.subTest(key=key), self.assertRaises(control.source.InputRefusal):
                self.owner.inspect({**self.binding, key: value})

    def test_foreign_cluster_never_stopped(self):
        self.queue()
        self.cluster.foreign = True
        with self.assertRaises(control.source.InputRefusal):
            self.owner.tick()
        self.assertEqual(self.child.process.terminations, 0)

    def test_child_swap_and_same_pid_cannot_grant_authority(self):
        self.queue()
        other = copy.copy(self.child)
        self.stack.children["concert-service"] = other
        with self.assertRaises(control.source.InputRefusal):
            self.owner.tick()
        self.assertEqual(self.child.process.terminations, 0)

    def test_changed_pin_refuses_before_mutation(self):
        self.queue()
        with patch.object(control.source, "validate_binary_cohort", return_value={"pinned": False}):
            with self.assertRaises(control.source.InputRefusal):
                self.owner.tick()
        self.assertEqual(self.child.process.terminations, 0)

    def test_changed_ledger_refuses_before_mutation(self):
        self.queue()
        with patch.object(control.migration, "check_ledger", side_effect=ValueError("private SQL")):
            with self.assertRaises(control.source.InputRefusal):
                self.owner.tick()
        self.assertEqual(self.child.process.terminations, 0)

    def test_wrong_principal_membership_refused(self):
        self.cluster.memberships = [{"userId": EVENT}]
        with self.assertRaises(control.source.InputRefusal):
            self.queue()
        self.assertEqual(self.child.process.terminations, 0)

    def test_other_event_refuses_global_concert_fault(self):
        self.cluster.events = EVENT + "," + A
        with self.assertRaises(control.source.InputRefusal):
            self.queue()
        self.assertEqual(self.child.process.terminations, 0)

    def test_expiry_not_extended_by_poll(self):
        self.queue()
        self.clock.advance(601)
        with self.assertRaises(control.source.InputRefusal):
            self.owner.checkpoint(self.binding, "window1:world-restart")
        with self.assertRaises(control.source.InputRefusal):
            self.owner.tick()
        self.assertEqual(self.child.process.terminations, 0)

    def test_graceful_stop_timeout_never_forces_or_replaces(self):
        self.queue()
        self.owner.tick()
        self.clock.advance(12)
        with self.assertRaises(control.source.InputRefusal):
            self.owner.tick()
        self.assertIs(self.owner.child, self.child)
        self.assertEqual(self.owner.checkpoint(self.binding, "window1:world-restart")["state"], "FAILED")

    def test_failed_exit_no_replacement(self):
        self.queue()
        self.owner.tick()
        self.child.process.code = 1
        with self.assertRaises(control.source.InputRefusal):
            self.owner.tick()
        self.assertIs(self.owner.child, self.child)

    def test_replacement_cleanup_owner_kept_when_readiness_fails(self):
        self.queue()
        self.stopped()
        self.clock.advance(5.1)
        self.owner.tick()
        replacement = self.owner.child
        with patch.object(control.world.dev.runtime, "http_probe", side_effect=ValueError("private endpoint")):
            with self.assertRaises(control.source.InputRefusal):
                self.owner.tick()
        self.assertIs(self.stack.children["concert-service"], replacement)
        self.assertIsNot(replacement, self.child)

    def test_request_operation_deadline_never_reset(self):
        self.queue()
        self.stopped()
        self.clock.advance(61)
        with self.assertRaises(control.source.InputRefusal):
            self.owner.tick()
        self.assertIs(self.owner.child, self.child)

    def test_postgres_mode_explicit_same_cluster_different_generation(self):
        result = self.complete(postgres=True)
        self.assertEqual(result["state"], "COMPLETE")
        self.assertEqual(result["action"], "POSTGRES_CONCERT_RESTART")
        self.assertEqual((self.cluster.stops, self.cluster.starts), (1, 1))
        self.assertEqual(result["before"]["generation"]["systemIdentifier"], result["after"]["generation"]["systemIdentifier"])
        self.assertNotEqual(result["before"]["generation"]["postmasterStartedAt"], result["after"]["generation"]["postmasterStartedAt"])

    def test_postgres_replacement_cluster_refused_before_spawn(self):
        self.queue(postgres=True)
        self.stopped()
        self.clock.advance(5.1)
        original = self.cluster.start
        def foreign_start():
            original()
            self.cluster.system = "foreign-cluster"
        self.cluster.start = foreign_start
        with self.assertRaises(control.source.InputRefusal):
            self.owner.tick()
        self.assertIs(self.owner.child, self.child)

    def test_private_root_permissions_rechecked(self):
        self.queue()
        self.root.chmod(0o755)
        with self.assertRaises(control.source.InputRefusal):
            self.owner.tick()
        self.assertEqual(self.child.process.terminations, 0)


class NodeProtocolCases(unittest.TestCase):
    def test_data_url_adapter_private_custody_and_closed_protocol(self):
        class ProtocolFixture:
            def __init__(self, root):
                self.root, self.root_identity, self.expires_at = root, control.private(root, "directory"), control.time.time() + 120
                self.calls = []
            def _check(self, **_):
                control.need(control.time.time() < self.expires_at)
            def binding(self):
                return {"candidate": "b" * 40, "environmentId": ENVIRONMENT, "eventId": EVENT,
                        "windowId": "node-window", "origin": "http://127.0.0.1:12345",
                        "domainHash": "d" * 64, "realtimeHash": "e" * 64, "members": [A, B]}
            def inspect(self, binding):
                control.need(binding == self.binding())
                self.calls.append("inspect")
                return {"binding": binding, "state": "OBSERVED", "generation": {"concertPid": 1}, "rooms": [], "memberships": []}
            def checkpoint(self, binding, request_id):
                control.need(binding == self.binding())
                self.calls.append("checkpoint")
                return {"binding": binding, "requestId": request_id, "action": "WORLD_RESTART", "state": "COMPLETE"}
            def request_world_restart(self, binding, request_id, before):
                control.need(binding == self.binding())
                self.calls.append("world")
                return {"binding": {} if before.get("foreign") else binding,
                        "requestId": request_id, "action": "WORLD_RESTART", "state": "QUEUED"}
            def request_postgres_concert_restart(self, binding, request_id, before):
                control.need(binding == self.binding())
                self.calls.append("postgres")
                return {"binding": binding, "requestId": request_id, "action": "POSTGRES_CONCERT_RESTART", "state": "QUEUED"}

        with tempfile.TemporaryDirectory(prefix="qa05-node-") as directory:
            root = Path(directory).resolve()
            root.chmod(0o700)
            fixture = ProtocolFixture(root)
            server = control.ControlServer(fixture)
            descriptor = root / "descriptor.json"
            descriptor.write_text(json.dumps(server.descriptor()))
            descriptor.chmod(0o600)
            stop = threading.Event()
            def serve():
                while not stop.is_set():
                    server.handle_request()
            thread = threading.Thread(target=serve)
            thread.start()
            try:
                script = r'''
import assert from 'node:assert/strict';
import { readFile, writeFile, chmod, symlink, unlink } from 'node:fs/promises';
import { createConnection } from 'node:net';
const bytes = await readFile(process.argv[1]);
const { ownerControl: owner } = await import('data:text/javascript;base64,' + bytes.toString('base64'));
const path = process.env.MCR_QA05_BACKEND_DESCRIPTOR;
const d = JSON.parse(await readFile(path, 'utf8'));
const c = { releaseRevision:d.binding.candidate, environmentId:d.binding.environmentId,
  eventId:d.binding.eventId, windowId:d.binding.windowId, origin:d.binding.origin,
  contractHashes:{ domain:d.binding.domainHash, realtime:d.binding.realtimeHash },
  members:d.binding.members.map(userId => ({ userId })) };
const refused = fn => assert.rejects(async () => await fn(), /QA05_OWNED_CONTROL_UNAVAILABLE/);
assert.equal((await owner.inspect(c)).state, 'OBSERVED');
const request = { requestId:'node-window:restart', beforeGeneration:{ concertPid:1 } };
assert.equal((await owner.requestWorldRestart(c,request)).state, 'QUEUED');
assert.equal((await owner.checkpoint(c,request.requestId)).state, 'COMPLETE');
assert.equal((await owner.requestPostgresConcertRestart(c,request)).action, 'POSTGRES_CONCERT_RESTART');
await refused(() => owner.requestWorldRestart(c,{...request, action:'inspect'}));
await refused(() => owner.requestWorldRestart(c,{requestId:'badresponse',beforeGeneration:{foreign:true}}));
await refused(() => owner.inspect({...c, windowId:'foreign'}));
await chmod(path,0o644); await refused(() => owner.inspect(c)); await chmod(path,0o600);
await chmod(d.root,0o755); await refused(() => owner.inspect(c)); await chmod(d.root,0o700);
for (const change of [{owner:'f'.repeat(48)}, {expiresAtEpoch:0},
  {rootIdentity:[0,0]}, {socketIdentity:[0,0]}]) {
  await writeFile(path,JSON.stringify({...d,...change}));
  await refused(() => owner.inspect(c));
}
await writeFile(path,JSON.stringify(d));
const link = path+'.link'; await symlink(path,link);
process.env.MCR_QA05_BACKEND_DESCRIPTOR=link;
await refused(() => owner.inspect(c));
process.env.MCR_QA05_BACKEND_DESCRIPTOR=path; await unlink(link);
async function raw(request) {
  return await new Promise((resolve,reject) => {
    const socket=createConnection(d.socket); let data='';
    socket.setTimeout(2000,()=>{socket.destroy();reject(new Error('fixture timeout'));});
    socket.on('connect',()=>socket.end(request+'\n'));
    socket.on('data',chunk=>data+=chunk);
    socket.on('error',reject); socket.on('end',()=>resolve(JSON.parse(data)));
  });
}
for (const request of [JSON.stringify({owner:d.owner,action:'erase',binding:d.binding}),
  JSON.stringify({owner:d.owner,action:'inspect',binding:d.binding,pid:1}),
  JSON.stringify({owner:'f'.repeat(48),action:'inspect',binding:d.binding}),
  '{"action":"inspect","action":"inspect"}', 'x'.repeat(8200)]) {
  assert.equal((await raw(request)).state,'UNAVAILABLE');
}
console.log('data-URL/private-custody/closed-protocol PASS');
'''
                result = subprocess.run(["node", "--input-type=module", "-e", script,
                    str(Path(__file__).with_name("control.mjs"))],
                    env={"PATH": os.environ["PATH"], "MCR_QA05_BACKEND_DESCRIPTOR": str(descriptor)},
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("closed-protocol PASS", result.stdout)
                self.assertEqual(fixture.calls, ["inspect", "world", "checkpoint", "postgres", "world"])
            finally:
                stop.set()
                thread.join(timeout=3)
                server.server_close()
                self.assertFalse(thread.is_alive())


if __name__ == "__main__":
    unittest.main()
