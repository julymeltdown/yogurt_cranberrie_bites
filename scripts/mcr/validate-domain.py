#!/usr/bin/env python3
"""C04 offline contract oracle. No database, provider, HTTP or concurrency proof."""
from __future__ import annotations

from datetime import date, datetime, timedelta
import importlib.util
import json
import math
from pathlib import Path
import re
import sys
from uuid import UUID
from urllib.parse import parse_qsl
from zoneinfo import ZoneInfo

from jsonschema import Draft202012Validator, FormatChecker, ValidationError
from referencing import Registry, Resource
from referencing.exceptions import NoSuchResource

ROOT = Path(__file__).resolve().parents[2]
DIRECTORY = ROOT / 'contracts/domain/v1'
FIXTURES = ROOT / 'contracts/fixtures/domain'
MAX_SAFE = 9007199254740991
SEOUL = ZoneInfo('Asia/Seoul')


class ContractError(ValueError):
    pass


def require(condition, reason):
    if not condition:
        raise ContractError(reason)


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def reject_constant(_):
    raise ContractError('non-finite number')


def read(path):
    try:
        return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=unique_pairs,
                          parse_constant=reject_constant)
    except (ValueError, RecursionError, UnicodeError):
        raise ContractError('invalid JSON document') from None


def inspect(value, depth=0):
    require(depth <= 32, 'JSON nesting limit')
    require(type(value) in (dict, list, str, int, float, bool, type(None)), 'non-JSON value')
    if type(value) in (int, float):
        require(type(value) is int or math.isfinite(value), 'non-finite number')
        require(abs(value) <= MAX_SAFE, 'number exceeds safe wire range')
    elif isinstance(value, str):
        try:
            value.encode('utf-8')
        except UnicodeError:
            raise ContractError('invalid Unicode') from None
    elif isinstance(value, list):
        for child in value:
            inspect(child, depth + 1)
    elif isinstance(value, dict):
        require(all(isinstance(key, str) for key in value), 'non-string JSON key')
        for key, child in value.items():
            inspect(key, depth + 1)
            inspect(child, depth + 1)


def timestamp(value):
    require(isinstance(value, str) and value.endswith('Z'), 'UTC timestamp required')
    try:
        return datetime.fromisoformat(value)
    except ValueError:
        raise ContractError('invalid timestamp') from None


def offline_only(uri):
    raise NoSuchResource(ref=uri)


FORMATS = FormatChecker()


@FORMATS.checks('uuid', raises=(ValueError, AttributeError))
def canonical_uuid(value):
    return not isinstance(value, str) or str(UUID(value)) == value


def distinct(rows, key):
    ids = [row[key] for row in rows]
    require(len(ids) == len(set(ids)), 'duplicate ' + key)


