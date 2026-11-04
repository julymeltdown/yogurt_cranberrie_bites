#!/usr/bin/env python3
"""Actual QA04 backend HTTP role/resource matrix; no synthetic positive fallback."""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import urllib.error
import urllib.parse
import urllib.request

ROOT=Path(__file__).resolve().parents[6]
ROLES=('owner','member','stranger','blocked','suspended','expired','admin')
AXES=('list','detail','mutation')
DOMAINS=('handle','claim','evidence','beacon','recipient')
UUID=re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
NIL='00000000-0000-0000-0000-000000000000'
class Refused(Exception):pass
def require(value):
    if not value:raise Refused()
def private_plan(path):
    require(path.is_absolute() and path.resolve()==path and path.is_relative_to('/private/tmp'))
    m=path.lstat();require(stat.S_ISREG(m.st_mode) and m.st_uid==os.getuid() and not m.st_mode&0o077 and m.st_size<=1048576)
    return json.loads(path.read_text())
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*_):raise Refused()
def qualify(plan):
    require(plan.get('version')==1 and plan.get('isolatedEnvironment') is True)
    origin=urllib.parse.urlsplit(plan['origin']);require(origin.geturl()==plan['origin'] and not origin.username and not origin.password and not origin.query and not origin.fragment and not origin.path)
    require(origin.scheme=='https' or origin.scheme=='http' and origin.hostname in ('localhost','127.0.0.1','::1'))
    for name in ('environmentId','seedRevision','serviceRevision','principalHandoff'):require(isinstance(plan.get(name),str) and 0<len(plan[name])<=512)
    principals=plan['principals'];require(set(principals)==set(ROLES))
    for principal in principals.values():
        require(all(UUID.fullmatch(principal.get(k,'')) and principal[k]!=NIL for k in ('userId','eventId')))
        require(re.fullmatch(r'[A-Z][A-Z0-9_]+',principal.get('tokenEnv','')))
        require(re.fullmatch(r'[A-Za-z0-9._~+/-]+=*',os.environ.get(principal['tokenEnv'],'')) and len(os.environ[principal['tokenEnv']])<=16384)
    require(len({p['userId'] for p in principals.values()})==7)
    operations=json.loads((ROOT/'contracts/domain/v1/operations.json').read_text());known={o['operationId']:o for o in operations}
    platforms=json.loads((ROOT/'contracts/domain/v1/common.schema.json').read_text())['$defs']['Platform']['enum']
    cells=plan['httpCells'];require(len(cells)==21 and {(c['role'],c['operation']) for c in cells}=={(r,a) for r in ROLES for a in AXES})
    output=[];mutation_keys=set()
    for cell in cells:
        probes=cell['probes'];require(0<len(probes)<=100 and all(any(d in p.get('domains',[]) for p in probes) for d in DOMAINS))
        for p in probes:
            o=known[p['operationId']];method=o['method'].upper();status=p['expectedStatus']
            require((cell['operation']=='mutation')==(method!='GET'))
            require(status==o['success_status'] or str(status) in o.get('errors',{}))
            # Input/media errors cannot masquerade as an authorization denial.
            require(status not in (400,405,413,415,422))
            params=p.get('parameters',{});required=set(re.findall(r'\{([^}]+)\}',o['path']));require(set(params)==required and all(isinstance(v,str) for v in params.values()))
            for name,value in params.items():
                reference=o['path_parameters'][name].get('$ref')
                if reference=='https://contracts.mcr.invalid/domain/v1/common.schema.json#/$defs/Uuid':require(UUID.fullmatch(value) and value!=NIL)
                elif reference=='https://contracts.mcr.invalid/domain/v1/common.schema.json#/$defs/Platform':require(value in platforms)
                else:raise Refused()
            require(params['eventId']==principals[cell['role']]['eventId'])
            path=o['path']
            for name,value in params.items():path=path.replace('{'+name+'}',value)
            query=p.get('query',{});require(isinstance(query,dict) and all(isinstance(k,str) and isinstance(v,(str,int,bool)) for k,v in query.items()))
            if query:path+='?'+urllib.parse.urlencode(query)
            forbidden=p['forbiddenValues'];positive=p['requiredValues'];require(len(forbidden)>=5 and all(isinstance(x,str) and 8<=len(x)<=4096 for x in forbidden))
            require(isinstance(positive,list) and all(isinstance(x,str) and 8<=len(x)<=4096 for x in positive) and (status>=400 or positive))
            headers={'Authorization':'Bearer '+os.environ[principals[cell['role']]['tokenEnv']],'Accept':'application/json','Cache-Control':'no-store'};body=None
            if method!='GET':
                require(plan.get('allowSeedMutations') is True and UUID.fullmatch(p.get('idempotencyKey','')) and p['idempotencyKey']!=NIL and p['idempotencyKey'] not in mutation_keys)
                mutation_keys.add(p['idempotencyKey']);headers.update({'Idempotency-Key':p['idempotencyKey'],'X-MCR-Intent':'browser','Origin':plan['origin']})
                if 'body' in p:require(o['request_location']=='body');body=json.dumps(p['body'],separators=(',',':')).encode();headers['Content-Type']='application/json';require(len(body)<=1048576)
                else:require(o['request_location']!='body')
            else:require('body' not in p)
            output.append((cell,p,o,urllib.request.Request(plan['origin']+path,data=body,headers=headers,method=method)))
    return output
