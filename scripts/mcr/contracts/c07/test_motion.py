import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location('motion_coverage', Path(__file__).with_name('motion_coverage.py'))
motion = importlib.util.module_from_spec(spec)
spec.loader.exec_module(motion)


def read(path):
    return json.loads(path.read_text())


class MotionCoverageTests(unittest.TestCase):
    def fixtures(self):
        return {p.relative_to(ROOT).as_posix(): read(p) for p in (ROOT / 'contracts/fixtures/assets/motion-v2').rglob('*.json')}

    def test_all_published_config_fixtures_have_explicit_outcomes(self):
        fixtures = self.fixtures()
        inventory = []
        motion.check_motion_fixtures(ROOT, read, fixtures, inventory)
        self.assertEqual(len(inventory), 62)
        self.assertEqual({r['file'] for r in inventory}, set(fixtures))
        self.assertEqual(sum(r['expected'] == 'accept' for r in inventory), 30)
        self.assertEqual(len(motion.motion_schema_paths(ROOT)), 4)
        self.assertTrue(all(p.is_file() for p in motion.motion_schema_paths(ROOT)))

    def test_missing_and_misclassified_fixture_never_pass(self):
        for prefix in ('contracts/fixtures/assets/motion-v2/', 'contracts/fixtures/assets/motion-v2/reviewed/'):
            fixtures = self.fixtures()
            positive = next(k for k in fixtures if k.startswith(prefix + 'valid/'))
            negative = next(k for k in fixtures if k.startswith(prefix + 'invalid/'))
            missing = dict(fixtures); del missing[positive]
            with self.assertRaisesRegex(ValueError, 'coverage drift'):
                motion.check_motion_fixtures(ROOT, read, missing, [])
            tampered = dict(fixtures); tampered[positive] = {'__unknown': True}
            with self.assertRaisesRegex(ValueError, 'expectation mismatch'):
                motion.check_motion_fixtures(ROOT, read, tampered, [])
            accepted_negative = dict(fixtures); accepted_negative[negative] = fixtures[positive]
            with self.assertRaisesRegex(ValueError, 'expectation mismatch'):
                motion.check_motion_fixtures(ROOT, read, accepted_negative, [])


if __name__ == '__main__':
    unittest.main()
