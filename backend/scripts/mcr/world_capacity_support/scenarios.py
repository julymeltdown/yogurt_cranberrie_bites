"""Public admission, bounded client state, capacity schedules and failure scenarios.

Public stream observations are not a server queue or database cleanup oracle.
"""
from __future__ import annotations

import asyncio
from datetime import datetime, timezone
import hashlib
import json
import re
import time
from uuid import UUID, uuid4

from .report import Failure, Samples, check, elapsed, fields, integer, new_report, number, observations
from .transport import Closed, Contracts, Network, browser_origin, endpoint

PIN_HTTP = ("map_id", "map_revision", "avatar_manifest_id", "avatar_manifest_version")
PIN_WS = ("mapId", "mapRevision", "avatarManifestId", "avatarManifestVersion")


def uuid(value):
    try:
        check(isinstance(value, str) and UUID(value).int != 0 and str(UUID(value)) == value, "uuid")
    except (ValueError, TypeError, AttributeError):
        raise Failure("uuid") from None
    return value


def validate_config(value, capacity):
    fields(value, {"allocation_id", "gateway_url", "ws_url", "browser_origin", "event_id", "bootstrap",
                   "principals", "rooms", "schedule"}, {"faults"})
    check(isinstance(value["allocation_id"], str) and 1 <= len(value["allocation_id"]) <= 128, "allocation-required")
    endpoint(value["gateway_url"])
    endpoint(value["ws_url"], True)
    browser_origin(value["browser_origin"])
    uuid(value["event_id"])
    check(isinstance(value["bootstrap"], dict), "bootstrap-input")
    rooms, principals = value["rooms"], value["principals"]
    check(isinstance(rooms, list) and 1 <= len(rooms) <= 16 and isinstance(principals, list), "cohort-input")
    integer(len(principals), 200 if capacity else 2, 200 if capacity else 50, "principal-count")
    if capacity:
        check(len(rooms) == 4, "original-four-rooms")
    ids = []
    counts = [0] * len(rooms)
    tokens = set()
    for room in rooms:
        fields(room, {"room_id", "capacity"})
        ids.append(uuid(room["room_id"]))
        integer(room["capacity"], 50 if capacity else 1, 50, "room-capacity")
    check(len(set(ids)) == len(ids), "duplicate-room")
    for principal in principals:
        fields(principal, {"bearer", "room_index"})
        token = principal["bearer"]
        check(isinstance(token, str) and re.fullmatch(r"[A-Za-z0-9._~-]{1,8192}", token) is not None, "bearer-shape")
        check(token not in tokens, "duplicate-bearer")
        tokens.add(token)
        counts[integer(principal["room_index"], 0, len(rooms) - 1, "room-index")] += 1
    check(all(count <= room["capacity"] for count, room in zip(counts, rooms)), "cohort-over-capacity")
    if capacity:
        check(counts == [50] * 4, "original-fifty-per-room")
    schedule = value["schedule"]
    fields(schedule, {"arrival_concurrency", "operation_timeout_seconds", "measurement_seconds", "total_timeout_seconds", "input_hz"},
           {"reconnect", "slow_reader"})
    integer(schedule["arrival_concurrency"], 1, len(principals), "arrival-concurrency")
    number(schedule["operation_timeout_seconds"], .05, 60, "operation-timeout")
    number(schedule["measurement_seconds"], .05, 3600, "measurement-duration")
    number(schedule["total_timeout_seconds"], .1, 7200, "total-timeout")
    check(schedule["total_timeout_seconds"] > schedule["measurement_seconds"], "whole-deadline")
    number(schedule["input_hz"], 0, 30, "input-rate")
    reconnects = schedule.get("reconnect", [])
    check(isinstance(reconnects, list) and len(reconnects) <= 16, "reconnect-count")
    for row in reconnects:
        fields(row, {"after_seconds", "participants"})
        number(row["after_seconds"], 0, schedule["measurement_seconds"], "reconnect-time")
        indices = row["participants"]
        check(isinstance(indices, list) and bool(indices), "reconnect-indices")
        for index in indices:
            integer(index, 0, len(principals) - 1, "reconnect-index")
        check(len(set(indices)) == len(indices), "reconnect-indices")
    slow = schedule.get("slow_reader")
    if slow is not None:
        fields(slow, {"participant_index", "after_seconds", "pause_seconds"})
        integer(slow["participant_index"], 0, len(principals) - 1, "slow-reader-index")
        number(slow["after_seconds"], 0, schedule["measurement_seconds"], "slow-reader-start")
        number(slow["pause_seconds"], .01, schedule["measurement_seconds"], "slow-reader-duration")
        check(slow["after_seconds"] + slow["pause_seconds"] <= schedule["measurement_seconds"], "slow-reader-window")
    faults = value.get("faults", [])
    allowed = {"ticket_replay", "application_gap", "application_delay", "slow_receiver", "room_switch", "world_restart"}
    check(isinstance(faults, list) and len(faults) <= len(allowed)
          and all(isinstance(f, str) and f in allowed for f in faults) and len(set(faults)) == len(faults), "fault-scenarios")
    check(not capacity or not faults, "capacity-fault-selection")
    return value


