"""Necessary regression cases for mixed receipt/process observations; finite offer only."""
import json
import unittest
from resource_snapshot import consistent_sample


class ConsistentCensus(unittest.TestCase):
    def sequence(self, values):
        iterator = iter(values)
        return lambda: next(iterator)

    def test_stable_pass_retains_every_capacity_field(self):
        original = {'owners': ['peer'], 'reservedProcesses': 24, 'reservedRSSBytes': 123, 'otherNewUpperBytes': 456}
        result = consistent_sample(lambda: original, lambda: 'same')
        self.assertEqual({key: result[key] for key in original}, original)
        self.assertEqual(result['receiptReadConsistency']['attempts'], 1)
        self.assertNotIn('receiptReadConsistency', original)

    def test_stable_unknown_root_is_not_retried(self):
        calls = []
        def fail():
            calls.append(True)
            raise AssertionError('unresolved workflow roots')
        with self.assertRaisesRegex(AssertionError, 'unresolved'):
            consistent_sample(fail, lambda: 1)
        self.assertEqual(len(calls), 1)

    def test_new_registration_requires_complete_strict_second_census(self):
        calls = []
        def read():
            calls.append(True)
            if len(calls) == 1:
                raise AssertionError('unresolved workflow roots')
            return {'owners': ['newly_registered'], 'reservedProcesses': 48}
        result = consistent_sample(read, self.sequence([0, 1, 1, 1]))
        self.assertEqual(len(calls), 2)
        self.assertEqual(result['reservedProcesses'], 48)
        self.assertEqual(result['receiptReadConsistency']['unstableObservations'][0]['strictObservation'], 'unresolved workflow roots')

    def test_pass_from_changed_set_does_not_bypass_new_cap_failure(self):
        calls = []
        def read():
            calls.append(True)
            if len(calls) == 1:
                return {'reservedProcesses': 24}
            raise AssertionError('global workflow reservations')
        with self.assertRaisesRegex(AssertionError, 'global workflow'):
            consistent_sample(read, self.sequence([0, 1, 1, 1]))
        self.assertEqual(len(calls), 2)

    def test_stable_script_mismatch_is_not_ignored(self):
        def fail():
            raise AssertionError('resource controller identity mismatch')
        with self.assertRaisesRegex(AssertionError, 'identity mismatch'):
            consistent_sample(fail, lambda: 9)

    def test_changed_identity_requires_strict_verified_replacement(self):
        reads = iter([AssertionError('resource PID birth mismatch'), {'owners': ['actual'], 'actualPids': [55]}])
        def read():
            value = next(reads)
            if isinstance(value, Exception):
                raise value
            return value
        result = consistent_sample(read, self.sequence([3, 4, 4, 4]))
        self.assertEqual(result['actualPids'], [55])
        self.assertEqual(result['receiptReadConsistency']['attempts'], 2)

    def test_unstable_set_is_bounded_without_sleep_or_workload_restart(self):
        revisions = iter(range(6))
        calls = []
        def read():
            calls.append(True)
            return {'owners': ['still_reserved']}
        with self.assertRaisesRegex(RuntimeError, 'all bounded full censuses'):
            consistent_sample(read, lambda: next(revisions))
        self.assertEqual(len(calls), 3)

    def test_stable_malformed_receipt_still_fails(self):
        def fail():
            raise json.JSONDecodeError('missing record', '', 0)
        with self.assertRaises(json.JSONDecodeError):
            consistent_sample(fail, lambda: 0)


if __name__ == '__main__':
    unittest.main()
