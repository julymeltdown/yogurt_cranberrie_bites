"""Synthetic inputs to the actual production processes() function, never real caps."""
import ast
import copy
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from receipt_proof import validate_full, zero, unique_pairs

HERE = Path(__file__).parent
INDEX = '/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d/owner-resource-receipts'
OWNER = 'fixture-peer'
GIB = 1024 ** 3
BIRTH = 'Thu Nov 5 10:00:00 2026'


class SyntheticReceipts(unittest.TestCase):
    def setUp(self):
        self.files = {
            '/v/roster': json.dumps({'issuer': 'Resource21d', 'management_membership_path': '/v/members', 'cohorts': [], 'closed_cohort_receipts': {}}),
            '/v/members': json.dumps({'entries': []}),
        }
        self.rows = {900: (1, 900, 1024, BIRTH, '/synthetic/python /tmp/mcr-fixture/owner.py')}
        self.a = {'output_directory': '/v/out', 'controller_pid': 900, 'global_roster_path': '/v/roster', 'process_cap': 4, 'rss_cap_bytes': GIB, 'kind': 'source'}
        self.namespace = None
        self.snapshots = []

    def record(self, **overrides):
        rec = {'ownerThreadId': OWNER, 'offerId': 'source-one', 'state': 'ACCEPTED', 'acceptedAtEpoch': 100,
               'kind': 'source', 'processCap': 4, 'rssCapBytes': GIB, 'remainingNewUpperBytes': 1024,
               'controller': None, 'guardControllers': [], 'receiptPaths': {'accept': '/v/accept', 'full': '/v/full'}}
        rec.update(overrides)
        self.files[INDEX + '/' + OWNER + '.json'] = json.dumps(rec)
        return rec

    def legacy(self, accepted=100):
        item = {'owner': OWNER, 'offerId': 'old-source', 'state': 'ACTUAL_ACCEPT', 'actualAccept': accepted,
                'kind': 'source', 'processCap': 4, 'rssCapGiB': 1, 'remainingNewConservativeUpperBytes': 2048,
                'knownRootMatchers': ['/tmp/mcr-old/']}
        self.files['/v/members'] = json.dumps({'entries': [item]})

    def closure(self, packet=None, **changes):
        if packet is None:
            packet = {'ownerThreadId': OWNER, 'offerId': 'old-source', 'acceptedAtEpoch': 100,
                      'actualFullEpoch': 200, 'workflows': 0, 'children': 0, 'guards': 0}
        raw = json.dumps(packet) + '\n'
        self.files['/v/full'] = raw
        value = {'state': 'FULL', 'ownerThreadId': OWNER, 'offerId': 'old-source', 'acceptedAtEpoch': 100,
                 'actualFullEpoch': 200, 'resourcesReleased': True, 'fullPath': '/v/full',
                 'fullSHA256': hashlib.sha256(raw.encode()).hexdigest()}
        value.update(changes)
        return value

    def run_census(self):
        files, rows = self.files, self.rows
        class VirtualPath:
            def __init__(self, value): self.value = str(value)
            def __str__(self): return self.value
            def __truediv__(self, value): return VirtualPath(self.value.rstrip('/') + '/' + str(value))
            @property
            def stem(self): return self.value.rsplit('/', 1)[-1].removesuffix('.json')
            def exists(self): return self.value in files
            def read_text(self): return files[self.value]
            def read_bytes(self): return files[self.value].encode()
            def stat(self): return SimpleNamespace(st_size=len(files[self.value].encode()))
            def glob(self, pattern):
                assert pattern == '*.json'
                return [VirtualPath(k) for k in files if k.startswith(self.value + '/') and k.endswith('.json')]
            def resolve(self): return self
        if self.namespace is None:
            tree = ast.parse((HERE / 'budget.py').read_text())
            actual = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'processes')
            self.namespace = {'json': json, 'Path': VirtualPath, 'os': SimpleNamespace(getpid=lambda: 900),
                              'time': SimpleNamespace(time=lambda: 1000), 'census': lambda: copy.deepcopy(rows),
                              'write': lambda out,name,value: self.snapshots.append(copy.deepcopy(value)), '_PROOF_CACHE': {}, 'zero': zero,
                              'unique_pairs': unique_pairs,
                              'retain_owner_index': lambda out,raw: {'ownerIndexSHA256':hashlib.sha256(raw).hexdigest(),'ownerIndexSnapshotPath':'/synthetic/retained-index.json'},
                              'validate_full': validate_full,
                              'digest': lambda p: hashlib.sha256(files[str(p)].encode()).hexdigest()}
            exec(compile(ast.Module(body=[actual], type_ignores=[]), 'actual-budget-processes', 'exec'), self.namespace)
        return self.namespace['processes'](self.a)

    def test_documented_aliases_keep_full_invisible_reservation(self):
        for state in ('ACCEPTED', 'ACCEPTED_RUNNING', 'ACCEPTED_BOOTSTRAP', 'ACCEPTED_GUARDS_STARTING', 'ACTUAL_ACCEPT', 'ACCEPTED_INNER_CENSUS', 'ACTUAL_ACCEPT_NO_FULL'):
            with self.subTest(state=state):
                self.record(state=state)
                sample = self.run_census()
                self.assertEqual(sample['declared_global_pids'], 8)
                self.assertEqual(sample['declared_global_rss'], 2 * GIB)
                self.assertEqual(sample['unresolved_accepted_reserved_pids'], 4)
                self.assertEqual(sample['other_accepted_remaining_new_bytes'], 1024)

    def test_malformed_owner_caps_clock_and_unknown_state_fail(self):
        for changes in ({'ownerThreadId': 'wrong'}, {'offerId': ''}, {'processCap': True}, {'rssCapBytes': None},
                        {'acceptedAtEpoch': None}, {'acceptedAtEpoch': 1001}, {'remainingNewUpperBytes': -1},
                        {'state': 'NOT_ACCEPTED'}, {'state': 'ACCEPTED_UNKNOWN_PHASE'}):
            with self.subTest(changes=changes):
                self.record(**changes)
                with self.assertRaises(AssertionError): self.run_census()

    def test_unknown_actual_pid_remains_strict(self):
        self.record()
        self.rows[901] = (1, 901, 1024, BIRTH, '/synthetic/python /tmp/mcr-unknown/controller.py')
        with self.assertRaisesRegex(AssertionError, 'unreconciled actual'): self.run_census()

    def test_live_known_birth_root_is_counted_once(self):
        controller = {'pid': 901, 'birth': BIRTH, 'executable': '/synthetic/python', 'script': '/tmp/mcr-peer/controller.py', 'outputRoot': '/tmp/mcr-peer'}
        self.record(controller=controller)
        self.rows[901] = (1, 901, 1024, BIRTH, '/synthetic/python /tmp/mcr-peer/controller.py')
        sample = self.run_census()
        self.assertEqual(sample['global_processes'], 2)
        self.assertEqual(sample['global_workflows'], 2)
        self.assertEqual(sample['unresolved_accepted_reserved_pids'], 0)
        controller['birth'] = 'reused PID'
        self.record(controller=controller)
        with self.assertRaisesRegex(AssertionError, 'birth/group'): self.run_census()
        controller['birth'] = BIRTH
        self.record(controller=controller)
        self.rows[901] = (1, 901, 1024, BIRTH, '<defunct>')
        with self.assertRaisesRegex(AssertionError, 'PID root no longer matches'): self.run_census()
        # A script-bound ordinary record need not have an unused accept path.
        self.rows[901] = (1, 901, 1024, BIRTH, '/synthetic/python /tmp/mcr-peer/controller.py')
        record = self.record(controller=controller, kind='ordinary', processCap=None, rssCapBytes=None)
        del record['receiptPaths']
        self.files[INDEX + '/' + OWNER + '.json'] = json.dumps(record)
        self.assertEqual(self.run_census()['global_processes'], 2)

    def test_ordinary_update_preserves_old_execution(self):
        self.legacy()
        self.record(offerId='ordinary-new', kind='ordinary', processCap=None, rssCapBytes=None, acceptedAtEpoch=300)
        sample = self.run_census()
        self.assertEqual(sample['declared_global_pids'], 8)
        self.assertEqual(sample['unresolved_accepted_reserved_pids'], 4)
        self.assertEqual(sample['other_accepted_remaining_new_bytes'], 3072)
        controller = {'pid':901,'birth':BIRTH,'executable':'/synthetic/python','script':'/tmp/mcr-peer/controller.py'}
        self.rows[901]=(1,901,1024,BIRTH,'/synthetic/python /tmp/mcr-peer/controller.py')
        self.record(offerId=None,kind='ordinary',processCap=None,rssCapBytes=None,acceptedAtEpoch=300,
                    controller=controller,executionWorkflow=False,authorizationReference='explicit ordinary booking',ordinaryBookingUpperBytes=1024)
        sample=self.run_census()
        self.assertEqual(sample['declared_global_pids'],8)
        self.assertEqual(sample['other_accepted_remaining_new_bytes'],3072)

    def test_exact_hashed_full_retires_only_original_acceptance(self):
        for old_epoch, expected in ((100, 4), (99, 8)):
            with self.subTest(old_epoch=old_epoch):
                self.legacy(accepted=old_epoch)
                self.record(offerId='ordinary-new', kind='ordinary', processCap=None, rssCapBytes=None, acceptedAtEpoch=300, closedHistory=[self.closure()])
                self.assertEqual(self.run_census()['declared_global_pids'], expected)
                if old_epoch==100:
                    proof=self.snapshots[-1]['matched_literal_history_full'][0]
                    self.assertEqual(proof['ownerIndexSHA256'],hashlib.sha256(self.files[INDEX+'/'+OWNER+'.json'].encode()).hexdigest())

    def test_forged_hash_packet_identity_and_resource_conflicts_fail(self):
        variants = [('hash', {'fullSHA256': '0' * 64}, None),
                    ('owner', {}, {'ownerThreadId': 'wrong'}),
                    ('offer', {}, {'offerId': 'different'}),
                    ('epoch', {}, {'acceptedAtEpoch': 101}),
                    ('resource', {}, {'children': [123]}),
                    ('resource-map', {}, {'resources': {'guards': 1}})]
        for label, changes, packet in variants:
            with self.subTest(label=label):
                self.legacy()
                closure = self.closure(packet=packet, **changes)
                self.record(offerId='ordinary-new', kind='ordinary', processCap=None, rssCapBytes=None, acceptedAtEpoch=300, closedHistory=[closure])
                with self.assertRaises(AssertionError): self.run_census()

    def test_jsonl_timestamp_is_not_acceptance_epoch(self):
        self.legacy()
        closure = self.closure()
        raw = json.dumps({'event': 'started', 'timestamp': 111, 'acceptedAtEpoch': 100}) + '\n' + json.dumps({'event': 'closed', 'timestamp': 222, 'resourcesReleased': True}) + '\n'
        self.files['/v/full'] = raw
        closure['fullSHA256'] = hashlib.sha256(raw.encode()).hexdigest()
        self.record(offerId='ordinary-new', kind='ordinary', processCap=None, rssCapBytes=None, acceptedAtEpoch=300, closedHistory=[closure])
        self.assertEqual(self.run_census()['declared_global_pids'], 4)

    def test_current_full_same_offer_different_epoch_cannot_close_old(self):
        self.legacy(accepted=99)
        self.record(offerId='old-source', state='FULL', acceptedAtEpoch=100, remainingNewUpperBytes=0)
        self.assertEqual(self.run_census()['declared_global_pids'], 8)

    def test_same_booking_full_cannot_mutate_even_with_rebound_hash(self):
        self.legacy()
        closure = self.closure()
        self.record(offerId='ordinary-new', kind='ordinary', processCap=None, rssCapBytes=None, acceptedAtEpoch=300, closedHistory=[closure])
        self.assertEqual(self.run_census()['declared_global_pids'], 4)
        packet = json.loads(self.files['/v/full']); packet['extra'] = 'changed after immutable closure'
        closure = self.closure(packet)
        self.record(offerId='ordinary-new', kind='ordinary', processCap=None, rssCapBytes=None, acceptedAtEpoch=300, closedHistory=[closure])
        with self.assertRaisesRegex(AssertionError, 'immutable historical FULL changed'): self.run_census()


if __name__ == '__main__':
    unittest.main(verbosity=2)