def release_pins(document):
    return (document["map_manifest"]["map_id"], document["map_manifest"]["map_revision"],
            document["avatar_manifest"]["manifest_id"], document["avatar_manifest"]["manifest_version"])


class StreamState:
    def __init__(self, actor, event, room, pins, contracts):
        self.actor, self.event, self.room, self.pins, self.contracts = actor, event, room, pins, contracts
        self.stream = self.epoch = None
        self.seq = self.tick = self.ack = self.sent = 0
        self.players, self.pending, self.sent_at = {}, None, {}
        self.ack_seconds, self.tick_gaps = Samples(), Samples()
        self.resets = self.deltas = self.removals = 0
        self.drop_next_delta = False
        self.dropped = self.gaps = 0
        self.rejection = None
        self.used_streams = set()
        self.access_expiry = None

    def accept(self, value, now):
        self.contracts.validate(value, "ServerMessage")
        kind = value["type"]
        if kind == "ping":
            return {"protocolVersion": "1.0.0", "type": "pong", "heartbeatId": value["heartbeatId"]}
        if kind == "switchRejected":
            check(self.pending is not None and self.pending[0] == "ROOM_SWITCH"
                  and value["requestId"] == self.pending[1] and value["streamId"] == self.stream, "switch-rejection-binding")
            self.rejection, self.pending = value["reason"], None
            return None
        if kind in ("revoke", "resyncRequired"):
            raise Failure("server-revoked-or-resync-required")
        check(kind in ("snapshot", "delta"), "server-message-kind")
        if kind == "snapshot":
            check(value["eventId"] == self.event and tuple(value[k] for k in PIN_WS) == self.pins, "stream-resource-binding")
            check(value["streamId"] not in self.used_streams and value["ackInputSeq"] == 0, "reset-stream-or-ack")
            check(self.access_expiry in (None, value["accessExpiresAt"]), "access-deadline-changed")
            old_stream = self.stream
            old_room = self.room
            if old_stream is None:
                check(value["cause"] == "INITIAL" and value["requestId"] is None, "initial-snapshot")
                check(value["ackInputSeq"] == 0, "initial-ack")
            else:
                check(self.pending is not None and value["cause"] == self.pending[0]
                      and value["requestId"] == self.pending[1] and value["streamId"] != old_stream, "reset-correlation")
                if self.pending[0] == "ROOM_SWITCH":
                    self.room = self.pending[2]
                self.resets += 1
                check(value["roomEpoch"] != self.epoch or value["roomId"] != old_room
                      or value["serverTick"] >= self.tick, "reset-tick")
            check(value["roomId"] == self.room and value["selfUserId"] == self.actor, "snapshot-membership")
            self.stream, self.epoch = value["streamId"], value["roomEpoch"]
            self.seq, self.tick, self.ack = 0, value["serverTick"], value["ackInputSeq"]
            self.sent = self.ack
            self.players = {p["userId"]: p for p in value["players"]}
            self.sent_at.clear()
            self.pending = None
            self.used_streams.add(self.stream)
            check(len(self.used_streams) <= 128, "stream-reset-bound")
            self.access_expiry = value["accessExpiresAt"]
        else:
            check(self.stream is not None and value["streamId"] == self.stream
                  and value["roomEpoch"] == self.epoch, "delta-binding")
            if self.drop_next_delta:
                self.drop_next_delta, self.dropped = False, self.dropped + 1
                return None
            if self.pending and self.pending[0] == "RESYNC":
                return None  # Withdrawn state stays empty until its correlated baseline.
            if value["streamSeq"] != self.seq + 1:
                check(value["streamSeq"] > self.seq + 1 and self.dropped > self.gaps, "unexpected-stream-gap")
                self.gaps += 1
                request = str(uuid4())
                self.pending = ("RESYNC", request, self.room)
                self.players.clear()
                return {"protocolVersion": "1.0.0", "type": "resync", "requestId": request,
                        "streamId": self.stream, "roomEpoch": self.epoch, "lastStreamSeq": self.seq, "reason": "GAP"}
            check(value["serverTick"] >= self.tick and self.ack <= value["ackInputSeq"] <= self.sent, "tick-ack-binding")
            self.tick_gaps.add(value["serverTick"] - self.tick)
            for sequence in list(self.sent_at):
                if sequence <= value["ackInputSeq"]:
                    self.ack_seconds.add(now - self.sent_at.pop(sequence))
            for actor in value["removes"]:
                check(actor in self.players and actor != self.actor, "unexpected-removal")
                del self.players[actor]
                self.removals += 1
            self.players.update((p["userId"], p) for p in value["upserts"])
            for player in value["upserts"]:
                config = player["avatar"]["config"]
                check((config["manifest_id"], config["manifest_version"]) == self.pins[2:], "avatar-resource-binding")
            self.seq, self.tick, self.ack = value["streamSeq"], value["serverTick"], value["ackInputSeq"]
            self.deltas += 1
        check(self.actor in self.players and len(self.players) <= 50 and self.ack <= self.sent, "stream-roster-bound")
        return None

    def input(self, now):
        check(self.stream is not None and self.pending is None and len(self.sent_at) < 64, "pending-input-bound")
        self.sent += 1
        self.sent_at[self.sent] = now
        return {"protocolVersion": "1.0.0", "type": "input", "streamId": self.stream,
                "roomEpoch": self.epoch, "seq": self.sent, "axisX": 0, "axisY": 0, "buttons": []}


