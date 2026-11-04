#!/usr/bin/env python3
"""Bounded public C03/C05 probe. Import run_probe; credentials remain in memory.

Pure fixtures qualify this helper, never a live MCR/IN01 journey. No database,
private owner endpoint, package installation or browser/session factory is used.
"""
from __future__ import annotations

import base64
import hashlib
import http.client
import io
import ipaddress
import json
import math
import os
import re
import select
import socket
import ssl
import struct
import sys
import time
from datetime import datetime, timezone
from urllib.parse import urlsplit
from uuid import UUID

PROTOCOL = "1.0.0"
MAX_HTTP = 2 * 1024 * 1024
MAX_FRAME = 65536
MAX_MESSAGES = 1600
# Source-specific BE12 qualification bounds, not a negotiated prediction profile.
MAX_PIXELS_PER_TICK = 8.0  # 160 px/s, fixed 50 ms step.
MAX_INTENT_HOLD_PIXELS = 40.0  # One intent expires after 250 ms.
PIN_KEYS = ("map_id", "map_revision", "avatar_manifest_id", "avatar_manifest_version")
WS_PIN_KEYS = ("mapId", "mapRevision", "avatarManifestId", "avatarManifestVersion")


class ProbeFailure(Exception):
    """Fixed labels only; never construct this with remote/credential text."""


def require(condition, label):
    if not condition:
        raise ProbeFailure(label)


def remaining(deadline):
    value = deadline - time.monotonic()
    require(value > 0, "deadline")
    return value


def uint(value, minimum=0):
    return type(value) is int and minimum <= value <= 9007199254740991


def uuid(value):
    try:
        return isinstance(value, str) and str(UUID(value)) == value and UUID(value).int != 0
    except (ValueError, TypeError, AttributeError):
        return False


def instant(value):
    require(isinstance(value, str) and value.endswith("Z"), "timestamp")
    try:
        return datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        raise ProbeFailure("timestamp") from None


def unique_json(raw):
    def pairs(items):
        obj = {}
        for key, value in items:
            require(key not in obj, "duplicate-json-key")
            obj[key] = value
        return obj
    try:
        if isinstance(raw, bytes):
            raw = raw.decode("utf-8")
        value = json.loads(raw, object_pairs_hook=pairs,
                           parse_constant=lambda _: (_ for _ in ()).throw(ProbeFailure("nonfinite-json")))
        require(isinstance(value, dict), "json-object")
        return value
    except (ValueError, UnicodeError, RecursionError):
        raise ProbeFailure("invalid-json") from None


def endpoint(value, websocket=False):
    require(isinstance(value, str) and len(value) <= 2048 and
            not any(ord(c) <= 32 or ord(c) >= 127 for c in value), "endpoint")
    try:
        u = urlsplit(value)
        require(u.scheme in (("ws", "wss") if websocket else ("http", "https")) and
                u.hostname and not u.username and not u.password and not u.query and
                not u.fragment and u.path in (("/world-ws",) if websocket else ("", "/")), "endpoint")
        # Root's owned stack exposes numeric loopback origins. Numeric targets
        # also avoid an unbounded external DNS resolver outside our deadline.
        ipaddress.ip_address(u.hostname)
        port = u.port or (443 if u.scheme in ("https", "wss") else 80)
        require(1 <= port <= 65535, "endpoint")
        return u, port
    except (ValueError, TypeError):
        raise ProbeFailure("endpoint") from None


def origin(value):
    require(isinstance(value, str) and len(value) <= 2048 and
            not any(ord(c) <= 32 or ord(c) >= 127 for c in value), "origin")
    u = urlsplit(value)
    require(u.scheme in ("http", "https") and u.hostname and not u.username and
            not u.password and not u.query and not u.fragment and not u.path, "origin")
    return value


def connect(u, port, deadline):
    sock = socket.create_connection((u.hostname, port), timeout=min(3, remaining(deadline)))
    try:
        if u.scheme in ("https", "wss"):
            sock.settimeout(min(3, remaining(deadline)))
            sock = ssl.create_default_context().wrap_socket(sock, server_hostname=u.hostname)
        return sock
    except BaseException:
        sock.close()
        raise


def host_header(u, port):
    name = "[" + u.hostname + "]" if ":" in u.hostname else u.hostname
    return name + ":" + str(port)


