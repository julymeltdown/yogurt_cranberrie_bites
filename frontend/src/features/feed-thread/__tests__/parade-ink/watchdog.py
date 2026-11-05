"""Independent live secondary: identical offer, inventory and original clocks."""
import json
import os
from pathlib import Path
import signal
import sys
import time
from guard_support import sample, violation

config = json.loads(Path(sys.argv[1]).read_text())
folder = Path(config['temporary'])
report = {'guard': 'independent-secondary', 'startedEpoch': time.time(), 'samples': 0,
          'peakProcesses': 0, 'peakRssKiB': 0, 'peakWholeBytes': 0}
seen = {}
notified = False
try:
    while True:
        if (folder / 'done').exists():
            break
        value = sample(config)
        report['samples'] += 1
        report['peakProcesses'] = max(report['peakProcesses'], len(value['processes']))
        report['peakRssKiB'] = max(report['peakRssKiB'], value['rssKiB'])
        report['peakWholeBytes'] = max(report['peakWholeBytes'], value['whole']['allocatedBytes'])
        for row in value['processes']:
            seen[row['pid']] = row['commandHash']
        problem = violation(value, config)
        if report['samples'] == 1:
            if problem:
                raise RuntimeError(problem)
            (folder / 'secondary-ready').write_text(str(os.getpid()))
        if time.time() >= config['workDeadline'] and not (folder / 'cleaning').exists():
            problem = 'Original work deadline'
        if problem and not notified:
            report['violation'] = problem
            notified = True
            os.kill(config['rootPid'], signal.SIGUSR1)
        if time.time() >= config['cleanupDeadline']:
            report['violation'] = 'Original FULL deadline'
            for row in reversed(value['processes']):
                if row['pid'] != os.getpid():
                    try:
                        os.kill(row['pid'], signal.SIGKILL)
                    except ProcessLookupError:
                        pass
            break
        time.sleep(.5)
except BaseException as error:
    report['error'] = str(error)
    try:
        os.kill(config['rootPid'], signal.SIGUSR1)
    except ProcessLookupError:
        pass
finally:
    report['finishedEpoch'] = time.time()
    Path(config['output'], 'secondary.json').write_text(json.dumps(report, indent=2) + '\n')
