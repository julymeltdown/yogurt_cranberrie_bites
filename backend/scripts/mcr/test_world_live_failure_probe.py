"""Two-principal in-memory orchestration; no listener, database or load qualification."""
import asyncio
import copy
from datetime import datetime, timedelta, timezone
import io
import json
import unittest
from unittest.mock import patch
from uuid import uuid4

import world_live_failure_probe
from test_world_capacity_probe import configuration, fixture
from world_capacity_support.report import Failure
from world_capacity_support.scenarios import Runner, run
from world_capacity_support.transport import Closed, Contracts


class FixtureSocket:
    def __init__(self, network):
        self.network = network
        self.queue = asyncio.Queue()
        self.closed = self.close_ack = False
        self.actor = self.room = self.stream = None
        self.seq = self.ack = 0
        self.tick = 0
        self.players = set()

    def rows(self):
        rows = []
        for socket in self.network.sockets:
            if socket.actor and not socket.closed and socket.room == self.room:
                row = copy.deepcopy(self.network.player)
                row["userId"] = socket.actor
                rows.append(row)
        return rows

    def snapshot(self, cause="INITIAL", request=None):
        self.stream, self.seq, self.ack = str(uuid4()), 0, 0
        rows = self.rows()
        self.players = {row["userId"] for row in rows}
        value = copy.deepcopy(self.network.snapshot)
        value.update(streamId=self.stream, roomEpoch=self.network.epoch, serverTick=self.tick,
                     eventId=self.network.config["event_id"], roomId=self.room, selfUserId=self.actor,
                     cause=cause, requestId=request, players=rows)
        self.queue.put_nowait(value)

    def delta(self):
        if self.closed or not self.stream: return
        rows = self.rows()
        actors = {row["userId"] for row in rows}
        self.seq += 1
        self.tick += 1
        self.queue.put_nowait({"protocolVersion": "1.0.0", "type": "delta", "streamId": self.stream,
                              "roomEpoch": self.network.epoch, "streamSeq": self.seq,
                              "serverTick": self.tick, "ackInputSeq": self.ack,
                              "upserts": rows, "removes": sorted(self.players - actors)})
        self.players = actors

    async def send(self, value):
        self.network.contracts.validate(value, "ClientMessage")
        if value["type"] == "authenticate":
            ticket = value["ticket"]
            if ticket in self.network.consumed:
                self.queue.put_nowait(Closed(4401))
                return
            self.network.consumed.add(ticket)
            self.actor = self.network.tickets[ticket]
            self.room = value["roomId"]
            self.snapshot()
            self.network.broadcast(exclude=self)
        elif value["type"] == "input":
            self.ack = value["seq"]
            self.delta()
        elif value["type"] == "resync":
            self.snapshot("RESYNC", value["requestId"])
        elif value["type"] == "switchRoom":
            target = value["roomId"]
            capacity = next(r["capacity"] for r in self.network.config["rooms"] if r["room_id"] == target)
            occupants = sum(s.actor is not None and not s.closed and s.room == target for s in self.network.sockets)
            if occupants >= capacity:
                self.queue.put_nowait({"protocolVersion": "1.0.0", "type": "switchRejected",
                                       "requestId": value["requestId"], "streamId": self.stream, "reason": "ROOM_FULL"})
            else:
                self.room = target
                self.snapshot("ROOM_SWITCH", value["requestId"])
                self.network.broadcast(exclude=self)

    async def receive(self):
        value = await self.queue.get()
        if isinstance(value, Closed):
            self.closed = self.close_ack = True
            raise value
        return value

    async def send_close(self):
        if not self.closed:
            self.closed = True
            self.queue.put_nowait(Closed(1000))
            self.network.broadcast(exclude=self)

    async def abort(self):
        self.closed = True

    def counters(self):
        return {"frames_in": 0, "bytes_in": 0, "frames_out": 0, "bytes_out": 0,
                "max_message_bytes": 0, "max_buffered_bytes": 0, "max_write_buffer_bytes": 0}


