"""Stdlib synthetic HTTP/RFC6455 fixtures. These tests never certify live IN01."""
import base64
import copy
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import socket
import socketserver
import struct
import threading
import time
import unittest
from unittest.mock import patch
from uuid import uuid4

import world_two_principal_probe as probe

ROOT = Path(__file__).resolve().parents[3]
ACTORS = [str(uuid4()), str(uuid4())]
ROOM = str(uuid4())
EPOCH = str(uuid4())
TOKENS = ['fixture-private-bearer-A', 'fixture-private-bearer-B']
TICKETS = ['A' * 43, 'B' * 43]


def read_exact(sock, size):
    value = b''
    while len(value) < size:
        data = sock.recv(size - len(value))
        if not data:
            raise EOFError()
        value += data
    return value


def frame(data, opcode=1, final=True):
    size = len(data)
    head = bytes([(128 if final else 0) | opcode])
    if size < 126:
        return head + bytes([size]) + data
    if size < 65536:
        return head + b'\x7e' + struct.pack('!H', size) + data
    return head + b'\x7f' + struct.pack('!Q', size) + data


def client_frame(sock):
    a, b = read_exact(sock, 2)
    if not b & 128:
        raise AssertionError('client must mask')
    size = b & 127
    if size == 126:
        size = struct.unpack('!H', read_exact(sock, 2))[0]
    if size > 4096:
        raise AssertionError('client frame bound')
    mask = read_exact(sock, 4)
    data = read_exact(sock, size)
    return a & 15, bytes(c ^ mask[i % 4] for i, c in enumerate(data))


