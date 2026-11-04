#!/usr/bin/env python3
"""Actual disposable Social HTTP flow; private plans and no secret output.

No request is sent without an explicit private disposable-runtime plan.
A successful two-principal flow remains PARTIAL for the original privacy goal.
"""
from __future__ import annotations
import hashlib
import argparse
import importlib.util
import json
import os
from pathlib import Path
import secrets
import urllib.error
import urllib.parse
import urllib.request
from uuid import UUID, uuid4

class Refused(Exception): pass
def require(value):
    if not value: raise Refused()
def canonical(value):
    require(isinstance(value,str) and str(UUID(value))==value and UUID(value).int)
    return value
def load_matrix(repo):
    path=repo/'backend/services/concert-service/tests/authorization/social_matrix_support/http_matrix.py'
    spec=importlib.util.spec_from_file_location('social_actual_matrix',path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module
def load_oracle(repo):
    path=repo/'backend/services/concert-service/tests/social_live_support/contract_oracle.py'
    spec=importlib.util.spec_from_file_location('social_http_contract_oracle',path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module.ContractOracle(repo)

class Flow:
    def __init__(self,repo,plan,report):
        self.report=report;self.ops={o['operationId']:o for o in json.loads((repo/'contracts/domain/v1/operations.json').read_text())}
        require(plan.get('version')==1 and plan.get('isolatedEnvironment') is True and plan.get('disposablePrincipals') is True and plan.get('allowSeedMutations') is True)
        origin=urllib.parse.urlsplit(plan['origin']);require(not origin.path and not origin.query and not origin.fragment and not origin.username and not origin.password)
        require(origin.hostname and origin.geturl()==plan['origin'])
        require(origin.scheme=='https' or origin.scheme=='http' and origin.hostname in ('127.0.0.1','localhost','::1'))
        self.origin=plan['origin'];self.event=canonical(plan['eventId']);self.intro=canonical(plan['introId']);self.intro_revision=plan['introRevision']
        require(isinstance(self.intro_revision,int) and 0<self.intro_revision<9007199254740991)
        self.actors=plan['principals'];require(set(self.actors)=={'requester','recipient'})
        for actor in self.actors.values():
            canonical(actor['userId']);require(actor['tokenEnv'].startswith('MCR_') and os.environ.get(actor['tokenEnv']))
        require(self.actors['requester']['userId']!=self.actors['recipient']['userId'])
        self.tokens={name:os.environ[a['tokenEnv']] for name,a in self.actors.items()}
        provenance=plan['claimProvenance'];require(provenance['kind'] in ('SYNTHETIC_ACCEPTANCE_FIXTURE','VERIFIABLE_PROVIDER'))
        require(all(isinstance(provenance.get(k),str) and 0<len(provenance[k])<=512 for k in ('issuer','sourceRevision','ownerEvidence')))
        require(all(isinstance(plan.get(k),str) and 0<len(plan[k])<=512 for k in ('serviceRevision','seedRevision')))
        self.report.update(claimProvenanceKind=provenance['kind'],reportedServiceRevision=plan['serviceRevision'],reportedSeedRevision=plan['seedRevision'])
        self.secret={name:'mcr_'+secrets.token_hex(10) for name in ('requester','recipient','replacement')}
        self.opener=urllib.request.build_opener(urllib.request.ProxyHandler({}),load_matrix(repo).NoRedirect())
        self.oracle=load_oracle(repo)
        self.report['response_oracle_sha256']=self.oracle.source_sha256
        self.report['contract_sha256']={name:hashlib.sha256((repo/'contracts/domain/v1'/name).read_bytes()).hexdigest() for name in ('operations.json','policy.json','social.schema.json','common.schema.json')}
        self.report['HTTP']='IN_PROGRESS';self.report['checks']=[]
    def call(self,label,role,operation,params=None,query=None,body=None,expected=(200,),key=None,secret=None):
        o=self.ops[operation];method=o['method'].upper();params={'eventId':self.event,**(params or {})};path=o['path']
        for name,value in params.items():path=path.replace('{'+name+'}',str(value))
        require('{' not in path and '}' not in path)
        if query:path+='?'+urllib.parse.urlencode(query)
        headers={'Authorization':'Bearer '+self.tokens[role],'Accept':'application/json','Cache-Control':'no-store'}
        if method!='GET':headers.update({'Origin':self.origin,'X-MCR-Intent':'browser','Idempotency-Key':key or str(uuid4())})
        data=None
        if body is not None:
            require(o['request_location']=='body');data=json.dumps(body,separators=(',',':')).encode();headers['Content-Type']='application/json'
        request=urllib.request.Request(self.origin+path,data=data,headers=headers,method=method)
        self.oracle.target(operation,request.full_url,self.event)
        try: response=self.opener.open(request,timeout=15)
        except urllib.error.HTTPError as error:response=error
        with response:
            require(response.status in expected and 'no-store' in [part.strip().lower() for part in response.headers.get('Cache-Control','').split(',')])
            raw=response.read(1048577);require(len(raw)<=1048576)
            value=self.oracle.response(raw,operation,response.status,self.actors[role]['userId'],self.event,request.full_url)
            encoded=json.dumps(value,ensure_ascii=False)
            for name,canary in self.secret.items():
                require((secret==name and canary in encoded) or (secret!=name and canary not in encoded))
            if response.status>=400:
                require(set(value)=={'code','message','traceId','timestamp','details'} and set(value['details'])=={'reason'})
                require(value['details']['reason'] in o['errors'][str(response.status)])
            self.report['checks'].append({'label':label,'operationId':operation,'status':response.status,'result':'PASS'})
        return value
    def handles(self,role):
        value=self.call(role+'-metadata',role,'listOwnConcertHandles')
        rows=[r for r in value['items'] if r['platform']=='INSTAGRAM'];require(len(rows)==1)
        require(rows[0]['user_id']==self.actors[role]['userId']);return rows[0]['version']
    def set_handle(self,role,version,canary,key=None):
        return self.call(role+'-set-handle',role,'setConcertHandle',{'platform':'INSTAGRAM'},body={'expected_revision':version,'handle':self.secret[canary]},key=key)['version']
    def create(self,a,b):
        value=self.call('explicit-request','requester','createConcertSocialRequest',body={'recipient_id':self.actors['recipient']['userId'],'intro_id':self.intro,'intro_revision':self.intro_revision,'consents':[{'platform':'INSTAGRAM','requester_handle_version':a,'recipient_handle_version':b,'requester_consents':True}]},expected=(200,))
        require(value['state']=='PENDING' and value['revision']==1);return canonical(value['request_id'])
    def approve(self,request):
        key=str(uuid4());body={'expected_revision':1,'decision':'APPROVED','approved_platforms':['INSTAGRAM'],'recipient_consents':True}
        first=self.call('explicit-approval','recipient','decideConcertSocialRequest',{'requestId':request},body=body,key=key)
        second=self.call('identical-decision-replay','recipient','decideConcertSocialRequest',{'requestId':request},body=body,key=key)
        require(first==second and first['state']=='APPROVED' and first['revision']==2)
    def reveal(self,request,role,canary=None):
        value=self.call('explicit-reveal' if canary else 'refused-reveal',role,'revealConcertSocialHandle',{'requestId':request},query={'platform':'INSTAGRAM'},expected=(200,) if canary else (403,404),secret=canary)
        if canary:require(value['handle']==self.secret[canary] and value['owner_id']==self.actors['recipient' if role=='requester' else 'requester']['userId'])
    def run(self):
        a0=self.handles('requester');b0=self.handles('recipient');key=str(uuid4())
        a=self.set_handle('requester',a0,'requester',key);require(a==a0+1)
        require(self.set_handle('requester',a0,'requester',key)==a)
        b=self.set_handle('recipient',b0,'recipient');require(b==b0+1)
        request=self.create(a,b);self.reveal(request,'requester')
        self.call('ordinary-detail','requester','getConcertSocialRequest',{'requestId':request})
        self.call('ordinary-list','requester','listConcertSocialRequests')
        self.approve(request);self.reveal(request,'requester','recipient');self.reveal(request,'recipient','requester')
        b2=self.set_handle('recipient',b,'replacement');require(b2==b+1)
        self.reveal(request,'requester');self.reveal(request,'recipient')
        request=self.create(a,b2);self.approve(request);self.reveal(request,'requester','replacement')
        revoked=self.call('explicit-revoke','requester','revokeConcertSocialRequest',{'requestId':request},body={'expected_revision':2});require(revoked['state']=='REVOKED')
        self.reveal(request,'requester');self.reveal(request,'recipient')
        request=self.create(a,b2);self.approve(request)
        target={'userId':self.actors['recipient']['userId']}
        self.call('local-block','requester','blockConcertUser',target,body={});self.reveal(request,'requester')
        self.call('own-block-metadata','requester','listConcertBlocks')
        self.call('local-unblock','requester','unblockConcertUser',target);self.reveal(request,'requester');self.reveal(request,'recipient')
        pending=self.create(a,b2)
        cancelled=self.call('cancel-pending','requester','cancelConcertSocialRequest',{'requestId':pending},query={'expected_revision':1});require(cancelled['state']=='CANCELLED')
        for role,version in [('requester',a),('recipient',b2)]:
            removed=self.call('remove-own-handle',role,'removeConcertHandle',{'platform':'INSTAGRAM'},query={'expected_revision':version});require(removed['configured'] is False and removed['version']==version+1)
        require(len({c['operationId'] for c in self.report['checks']})==13)
        self.report.update(HTTP='PASS',result='PARTIAL')

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--repo',type=Path,required=True);parser.add_argument('--plan',type=Path,required=True);parser.add_argument('--report',type=Path,required=True);args=parser.parse_args()
    require(args.report.is_absolute() and not args.report.exists())
    report={'result':'NOT_RUN','HTTP':'NOT_RUN','WS':'NOT_RUN','policyBaseline':'NOT_RUN','serverExpiryInvalidation':'NOT_RUN','cachedProductUI':'NOT_RUN','limitations':['No provider qualification is inferred from a selected synthetic acceptance fixture.','Canonical global-Friend block, age-expiry/unsupported cases, seven roles, WS/cache, process ancestry and schema qualification remain separate gates.','0051 database generation/native publisher fixtures do not certify actual policy.baseline or policy.invalidate socket delivery and synchronous product-cache withdrawal.']}
    try:
        repo=args.repo.resolve();plan=load_matrix(repo).private_plan(args.plan);Flow(repo,plan,report).run()
    except Exception:report.update(result='NOT_RUN' if report['HTTP']=='NOT_RUN' else 'FAIL',HTTP='NOT_RUN' if report['HTTP']=='NOT_RUN' else 'FAIL',reason='Private setup unavailable or actual HTTP assertion failed; no private detail emitted')
    args.report.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');print(json.dumps({'result':report['result'],'HTTP':report['HTTP'],'report':str(args.report)}));return 2 if report['result'] in ('NOT_RUN','PARTIAL') else 1
if __name__=='__main__':raise SystemExit(main())
