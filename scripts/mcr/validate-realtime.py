#!/usr/bin/env python3
"""Offline C05 wire/trace oracle. This is not a WebSocket server or auth store."""
from __future__ import annotations

import copy
from datetime import datetime
import json
import math
from pathlib import Path
import sys
from uuid import UUID

from jsonschema import Draft202012Validator, FormatChecker, ValidationError
from referencing import Registry, Resource
from referencing.exceptions import NoSuchResource

ROOT = Path(__file__).resolve().parents[2]
DIRECTORY = ROOT / 'contracts/realtime/v1'
FIXTURES = ROOT / 'contracts/fixtures/realtime'
IDENTITY = ('eventId', 'roomId', 'mapId', 'mapRevision',
            'avatarManifestId', 'avatarManifestVersion')


class WireError(ValueError):
    def __init__(self, reason, code=4400):
        super().__init__(reason)
        self.code = code


def require(condition, reason, code=4400):
    if not condition:
        raise WireError(reason, code)


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def reject_constant(_value):
    raise WireError('non-finite JSON number')


def read(path):
    return json.loads(path.read_text(encoding='utf-8'),
                      object_pairs_hook=unique_pairs, parse_constant=reject_constant)


def inspect_json(value, depth=0):
    require(depth <= 16, 'JSON nesting limit')
    require(type(value) in (dict, list, str, int, float, bool, type(None)), 'invalid JSON value')
    if isinstance(value, float):
        require(math.isfinite(value), 'non-finite JSON number')
    elif isinstance(value, str):
        try:
            value.encode('utf-8')
        except UnicodeError:
            raise WireError('invalid Unicode string', 1007) from None
    elif isinstance(value, dict):
        require(all(isinstance(k, str) for k in value), 'invalid JSON key')
        for key, child in value.items():
            inspect_json(key, depth + 1)
            inspect_json(child, depth + 1)
    elif isinstance(value, list):
        for child in value:
            inspect_json(child, depth + 1)


def offline_only(uri):
    raise NoSuchResource(ref=uri)


FORMATS = FormatChecker()


@FORMATS.checks('uuid', raises=(ValueError, AttributeError))
def canonical_uuid(value):
    return not isinstance(value, str) or str(UUID(value)) == value