class Client:
    def __init__(self, runner, index):
        self.runner, self.index = runner, index
        principal = runner.config["principals"][index]
        self.token, self.room_index = principal["bearer"], principal["room_index"]
        self.actor = None
        self.state = None
        self.ws = self.reader_task = self.sender_task = None
        self.error = None
        self.closing = False
        self.pause_until = 0
        self.expected_slow_close = False
        self.close_code = None
        self.ticket_message = None
        self.allowed_rooms = {self.room_index}
        self.counted = self.expected_restart = self.peer_disconnected = False

    async def bootstrap(self):
        status, document = await self.runner.http("bootstrap", self.token)
        check(status == 200, "public-bootstrap-status")
        actor = self.runner.contracts.bootstrap(document, self.runner.config["event_id"])
        check(self.actor in (None, actor), "principal-changed")
        expected = self.runner.config["bootstrap"]
        check(all(document[k] == expected[k] for k in ("map_manifest", "avatar_manifest", "theme_manifests")), "release-mismatch")
        self.actor = actor

    async def ticket(self, room_index):
        room = self.runner.config["rooms"][room_index]["room_id"]
        request = {"room_id": room, **dict(zip(PIN_HTTP, self.runner.pins))}
        status, value = await self.runner.http("world-tickets", self.token, request)
        check(status == 200, "public-ticket-status")
        self.runner.contracts.validate(value, "WorldTicketResponse")
        check(value["event_id"] == self.runner.config["event_id"] and all(value[k] == v for k, v in request.items()), "ticket-binding")
        expiry = datetime.fromisoformat(value["expires_at"])
        check(expiry > datetime.now(timezone.utc) and value["auth_timeout_ms"] == 3000 and value["ws_path"] == "/world-ws", "ticket-expired")
        return {"protocolVersion": "1.0.0", "type": "authenticate", "ticket": value["ticket"],
                "eventId": self.runner.config["event_id"], "roomId": room, **dict(zip(PIN_WS, self.runner.pins))}

    async def connect(self):
        start = time.monotonic()
        old_stream = self.state.stream if self.state else None
        await self.bootstrap()
        self.ticket_message = await self.ticket(self.room_index)
        self.ws = await self.runner.network.websocket()
        self.reader_task = None
        self.runner.opened += 1
        self.runner.active += 1
        self.counted = True
        self.runner.peak = max(self.runner.peak, self.runner.active)
        self.closing, self.close_code, self.error = False, None, None
        self.peer_disconnected = False
        self.pause_until = 0
        room = self.runner.config["rooms"][self.room_index]["room_id"]
        self.state = StreamState(self.actor, self.runner.config["event_id"], room, self.runner.pins, self.runner.contracts)
        await self.ws.send(self.ticket_message)
        self.reader_task = asyncio.create_task(self.read())
        await self.runner.wait(lambda: self.state.stream is not None, "snapshot-deadline")
        check(old_stream is None or self.state.stream != old_stream, "reconnect-reused-stream")
        self.runner.connect_seconds.add(time.monotonic() - start)
        self.sender_task = asyncio.create_task(self.send_inputs())

    async def read(self):
        try:
            while True:
                if self.pause_until > time.monotonic():
                    pause_start = time.monotonic()
                    try:
                        await asyncio.sleep(self.pause_until - pause_start)
                    finally:
                        self.runner.read_pauses.add(time.monotonic() - pause_start)
                value = await self.ws.receive()
                if self.expected_slow_close and value.get("type") == "resyncRequired" and value.get("reason") == "BACKPRESSURE":
                    self.runner.contracts.validate(value, "ServerMessage")
                    self.runner.slow_backpressure_controls += 1
                    self.state.players.clear()
                    self.state.pending = ("BACKPRESSURE", None, self.state.room)
                    continue
                reply = self.state.accept(value, time.monotonic())
                if reply:
                    self.runner.contracts.validate(reply, "ClientMessage")
                    await self.ws.send(reply)
                if self.state.room != self.runner.config["rooms"][self.room_index]["room_id"]:
                    self.room_index = next(i for i, room in enumerate(self.runner.config["rooms"])
                                           if room["room_id"] == self.state.room)
                self.runner.check_roster(self)
                await asyncio.sleep(0)
        except Closed as error:
            self.close_code = error.code
            self.peer_disconnected = True
            if not self.closing and not self.expected_slow_close and not self.expected_restart:
                self.error = Failure("unexpected-websocket-close")
        except asyncio.CancelledError:
            raise
        except BaseException as error:
            if self.expected_restart and isinstance(error, (OSError, asyncio.IncompleteReadError)):
                self.peer_disconnected, self.ws.closed, self.close_code = True, True, 1006
            else:
                self.error = error if isinstance(error, Failure) else Failure("receive-transport")
        finally:
            self.finish_active()

    def finish_active(self):
        if self.counted:
            self.runner.active -= 1
            self.counted = False

    async def send_inputs(self):
        hz = self.runner.config["schedule"]["input_hz"]
        if hz == 0:
            return
        try:
            while not self.closing and not self.ws.closed:
                start = time.monotonic()
                if self.state.pending is None and len(self.state.sent_at) < 64:
                    message = self.state.input(start)
                    self.runner.contracts.validate(message, "ClientMessage")
                    await self.ws.send(message)
                else:
                    self.runner.input_backpressure += 1
                await asyncio.sleep(1 / hz)
                self.runner.loop_delay.add(max(0, time.monotonic() - start - 1 / hz))
        except asyncio.CancelledError:
            raise
        except BaseException as error:
            if not self.expected_restart:
                self.error = error if isinstance(error, Failure) else Failure("send-transport")

    async def stop(self):
        if self.ws is None:
            return
        self.closing = True
        if self.sender_task:
            self.sender_task.cancel()
            await asyncio.gather(self.sender_task, return_exceptions=True)
        try:
            await asyncio.wait_for(self.ws.send_close(), .5)
            if self.reader_task:
                await asyncio.wait_for(asyncio.shield(self.reader_task), 1)
        except (Exception, asyncio.CancelledError):
            if self.reader_task:
                self.reader_task.cancel()
                await asyncio.gather(self.reader_task, return_exceptions=True)
        finally:
            self.runner.close_acks += int(self.ws.close_ack)
            self.runner.counters.append(self.ws.counters())
            await self.ws.abort()
            self.ws = None
            self.finish_active()


