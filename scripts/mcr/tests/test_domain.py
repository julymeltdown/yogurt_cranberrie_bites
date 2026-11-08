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
        self.assertEqual(domain.validate_fixtures(self.contract), 55)

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
        bound = event or uid(1) if 'eventId' in row['path_parameters'] else None
        return self.contract.validate_target(operation, path, query, event_id=bound)

    def test_all_operation_targets_have_typed_closed_paths_and_queries(self):
        required = {'expected_revision': 1, 'party_revision': 2, 'after_revision': 1,
                    'platform': 'X', 'operation_id': 'reserveConcertGiveaway',
                    'idempotency_key': uid(9)}
        for row in self.contract.operations:
            with self.subTest(operation=row['operationId']):
                fields = self.contract._definition(row['query_schema']).get('required', [])
                query = urlencode({name: required[name] for name in fields})
                result = self.target(row['operationId'], query)
                if 'eventId' in row['path_parameters']:
                    self.assertEqual(result['path']['eventId'], uid(1))
                else:
                    self.assertNotIn('eventId', result['path'])
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
                                    actor_id=uid(2), event_id=uid(1), idempotency_key=receipt['idempotency_key'])
        for operation, actor, event in [('claimConcertReservation', uid(2), uid(1)),
                                       ('reserveConcertGiveaway', uid(3), uid(1)),
                                       ('reserveConcertGiveaway', uid(2), uid(99))]:
            self.rejected(lambda: self.contract.validate_http(receipt, operation, 202,
                                                               actor_id=actor, event_id=event, idempotency_key=receipt['idempotency_key']))

    def test_receipt_fingerprints_and_key_material_stay_server_only(self):
        receipt = fixture('command-pending')
        self.contract.validate(receipt, 'common.CommandReceipt')
        for key, value in [('request_hash', 'sha256:' + 'a' * 64),
                           ('fingerprint', 'hmac-sha256:v1:test:' + 'a' * 64),
                           ('key_id', 'test'), ('mac_secret', 'synthetic-secret'),
                           ('canonical_body', {'code': '123456'})]:
            with self.subTest(key=key):
                self.rejected(lambda: self.contract.validate(
                    {**receipt, key: value}, 'common.CommandReceipt'))

    def test_status_query_returns_original_command_metadata(self):
        receipt = fixture('command-pending')
        self.contract.validate_http(receipt, 'getConcertCommandStatus', 200,
                                    actor_id=uid(2), event_id=uid(1),
                                    request_path='/api/concert/events/' + uid(1) + '/commands',
                                    request_query=urlencode({'operation_id': receipt['operation_id'], 'idempotency_key': receipt['idempotency_key']}))

    def test_202_never_claims_success(self):
        receipt = fixture('command-pending')
        receipt.update(state='SUCCEEDED', result_status=200, resource_id=uid(6), resource_revision=1)
        self.rejected(lambda: self.contract.validate_http(receipt, 'reserveConcertGiveaway', 202,
                                                           actor_id=uid(2), event_id=uid(1), idempotency_key=receipt['idempotency_key']))

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

    # --- WP-01 profile domain contract (T-002 .. T-005) --------------------

    PROFILE_OPERATIONS = [
        'listIntroPosts', 'getIntroPost', 'getOwnIntroPost', 'listOwnIntroPosts',
        'saveIntroPostDraft', 'publishIntroPost', 'deleteIntroPost', 'setIntroPostLike',
        'clearIntroPostLike', 'listIntroPostLikers', 'listIntroPostComments',
        'createIntroPostComment', 'deleteIntroPostComment',
        'listProfilePosts', 'createProfilePost', 'deleteProfilePost', 'initProfileMediaUpload',
        'putProfileMediaBytes', 'finalizeProfileMedia', 'getProfileMedia', 'readProfileMedia',
        'getProfileRoom', 'getOwnRoom', 'saveOwnRoom', 'setOwnRoomVisibility', 'uploadRoomPoster',
        'getOwnProfileAvatar', 'setOwnProfileAvatar', 'listActivity', 'listNotifications',
        'markNotificationRead', 'followProfile', 'unfollowProfile',
    ]

    # Viewer-facing objects: none may reveal the audience or a numeric age.
    VIEWER_FACING = ['IntroPostDetail', 'IntroPostPage', 'LikerProfile', 'IntroComment',
                     'ActivityActor', 'ActivityItem', 'ProfilePost', 'ProfileRoom']

    def definition(self, name):
        return self.contract._definition('social.' + name)

    def test_profile_operation_register_is_additive_unique_and_event_free(self):
        new = [op for op in self.contract.operations if op['operationId'] in self.PROFILE_OPERATIONS]
        self.assertEqual(len(new), len(self.PROFILE_OPERATIONS))
        for op in new:
            with self.subTest(operation=op['operationId']):
                self.assertNotIn('/events/{eventId}', op['path'])
                self.assertEqual(op['cache_control'], 'no-store')
                self.assertIn(op['rule'], self.contract.policy['rules'])
                self.assertTrue(op['errors'])
        self.assertEqual(len(self.contract.operations), 150)
        ids = [op['operationId'] for op in self.contract.operations]
        routes = [(op['method'], op['path']) for op in self.contract.operations]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(len(routes), len(set(routes)))

    def test_profile_mutations_declare_conflict_and_rate_limit(self):
        for op in self.contract.operations:
            if op['operationId'] not in self.PROFILE_OPERATIONS or op['method'] == 'get':
                continue
            with self.subTest(operation=op['operationId']):
                self.assertIn('429', op['errors'])
                self.assertIn('RATE_LIMITED', op['errors']['429'])
                self.assertIn('409', op['errors'])
                if op['idempotency_key']:
                    self.assertIn('IDEMPOTENCY_CONFLICT', op['errors']['409'])

    def test_intro_post_detail_never_reveals_the_audience(self):
        properties = set(self.definition('IntroPostDetail')['properties'])
        for field in ('audience', 'allowed_gender', 'minimum_age', 'maximum_age_exclusive',
                      'age_bounds', 'audience_summary', 'room_id', 'event_id',
                      'external_account', 'handle', 'age'):
            with self.subTest(field=field):
                self.assertNotIn(field, properties)
        self.assertIs(self.definition('IntroPostDetail')['additionalProperties'], False)

    def test_intro_post_detail_rejects_audience_payload_fields(self):
        value = {
            'intro_id': uid(40), 'author': None, 'body': 'Looking for mutuals',
            'favorites_snapshot': [], 'currently_seeking': None, 'interaction_style': None,
            'attachments': [],
            'like': {'count': 3, 'liked_by_viewer': False}, 'comment_count': 1,
            'published_at': '2026-11-08T00:00:00Z', 'expires_at': None,
        }
        for field, injected in [('audience', {'allowed_gender': 'female'}),
                                ('allowed_gender', 'female'), ('minimum_age', 18),
                                ('maximum_age_exclusive', 30), ('room_id', uid(41))]:
            with self.subTest(field=field):
                self.rejected(lambda: self.contract.validate({**value, field: injected},
                                                             'social.IntroPostDetail'))

    def test_viewer_facing_objects_never_expose_age_or_audience(self):
        forbidden = {'age', 'minimum_age', 'maximum_age_exclusive', 'age_bounds',
                     'allowed_gender', 'audience', 'audience_summary', 'exact_age',
                     'verified_age', 'date_of_birth'}
        for name in self.VIEWER_FACING:
            with self.subTest(name=name):
                self.assertFalse(forbidden & set(self.definition(name)['properties']))

    def test_own_intro_post_carries_paired_audience_bounds(self):
        audience = self.definition('IntroAudience')
        self.assertIs(audience['additionalProperties'], False)
        self.assertEqual(audience['required'],
                         ['allowed_gender', 'minimum_age', 'maximum_age_exclusive'])
        self.assertIn('dependentRequired', audience)
        self.contract.validate({'allowed_gender': 'any', 'minimum_age': None,
                                'maximum_age_exclusive': None}, 'social.IntroAudience')
        self.contract.validate({'allowed_gender': 'female', 'minimum_age': 18,
                                'maximum_age_exclusive': 30}, 'social.IntroAudience')
        for bounds in [{'allowed_gender': 'any', 'minimum_age': 18, 'maximum_age_exclusive': None},
                       {'allowed_gender': 'any', 'minimum_age': None, 'maximum_age_exclusive': 30},
                       {'allowed_gender': 'any', 'minimum_age': 13, 'maximum_age_exclusive': 30},
                       {'allowed_gender': 'any', 'minimum_age': 18, 'maximum_age_exclusive': 102},
                       {'allowed_gender': 'everyone', 'minimum_age': None,
                        'maximum_age_exclusive': None}]:
            with self.subTest(bounds=bounds):
                self.rejected(lambda: self.contract.validate(bounds, 'social.IntroAudience'))

    def test_intro_like_state_count_is_nullable_only_for_lost_access(self):
        schema = self.definition('IntroLikeState')
        self.assertIs(schema['additionalProperties'], False)
        self.assertEqual(schema['required'], ['liked', 'like_count'])
        self.contract.validate({'liked': True, 'like_count': 4}, 'social.IntroLikeState')
        self.contract.validate({'liked': False, 'like_count': None}, 'social.IntroLikeState')
        self.rejected(lambda: self.contract.validate({'liked': True, 'like_count': -1},
                                                     'social.IntroLikeState'))

    def test_room_layout_limits_posters_and_carries_no_event_or_intro(self):
        layout = self.definition('RoomLayout')
        self.assertIs(layout['additionalProperties'], False)
        self.assertEqual(layout['properties']['posters']['maxItems'], 4)
        self.assertEqual(layout['properties']['furniture']['maxItems'], 32)
        self.assertEqual(layout['properties']['version']['const'], 1)
        self.assertNotIn('intro_id', layout['properties'])
        self.assertNotIn('event_id', layout['properties'])
        posters = [{'poster_media_id': uid(50 + index), 'slot': index, 'x': index, 'y': 0,
                    'width': 8, 'height': 8} for index in range(4)]
        base = {'version': 1, 'wall': 'wall_default', 'floor': 'floor_default',
                'furniture': [], 'character': {'cell_x': 4, 'cell_y': 4, 'facing': 'RIGHT'}}
        self.contract.validate({**base, 'posters': posters}, 'social.RoomLayout')
        self.rejected(lambda: self.contract.validate({**base, 'posters': posters + [
            {'poster_media_id': uid(60), 'slot': 0, 'x': 0, 'y': 0, 'width': 8, 'height': 8}]},
            'social.RoomLayout'))
        self.rejected(lambda: self.contract.validate({**base, 'posters': [], 'intro_id': uid(61)},
                                                     'social.RoomLayout'))
        self.rejected(lambda: self.contract.validate({**base, 'posters': [], 'event_id': uid(62)},
                                                     'social.RoomLayout'))
        self.rejected(lambda: self.contract.validate({**base, 'posters': [], 'version': 2},
                                                     'social.RoomLayout'))

    def test_upload_poster_content_type_is_the_png_literal(self):
        schema = self.definition('UploadPoster')
        self.assertIs(schema['additionalProperties'], False)
        self.assertEqual(schema['properties']['content_type'], {'const': 'image/png'})
        self.assertEqual(schema['properties']['preset']['enum'], ['FINE', 'CHUNKY'])
        self.assertEqual(schema['properties']['width']['maximum'], 128)
        self.assertEqual(schema['properties']['height']['maximum'], 128)
        self.assertEqual(schema['properties']['bytes_base64']['maxLength'], 1048576)
        value = {'preset': 'FINE', 'width': 64, 'height': 64,
                 'content_type': 'image/png', 'bytes_base64': 'iVBORw0KGgo='}
        self.contract.validate(value, 'social.UploadPoster')
        for content_type in ['image/jpeg', 'image/webp', 'image/gif', 'IMAGE/PNG', 'image/png;q=1']:
            with self.subTest(content_type=content_type):
                self.rejected(lambda: self.contract.validate({**value, 'content_type': content_type},
                                                             'social.UploadPoster'))

    def test_room_and_post_requests_are_closed(self):
        for name in ['ProfileRoom', 'OwnRoom', 'SaveRoom', 'SetRoomVisibility', 'RoomPoster',
                     'ProfilePost', 'CreateProfilePost', 'ProfilePostPage', 'IntroPostDetail',
                     'OwnIntroPost', 'SaveIntroPost', 'LikerPage', 'IntroComment',
                     'CreateIntroComment', 'IntroCommentPage', 'ActivityPage', 'FollowState']:
            with self.subTest(name=name):
                schema = self.definition(name)
                self.assertIs(schema['additionalProperties'], False)
                self.assertIn('required', schema)
                self.assertLessEqual(set(schema['required']), set(schema['properties']))

    def test_new_policy_rules_are_complete(self):
        required = ['intro_like', 'intro_likers', 'intro_comment_read', 'intro_comment_create',
                    'intro_comment_delete', 'room_save', 'room_visibility', 'room_poster_upload',
                    'profile_post_create', 'profile_post_delete', 'identity_start',
                    'identity_callback', 'identity_disconnect', 'profile_update',
                    'favorites_set', 'avatar_set', 'follow']
        for name in required:
            with self.subTest(rule=name):
                rule = self.contract.policy['rules'][name]
                self.assertTrue(rule['preconditions'])
                self.assertTrue(rule['result'])

    def test_every_profile_operation_rule_exists_in_policy(self):
        for op in self.contract.operations:
            if op['operationId'] in self.PROFILE_OPERATIONS:
                with self.subTest(operation=op['operationId']):
                    self.assertIn(op['rule'], self.contract.policy['rules'])

    def test_notification_kind_and_target_are_additive(self):
        notification = self.contract._definition('feed.Notification')
        target = self.contract._definition('feed.NotificationTarget')
        self.assertEqual(notification['properties']['kind']['enum'],
                         ['PARTY', 'GIVEAWAY', 'SOCIAL', 'MODERATION', 'NOTICE', 'FEED', 'INTRO_LIKED'])
        self.assertEqual(target['properties']['target_type']['enum'],
                         ['PARTY', 'APPLICATION', 'GIVEAWAY', 'RESERVATION', 'SOCIAL_REQUEST',
                          'POST', 'NOTICE', 'REPORT', 'INTRO_POST'])

    def test_proto_enum_numbers_are_append_only(self):
        source = (domain.ROOT / 'backend/proto/notification/v1/concert_inbox.proto').read_text()
        kind = source.split('enum NotificationKind {', 1)[1].split('}', 1)[0]
        target = source.split('enum TargetType {', 1)[1].split('}', 1)[0]
        def numbers(block):
            return {name: int(value) for name, value in
                    (row.strip().rstrip(';').split(' = ') for row in block.strip().splitlines())}
        kind_values = numbers(kind)
        target_values = numbers(target)
        self.assertEqual(kind_values['NOTIFICATION_KIND_UNSPECIFIED'], 0)
        self.assertEqual(kind_values['PARTY'], 1)
        self.assertEqual(kind_values['GIVEAWAY'], 2)
        self.assertEqual(kind_values['SOCIAL'], 3)
        self.assertEqual(kind_values['MODERATION'], 4)
        self.assertEqual(kind_values['NOTICE'], 5)
        self.assertEqual(kind_values['FEED'], 6)
        self.assertEqual(kind_values['INTRO_LIKED'], 7)
        self.assertEqual(target_values['TARGET_TYPE_UNSPECIFIED'], 0)
        self.assertEqual(target_values['TARGET_PARTY'], 1)
        self.assertEqual(target_values['REPORT'], 8)
        self.assertEqual(target_values['INTRO_POST'], 9)
        self.assertEqual(len(set(kind_values.values())), len(kind_values))
        self.assertEqual(len(set(target_values.values())), len(target_values))


if __name__ == '__main__':
    unittest.main()
