#!/usr/bin/env python3
"""Pin exact Cargo artifacts before releasing its lease, then qualify owned PG16.

No Cargo, Docker, default services or foreign database is started by this script.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[6]

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def pins():
    paths = subprocess.check_output(['git', 'diff', '--name-only', 'HEAD'], cwd=ROOT, text=True).splitlines()
    paths += subprocess.check_output(['git', 'ls-files', '--others', '--exclude-standard'], cwd=ROOT, text=True).splitlines()
    paths += [
        'backend/services/profile-service/src/adapters/out/persistence/concert_identity.rs',
        'backend/crates/profile-application/src/concert_identity.rs',
        'backend/services/profile-service/tests/support/concert_identity_fixture.rs',
        'backend/scripts/mcr/migration_smoke.py',
        'backend/crates/platform/src/actor_delegation.rs',
        'backend/crates/platform/src/actor_delegation/read_authority.rs',
        'backend/crates/platform/src/actor_delegation/authority_deadline.rs',
        'backend/crates/platform/src/call_deadline.rs',
        'backend/services/concert-service/src/adapters/out/relationship_authority/world_auth.rs',
        'backend/services/concert-service/src/adapters/out/relationship_authority/world_projection.rs',
    ]
    paths += [str(p.relative_to(ROOT)) for p in (ROOT/'backend/services/profile-service/migrations').glob('*.sql')]
    return {p: sha(ROOT/p) for p in sorted(set(paths)) if (ROOT/p).is_file() and p.startswith('backend/')}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--pin', type=Path, help='Cargo JSON output; copy executables while lease held')
    p.add_argument('--pinned', type=Path, help='pin.json produced by --pin; no target lease needed')
    p.add_argument('--pg-bin', type=Path, default=Path('/opt/homebrew/opt/postgresql@16/bin'))
    args = p.parse_args()
    if bool(args.pin) == bool(args.pinned):
        p.error('exactly one of --pin/--pinned required')
    if args.pin:
        items = [json.loads(s) for s in args.pin.read_text().splitlines() if s.startswith('{')]
        selected = {}
        for name, manifest in [('world_identity_rpc', 'services/profile-service'),
                               ('concert_service', 'services/concert-service'),
                               ('profile_service', 'services/profile-service'),
                               ('platform', 'crates/platform')]:
            expected = str(ROOT/'backend'/manifest/'Cargo.toml')
            candidates = [a for a in items if a.get('reason') == 'compiler-artifact' and a.get('executable')
                          and a['target']['name'] == name and a.get('manifest_path') == expected and a['profile']['test']]
            if len(candidates) != 1:
                p.error('expected one exact test executable for '+name)
            a = candidates[0]
            if a['profile']['debuginfo'] != 0:
                p.error('debug zero artifact required')
            selected[name] = a
        out = Path(tempfile.mkdtemp(prefix='mcr-world-profile-pinned-', dir='/tmp')).resolve()
        report = {'head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                  'source_sha256': pins(), 'cargo_json_sha256': sha(args.pin), 'artifacts': {}}
        for name, a in selected.items():
            src = Path(a['executable']).resolve(strict=True)
            dst = out/name
            shutil.copy2(src, dst)
            assert sha(dst) == sha(src)
            report['artifacts'][name] = {'binary':str(dst),'sha256':sha(dst),'artifact':a}
        (out/'pin.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps({'pinned':str(out/'pin.json')}))
        return 0
    report = json.loads(args.pinned.read_text())
    out = args.pinned.resolve().parent
    assert out.stat().st_mode & 0o077 == 0
    for a in report['artifacts'].values():
        assert sha(a['binary']) == a['sha256']
    assert all(sha(ROOT/k) == v for k,v in report['source_sha256'].items()), 'source changed after pin'
    report.update(result='IN_PROGRESS',commands={},transport='signed caller/actor middleware on controlled plaintext loopback; TLS not qualified',migration_mode='seven retained SQL files in order; no startup/SQLx ledger claim')
    cluster = None
    def save(): (out/'native-report.json').write_text(json.dumps(report,indent=2)+'\n')
    def run(name, binary, options, env):
        if cluster: cluster.assert_owned()
        a = report['artifacts'][binary]
        assert sha(a['binary']) == a['sha256']
        cmd = [a['binary'], *options, '--test-threads=1']
        log = out/(name+'.log')
        started=time.monotonic()
        try:
            with log.open('w') as f: code=subprocess.run(cmd,cwd=ROOT,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=60).returncode
        except subprocess.TimeoutExpired: code='TIMEOUT'
        report['commands'][name]={'args':cmd,'exit_code':code,'seconds':time.monotonic()-started,'log_sha256':sha(log)}
        save()
        text=log.read_text()
        counts=re.findall(r'test result: ok\. (\d+) passed;', text)
        report['commands'][name]['passed']=sum(map(int,counts))
        save()
        if cluster: assert cluster.password not in text and cluster.url('mcr_profile_self_rpc') not in text
        for canary in ['private-bio-canary','legacy-token-canary','private-delete-canary','private-handle-']:
            assert canary not in text, 'private diagnostic leak'
        assert code == 0, name+' failed; see preserved private log'
        assert counts and sum(map(int,counts))>0, 'zero executed tests refused'
    save()
    try:
        env={k:v for k,v in os.environ.items() if not k.startswith(('PG','MCR_PROFILE_RPC_')) and not k.endswith('DATABASE_URL')}
        run('signed-profile','world_identity_rpc',[],env)
        run('typed-client','concert_service',['adapters::out::world_profile::tests::'],env)
        run('platform-regression','platform',[],env)
        run('profile-policy-regression','profile_service',['method_policy::tests::'],env)
        sys.path.insert(0,str(ROOT/'backend/scripts/mcr'))
        import migration_smoke as m
        cluster=m.Cluster({name:args.pg_bin/name for name in m.PG_TOOLS})
        report['owned_root']=str(cluster.root);report['owner_identity']=list(cluster.identity);save()
        cluster.init();cluster.start()
        db='mcr_profile_self_rpc'
        cluster.sql('postgres','create database mcr_profile_self_rpc template template0;')
        migrations=sorted((ROOT/'backend/services/profile-service/migrations').glob('*.sql'))
        assert len(migrations)==7
        for path in migrations: cluster.sql(db,path.read_text())
        report['postgres_version']=cluster.sql('postgres','show server_version;')
        cluster.sql(db,'create role mcr_profile_rpc_reader nologin; create role mcr_profile_rpc_noaccess nologin; grant select(user_id,nickname,bio,deleted_at) on profiles to mcr_profile_rpc_reader; grant select(user_id) on profile_user_deletion_tombstones to mcr_profile_rpc_reader;')
        fixture_env={**cluster.env,'MCR_PROFILE_RPC_OWNER_ROOT':str(cluster.root),
            'MCR_PROFILE_RPC_OWNER_IDENTITY':':'.join(map(str,cluster.identity)),
            'MCR_PROFILE_RPC_OWNER_TOKEN':cluster.token,'MCR_PROFILE_RPC_DATABASE_URL':cluster.url(db)}
        run('native-profile','world_identity_rpc',['native_','--ignored','--skip','native_world_profile_stopped_database_is_sanitized'],fixture_env)
        run('native-typed-client','concert_service',['adapters::out::world_profile::tests::native_','--ignored'],fixture_env)
        cluster.stop();report['owned_stopped_for_outage']=True;save()
        run('native-outage','world_identity_rpc',['--exact','native_world_profile_stopped_database_is_sanitized','--ignored'],fixture_env)
        cluster.start()
        run('native-recovery','world_identity_rpc',['--exact','native_world_profile_current_private_content_and_fences','--ignored'],fixture_env)
        assert all(sha(ROOT/k)==v for k,v in report['source_sha256'].items())
        report['result']='PASS'
    except BaseException as exc:
        report.update(result='FAIL',failure_type=type(exc).__name__)
    finally:
        if cluster:
            root=cluster.root
            try:
                cluster.cleanup();report['cleanup']={'exact_owned_root_absent':not root.exists(),'owned_server_stopped':True}
            except BaseException as exc:
                report.update(result='FAIL',cleanup={'failure_type':type(exc).__name__})
        else: report['cleanup']={'no_cluster_started':True}
        save()
    print(json.dumps({'result':report['result'],'report':str(out/'native-report.json'),'cleanup':report['cleanup']}))
    return 0 if report['result']=='PASS' else 1

if __name__=='__main__': raise SystemExit(main())