class DeadlineReader(io.RawIOBase):
    def __init__(self, sock, deadline):
        self.sock, self.deadline = sock, deadline

    def readable(self):
        return True

    def readinto(self, buffer):
        self.sock.settimeout(min(3, remaining(self.deadline)))
        return self.sock.recv_into(buffer)


class ResponseSocket:
    def __init__(self, sock, deadline):
        self.sock, self.deadline = sock, deadline

    def makefile(self, *_args, **_kwargs):
        return io.BufferedReader(DeadlineReader(self.sock, self.deadline))


def http_json(target, browser_origin, path, token, deadline, payload=None):
    u, port = target
    body = b"" if payload is None else json.dumps(payload, separators=(",", ":")).encode()
    require(len(body) <= 4096, "request-limit")
    headers = [f"{'GET' if payload is None else 'POST'} {path} HTTP/1.1",
               "Host: " + host_header(u, port), "Origin: " + browser_origin,
               "Authorization: Bearer " + token, "X-MCR-Intent: browser",
               "Content-Type: application/json", "Content-Length: " + str(len(body)),
               "Connection: close", "", ""]
    sock = connect(u, port, deadline)
    try:
        sock.settimeout(min(3, remaining(deadline)))
        sock.sendall("\r\n".join(headers).encode("ascii") + body)
        response = http.client.HTTPResponse(ResponseSocket(sock, deadline))
        try:
            response.begin()
            require(response.status == 200, "public-http-status")
            require(response.getheader("Location") is None, "http-redirect")
            require(response.getheader("Cache-Control") == "no-store", "http-no-store")
            raw = response.read(MAX_HTTP + 1)
            require(len(raw) <= MAX_HTTP, "http-body-limit")
            return unique_json(raw)
        finally:
            response.close()
    finally:
        sock.close()


class WebSocket:
    """RFC6455 subset: no extensions/subprotocols; bounded text and fragmentation."""
    def __init__(self, sock, deadline):
        self.sock, self.deadline = sock, deadline
        self.buffer = bytearray()
        self.closed = False

    @classmethod
    def open(cls, url, browser_origin, deadline):
        u, port = endpoint(url, True)
        sock = connect(u, port, deadline)
        ws = cls(sock, deadline)
        try:
            key = base64.b64encode(os.urandom(16)).decode("ascii")
            request = ("GET /world-ws HTTP/1.1\r\nHost: " + host_header(u, port) +
                       "\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13"
                       "\r\nSec-WebSocket-Key: " + key + "\r\nOrigin: " + browser_origin + "\r\n\r\n")
            sock.settimeout(min(3, remaining(deadline)))
            sock.sendall(request.encode("ascii"))
            raw = bytearray()
            while b"\r\n\r\n" not in raw:
                require(len(raw) < 8192, "ws-header-limit")
                sock.settimeout(min(3, remaining(deadline)))
                part = sock.recv(min(1024, 8192 - len(raw)))
                require(bool(part), "ws-upgrade-eof")
                raw.extend(part)
            head, tail = bytes(raw).split(b"\r\n\r\n", 1)
            lines = head.decode("ascii").split("\r\n")
            require(lines[0].split(" ", 2)[:2] == ["HTTP/1.1", "101"], "ws-upgrade-status")
            headers = {}
            for line in lines[1:]:
                name, sep, value = line.partition(":")
                require(bool(sep) and name and name.lower() not in headers, "ws-upgrade-headers")
                headers[name.lower()] = value.strip()
            expected = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            require(headers.get("upgrade", "").lower() == "websocket" and
                    "upgrade" in [x.strip().lower() for x in headers.get("connection", "").split(",")] and
                    headers.get("sec-websocket-accept") == expected and
                    "sec-websocket-extensions" not in headers and "sec-websocket-protocol" not in headers,
                    "ws-upgrade-binding")
            ws.buffer.extend(tail)
            return ws
        except BaseException:
            sock.close()
            raise

    def exact(self, count):
        while len(self.buffer) < count:
            self.sock.settimeout(min(3, remaining(self.deadline)))
            part = self.sock.recv(min(8192, count - len(self.buffer)))
            require(bool(part), "ws-eof")
            self.buffer.extend(part)
        data = bytes(self.buffer[:count])
        del self.buffer[:count]
        return data

    def send_frame(self, opcode, payload):
        require(not self.closed and len(payload) <= (125 if opcode >= 8 else 4096), "ws-send-limit")
        mask = os.urandom(4)
        length = bytes([len(payload)]) if len(payload) < 126 else b"\x7e" + struct.pack("!H", len(payload))
        frame = bytes([0x80 | opcode, 0x80 | length[0]]) + length[1:] + mask
        frame += bytes(value ^ mask[i % 4] for i, value in enumerate(payload))
        self.sock.settimeout(min(3, remaining(self.deadline)))
        self.sock.sendall(frame)

    def send(self, value):
        self.send_frame(1, json.dumps(value, allow_nan=False, separators=(",", ":")).encode())

    def frame(self):
        a, b = self.exact(2)
        final, opcode, size = bool(a & 128), a & 15, b & 127
        require(not (a & 112) and not (b & 128) and opcode in (0, 1, 8, 9, 10), "ws-frame-flags")
        if size == 126:
            size = struct.unpack("!H", self.exact(2))[0]
            require(size >= 126, "ws-noncanonical-length")
        elif size == 127:
            size = struct.unpack("!Q", self.exact(8))[0]
            require(size >= 65536 and size < 2**63, "ws-noncanonical-length")
        require(size <= MAX_FRAME and (opcode < 8 or (final and size <= 125)), "ws-frame-limit")
        return final, opcode, self.exact(size)

    def receive(self):
        text = bytearray()
        started = False
        for _ in range(128):
            final, opcode, data = self.frame()
            if opcode in (9, 10):
                if opcode == 9:
                    self.send_frame(10, data)
                continue
            require(opcode != 8, "ws-server-close")
            require((opcode == 1 and not started) or (opcode == 0 and started), "ws-fragment-order")
            started = True
            text.extend(data)
            require(len(text) <= MAX_FRAME, "ws-message-limit")
            if final:
                return unique_json(bytes(text))
        raise ProbeFailure("ws-fragment-limit")

    def close(self):
        if self.closed:
            return True
        self.deadline = min(self.deadline, time.monotonic() + 1)
        acknowledged = False
        try:
            self.send_frame(8, struct.pack("!H", 1000))
            for _ in range(64):
                final, opcode, data = self.frame()
                if opcode == 8:
                    require(len(data) != 1, "ws-close-shape")
                    if data:
                        code = struct.unpack("!H", data[:2])[0]
                        require(code == 1000, "ws-close-code")
                        data[2:].decode("utf-8")
                    acknowledged = True
                    break
                if opcode == 9:
                    self.send_frame(10, data)
        except (OSError, ProbeFailure, UnicodeError):
            pass
        finally:
            self.closed = True
            self.sock.close()
        return acknowledged


