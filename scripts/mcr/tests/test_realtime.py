import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('mcr_realtime', ROOT / 'scripts/mcr/validate-realtime.py')
rt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rt)


class RealtimeContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = rt.Contract()

    def fixture(self, name):
        return rt.read(rt.FIXTURES / (name + '.json'))

    def started(self):
        fixture = self.fixture('movement-heartbeat')
        trace = rt.Trace(self.contract, fixture['actor_id'])
        for row in fixture['messages'][:2]:
            trace.consume(row['direction'], row['message'])
        return trace

    def reject(self, fn, code=None):
        with self.assertRaises(rt.WireError) as raised:
            fn()
        if code is not None:
            self.assertEqual(raised.exception.code, code)

    def test_all_indexed_fixtures(self):
        self.assertEqual(rt.validate_fixtures(), 24)

    def test_limits_measure_utf8_bytes_before_json_decode(self):
        for direction, fixture_index, limit in [('client', 0, 4096), ('server', 1, 65536)]:
            value = self.fixture('movement-heartbeat')['messages'][fixture_index]['message']
            if direction == 'server':
                value['players'][0]['displayName'] = '\u00e9' * 40
            data = json.dumps(value, ensure_ascii=False).encode()
            padded = data + b' ' * (limit - len(data))
            self.contract.decode(padded, direction)
            self.reject(lambda: self.contract.decode(padded + b' ', direction), 1009)
        self.reject(lambda: self.contract.decode(b'\xff' * 4097, 'client'), 1009)

    def test_invalid_utf8_and_surrogates(self):
        self.reject(lambda: self.contract.decode(b'\xff', 'client'), 1007)
        value = self.fixture('movement-heartbeat')['messages'][1]['message']
        value['players'][0]['displayName'] = '\ud800'
        self.reject(lambda: self.contract.decode(json.dumps(value), 'server'), 1007)

    def test_duplicate_keys_nonfinite_overflow_and_deep_json(self):
        for payload in ['{"type":"input","type":"input"}', '{"axisX":NaN}',
                        '{"axisX":Infinity}', '{"axisX":1e999}', '[' * 20 + '0' + ']' * 20]:
            with self.subTest(payload=payload):
                self.reject(lambda: self.contract.decode(payload, 'client'), 4400)

    def test_unknown_fields_version_and_client_authoritative_positions(self):
        original = self.fixture('movement-heartbeat')['messages'][2]['message']
        for patch in [{'x': 100}, {'position': {'x': 100, 'y': 200}},
                      {'protocolVersion': '2.0.0'}, {'seq': 9007199254740992},
                      {'seq': True}, {'axisX': 1, 'axisY': 1}]:
            self.reject(lambda: self.contract.decode(json.dumps(original | patch), 'client'))

    def test_uuid_is_canonical(self):
        value = self.fixture('movement-heartbeat')['messages'][0]['message']
        value['eventId'] = 'ABCDEF00-0000-4000-8000-000000000000'
        self.reject(lambda: self.contract.decode(json.dumps(value), 'client'))

    def test_all_schema_refs_are_offline_and_resolve(self):
        resources = self.contract.validators['Snapshot']._registry
        for schema in [self.contract.schema, rt.read(ROOT / 'contracts/core/v1/core.schema.json')]:
            def visit(node, base):
                if isinstance(node, dict):
                    if '$ref' in node:
                        resources.resolver(base).lookup(node['$ref'])
                    for value in node.values():
                        visit(value, base)
                elif isinstance(node, list):
                    for value in node:
                        visit(value, base)
            visit(schema, schema['$id'])
        with self.assertRaises(Exception):
            resources.get_or_retrieve('https://example.invalid/not-a-contract')

    def test_initial_auth_and_snapshot_order(self):
        fixture = self.fixture('movement-heartbeat')
        trace = rt.Trace(self.contract, fixture['actor_id'])
        self.reject(lambda: trace.consume('client', fixture['messages'][2]['message']), 4401)
        trace.consume('client', fixture['messages'][0]['message'])
        self.reject(lambda: trace.consume('server', fixture['messages'][3]['message']))
        self.reject(lambda: trace.consume('client', fixture['messages'][0]['message']))

    def test_initial_snapshot_matches_ticket_scope_and_identity(self):
        fixture = self.fixture('movement-heartbeat')
        for key, value in [('selfUserId', '00000000-0000-4000-8000-000000000002'),
                           ('roomId', '00000000-0000-4000-8000-000000000012'),
                           ('mapRevision', 2), ('avatarManifestVersion', 2),
                           ('ackInputSeq', 1), ('cause', 'EPOCH_CHANGED')]:
            trace = rt.Trace(self.contract, fixture['actor_id'])
            trace.consume('client', fixture['messages'][0]['message'])
            self.reject(lambda: trace.consume('server', fixture['messages'][1]['message'] | {key: value}))
            self.assertIsNone(trace.snapshot)

    def test_input_acknowledgements_cannot_lead_or_regress(self):
        fixture = self.fixture('movement-heartbeat')
        trace = self.started()
        delta = fixture['messages'][3]['message']
        self.reject(lambda: trace.consume('server', delta))
        trace.consume('client', fixture['messages'][2]['message'])
        trace.consume('server', delta)
        self.reject(lambda: trace.consume('server', delta | {'streamSeq': 2, 'ackInputSeq': 0}))
        self.reject(lambda: trace.consume('client', fixture['messages'][2]['message']))

    def test_sequence_gaps_duplicates_epoch_and_tick_regression(self):
        trace = self.started()
        delta = self.fixture('movement-heartbeat')['messages'][3]['message'] | {'ackInputSeq': 0}
        for patch in [{'streamSeq': 2}, {'roomEpoch': '00000000-0000-4000-8000-000000000099'},
                      {'streamId': '00000000-0000-4000-8000-000000000099'}]:
            self.reject(lambda: trace.consume('server', delta | patch))
        trace.consume('server', delta)
        self.reject(lambda: trace.consume('server', delta))
        self.reject(lambda: trace.consume('server', delta | {'streamSeq': 2, 'serverTick': 0}))

    def test_selected_static_player_uses_union_without_changing_protocol(self):
        value = self.fixture('movement-heartbeat')['messages'][1]['message']
        directory = ROOT / 'contracts/fixtures/assets/selected-static'
        value['avatarManifestId'] = 'mcr_selected_static_20261103'
        for i, player in enumerate(value['players']):
            player['avatar']['config'] = rt.read(directory / f'photo01_front_{i % 6 + 1:02}.json')
        self.contract.decode(json.dumps(value), 'server')
        value['players'][0]['avatar']['config']['item_ids'] = {}
        self.reject(lambda: self.contract.decode(json.dumps(value), 'server'))

    def test_avatar_mismatch_duplicate_players_and_private_fields(self):
        delta = self.fixture('movement-heartbeat')['messages'][3]['message'] | {'ackInputSeq': 0}
        for mutation in ['manifest', 'duplicate', 'private']:
            bad = copy.deepcopy(delta)
            if mutation == 'manifest':
                bad['upserts'][0]['avatar']['config']['manifest_version'] = 2
            elif mutation == 'duplicate':
                bad['upserts'] *= 2
            else:
                bad['upserts'][0]['socialHandles'] = {'x': 'hidden'}
            self.reject(lambda: self.started().consume('server', bad))

    def test_removals_are_atomic_known_nonself_and_disjoint(self):
        trace = self.started()
        delta = self.fixture('movement-heartbeat')['messages'][3]['message'] | {'ackInputSeq': 0}
        before = copy.deepcopy(trace.players)
        for removes in [[trace.actor], [next(uid for uid in trace.players if uid != trace.actor),
                                      '00000000-0000-4000-8000-000000000099']]:
            self.reject(lambda: trace.consume('server', delta | {'removes': removes}))
            self.assertEqual(trace.players, before)
            self.assertEqual(trace.sequence, 0)

    def test_resync_requires_correlation_new_stream_and_full_replacement(self):
        fixture = self.fixture('resync')
        trace = self.started()
        for row in fixture['messages'][2:4]:
            trace.consume(row['direction'], row['message'])
        replacement = fixture['messages'][4]['message']
        self.reject(lambda: trace.consume('server', replacement | {'requestId': None}))
        self.reject(lambda: trace.consume('server', replacement | {'streamId': trace.snapshot['streamId']}))
        self.reject(lambda: trace.consume('client', self.fixture('movement-heartbeat')['messages'][2]['message']))
        trace.consume('server', replacement)
        self.assertEqual(set(trace.players), {trace.actor})
        self.assertEqual(trace.input_sequence, 0)

    def test_switch_failure_preserves_baseline_success_replaces_it(self):
        fixture = self.fixture('room-switch')
        trace = self.started()
        before = copy.deepcopy(trace.snapshot)
        for row in fixture['messages'][2:4]:
            trace.consume(row['direction'], row['message'])
        self.assertEqual(trace.snapshot, before)
        for row in fixture['messages'][4:]:
            trace.consume(row['direction'], row['message'])
        self.assertNotEqual(trace.snapshot['roomId'], before['roomId'])
        self.assertEqual(set(trace.players), {trace.actor})

    def test_switch_cannot_change_event_or_reuse_request_id(self):
        fixture = self.fixture('room-switch')
        trace = self.started()
        switch = fixture['messages'][2]['message']
        self.reject(lambda: trace.consume('client', switch | {'eventId': '00000000-0000-4000-8000-000000000099'}))
        trace.consume('client', switch)
        trace.consume('server', fixture['messages'][3]['message'])
        self.reject(lambda: trace.consume('client', switch))

    def test_room_switch_cannot_change_map_or_avatar_manifest(self):
        switch = self.fixture('room-switch')['messages'][2]['message']
        for patch in [{'mapId': 'different_map'}, {'mapRevision': 2},
                      {'avatarManifestId': 'different_avatar'}, {'avatarManifestVersion': 2}]:
            self.reject(lambda: self.started().consume('client', switch | patch))

    def test_queued_old_stream_delta_is_discarded_while_waiting_for_resync(self):
        trace = self.started()
        fixture = self.fixture('resync')
        trace.consume('client', fixture['messages'][3]['message'])
        before = copy.deepcopy(trace.players)
        delta = self.fixture('movement-heartbeat')['messages'][3]['message'] | {'ackInputSeq': 0}
        trace.consume('server', delta)
        self.assertEqual(trace.players, before)
        self.assertEqual(trace.sequence, 0)
        trace.consume('server', fixture['messages'][4]['message'])
        self.assertEqual(set(trace.players), {trace.actor})

    def test_snapshot_metadata_does_not_retain_removed_roster_or_ticket(self):
        trace = self.started()
        self.assertNotIn('ticket', trace.authenticate)
        self.assertNotIn('players', trace.snapshot)
        delta = self.fixture('movement-heartbeat')['messages'][3]['message']
        removed = next(uid for uid in trace.players if uid != trace.actor)
        trace.consume('server', delta | {'ackInputSeq': 0, 'upserts': [], 'removes': [removed]})
        self.assertNotIn(removed, trace.players)

    def test_all_replacement_causes_preserve_the_connection_access_deadline(self):
        for name in ('resync', 'room-switch', 'epoch-replacement'):
            trace = self.started()
            fixture = self.fixture(name)
            for row in fixture['messages'][2:-1]:
                trace.consume(row['direction'], row['message'])
            value = fixture['messages'][-1]['message']
            for deadline in ('2099-11-07T12:00:00Z', '2025-11-07T12:00:00Z'):
                self.reject(lambda: trace.consume('server', value | {'accessExpiresAt': deadline}))
            trace.consume('server', value)

    def test_imported_avatar_revision_is_bounded_without_duplicating_its_schema(self):
        snapshot = self.fixture('movement-heartbeat')['messages'][1]['message']
        for revision in (9007199254740992, 10 ** 1000):
            snapshot['players'][0]['avatar']['revision'] = revision
            self.reject(lambda: self.contract.decode(json.dumps(snapshot), 'server'))
        self.reject(lambda: self.contract.decode('{"revision":' + '9' * 5000 + '}', 'server'))

    def test_asset_mismatch_and_backpressure_require_a_new_connection(self):
        for reason in ('MAP_VERSION_MISMATCH', 'BACKPRESSURE'):
            trace = self.started()
            value = self.fixture('resync')['messages'][2]['message'] | {'reason': reason}
            trace.consume('server', value)
            self.assertTrue(trace.closed)
            self.assertFalse(trace.players)
            self.assertIsNone(trace.snapshot)
            self.reject(lambda: trace.consume('client', self.fixture('resync')['messages'][3]['message']))
            self.reject(lambda: trace.consume('server', self.fixture('resync')['messages'][4]['message']))

    def test_epoch_replacement_requires_new_epoch(self):
        trace = self.started()
        value = self.fixture('epoch-replacement')['messages'][2]['message']
        self.reject(lambda: trace.consume('server', value | {'roomEpoch': trace.snapshot['roomEpoch']}))
        trace.consume('server', value)
        self.assertEqual(trace.sequence, 0)

    def test_revoke_clears_state_and_rejects_all_followups(self):
        trace = self.started()
        trace.consume('server', self.fixture('revoke')['messages'][2]['message'])
        self.assertFalse(trace.players)
        self.assertIsNone(trace.snapshot)
        self.assertIsNone(trace.authenticate)
        self.reject(lambda: trace.consume('server', self.fixture('movement-heartbeat')['messages'][4]['message']))
        self.reject(lambda: trace.consume('client', self.fixture('movement-heartbeat')['messages'][0]['message']))

    def test_heartbeat_is_correlated_once_and_does_not_advance_stream(self):
        fixture = self.fixture('movement-heartbeat')
        trace = self.started()
        ping, pong = [r['message'] for r in fixture['messages'][4:6]]
        self.reject(lambda: trace.consume('client', pong))
        trace.consume('server', ping)
        self.reject(lambda: trace.consume('server', ping))
        trace.consume('client', pong)
        self.reject(lambda: trace.consume('client', pong))
        self.assertEqual(trace.sequence, 0)

    def test_filtered_recipients_have_independent_gapless_sequences(self):
        fixture = self.fixture('filtered-viewers')
        result = rt.filtered_projection(fixture, self.contract)
        for deltas in result.values():
            self.assertEqual([d['streamSeq'] for d in deltas], list(range(1, len(deltas) + 1)))
        second = list(result.values())[1]
        self.assertEqual([d['serverTick'] for d in second], [2, 6])
        fixture['expected'][list(result)[1]][1]['streamSeq'] = 3
        self.reject(lambda: rt.filtered_projection(fixture, self.contract))

    def test_ticket_ttl_and_catalog_consistency(self):
        ticket = self.fixture('ticket-response')
        for expires in [ticket['server_time'], '2026-11-07T10:00:31Z']:
            self.reject(lambda: self.contract.validate(ticket | {'expires_at': expires}, 'WorldTicketResponse'))
        rooms = self.fixture('rooms')
        self.reject(lambda: self.contract.validate(rooms | {'items': rooms['items'] * 2}, 'WorldRooms'))
        self.reject(lambda: self.contract.validate(rooms | {'items': rooms['items'][1:]}, 'WorldRooms'))

    def test_http_response_cannot_cross_event_room_or_asset_version(self):
        response, request = self.fixture('ticket-response'), self.fixture('ticket-request')
        for field, value in [('event_id', '00000000-0000-4000-8000-000000000099'),
                             ('room_id', '00000000-0000-4000-8000-000000000099'),
                             ('map_revision', 2), ('avatar_manifest_version', 2)]:
            self.reject(lambda: self.contract.validate_http(response | {field: value},
                        'issueWorldTicket', 200, event_id=response['event_id'], request=request))

    def test_http_error_operation_and_status_are_bound(self):
        conflict = self.fixture('map-conflict')
        self.contract.validate_http(conflict, 'issueWorldTicket', 409)
        self.reject(lambda: self.contract.validate_http(conflict, 'listConcertRooms', 409))
        self.reject(lambda: self.contract.validate_http(conflict, 'issueWorldTicket', 403))
        self.reject(lambda: self.contract.validate_http(conflict, 'issueWorldTicket', 418))
        self.reject(lambda: self.contract.validate_http(conflict | {'debug': 'private'}, 'issueWorldTicket', 409))

    def test_http_registry_has_unique_operations_frozen_guards_and_no_runtime_claim(self):
        operations = self.contract.operations
        self.assertEqual(len({op['operationId'] for op in operations}), len(operations))
        self.assertEqual(len({(op['method'], op['path']) for op in operations}), len(operations))
        for op in operations:
            self.assertEqual(op['cache'], 'NO_STORE')
            self.assertEqual(op['runtime_status'], 'NOT_IMPLEMENTED')
            self.assertTrue(op['authenticated'])
            if op['method'] == 'post':
                self.assertEqual(op['retry'], 'NO_AUTOMATIC_REPLAY')
                self.assertEqual(op['origin_policy'], 'EXACT_ALLOWLIST_AND_X_MCR_INTENT_BROWSER')

    def test_close_policy_follows_reserved_ranges_and_utf8_budget(self):
        policy = self.contract.policy
        self.assertEqual(policy['close_reason_max_utf8_bytes'], 123)
        self.assertTrue(all(c in (1000, 1003, 1007, 1009) or 4000 <= c <= 4999
                            for c in policy['close_codes'].values()))
        self.assertLessEqual(policy['auth_lease_max_ms'], 5000)
        self.assertEqual(policy['capacity_qualification'], 'NOT_RUN')


if __name__ == '__main__':
    unittest.main()