def run(args,report):
    plan=private_plan(args.plan);requests=qualify(plan)
    path=ROOT/'backend/services/concert-service/tests/social_live_support/contract_oracle.py'
    spec=importlib.util.spec_from_file_location('social_matrix_contract_oracle',path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    oracle=module.ContractOracle(ROOT)
    report['response_oracle_sha256']=oracle.source_sha256
    for cell,p,o,request in requests:
        oracle.target(p['operationId'],request.full_url,plan['principals'][cell['role']]['eventId'])
    report.update(reportedServiceRevision=plan['serviceRevision'],reportedSeedRevision=plan['seedRevision'],contract_sha256={name:hashlib.sha256((ROOT/'contracts/domain/v1'/name).read_bytes()).hexdigest() for name in ('operations.json','policy.json','social.schema.json','common.schema.json')})
    opener=urllib.request.build_opener(NoRedirect());report['HTTP']='IN_PROGRESS';cells={};unavailable=False
    for cell,p,o,request in requests:
        try:response=opener.open(request,timeout=15)
        except urllib.error.HTTPError as error:response=error
        with response:
            require(response.status==p['expectedStatus']);require(re.search(r'(?:^|,)\s*no-store\s*(?:,|$)',response.headers.get('Cache-Control',''),re.I))
            raw=response.read(1048577);require(len(raw)<=1048576)
            principal=plan['principals'][cell['role']]
            value=oracle.response(raw,p['operationId'],response.status,principal['userId'],principal['eventId'],request.full_url)
            decoded=json.dumps(value,separators=(',',':'),ensure_ascii=False)
            require(not any(x in decoded for x in p['forbiddenValues']) and all(x in decoded for x in p['requiredValues']))
            if response.status>=400:
                require(isinstance(value,dict) and set(value)=={'code','message','traceId','timestamp','details'} and isinstance(value['details'],dict) and set(value['details'])=={'reason'})
                require(value['details']['reason'] in o['errors'][str(response.status)])
            unavailable|=response.status in (429,503,504)
        key=(cell['role'],cell['operation']);cells[key]=cells.get(key,0)+1
    require(len(cells)==21)
    report['cells']=[{'role':r,'operation':a,'probes':cells[r,a],'result':'PASS','provenance':'REAL_HTTP'} for r in ROLES for a in AXES]
    report['HTTP']='PARTIAL' if unavailable else 'PASS';report['result']='PARTIAL'
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--plan',type=Path,required=True);p.add_argument('--report',type=Path,required=True);args=p.parse_args()
    if not args.report.is_absolute() or args.report.exists():p.error('new absolute report required')
    report={'task':'QA04','result':'NOT_RUN','HTTP':'NOT_RUN','WS':'NOT_RUN','cachedProductUI':'NOT_RUN','privatePayloadOrCredentialLogging':False,'limitations':['Runtime/source/seed provenance comes from the named runtime owner plan; this harness does not attest process ancestry.','Actual WebSocket/cached-product lifetime and public client behavior remain required.']}
    try:run(args,report)
    except Exception:report.update(result='NOT_RUN' if report['HTTP']=='NOT_RUN' else 'FAIL',HTTP='NOT_RUN' if report['HTTP']=='NOT_RUN' else 'FAIL',reason='Setup unavailable or HTTP matrix assertion failed; private detail withheld')
    args.report.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');print(json.dumps({'result':report['result'],'HTTP':report['HTTP'],'report':str(args.report)}));return 2 if report['result'] in ('NOT_RUN','PARTIAL') else 1
if __name__=='__main__':raise SystemExit(main())