class FixtureNetwork:
    def __init__(self, config, contracts):
        self.config, self.contracts = config, contracts
        self.sockets, self.tickets, self.consumed = [], {}, set()
        self.epoch = str(uuid4())
        self.actors = {p["bearer"]: str(uuid4()) for p in config["principals"]}
        self.snapshot = fixture("realtime/resync.json")["messages"][1]["message"]
        self.player = self.snapshot["players"][0]
        # Full fixture manifests/configs remain synthetic, never approved art.
        self.player["avatar"] = copy.deepcopy(config["bootstrap"]["avatar"])
        self.snapshot.update(mapId=config["bootstrap"]["map_manifest"]["map_id"],
                             mapRevision=config["bootstrap"]["map_manifest"]["map_revision"],
                             avatarManifestId=config["bootstrap"]["avatar_manifest"]["manifest_id"],
                             avatarManifestVersion=config["bootstrap"]["avatar_manifest"]["manifest_version"])

    async def http(self, path, token, payload=None):
        if path.endswith("/bootstrap"):
            result = copy.deepcopy(self.config["bootstrap"])
            result["participant"]["user_id"] = self.actors[token]
            result["participant"]["profile"]["user_id"] = self.actors[token]
            return 200, result
        if path.endswith("/rooms"):
            result = fixture("realtime/rooms.json")
            result.update(event_id=self.config["event_id"], default_room_id=self.config["rooms"][0]["room_id"])
            result["items"] = [{**r, "label": "Fixture room", "availability": "AVAILABLE"} for r in self.config["rooms"]]
            return 200, result
        result = fixture("realtime/ticket-response.json")
        ticket = uuid4().hex + uuid4().hex[:11]
        self.tickets[ticket] = self.actors[token]
        now = datetime.now(timezone.utc)
        result.update(payload, ticket=ticket, event_id=self.config["event_id"],
                      server_time=now.isoformat().replace("+00:00", "Z"),
                      expires_at=(now+timedelta(seconds=30)).isoformat().replace("+00:00", "Z"))
        return 200, result

    async def websocket(self):
        socket = FixtureSocket(self)
        self.sockets.append(socket)
        return socket

    def broadcast(self, exclude=None):
        for socket in self.sockets:
            if socket is not exclude: socket.delta()

    async def restart(self):
        for socket in self.sockets:
            if not socket.closed: socket.queue.put_nowait(Closed(1012))
        self.epoch = str(uuid4())


