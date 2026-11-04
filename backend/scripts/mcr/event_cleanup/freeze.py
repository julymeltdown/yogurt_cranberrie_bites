#!/usr/bin/env python3
"""Source snapshot and exact Cargo JSON artifact freeze; run only under named lease."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[4]
PIN_PATHS = [
 ('application','backend/crates/concert-application/src/event_cleanup/mod.rs'),
 ('runtime','backend/services/concert-service/src/event_cleanup_live/mod.rs'),
 ('migration','backend/services/concert-service/migrations/0046_event_cleanup_runtime.sql'),
 ('config','backend/services/concert-service/src/config/event_cleanup.rs'),
 ('application_tests','backend/crates/concert-application/tests/event_cleanup.rs'),
 ('runtime_tests','backend/services/concert-service/tests/event_cleanup_runtime.rs'),
 ('test_support','backend/services/concert-service/tests/event_cleanup_support/mod.rs')]
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def snapshot():
    paths = subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z','backend','contracts','AGENTS.md'],cwd=ROOT).decode().split('\0')
    hashes = {p:sha(ROOT/p) for p in sorted(set(paths)) if p and (ROOT/p).is_file()}
    pin = hashlib.sha256()
    for name, path in PIN_PATHS:
        pin.update(name.encode()+b'\0'+(ROOT/path).read_bytes()+b'\0')
    return {'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'files':hashes,'owned_source_pin':pin.hexdigest()}
def main():
    p=argparse.ArgumentParser();p.add_argument('--snapshot',type=Path);p.add_argument('--before',type=Path);p.add_argument('--cargo-json',type=Path,action='append');p.add_argument('--out',type=Path);a=p.parse_args()
    if a.snapshot:
        a.snapshot.write_text(json.dumps(snapshot(),indent=2)+'\n');return
    if not (a.before and a.cargo_json and a.out):p.error('freeze requires --before, --cargo-json and --out')
    before=json.loads(a.before.read_text());assert before==snapshot(),'source changed since pre-build snapshot'
    assert not a.out.exists(),'use a fresh artifact destination'
    a.out.mkdir(mode=0o700);(a.out/'bin').mkdir()
    wanted={'event_cleanup':None,'event_cleanup_runtime':None}
    for log in a.cargo_json:
        success=False
        for line in log.read_text().splitlines():
            row=json.loads(line)
            if row.get('reason')=='build-finished':success=row['success']
            name=row.get('target',{}).get('name')
            if row.get('reason')=='compiler-artifact' and name in wanted and row.get('executable') and row.get('profile',{}).get('test'):
                expected = ROOT/('backend/crates/concert-application/tests/event_cleanup.rs' if name=='event_cleanup' else 'backend/services/concert-service/tests/event_cleanup_runtime.rs')
                assert Path(row['target']['src_path']).resolve()==expected.resolve(), 'wrong test source'
                assert row['profile'].get('debuginfo') in (0, None), 'debug information must be disabled'
                candidate=Path(row['executable']).resolve();assert candidate.is_file();wanted[name]=candidate
        assert success,f'build did not finish successfully: {log}'
    receipt={'source':before,'binaries':{},'cargo_json':[],
             'rustc':subprocess.check_output([str(Path.home()/'.cargo/bin/rustc'),'+1.88.0','-vV'],text=True),
             'build_environment':{key:os.environ.get(key) for key in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG','RUSTFLAGS']}}
    for name, source in wanted.items():
        assert source is not None,f'missing exact Cargo test artifact: {name}'
        dest=a.out/'bin'/name;shutil.copy2(source,dest)
        receipt['binaries'][name]={'sha256':sha(dest),'size':dest.stat().st_size}
    for i,log in enumerate(a.cargo_json):
        dest=a.out/f'cargo-{i}.jsonl';shutil.copy2(log,dest);receipt['cargo_json'].append({'name':dest.name,'sha256':sha(dest)})
    (a.out/'freeze.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(a.out)
if __name__=='__main__':main()
