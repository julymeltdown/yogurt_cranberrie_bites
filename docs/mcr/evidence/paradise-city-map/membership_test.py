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
        membership._verified.clear()
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
        patch.dict(os.environ, MCR_VENU_OTHER_NEW=str(5 * 1024**3)).start()

    def run_sample(self):
        (self.records / (membership.OWNER + '.json')).write_text(json.dumps(self.record))
        (self.base / 'resource-membership-current.json').write_text(json.dumps({'entries': self.legacy}))
        return membership.sample()

    def test_absent_pid_does_not_release_accepted_reservation(self):
        self.rows = {}
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)

    def yield_record(self):
        value = dict(ownerThreadId='yielding', offerId='future', state='UNREADY_YIELD', kind='source',
                     acceptedAtEpoch=None, remainingNewUpperBytes=0, controller=None, guardControllers=[], workflow=0)
        (self.records / 'yielding.json').write_text(json.dumps(value))
        return value

    def test_unready_yield_is_not_a_workflow_reservation(self):
        self.yield_record()
        result = self.run_sample()
        self.assertEqual(result['owners'], [membership.OWNER])
        self.assertEqual(result['reservedProcesses'], 24)

    def test_unready_yield_cannot_hide_positive_or_unknown_work(self):
        value = self.yield_record()
        for patch_value in (dict(acceptedAtEpoch=10), dict(remainingNewUpperBytes=1), dict(controller=self.record['controller']), dict(workflow=1), dict(workflow=1, resources=0)):
            (self.records / 'yielding.json').write_text(json.dumps(dict(value, **patch_value)))
            with self.assertRaisesRegex(AssertionError, 'yield cannot hide'):
                self.run_sample()
        self.yield_record()
        self.rows[900] = dict(parent=1, birth='Thu Nov 5 00:00:01 2026', rss=4096, command='python3 /private/tmp/mcr-yielding/payload.py')
        with self.assertRaisesRegex(AssertionError, 'unresolved workflow'):
            self.run_sample()

    def test_unready_yield_needs_actual_prior_full_to_reconcile_legacy(self):
        value = self.yield_record()
        self.legacy = [dict(owner='yielding', cohort='previous', kind='source', state='ACTUAL_ACCEPT_NO_FULL',
                            acceptedAtEpoch=9, processCap=80, rssCapGiB=1, remainingNewConservativeUpperBytes=0)]
        with self.assertRaises(AssertionError):
            self.run_sample()
        prior = dict(ownerThreadId='yielding', offerId='previous', state='FULL', acceptedAtEpoch=9, actualFullEpoch=20, resources=0)
        (self.records / 'yielding.json').write_text(json.dumps(dict(value, previousActualExecution=prior)))
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
        self.rows[901]['command'] = 'python3 -B scripts/tests.py'
        with self.assertRaisesRegex(AssertionError, 'ordinary role cannot cover runtime payload'):
            self.run_sample()

    def test_reused_pid_fails_closed(self):
        self.run_sample()
        self.rows[777]['birth'] = 'Thu Nov 5 00:00:02 2026'
        with self.assertRaisesRegex(AssertionError, 'birth mismatch'):
            self.run_sample()
        self.rows[777]['birth'] = 'Thu Nov 5 00:00:00 2026'
        self.rows[777].update(command='<defunct>', status='Z')
        with self.assertRaisesRegex(AssertionError, 'independently verified'):
            self.run_sample()

    def test_terminal_identity_requires_this_guard_prior_proof_and_keeps_full_cap(self):
        self.rows[777].update(command='(Python)', status='Z')
        with self.assertRaisesRegex(AssertionError, 'independently verified'):
            self.run_sample()
        self.rows[777]['command'] = 'python3 /owned.py'
        self.run_sample()
        self.rows[777].update(command='(Python)', status='S')
        with self.assertRaisesRegex(AssertionError, 'independently verified'):
            self.run_sample()
        self.rows[777]['status'] = 'Z'
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)
        self.assertEqual(self.run_sample()['actualPids'], [777])
        self.record['offerId'] = 'different-offer'
        with self.assertRaisesRegex(AssertionError, 'independently verified'):
            self.run_sample()

    def test_inline_metadata_filename_is_not_an_executed_script(self):
        self.rows[900] = dict(parent=1, birth='Thu Nov 5 00:00:01 2026', rss=4096,
                             command="/bin/zsh -c python3 - <<'PY'\np='/private/tmp/mcr-metadata/controller.py'\nPY")
        self.assertEqual(self.run_sample()['owners'], [membership.OWNER])

    def test_exact_new_full_resamples_a_stale_terminal_claim(self):
        self.rows[777].update(command='(Python)', status='Z')
        def after_records():
            latest = dict(self.record, state='FULL', resources=0, actualFullEpoch=20)
            (self.records / (membership.OWNER + '.json')).write_text(json.dumps(latest))
            return self.rows
        with patch.object(membership, 'processes', after_records):
            result = self.run_sample()
        self.assertEqual(result['owners'], [])
        self.assertEqual(result['reservedProcesses'], 0)

    def test_incomplete_or_other_booking_full_cannot_release_terminal_claim(self):
        self.rows[777].update(command='(Python)', status='Z')
        for fields in (dict(resources=1), dict(offerId='other'), dict(acceptedAtEpoch=11)):
            with self.subTest(fields=fields):
                def after_records():
                    latest = dict(self.record, state='FULL', resources=0, actualFullEpoch=20)
                    latest.update(fields)
                    (self.records / (membership.OWNER + '.json')).write_text(json.dumps(latest))
                    return self.rows
                with patch.object(membership, 'processes', after_records):
                    with self.assertRaisesRegex(AssertionError, 'independently verified'):
                        self.run_sample()

    def test_unknown_relative_python_script_still_fails_closed(self):
        self.rows[900] = dict(parent=1, birth='Thu Nov 5 00:00:01 2026', rss=4096,
                             command='python3 -B scripts/unknown.py --worktree /mcr-worktrees/unknown')
        with self.assertRaisesRegex(AssertionError, 'unresolved workflow'):
            self.run_sample()

    def test_factual_ordinary_parent_counts_without_hiding_runtime_siblings(self):
        self.ordinary_publisher()
        self.rows[899] = dict(parent=1, birth='Thu Nov 5 00:00:01 2026', rss=4096,
                             command='/bin/zsh -c python3 /mcr-evidence/publish.py')
        self.rows[900]['parent'] = 899
        self.assertEqual(self.run_sample()['ordinaryActualPids'], [899, 900])
        self.assertEqual(self.run_sample()['reservedProcesses'], 24)
        self.rows[901] = dict(parent=899, birth='Thu Nov 5 00:00:02 2026', rss=4096,
                             command='node /mcr-worktrees/unknown/tests.mjs')
        with self.assertRaisesRegex(AssertionError, 'unresolved workflow'):
            self.run_sample()


if __name__ == '__main__':
    unittest.main()