class Contract:
    def __init__(self):
        paths = [DIRECTORY / 'world.schema.json', ROOT / 'contracts/core/v1/core.schema.json']
        paths += sorted((ROOT / 'contracts/assets/v1').glob('*.schema.json'))
        documents = [read(path) for path in paths]
        for document in documents:
            Draft202012Validator.check_schema(document)
        self.schema = documents[0]
        self.policy = read(DIRECTORY / 'policy.json')
        self.operations = read(DIRECTORY / 'operations.json')
        registry = Registry(retrieve=offline_only).with_resources(
            (doc['$id'], Resource.from_contents(doc)) for doc in documents)
        self.validators = {
            name: Draft202012Validator(
                {'$ref': self.schema['$id'] + '#/$defs/' + name},
                registry=registry, format_checker=FORMATS)
            for name in self.schema['$defs']
        }
        for name in ('CoreErrorResponse', 'OriginErrorResponse'):
            self.validators[name] = Draft202012Validator(
                {'$ref': documents[1]['$id'] + '#/$defs/' + name},
                registry=registry, format_checker=FORMATS)

    def validate_http(self, value, operation_id, status, *, event_id=None, request=None):
        operations = [op for op in self.operations if op['operationId'] == operation_id]
        require(len(operations) == 1, 'unknown HTTP operation')
        operation = operations[0]
        if status == operation['response_status']:
            self.validate(value, operation['response'])
            if event_id is not None:
                require(value['event_id'] == event_id, 'HTTP event mismatch')
            if operation['request'] is not None:
                require(request is not None, 'ticket request context required')
                self.validate(request, operation['request'])
                require(all(value[key] == field for key, field in request.items()),
                        'ticket response binding mismatch')
            return
        require(str(status) in operation['errors'], 'undocumented HTTP status')
        if status == 403 and 'origin_error_schema' in operation:
            try:
                self.validate(value, 'OriginErrorResponse')
                return
            except WireError:
                pass
        self.validate(value, 'CoreErrorResponse')
        require(value['details']['reason'] in operation['errors'][str(status)],
                'HTTP reason/status mismatch')

    def validate(self, value, name):
        require(name in self.validators, 'unknown schema')
        inspect_json(value)
        try:
            self.validators[name].validate(value)
        except ValidationError:
            raise WireError('schema mismatch') from None
        kind = value.get('type') if isinstance(value, dict) else None
        if kind == 'input':
            require(value['axisX'] ** 2 + value['axisY'] ** 2 <= 1 + 1e-9,
                    'input vector exceeds unit length')
        if kind in ('snapshot', 'delta'):
            rows = value['players'] if kind == 'snapshot' else value['upserts']
            ids = [row['userId'] for row in rows]
            require(len(set(ids)) == len(ids), 'duplicate player')
            if kind == 'snapshot':
                require(value['selfUserId'] in ids, 'snapshot omits self')
                for row in rows:
                    self.avatar_matches(row, value)
                correlated = value['cause'] in ('RESYNC', 'ROOM_SWITCH')
                require((value['requestId'] is not None) == correlated,
                        'snapshot correlation mismatch')
            else:
                require(not set(ids).intersection(value['removes']), 'upsert/remove overlap')
        if name == 'WorldTicketResponse':
            seconds = (datetime.fromisoformat(value['expires_at']) -
                       datetime.fromisoformat(value['server_time'])).total_seconds()
            require(0 < seconds <= self.policy['ticket_ttl_ms'] / 1000, 'ticket TTL mismatch')
        if name == 'WorldRooms':
            ids = [row['room_id'] for row in value['items']]
            require(len(ids) == len(set(ids)), 'duplicate room')
            require(value['default_room_id'] in ids, 'default room missing')
        return value

    @staticmethod
    def avatar_matches(player, context):
        config = player['avatar']['config']
        require(config['manifest_id'] == context['avatarManifestId'] and
                config['manifest_version'] == context['avatarManifestVersion'],
                'avatar manifest mismatch')

    def decode(self, payload, direction, *, binary=False):
        require(direction in ('client', 'server'), 'invalid direction')
        require(not binary, 'binary data unsupported', 1003)
        require(isinstance(payload, (str, bytes)), 'invalid payload type')
        try:
            data = payload.encode('utf-8') if isinstance(payload, str) else payload
        except UnicodeError:
            raise WireError('invalid UTF-8', 1007) from None
        require(len(data) <= self.policy[direction + '_message_bytes'], 'message too big', 1009)
        try:
            value = json.loads(data.decode('utf-8'), object_pairs_hook=unique_pairs,
                               parse_constant=reject_constant)
        except UnicodeError:
            raise WireError('invalid UTF-8', 1007) from None
        except (json.JSONDecodeError, RecursionError):
            raise WireError('invalid JSON') from None
        return self.validate(value, 'ClientMessage' if direction == 'client' else 'ServerMessage')