class Runner:
    def __init__(self, config, capacity, network=None, contracts=None):
        self.config = validate_config(config, capacity)
        self.capacity = capacity
        self.contracts = contracts or Contracts()
        self.contracts.bootstrap(config["bootstrap"], config["event_id"])
        self.pins = release_pins(config["bootstrap"])
        self.network = network or Network(config)
        self.fixture = network is not None
        self.base = "/api/concert/events/" + config["event_id"] + "/"
        self.clients = [Client(self, n) for n in range(len(config["principals"]))]
        self.opened = self.active = self.peak = self.close_acks = self.input_backpressure = 0
        self.connect_seconds, self.http_seconds, self.loop_delay = Samples(), Samples(), Samples()
        self.counters = []
        self.read_pauses = Samples()
        self.slow_backpressure_controls = 0
        self.by_actor = {}

    async def http(self, suffix, token, payload=None):
        start = time.monotonic()
        async with asyncio.timeout(self.config["schedule"]["operation_timeout_seconds"]):
            result = await self.network.http(self.base + suffix, token, payload)
        self.http_seconds.add(time.monotonic() - start)
        return result

    def check_errors(self):
        for client in self.clients:
            if client.error:
                raise client.error

    def check_roster(self, client):
        for actor in client.state.players:
            other = self.by_actor.get(actor)
            check(other is not None and client.room_index in other.allowed_rooms, "cross-room-or-unknown-player")

    async def wait(self, predicate, label):
        async with asyncio.timeout(self.config["schedule"]["operation_timeout_seconds"]):
            while not predicate():
                self.check_errors()
                await asyncio.sleep(.005)
        self.check_errors()

    async def parallel(self, items, work):
        semaphore = asyncio.Semaphore(self.config["schedule"]["arrival_concurrency"])
        async def call(item):
            async with semaphore:
                async with asyncio.timeout(self.config["schedule"]["operation_timeout_seconds"]):
                    return await work(item)
        tasks = [asyncio.create_task(call(item)) for item in items]
        try:
            return await asyncio.gather(*tasks)
        finally:
            for task in tasks:
                if not task.done():
                    task.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)

    async def prepare(self):
        await self.parallel(self.clients, lambda c: c.bootstrap())
        self.by_actor = {c.actor: c for c in self.clients}
        check(len(self.by_actor) == len(self.clients), "distinct-canonical-principals")
        for client in self.clients:
            status, catalog = await self.http("rooms", client.token)
            check(status == 200, "public-rooms-status")
            self.contracts.validate(catalog, "WorldRooms")
            check(catalog["event_id"] == self.config["event_id"], "room-catalog-event")
            rooms = {r["room_id"]: r for r in catalog["items"]}
            for requested in self.config["rooms"]:
                actual = rooms.get(requested["room_id"])
                check(actual is not None and actual["capacity"] == requested["capacity"]
                      and actual["availability"] == "AVAILABLE", "registered-room-capacity")
        await self.parallel(self.clients, lambda c: c.connect())
        await self.converge()

    async def converge(self):
        def complete():
            active = [c for c in self.clients if c.ws and not c.ws.closed and not c.closing]
            for client in active:
                expected = {c.actor for c in active if c.room_index == client.room_index}
                if client.state.pending is not None or set(client.state.players) != expected:
                    return False
            return True
        await self.wait(complete, "roster-convergence")
        for client in self.clients:
            client.allowed_rooms = {client.room_index}

    async def reconnect(self, indices):
        start = time.monotonic()
        selected = [self.clients[i] for i in indices]
        await self.parallel(selected, lambda c: c.stop())
        await self.converge()
        await self.parallel(selected, lambda c: c.connect())
        await self.converge()
        return elapsed(start)

    async def switch(self, client, target, expect_full):
        start = time.monotonic()
        old_room, old_stream = client.room_index, client.state.stream
        auth = await client.ticket(target)
        request = str(uuid4())
        client.state.pending = ("ROOM_SWITCH", request, auth["roomId"])
        client.allowed_rooms.add(target)
        client.state.rejection = None
        # Permit the new baseline's roster check to use the intended room only
        # after a correlated accepted snapshot changes StreamState.room.
        message = {**auth, "type": "switchRoom", "requestId": request, "streamId": old_stream}
        self.contracts.validate(message, "ClientMessage")
        await client.ws.send(message)
        await self.wait(lambda: client.state.pending is None, "room-switch-deadline")
        if expect_full:
            check(client.state.rejection == "ROOM_FULL" and client.room_index == old_room
                  and client.state.stream == old_stream and client.actor in client.state.players, "full-room-retained-membership")
            old_ack = client.state.ack
            await client.ws.send(client.state.input(time.monotonic()))
            await self.wait(lambda: client.state.ack > old_ack, "full-room-old-stream-progress")
        else:
            check(client.state.rejection is None and client.state.room == auth["roomId"]
                  and client.state.stream != old_stream, "room-switch-admission")
            client.room_index = target
        await self.converge()
        return elapsed(start)

    async def measure(self, report):
        schedule = self.config["schedule"]
        start = time.monotonic()
        actions = sorted(schedule.get("reconnect", []), key=lambda r: r["after_seconds"])
        slow = schedule.get("slow_reader")
        slow_started, slow_finished = False, False
        minimum = self.active
        initial_ticks = [c.state.tick for c in self.clients]
        while time.monotonic() - start < schedule["measurement_seconds"]:
            now = time.monotonic() - start
            self.check_errors()
            minimum = min(minimum, self.active)
            if slow and not slow_started and now >= slow["after_seconds"]:
                client = self.clients[slow["participant_index"]]
                client.pause_until = time.monotonic() + slow["pause_seconds"]
                client.expected_slow_close = True
                slow_started = True
            if slow_started and not slow_finished and now >= slow["after_seconds"] + slow["pause_seconds"]:
                client = self.clients[slow["participant_index"]]
                await self.wait(lambda: client.pause_until <= time.monotonic(), "slow-reader-deadline")
                await self.reconnect([client.index])
                client.expected_slow_close = False
                slow_finished = True
            if actions and now >= actions[0]["after_seconds"]:
                action = actions.pop(0)
                seconds = await self.reconnect(action["participants"])
                report["steps"].append({"kind": "scheduled-reconnect", "participants": len(action["participants"]), "seconds": seconds})
            await asyncio.sleep(.01)
        for action in actions:
            report["steps"].append({"kind": "scheduled-reconnect", "status": "NOT_RUN_WINDOW_ENDED", "participants": len(action["participants"])})
        check(not actions and (not slow or slow_finished), "measurement-schedule-window-ended")
        await self.converge()
        check(self.active == len(self.clients), "measurement-final-cohort-incomplete")
        report["measurement"] = {"requested_seconds": schedule["measurement_seconds"], "actual_seconds": elapsed(start),
                                 "minimum_sampled_active_sockets": minimum, "active_at_end": self.active,
                                 "clients_with_latest_stream_tick_above_initial": sum(c.state.tick > t for c, t in zip(self.clients, initial_ticks)),
                                 "slow_reader": "ACTUAL_SOCKET_READ_PAUSE" if slow_started else "NOT_RUN"}

    async def failures(self, report, restart_hook):
        client = self.clients[0]
        for fault in self.config.get("faults", []):
            start = time.monotonic()
            row = {"kind": fault, "status": "OBSERVED"}
            if fault == "ticket_replay":
                ws = await self.network.websocket()
                self.opened += 1
                self.active += 1
                self.peak = max(self.peak, self.active)
                try:
                    await ws.send(client.ticket_message)
                    async with asyncio.timeout(self.config["schedule"]["operation_timeout_seconds"]):
                        while True:
                            try:
                                message = await ws.receive()
                            except Closed as closed:
                                check(closed.code == 4401, "ticket-replay-close")
                                break
                            self.contracts.validate(message, "ServerMessage")
                            check(message["type"] not in ("snapshot", "delta"), "ticket-replay-admitted")
                            check(message["type"] == "revoke", "ticket-replay-response")
                finally:
                    self.close_acks += int(ws.close_ack)
                    self.counters.append(ws.counters())
                    await ws.abort()
                    self.active -= 1
            elif fault == "application_gap":
                check(self.config["schedule"]["input_hz"] > 0, "gap-requires-ack-traffic")
                before = client.state.resets
                client.state.drop_next_delta = True
                await self.wait(lambda: client.state.resets > before, "gap-resync-deadline")
                await self.converge()
                row["boundary"] = "ONE_RECEIVED_APPLICATION_DELTA_DISCARDED_NOT_NETWORK_LOSS"
            elif fault in ("application_delay", "slow_receiver"):
                slow = self.config["schedule"].get("slow_reader")
                check(slow is not None, "fault-delay-config-required")
                delayed = self.clients[slow["participant_index"]]
                delayed.expected_slow_close = True
                delayed.pause_until = time.monotonic() + slow["pause_seconds"]
                await asyncio.sleep(slow["pause_seconds"])
                # Reconnect is an explicit scenario action, not mutation retry.
                await self.reconnect([delayed.index])
                delayed.expected_slow_close = False
                row["boundary"] = "ACTUAL_CLIENT_SOCKET_READ_PAUSE_NOT_NETWORK_IMPAIRMENT"
            elif fault == "room_switch":
                check(len(self.config["rooms"]) >= 2, "switch-target-required")
                target = 1 if client.room_index == 0 else 0
                occupants = sum(c.room_index == target for c in self.clients)
                row["target_full"] = occupants >= self.config["rooms"][target]["capacity"]
                row["seconds"] = await self.switch(client, target, row["target_full"])
            elif fault == "world_restart":
                if restart_hook is None:
                    row["status"] = "NOT_RUN_NO_PRODUCER_HOOK"
                else:
                    epochs = [c.state.epoch for c in self.clients]
                    for c in self.clients:
                        c.expected_restart = True
                    async with asyncio.timeout(self.config["schedule"]["operation_timeout_seconds"]):
                        await restart_hook()
                    await self.wait(lambda: all(c.peer_disconnected for c in self.clients), "restart-peer-disconnect")
                    row["observed_peer_disconnects"] = sum(c.peer_disconnected for c in self.clients)
                    await self.parallel(self.clients, lambda c: c.stop())
                    await self.parallel(self.clients, lambda c: c.connect())
                    check(all(c.state.epoch != old for c, old in zip(self.clients, epochs)), "restart-epoch-not-changed")
                    await self.converge()
                    for c in self.clients:
                        c.expected_restart = False
                    row["boundary"] = "EXTERNAL_PRODUCER_HOOK_PLUS_CHANGED_PUBLIC_ROOM_EPOCH"
                    row["process_lifecycle_oracle"] = "PRODUCER_OWNED_NOT_INDEPENDENTLY_OBSERVED"
            row.setdefault("seconds", elapsed(start))
            report["steps"].append(row)


