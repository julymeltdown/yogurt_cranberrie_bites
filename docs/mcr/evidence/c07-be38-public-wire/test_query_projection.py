"""Focused fail-closed regression cases for the granted query projection."""
import copy
import importlib.util
from pathlib import Path
import unittest
ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('transport', ROOT/'scripts/mcr/generate-domain-transport.py')
transport = importlib.util.module_from_spec(spec)
spec.loader.exec_module(transport)
BASE = {'type':'object','additionalProperties':False,'properties':{'v':{'type':'string','pattern':'^[a-z]+$','description':'documentation'}}}
class QueryProjection(unittest.TestCase):
    def test_optional_and_annotation(self):
        self.assertEqual(transport.query_schema(copy.deepcopy(BASE)), {'required':[], 'properties':{'v':{'type':'string','pattern':'^[a-z]+$'}}})
        required = {**BASE,'required':['v']}
        self.assertEqual(transport.query_schema(required)['required'], ['v'])
        self.assertEqual(BASE['properties']['v']['description'], 'documentation')
    def test_invalid_required(self):
        for required in (None,'v',1,[1],['v','v'],['unknown']):
            with self.subTest(required=required), self.assertRaises(AssertionError):
                transport.query_schema({**BASE,'required':required})
    def test_unknown_keywords_fail_closed(self):
        for schema in ({**BASE,'anyOf':[]}, {**BASE,'additionalProperties':True}, {**BASE,'description':'object annotation not granted'}):
            with self.subTest(schema=schema), self.assertRaises(AssertionError):
                transport.query_schema(schema)
        for change in ({'description':None},{'description':42},{'unknown':True},{'minItems':1}):
            with self.subTest(change=change), self.assertRaises(AssertionError):
                transport.query_schema({**BASE,'properties':{'v':{**BASE['properties']['v'],**change}}})
    def test_prior_scalar_projection_is_preserved(self):
        for value in ({'type':'integer','minimum':0,'maximum':50,'default':1}, {'type':'string','format':'uuid'}, {'type':'boolean'}):
            expected = {k:v for k,v in value.items() if k != 'default'}
            self.assertEqual(transport.scalar(value),expected)
if __name__ == '__main__':
    unittest.main()