class Trace:
    """A delivered, ordered connection transcript; never authenticates a real user."""
    def __init__(self, contract, actor_id):
        require(canonical_uuid(actor_id) and isinstance(actor_id, str), 'invalid actor')
        self.contract, self.actor = contract, actor_id
        self.authenticate = None
        self.snapshot = None
        self.players = {}
        self.sequence = self.input_sequence = self.ack = 0
        self.tick = 0
        self.pending = None
        self.heartbeat = None
        self.used_streams, self.used_requests = set(), set()
        self.suspended = self.closed = False

    def consume(self, direction, message):
        # Route objects through the exact wire parser and byte limits as fixtures do.
        message = self.contract.decode(json.dumps(message, ensure_ascii=False,
                                                  allow_nan=False), direction)
        require(not self.closed, 'message after revoke')
        kind = message['type']
        if direction == 'client' and kind == 'authenticate':
            require(self.authenticate is None, 'duplicate authenticate')
            self.authenticate = copy.deepcopy(message)
            return
        require(self.authenticate is not None, 'authenticate must be first', 4401)
        if direction == 'server' and kind == 'snapshot':
            self.accept_snapshot(message)
            return
        require(self.snapshot is not None, 'snapshot required')
        if 'streamId' in message:
            require(message['streamId'] == self.snapshot['streamId'], 'obsolete stream')
        if 'roomEpoch' in message:
            require(message['roomEpoch'] == self.snapshot['roomEpoch'], 'epoch changed', 4409)
        if direction == 'client':
            if kind == 'input':
                require(not self.suspended, 'input during resync')
                require(message['seq'] > self.input_sequence, 'input replay')
                self.input_sequence = message['seq']
            elif kind in ('resync', 'switchRoom'):
                require(self.pending is None, 'control request already pending')
                require(message['requestId'] not in self.used_requests, 'request ID reused')
                if kind == 'resync':
                    require(message['lastStreamSeq'] <= self.sequence, 'future stream acknowledgement')
                    self.suspended = True
                else:
                    require(not self.suspended, 'switch during resync')
                    require(message['eventId'] == self.snapshot['eventId'], 'cross-event switch')
                    require(message['roomId'] != self.snapshot['roomId'], 'switch targets current room')
                self.used_requests.add(message['requestId'])
                self.pending = copy.deepcopy(message)
            elif kind == 'pong':
                require(message['heartbeatId'] == self.heartbeat, 'unsolicited pong')
                self.heartbeat = None
            return
        if kind == 'delta':
            require(not self.suspended, 'delta before resync snapshot', 4409)
            require(message['streamSeq'] == self.sequence + 1, 'stream sequence gap/replay', 4409)
            require(message['serverTick'] >= self.tick, 'server tick regressed')
            require(self.ack <= message['ackInputSeq'] <= self.input_sequence, 'invalid input acknowledgement')
            updated = copy.deepcopy(self.players)
            for removed in message['removes']:
                require(removed in updated and removed != self.actor, 'invalid player removal')
                del updated[removed]
            for row in message['upserts']:
                self.contract.avatar_matches(row, self.snapshot)
                updated[row['userId']] = copy.deepcopy(row)
            require(len(updated) <= self.contract.policy['room_capacity_max'], 'too many players')
            self.players = updated
            self.sequence, self.tick, self.ack = message['streamSeq'], message['serverTick'], message['ackInputSeq']
        elif kind == 'switchRejected':
            require(self.pending is not None and self.pending['type'] == 'switchRoom' and
                    self.pending['requestId'] == message['requestId'], 'uncorrelated switch rejection')
            self.pending = None
        elif kind == 'resyncRequired':
            require(self.pending is None or self.pending['type'] == 'resync', 'conflicting control transition')
            self.suspended = True
        elif kind == 'revoke':
            self.closed = True
            self.players.clear()
            self.pending = self.heartbeat = None
        elif kind == 'ping':
            require(self.heartbeat is None, 'previous heartbeat unanswered')
            self.heartbeat = message['heartbeatId']

    def accept_snapshot(self, message):
        require(message['selfUserId'] == self.actor, 'self identity mismatch')
        require(message['streamId'] not in self.used_streams, 'snapshot reused stream ID')
        require(message['ackInputSeq'] == 0, 'new stream must reset input acknowledgement')
        if self.snapshot is None:
            require(message['cause'] == 'INITIAL', 'initial snapshot cause mismatch')
            expected = self.authenticate
        elif message['cause'] in ('RESYNC', 'ROOM_SWITCH'):
            expected_type = 'resync' if message['cause'] == 'RESYNC' else 'switchRoom'
            require(self.pending is not None and self.pending['type'] == expected_type and
                    self.pending['requestId'] == message['requestId'], 'uncorrelated snapshot')
            expected = self.pending if expected_type == 'switchRoom' else self.snapshot
        else:
            require(message['cause'] == 'EPOCH_CHANGED' and self.pending is None and
                    message['roomEpoch'] != self.snapshot['roomEpoch'], 'invalid epoch replacement')
            expected = self.snapshot
        require(all(message[key] == expected[key] for key in IDENTITY), 'snapshot resource mismatch')
        if self.snapshot is not None and message['roomId'] == self.snapshot['roomId'] and message['roomEpoch'] == self.snapshot['roomEpoch']:
            require(message['serverTick'] >= self.tick, 'snapshot tick regressed')
        # Validate everything before replacing the old complete baseline.
        self.snapshot = copy.deepcopy(message)
        self.players = {row['userId']: copy.deepcopy(row) for row in message['players']}
        self.sequence = self.input_sequence = self.ack = 0
        self.tick = message['serverTick']
        self.pending = None
        self.suspended = False
        self.used_streams.add(message['streamId'])


