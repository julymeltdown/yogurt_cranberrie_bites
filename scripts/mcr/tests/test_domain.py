"""Adversarial C04 examples; these do not certify service authorization or SQL races."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest
from urllib.parse import urlencode

PATH = Path(__file__).resolve().parents[1] / 'validate-domain.py'
SPEC = importlib.util.spec_from_file_location('mcr_domain_oracle', PATH)
domain = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(domain)


def fixture(name):
    return domain.read(domain.FIXTURES / (name + '.json'))


def uid(number):
    return f'00000000-0000-4000-8000-{number:012x}'


class DomainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = domain.Contract()

    def rejected(self, action):
        with self.assertRaises(domain.ContractError):
            action()

    def test_golden_and_negative_fixture_index(self):
        self.assertEqual(domain.validate_fixtures(self.contract), 35)

    def test_all_refs_and_exact_inherited_paths_resolve(self):
        self.contract.check_inherited()
        self.assertGreaterEqual(len(self.contract.validators), 140)

    def test_error_status_drift_fails(self):
        altered = copy.deepcopy(self.contract)
        row = next(op for op in altered.operations if op['operationId'] == 'reserveConcertGiveaway')
        row['errors']['400'].append('GIVEAWAY_SOLD_OUT')
        self.rejected(altered.check_register)

    def test_duplicate_routes_fail(self):
        altered = copy.deepcopy(self.contract)
        altered.operations.append(copy.deepcopy(altered.operations[0]))
        self.rejected(altered.check_register)

    def target(self, operation, query='', event=None):
        row = next(op for op in self.contract.operations if op['operationId'] == operation)
        path = row['path']
        for name in row['path_parameters']:
            path = path.replace('{' + name + '}', 'X' if name == 'platform' else event or uid(1))
        return self.contract.validate_target(operation, path, query, event_id=event or uid(1))

    def test_all_operation_targets_have_typed_closed_paths_and_queries(self):
        required = {'expected_revision': 1, 'party_revision': 2, 'after_revision': 1,
                    'platform': 'X', 'operation_id': 'reserveConcertGiveaway',
                    'idempotency_key': uid(9)}
        for row in self.contract.operations:
            with self.subTest(operation=row['operationId']):
                fields = self.contract._definition(row['query_schema'])['required']
                query = urlencode({name: required[name] for name in fields})
                result = self.target(row['operationId'], query)
                self.assertEqual(result['path']['eventId'], uid(1))
                self.rejected(lambda: self.target(row['operationId'], query + ('&' if query else '') + 'token=hidden'))

    def test_missing_unknown_or_wrong_path_parameter_metadata_fails(self):
        for mutate in [lambda row: row.pop('path_parameters'),
                       lambda row: row['path_parameters'].update(other={'type': 'string'}),
                       lambda row: row['path_parameters'].update(eventId={'type': 'string'}),
                       lambda row: row.update(query_schema='not.registered'),
                       lambda row: row.update(request_location='body')]:
            altered = copy.deepcopy(self.contract)
            mutate(altered.operations[0])
            self.rejected(altered.check_register)

    def test_path_identifiers_are_canonical_and_event_bound(self):
        path = '/api/concert/events/' + uid(1) + '/giveaways/'
        for value in [uid(10).upper(), '%30' + uid(2)[1:], '..', 'bad', uid(2) + '/', uid(2) + '?q=x']:
            self.rejected(lambda: self.contract.validate_target('getConcertGiveaway', path + value, event_id=uid(1)))
        self.rejected(lambda: self.contract.validate_target('getConcertGiveaway', path + uid(2), event_id=uid(3)))
        self.rejected(lambda: self.contract.validate_target('removeConcertHandle',
            '/api/concert/events/' + uid(1) + '/me/social-handles/x', 'expected_revision=1'))

    def test_query_scalars_are_bounded_without_coercion(self):
        self.assertEqual(self.target('listConcertGiveaways', 'limit=100&in_stock=false')['query'],
                         {'limit': 100, 'in_stock': False})
        for query in ['limit=0', 'limit=101', 'limit=01', 'limit=+1', 'limit=1.0',
                      'limit=1e2', 'limit=-1', 'limit=9007199254740992', 'limit=null',
                      'limit=1&limit=2', 'limit=1&%6cimit=2', 'in_stock=1',
                      'in_stock=False', 'cursor=', 'cursor=null=', 'cursor=abc%2Fdef',
                      'cursor=' + 'A' * 1025, 'state=DRAFT']:
            with self.subTest(query=query[:80]):
                self.rejected(lambda: self.target('listConcertGiveaways', query))

    def test_query_utf8_is_decoded_once_and_malformed_input_rejected(self):
        self.assertEqual(self.target('listConcertIntros', 'q=%C3%A9+fan')['query'], {'q': 'é fan'})
        self.assertEqual(self.target('listConcertIntros', 'q=%252F')['query'], {'q': '%2F'})
        for query in ['q=%', 'q=%0', 'q=%GG', 'q=%FF', 'q=%ED%A0%80', 'q=é', 'q=a b',
                      'q=a#b', 'q=%00', 'q=%0a', 'q=%7F', 'q', '=x', 'q=x&',
                      'q=' + 'a' * 8192, '&'.join('q=x' for _ in range(33))]:
            with self.subTest(query=query[:80]):
                self.rejected(lambda: self.target('listConcertIntros', query))

    def test_filter_time_windows_are_nonempty_utc_ranges(self):
        query = urlencode({'pickup_after': '2026-11-03T00:00:00Z', 'pickup_before': '2026-11-03T01:00:00Z'})
        self.target('listConcertGiveaways', query)
        for upper in ['2026-11-03T00:00:00Z', '2026-11-02T00:00:00Z', '2026-11-03T12:00:00+09:00']:
            self.rejected(lambda: self.target('listConcertGiveaways', urlencode({
                'pickup_after': '2026-11-03T00:00:00Z', 'pickup_before': upper})))

    def test_private_activity_collections_have_independent_cursors(self):
        self.target('getConcertPartyActivity', 'owned_cursor=a&membership_cursor=b&application_cursor=c&limit=20')
        self.target('getConcertGiftActivity', 'giving_cursor=a&receiving_cursor=b&limit=20')
        self.rejected(lambda: self.target('getConcertPartyActivity', 'cursor=ambiguous'))
        empty = {'items': [], 'next_cursor': None}
        self.contract.validate({'owned': empty, 'memberships': empty, 'applications': empty}, 'party.PartyActivity')
        self.rejected(lambda: self.contract.validate({'owned': [], 'memberships': [], 'applications': [],
                                                      'next_cursor': None}, 'party.PartyActivity'))

    def test_delete_requires_revision_query_and_body_routes_reject_queries(self):
        self.target('cancelConcertReservation', 'expected_revision=1')
        self.rejected(lambda: self.target('cancelConcertReservation'))
        self.rejected(lambda: self.target('removeConcertMember', 'expected_revision=1'))
        self.target('removeConcertMember', 'expected_revision=1&party_revision=3')
        self.rejected(lambda: self.target('reserveConcertGiveaway', 'expected_revision=1'))

    def test_receipt_and_audit_query_operation_is_a_registered_command(self):
        for operation in ['unknown', 'listConcertParties']:
            query = urlencode({'operation_id': operation, 'idempotency_key': uid(2)})
            self.rejected(lambda: self.target('getConcertCommandStatus', query))
            self.rejected(lambda: self.target('listConcertAudit', urlencode({'operation_id': operation})))

    def test_no_browser_guard_bypass_in_command_register(self):
        for operation in self.contract.operations:
            if operation['method'] == 'get':
                continue
            with self.subTest(operation=operation['operationId']):
                altered = copy.deepcopy(self.contract)
                row = next(x for x in altered.operations if x['operationId'] == operation['operationId'])
                row['errors']['403'].remove('BROWSER_INTENT_REQUIRED')
                self.rejected(altered.check_register)

    def test_unknown_reference_cannot_retrieve_network(self):
        self.rejected(lambda: self.contract._resolve_all(
            {'$ref': 'https://unconfigured.invalid/schema.json'}, 'https://contracts.mcr.invalid/'))

    def test_unsafe_numbers_in_imported_nested_avatar_rejected(self):
        value = fixture('party-full')
        value['owner']['avatar']['revision'] = 2**53
        self.rejected(lambda: self.contract.validate(value, 'party.PartyPublic'))

    def test_nonfinite_and_invalid_unicode_never_reach_schema(self):
        for value in [float('nan'), float('inf'), 10**1000, '\ud800', {'bad': '\udfff'}]:
            with self.subTest(kind=type(value).__name__):
                self.rejected(lambda: domain.inspect(value))

    def test_duplicate_json_keys_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'duplicate.json'
            path.write_text('{"quantity_total": 1, "quantity_total": 9000}')
            self.rejected(lambda: domain.read(path))

    def test_error_matrix_rejects_sold_out_as_403(self):
        error = fixture('sold-out-error')
        self.contract.validate_http(error, 'reserveConcertGiveaway', 409)
        self.rejected(lambda: self.contract.validate_http(error, 'reserveConcertGiveaway', 403))

    def test_receipt_is_scoped_to_operation_actor_event(self):
        receipt = fixture('command-pending')
        self.contract.validate_http(receipt, 'reserveConcertGiveaway', 202,
                                    actor_id=uid(2), event_id=uid(1))
        for operation, actor, event in [('claimConcertReservation', uid(2), uid(1)),
                                       ('reserveConcertGiveaway', uid(3), uid(1)),
                                       ('reserveConcertGiveaway', uid(2), uid(99))]:
            self.rejected(lambda: self.contract.validate_http(receipt, operation, 202,
                                                               actor_id=actor, event_id=event))

    def test_status_query_returns_original_command_metadata(self):
        self.contract.validate_http(fixture('command-pending'), 'getConcertCommandStatus', 200,
                                    actor_id=uid(2), event_id=uid(1))

    def test_202_never_claims_success(self):
        receipt = fixture('command-pending')
        receipt.update(state='SUCCEEDED', result_status=200, resource_id=uid(6), resource_revision=1)
        self.rejected(lambda: self.contract.validate_http(receipt, 'reserveConcertGiveaway', 202,
                                                           actor_id=uid(2), event_id=uid(1)))

    def test_terminal_party_cannot_reopen_or_complete_cancelled(self):
        for before, after in [('CANCELLED', 'OPEN'), ('CANCELLED', 'COMPLETED'), ('COMPLETED', 'OPEN')]:
            self.rejected(lambda: self.contract.transition('party', before, after))
        self.contract.transition('party', 'CLOSED', 'OPEN')

    def test_claimed_stock_never_released_by_cancel_or_expiry(self):
        for after in ('CANCELLED', 'EXPIRED', 'READY'):
            self.rejected(lambda: self.contract.transition('reservation', 'CLAIMED', after))
        self.rejected(lambda: self.contract.transition('reservation', 'HELD', 'CLAIMED'))

    def test_stock_equation_and_foreign_reservation_fail(self):
        for modify in [lambda x: x.update(quantity_claimed=3),
                       lambda x: x['reservations'][0].update(event_id=uid(99)),
                       lambda x: x['reservations'].append(copy.deepcopy(x['reservations'][0]))]:
            value = fixture('giveaway-management'); modify(value)
            self.rejected(lambda: self.contract.validate(value, 'giveaway.GiveawayManagement'))

    def test_pickup_window_not_ten_minute_booking_lifetime(self):
        reservation = fixture('reservation-held')
        giveaway = fixture('giveaway-management')['giveaway']
        self.contract.reservation_context(reservation, giveaway, '2026-11-03T01:01:00Z')
        reservation['expires_at'] = '2026-11-03T01:10:00Z'
        self.rejected(lambda: self.contract.reservation_context(reservation, giveaway,
                                                                '2026-11-03T01:01:00Z'))

    def test_ready_and_expiry_exact_boundaries(self):
        reservation = fixture('reservation-held')
        giveaway = fixture('giveaway-management')['giveaway']
        self.rejected(lambda: self.contract.reservation_context(reservation, giveaway,
                                                                '2026-11-07T08:00:00Z'))
        reservation['state'] = 'READY'
        self.contract.reservation_context(reservation, giveaway, '2026-11-07T08:00:00Z')
        self.rejected(lambda: self.contract.reservation_context(reservation, giveaway,
                                                                '2026-11-07T09:15:00Z'))
        reservation['state'] = 'EXPIRED'
        self.contract.reservation_context(reservation, giveaway, '2026-11-07T09:15:00Z')

    def test_age_source_before_on_after_birthday(self):
        for birthday, expected in [('07-11', 26), ('07-12', 26), ('07-13', 25)]:
            self.assertEqual(domain.derive_age('2000', birthday, '2026-07-12')['age_at_verification'], expected)
        self.assertEqual(domain.derive_age('2000', '07-12', '2026-07-12')['next_birthday_at'], '2027-07-12')

    def test_leap_day_source_semantics(self):
        self.assertEqual(domain.derive_age('2000', '02-29', '2024-02-28'),
                         {'age_at_verification': 23, 'next_birthday_at': '2024-02-29'})
        self.assertEqual(domain.derive_age('2000', '02-29', '2023-02-27'),
                         {'age_at_verification': 22, 'next_birthday_at': '2023-02-28'})

    def test_age_source_range_and_malformed_provider_fields(self):
        for year, birthday in [('2012', '07-13'), ('1924', '07-13'), ('20x0', '07-12'),
                               (None, '07-12'), ('2000', None), ('2000', '07/12'),
                               ('2000', '02-30'), ('2027', '07-12')]:
            self.rejected(lambda: domain.derive_age(year, birthday, '2026-07-12'))
        self.assertEqual(domain.derive_age('2011', '07-13', '2026-07-12')['age_at_verification'], 14)
        self.assertEqual(domain.derive_age('1925', '07-13', '2026-07-12')['age_at_verification'], 100)

    def test_seoul_midnight_birthday_and_exclusive_upper_bound(self):
        claim = fixture('private-age-claim')
        bounds = {'minimum_age': 18, 'maximum_age_exclusive': 26}
        self.assertTrue(self.contract.age_allowed([claim], uid(2), '2026-11-06T14:59:59Z', bounds))
        self.assertFalse(self.contract.age_allowed([claim], uid(2), '2026-11-06T15:00:00Z', bounds))

    def test_seoul_year_boundary(self):
        claim = fixture('private-age-claim')
        claim.update(verified_at='2026-12-31T01:00:00Z', expires_at='2027-01-31T01:00:00Z',
                     next_birthday_at='2027-01-01')
        self.assertEqual(self.contract.current_age([claim], uid(2), '2026-12-31T14:59:59Z'), 25)
        self.assertEqual(self.contract.current_age([claim], uid(2), '2026-12-31T15:00:00Z'), 26)

    def test_unknown_claim_fails_only_gated_policy(self):
        self.assertFalse(self.contract.age_allowed([], uid(2), '2026-11-07T08:00:00Z',
                                                  {'minimum_age': 18, 'maximum_age_exclusive': 30}))
        self.assertTrue(self.contract.age_allowed([], uid(2), '2026-11-07T08:00:00Z',
                                                 {'minimum_age': None, 'maximum_age_exclusive': None}))

    def test_canonical_claim_selected_before_validity(self):
        old = fixture('private-age-claim'); newer = copy.deepcopy(old)
        newer.update(claim_id=uid(31), verified_at='2026-11-02T00:00:00Z', status='REVOKED')
        self.assertIsNone(self.contract.current_age([old, newer], uid(2), '2026-11-07T08:00:00Z'))
        newer.update(status='VERIFIED', expires_at='2026-11-03T00:00:00Z')
        self.assertIsNone(self.contract.current_age([old, newer], uid(2), '2026-11-07T08:00:00Z'))

    def test_canonical_expiry_then_id_tiebreak(self):
        row = fixture('private-age-claim'); superseded = copy.deepcopy(row)
        superseded.update(claim_id=uid(999), status='REPLACED', expires_at=row['verified_at'])
        self.assertEqual(self.contract.current_age([row, superseded], uid(2), '2026-11-02T00:00:00Z'), 25)
        superseded.update(expires_at=row['expires_at'], status='REVOKED')
        self.assertIsNone(self.contract.current_age([row, superseded], uid(2), '2026-11-02T00:00:00Z'))

    def test_claim_expiry_is_exclusive_and_future_claim_invalid(self):
        claim = fixture('private-age-claim')
        self.assertIsNone(self.contract.current_age([claim], uid(2), claim['expires_at']))
        self.assertIsNone(self.contract.current_age([claim], uid(2), '2026-10-31T00:00:00Z'))
        claim['expires_at'] = '2027-02-01T00:00:00Z'
        self.rejected(lambda: self.contract.current_age([claim], uid(2), '2026-11-07T08:00:00Z'))

    def reveal_context(self):
        request = fixture('approved-social-request')
        forward = fixture('social-grant'); reverse = copy.deepcopy(forward)
        reverse.update(grant_id=uid(43), owner_id=uid(3), viewer_id=uid(2), handle_version=3, peer_handle_version=2)
        handles = [dict(user_id=uid(2), platform='INSTAGRAM', version=2, configured=True, handle='fixture_a'),
                   dict(user_id=uid(3), platform='INSTAGRAM', version=3, configured=True, handle='fixture_b')]
        return dict(request=request, grants=[forward, reverse], handles=handles, actor_id=uid(2),
                    event_id=uid(1), platform='INSTAGRAM', now='2026-11-07T08:01:00Z',
                    eligible_users=[uid(2), uid(3)])

    def test_reveal_returns_only_counterpart(self):
        context = self.reveal_context()
        self.assertEqual(self.contract.authorize_reveal(**context), 'fixture_b')
        context['actor_id'] = uid(3)
        self.assertEqual(self.contract.authorize_reveal(**context), 'fixture_a')

    def test_reveal_denies_wrong_user_event_platform_or_policy(self):
        for key, value in [('actor_id', uid(99)), ('event_id', uid(99)), ('platform', 'X'),
                           ('eligible_users', [uid(2)]), ('now', '2026-11-07T07:59:59Z'),
                           ('now', '2026-11-07T10:00:00Z')]:
            context = self.reveal_context(); context[key] = value
            self.rejected(lambda: self.contract.authorize_reveal(**context))

    def test_each_reciprocal_grant_must_still_be_active(self):
        for i in (0, 1):
            for key, value in [('state', 'REVOKED'), ('expires_at', '2026-11-07T08:01:00Z'),
                               ('handle_version', 9), ('peer_handle_version', 9), ('event_id', uid(99))]:
                context = self.reveal_context(); context['grants'][i][key] = value
                self.rejected(lambda: self.contract.authorize_reveal(**context))

    def test_replaced_handle_never_reuses_old_grant(self):
        for i in (0, 1):
            context = self.reveal_context(); context['handles'][i]['version'] += 1
            self.rejected(lambda: self.contract.authorize_reveal(**context))
            context = self.reveal_context(); context['handles'][i]['configured'] = False
            self.rejected(lambda: self.contract.authorize_reveal(**context))

    def test_missing_or_duplicate_grants_fail_closed(self):
        for mode in ('missing', 'duplicate'):
            context = self.reveal_context()
            if mode == 'missing': context['grants'].pop()
            else: context['grants'].append(copy.deepcopy(context['grants'][0]))
            self.rejected(lambda: self.contract.authorize_reveal(**context))

    def test_pending_and_revoked_requests_never_reveal(self):
        context = self.reveal_context(); context['request']['state'] = 'REVOKED'
        self.rejected(lambda: self.contract.authorize_reveal(**context))
        context['request'].update(state='PENDING', approved_platforms=[])
        self.rejected(lambda: self.contract.authorize_reveal(**context))

    def test_handle_validation_is_platform_specific(self):
        domain.validate_handle('X', 'fixture_name')
        domain.validate_handle('INSTAGRAM', 'fixture.name')
        for platform, handle in [('X', 'fixture.name'), ('X', 'a'*16), ('INSTAGRAM', '.start'),
                                 ('INSTAGRAM', 'end.'), ('INSTAGRAM', 'two..dots'), ('X', '한글')]:
            self.rejected(lambda: domain.validate_handle(platform, handle))

    def test_restore_and_control_cannot_unfreeze_mural(self):
        self.contract.transition('mural', 'PAUSED', 'EDITABLE')
        for state in ('EDITABLE', 'PAUSED'):
            self.rejected(lambda: self.contract.transition('mural', 'FROZEN', state))

    def test_reset_delta_cannot_replay_old_pixels(self):
        value = fixture('mural-delta'); value['reset_required'] = True
        self.rejected(lambda: self.contract.validate(value, 'mural.MuralDelta'))
        value['operations'] = []
        self.contract.validate(value, 'mural.MuralDelta')

    def test_public_projection_never_accepts_private_fields(self):
        for name, schema, field in [('public-intro', 'social.IntroPublic', 'age_bounds'),
                                    ('party-full', 'party.PartyPublic', 'meeting'),
                                    ('public-post', 'feed.PostPublic', 'recipient_ids'),
                                    ('notification-safe', 'feed.Notification', 'handle')]:
            value = fixture(name); value[field] = 'private'
            self.rejected(lambda: self.contract.validate(value, schema))


if __name__ == '__main__':
    unittest.main()
