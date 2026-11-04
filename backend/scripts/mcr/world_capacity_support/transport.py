"""Bounded asynchronous HTTP/RFC6455 transport, using existing frozen validators.

No redirects, DNS, proxies, cookies, credential persistence or server lifecycle calls.
"""
from __future__ import annotations

import asyncio
import base64
from datetime import datetime, timezone
import hashlib
import importlib.util
import ipaddress
import json
import os
import re
import ssl
import struct
import time
from urllib.parse import urlsplit

from .report import ROOT, Failure, check, json_value

MAX_HEADER, MAX_HTTP, MAX_FRAME, MAX_SEND = 8192, 2 * 1024 * 1024, 65536, 4096


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


class Contracts:
    def __init__(self):
        self.wire = module("mcr_capacity_c05", ROOT / "scripts/mcr/validate-realtime.py")
        self.core = module("mcr_capacity_c03", ROOT / "scripts/mcr/validate-core.py")
        self.c05 = self.wire.Contract()
        self.core_document = json_value((ROOT / "backend/docs/api/mcr-core.openapi.json").read_bytes())

    def validate(self, value, name):
        try:
            self.c05.validate(value, name)
        except Exception:
            raise Failure("c05-schema") from None

    def bootstrap(self, value, event):
        try:
            self.core.validate(value, "Bootstrap", self.core_document, event_id=event)
        except Exception:
            raise Failure("c03-bootstrap-schema") from None
        check(value["event"]["event_id"] == event and value["participant"]["event_id"] == event
              and value["capabilities"]["world"]["available"] is True, "bootstrap-eligibility")
        actor = value["participant"]["user_id"]
        check(value["participant"]["profile"]["user_id"] == actor, "bootstrap-profile-identity")
        return actor


def endpoint(value, websocket=False):
    check(isinstance(value, str) and len(value) <= 2048 and not any(ord(c) < 33 for c in value), "endpoint")
    try:
        u = urlsplit(value)
        check(u.scheme in (("ws", "wss") if websocket else ("http", "https"))
              and not u.username and not u.password and not u.query and not u.fragment, "endpoint")
        ipaddress.ip_address(u.hostname)
        port = u.port or (443 if u.scheme in ("https", "wss") else 80)
        check(1 <= port <= 65535 and u.path in (("/world-ws",) if websocket else ("", "/")), "endpoint-path")
        return u, port
    except (ValueError, TypeError, AttributeError):
        raise Failure("endpoint") from None


def browser_origin(value):
    check(isinstance(value, str) and len(value) <= 2048 and value.isascii()
          and not any(ord(c) < 33 or ord(c) == 127 for c in value), "browser-origin")
    u = urlsplit(value)
    try:
        check(u.scheme in ("http", "https") and u.hostname and not u.username and not u.password
              and u.path == "" and not u.query and not u.fragment and (u.port is None or u.port > 0), "browser-origin")
    except ValueError:
        raise Failure("browser-origin") from None
    return value


def host(u, port):
    return (f"[{u.hostname}]" if ":" in u.hostname else u.hostname) + f":{port}"


async def connection(target):
    u, port = target
    tls = ssl.create_default_context() if u.scheme in ("https", "wss") else None
    return await asyncio.open_connection(u.hostname, port, ssl=tls,
                                         server_hostname=u.hostname if tls else None, limit=MAX_FRAME)


async def close_writer(writer):
    writer.close()
    try:
        await asyncio.wait_for(writer.wait_closed(), .5)
    except (TimeoutError, OSError):
        writer.transport.abort()
    except asyncio.CancelledError:
        writer.transport.abort()
        raise


async def headers(reader):
    try:
        raw = await reader.readuntil(b"\r\n\r\n")
        check(len(raw) <= MAX_HEADER, "http-header-limit")
        lines = raw[:-4].decode("ascii").split("\r\n")
        match = re.fullmatch(r"HTTP/1\.[01] ([0-9]{3})(?: [\x20-\x7e]*)?", lines[0])
        check(match is not None, "http-status-line")
        result = {}
        for line in lines[1:]:
            check(":" in line, "http-header")
            key, val = line.split(":", 1)
            check(re.fullmatch(r"[A-Za-z0-9!#$%&'*+.^_`|~-]+", key) and not any(ord(c) < 32 and c != "\t" for c in val), "http-header")
            key = key.lower()
            # Vary is a list field: intermediaries may append separate lines.
            # Keep framing, authorization and cache-policy duplicates strict.
            if key == "vary" and key in result:
                result[key] += ", " + val.strip()
                continue
            # Only fixed public protocol names may enter sanitized diagnostics.
            duplicate_label = ("duplicate-http-header-" + key if key in
                ("date", "server", "cache-control", "content-type", "content-length",
                 "transfer-encoding", "connection", "vary", "access-control-allow-origin")
                else "duplicate-http-header")
            check(key not in result, duplicate_label)
            result[key] = val.strip()
        return int(match[1]), result
    except (UnicodeError, asyncio.LimitOverrunError, asyncio.IncompleteReadError):
        raise Failure("http-header") from None