def filtered_projection(fixture, contract):
    """Reference fixture producer: privacy projection precedes sequence allocation."""
    traces, delivered, all_players = {}, {}, {}
    for viewer in fixture['viewers']:
        trace = Trace(contract, viewer['actor_id'])
        for row in viewer['initial']:
            trace.consume(row['direction'], row['message'])
        traces[viewer['actor_id']] = trace
        delivered[viewer['actor_id']] = []
        all_players.update(copy.deepcopy(trace.players))
    for change in fixture['updates']:
        for row in change['upserts']:
            all_players[row['userId']] = copy.deepcopy(row)
        for user_id in change['removes']:
            all_players.pop(user_id, None)
        blocked = {frozenset(pair) for pair in change['blocked_pairs']}
        for actor, trace in traces.items():
            visible = {uid: row for uid, row in all_players.items()
                       if uid == actor or frozenset((actor, uid)) not in blocked}
            upserts = [row for uid, row in sorted(visible.items()) if trace.players.get(uid) != row]
            removes = sorted(set(trace.players) - set(visible))
            if not upserts and not removes:
                continue
            delta = {'protocolVersion': '1.0.0', 'type': 'delta',
                     'streamId': trace.snapshot['streamId'], 'streamSeq': trace.sequence + 1,
                     'roomEpoch': trace.snapshot['roomEpoch'], 'serverTick': change['server_tick'],
                     'ackInputSeq': trace.ack, 'upserts': upserts, 'removes': removes}
            trace.consume('server', delta)
            delivered[actor].append(delta)
    require(delivered == fixture['expected'], 'privacy projection golden mismatch')
    return delivered


def validate_fixtures():
    contract = Contract()
    index = read(FIXTURES / 'index.json')
    seen = set()
    for entry in index:
        name = entry['file']
        require(Path(name).name == name and name.endswith('.json') and name not in seen,
                'invalid fixture index')
        seen.add(name)
        fixture = read(FIXTURES / name)
        kind = entry['kind']
        if kind == 'trace':
            trace = Trace(contract, fixture['actor_id'])
            for row in fixture['messages']:
                trace.consume(row['direction'], row['message'])
            require(sorted(trace.players) == fixture['expected_players'], 'trace final roster mismatch')
        elif kind == 'projection':
            filtered_projection(fixture, contract)
        elif kind == 'wire-rejection':
            try:
                contract.decode(fixture['payload'], fixture['direction'], binary=fixture.get('binary', False))
            except WireError as error:
                require(error.code == fixture['close_code'], 'unexpected rejection code')
            else:
                raise WireError('negative wire fixture accepted')
        elif kind == 'http-response':
            request = read(FIXTURES / entry['request_file']) if entry.get('request_file') else None
            contract.validate_http(fixture, entry['operation_id'], entry['status'],
                                   event_id=entry.get('event_id'), request=request)
        else:
            require(kind == 'http', 'unknown fixture kind')
            contract.validate(fixture, entry['schema'])
    require(seen == {p.name for p in FIXTURES.glob('*.json')} - {'index.json'}, 'unindexed fixture')
    return len(seen)


if __name__ == '__main__':
    try:
        count = validate_fixtures()
    except (ValueError, OSError) as error:
        print(f'Realtime contract check failed: {error}', file=sys.stderr)
        raise SystemExit(1)
    print(json.dumps({'state': 'CONTRACT_READY', 'fixtures': count,
                      'websocket_runtime': 'NOT_RUN', 'capacity': 'NOT_RUN'}))
