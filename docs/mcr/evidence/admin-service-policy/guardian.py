"""Independent guard for one explicitly accepted FE38 allocation; adapted from continuity guardian at bd087fde."""
import json
import os
from pathlib import Path
import signal
import subprocess as sp
import sys
import time


def census(paths):
    seen = set(); total = 0; counts = []
    for root in paths:
        subtotal = 0; count = 0
        root = Path(root)
        if root.is_symlink() or root.is_file():
            entries = [(root.parent, [], [root.name])]
        else:
            entries = __import__('os').walk(root, followlinks=False)
        for here, directories, files in entries:
            for name in ['.'] + directories + files:
                path = Path(here) if name == '.' else Path(here) / name
                try: stat = path.lstat()
                except FileNotFoundError: continue
                key = (stat.st_dev, stat.st_ino)
                if key in seen: continue
                seen.add(key); subtotal += max(stat.st_size, stat.st_blocks * 512); count += 1
        total += subtotal; counts.append({'root': str(root), 'bytes': subtotal, 'inodes': count})
    return {'total': total, 'roots': counts}


def size(path):
    return census([path])['total'] if path.exists() else 0


def fresh_bytes(config):
    root = Path(config['root'])
    objects = {row.split()[0] for row in sp.check_output(['git', 'rev-list', '--objects', config['base'] + '..HEAD'], cwd=root, text=True).splitlines()}
    git_bytes = sum(int(row) for row in sp.check_output(['git', 'cat-file', '--batch-check=%(objectsize)'], cwd=root,
        input='\n'.join(sorted(objects)) + '\n', text=True).splitlines()) if objects else 0
    git_bytes += size(Path(sp.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=root, text=True).strip()))
    return git_bytes + sum(size(root / path) for path in config['freshPaths'])


def sample(config):
    rows = [tuple(map(int, row.split())) for row in sp.check_output(['ps', '-axo', 'pid=,ppid=,pgid=,rss='], text=True).splitlines() if row.strip()]
    ids = {config['supervisor']}
    while True:
        more = ids | {pid for pid, parent, _, _ in rows if parent in ids}
        if more == ids:
            break
        ids = more
    temporary = size(Path(config['temporary']))
    fresh = fresh_bytes(config) + temporary
    return {'pids': sorted(ids), 'processes': len(ids), 'rss': sum(rss * 1024 for pid, _, _, rss in rows if pid in ids),
            'temporary': temporary, 'durable': size(Path(config['evidence'])), 'new': fresh,
            'whole': max(config['fixedCarry'], census(config['wholeRoots'])['total']) + fresh, 'free': __import__('shutil').disk_usage(config['root']).free}


def breach(config, values):
    caps = config['offer']
    return ('deadline' if time.time() >= config['workDeadline'] else
            'live free' if values['free'] < caps['continuousFreeBytes'] else
            'after ALL free' if values['free'] - caps['newBytes'] - caps['otherUpperBytes'] < caps['afterAllFreeBytes'] else
            next((key for key, field in [('processes', 'processes'), ('rss', 'rssBytes'), ('temporary', 'temporaryBytes'),
                  ('durable', 'durableBytes'), ('new', 'newBytes'), ('whole', 'wholeBytes')] if values[key] > caps[field]), None))


def main():
    path = Path(sys.argv[1]); config = json.loads(path.read_text())
    report = {'breach': None, 'samples': 0, 'peaks': {}}
    try:
        while not Path(config['stop']).exists():
            os.kill(config['supervisor'], 0)
            values = sample(config); report['samples'] += 1
            for key in ['processes', 'rss', 'temporary', 'durable', 'new', 'whole']:
                report['peaks'][key] = max(report['peaks'].get(key, 0), values[key])
            report['minimumFree'] = min(report.get('minimumFree', values['free']), values['free'])
            failed = breach(config, values)
            if failed:
                report['breach'] = failed
                active = Path(config['activeChild'])
                if active.exists():
                    try:
                        os.killpg(int(active.read_text()), signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                os.kill(config['supervisor'], signal.SIGTERM)
                break
            Path(config['ready']).touch()
            time.sleep(.4)
    except ProcessLookupError:
        report['breach'] = report['breach'] or 'supervisor disappeared'
    finally:
        Path(config['report']).write_text(json.dumps(report, indent=2) + '\n')
    return 1 if report['breach'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