async def body(reader, values):
    check(not ("transfer-encoding" in values and "content-length" in values), "ambiguous-http-framing")
    if "transfer-encoding" in values:
        check(values["transfer-encoding"].lower() == "chunked", "http-transfer-encoding")
        output = bytearray()
        for _ in range(8192):
            line = await reader.readuntil(b"\r\n")
            check(len(line) <= 128 and re.fullmatch(rb"[0-9A-Fa-f]+\r\n", line), "http-chunk-size")
            size = int(line[:-2], 16)
            check(size <= MAX_HTTP - len(output), "http-body-limit")
            if size == 0:
                check(await reader.readexactly(2) == b"\r\n", "http-trailers")
                return bytes(output)
            output.extend(await reader.readexactly(size))
            check(await reader.readexactly(2) == b"\r\n", "http-chunk-end")
        raise Failure("http-chunk-count")
    if "content-length" in values:
        check(re.fullmatch(r"[0-9]{1,9}", values["content-length"]) is not None, "http-length")
        size = int(values["content-length"])
        check(size <= MAX_HTTP, "http-body-limit")
        return await reader.readexactly(size)
    output = bytearray()
    while True:
        part = await reader.read(min(65536, MAX_HTTP + 1 - len(output)))
        if not part:
            return bytes(output)
        output.extend(part)
        check(len(output) <= MAX_HTTP, "http-body-limit")


async def http_json(target, origin, path, token, payload=None):
    check(re.fullmatch(r"/api/concert/events/[0-9a-f-]{36}/(?:bootstrap|rooms|world-tickets)", path) is not None, "http-target")
    check(isinstance(token, str) and re.fullmatch(r"[A-Za-z0-9._~-]{1,8192}", token) is not None, "bearer-shape")
    raw = b"" if payload is None else json.dumps(payload, separators=(",", ":"), allow_nan=False).encode()
    check(len(raw) <= MAX_SEND, "http-request-limit")
    reader, writer = await connection(target)
    try:
        u, port = target
        request = [f"{'GET' if payload is None else 'POST'} {path} HTTP/1.1", f"Host: {host(u, port)}",
                   f"Origin: {origin}", f"Authorization: Bearer {token}", "X-MCR-Intent: browser",
                   "Content-Type: application/json", f"Content-Length: {len(raw)}", "Connection: close", "", ""]
        writer.write("\r\n".join(request).encode("ascii") + raw)
        await writer.drain()
        status, values = await headers(reader)
        check(values.get("cache-control") == "no-store" and "location" not in values, "http-response-policy")
        check(values.get("content-type", "").split(";", 1)[0].strip().lower() == "application/json", "http-content-type")
        return status, json_value(await body(reader, values))
    finally:
        await close_writer(writer)


class Closed(Exception):
    def __init__(self, code):
        self.code = code
        super().__init__("websocket-closed")


