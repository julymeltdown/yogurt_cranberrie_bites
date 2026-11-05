"""Only newly changed FULL-finisher cases; accepted-lease execution required."""
import json
import os
from pathlib import Path
import runpy

scratch = Path(os.environ['MCR_PI12_PREWORLD_TEMP'])
assert scratch.is_dir() and str(scratch).startswith('/private/tmp/mcr-pi12-preworld-')
root = scratch / 'finisher-fixtures'
receipts = root / 'owner-resource-receipts'
receipts.mkdir(parents=True)
owner = '11111111-1111-1111-1111-111111111111'
full = root / 'literal-FULL'
metadata = root / 'membership.json'
metadata.write_text(json.dumps(dict(asOfUTC='2026-11-05T02:53:11Z', entries=[])))
(root / 'resource-receipt-interface.json').write_text('{}')
script = '/Users/lhs/Desktop/mcr-worktrees/unit/adapter-controller.py'
birth = 'Thu Nov 5 11:52:56 2026'
record = dict(ownerThreadId=owner, state='FULL', kind='source', offerId='EXACT-FINISHED',
    controller=dict(pid=60660, birth=birth, script=script), receiptPaths=dict(full=str(full)))
record_path = receipts / (owner + '.json')
record_path.write_text(json.dumps(record))
literal = dict(state='FULL', allocation='EXACT-FINISHED', ownedProcessesRemaining=[], dependencyLinkAbsent=True, scratchCacheAbsent=True)
full.write_text(json.dumps(literal))
sample = runpy.run_path(str(Path(__file__).with_name('guardian.py')))['global_sample']
sample.__globals__.update(MEMBERSHIP=metadata, RECEIPTS=receipts)
config = dict(mode='browser', offer=dict(processes=24, rssBytes=3758096384, newBytes=335544320, otherUpperBytes=3221225472))
rows = {60660: dict(pid=60660, parent=1, group=60660, rss=1000, birth=birth, command='/framework/Python -B ' + script)}
state = sample(config, rows, set())
assert state['unresolvedPids'] == [] and state['chargedPIDs'] == 25 and state['chargedRSS'] == 3758097384
assert state['workflows'] == 1 and state['sources'] == 0 and state['browsers'] == 1
rows[60661] = dict(pid=60661, parent=60660, group=60660, rss=2000, birth=birth, command='/bin/node /Users/lhs/Desktop/mcr-worktrees/unit/new-work.mjs')
assert sample(config, rows, set())['unresolvedPids'] == [60661]
del rows[60661]
rows[60660]['birth'] = 'Thu Nov 5 11:52:57 2026'
assert sample(config, rows, set())['unresolvedPids'] == [60660]
rows[60660]['birth'] = birth
literal['ownedProcessesRemaining'] = [42]
full.write_text(json.dumps(literal))
assert sample(config, rows, set())['unresolvedPids'] == [60660]
literal['ownedProcessesRemaining'] = []
literal['allocation'] = 'ANOTHER-ROUND'
full.write_text(json.dumps(literal))
assert sample(config, rows, set())['unresolvedPids'] == [60660]
print(json.dumps(dict(result='PASS', cases=5, behavior='Exact FULL finisher retains physical PID/RSS only; new child, reused birth, incomplete cleanup and different offer fail closed')))
