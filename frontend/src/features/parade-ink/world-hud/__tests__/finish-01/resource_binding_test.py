"""Two new binding regressions; retain the eight qualified read-consistency cases."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from resource_snapshot import receipt_binding, receipt_revision


class BindingRevision(unittest.TestCase):
    def test_atomic_bookkeeping_replacement_keeps_actual_authority_revision(self):
        record = {'ownerThreadId': 'peer', 'offerId': 'active', 'state': 'ACCEPTED',
                  'kind': 'source', 'processCap': 24, 'rssCapBytes': 100,
                  'remainingNewUpperBytes': 50, 'controller': {'pid': 9, 'birth': 'actual', 'script': '/actual.py'},
                  'ordinaryObservation': {'actualpid': 12, 'birth': 'ordinary', 'role': 'metadata', 'observedAt': 1}}
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            path = directory / 'peer.json'
            path.write_text(json.dumps(record))
            first = receipt_revision(directory, [])
            record.update(publishedAtEpoch=99, ordinaryObservationHistory=[{'endedAt': 2}],
                          admissionDisposition={'state': 'UNREADY_YIELD', 'offerId': 'future'})
            record['ordinaryObservation']['observedAt'] = 500
            replacement = directory / '.next'
            replacement.write_text(json.dumps(record))
            replacement.replace(path)
            self.assertEqual(first, receipt_revision(directory, []))
            self.assertEqual(receipt_binding(record)['processCap'], 24)
            self.assertEqual(receipt_binding(record)['state'], 'ACCEPTED')
            record['processCap'] = 25
            path.write_text(json.dumps(record))
            self.assertNotEqual(first, receipt_revision(directory, []))

    def test_caps_identity_lifecycle_and_closed_release_proof_remain_binding(self):
        record = {'ownerThreadId': 'peer', 'offerId': 'active', 'state': 'ACCEPTED',
                  'kind': 'browser', 'acceptedAtEpoch': 1, 'workDeadlineEpoch': 2,
                  'fullDeadlineEpoch': 3, 'actualFullEpoch': None, 'processCap': 24,
                  'rssCapBytes': 100, 'remainingNewUpperBytes': 50, 'resources': 1,
                  'controller': {'pid': 9, 'birth': 'actual', 'executable': '/Python', 'script': '/actual.py', 'cwd': '/actual'},
                  'guardControllers': [{'pid': 10, 'birth': 'guard', 'script': '/guard.py'}],
                  'closedOffers': [{'offerId': 'prior', 'actualFullEpoch': 0.5, 'immutableFULLsha256': 'old'}],
                  'ordinaryObservation': {'actualpid': 12, 'birth': 'ordinary', 'executable': '/Python', 'script': '/ordinary.py', 'cwd': '/ordinary', 'role': 'metadata', 'booking': 'same'}}
        original = receipt_binding(record)
        for key in record:
            changed = copy.deepcopy(record)
            changed[key] = {'different': True}
            with self.subTest(authority=key):
                self.assertNotEqual(original, receipt_binding(changed))
        for key in ['actualpid', 'birth', 'executable', 'script', 'cwd', 'role', 'booking', 'endedAt', 'exitCode']:
            changed = copy.deepcopy(record)
            changed['ordinaryObservation'][key] = 'different'
            with self.subTest(ordinary=key):
                self.assertNotEqual(original, receipt_binding(changed))


if __name__ == '__main__':
    unittest.main()