def semantic(value):
    """Cross-field invariants in the candidate's closed DTOs, including nested DTOs."""
    if isinstance(value, list):
        for child in value:
            semantic(child)
        return
    if not isinstance(value, dict):
        return
    for child in value.values():
        semantic(child)
    for start, end in [('meeting_starts_at', 'meeting_ends_at'),
                       ('pickup_starts_at', 'pickup_ends_at'),
                       ('created_at', 'expires_at'), ('verified_at', 'expires_at')]:
        if value.get(start) is not None and value.get(end) is not None:
            left, right = timestamp(value[start]), timestamp(value[end])
            replaced = start == 'verified_at' and value.get('status') == 'REPLACED'
            require(left <= right if replaced else left < right, 'invalid time window')
    if 'minimum_age' in value:
        lower, upper = value['minimum_age'], value['maximum_age_exclusive']
        require((lower is None) == (upper is None), 'partially specified age bounds')
        require(lower is None or lower < upper, 'empty age range')
    for lower, upper in [('starts_after', 'starts_before'), ('pickup_after', 'pickup_before'),
                         ('from', 'until')]:
        if lower in value and upper in value:
            require(timestamp(value[lower]) < timestamp(value[upper]), 'empty filter time range')
    if 'capacity' in value and 'member_count' in value:
        require(value['member_count'] <= value['capacity'], 'party exceeds capacity')
    if 'quantity_total' in value and 'available' in value:
        require(value['available'] <= value['quantity_total'], 'invalid available stock')
    if {'giveaway', 'reserved_unclaimed', 'quantity_claimed'} <= value.keys():
        giveaway = value['giveaway']
        require(giveaway['available'] == giveaway['quantity_total'] -
                value['reserved_unclaimed'] - value['quantity_claimed'], 'stock equation mismatch')
        distinct(value['reservations'], 'reservation_id')
        require(all(row['giveaway_id'] == giveaway['giveaway_id'] and
                    row['event_id'] == giveaway['event_id'] for row in value['reservations']),
                'foreign reservation in management page')
    if 'claim_receipt_id' in value:
        require((value['state'] == 'CLAIMED') == (value['claim_receipt_id'] is not None),
                'pickup receipt/state mismatch')
    for key in ('consents', 'available_handles'):
        if key in value:
            distinct(value[key], 'platform')
    if {'requester_id', 'recipient_id'} <= value.keys():
        require(value['requester_id'] != value['recipient_id'], 'self social request')
    if 'approved_platforms' in value and 'consents' in value:
        approved = set(value['approved_platforms'])
        require(approved <= {row['platform'] for row in value['consents']}, 'unrequested platform')
        if value['state'] == 'APPROVED':
            require(bool(approved), 'approval without platforms')
        if value['state'] in ('PENDING', 'REJECTED', 'CANCELLED'):
            require(not approved, 'unapproved request has grant platforms')
    if {'owner_id', 'viewer_id'} <= value.keys():
        require(value['owner_id'] != value['viewer_id'], 'self social grant')
    if 'x_min' in value:
        require(value['x_min'] < value['x_max_exclusive'] and
                value['y_min'] < value['y_max_exclusive'], 'empty mural region')
    if {'from_revision', 'to_revision', 'operations', 'reset_required'} <= value.keys():
        require(value['from_revision'] <= value['to_revision'], 'mural revision regression')
        if value['reset_required']:
            require(not value['operations'], 'reset must not contain operations')
        else:
            revisions = [row['revision'] for row in value['operations']]
            require(value['to_revision'] - value['from_revision'] <= 512, 'mural delta too long')
            require(revisions == list(range(value['from_revision'] + 1, value['to_revision'] + 1)),
                    'mural operation gap')
            require(all(row['event_id'] == value['event_id'] for row in value['operations']),
                    'foreign mural operation')
    if 'steps' in value:
        distinct(value['steps'], 'name')
        if value['state'] == 'COMPLETED':
            require(all(step['state'] == 'SUCCEEDED' for step in value['steps']),
                    'incomplete closure marked complete')
    if {'receipt_id', 'failure_reason', 'result_status'} <= value.keys():
        state = value['state']
        require((state == 'SUCCEEDED') == (value['result_status'] is not None),
                'receipt result/status mismatch')
        require((state == 'FAILED') == (value['failure_reason'] is not None),
                'receipt failure mismatch')
        if state in ('PENDING', 'RETIRED'):
            require(value['resource_id'] is None and value['resource_revision'] is None,
                    'unresolved receipt contains result')


