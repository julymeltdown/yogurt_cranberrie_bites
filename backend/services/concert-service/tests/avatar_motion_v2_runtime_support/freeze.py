#!/usr/bin/env python3
"""Copy exact successful Cargo test executables outside the allocated target."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
ROOT = Path(__file__).resolve().parents[5]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--domain-metadata',type=Path,required=True)
    parser.add_argument('--service-metadata',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    destination=args.output.resolve()
    if '/target/' in str(destination) or '/mcr-target-' in str(destination):
        raise ValueError('snapshot must be outside build targets')
    destination.mkdir(mode=0o700,parents=True,exist_ok=False)
    receipt={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
             'cargo_metadata_sha256':hashlib.sha256(args.service_metadata.read_bytes()).hexdigest(),'artifacts':[]}
    for package,target,metadata in [('concert-domain','avatar_motion_v2',args.domain_metadata),('concert-service','avatar_motion_v2_runtime',args.service_metadata)]:
        rows=[json.loads(line) for line in metadata.read_text().splitlines()]
        assert any(r.get('reason')=='build-finished' and r.get('success') is True for r in rows), 'Cargo did not succeed'
        matches=[r for r in rows if r.get('reason')=='compiler-artifact' and r.get('target',{}).get('name')==target and 'test' in r.get('target',{}).get('kind',[]) and r.get('executable')]
        assert len(matches)==1,'exact unique executable required'
        row=matches[0]; source=Path(row['executable']); copied=destination/target
        assert row['manifest_path']==str(ROOT / ('backend/crates/concert-domain/Cargo.toml' if package=='concert-domain' else 'backend/services/concert-service/Cargo.toml'))
        shutil.copy2(source,copied)
        assert hashlib.sha256(source.read_bytes()).digest()==hashlib.sha256(copied.read_bytes()).digest()
        receipt['artifacts'].append({'package':package,'target':target,'manifest_path':row['manifest_path'],'compiler_executable':str(source),'snapshot':str(copied),'sha256':hashlib.sha256(copied.read_bytes()).hexdigest()})
        shutil.copy2(metadata,destination/(target+'.cargo.jsonl'))
    (destination/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps(receipt,sort_keys=True))
if __name__=='__main__':main()
