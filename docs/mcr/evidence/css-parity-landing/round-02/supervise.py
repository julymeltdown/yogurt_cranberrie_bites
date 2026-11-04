"""Round-02 finite owner process-group guard; set clock only after resource ACCEPT."""
from pathlib import Path
import subprocess, os, signal, sys, time, shutil, json
from datetime import datetime, timezone

ROOT = Path('/Users/lhs/.codex/worktrees/fb96/mcr')
TMP = Path('/tmp/mcr-css-parity-landing-round-02')
EVIDENCE = ROOT / 'docs/mcr/evidence/css-parity-landing'
NODE = '/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node'
def utc():
    return datetime.now(timezone.utc).isoformat()

def birth(pid):
    result = subprocess.run(['ps', '-p', str(pid), '-o', 'lstart='], capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else ''

def custody(payload):
    """Detached deadline custodian; never depends on another assistant turn.

    Each job gets an independent custodian. Only the recorded child group and
    exact owned filesystem inodes can be touched. The normal supervisor owns
    wait/reaping; the custodian also stops children if that supervisor stalls.
    """
    pid = payload['pid']
    while time.time() < payload['stop']:
        time.sleep(min(0.5, max(0, payload['stop'] - time.time())))
    if payload['birth'] and birth(pid) == payload['birth']:
        try:
            os.killpg(pid, signal.SIGTERM)
            time.sleep(3)
            if birth(pid) == payload['birth']:
                os.killpg(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    # Normal supervisor has time to reap the child before removing its scratch.
    time.sleep(5)
    link = ROOT / 'frontend/node_modules'
    if link.is_symlink() and link.lstat().st_ino == payload['linkInode'] and os.readlink(link) == payload['linkTarget']:
        link.unlink()
    if TMP.exists() and not TMP.is_symlink() and TMP.stat().st_ino == payload['tmpInode']:
        shutil.rmtree(TMP)
    print(json.dumps({'custodyCleanupUTC': utc(), 'child': pid,
                      'hardDeadline': payload['hard'], 'late': time.time() > payload['hard']}), flush=True)

if len(sys.argv) == 3 and sys.argv[1] == '--custody':
    custody(json.loads(sys.argv[2]))
    raise SystemExit(0)

accepted = float(os.environ['MCR_LANDING_ACCEPT_EPOCH'])
stop = accepted + 420
assert Path.cwd() == ROOT and accepted <= time.time() < stop
assert shutil.disk_usage(ROOT).free >= 8 * 1024**3

def size(path):
    return sum(p.stat().st_size for p in path.rglob('*') if p.is_file() and not p.is_symlink()) if path.exists() else 0

env = dict(os.environ, PATH=str(Path(NODE).parent) + ':' + os.environ['PATH'], TMPDIR=str(TMP / 'tmp'))
command = sys.argv[1:]
assert command
label = env.get('MCR_LANDING_PHASE', 'assessment')
log_path = EVIDENCE / 'round-02' / (label + '-attempt.log')
with log_path.open('a') as log:
    log.write(f'UTCepoch={time.time()} accepted={accepted} stop={stop} command={command!r}\n'); log.flush()
    process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    failure = None
    try:
        link = ROOT / 'frontend/node_modules'
        assert link.is_symlink() and os.readlink(link) == '/Users/lhs/Desktop/mcr/frontend/node_modules'
        assert TMP.is_dir() and not TMP.is_symlink()
        payload = {'pid': process.pid, 'birth': birth(process.pid), 'stop': stop,
                   'hard': accepted + 480, 'linkInode': link.lstat().st_ino,
                   'linkTarget': os.readlink(link), 'tmpInode': TMP.stat().st_ino}
        # Inherits no tool-session pipes; survives completed jobs and compaction.
        with (EVIDENCE / 'round-02/custody-attempt.log').open('a') as custody_log:
            subprocess.Popen([sys.executable, str(Path(__file__).resolve()), '--custody', json.dumps(payload)],
                             stdin=subprocess.DEVNULL, stdout=custody_log, stderr=subprocess.STDOUT,
                             start_new_session=True, close_fds=True)
        while process.poll() is None:
            temporary, durable = size(TMP), size(EVIDENCE)
            if time.time() >= stop: failure = 'active deadline'; break
            if shutil.disk_usage(ROOT).free < 8*1024**3: failure = 'disk floor'; break
            if temporary > 128849018 or durable > 15*1024**2 or temporary + durable > 214748364: failure = 'byte cap'; break
            time.sleep(.5)
    except BaseException:
        failure = failure or 'supervisor exception'
        raise
    finally:
        if failure:
            os.killpg(process.pid, signal.SIGTERM)
            try: process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL); process.wait()
        code = process.wait()
        log.write(f'EXIT={code} guard={failure} stoppedUTC={utc()}\n')
    print('EXIT', code, 'guard', failure, 'temporaryBytes', size(TMP), 'durableBytes', size(EVIDENCE))
    sys.exit(code if not failure else 2)