def pins(document):
    m, a = document["map_manifest"], document["avatar_manifest"]
    require(uint(m["map_revision"], 1) and uint(a["manifest_version"], 1), "release-pin-types")
    result = (m["map_id"], m["map_revision"], a["manifest_id"], a["manifest_version"])
    require(result == ("mcr_reference_scene_r3", 4, "mcr_selected_static_20261103", 1), "selected-release-pins")
    return result


def checked_bootstrap(document, expected, event):
    require(document["event"]["event_id"] == event and document["participant"]["event_id"] == event and
            document["capabilities"]["world"]["available"] is True, "bootstrap-world-binding")
    require(pins(document) == pins(expected) and document["map_manifest"] == expected["map_manifest"] and
            document["avatar_manifest"] == expected["avatar_manifest"], "bootstrap-release-binding")
    actor = document["participant"]["user_id"]
    require(uuid(actor) and document["participant"]["profile"]["user_id"] == actor, "bootstrap-identity")
    return actor


def authenticate(ticket, expected, event, room):
    fields = {"protocol_version", "ticket", "event_id", "room_id", *PIN_KEYS,
              "server_time", "expires_at", "ws_path", "auth_timeout_ms"}
    require(set(ticket) == fields and ticket["protocol_version"] == PROTOCOL and
            ticket["event_id"] == event and ticket["room_id"] == room and
            uint(ticket["map_revision"], 1) and uint(ticket["avatar_manifest_version"], 1) and
            tuple(ticket[k] for k in PIN_KEYS) == pins(expected) and
            ticket["ws_path"] == "/world-ws" and ticket["auth_timeout_ms"] == 3000 and
            isinstance(ticket["ticket"], str) and re.fullmatch(r"[A-Za-z0-9_-]{43}", ticket["ticket"]), "ticket-binding")
    lifetime = (instant(ticket["expires_at"]) - instant(ticket["server_time"])).total_seconds()
    require(0 < lifetime <= 30 and instant(ticket["expires_at"]) > datetime.now(timezone.utc), "ticket-expiry")
    return {"protocolVersion": PROTOCOL, "type": "authenticate", "ticket": ticket["ticket"],
            "eventId": event, "roomId": room, **dict(zip(WS_PIN_KEYS, pins(expected)))}


