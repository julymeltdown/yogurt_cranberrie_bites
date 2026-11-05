"""Deterministic compatibility reconstructions; never live peer qualification."""
import copy
import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('receipt_consumer', Path(__file__).with_name('receipt-consumer.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

OWNER = 'owner-a'
RUNTIME = '/framework/Resources/Python.app/Contents/MacOS/Python'
LAUNCHER = '/framework/bin/python3'
SCRIPT = '/private/work/guard.py'


def claim(**patch):
    return dict(dict(ownerThreadId=OWNER, offerId='offer-a', state='ACCEPTED',
                     acceptedAtEpoch=10, workDeadlineEpoch=130, fullDeadlineEpoch=190,
                     kind='source', processCap=8, rssCapBytes=512 * 1024**2,
                     remainingNewUpperBytes=16 * 1024**2, controller=None), **patch)


def control(pid=10, **patch):
    return dict(dict(pid=pid, birth='Thu Nov 5 10:00:00 2026', executable=RUNTIME,
                     cwd='/private/work', script=SCRIPT), **patch)


def row(parent=1, **patch):
    return dict(dict(parent=parent, birth='Thu Nov  5 10:00:00 2026', rss=1024,
                     executable=RUNTIME, cwd='/private/work',
                     argv=[LAUNCHER, '-B', 'guard.py'], commandDigest='measured'), **patch)


def packet(**patch):
    return dict(dict(ownerThreadId=OWNER, offerId='offer-a', acceptedAtEpoch=10,
                     state='FULL', actualFullEpoch=20, resourcesReleased=True), **patch)


def binding(data, **patch):
    return dict(dict(ownerThreadId=OWNER, offerId='offer-a', acceptedAtEpoch=10,
                     actualFullEpoch=20, fullPath='/private/full.json',
                     fullSha256=hashlib.sha256(data).hexdigest()), **patch)


class NormalizationTests(unittest.TestCase):
    def test_active_aliases_retain_every_cap(self):
        for state in ('ACCEPTED', 'ACCEPTED_RUNNING', 'ACCEPTED_GUARDS_STARTING', 'ACTUAL_ACCEPT_NO_FULL', 'ACCEPTED_BOOTSTRAP', 'ACCEPTED_INNER_CENSUS', 'ACTUAL_ACCEPT'):
            with self.subTest(state=state):
                p = c.project(c.reconcile([claim(state=state)]), {}, unknown_candidates=[])
                self.assertEqual((p['globalWorkflows'], p['source'], p['reservedProcesses']), (1, 1, 8))
                self.assertEqual(p['reservedRSSBytes'], 512 * 1024**2)

    def test_ordinary_null_before_arithmetic(self):
        r = claim(kind='ordinary', processCap=None, rssCapBytes=None, controller=None,
                  acceptedAtEpoch=None, remainingNewUpperBytes=None)
        original = copy.deepcopy(r)
        n = c.normalize(r)
        self.assertFalse(n['_active'])
        self.assertEqual(r, original)
        self.assertEqual(n['state'], 'ACCEPTED')

    def test_observation_only_invents_no_state_or_cap(self):
        n = c.normalize(dict(ownerThreadId=OWNER, ordinaryObservation={'pid': 20}))
        self.assertNotIn('state', n)
        self.assertNotIn('processCap', n)
        self.assertFalse(n['_active'])

    def test_unknown_kind_or_state_fail_closed(self):
        for patch in ({'kind': None}, {'kind': 'mystery'}, {'state': 'IDLE'}, {'rssCapBytes': float('nan')}, {'processCap': True}):
            with self.subTest(patch=patch), self.assertRaises(c.ReceiptError):
                c.normalize(claim(**patch))

    def test_bounded_reader_and_owner_fidelity(self):
        with tempfile.TemporaryDirectory() as directory:
            p = Path(directory) / (OWNER + '.json')
            p.write_text(json.dumps(claim()))
            self.assertEqual(len(c.load_receipts(directory)), 1)
            with self.assertRaises(c.ReceiptError):
                c.load_receipts(directory, max_bytes=4)
            with self.assertRaises(c.ReceiptError):
                c.load_receipts(directory, max_files=0)
            p.write_text(json.dumps(claim(ownerThreadId='someone-else')))
            with self.assertRaises(c.ReceiptError):
                c.load_receipts(directory)
            p.write_text('{"ownerThreadId":"a","ownerThreadId":"b"}')
            with self.assertRaises(c.ReceiptError):
                c.load_receipts(directory)


class ReconciliationTests(unittest.TestCase):
    def test_absence_and_current_full_do_not_release_legacy(self):
        for current in ([], [claim(state='FULL')], [claim(state='DECLINED')]):
            with self.subTest(current=current):
                self.assertEqual(len(c.reconcile(current, [claim()])['claims']), 1)

    def test_previous_unclosed_claim_survives_missing_current(self):
        self.assertEqual(len(c.reconcile([], previous=[claim()])['claims']), 1)

    def test_newer_ordinary_does_not_erase_old_execution(self):
        # Reconstructs allocator/producer's reported ordinary20 versus legacy9.
        old = claim(state='ACTUAL_ACCEPT_NO_FULL', offerId='old-source', acceptedAtEpoch=9)
        current = claim(kind='ordinary', acceptedAtEpoch=20, controller=None)
        self.assertEqual(c.reconcile([current], [old])['claims'][0]['offerId'], 'old-source')

    def test_missing_offer_does_not_equal_missing_offer(self):
        with self.assertRaises(c.ReceiptError):
            c.reconcile([claim(kind='ordinary', offerId=None)], [claim(offerId=None)])

    def test_different_offers_are_both_charged(self):
        x = c.reconcile([claim(offerId='new')], [claim(offerId='old')])
        self.assertEqual(len(x['claims']), 2)

    def test_same_offer_clock_and_deadline_reset_rejected(self):
        for patch in ({'acceptedAtEpoch': 11}, {'workDeadlineEpoch': 131}, {'fullDeadlineEpoch': 191}):
            with self.subTest(patch=patch), self.assertRaises(c.ReceiptError):
                c.reconcile([claim(**patch)], previous=[claim()])
        with self.assertRaises(c.ReceiptError):
            c.reconcile([claim(acceptedAtEpoch=11)], [claim()])

    def test_same_offer_execution_kind_cannot_change_lane(self):
        for kind in ('browser','target','copied-native'):
            with self.subTest(kind=kind), self.assertRaises(c.ReceiptError):
                c.reconcile([claim(kind=kind)], previous=[claim()])
        # Ordinary overlays continue retaining the prior execution kind.
        result = c.reconcile([claim(kind='ordinary')], previous=[claim()])
        self.assertEqual(result['claims'][0]['kind'],'source')

    def test_no_reduction_by_current_caps(self):
        r = c.reconcile([claim(processCap=2)], [claim()])
        self.assertEqual(r['claims'][0]['processCap'], 8)

    def test_exact_closed_history_retires_only_bound_claim(self):
        data = json.dumps(packet()).encode()
        for alias in ('closedOffers', 'closedReceipts', 'closedHistory'):
            with self.subTest(alias=alias):
                r = claim(kind='ordinary', **{alias: [binding(data)]})
                result = c.reconcile([r], [claim()], reader=lambda p, n: data)
                self.assertEqual(result['claims'], [])
                self.assertEqual(len(result['verifiedFull']), 1)

    def test_closed_history_other_offer_or_acceptance_keeps_reservation(self):
        data = json.dumps(packet()).encode()
        for patch in ({'offerId': 'other'}, {'acceptedAtEpoch': 11}):
            r = claim(kind='ordinary', closedReceipts=[binding(data, **patch)])
            self.assertEqual(len(c.reconcile([r], [claim()], reader=lambda p,n:data)['claims']), 1)


class PacketTests(unittest.TestCase):
    def verify(self, value, **kwargs):
        data = json.dumps(value).encode()
        return c.verify_full(binding(data), reader=lambda p, n: data, **kwargs)

    def test_failed_validation_can_still_be_literal_full(self):
        self.assertEqual(self.verify(packet(validation='PARTIAL_FAIL'))['acceptedAtEpoch'], 10)

    def test_bad_sha_owner_epoch_and_nonzero_fail(self):
        data = json.dumps(packet()).encode()
        for h in (binding(data, fullSha256='0'*64), binding(data, ownerThreadId='b'),
                  binding(data, acceptedAtEpoch=11), binding(data, actualFullEpoch=21),
                  binding(data, fullSHA='0'*64),
                  binding(data, fullPath='relative.json')):
            with self.subTest(h=h), self.assertRaises(c.ReceiptError):
                c.verify_full(h, reader=lambda p,n:data)
        for value in (packet(state='PASS'), packet(resourcesReleased=False),
                      packet(resourcesReleased={'pids':1}), packet(resourcesReleased=True,resources={'pids':1}), packet(resourcesReleased={'pids':True})):
            with self.subTest(value=value), self.assertRaises(c.ReceiptError):
                self.verify(value)

    def test_explicit_zero_shapes(self):
        for value in (True, 0, [], {'pids':0, 'groups':[], 'ports':0}, [0,0]):
            with self.subTest(value=value):
                self.verify(packet(resourcesReleased=value))

    def test_jsonl_is_format_aware_and_requires_exact_adapter(self):
        records = [dict(event='ACCEPT', at='different-event-time'), packet(event='FULL')]
        data = b'\n'.join(json.dumps(x).encode() for x in records)
        h = binding(data, fullPath='/private/full.jsonl')
        with self.assertRaises(c.ReceiptError):
            c.verify_full(h, reader=lambda p,n:data)
        result = c.verify_full(h, reader=lambda p,n:data,
                               adapter=lambda records, h: records[-1])
        self.assertEqual(result['actualFullEpoch'], 20)
        with self.assertRaises(c.ReceiptError):
            c.verify_full(h, reader=lambda p,n:data,
                          adapter=lambda records, h: dict(records[-1], acceptedAtEpoch=11))

    def test_owner_index_attested_proof_is_distinct_and_hashed(self):
        # Reconstructs real ART231 envelope semantics: event time != bootstrap.
        records = [dict(event='ACCEPT', offer='offer-a', at='event-log-time'),
                   dict(event='FULL', at='1970-01-01T00:00:20+00:00', status='FAIL',
                        allChildrenExited=True, resourceGuardianExited=True, tempAbsent=True)]
        data = b'\n'.join(json.dumps(x).encode() for x in records)
        h = binding(data, fullPath='/private/full.jsonl', resourcesReleased=True)
        index = json.dumps(dict(ownerThreadId=OWNER, closedOffers=[h])).encode()
        proof = c.verify_attested_full(h, index, reader=lambda p,n:data)
        self.assertEqual(proof['proofKind'], 'OWNER_INDEX_ATTESTED')
        self.assertEqual(proof['ownerIndexSHA256'], hashlib.sha256(index).hexdigest())
        self.assertEqual(proof['acceptedAtEpoch'], 10)
        for event_patch in ({'acceptedAtEpoch':10.001}, {'ownerThreadId':'other'}, {'offerId':'other'}):
            with self.subTest(event_patch=event_patch), self.assertRaises(c.ReceiptError):
                changed = copy.deepcopy(records)
                changed[0].update(event_patch)
                bad = b'\n'.join(json.dumps(x).encode() for x in changed)
                bh = binding(bad, fullPath='/private/full.jsonl', resourcesReleased=True)
                bi = json.dumps(dict(ownerThreadId=OWNER, closedOffers=[bh])).encode()
                c.verify_attested_full(bh, bi, reader=lambda p,n:bad)
        with self.assertRaises(c.ReceiptError):
            c.verify_attested_full(h, b'{"ownerThreadId":"other"}', reader=lambda p,n:data)
        with self.assertRaises(c.ReceiptError):
            c.verify_attested_full(h, json.dumps(dict(ownerThreadId=OWNER,closedOffers=[])).encode(), reader=lambda p,n:data)

    def test_bounded_packet_reader(self):
        data = json.dumps(packet()).encode()
        with self.assertRaises(c.ReceiptError):
            c.verify_full(binding(data), reader=lambda p,n:data, max_bytes=4)
        with self.assertRaises(c.ReceiptError):
            c.verify_full(binding(data), reader=lambda p,n: (_ for _ in ()).throw(FileNotFoundError()))


class ProjectionTests(unittest.TestCase):
    def project(self, records=None, rows=None, **kwargs):
        return c.project(c.reconcile(records or [claim(controller=control())]),
                         rows or {10:row()}, unknown_candidates=kwargs.pop('unknown_candidates', [10]), **kwargs)

    def test_native10_working_invocation_contract_reconstruction(self):
        # Source77cfe94, native10-invocation.json published651159ec. Recorded
        # controller invocation, with synthetic process/RSS snapshot, not replay.
        cwd = '/Users/lhs/Desktop/mcr-worktrees/fe-runtime-preferences'
        script = cwd + '/frontend/src/app/__tests__/parade-ink-foundation/controller.py'
        runtime = '/Library/Frameworks/Python.framework/Versions/3.14/Resources/Python.app/Contents/MacOS/Python'
        launcher = '/Library/Frameworks/Python.framework/Versions/3.14/bin/python3'
        descriptor = control(88733,birth='Thu Nov 5 10:19:15 2026',executable=runtime,cwd=cwd,script=script)
        r = claim(ownerThreadId='01a1a26e-1de2-7b41-a294-c9a79cfded9b',
                  offerId='PI01-FOCUSED-NATIVE10-0118',kind='browser',
                  acceptedAtEpoch=1793841555.855031,workDeadlineEpoch=1793841855.855031,
                  fullDeadlineEpoch=1793841915.855031,processCap=24,rssCapBytes=3758096384,
                  remainingNewUpperBytes=109051904,controller=descriptor)
        p = self.project(records=[r],rows={88733:row(birth=descriptor['birth'],executable=runtime,cwd=cwd,argv=[launcher,'-B',script])},unknown_candidates=[88733])
        self.assertEqual((p['browser'],p['reservedProcesses']), (1,24))
        self.assertEqual(p['actualPids'], [88733])

    def test_python_runtime_relative_program_actual_cwd(self):
        p = self.project()
        self.assertEqual(p['actualPids'], [10])
        self.assertNotEqual(LAUNCHER, RUNTIME)

    def test_missing_wrong_cwd_launcher_or_pid_reuse_fails(self):
        for patch in ({'cwd':None}, {'cwd':'/else'}, {'executable':LAUNCHER}, {'birth':'reused'}):
            with self.subTest(patch=patch), self.assertRaises(c.ReceiptError):
                self.project(rows={10:row(**patch)})

    def test_script_argument_is_not_primary_program(self):
        with self.assertRaises(c.ReceiptError) as error:
            self.project(rows={10:row(argv=[LAUNCHER, 'evil.py', SCRIPT, 'secret-token'])})
        self.assertNotIn('secret-token', str(error.exception))
        self.assertIn('actual-script-mismatch', str(error.exception))

    def test_null_controller_guard_is_real_root_no_invented_pid(self):
        p = self.project(records=[claim(controller=None, guardControllers=[control(11)])], rows={11:row()}, unknown_candidates=[11])
        self.assertEqual(p['actualPids'], [11])
        p = c.project(c.reconcile([claim(controller=None, controlCopies=['/not-a-pid'])]), {}, unknown_candidates=[])
        self.assertEqual(p['actualPids'], [])
        self.assertEqual(p['reservedProcesses'], 8)

    def test_descendant_dedup_and_cross_owner_ambiguity(self):
        p = self.project(records=[claim(controller=control(), guardControllers=[control(11)])], rows={10:row(),11:row(parent=10)}, unknown_candidates=[10,11])
        self.assertEqual(p['actualPids'], [10,11])
        self.assertEqual(p['actualRSSBytes'], 2048)
        with self.assertRaises(c.ReceiptError):
            self.project(records=[claim(controller=control()),claim(ownerThreadId='b',controller=control(11))], rows={10:row(),11:row(parent=10)}, unknown_candidates=[10,11])

    def test_ordinary_factual_counts_without_slot(self):
        obs = control(role='ordinary final Git metadata')
        p = self.project(records=[dict(ownerThreadId=OWNER, ordinaryObservation=obs)], rows={10:row(resourceRole='ordinary')})
        self.assertEqual(p['ordinaryActualPids'], [10])
        self.assertEqual(p['globalWorkflows'], 0)
        self.assertEqual(p['reservedProcesses'], 0)

    def test_optional_ordinary_executable_uses_exact_digest(self):
        obs = control(role='metadata', script=None, executable=None, commandDigest='measured')
        p = self.project(records=[dict(ownerThreadId=OWNER,ordinaryObservation=obs)],rows={10:row(resourceRole='ordinary')})
        self.assertEqual(p['ordinaryActualPids'], [10])
        for changed in ({'commandDigest':'different'},{'executable':None}):
            with self.subTest(changed=changed), self.assertRaises(c.ReceiptError):
                self.project(records=[dict(ownerThreadId=OWNER,ordinaryObservation=obs)],rows={10:row(resourceRole='ordinary',**changed)})

    def test_ordinary_reused_pid_unknown_not_exempt(self):
        with self.assertRaises(c.ReceiptError):
            self.project(records=[dict(ownerThreadId=OWNER, ordinaryObservation=control(role='metadata',birth='old'))], rows={10:row(resourceRole='ordinary')})

    def test_ordinary_cannot_hide_payload_even_with_role_label(self):
        for executable, argv in (('/bin/node',['node','app.js']),('/bin/cargo',['cargo','test']),
                                 ('/bin/rustc',['rustc','lib.rs']), (RUNTIME,[LAUNCHER,'-m','unittest']),
                                 ('/Applications/Chrome',['Chrome','--headless=new'])):
            with self.subTest(executable=executable), self.assertRaises(c.ReceiptError):
                self.project(records=[dict(ownerThreadId=OWNER,ordinaryObservation=control(role='metadata'))], rows={10:row(resourceRole='ordinary'),11:row(parent=10,executable=executable,argv=argv,resourceRole='ordinary')}, unknown_candidates=[10,11])

    def test_invalid_command_digest_is_never_echoed(self):
        with self.assertRaises(c.ReceiptError) as error:
            self.project(rows={10:row(executable=LAUNCHER,commandDigest='secret-token')})
        self.assertNotIn('secret-token',str(error.exception))

    def test_shell_unknown_has_no_blanket_exemption(self):
        with self.assertRaises(c.ReceiptError):
            self.project(rows={10:row(),12:row(executable='/bin/zsh',argv=['zsh','-c','opaque'])}, unknown_candidates=[10,12])

    def test_exact_infrastructure_does_not_exempt_children(self):
        rows = {10:row(),11:row(parent=10)}
        with self.assertRaises(c.ReceiptError):
            c.project(c.reconcile([]), rows, unknown_candidates=[10,11], infrastructure=[control()])

    def test_terminal_requires_previously_verified_exact_identity(self):
        # Synthetic cache transition only; no claim of live macOS qualification.
        live = self.project(rows={10:row(parent=7, state='S')})
        dead = {10:row(parent=7,state='Z',executable='(Python)',argv=['(Python)'],rss=0)}
        p = self.project(rows=dead, previous_identities=live['verifiedIdentities'])
        self.assertEqual(p['actualPids'], [10])
        self.assertEqual(p['reservedProcesses'], 8)
        self.assertEqual(p['globalWorkflows'], 1)
        p2 = self.project(rows=dead, previous_identities=json.loads(json.dumps(p['verifiedIdentities'])))
        self.assertEqual(p2['actualPids'], [10])
        for cache, changed in (({},dead), (live['verifiedIdentities'],{10:dict(dead[10],birth='new')}),
                               (live['verifiedIdentities'],{10:dict(dead[10],parent=8)})):
            with self.subTest(changed=changed), self.assertRaises(c.ReceiptError):
                self.project(rows=changed,previous_identities=cache)
        with self.assertRaises(c.ReceiptError):
            self.project(records=[claim(controller=control(11))],rows={11:dict(dead[10])},unknown_candidates=[11],previous_identities=live['verifiedIdentities'])
        with self.assertRaises(c.ReceiptError):
            self.project(rows={10:dict(dead[10],state='S')},previous_identities=live['verifiedIdentities'])
        with self.assertRaises(c.ReceiptError):
            self.project(records=[claim(offerId='next-offer',controller=control())],rows=dead,previous_identities=live['verifiedIdentities'])
        with self.assertRaises(c.ReceiptError):
            self.project(records=[claim(controller=control(script='/different.py'))],rows=dead,previous_identities=live['verifiedIdentities'])

    def test_other_upper_and_declared_actual_limits(self):
        p = self.project(own_owner=OWNER, ordinary_future_bytes=100)
        self.assertEqual(p['otherNewUpperBytes'], 100)
        c.check_limits(p, {'globalWorkflows':4,'source':3,'actualPids':96})
        with self.assertRaises(c.ReceiptError):
            c.check_limits(p, {'reservedProcesses':7})


if __name__ == '__main__':
    unittest.main(verbosity=2)
