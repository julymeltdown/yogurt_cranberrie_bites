"""Resource admission regressions with isolated metadata, no peer processes."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import membership


class MembershipTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.base = Path(self.directory.name)
        self.records = self.base / 'receipts'
        self.records.mkdir()
        policy = dict(global_=4, source=3, browser=2, warmRustTarget=1, sumPIDCaps=96,
                      sumRSSBytes=18 * 1024**3, ordinaryFuturePositiveUpperBytes=0)
        policy['global'] = policy.pop('global_')
        (self.base / 'resource-receipt-interface.json').write_text(json.dumps({'limits': policy}))
        self.legacy = []
        self.record = dict(ownerThreadId=membership.OWNER, offerId='own', state='ACCEPTED', acceptedAtEpoch=10,
                           kind='browser', processCap=24, rssCapBytes=3 * 1024**3, remainingNewUpperBytes=0,
                           controller=dict(pid=777, birth='Thu Nov 5 00:00:00 2026', script='/owned.py'))
        self.rows = {777: dict(parent=1, birth='Thu Nov 5 00:00:00 2026', rss=4096, command='python3 /owned.py')}
        self.addCleanup(patch.stopall)
        patch.object(membership, 'BASE', self.base).start()
        patch.object(membership, 'DIRECTORY', self.records).start()
        patch.object(membership, 'processes', lambda: self.rows).start()
        patch.dict(os.environ, MCR_PI_OTHER_NEW=str(5 * 1024**3)).start()

    def run_sample(self):
        (self.records / (membership.OWNER + '.json')).write_text(json.dumps(self.record))
        (self.base / 'resource-membership-current.json').write_text(json.dumps({'entries': self.legacy}))
        return membership.sample()

    def test_absent_pid_does_not_release_accepted_reservation(self):
        self.rows = {}
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)

    def test_modern_full_entry_does_not_restore_a_closed_claim(self):
        self.legacy = [dict(ownerThreadId='closed', offerId='closed', state='FULL', processCap=96)]
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)

    def test_controller_and_child_count_once(self):
        self.rows[778] = dict(parent=777, birth='Thu Nov 5 00:00:01 2026', rss=4096, command='node /mcr-worktrees/x/test.mjs')
        result = self.run_sample()
        self.assertEqual(result['owners'], [membership.OWNER])
        self.assertEqual(result['actualPids'], [777, 778])

    def test_missing_new_receipt_preserves_legacy_cap(self):
        self.legacy = [dict(owner='legacy', cohort='old', kind='source', state='ACTUAL_ACCEPT_NO_FULL',
                            processCap=80, rssCapGiB=1, remainingNewConservativeUpperBytes=0)]
        with self.assertRaises(AssertionError):
            self.run_sample()

    def test_unknown_scoped_runtime_fails_closed(self):
        self.rows[900] = dict(parent=1, birth='Thu Nov 5 00:00:01 2026', rss=4096, command='node /mcr-worktrees/unknown/run.mjs')
        with self.assertRaisesRegex(AssertionError, 'unresolved workflow'):
            self.run_sample()

    def test_wrong_script_reports_owner_and_pid_without_command(self):
        self.rows[777]['command'] = 'python3 /unrelated.py secret-token'
        with self.assertRaisesRegex(AssertionError, 'resource controller identity mismatch') as caught:
            self.run_sample()
        self.assertIn(membership.OWNER, str(caught.exception))
        self.assertIn('777', str(caught.exception))
        self.assertNotIn('secret-token', str(caught.exception))

    def ordinary_publisher(self):
        record = dict(ownerThreadId='publisher', state='FULL', kind='ordinary',
                      controller=dict(pid=900, birth='Thu Nov 5 00:00:01 2026', script='/mcr-evidence/publish.py'))
        (self.records / 'publisher.json').write_text(json.dumps(record))
        self.rows[900] = dict(parent=1, birth='Thu Nov 5 00:00:01 2026', rss=4096,
                              command='python3 /mcr-evidence/publish.py')

    def test_live_full_ordinary_is_counted_without_workflow_reservation(self):
        self.ordinary_publisher()
        result = self.run_sample()
        self.assertEqual(result['owners'], [membership.OWNER])
        self.assertEqual(result['reservedProcesses'], 24)
        self.assertEqual(result['actualPids'], [777, 900])
        self.assertEqual(result['ordinaryActualPids'], [900])

    def test_ordinary_script_mismatch_fails_closed(self):
        self.ordinary_publisher()
        self.rows[900]['command'] = 'python3 /mcr-evidence/unknown.py'
        with self.assertRaisesRegex(AssertionError, 'ordinary script identity mismatch'):
            self.run_sample()

    def test_ordinary_cannot_hide_compiler_or_test_child(self):
        self.ordinary_publisher()
        self.rows[901] = dict(parent=900, birth='Thu Nov 5 00:00:02 2026', rss=4096,
                              command='node /mcr-worktrees/unknown/tests.mjs')
        with self.assertRaisesRegex(AssertionError, 'ordinary role cannot cover runtime payload'):
            self.run_sample()

    def test_ordinary_only_record_does_not_erase_legacy_reservation(self):
        (self.records / 'legacy.json').write_text(json.dumps({'ordinaryObservation': {
            'actualpid': 999, 'birth': 'Thu Nov 5 00:00:01 2026', 'cwd': '/mcr-worktrees/legacy',
            'role': 'ordinary Git metadata', 'observedAt': 12, 'commandDigest': '0' * 64, 'booking': 'ordinary-only'}}))
        self.legacy = [dict(owner='legacy', kind='source', state='ACTUAL_ACCEPT_NO_FULL', actualAccept=9,
                            processCap=80, rssCapGiB=1, remainingNewConservativeUpperBytes=0)]
        with self.assertRaises(AssertionError):
            self.run_sample()

    def test_newer_ordinary_null_controller_does_not_release_old_execution(self):
        (self.records / 'legacy.json').write_text(json.dumps(dict(ownerThreadId='legacy', kind='ordinary',
            state='ACCEPTED', acceptedAtEpoch=20, controller=None)))
        self.legacy = [dict(owner='legacy', cohort='old-source', kind='source', state='ACTUAL_ACCEPT_NO_FULL', actualAccept=9,
                            processCap=80, rssCapGiB=1, remainingNewConservativeUpperBytes=0)]
        with self.assertRaises(AssertionError):
            self.run_sample()

    def historical_full(self):
        full = self.base / 'old-full.json'
        full.write_text(json.dumps(dict(offerId='old-source', state='FULL', acceptedEpoch=9, resources={'source': 0}, validation='FAIL')))
        import hashlib
        record = dict(ownerThreadId='legacy', kind='ordinary', state='FULL', acceptedAtEpoch=20, controller=None,
            closedHistory=[dict(ownerThreadId='legacy', offerId='old-source', state='FULL', acceptedAtEpoch=9,
                                fullPath=str(full), sha256=hashlib.sha256(full.read_bytes()).hexdigest())])
        self.legacy = [dict(owner='legacy', cohort='old-source', kind='source', state='ACTUAL_ACCEPT_NO_FULL', actualAccept=9,
                            processCap=80, rssCapGiB=1, remainingNewConservativeUpperBytes=0)]
        return record

    def test_exact_hashed_failed_full_releases_history_without_claiming_pass(self):
        record = self.historical_full()
        (self.records / 'legacy.json').write_text(json.dumps(record))
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)

    def test_wrong_historical_full_hash_fails_closed(self):
        record = self.historical_full(); record['closedHistory'][0]['sha256'] = '0' * 64
        (self.records / 'legacy.json').write_text(json.dumps(record))
        with self.assertRaisesRegex(AssertionError, 'historical FULL hash mismatch'):
            self.run_sample()

    def test_wrong_historical_accepted_epoch_does_not_release_reservation(self):
        record = self.historical_full(); record['closedHistory'][0]['acceptedAtEpoch'] = 10
        (self.records / 'legacy.json').write_text(json.dumps(record))
        with self.assertRaises(AssertionError):
            self.run_sample()

    def test_running_alias_retains_all_declared_reservations(self):
        self.record['state'] = 'ACCEPTED_RUNNING'
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)

    def test_starting_alias_with_null_controller_retains_reservations(self):
        self.record.update(state='ACCEPTED_GUARDS_STARTING', controller=None)
        self.rows = {}
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)

    def test_bootstrap_alias_retains_caps(self):
        self.record.update(state='ACCEPTED_BOOTSTRAP', controller=None)
        self.rows = {}
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)

    def test_actual_accept_alias_retains_caps(self):
        self.record['state'] = 'ACTUAL_ACCEPT'
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)

    def test_reused_pid_fails_closed(self):
        self.rows[777]['birth'] = 'Thu Nov 5 00:00:02 2026'
        with self.assertRaisesRegex(AssertionError, 'birth mismatch'):
            self.run_sample()


if __name__ == '__main__':
    unittest.main()
