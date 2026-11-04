"""Namespace coverage regressions without mutating canonical or generated files."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('c07_generator', ROOT / 'scripts/mcr/generate-api-client.py')
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)


class InventoryTests(unittest.TestCase):
    def test_both_namespaces_reject_unknown_missing_malformed_and_nonjson_files(self):
        expected = {prefix + '/fixture.json': '{}\n' for prefix in
                    ('contracts/generated/api-client', 'frontend/src/shared/api/generated', 'contracts/fixtures/c07')}
        with tempfile.TemporaryDirectory(prefix='mcr-c07-inventory-') as directory:
            root = Path(directory)
            for name, content in expected.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            generator.verify(expected, root)
            for name, content in expected.items():
                with self.subTest(namespace=name):
                    path = root / name
                    path.unlink()
                    with self.assertRaisesRegex(ValueError, 'missing/unexpected'): generator.verify(expected, root)
                    for value in ('{broken', '{"unknown_enum":"NEW"}', '[]'):
                        path.write_text(value)
                        with self.assertRaisesRegex(ValueError, 'generated drift'): generator.verify(expected, root)
                    path.write_text(content)
                    for extra in ('unindexed.json', 'unknown.txt'):
                        unknown = path.parent / extra
                        unknown.write_text('{}')
                        with self.assertRaisesRegex(ValueError, 'missing/unexpected'): generator.verify(expected, root)
                        unknown.unlink()
                    path.unlink()
                    path.symlink_to(root / next(k for k in expected if k != name))
                    with self.assertRaisesRegex(ValueError, 'nonregular'): generator.verify(expected, root)
                    path.unlink()
                    path.write_text(content)

    def test_unsupported_schema_extensions_fail_generation(self):
        for rule in ({'contains': {}}, {'format': 'unimplemented'}, {'type': 'unimplemented'},
                     {'not': {'contains': {}}}):
            with self.assertRaises(ValueError): generator.check_schema(rule)

    def test_negation_children_are_admitted_recursively(self):
        for rule in ({'not': True}, {'not': False}, {'not': {'const': 0}},
                     {'not': {'not': {'type': 'integer'}}}):
            generator.check_schema(rule)

    def test_motion_keyword_shapes_are_validated_as_schema_data(self):
        for value in (None, True, False, '0', [], {}, float('nan'), float('inf'), -float('inf')):
            with self.subTest(bound=value), self.assertRaisesRegex(ValueError, 'Invalid exclusiveMinimum'):
                generator.check_schema({'exclusiveMinimum': value})
        for value in (None, [], True, {'a': 'b'}, {'a': [1]}, {'a': ['b', 'b']}, {'a': [{}]}, {1: []}):
            with self.subTest(dependencies=value), self.assertRaisesRegex(ValueError, 'Invalid dependentRequired'):
                generator.check_schema({'dependentRequired': value})
        for rule in ({'exclusiveMinimum': 0}, {'exclusiveMinimum': -0.5},
                     {'dependentRequired': {}}, {'dependentRequired': {'a': ['b'], 'b': ['a'], 'c': []}},
                     {'not': {'dependentRequired': {'a': ['b']}}}):
            generator.check_schema(rule)


if __name__ == '__main__':
    unittest.main()