class DriverTests(unittest.IsolatedAsyncioTestCase):
    async def test_admission_refusal_reports_only_numeric_status_without_retry(self):
        config = configuration()
        contracts = Contracts()
        network = FixtureNetwork(config, contracts)
        calls = []
        async def refused(path, token, payload=None):
            calls.append((path, token))
            return 503, {"detail": "SECRET-PRIVATE-RESPONSE"}
        network.http = refused
        result = await run(config, capacity=False, network=network, contracts=contracts)
        self.assertEqual(result["failure"], "public-bootstrap-status-503")
        self.assertEqual(len(calls), len(config["principals"]))
        self.assertEqual(len(calls), len(set(calls)))
        self.assertNotIn("SECRET", json.dumps(result))

    async def execute(self, config, **kwargs):
        contracts = Contracts()
        network = FixtureNetwork(config, contracts)
        if kwargs.pop("restart", False): kwargs["restart_hook"] = network.restart
        result = await run(config, capacity=False, network=network, contracts=contracts, **kwargs)
        self.assertTrue(all(s.closed for s in network.sockets))
        return result, network

    async def test_fixture_boundary_and_missing_oracles(self):
        result, _ = await self.execute(configuration())
        self.assertEqual(result["result"], "COMPLETED", result.get("failure"))
        self.assertEqual(result["transport_boundary"], "IN_MEMORY_FIXTURE")
        self.assertEqual(result["acceptance"], "NOT_CLAIMED")
        self.assertEqual(result["server_metrics_and_cleanup_oracle"]["status"], "NOT_RUN")
        self.assertEqual(result["initial_admitted_sockets"], 2)

    async def test_replay_gap_and_accepted_room_switch(self):
        config = configuration()
        config["faults"] = ["ticket_replay", "application_gap", "room_switch"]
        result, network = await self.execute(config)
        self.assertEqual(result["result"], "COMPLETED", result.get("failure"))
        self.assertEqual(result["peak_active_sockets"], 3)
        self.assertEqual(len(result["steps"]), 3)
        public = json.dumps(result)
        for secret in list(network.tickets) + list(network.actors.values()) + [config["gateway_url"], config["principals"][0]["bearer"]]:
            self.assertNotIn(secret, public)

    async def test_full_room_rejection_retains_old_ack_progress(self):
        config = configuration()
        config["principals"][1]["room_index"] = 1
        config["rooms"][1]["capacity"] = 1
        config["faults"] = ["room_switch"]
        result, _ = await self.execute(config)
        self.assertEqual(result["result"], "COMPLETED", result.get("failure"))
        self.assertTrue(result["steps"][0]["target_full"])

    async def test_external_restart_observes_close_and_new_epoch(self):
        config = configuration()
        config["faults"] = ["world_restart"]
        result, _ = await self.execute(config, restart=True)
        self.assertEqual(result["result"], "COMPLETED", result.get("failure"))
        self.assertEqual(result["steps"][0]["observed_peer_disconnects"], 2)
        self.assertEqual(result["cleanup"]["opened_connections"], 4)

    async def test_restart_without_hook_is_not_run(self):
        config = configuration()
        config["faults"] = ["world_restart"]
        result, _ = await self.execute(config)
        self.assertEqual(result["steps"][0]["status"], "NOT_RUN_NO_PRODUCER_HOOK")

    async def test_reconnect_schedule_and_slow_reader(self):
        config = configuration()
        config["schedule"].update(measurement_seconds=.2,
            reconnect=[{"after_seconds": .01, "participants": [0, 1]}],
            slow_reader={"participant_index": 1, "after_seconds": .07, "pause_seconds": .02})
        result, _ = await self.execute(config)
        self.assertEqual(result["result"], "COMPLETED", result.get("failure"))
        self.assertEqual(result["cleanup"]["opened_connections"], 5)

    async def test_duplicate_canonical_identity_prevents_admission(self):
        config, contracts = configuration(), Contracts()
        network = FixtureNetwork(config, contracts)
        network.actors[config["principals"][1]["bearer"]] = network.actors[config["principals"][0]["bearer"]]
        result = await run(config, capacity=False, network=network, contracts=contracts)
        self.assertEqual(result["failure"], "distinct-canonical-principals")
        self.assertFalse(network.sockets)

    async def test_deadline_closes_admitted_handles_and_redacts_exception(self):
        config, contracts = configuration(), Contracts()
        config["schedule"]["total_timeout_seconds"] = .15
        config["schedule"]["measurement_seconds"] = .1
        network = FixtureNetwork(config, contracts)
        original = network.http
        async def blocked(path, token, payload=None):
            if payload is not None and token == config["principals"][1]["bearer"]: await asyncio.sleep(3)
            return await original(path, token, payload)
        network.http = blocked
        result = await run(config, capacity=False, network=network, contracts=contracts)
        self.assertEqual(result["result"], "FAIL")
        self.assertTrue(all(s.closed for s in network.sockets))
        self.assertEqual(result["cleanup"]["status"], "CLIENT_HANDLES_CLOSED")

    async def test_deadline_context_distinguishes_initial_snapshot_and_keeps_no_private_values(self):
        config, contracts = configuration(), Contracts()
        config["schedule"]["operation_timeout_seconds"] = .5
        network = FixtureNetwork(config, contracts)
        original = network.websocket
        async def silent_snapshot():
            socket = await original()
            socket.snapshot = lambda *args, **kwargs: None
            return socket
        network.websocket = silent_snapshot
        result = await run(config, capacity=False, network=network, contracts=contracts)
        self.assertEqual(result["failure"], "whole-or-operation-deadline")
        context = result["failure_context"]
        self.assertEqual(context["phase"], "initial-admission")
        self.assertEqual(context["client_stage_counts"], {"initial-snapshot": 2})
        self.assertEqual(context["active_sockets_before_cleanup"], 2)
        self.assertEqual(context["operation_timeout_seconds"], .5)
        self.assertTrue(all(socket.closed for socket in network.sockets))
        for private in [*network.actors, *network.actors.values(), config["gateway_url"], config["ws_url"]]:
            self.assertNotIn(private, json.dumps(context))

    async def test_deadline_context_distinguishes_public_bootstrap_before_sockets(self):
        config, contracts = configuration(), Contracts()
        config["schedule"]["operation_timeout_seconds"] = .2
        network = FixtureNetwork(config, contracts)
        async def stalled(*args, **kwargs):
            await asyncio.sleep(2)
        network.http = stalled
        result = await run(config, capacity=False, network=network, contracts=contracts)
        self.assertEqual(result["failure"], "whole-or-operation-deadline")
        self.assertEqual(result["failure_context"]["phase"], "prepare-bootstrap")
        self.assertEqual(result["failure_context"]["client_stage_counts"], {"bootstrap-http": 2})
        self.assertEqual(result["failure_context"]["active_sockets_before_cleanup"], 0)
        self.assertFalse(network.sockets)

    async def test_producer_oracle_whitelist(self):
        async def valid(): return {"memberships": 0, "prepared_transactions": 0}
        result, _ = await self.execute(configuration(), observation_hook=valid)
        self.assertEqual(result["server_metrics_and_cleanup_oracle"]["memberships"], 0)
        async def invalid(): return {"raw_sql": "SECRET"}
        result, _ = await self.execute(configuration(), observation_hook=invalid)
        self.assertEqual(result["server_metrics_and_cleanup_oracle"]["status"], "UNAVAILABLE")
        self.assertNotIn("SECRET", json.dumps(result))


class CheckpointTests(unittest.TestCase):
    def test_restart_checkpoint_requires_separate_private_file(self):
        with patch.object(world_live_failure_probe, "read_private", side_effect=AssertionError), patch("sys.stdout", new_callable=io.StringIO) as output:
            self.assertEqual(world_live_failure_probe.main(["--execute", "--restart-checkpoint"]), 1)
        self.assertEqual(json.loads(output.getvalue())["failure"], "restart-needs-private-file-input")


if __name__ == "__main__": unittest.main()
