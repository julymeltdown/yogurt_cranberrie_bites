#!/usr/bin/env python3
"""Pin the complete source and one exact Cargo test artifact under a named lease."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[4]
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def snapshot():
    paths = subprocess.check_output(['git', 'ls-files', '-z', 'backend', 'contracts', 'AGENTS.md',
        'docs/mcr/backend/age-account-deletion.md'], cwd=ROOT).decode().split('\0')
    return {'head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
            'files': {p: sha(ROOT/p) for p in paths if p and (ROOT/p).is_file()}}
def main():
    p=argparse.ArgumentParser();p.add_argument('--snapshot',type=Path);p.add_argument('--before',type=Path)
    p.add_argument('--cargo-json',type=Path,action='append');p.add_argument('--out',type=Path);a=p.parse_args()
    if a.snapshot: a.snapshot.write_text(json.dumps(snapshot(),indent=2)+'\n');return
    before=json.loads(a.before.read_text());assert before==snapshot(),'source drift'
    assert not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True),'uncommitted source'
    assert not a.out.exists();a.out.mkdir(mode=0o700);(a.out/'bin').mkdir()
    binary=None
    for log in a.cargo_json:
        success=False
        for line in log.read_text().splitlines():
            row=json.loads(line)
            if row.get('reason')=='build-finished':success=row['success']
            if row.get('reason')=='compiler-artifact' and row.get('target',{}).get('name')=='age_deletion_native' and row.get('executable'):
                assert row['profile']['test'] and row['profile'].get('debuginfo') in (0,None)
                assert Path(row['target']['src_path']).resolve()==ROOT/'backend/services/concert-service/tests/age_deletion_native.rs'
                binary=Path(row['executable']).resolve()
        assert success,'Cargo failed'
    assert binary is not None
    dest=a.out/'bin/age_deletion_native';shutil.copy2(binary,dest)
    shutil.copytree(ROOT/'backend/services/concert-service/migrations',a.out/'migrations')
    receipt={'source':before,'binaries':{'age_deletion_native':{'size':dest.stat().st_size,'sha256':sha(dest)}},
        'migrations':{p.name:sha(p) for p in (a.out/'migrations').glob('*.sql')}}
    (a.out/'freeze.json').write_text(json.dumps(receipt,indent=2)+'\n')
if __name__=='__main__':main()