class WebSocket:
    def __init__(self, reader, writer):
        self.reader, self.writer = reader, writer
        self.lock = asyncio.Lock()
        self.closed, self.close_sent, self.close_ack = False, False, False
        self.frames_in = self.bytes_in = self.frames_out = self.bytes_out = 0
        self.max_message_bytes = self.max_buffered_bytes = self.max_write_buffer_bytes = 0

    @classmethod
    async def open(cls, target, origin):
        reader, writer = await connection(target)
        try:
            u, port = target
            nonce = base64.b64encode(os.urandom(16)).decode()
            writer.write((f"GET /world-ws HTTP/1.1\r\nHost: {host(u, port)}\r\nOrigin: {origin}\r\n"
                          "Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\n"
                          f"Sec-WebSocket-Key: {nonce}\r\n\r\n").encode("ascii"))
            await writer.drain()
            status, values = await headers(reader)
            accept = base64.b64encode(hashlib.sha1((nonce + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            check(status == 101 and values.get("upgrade", "").lower() == "websocket"
                  and "upgrade" in [s.strip().lower() for s in values.get("connection", "").split(",")]
                  and values.get("sec-websocket-accept") == accept
                  and "sec-websocket-extensions" not in values and "sec-websocket-protocol" not in values, "websocket-upgrade")
            return cls(reader, writer)
        except BaseException:
            await close_writer(writer)
            raise

    async def frame(self, opcode, payload):
        check(len(payload) <= (125 if opcode >= 8 else MAX_SEND), "outgoing-frame-limit")
        async with self.lock:
            check(not self.closed, "write-after-close")
            if opcode == 8:
                if self.close_sent:
                    return
                self.close_sent = True
            elif opcode < 8:
                check(not self.close_sent, "data-after-close")
            mask = os.urandom(4)
            header = bytes([0x80 | opcode, 0x80 | len(payload)]) if len(payload) < 126 else bytes([0x80 | opcode, 0xfe]) + struct.pack("!H", len(payload))
            self.writer.write(header + mask + bytes(v ^ mask[i % 4] for i, v in enumerate(payload)))
            self.max_write_buffer_bytes = max(self.max_write_buffer_bytes, self.writer.transport.get_write_buffer_size())
            await self.writer.drain()
            self.frames_out += 1
            self.bytes_out += len(payload)

    async def send(self, value):
        await self.frame(1, json.dumps(value, separators=(",", ":"), allow_nan=False).encode())

    async def receive(self):
        fragments, size, started, controls = [], 0, False, 0
        while True:
            self.max_buffered_bytes = max(self.max_buffered_bytes, len(self.reader._buffer))
            raw = await self.reader.readexactly(2)
            final, opcode, masked, length = bool(raw[0] & 128), raw[0] & 15, bool(raw[1] & 128), raw[1] & 127
            check(not (raw[0] & 0x70) and not masked and opcode in (0, 1, 8, 9, 10), "incoming-frame-shape")
            if length == 126:
                length = struct.unpack("!H", await self.reader.readexactly(2))[0]
                check(length >= 126, "nonminimal-frame-length")
            elif length == 127:
                length = struct.unpack("!Q", await self.reader.readexactly(8))[0]
                check(length >= 65536 and length < 2**63, "nonminimal-frame-length")
            check(length <= MAX_FRAME and (opcode < 8 or final and length <= 125), "incoming-frame-limit")
            check(opcode >= 8 or size + length <= MAX_FRAME, "fragment-limit")
            payload = await self.reader.readexactly(length)
            self.frames_in += 1
            self.bytes_in += length
            if opcode == 8:
                check(len(payload) != 1, "close-payload")
                code = struct.unpack("!H", payload[:2])[0] if payload else 1005
                check(not payload or 3000 <= code < 5000 or
                      1000 <= code <= 1014 and code not in (1004, 1005, 1006), "close-code")
                try:
                    payload[2:].decode("utf-8")
                except UnicodeError:
                    raise Failure("close-utf8") from None
                if not self.close_sent:
                    await self.frame(8, payload)
                self.close_ack, self.closed = True, True
                raise Closed(code)
            if opcode in (9, 10):
                controls += 1
                check(controls <= 128, "control-frame-budget")
                if opcode == 9:
                    await self.frame(10, payload)
                continue
            check((opcode == 1 and not started) or (opcode == 0 and started), "fragment-state")
            started = True
            fragments.append(payload)
            size += length
            check(len(fragments) <= 128, "fragment-count")
            if final:
                self.max_message_bytes = max(self.max_message_bytes, size)
                return json_value(b"".join(fragments))

    async def send_close(self):
        if not self.closed and not self.close_sent:
            await self.frame(8, struct.pack("!H", 1000))

    async def abort(self):
        self.closed = True
        await close_writer(self.writer)

    def counters(self):
        return {k: getattr(self, k) for k in ["frames_in", "bytes_in", "frames_out", "bytes_out",
                "max_message_bytes", "max_buffered_bytes", "max_write_buffer_bytes"]}


class Network:
    def __init__(self, config):
        self.target = endpoint(config["gateway_url"])
        self.ws_target = endpoint(config["ws_url"], True)
        self.origin = browser_origin(config["browser_origin"])

    async def http(self, path, token, payload=None):
        return await http_json(self.target, self.origin, path, token, payload)

    async def websocket(self):
        return await WebSocket.open(self.ws_target, self.origin)
