"""Bounded owned-control regression; run only inside a finite source allocation.

Synthetic metadata/process rows only. Never sends a real signal, edits peer
receipts, launches a workload, or claims a product integration result.
"""
import hashlib
import json
import os
from pathlib import Path
import signal
import tempfile
from unittest.mock import patch
import guardian as g


def main():
    with tempfile.TemporaryDirectory(dir=os.environ['MCR_FE38_SCRATCH'], prefix='receipt-regression-') as directory:
        root = Path(directory)
        membership = root / 'membership.json'
        receipts = root / 'receipts'; receipts.mkdir()
        executable = '/synthetic/python'
        def process(pid, command='/mcr-tools/node workload', parent=999):
            return {'pid': pid, 'parent': parent, 'group': pid, 'rss': 1024,
                    'birth': 'Thu Nov 5 10:00:00 2026', 'executable': executable, 'command': command}
        config = {'offer': {'id': 'owned-round', 'processes': 12, 'rssBytes': 3 * 1024**3,
                            'newBytes': 128 * 1024**2, 'otherUpperBytes': int(2.5 * 1024**3)},
                  'mode': 'source', 'ordinaryPrograms': {}}
        own = process(101)
        old = {'ownerThreadId': 'peer', 'offerId': 'old', 'acceptedAtEpoch': 7,
               'state': 'ACCEPTED', 'kind': 'source', 'controller': None,
               'processCap': 4, 'rssCapBytes': 4096, 'remainingNewUpperBytes': 2048}
        def write(entries):
            membership.write_text(json.dumps({'asOfUTC': 'synthetic', 'entries': entries}))
        def receipt(value):
            (receipts / (value['ownerThreadId'] + '.json')).write_text(json.dumps(value))
        with patch.object(g, 'MEMBERSHIP', membership), patch.object(g, 'RECEIPTS', receipts):
            # Null controllers in closed metadata are legal, not live roots.
            write([{**old, 'state': 'FULL'}, {'ownerThreadId': g.OWNER, 'offerId': 'owned-round',
                   'state': 'ACCEPTED', 'kind': 'source', 'controller': None, 'guardControllers': None}])
            result = g.global_sample(config, {101: own}, {101})
            assert result['workflows'] == 1 and result['chargedPIDs'] == 12
            (receipts / 'management.json').write_text(json.dumps({'ordinaryObservation': {
                'role': 'ordinary Main management authoring and Git only', 'actualpid': None,
                'ordinaryCommandsComplete': True, 'commandExit': 0}}))
            assert g.global_sample(config, {101: own}, {101})['workflows'] == 1
            # No visible PID and no new receipt cannot release a legacy claim.
            write([old]); result = g.global_sample(config, {101: own}, {101})
            assert result['workflows'] == 2 and result['chargedPIDs'] == 16
            # A different new round does not erase the older accepted one.
            receipt({**old, 'offerId': 'new', 'acceptedAtEpoch': 8})
            assert g.global_sample(config, {101: own}, {101})['workflows'] == 3
            # Only an exact-round literal FULL closes that legacy reservation.
            receipt({**old, 'state': 'FULL'})
            assert g.global_sample(config, {101: own}, {101})['workflows'] == 1
            # A pinned literal old-round release survives replacement of the
            # peer's single current record by a newer accepted round.
            receipt({**old, 'offerId': 'new', 'acceptedAtEpoch': 8})
            full = root / 'old-FULL.json'; full.write_text(json.dumps({'state': 'ACTUAL_FULL', 'remainingOwned': []}))
            config['peerFullReceipts'] = [{'ownerThreadId': 'peer', 'offerId': 'old', 'acceptedAtEpoch': 7,
                                          'path': str(full), 'sha256': hashlib.sha256(full.read_bytes()).hexdigest()}]
            assert g.global_sample(config, {101: own}, {101})['workflows'] == 2
            receipt({**old, 'state': 'FULL'})
            # Known Git-only controller is ordinary; an app child still fails.
            program = root / 'ordinary.py'; program.write_text('# synthetic Git-only program\n')
            config['ordinaryPrograms'][str(program)] = hashlib.sha256(program.read_bytes()).hexdigest()
            publisher = process(102, '/synthetic/python ' + str(program))
            receipt({'ownerThreadId': 'ordinary', 'offerId': None, 'state': 'ACCEPTED', 'kind': 'ordinary',
                     'controller': {**publisher, 'script': str(program)}, 'executionWorkflow': False,
                     'processCap': None, 'rssCapBytes': None, 'remainingNewUpperBytes': 1024})
            result = g.global_sample(config, {101: own, 102: publisher}, {101})
            assert result['workflows'] == 1 and not result['unresolvedPids']
            child = process(103, parent=102)
            assert g.global_sample(config, {101: own, 102: publisher, 103: child}, {101})['unresolvedPids'] == [103]
            config['ordinaryPrograms'][str(program)] = 'changed'
            assert g.global_sample(config, {101: own, 102: publisher}, {101})['unresolvedPids'] == [102]
        # PermissionError on one verified owned PID cannot abort the rest;
        # changed birth prevents signaling even if the PID is the same.
        second, reused = process(104), process(105)
        calls = []
        def fake_kill(pid, sig):
            calls.append(pid)
            if pid == 104: raise PermissionError('synthetic EPERM')
        with patch.object(g, 'process_rows', return_value={104: second, 105: {**reused, 'birth': 'different'}, 106: process(106)}), patch.object(g.os, 'kill', side_effect=fake_kill):
            errors = g.signal_owned({104: second, 105: reused, 106: process(106)}, signal.SIGTERM)
        assert calls == [104, 106] and len(errors) == 1
        assert not g.matches({**own, 'executable': '/different'}, own)
    print(json.dumps({'controlGroups': 10, 'status': 'PASS', 'realSignals': 0, 'productCases': 0}))


if __name__ == '__main__':
    main()
