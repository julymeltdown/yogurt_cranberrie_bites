"""Offline transport and source-driver regressions; never a 200-client load result."""
import asyncio
import copy
import io
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import AsyncMock, patch

import world_capacity_probe
from world_capacity_support import transport
from world_capacity_support.report import ROOT, Failure, Samples, json_value, read_private
from world_capacity_support.scenarios import StreamState, validate_config


def fixture(name):
    return json_value((ROOT / "contracts/fixtures" / name).read_bytes())


def configuration(capacity=False):
    bootstrap = fixture("core/bootstrap-open.json")
    bootstrap["capabilities"]["world"] = {"available": True, "reason": None}
    count = 200 if capacity else 2
    rooms = [{"room_id": f"00000000-0000-4000-8000-{n+11:012d}", "capacity": 50}
             for n in range(4 if capacity else 2)]
    return {"allocation_id": "offline-test-only", "gateway_url": "http://127.0.0.1:1",
            "ws_url": "ws://127.0.0.1:1/world-ws", "browser_origin": "http://localhost:5173",
            "event_id": bootstrap["event"]["event_id"], "bootstrap": bootstrap,
            "rooms": rooms, "principals": [{"bearer": f"fixture-token-{n}", "room_index": n//50 if capacity else 0} for n in range(count)],
            "schedule": {"arrival_concurrency": 2, "operation_timeout_seconds": 1,
                         "measurement_seconds": .08, "total_timeout_seconds": 5, "input_hz": 20}}


class Writer:
    def __init__(self):
        self.data = bytearray()
        self.closed = False
        self.aborted = False
        self.transport = self
    def write(self, value): self.data.extend(value)
    async def drain(self): pass
    def close(self): self.closed = True
    def abort(self): self.aborted = True
    async def wait_closed(self): pass
    def get_write_buffer_size(self): return len(self.data)


def reader(raw, eof=True):
    result = asyncio.StreamReader()
    result.feed_data(raw)
    if eof: result.feed_eof()
    return result


def frame(payload, opcode=1, final=True):
    return bytes([(128 if final else 0) | opcode, len(payload)]) + payload


class InputTests(unittest.TestCase):
    def test_original_scale_is_required(self):
        config = configuration(True)
        validate_config(config, True)
        config["principals"].pop()
        with self.assertRaisesRegex(Failure, "principal-count"): validate_config(config, True)

    def test_capacity_distribution_and_room_identity(self):
        config = configuration(True)
        config["principals"][0]["room_index"] = 1
        with self.assertRaises(Failure): validate_config(config, True)
        config = configuration(True)
        config["rooms"][1]["room_id"] = config["rooms"][0]["room_id"]
        with self.assertRaisesRegex(Failure, "duplicate-room"): validate_config(config, True)

    def test_no_header_injection_or_duplicate_credentials(self):
        for token in ("secret\r\nInjected: yes", "fixture-token-0"):
            config = configuration()
            config["principals"][1]["bearer"] = token
            with self.assertRaises(Failure): validate_config(config, False)

    def test_invalid_schedule_and_unknown_fields(self):
        for key, value in (("input_hz", float("nan")), ("arrival_concurrency", True),
                           ("reconnect", [{"after_seconds": 0, "participants": [{}]}])):
            config = configuration()
            config["schedule"][key] = value
            with self.assertRaises(Failure): validate_config(config, False)
        config = configuration()
        config["sql"] = "forbidden"
        with self.assertRaises(Failure): validate_config(config, False)

    def test_duplicate_nonfinite_and_non_utf8_json(self):
        for raw in (b'{"a":1,"a":2}', b'{"a":NaN}', '{}'.encode('utf-16'), b'\xff'):
            with self.assertRaises(Failure): json_value(raw)

    def test_private_file_mode_and_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input"
            path.write_bytes(b'{}')
            path.chmod(0o600)
            self.assertEqual(read_private(str(path)), {})
            path.chmod(0o644)
            with self.assertRaises(Failure): read_private(str(path))
            link = Path(directory) / "link"
            link.symlink_to(path)
            with self.assertRaises(OSError): read_private(str(link))

    def test_samples_bound_retention_and_describe_window(self):
        samples = Samples(2)
        for n in (1, 2, 30): samples.add(n)
        self.assertEqual(samples.public()["retained_samples"], 2)
        self.assertEqual(samples.public()["count"], 3)
        self.assertEqual(samples.public()["max"], 30)
        self.assertEqual(samples.public()["retained_p95"], 2)

    def test_no_execute_never_reads_credentials(self):
        with patch.object(world_capacity_probe, "read_private", side_effect=AssertionError), patch("sys.stdout", new_callable=io.StringIO) as output:
            self.assertEqual(world_capacity_probe.main([]), 2)
        self.assertEqual(json.loads(output.getvalue())["result"], "NOT_RUN")

    def test_cli_sanitizes_unexpected_errors(self):
        with patch.object(world_capacity_probe, "read_private", side_effect=RuntimeError("SECRET-ENDPOINT-TOKEN")), patch("sys.stdout", new_callable=io.StringIO) as output:
            self.assertEqual(world_capacity_probe.main(["--execute"]), 1)
        self.assertNotIn("SECRET", output.getvalue())


class TransportTests(unittest.IsolatedAsyncioTestCase):
    async def test_chunked_body_and_ambiguous_framing(self):
        self.assertEqual(await transport.body(reader(b'2\r\n{}\r\n0\r\n\r\n'), {"transfer-encoding": "chunked"}), b'{}')
        with self.assertRaisesRegex(Failure, "ambiguous-http-framing"):
            await transport.body(reader(b''), {"transfer-encoding": "chunked", "content-length": "2"})

    async def test_http_limits_and_duplicate_headers(self):
        for raw in (b'HTTP/1.1 200 OK\r\nX: one\r\nX: two\r\n\r\n', b'HTTP/1.1 200 OK\r\nX: '+b'x'*8192+b'\r\n\r\n'):
            with self.assertRaises(Failure): await transport.headers(reader(raw))
        with self.assertRaisesRegex(Failure, "http-body-limit"):
            await transport.body(reader(b''), {"content-length": str(transport.MAX_HTTP+1)})

    async def test_http_request_and_close_on_bad_policy(self):
        writer = Writer()
        with patch.object(transport, "connection", AsyncMock(return_value=(reader(b'HTTP/1.1 302 Found\r\nLocation: /other\r\n\r\n'), writer))):
            with self.assertRaisesRegex(Failure, "http-response-policy"):
                await transport.http_json(transport.endpoint("http://127.0.0.1:1"), "http://localhost", "/api/concert/events/00000000-0000-4000-8000-000000000010/bootstrap", "test-token")
        self.assertTrue(writer.closed)
        self.assertIn(b'Authorization: Bearer test-token\r\n', writer.data)

    async def test_fragmented_text_with_ping(self):
        writer = Writer()
        ws = transport.WebSocket(reader(frame(b'{"a":', final=False)+frame(b'x', 9)+frame(b'1}', 0)), writer)
        self.assertEqual(await ws.receive(), {"a": 1})
        self.assertEqual(writer.data[0], 0x8a)
        self.assertTrue(writer.data[1] & 128)

    async def test_invalid_wire_shapes_and_utf8(self):
        for raw in (b'\x81\x80', b'\x82\x00', b'\x81\x7e\x00\x01x', frame(b'\xff'), frame(b'{"a":1,"a":2}'), frame(b'{}', 0)):
            with self.assertRaises(Failure): await transport.WebSocket(reader(raw), Writer()).receive()

    async def test_close_ack_and_illegal_code(self):
        writer = Writer()
        ws = transport.WebSocket(reader(frame(struct.pack('!H', 4401), 8)), writer)
        with self.assertRaises(transport.Closed) as error: await ws.receive()
        self.assertEqual(error.exception.code, 4401)
        self.assertTrue(ws.closed and ws.close_ack)
        with self.assertRaises(Failure):
            await transport.WebSocket(reader(frame(struct.pack('!H', 1006), 8)), Writer()).receive()

    async def test_reserved_wire_close_code_is_not_empty_sentinel(self):
        writer = Writer()
        with self.assertRaisesRegex(Failure, "close-code"):
            await transport.WebSocket(reader(frame(struct.pack('!H', 1005), 8)), writer).receive()
        self.assertFalse(writer.data)
        ws = transport.WebSocket(reader(frame(b'', 8)), Writer())
        with self.assertRaises(transport.Closed) as error: await ws.receive()
        self.assertEqual(error.exception.code, 1005)

    async def test_graceful_shutdown_timeout_forces_transport_abort(self):
        writer = Writer()
        async def stalled(): await asyncio.sleep(10)
        writer.wait_closed = stalled
        await transport.close_writer(writer)
        self.assertTrue(writer.closed and writer.aborted)

    async def test_concurrent_close_is_single_and_prevents_data(self):
        writer = Writer()
        started, release = asyncio.Event(), asyncio.Event()
        async def stalled():
            started.set()
            await release.wait()
        writer.drain = stalled
        ws = transport.WebSocket(reader(b''), writer)
        task = asyncio.create_task(ws.send_close())
        await started.wait()
        other = asyncio.create_task(ws.send_close())
        release.set()
        await asyncio.gather(task, other)
        self.assertEqual(ws.frames_out, 1)
        with self.assertRaisesRegex(Failure, "data-after-close"): await ws.send({})

    async def test_receive_cancellation_does_not_hide_pending_task(self):
        ws = transport.WebSocket(reader(b'', False), Writer())
        with self.assertRaises(TimeoutError):
            async with asyncio.timeout(.01): await ws.receive()
        await ws.abort()
        self.assertTrue(ws.closed)


class StreamTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls): cls.contracts = transport.Contracts()

    def setUp(self):
        trace = fixture("realtime/resync.json")
        self.snapshot = trace["messages"][1]["message"]
        auth = trace["messages"][0]["message"]
        self.state = StreamState(trace["actor_id"], auth["eventId"], auth["roomId"], tuple(auth[k] for k in ("mapId", "mapRevision", "avatarManifestId", "avatarManifestVersion")), self.contracts)
        self.state.accept(self.snapshot, 0)

    def delta(self, sequence=1, ack=0):
        return {"protocolVersion": "1.0.0", "type": "delta", "streamId": self.state.stream,
                "roomEpoch": self.state.epoch, "streamSeq": sequence, "serverTick": sequence,
                "ackInputSeq": ack, "upserts": [], "removes": []}

    def test_actual_c05_delta_has_no_event_or_asset_fields(self):
        self.state.accept(self.delta(), 1)
        self.assertEqual(self.state.seq, 1)

    def test_input_ack_and_pending_bound(self):
        for n in range(64): self.state.input(n)
        with self.assertRaisesRegex(Failure, "pending-input-bound"): self.state.input(65)
        self.state.accept(self.delta(1, 64), 66)
        self.assertEqual(self.state.ack_seconds.count, 64)
        self.assertFalse(self.state.sent_at)

    def test_ack_overflow_and_stale_epoch_rejected(self):
        with self.assertRaises(Failure): self.state.accept(self.delta(1, 1), 0)
        value = self.delta()
        value["roomEpoch"] = "00000000-0000-4000-8000-000000000099"
        with self.assertRaisesRegex(Failure, "delta-binding"): self.state.accept(value, 0)

    def test_gap_withdrawal_and_correlated_baseline(self):
        self.state.drop_next_delta = True
        self.state.accept(self.delta(1), 1)
        reply = self.state.accept(self.delta(2), 2)
        self.contracts.validate(reply, "ClientMessage")
        self.assertFalse(self.state.players)
        reset = copy.deepcopy(self.snapshot)
        reset.update(cause="RESYNC", requestId=reply["requestId"], streamId="00000000-0000-4000-8000-000000000099", serverTick=3)
        self.state.accept(reset, 3)
        self.assertEqual(self.state.resets, 1)
        self.assertIn(self.state.actor, self.state.players)

    def test_unsolicited_gap_and_uncorrelated_reset_fail(self):
        with self.assertRaisesRegex(Failure, "unexpected-stream-gap"): self.state.accept(self.delta(2), 0)
        reset = copy.deepcopy(self.snapshot)
        reset["streamId"] = "00000000-0000-4000-8000-000000000099"
        with self.assertRaisesRegex(Failure, "reset-correlation"): self.state.accept(reset, 0)

    def test_core_bootstrap_uses_existing_full_validator(self):
        document = configuration()["bootstrap"]
        self.assertEqual(self.contracts.bootstrap(document, document["event"]["event_id"]), document["participant"]["user_id"])
        document["map_manifest"]["unexpected"] = True
        with self.assertRaisesRegex(Failure, "c03-bootstrap-schema"):
            self.contracts.bootstrap(document, document["event"]["event_id"])


if __name__ == "__main__": unittest.main()
