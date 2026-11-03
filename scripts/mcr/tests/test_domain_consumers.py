"""Independent FE21/FE24 context regressions; no service authorization/replay proof."""
import copy
import importlib.util
from pathlib import Path
import unittest
from urllib.parse import urlencode

PATH = Path(__file__).resolve().parents[1] / 'validate-domain.py'
SPEC = importlib.util.spec_from_file_location('mcr_domain_consumer_oracle', PATH)
domain = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(domain)


def uid(number):
    return f'00000000-0000-4000-8000-{number:012x}'


EVENT, ACTOR, OTHER = uid(1), uid(2), uid(3)
PARTY, GIFT, APPLICATION, KEY = uid(4), uid(5), uid(8), uid(71)
ABSENT = object()


def fixture(name):
    return domain.read(domain.FIXTURES / (name + '.json'))


def page(items=(), cursor=None):
    return {'items': list(items), 'next_cursor': cursor}


def party_activity():
    party = fixture('party-full')
    application = {'application_id': APPLICATION, 'event_id': EVENT, 'party_id': uid(14),
                   'applicant': copy.deepcopy(party['owner']), 'note': '',
                   'created_at': party['created_at'], 'state': 'PENDING', 'revision': 1}
    member = {'event_id': EVENT, 'party_id': uid(9), 'user_id': ACTOR, 'role': 'MEMBER',
              'state': 'ACTIVE', 'revision': 1, 'joined_at': party['created_at']}
    return {'owned': page([party]), 'memberships': page([member]), 'applications': page([application])}


def gift_activity():
    reservation = fixture('reservation-held')
    reservation.update(recipient_id=ACTOR, giveaway_id=uid(15))
    return {'giving': page([fixture('giveaway-management')['giveaway']]), 'receiving': page([reservation])}


def read_payload(operation):
    return {
        'listConcertParties': lambda: page([fixture('party-full')]),
        'getConcertParty': lambda: fixture('party-full'),
        'getConcertPartyActivity': party_activity,
        'listConcertGiveaways': lambda: page([fixture('giveaway-management')['giveaway']]),
        'getConcertGiveaway': lambda: fixture('giveaway-management')['giveaway'],
        'getConcertGiftActivity': gift_activity,
    }[operation]()


READS = ('listConcertParties', 'getConcertParty', 'getConcertPartyActivity',
         'listConcertGiveaways', 'getConcertGiveaway', 'getConcertGiftActivity')


class DomainConsumerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = domain.Contract()

    def target(self, operation):
        op = next(row for row in self.contract.operations if row['operationId'] == operation)
        return (op['path'].replace('{eventId}', EVENT).replace('{partyId}', PARTY)
                .replace('{giveawayId}', GIFT).replace('{applicationId}', APPLICATION))

    def check_http(self, value, operation, status=200, *, actor=ACTOR, event=EVENT,
                   path=ABSENT, query='', key=None, resource=None):
        self.contract.validate_http(value, operation, status, actor_id=actor, event_id=event,
                                    request_path=self.target(operation) if path is ABSENT else path,
                                    request_query=query, idempotency_key=key,
                                    expected_resource_id=resource)

    def rejected(self, action):
        with self.assertRaises(domain.ContractError):
            action()

    def receipt(self, state='PENDING'):
        value = fixture('command-pending')
        value.update(operation_id='applyConcertParty', state=state)
        if state == 'SUCCEEDED':
            value.update(result_status=200, resource_id=APPLICATION, resource_revision=1)
        elif state == 'FAILED':
            value['failure_reason'] = 'PARTY_FULL'
        return value

    def lookup(self, value, **kwargs):
        kwargs.setdefault('query', urlencode({'operation_id': 'applyConcertParty', 'idempotency_key': KEY}))
        return self.check_http(value, 'getConcertCommandStatus', **kwargs)

    def test_six_valid_reads_preserve_payload(self):
        for operation in READS:
            with self.subTest(operation=operation):
                payload = read_payload(operation)
                before = copy.deepcopy(payload)
                self.check_http(payload, operation)
                self.assertEqual(payload, before)

    def test_successful_reads_require_canonical_actor_event_and_path(self):
        for operation in READS:
            for context in ({'actor': None}, {'event': None}, {'actor': 'not-a-uuid'},
                            {'event': 'not-a-uuid'}, {'path': None}):
                with self.subTest(operation=operation, context=context):
                    self.rejected(lambda: self.check_http(read_payload(operation), operation, **context))

    def test_reads_validate_actual_path_even_for_empty_pages(self):
        for operation in ('listConcertParties', 'listConcertGiveaways'):
            for path in (self.target(operation).replace(EVENT, uid(99)),
                         self.target(operation) + '/unknown',
                         self.target(operation).replace(EVENT, '%30' + EVENT[1:])):
                with self.subTest(operation=operation, path=path):
                    self.rejected(lambda: self.check_http(page(), operation, path=path))

    def test_successful_reads_validate_actual_query(self):
        for query in ('unknown=value', 'limit=01', 'limit=1&limit=2', 'cursor=%FF', 'state=DRAFT'):
            with self.subTest(query=query):
                self.rejected(lambda: self.check_http(page(), 'listConcertGiveaways', query=query))

    def test_detail_identity_is_bound_to_requested_id(self):
        for operation, field in (('getConcertParty', 'party_id'), ('getConcertGiveaway', 'giveaway_id')):
            payload = read_payload(operation)
            payload[field] = uid(99)
            with self.subTest(operation=operation):
                self.rejected(lambda: self.check_http(payload, operation))

    def test_public_list_and_detail_rows_are_event_bound(self):
        for operation in ('listConcertParties', 'getConcertParty', 'listConcertGiveaways', 'getConcertGiveaway'):
            payload = read_payload(operation)
            (payload['items'][0] if 'items' in payload else payload)['event_id'] = uid(99)
            with self.subTest(operation=operation):
                self.rejected(lambda: self.check_http(payload, operation))

    def test_every_private_collection_row_is_event_bound(self):
        for operation, collections in (('getConcertPartyActivity', ('owned', 'memberships', 'applications')),
                                       ('getConcertGiftActivity', ('giving', 'receiving'))):
            for collection in collections:
                payload = read_payload(operation)
                payload[collection]['items'][0]['event_id'] = uid(99)
                with self.subTest(operation=operation, collection=collection):
                    self.rejected(lambda: self.check_http(payload, operation))

    def test_private_owned_party_and_giving_owner_are_actor_bound(self):
        for operation, collection in (('getConcertPartyActivity', 'owned'), ('getConcertGiftActivity', 'giving')):
            payload = read_payload(operation)
            payload[collection]['items'][0]['owner']['user_id'] = OTHER
            with self.subTest(operation=operation):
                self.rejected(lambda: self.check_http(payload, operation))

    def test_own_application_applicant_and_membership_are_actor_bound(self):
        for collection in ('applications', 'memberships'):
            payload = party_activity()
            row = payload[collection]['items'][0]
            if collection == 'applications':
                row['applicant']['user_id'] = OTHER
            else:
                row['user_id'] = OTHER
            with self.subTest(collection=collection):
                self.rejected(lambda: self.check_http(payload, 'getConcertPartyActivity'))

    def test_own_receiving_recipient_is_actor_bound(self):
        payload = gift_activity()
        payload['receiving']['items'][0]['recipient_id'] = OTHER
        self.rejected(lambda: self.check_http(payload, 'getConcertGiftActivity'))

    def test_new_membership_wire_field_is_required_and_canonical(self):
        member = party_activity()['memberships']['items'][0]
        self.contract.validate(member, 'party.Member')
        for value in (None, 'not-a-uuid'):
            altered = {**member, 'event_id': value}
            self.rejected(lambda: self.contract.validate(altered, 'party.Member'))
        del member['event_id']
        self.rejected(lambda: self.contract.validate(member, 'party.Member'))

    def test_browser_candidate_artifact_requires_same_membership_event_binding(self):
        artifact = domain.read(domain.ROOT / 'contracts/generated/mcr-domain.openapi.json')
        validator = domain.Draft202012Validator(
            {'$ref': '#/components/schemas/Domain_party_Member', 'components': artifact['components']},
            format_checker=domain.FORMATS)
        member = party_activity()['memberships']['items'][0]
        validator.validate(member)
        del member['event_id']
        with self.assertRaises(domain.ValidationError):
            validator.validate(member)

    def test_duplicate_ids_are_rejected_per_collection(self):
        for operation, collections in (('listConcertParties', (None,)), ('listConcertGiveaways', (None,)),
                                       ('getConcertPartyActivity', ('owned', 'memberships', 'applications')),
                                       ('getConcertGiftActivity', ('giving', 'receiving'))):
            for collection in collections:
                payload = read_payload(operation)
                rows = payload[collection]['items'] if collection else payload['items']
                rows.append(copy.deepcopy(rows[0]))
                with self.subTest(operation=operation, collection=collection):
                    self.rejected(lambda: self.check_http(payload, operation))

    def test_multiple_memberships_for_same_actor_in_distinct_parties_are_valid(self):
        payload = party_activity()
        second = {**payload['memberships']['items'][0], 'party_id': uid(19)}
        payload['memberships']['items'].append(second)
        self.check_http(payload, 'getConcertPartyActivity')

    def test_public_gift_drafts_are_rejected_but_owned_drafts_remain_valid(self):
        draft = fixture('giveaway-management')['giveaway']
        draft['state'] = 'DRAFT'
        self.rejected(lambda: self.check_http(draft, 'getConcertGiveaway'))
        self.rejected(lambda: self.check_http(page([draft]), 'listConcertGiveaways'))
        payload = gift_activity()
        payload['giving']['items'] = [draft]
        self.check_http(payload, 'getConcertGiftActivity')

    def test_public_projections_reject_private_peer_lists_and_meeting_instructions(self):
        for operation, key, value in (('getConcertParty', 'applications', []),
                                      ('getConcertGiveaway', 'recipient_ids', [OTHER]),
                                      ('getConcertGiveaway', 'instructions', 'private synthetic meeting')):
            payload = read_payload(operation)
            payload[key] = value
            with self.subTest(operation=operation, key=key):
                self.rejected(lambda: self.check_http(payload, operation))

    def test_independent_activity_cursors_and_empty_continuations_are_preserved(self):
        for operation, names in (('getConcertPartyActivity', ('owned', 'memberships', 'applications')),
                                 ('getConcertGiftActivity', ('giving', 'receiving'))):
            payload = {name: page(cursor=name + '_next') for name in names}
            query = ('owned_cursor=a&membership_cursor=b&application_cursor=c&limit=1'
                     if operation == 'getConcertPartyActivity' else 'giving_cursor=a&receiving_cursor=b&limit=1')
            before = copy.deepcopy(payload)
            self.check_http(payload, operation, query=query)
            self.assertEqual(payload, before)
        for operation in ('listConcertParties', 'listConcertGiveaways'):
            self.check_http(page(cursor='next_page'), operation, query='limit=1')

    def test_requested_page_bound_applies_independently_to_each_collection(self):
        for operation, collections in (('listConcertParties', (None,)), ('listConcertGiveaways', (None,)),
                                       ('getConcertPartyActivity', ('owned', 'memberships', 'applications')),
                                       ('getConcertGiftActivity', ('giving', 'receiving'))):
            for collection in collections:
                payload = read_payload(operation)
                rows = payload[collection]['items'] if collection else payload['items']
                second = copy.deepcopy(rows[0])
                identity = ('party_id' if operation == 'listConcertParties' or collection in ('owned', 'memberships')
                            else 'application_id' if collection == 'applications'
                            else 'reservation_id' if collection == 'receiving' else 'giveaway_id')
                second[identity] = uid(90)
                rows.append(second)
                with self.subTest(operation=operation, collection=collection):
                    self.check_http(payload, operation)
                    self.rejected(lambda: self.check_http(payload, operation, query='limit=1'))

    def test_public_gift_terminal_and_paused_states_remain_readable(self):
        for state in ('OPEN', 'PAUSED', 'CLOSED', 'CANCELLED'):
            payload = fixture('giveaway-management')['giveaway']
            payload['state'] = state
            with self.subTest(state=state):
                self.check_http(payload, 'getConcertGiveaway')

    def test_receipt_lookup_accepts_all_documented_states_with_correct_binding(self):
        for state in ('PENDING', 'SUCCEEDED', 'FAILED', 'RETIRED'):
            with self.subTest(state=state):
                self.lookup(self.receipt(state))

    def test_receipt_lookup_requires_canonical_actor_event_path_and_exact_query(self):
        for context in ({'actor': None}, {'event': None}, {'actor': 'invalid'}, {'event': 'invalid'},
                        {'path': None}, {'query': ''}, {'query': 'operation_id=applyConcertParty'},
                        {'path': self.target('getConcertCommandStatus') + '/unknown'}):
            with self.subTest(context=context):
                self.rejected(lambda: self.lookup(self.receipt(), **context))

    def test_receipt_actor_event_operation_and_key_must_match_lookup(self):
        for field, value in (('actor_id', OTHER), ('event_id', uid(99)), ('event_id', None),
                             ('operation_id', 'reserveConcertGiveaway'), ('idempotency_key', uid(72))):
            payload = self.receipt()
            payload[field] = value
            with self.subTest(field=field, value=value):
                self.rejected(lambda: self.lookup(payload))

    def test_receipt_lookup_rejects_duplicate_or_unknown_query_fields(self):
        query = urlencode({'operation_id': 'applyConcertParty', 'idempotency_key': KEY})
        for suffix in ('&operation_id=reserveConcertGiveaway', '&extra=ignored'):
            self.rejected(lambda: self.lookup(self.receipt(), query=query + suffix))

    def test_direct_pending_receipt_requires_captured_canonical_key(self):
        self.check_http(self.receipt(), 'applyConcertParty', 202, key=KEY)
        for key in (None, 'invalid', uid(72)):
            with self.subTest(key=key):
                self.rejected(lambda: self.check_http(self.receipt(), 'applyConcertParty', 202, key=key))

    def test_202_cannot_be_completed_failed_or_retired(self):
        for state in ('SUCCEEDED', 'FAILED', 'RETIRED'):
            with self.subTest(state=state):
                self.rejected(lambda: self.check_http(self.receipt(state), 'applyConcertParty', 202, key=KEY))

    def test_succeeded_receipt_uses_registered_final_status_never_pending_or_other_2xx(self):
        self.lookup(self.receipt('SUCCEEDED'))
        for status in (201, 202, 204, 299):
            payload = self.receipt('SUCCEEDED')
            payload['result_status'] = status
            with self.subTest(status=status):
                self.rejected(lambda: self.lookup(payload))

    def test_failed_receipt_cannot_use_another_commands_failure_reason(self):
        self.lookup(self.receipt('FAILED'))
        payload = self.receipt('FAILED')
        payload['failure_reason'] = 'MURAL_FROZEN'
        self.rejected(lambda: self.lookup(payload))

    def test_known_nonnull_result_target_must_match_expected_resource(self):
        self.lookup(self.receipt('SUCCEEDED'), resource=APPLICATION)
        for resource in (uid(99), 'invalid'):
            with self.subTest(resource=resource):
                self.rejected(lambda: self.lookup(self.receipt('SUCCEEDED'), resource=resource))
        self.lookup(self.receipt(), resource=APPLICATION)

    def test_unavailable_lookup_is_an_error_projection_not_unknown_success(self):
        error = {'code': 'NOT_FOUND', 'message': 'Unavailable', 'details': {'reason': 'RESOURCE_UNAVAILABLE'}}
        self.lookup(error, status=404)
        self.rejected(lambda: self.lookup(error, status=200))
        unknown = self.receipt()
        unknown['state'] = 'UNKNOWN'
        self.rejected(lambda: self.lookup(unknown))

    def test_retired_receipt_cannot_smuggle_completed_result_or_failure(self):
        for fields in ({'resource_id': APPLICATION}, {'result_status': 200}, {'failure_reason': 'PARTY_FULL'}):
            with self.subTest(fields=fields):
                self.rejected(lambda: self.lookup({**self.receipt('RETIRED'), **fields}))


if __name__ == '__main__':
    unittest.main()