async def run(config, *, capacity=True, network=None, contracts=None, restart_hook=None, observation_hook=None):
    report = new_report("QA08-public-200-four-room" if capacity else "QA05-public-world-failures", network is not None)
    start, cpu_start = time.monotonic(), time.process_time()
    runner = None
    try:
        runner = Runner(config, capacity, network, contracts)
        report["requested_schedule"] = config["schedule"].copy()
        # Schedule indices are local ordinal positions, never actor IDs.
        resources = {key: config["bootstrap"][key] for key in ("map_manifest", "avatar_manifest", "theme_manifests")}
        report["expected_public_resource_bundle_sha256"] = hashlib.sha256(json.dumps(resources, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()).hexdigest()
        async with asyncio.timeout(config["schedule"]["total_timeout_seconds"]):
            await runner.prepare()
            report["canonical_principals_verified"] = len(runner.by_actor)
            report["registered_rooms_verified"] = len(config["rooms"])
            report["initial_admitted_sockets"] = runner.active
            if capacity:
                seconds = await runner.switch(runner.clients[0], 1, True)
                report["steps"].append({"kind": "full-room-switch-keeps-old-membership", "seconds": seconds, "status": "OBSERVED"})
            else:
                await runner.failures(report, restart_hook)
            await runner.measure(report)
            runner.check_errors()
            report["result"] = "COMPLETED"
    except Failure as error:
        report["failure"] = error.label
    except (TimeoutError, asyncio.CancelledError):
        report["failure"] = "whole-or-operation-deadline"
    except BaseException:
        report["failure"] = "transport-or-input-unavailable"
    finally:
        if runner:
            # Cancellation/deadline does not abandon another client's socket.
            outcomes = await asyncio.gather(*(c.stop() for c in runner.clients), return_exceptions=True)
            report["cleanup"] = {"status": "CLIENT_HANDLES_CLOSED" if not any(isinstance(x, BaseException) for x in outcomes) else "FAILED",
                                 "opened_connections": runner.opened, "close_acknowledged": runner.close_acks,
                                 "database_memberships": "NOT_RUN_WITHOUT_PRODUCER_ORACLE"}
            report["peak_active_sockets"] = runner.peak
            report["http_seconds"] = runner.http_seconds.public()
            report["admission_seconds"] = runner.connect_seconds.public()
            report["client_event_loop_lateness_seconds"] = runner.loop_delay.public()
            report["input_backpressure_count"] = runner.input_backpressure
            report["observed_client_read_pause_seconds"] = runner.read_pauses.public()
            report["slow_backpressure_controls"] = runner.slow_backpressure_controls
            report["transport_counters"] = {key: (max if key.startswith("max_") else sum)(r[key] for r in runner.counters)
                                             for key in runner.counters[0]} if runner.counters else {}
            report["last_assigned_room_counts_before_cleanup"] = [sum(c.room_index == i for c in runner.clients) for i in range(len(config["rooms"]))]
            report["latest_connection_ack_latency_seconds"] = [c.state.ack_seconds.public() for c in runner.clients if c.state]
        if observation_hook:
            try:
                async with asyncio.timeout(5):
                    report["server_metrics_and_cleanup_oracle"] = observations(await observation_hook())
            except BaseException:
                report["server_metrics_and_cleanup_oracle"] = {"status": "UNAVAILABLE"}
        report["elapsed_seconds"] = elapsed(start)
        report["client_process_cpu_seconds"] = max(0, time.process_time() - cpu_start)
        if report["cleanup"]["status"] == "FAILED":
            report["result"] = "FAIL"
    return report