class Contract:
    def __init__(self):
        paths = sorted(DIRECTORY.glob('*.schema.json'))
        paths += [ROOT / 'contracts/core/v1/core.schema.json']
        paths += sorted((ROOT / 'contracts/assets/v1').glob('*.schema.json'))
        self.documents = [read(path) for path in paths]
        for document in self.documents:
            Draft202012Validator.check_schema(document)
        ids = [doc['$id'] for doc in self.documents]
        require(len(ids) == len(set(ids)), 'duplicate schema ID')
        self.registry = Registry(retrieve=offline_only).with_resources(
            (doc['$id'], Resource.from_contents(doc)) for doc in self.documents)
        self.validators = {}
        for path, doc in zip(paths, self.documents):
            if path.parent == DIRECTORY:
                group = path.name.split('.')[0]
                for name in doc['$defs']:
                    self.validators[group + '.' + name] = Draft202012Validator(
                        {'$ref': doc['$id'] + '#/$defs/' + name}, registry=self.registry,
                        format_checker=FORMATS)
            self._resolve_all(doc, doc['$id'])
        self.policy = read(DIRECTORY / 'policy.json')
        self.operations = read(DIRECTORY / 'operations.json')
        self.inherited = read(DIRECTORY / 'inherited-map.json')
        self.check_register()

    def _resolve_all(self, value, base):
        if isinstance(value, dict):
            if '$ref' in value:
                try:
                    self.registry.resolver(base).lookup(value['$ref'])
                except Exception:
                    raise ContractError('unresolvable offline schema reference') from None
            for child in value.values():
                self._resolve_all(child, base)
        elif isinstance(value, list):
            for child in value:
                self._resolve_all(child, base)

    def check_register(self):
        ids, routes = set(), set()
        known_reasons = {b['properties']['reason']['const'] for doc in self.documents
                         if doc['$id'].endswith('/domain/v1/common.schema.json')
                         for b in doc['$defs']['DomainError']['properties']['details']['oneOf']}
        for op in self.operations:
            require(op['operationId'] not in ids, 'duplicate operation ID')
            require((op['method'], op['path']) not in routes, 'duplicate route')
            ids.add(op['operationId']); routes.add((op['method'], op['path']))
            require(op['path'].startswith('/api/concert/events/{eventId}/'), 'invalid event scope')
            require(op['method'] in ('get', 'post', 'put', 'patch', 'delete'), 'invalid method')
            require(op['actor'] and op['rule'] in self.policy['rules'], 'missing semantic rule')
            rule = self.policy['rules'][op['rule']]
            require(rule['preconditions'] and rule['result'], 'empty operation semantics')
            for key in ('request', 'response', 'pending_response'):
                name = op[key]
                require(name in (None, 'BINARY') or name in self.validators, 'unknown operation schema')
            parameters = re.findall(r'\{(\w+)\}', op['path'])
            require(len(parameters) == len(set(parameters)) and
                    set(op.get('path_parameters', {})) == set(parameters),
                    'path parameter register mismatch')
            for parameter, schema in op['path_parameters'].items():
                expected = 'Platform' if parameter == 'platform' else 'Uuid'
                require(schema == {'$ref': 'https://contracts.mcr.invalid/domain/v1/common.schema.json#/$defs/' + expected},
                        'path parameter type mismatch')
            query = op.get('query_schema')
            require(query in self.validators, 'missing query schema')
            query_definition = self._definition(query)
            require(query_definition.get('type') == 'object' and
                    query_definition.get('additionalProperties') is False, 'query must be closed')
            for scalar in query_definition['properties'].values():
                self._scalar_schema(scalar)
            expected_query = op['request'] if op['request_location'] == 'query' and op['request'] else 'common.EmptyRequest'
            require(query == expected_query, 'query/request register mismatch')
            require(op['request_location'] == ('query' if op['method'] in ('get', 'delete') else 'body'),
                    'request location mismatch')
            require(op['write_policy'] in self.policy['write_policies'], 'unknown write policy')
            require(op['cache_control'] == 'no-store', 'personalized cache leak')
            require(all(r in known_reasons for rs in op['errors'].values() for r in rs),
                    'unknown error reason')
            require(all(self.policy['http_error_status'].get(reason) == int(status)
                        for status, reasons in op['errors'].items() for reason in reasons),
                    'error status registry mismatch')
            mutation = op['method'] != 'get'
            require(op['idempotency_key'] == mutation, 'missing command recovery')
            if mutation:
                require({'ORIGIN_NOT_ALLOWED', 'BROWSER_INTENT_REQUIRED'} <= set(op['errors']['403']),
                        'missing browser guard contract')
                require(op['pending_status'] == 202 and op['pending_response'] == 'common.CommandReceipt',
                        'missing async recovery contract')
            else:
                require(op['pending_status'] is None and op['pending_response'] is None,
                        'read has command receipt')
        for aggregate, edges in self.policy['transitions'].items():
            require(all(state in edges for targets in edges.values() for state in targets),
                    'transition has unknown state: ' + aggregate)
        require(all(row['operationId'] in ids for row in self.inherited['operations']),
                'mapping has unknown target operation')

    def check_inherited(self):
        path = ROOT / 'backend/scripts/docs/generate-openapi.py'
        spec = importlib.util.spec_from_file_location('mcr_inherited_domain_audit', path)
        module = importlib.util.module_from_spec(spec)
        sys.path.insert(0, str(path.parent))
        try:
            spec.loader.exec_module(module)
            document = module.build_document()
        finally:
            sys.path.pop(0)
        for op in self.inherited['operations'] + self.inherited['receipt_queries']:
            expected = op.get('inherited_operationId', op['operationId'])
            actual = document['paths'].get(op['path'], {}).get(op['method'], {}).get('operationId')
            require(actual == expected, 'inherited operation drift')

    def validate(self, value, name):
        require(name in self.validators, 'unknown schema')
        inspect(value)
        try:
            self.validators[name].validate(value)
        except ValidationError:
            raise ContractError('schema mismatch: ' + name) from None
        semantic(value)
        if name == 'social.PrivateAgeClaim':
            start, end = timestamp(value['verified_at']), timestamp(value['expires_at'])
            require(end - start <= timedelta(days=90), 'age claim exceeds 90 days')
            birthday = date.fromisoformat(value['next_birthday_at'])
            verified = start.astimezone(SEOUL).date()
            require(verified < birthday <= verified + timedelta(days=366), 'invalid next birthday')
        if name == 'social.HandleList':
            require({row['platform'] for row in value['items']} == {'INSTAGRAM', 'X'},
                    'incomplete handle platform states')
        return value

    def _definition(self, name):
        group, definition = name.split('.')
        return next(doc['$defs'][definition] for doc in self.documents
                    if doc['$id'].endswith('/domain/v1/' + group + '.schema.json'))

    def _scalar_schema(self, schema):
        # The target query contract intentionally supports scalars only.
        seen = set()
        resolver = self.registry.resolver()
        while '$ref' in schema:
            require(set(schema) == {'$ref'}, 'ambiguous scalar reference')
            require(schema['$ref'] not in seen and len(seen) < 16, 'cyclic scalar reference')
            seen.add(schema['$ref'])
            resolved = resolver.lookup(schema['$ref'])
            schema, resolver = resolved.contents, resolved.resolver
        require(schema.get('type') in ('string', 'integer', 'boolean') or
                isinstance(schema.get('enum'), list) and
                all(isinstance(value, str) for value in schema['enum']), 'non-scalar query schema')
        return schema

    def validate_target(self, operation_id, path, raw_query='', *, event_id=None):
        """Offline target parsing oracle, not an HTTP router or authorization decision."""
        matches = [op for op in self.operations if op['operationId'] == operation_id]
        require(len(matches) == 1, 'unknown HTTP operation')
        op = matches[0]
        require(isinstance(path, str), 'path required')
        pattern = re.sub(r'\\\{\w+\\\}', r'([^/]+)', re.escape(op['path']))
        match = re.fullmatch(pattern, path)
        require(match is not None, 'route path mismatch')
        parameters = dict(zip(re.findall(r'\{(\w+)\}', op['path']), match.groups()))
        for name, value in parameters.items():
            self.validate(value, 'common.Platform' if name == 'platform' else 'common.Uuid')
        if event_id is not None:
            self.validate(event_id, 'common.Uuid')
            require(parameters['eventId'] == event_id, 'request event mismatch')
        policy = self.policy['request_targets']
        require(isinstance(raw_query, str) and raw_query.isascii() and
                len(raw_query) <= policy['query_max_encoded_bytes'], 'invalid query encoding or length')
        require(re.search(r'%(?![0-9a-fA-F]{2})', raw_query) is None and '#' not in raw_query and
                not any(ord(char) <= 32 or ord(char) == 127 for char in raw_query),
                'malformed query encoding')
        try:
            pairs = parse_qsl(raw_query, keep_blank_values=True, strict_parsing=True,
                              encoding='utf-8', errors='strict',
                              max_num_fields=policy['query_max_fields'])
        except (ValueError, UnicodeError):
            raise ContractError('malformed query') from None
        schema = self._definition(op['query_schema'])
        require(schema.get('type') == 'object' and schema.get('additionalProperties') is False,
                'query must be closed')
        query = {}
        for name, value in pairs:
            require(name not in query and name in schema['properties'], 'duplicate or unknown query parameter')
            require(not any(ord(char) < 32 or ord(char) == 127 for char in value), 'query contains control')
            scalar = self._scalar_schema(schema['properties'][name])
            kind = scalar.get('type', 'string')
            if kind == 'integer':
                require(re.fullmatch(r'0|[1-9][0-9]{0,15}', value) is not None, 'noncanonical query integer')
                value = int(value)
            elif kind == 'boolean':
                require(value in ('true', 'false'), 'noncanonical query boolean')
                value = value == 'true'
            query[name] = value
        self.validate(query, op['query_schema'])
        if 'operation_id' in query:
            require(any(row['operationId'] == query['operation_id'] and row['method'] != 'get'
                        for row in self.operations), 'query references unknown command')
        return {'path': parameters, 'query': query}

    def _consumer_read(self, value, operation_id, actor_id, event_id, target):
        """Six discovery/detail/own-activity predicates; schema validity is not authorization."""
        def rows(page, key, owner=None, public_gift=False):
            distinct(page['items'], key)
            require(len(page['items']) <= target['query'].get('limit', 20), 'response exceeds requested page size')
            for row in page['items']:
                require(row['event_id'] == event_id, 'row event mismatch')
                if owner:
                    identity = row[owner]
                    if isinstance(identity, dict):
                        identity = identity['user_id']
                    require(identity == actor_id, 'own activity actor mismatch')
                if public_gift:
                    require(row['state'] != 'DRAFT', 'public draft disclosure')

        if operation_id == 'listConcertParties':
            rows(value, 'party_id')
        elif operation_id == 'listConcertGiveaways':
            rows(value, 'giveaway_id', public_gift=True)
        elif operation_id in ('getConcertParty', 'getConcertGiveaway'):
            gift = operation_id == 'getConcertGiveaway'
            require(value['giveaway_id' if gift else 'party_id'] == target['path']['giveawayId' if gift else 'partyId'],
                    'detail target mismatch')
            if gift:
                require(value['state'] != 'DRAFT', 'public draft disclosure')
        elif operation_id == 'getConcertPartyActivity':
            rows(value['owned'], 'party_id', 'owner')
            rows(value['memberships'], 'party_id', 'user_id')
            rows(value['applications'], 'application_id', 'applicant')
        elif operation_id == 'getConcertGiftActivity':
            rows(value['giving'], 'giveaway_id', 'owner')
            rows(value['receiving'], 'reservation_id', 'recipient_id')

    def validate_http(self, value, operation_id, status, *, actor_id=None, event_id=None,
                      request_path=None, request_query='', idempotency_key=None, expected_resource_id=None,
                      expected_resource_revision=None, expected_receipt_id=None, original_command=None):
        require(type(status) is int, 'HTTP status required')
        matches = [op for op in self.operations if op['operationId'] == operation_id]
        require(len(matches) == 1, 'unknown HTTP operation')
        op = matches[0]
        if status in (op['success_status'], op['pending_status']):
            name = op['response'] if status == op['success_status'] else op['pending_response']
            require(name != 'BINARY', 'binary response needs media qualification')
            self.validate(value, name)
            reads = {'listConcertParties', 'getConcertParty', 'getConcertPartyActivity',
                     'listConcertGiveaways', 'getConcertGiveaway', 'getConcertGiftActivity'}
            target = None
            if operation_id in {'getConcertMuralOwnResult', 'getConcertNotificationUnreadCount'}:
                self.validate(actor_id, 'common.Uuid')
                self.validate(event_id, 'common.Uuid')
                require(request_path is not None, 'captured request target required')
                target = self.validate_target(operation_id, request_path, request_query, event_id=event_id)
                if operation_id == 'getConcertMuralOwnResult':
                    self.validate(idempotency_key, 'common.Uuid')
                    require(target['path']['key'] == idempotency_key, 'foreign original command key')
                    self.validate(expected_resource_id, 'common.Uuid')
                    self.validate(expected_resource_revision, 'common.Revision')
                    self.validate(expected_receipt_id, 'common.Uuid')
                    self.validate(original_command, 'mural.PlacePixel')
                    require(expected_resource_id == event_id and
                            value['receipt_id'] == expected_receipt_id and
                            value['idempotency_key'] == idempotency_key and
                            value['result']['event_id'] == event_id and
                            value['result']['revision'] == expected_resource_revision,
                            'original result does not match verified receipt')
                    require(value['result']['revision'] == original_command['expected_revision'] + 1 and
                            all(value['result'][k] == original_command[k] for k in ('x', 'y', 'color_index')),
                            'original pixel command mismatch')
                else:
                    require(value['actor_id'] == actor_id, 'foreign unread count')
            if operation_id in reads or name == 'common.CommandReceipt':
                self.validate(actor_id, 'common.Uuid')
                self.validate(event_id, 'common.Uuid')
                if operation_id in reads or operation_id == 'getConcertCommandStatus':
                    require(request_path is not None, 'captured request target required')
                    target = self.validate_target(operation_id, request_path, request_query, event_id=event_id)
            if isinstance(value, dict) and 'event_id' in value and event_id is not None:
                require(value['event_id'] == event_id, 'response event mismatch')
            if operation_id in reads:
                self._consumer_read(value, operation_id, actor_id, event_id, target)
            if name == 'common.CommandReceipt':
                if operation_id == 'getConcertCommandStatus':
                    command_id = target['query']['operation_id']
                    key = target['query']['idempotency_key']
                else:
                    command_id, key = operation_id, idempotency_key
                self.validate(key, 'common.Uuid')
                require(value['operation_id'] == command_id, 'receipt operation mismatch')
                require(value['idempotency_key'] == key, 'receipt key mismatch')
                require(actor_id is not None and value['actor_id'] == actor_id, 'receipt actor mismatch')
                commands = [row for row in self.operations if row['operationId'] == command_id and row['method'] != 'get']
                require(len(commands) == 1, 'receipt references unknown command')
                command = commands[0]
                if value['state'] == 'SUCCEEDED':
                    require(value['result_status'] == command['success_status'], 'receipt final status mismatch')
                    if expected_resource_id is not None:
                        self.validate(expected_resource_id, 'common.Uuid')
                        require(value['resource_id'] == expected_resource_id, 'receipt resource mismatch')
                if value['state'] == 'FAILED':
                    require(value['failure_reason'] in {reason for reasons in command['errors'].values() for reason in reasons},
                            'receipt failure reason not registered for command')
                if status == 202:
                    require(value['state'] == 'PENDING', '202 must remain pending')
            return
        require(str(status) in op['errors'], 'undocumented response status')
        self.validate(value, 'common.DomainError')
        require(value['details']['reason'] in op['errors'][str(status)], 'error/status mismatch')

    def transition(self, aggregate, before, after):
        edges = self.policy['transitions'].get(aggregate, {})
        require(before in edges and after in edges[before], 'invalid aggregate transition')

    def current_age(self, claims, user_id, now):
        """Select the canonical row first. Never fall back to a superseded valid row."""
        instant = timestamp(now)
        owned = [row for row in claims if row.get('user_id') == user_id]
        if not owned:
            return None
        for row in owned:
            self.validate(row, 'social.PrivateAgeClaim')
        claim = max(owned, key=lambda row: (timestamp(row['verified_at']),
                                           timestamp(row['expires_at']), row['claim_id']))
        if claim['status'] != 'VERIFIED' or not timestamp(claim['verified_at']) <= instant < timestamp(claim['expires_at']):
            return None
        age = claim['age_at_verification'] + int(instant.astimezone(SEOUL).date() >=
                                                date.fromisoformat(claim['next_birthday_at']))
        return age if 14 <= age <= 100 else None

    def age_allowed(self, claims, user_id, now, bounds):
        self.validate(bounds, 'common.AgeBounds')
        if bounds['minimum_age'] is None:
            return True
        age = self.current_age(claims, user_id, now)
        return age is not None and bounds['minimum_age'] <= age < bounds['maximum_age_exclusive']

    def reservation_context(self, reservation, giveaway, now):
        self.validate(reservation, 'giveaway.Reservation')
        self.validate(giveaway, 'giveaway.GiveawayPublic')
        require(reservation['giveaway_id'] == giveaway['giveaway_id'] and
                reservation['event_id'] == giveaway['event_id'], 'reservation context mismatch')
        start = timestamp(giveaway['pickup_starts_at'])
        expiry = timestamp(giveaway['pickup_ends_at']) + timedelta(seconds=giveaway['pickup_grace_seconds'])
        instant = timestamp(now)
        require(timestamp(reservation['expires_at']) == expiry, 'arbitrary reservation expiry')
        if reservation['state'] in ('HELD', 'READY'):
            require(instant < expiry, 'active reservation already expired')
            require(reservation['state'] == ('HELD' if instant < start else 'READY'),
                    'reservation READY transition missing')
        if reservation['state'] == 'EXPIRED':
            require(instant >= expiry, 'premature expiry')

    def authorize_reveal(self, request, grants, handles, *, actor_id, event_id, platform,
                         now, eligible_users):
        """Fixture oracle only; eligible_users represents freshly evaluated server policy."""
        self.validate(request, 'social.SocialRequest')
        instant = timestamp(now)
        require(request['event_id'] == event_id, 'reveal event mismatch')
        pair = {request['requester_id'], request['recipient_id']}
        require(actor_id in pair and pair <= set(eligible_users), 'reveal policy denied')
        require(request['state'] == 'APPROVED' and timestamp(request['created_at']) <= instant <
                timestamp(request['expires_at']),
                'request not active')
        require(platform in request['approved_platforms'], 'platform not approved')
        binding = next(c for c in request['consents'] if c['platform'] == platform)
        versions = {request['requester_id']: binding['requester_handle_version'],
                    request['recipient_id']: binding['recipient_handle_version']}
        matching = [g for g in grants if g.get('request_id') == request['request_id'] and
                    g.get('platform') == platform]
        require(len(matching) == 2, 'reciprocal grants missing or duplicated')
        require({g['owner_id'] for g in matching} == pair, 'reciprocal owner mismatch')
        for grant in matching:
            self.validate(grant, 'social.SocialGrant')
            require({grant['owner_id'], grant['viewer_id']} == pair and
                    grant['event_id'] == event_id, 'grant identity mismatch')
            require(grant['state'] == 'ACTIVE' and instant < timestamp(grant['expires_at']) <=
                    timestamp(request['expires_at']), 'grant expired or revoked')
            require(grant['handle_version'] == versions[grant['owner_id']] and
                    grant['peer_handle_version'] == versions[grant['viewer_id']], 'grant version mismatch')
        current = [h for h in handles if h.get('user_id') in pair and h.get('platform') == platform]
        require(len(current) == 2 and {h['user_id'] for h in current} == pair,
                'current handles missing or duplicated')
        for handle in current:
            require(handle.get('configured') is True and handle['version'] == versions[handle['user_id']],
                    'handle replaced or removed')
            validate_handle(platform, handle['handle'])
        peer = (pair - {actor_id}).pop()
        return next(h['handle'] for h in current if h['user_id'] == peer)


