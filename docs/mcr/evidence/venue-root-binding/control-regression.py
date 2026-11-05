"""Controlled identity rows: ordinary parent never hides an execution child."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import guardian

with tempfile.TemporaryDirectory(prefix='venue-root-control-', dir=__import__('os').environ['TMPDIR']) as name:
    root = Path(name); receipts = root / 'receipts'; receipts.mkdir()
    script = root / 'observer.py'; script.write_text('# source-only observer fixture\n')
    membership = root / 'membership.json'; membership.write_text(json.dumps({'asOfUTC': 'controlled', 'entries': []}))
    guardian.MEMBERSHIP = membership; guardian.RECEIPTS = receipts
    owner = '00000000-0000-4000-8000-000000000001'
    identity = {'actualpid': 10, 'birth': 'Thu Nov 5 12:00:00 2026', 'executable': sys.executable, 'script': str(script),
                'cwd': name, 'role': 'controlled ordinary source observer'}
    record = {'ownerThreadId': owner, 'offerId': 'ordinary-fixture', 'acceptedAtEpoch': 1, 'state': 'ACCEPTED',
              'kind': 'ordinary', 'controller': None, 'processCap': None, 'rssCapBytes': None,
              'remainingNewUpperBytes': 16, 'ordinaryObservation': identity}
    path = receipts / (owner + '.json')
    rows = {10: {'pid': 10, 'parent': 0, 'birth': identity['birth'], 'executable': sys.executable,
                 'command': sys.executable + ' ' + str(script), 'rss': 1},
            11: {'pid': 11, 'parent': 10, 'birth': identity['birth'], 'executable': '/controlled/node',
                 'command': '/controlled/node /mcr/unregistered-payload.js', 'rss': 1},
            20: {'pid': 20, 'parent': 0, 'birth': identity['birth'], 'executable': sys.executable,
                 'command': sys.executable + ' /mcr/owned-controller.py', 'rss': 1}}
    config = {'mode': 'source', 'offer': {'id': 'owned-fixture', 'processes': 1, 'rssBytes': 100, 'newBytes': 1, 'otherUpperBytes': 0},
              'ordinaryPrograms': {str(script): hashlib.sha256(script.read_bytes()).hexdigest()},
              'ordinaryRolePins': [{'ownerThreadId': owner, 'offerId': 'ordinary-fixture', 'acceptedAtEpoch': 1,
                                    'phase': None, 'role': identity['role'], 'script': str(script)}]}
    path.write_text(json.dumps(record))
    actual = guardian.global_sample(config, rows, {20})
    assert actual['ordinary'][0]['verified'] is True and actual['unresolvedPids'] == [11]
    record['ordinaryObservation'] = {**identity, 'birth': 'WRONG BIRTH'}; path.write_text(json.dumps(record))
    actual = guardian.global_sample(config, rows, {20})
    assert actual['ordinary'][0]['verified'] is False and actual['unresolvedPids'] == [10, 11]
    record['ordinaryObservation'] = {k: v for k, v in identity.items() if k != 'executable'}; path.write_text(json.dumps(record))
    actual = guardian.global_sample(config, rows, {20})
    assert actual['ordinary'][0]['verified'] is False and actual['unresolvedPids'] == [10, 11]
print(json.dumps({'groups': 3, 'result': 'PASS', 'classification': 'CONTROLLED_ROWS_NOT_LIVE_ADMISSION', 'signals': 0}))