class View:
    def __init__(self, actor, expected, event, room):
        self.actor, self.expected, self.event, self.room = actor, expected, event, room
        self.stream = self.epoch = None
        self.seq, self.tick, self.ack, self.sent_seq, self.frames = 0, 0, 0, 0, 0
        self.players = {}
        self.geometry = expected["map_manifest"]["geometry"]
        g = self.geometry
        w, h = g["width_cells"], g["height_cells"]
        require(all(type(n) is int and 1 <= n <= 1048576 for n in
                    (w, h, g["tile_width_px"], g["tile_height_px"])) and w * h <= 1048576 and
                isinstance(g["collision_grid"], list) and len(g["collision_grid"]) == h and
                all(isinstance(row, list) and len(row) == w and
                    all(type(cell) is int and cell in (0, 1) for cell in row) for row in g["collision_grid"]), "geometry-shape")

    def walkable(self, x, y):
        g = self.geometry
        w, h = g["width_cells"], g["height_cells"]
        tw, th = g["tile_width_px"], g["tile_height_px"]
        require(all(type(n) is int and n > 0 for n in (w, h, tw, th)) and w * h <= 1048576, "geometry-shape")
        return 0 <= x < w * tw and 0 <= y < h * th and g["collision_grid"][int(y // th)][int(x // tw)] == 0

    def player(self, value):
        require(isinstance(value, dict) and set(value) == {"userId", "displayName", "position", "facing", "moving", "avatar"}, "player-shape")
        require(uuid(value["userId"]) and isinstance(value["displayName"], str) and 1 <= len(value["displayName"]) <= 40 and
                value["facing"] in ("up", "down", "left", "right") and type(value["moving"]) is bool, "player-fields")
        p = value["position"]
        require(isinstance(p, dict) and set(p) == {"x", "y"} and
                all(type(p[k]) in (int, float) and math.isfinite(p[k]) for k in p), "position")
        require(self.walkable(p["x"], p["y"]), "position-collision")
        avatar = value["avatar"]
        require(isinstance(avatar, dict) and set(avatar) == {"revision", "config"} and uint(avatar["revision"], 1), "avatar-shape")
        c = avatar["config"]
        require(isinstance(c, dict) and set(c) == {"contract_version", "kind", "manifest_id", "manifest_version", "representation", "preset_id"} and
                c["contract_version"] == "1.1.0" and c["kind"] == "avatar_config" and c["representation"] == "whole_character" and
                uint(c["manifest_version"], 1) and
                (c["manifest_id"], c["manifest_version"]) == pins(self.expected)[2:] and
                c["preset_id"] in {f"photo01_front_{n:02}" for n in range(1, 7)}, "avatar-release")
        return value["userId"]

    def accept(self, value, ws):
        require(value.get("protocolVersion") == PROTOCOL, "c05-version")
        kind = value.get("type")
        if kind == "ping":
            require(set(value) == {"protocolVersion", "type", "heartbeatId"} and uuid(value["heartbeatId"]), "c05-ping")
            ws.send({"protocolVersion": PROTOCOL, "type": "pong", "heartbeatId": value["heartbeatId"]})
            return
        require(kind in ("snapshot", "delta"), "c05-control-or-unknown")
        self.frames += 1
        require(self.frames <= MAX_MESSAGES and uint(value["streamSeq"]) and uint(value["serverTick"]) and
                uint(value["ackInputSeq"]) and uuid(value["streamId"]) and uuid(value["roomEpoch"]), "c05-frame-fields")
        old_tick = self.tick
        if kind == "snapshot":
            require(set(value) == {"protocolVersion", "type", "streamId", "streamSeq", "roomEpoch", "serverTick", "eventId", "roomId", *WS_PIN_KEYS,
                                   "cause", "requestId", "selfUserId", "ackInputSeq", "accessExpiresAt", "players"} and
                    self.stream is None and value["streamSeq"] == 0 and value["cause"] == "INITIAL" and
                    value["requestId"] is None and value["selfUserId"] == self.actor and value["eventId"] == self.event and
                    value["roomId"] == self.room and uint(value["mapRevision"], 1) and uint(value["avatarManifestVersion"], 1) and
                    tuple(value[k] for k in WS_PIN_KEYS) == pins(self.expected), "snapshot-binding")
            require(instant(value["accessExpiresAt"]) > datetime.now(timezone.utc), "snapshot-expiry")
            self.stream, self.epoch = value["streamId"], value["roomEpoch"]
            changed, removed = value["players"], []
            require(isinstance(changed, list) and 1 <= len(changed) <= 50, "snapshot-players")
        else:
            require(set(value) == {"protocolVersion", "type", "streamId", "streamSeq", "roomEpoch", "serverTick", "ackInputSeq", "upserts", "removes"} and
                    self.stream is not None and value["streamId"] == self.stream and value["roomEpoch"] == self.epoch and
                    value["streamSeq"] == self.seq + 1, "delta-sequence-binding")
            changed, removed = value["upserts"], value["removes"]
            require(isinstance(changed, list) and isinstance(removed, list) and len(changed) <= 50 and len(removed) <= 50 and
                    all(uuid(x) for x in removed) and len(set(removed)) == len(removed), "delta-players")
        require(value["serverTick"] >= self.tick and self.ack <= value["ackInputSeq"] <= self.sent_seq, "tick-or-ack")
        seen = set()
        for player in changed:
            actor = self.player(player)
            require(actor not in seen and actor not in removed, "duplicate-player")
            seen.add(actor)
            previous = self.players.get(actor)
            if previous:
                p, q = previous["position"], player["position"]
                require(math.hypot(q["x"] - p["x"], q["y"] - p["y"]) <= MAX_PIXELS_PER_TICK * (value["serverTick"] - old_tick) + 1e-6,
                        "server-speed-bound")
            self.players[actor] = player
        for actor in removed:
            require(actor in self.players and actor != self.actor, "unexpected-removal")
            del self.players[actor]
        require(self.actor in self.players and len(self.players) <= 50, "self-membership")
        self.seq, self.tick, self.ack = value["streamSeq"], value["serverTick"], value["ackInputSeq"]

    def intent(self, x, y):
        require(self.stream is not None and (x, y) in ((0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)), "input-state")
        self.sent_seq += 1
        return {"protocolVersion": PROTOCOL, "type": "input", "streamId": self.stream,
                "roomEpoch": self.epoch, "seq": self.sent_seq, "axisX": x, "axisY": y, "buttons": []}


def pump(sockets, views, predicate, deadline):
    for _ in range(MAX_MESSAGES):
        if predicate():
            return
        remaining(deadline)
        active = [i for i, ws in enumerate(sockets) if not ws.closed]
        buffered = [i for i in active if sockets[i].buffer or
                    (isinstance(sockets[i].sock, ssl.SSLSocket) and sockets[i].sock.pending())]
        ready, _, _ = select.select([sockets[i].sock for i in active], [], [], 0 if buffered else min(.1, remaining(deadline)))
        for i in active:
            if i in buffered or sockets[i].sock in ready:
                views[i].accept(sockets[i].receive(), sockets[i])
    raise ProbeFailure("message-budget")


def run_probe(gateway_url, browser_origin, event_id, room_id, bearer_tokens, ws_url, bootstrap, *, timeout=20.0):
    """Return sanitized evidence. Caller supplies real public Auth tokens and C03.

    Numeric network targets avoid DNS deadline ambiguity. Browser Origin may use
    its configured hostname. No secret is placed in URLs, CLI arguments or output.
    """
    report = {"result": "FAIL", "scope": "public-two-principal-world", "phase": "inputs",
              "credential_values_recorded": False, "cleanup": {"opened": 0, "close_acknowledged": 0}}
    sockets, views = [], []
    try:
        require(type(timeout) in (int, float) and math.isfinite(timeout) and 1 <= timeout <= 45, "timeout-range")
        deadline = time.monotonic() + timeout
        target = endpoint(gateway_url)
        endpoint(ws_url, True)
        browser_origin = origin(browser_origin)
        require(uuid(event_id) and uuid(room_id), "event-room")
        require(isinstance(bearer_tokens, (list, tuple)) and len(bearer_tokens) == 2 and
                all(isinstance(t, str) and 1 <= len(t) <= 8192 and re.fullmatch(r"[A-Za-z0-9._~-]+", t) for t in bearer_tokens) and
                bearer_tokens[0] != bearer_tokens[1], "two-bearers")
        pins(bootstrap)
        base_path = "/api/concert/events/" + event_id
        report["phase"] = "public-bootstrap"
        documents = [http_json(target, browser_origin, base_path + "/bootstrap", t, deadline) for t in bearer_tokens]
        actors = [checked_bootstrap(d, bootstrap, event_id) for d in documents]
        require(actors[0] != actors[1], "distinct-public-identities")
        report["phase"] = "ticket-authenticate"
        for i, token in enumerate(bearer_tokens):
            request = {"room_id": room_id, **dict(zip(PIN_KEYS, pins(bootstrap)))}
            ticket = http_json(target, browser_origin, base_path + "/world-tickets", token, deadline, request)
            message = authenticate(ticket, bootstrap, event_id, room_id)
            ws = WebSocket.open(ws_url, browser_origin, deadline)
            sockets.append(ws)
            report["cleanup"]["opened"] += 1
            views.append(View(actors[i], bootstrap, event_id, room_id))
            ws.send(message)
            pump(sockets, views, lambda: views[i].stream is not None, deadline)
        report["phase"] = "mutual-membership"
        pump(sockets, views, lambda: all(set(actors) <= set(v.players) for v in views), deadline)
        require(views[0].stream != views[1].stream and views[0].epoch == views[1].epoch, "distinct-streams-shared-room")
        report["actor_count"] = 2
        report["mutual_membership"] = "PASS"
        start = dict(views[1].players[actors[0]]["position"])
        idle_start = dict(views[1].players[actors[1]]["position"])
        require(start == views[0].players[actors[0]]["position"], "initial-position-agreement")
        axis = next(((x, y) for x, y in ((1, 0), (-1, 0), (0, 1), (0, -1))
                     if all(views[0].walkable(start["x"] + x * n, start["y"] + y * n) for n in range(41))), None)
        require(axis is not None, "walkable-movement-segment")
        report["phase"] = "authoritative-movement"
        sockets[0].send(views[0].intent(*axis))
        def moved():
            point = views[1].players[actors[0]]["position"]
            return views[0].ack >= 1 and math.hypot(point["x"] - start["x"], point["y"] - start["y"]) > .1
        pump(sockets, views, moved, deadline)
        sockets[0].send(views[0].intent(0, 0))
        pump(sockets, views, lambda: views[0].ack >= 2 and not views[0].players[actors[0]]["moving"] and
             not views[1].players[actors[0]]["moving"] and
             views[0].players[actors[0]]["position"] == views[1].players[actors[0]]["position"], deadline)
        end = views[1].players[actors[0]]["position"]
        dx, dy = end["x"] - start["x"], end["y"] - start["y"]
        require(.1 < math.hypot(dx, dy) <= MAX_INTENT_HOLD_PIXELS + 1e-6 and
                dx * axis[0] + dy * axis[1] > 0 and abs(dx * axis[1] - dy * axis[0]) < 1e-6 and
                views[1].players[actors[1]]["position"] == idle_start, "movement-intent-binding")
        report["movement"] = "PASS"
        report["consumed_input_ack"] = views[0].ack
        report["source_bound_speed_check"] = "PASS; 8px/serverTick (BE12), not a negotiated prediction profile"
        report["phase"] = "disconnect-removal"
        require(sockets[0].close(), "actor-close-handshake")
        report["cleanup"]["close_acknowledged"] += 1
        pump(sockets, views, lambda: actors[0] not in views[1].players and actors[1] in views[1].players, deadline)
        report["peer_observed_removal"] = "PASS"
        report["stream_frames"] = [v.frames for v in views]
        report["result"], report["phase"] = "PASS", "complete"
    except ProbeFailure as error:
        report["failure"] = str(error)
    except Exception:
        # Never include exception repr, remote payload, endpoint, identity or secret.
        report["failure"] = "transport-or-shape"
    finally:
        for ws in sockets:
            if not ws.closed:
                if ws.close():
                    report["cleanup"]["close_acknowledged"] += 1
        if report["cleanup"]["close_acknowledged"] != report["cleanup"]["opened"]:
            report["result"] = "FAIL"
            report.setdefault("failure", "close-handshake")
    return report


def main():
    try:
        raw = sys.stdin.buffer.read(MAX_HTTP + 1)
        require(len(raw) <= MAX_HTTP, "stdin-limit")
        config = unique_json(raw)
        result = run_probe(**config)
    except Exception:
        result = {"result": "FAIL", "failure": "probe-input", "credential_values_recorded": False}
    print(json.dumps(result, allow_nan=False, sort_keys=True))
    return 0 if result["result"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