def validate_handle(platform, handle):
    require(platform in ('INSTAGRAM', 'X') and isinstance(handle, str), 'unsupported handle')
    pattern = r'[A-Za-z0-9_]{1,15}' if platform == 'X' else r'[A-Za-z0-9._]{1,30}'
    require(re.fullmatch(pattern, handle) is not None, 'invalid handle')
    if platform == 'INSTAGRAM':
        require(not handle.startswith('.') and not handle.endswith('.') and '..' not in handle,
                'invalid Instagram dots')


def derive_age(birthyear, birthday, verified_on):
    """Adapted source semantics; provider proof/identity are deliberately outside this oracle."""
    require(isinstance(birthyear, str) and re.fullmatch(r'[0-9]{4}', birthyear), 'invalid birthyear')
    require(isinstance(birthday, str) and re.fullmatch(r'[0-9]{2}-[0-9]{2}', birthday), 'invalid birthday')
    try:
        born = date.fromisoformat(birthyear + '-' + birthday)
        verified = date.fromisoformat(verified_on)
    except ValueError:
        raise ContractError('impossible birth date') from None
    require(born <= verified, 'future birth date')

    def occurrence(year):
        try:
            return date(year, born.month, born.day)
        except ValueError:
            require(born.month == 2 and born.day == 29, 'impossible birthday')
            return date(year, 2, 28)

    on = occurrence(verified.year)
    age = verified.year - born.year - int(verified < on)
    require(14 <= age <= 100, 'unsupported age')
    next_day = on if on > verified else occurrence(verified.year + 1)
    return {'age_at_verification': age, 'next_birthday_at': next_day.isoformat()}


def validate_fixtures(contract):
    rows = read(FIXTURES / 'index.json')
    seen = set()
    for row in rows:
        relative = Path(row['file'])
        require(len(relative.parts) == 1 and relative.suffix == '.json', 'unsafe fixture path')
        require(row['file'] not in seen, 'duplicate fixture')
        seen.add(row['file'])
        value = read(FIXTURES / relative)
        rejected = False
        try:
            contract.validate(value, row['schema'])
        except ContractError:
            rejected = True
        require(rejected == (row['expected'] == 'reject'), 'fixture expectation mismatch: ' + row['file'])
    require(seen == {p.name for p in FIXTURES.glob('*.json')} - {'index.json'}, 'fixture index drift')
    return len(seen)


def main():
    try:
        contract = Contract()
        contract.check_inherited()
        count = validate_fixtures(contract)
        print(json.dumps({'state': contract.policy['state'], 'schemas': len(contract.validators),
                          'operations': len(contract.operations), 'fixtures': count,
                          'runtime': 'NOT_IMPLEMENTED', 'freeze': 'NOT_ACCEPTED'}))
        return 0
    except (ValueError, OSError) as error:
        print('Domain contract validation failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
