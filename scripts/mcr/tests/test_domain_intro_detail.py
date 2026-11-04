"""Additive BE27 detail contract; no service/HTTP authorization acceptance."""
import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('intro_detail_contract', ROOT / 'scripts/mcr/validate-domain.py')
domain = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(domain)


def uid(n):
    return f'00000000-0000-4000-8000-{n:012x}'


class IntroDetailContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = domain.Contract()
        cls.operation = next(o for o in cls.contract.operations if o['operationId'] == 'getConcertIntro')

    def test_detail_is_a_closed_public_read_using_the_existing_intro_schema(self):
        listing = next(o for o in self.contract.operations if o['operationId'] == 'listConcertIntros')
        expected = copy.deepcopy(listing)
        expected.update(operationId='getConcertIntro',
                        path='/api/concert/events/{eventId}/intros/{introId}',
                        request=None, response='social.IntroPublic', query_schema='common.EmptyRequest')
        expected['path_parameters']['introId'] = copy.deepcopy(expected['path_parameters']['eventId'])
        self.assertEqual(self.operation, expected)
        self.assertEqual(self.contract.policy['version'], '0.1.3')

    def test_direct_ids_are_canonical_event_bound_and_query_is_empty(self):
        path = f'/api/concert/events/{uid(1)}/intros/{uid(10)}'
        result = self.contract.validate_target('getConcertIntro', path, '', event_id=uid(1))
        self.assertEqual(result['path'], {'eventId': uid(1), 'introId': uid(10)})
        self.assertEqual(result['query'], {})
        for invalid in (path + '/', path.replace(uid(10), uid(10).upper()),
                        path.replace(uid(10), 'bad'), path.replace(uid(10), '%30' + uid(10)[1:])):
            with self.subTest(path=invalid), self.assertRaises(domain.ContractError):
                self.contract.validate_target('getConcertIntro', invalid, '', event_id=uid(1))
        for query in ('limit=20', 'cursor=x', 'q=fan', 'actor_id=' + uid(2)):
            with self.subTest(query=query), self.assertRaises(domain.ContractError):
                self.contract.validate_target('getConcertIntro', path, query, event_id=uid(1))
        with self.assertRaises(domain.ContractError):
            self.contract.validate_target('getConcertIntro', path, '', event_id=uid(3))

    def test_detail_uses_the_same_closed_public_projection_as_list_items(self):
        public = domain.read(domain.FIXTURES / 'public-intro.json')
        self.contract.validate_http(public, 'getConcertIntro', 200)
        self.contract.validate({'items': [public], 'next_cursor': None}, 'social.IntroList')
        for field in ('age_bounds', 'expires_at', 'minimum_age', 'date_of_birth', 'owner_id'):
            with self.subTest(field=field), self.assertRaises(domain.ContractError):
                self.contract.validate_http({**public, field: None}, 'getConcertIntro', 200)
        with self.assertRaises(domain.ContractError):
            self.contract.validate_http(public, 'getConcertIntro', 202)