class Fixture:
    def __init__(self, fault=None):
        self.bootstrap = json.loads((ROOT / 'backend/releases/operator-bundle/quality-v6/bootstrap.json').read_text())
        self.bootstrap['capabilities']['world'] = {'available': True, 'reason': None}
        self.event = self.bootstrap['event']['event_id']
        self.origin = 'http://localhost:49151'
        self.fault = fault
        self.lock = threading.Lock()
        self.clients = {}
        self.tick = 0
        self.errors = []
        self.requests = []
        g = self.bootstrap['map_manifest']['geometry']
        spawn = next(x for x in g['spawns'] if x['spawn_id'] == g['default_spawn_id'])
        self.players = [dict(userId=actor, displayName='Fixture actor',
            position={'x': (spawn['cell_x'] + .5) * g['tile_width_px'],
                      'y': (spawn['cell_y'] + .5) * g['tile_height_px']},
            facing='down', moving=False, avatar=copy.deepcopy(self.bootstrap['avatar'])) for actor in ACTORS]
        owner = self

        class HTTP(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_GET(self):
                self.dispatch(False)

            def do_POST(self):
                self.dispatch(True)

            def dispatch(self, post):
                token = self.headers.get('Authorization', '')
                try:
                    i = TOKENS.index(token.removeprefix('Bearer '))
                    assert self.headers.get('Origin') == owner.origin
                    assert self.headers.get('X-MCR-Intent') == 'browser'
                    prefix = '/api/concert/events/' + owner.event
                    owner.requests.append(self.path)
                    assert self.path == prefix + ('/world-tickets' if post else '/bootstrap')
                    if post:
                        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                        assert request == {'room_id': ROOM, **dict(zip(probe.PIN_KEYS, probe.pins(owner.bootstrap)))}
                        now = time.time()
                        date = lambda value: probe.datetime.fromtimestamp(value, probe.timezone.utc).isoformat().replace('+00:00', 'Z')
                        value = dict(protocol_version='1.0.0', ticket=TICKETS[i], event_id=owner.event,
                                     room_id=ROOM, **dict(zip(probe.PIN_KEYS, probe.pins(owner.bootstrap))),
                                     server_time=date(now), expires_at=date(now + 20), ws_path='/world-ws', auth_timeout_ms=3000)
                        if owner.fault == 'ticket-binding':
                            value['room_id'] = str(uuid4())
                        if owner.fault == 'ticket-pin-type':
                            value['avatar_manifest_version'] = True
                    else:
                        value = copy.deepcopy(owner.bootstrap)
                        actor = ACTORS[0] if owner.fault == 'duplicate-actor' else ACTORS[i]
                        value['participant']['user_id'] = actor
                        value['participant']['profile']['user_id'] = actor
                        if owner.fault == 'release':
                            value['map_manifest']['geometry_hash'] = 'bad'
                    raw = json.dumps(value).encode()
                    self.send_response(503 if owner.fault == 'http-error' else 200)
                    self.send_header('Cache-Control', 'public' if owner.fault == 'cache' else 'no-store')
                    self.send_header('Content-Length', str(len(raw)))
                    self.end_headers()
                    self.wfile.write(raw)
                except Exception:
                    owner.errors.append('http-fixture-check')
                    self.send_error(400)

        class WS(socketserver.BaseRequestHandler):
            def handle(self):
                sock = self.request
                sock.settimeout(4)
                actor = None
                try:
                    raw = b''
                    while b'\r\n\r\n' not in raw:
                        raw += read_exact(sock, 1)
                        assert len(raw) <= 8192
                    lines = raw.decode().split('\r\n')
                    assert lines[0] == 'GET /world-ws HTTP/1.1'
                    h = dict(line.split(': ', 1) for line in lines[1:] if line)
                    assert h['Origin'] == owner.origin
                    assert 'Authorization' not in h and 'Sec-WebSocket-Protocol' not in h
                    accept = base64.b64encode(hashlib.sha1((h['Sec-WebSocket-Key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
                    if owner.fault == 'upgrade':
                        accept = 'bad-accept'
                    sock.sendall(('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ' + accept + '\r\n\r\n').encode())
                    opcode, raw = client_frame(sock)
                    message = json.loads(raw)
                    assert opcode == 1 and message['type'] == 'authenticate'
                    actor = TICKETS.index(message.pop('ticket'))
                    assert message == {'protocolVersion': '1.0.0', 'type': 'authenticate',
                        'eventId': owner.event, 'roomId': ROOM, **dict(zip(probe.WS_PIN_KEYS, probe.pins(owner.bootstrap)))}
                    with owner.lock:
                        stream = str(uuid4())
                        owner.clients[actor] = {'sock': sock, 'stream': stream, 'seq': 0, 'ack': 0}
                        snapshot = dict(protocolVersion='1.0.0', type='snapshot', streamId=stream, streamSeq=0,
                            roomEpoch=EPOCH, serverTick=owner.tick, eventId=owner.event, roomId=ROOM,
                            **dict(zip(probe.WS_PIN_KEYS, probe.pins(owner.bootstrap))), cause='INITIAL', requestId=None,
                            selfUserId=ACTORS[actor], ackInputSeq=0,
                            accessExpiresAt=probe.datetime.fromtimestamp(time.time()+60, probe.timezone.utc).isoformat().replace('+00:00', 'Z'),
                            players=[owner.players[x] for x in sorted(owner.clients)])
                        if owner.fault == 'snapshot-identity':
                            snapshot['selfUserId'] = str(uuid4())
                        if owner.fault == 'room':
                            snapshot['roomId'] = str(uuid4())
                        if owner.fault == 'avatar':
                            snapshot['players'][0]['avatar']['config']['preset_id'] = 'unapproved_fixture'
                        if owner.fault == 'extra-field':
                            snapshot['privateSecret'] = TOKENS[0]
                        if owner.fault == 'duplicate-player':
                            snapshot['players'].append(copy.deepcopy(snapshot['players'][0]))
                        sock.sendall(frame(json.dumps(snapshot).encode()))
                        if actor == 1:
                            owner.emit([owner.players[1]], targets=[0])
                    while True:
                        opcode, raw = client_frame(sock)
                        if opcode == 8:
                            sock.sendall(frame(raw, 8))
                            with owner.lock:
                                del owner.clients[actor]
                                if actor == 0 and 1 in owner.clients:
                                    owner.emit([], removes=[ACTORS[0]])
                            return
                        message = json.loads(raw)
                        assert opcode == 1 and message['type'] == 'input'
                        assert set(message) == {'protocolVersion', 'type', 'streamId', 'roomEpoch', 'seq', 'axisX', 'axisY', 'buttons'}
                        assert actor == 0 and message['buttons'] == []
                        with owner.lock:
                            owner.tick += 1
                            owner.clients[actor]['ack'] = message['seq']
                            p = owner.players[actor]
                            p['position']['x'] += message['axisX'] * 8
                            p['position']['y'] += message['axisY'] * 8
                            p['moving'] = bool(message['axisX'] or message['axisY'])
                            if owner.fault == 'teleport':
                                p['position']['x'] += 16
                            owner.emit([p])
                except (EOFError, BrokenPipeError, ConnectionResetError):
                    if owner.fault != 'upgrade':
                        owner.errors.append('unexpected-fixture-eof')
                except Exception:
                    owner.errors.append('ws-fixture-check')

        class Server(socketserver.ThreadingTCPServer):
            allow_reuse_address = True
            daemon_threads = True

        self.http = ThreadingHTTPServer(('127.0.0.1', 0), HTTP)
        self.ws = Server(('127.0.0.1', 0), WS)
        self.threads = [threading.Thread(target=s.serve_forever, kwargs={'poll_interval': .01}, daemon=True) for s in (self.http, self.ws)]

    def emit(self, upserts, removes=(), targets=None):
        for i in list(self.clients) if targets is None else targets:
            c = self.clients[i]
            c['seq'] += 1
            value = dict(protocolVersion='1.0.0', type='delta', streamId=c['stream'], streamSeq=c['seq'],
                         roomEpoch=EPOCH, serverTick=self.tick, ackInputSeq=c['ack'], upserts=upserts, removes=list(removes))
            if self.fault == 'sequence':
                value['streamSeq'] += 1
            if self.fault == 'epoch':
                value['roomEpoch'] = str(uuid4())
            if self.fault == 'ack':
                value['ackInputSeq'] = 99
            c['sock'].sendall(frame(json.dumps(value).encode()))

    def __enter__(self):
        for thread in self.threads:
            thread.start()
        return self

    def __exit__(self, *_args):
        for server in (self.http, self.ws):
            server.shutdown()
            server.server_close()
        for thread in self.threads:
            thread.join(1)

    def run(self):
        return probe.run_probe('http://127.0.0.1:' + str(self.http.server_port), self.origin, self.event, ROOM,
                               TOKENS, 'ws://127.0.0.1:' + str(self.ws.server_address[1]) + '/world-ws', self.bootstrap, timeout=5)


class ProbeChecks(unittest.TestCase):
    def test_synthetic_public_two_actor_flow_movement_ack_peer_removal_and_closes(self):
        with Fixture() as fixture:
            report = fixture.run()
            self.assertEqual(report['result'], 'PASS', report)
            self.assertEqual(report['actor_count'], 2)
            self.assertEqual(report['consumed_input_ack'], 2)
            self.assertEqual(report['peer_observed_removal'], 'PASS')
            self.assertEqual(report['cleanup'], {'opened': 2, 'close_acknowledged': 2})
            self.assertEqual(len(fixture.requests), 4)
            self.assertEqual(fixture.errors, [])
            self.assertFalse(any(secret in json.dumps(report) for secret in TOKENS + TICKETS + ACTORS))

    def test_untrusted_bindings_and_movement_fail_closed_without_secrets(self):
        for fault in ['duplicate-actor', 'release', 'ticket-binding', 'ticket-pin-type', 'snapshot-identity',
                      'upgrade', 'sequence', 'teleport', 'room', 'avatar', 'extra-field', 'duplicate-player',
                      'epoch', 'ack', 'cache', 'http-error']:
            with self.subTest(fault=fault), Fixture(fault) as fixture:
                report = fixture.run()
                self.assertEqual(report['result'], 'FAIL')
                self.assertFalse(any(secret in json.dumps(report) for secret in TOKENS + TICKETS + ACTORS))

    def test_invalid_targets_and_same_bearer_never_open_network(self):
        with patch.object(probe, 'connect') as connect:
            with Fixture() as fixture:
                for url in ['http://127.0.0.1/?secret', 'http://user:password@127.0.0.1', 'http://example.com', 'file:///tmp/x']:
                    r = probe.run_probe(url, fixture.origin, fixture.event, ROOM, TOKENS, 'ws://127.0.0.1/world-ws', fixture.bootstrap)
                    self.assertEqual(r['result'], 'FAIL')
                r = probe.run_probe('http://127.0.0.1', fixture.origin, fixture.event, ROOM, [TOKENS[0]]*2,
                                    'ws://127.0.0.1/world-ws', fixture.bootstrap)
                self.assertEqual(r['failure'], 'two-bearers')
            connect.assert_not_called()

    def test_duplicate_and_nonfinite_json_rejected(self):
        for raw in [b'{"type":"ping","type":"delta"}', b'{"x":NaN}', b'{"x":Infinity}', b'[]', b'\xff', '{}'.encode('utf-16')]:
            with self.subTest(raw=raw), self.assertRaises(probe.ProbeFailure):
                probe.unique_json(raw)

    def test_fragmented_text_interleaved_ping_and_masked_pong(self):
        a, b = socket.socketpair()
        self.addCleanup(a.close)
        self.addCleanup(b.close)
        ws = probe.WebSocket(a, time.monotonic()+1)
        b.sendall(frame(b'{"type":', final=False) + frame(b'ping-data', 9) + frame(b'"fixture"}', 0))
        self.assertEqual(ws.receive(), {'type': 'fixture'})
        opcode, data = client_frame(b)
        self.assertEqual((opcode, data), (10, b'ping-data'))

    def test_masked_outgoing_text_and_application_pong(self):
        a, b = socket.socketpair()
        self.addCleanup(a.close)
        self.addCleanup(b.close)
        ws = probe.WebSocket(a, time.monotonic()+1)
        ws.send({'test': 'x'*200})
        opcode, raw = client_frame(b)
        self.assertEqual(opcode, 1)
        self.assertEqual(json.loads(raw), {'test': 'x'*200})
        with Fixture() as fixture:
            view = probe.View(ACTORS[0], fixture.bootstrap, fixture.event, ROOM)
            heartbeat = str(uuid4())
            view.accept({'protocolVersion':'1.0.0','type':'ping','heartbeatId':heartbeat}, ws)
            _, raw = client_frame(b)
            self.assertEqual(json.loads(raw), {'protocolVersion':'1.0.0','type':'pong','heartbeatId':heartbeat})

    def test_frame_rejection_before_unbounded_payload_read(self):
        invalid = [b'\x81\x80', b'\xc1\x00', b'\x82\x00', b'\x09\x00',
                   b'\x81\x7e\x00\x01', b'\x81\x7f'+struct.pack('!Q', 65537),
                   b'\x80\x02{}', b'\x81\x02\xff\xff', b'\x88\x00']
        for raw in invalid:
            with self.subTest(raw=raw):
                a, b = socket.socketpair()
                try:
                    ws = probe.WebSocket(a, time.monotonic()+.1)
                    b.sendall(raw)
                    with self.assertRaises(probe.ProbeFailure):
                        ws.receive()
                finally:
                    a.close()
                    b.close()

    def test_receive_deadline_bounds_partial_frame(self):
        a, b = socket.socketpair()
        try:
            ws = probe.WebSocket(a, time.monotonic()+.05)
            b.sendall(b'\x81\x05x')
            start = time.monotonic()
            with self.assertRaises((probe.ProbeFailure, TimeoutError)):
                ws.receive()
            self.assertLess(time.monotonic()-start, .3)
        finally:
            a.close()
            b.close()

    def test_http_chunked_and_eof_reads_have_total_deadline(self):
        for data in [b'HTTP/1.1 200 OK\r\nCache-Control: no-store\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n',
                     b'HTTP/1.1 200 OK\r\nCache-Control: no-store\r\n\r\n{}']:
            a, b = socket.socketpair()
            b.sendall(data)
            b.shutdown(socket.SHUT_WR)
            try:
                with patch.object(probe, 'connect', return_value=a):
                    value = probe.http_json(probe.endpoint('http://127.0.0.1'), 'http://localhost', '/fixture', TOKENS[0], time.monotonic()+1)
                self.assertEqual(value, {})
            finally:
                a.close()
                b.close()

    def test_partial_http_body_respects_deadline_and_closes_socket(self):
        a, b = socket.socketpair()
        b.sendall(b'HTTP/1.1 200 OK\r\nCache-Control: no-store\r\nContent-Length: 10\r\n\r\n{')
        try:
            start = time.monotonic()
            with patch.object(probe, 'connect', return_value=a), self.assertRaises((TimeoutError, probe.ProbeFailure)):
                probe.http_json(probe.endpoint('http://127.0.0.1'), 'http://localhost', '/fixture', TOKENS[0], time.monotonic()+.05)
            self.assertLess(time.monotonic()-start, .3)
            self.assertEqual(a.fileno(), -1)
        finally:
            a.close()
            b.close()


if __name__ == '__main__':
    unittest.main()
