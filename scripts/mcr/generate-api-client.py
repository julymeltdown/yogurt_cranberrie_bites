#!/usr/bin/env python3
"""C07 deterministic public API client generation; never edits canonical inputs."""
from __future__ import annotations
import argparse
from copy import deepcopy
import hashlib
import importlib.util
import json
import re
from pathlib import Path
import sys
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'scripts/mcr/contracts/c07'
CLIENTS = ('contracts/generated/api-client', 'frontend/src/shared/api/generated')
SCHEMA_KEYS = {'$ref','type','properties','required','additionalProperties','items','const','enum','anyOf','oneOf','allOf','if','then','else','minimum','maximum','minLength','maxLength','pattern','format','minItems','maxItems','minProperties','uniqueItems','description','default','example','readOnly','x-mcr-utf8-bytes'}

def dump(v): return json.dumps(v, sort_keys=True, ensure_ascii=False, allow_nan=False, indent=2)+'\n'
def sha(raw): return hashlib.sha256(raw).hexdigest()
def uri_pattern():
    # Port the already-pinned oracle expression; no new frontend dependency.
    import rfc3986_validator
    if rfc3986_validator.__version__!='0.1.1':raise ValueError('C07 URI oracle version mismatch')
    source=rfc3986_validator.URI_RE
    result=[];inside=False;escaped=False;comment=False
    for char in source:
        if comment:
            if char=='\n':comment=False
            continue
        if escaped:result.extend(('\\',char));escaped=False;continue
        if char=='\\':escaped=True;continue
        if char=='[':inside=True
        if char==']':inside=False
        if not inside and char=='#':comment=True;continue
        if not inside and char.isspace():continue
        result.append(char)
    if escaped:raise ValueError('C07 malformed URI oracle expression')
    # Python accepts {,n}; ECMAScript spells the same bound {0,n}.
    return '^(?:'+re.sub(r'\{,([0-9]+)\}',r'{0,\1}',''.join(result))+')$',sha(source.encode())
def load(path):
    def pairs(rows):
        value = {}
        for k,v in rows:
            if k in value: raise ValueError('Duplicate JSON key: '+k)
            value[k]=v
        return value
    return json.loads((ROOT/path).read_text(),object_pairs_hook=pairs,parse_constant=lambda _: (_ for _ in ()).throw(ValueError('Nonfinite JSON')))
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,ROOT/path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def deref(s, doc):
    if '$ref' not in s:return s
    p=s['$ref']; assert p.startswith('#/'),p
    value=doc
    for token in p[2:].split('/'):value=value[token.replace('~1','/').replace('~0','~')]
    return {**deepcopy(value),**{k:v for k,v in s.items() if k!='$ref'}}

def check_schema(s):
    if isinstance(s,bool):return
    if not isinstance(s,dict) or set(s)-SCHEMA_KEYS:raise ValueError('Unsupported schema keyword: '+str(set(s)-SCHEMA_KEYS))
    if 'format' in s and s['format'] not in {'date','date-time','email','int64','password','uri','uuid','binary'}:raise ValueError('Unsupported schema format')
    if 'type' in s and any(k not in {'null','boolean','object','array','number','integer','string'} for k in (s['type'] if isinstance(s['type'],list) else [s['type']])):raise ValueError('Unsupported schema type')
    for k in ('anyOf','oneOf','allOf'):
        for v in s.get(k,[]):check_schema(v)
    for v in s.get('properties',{}).values():check_schema(v)
    for k in ('items','additionalProperties','if','then','else'):
        if k in s and isinstance(s[k],(dict,bool)):check_schema(s[k])

def ts(s):
    if s is True:return 'unknown'
    if s is False:return 'never'
    pieces=[]
    if '$ref' in s:pieces.append('Schemas['+json.dumps(s['$ref'].split('/')[-1])+']')
    if 'const' in s:pieces.append(json.dumps(s['const']))
    elif 'enum' in s:pieces.append('('+' | '.join(json.dumps(x) for x in s['enum'])+')')
    for k in ('anyOf','oneOf'):
        if k in s:pieces.append('('+' | '.join(ts(x) for x in s[k])+')')
    if 'allOf' in s:pieces.extend(ts(x) for x in s['allOf'])
    kind=s.get('type')
    if isinstance(kind,list):pieces.append('('+' | '.join(ts({**s,'type':k}) for k in kind)+')')
    elif kind=='object' or 'properties' in s:
        props=s.get('properties',{});required=s.get('required',[])
        fields=['readonly '+json.dumps(k)+('' if k in required else '?')+': '+ts(v) for k,v in props.items()]
        if s.get('additionalProperties',True) is not False:fields.append('[key: string]: unknown')
        pieces.append('{ '+'; '.join(fields)+' }')
    elif kind=='array':pieces.append('ReadonlyArray<'+ts(s.get('items',True))+'>')
    elif kind in ('integer','number'):pieces.append('number')
    elif kind in ('string','boolean','null'):pieces.append(kind)
    return ' & '.join(pieces) or 'unknown'

def normalized(doc):
    operations={}
    for path, methods in sorted(doc['paths'].items()):
        for method,raw in methods.items():
            if method not in {'get','post','put','patch','delete','head','options','trace'}:continue
            op=deepcopy(raw);identity=op['operationId'];assert identity not in operations
            op['path']=path;op['method']=method.upper()
            op['parameters']=[deref(p,doc) for p in methods.get('parameters',[])+op.get('parameters',[])]
            for p in op['parameters']:
                if p['in'] not in {'path','query','header'}:raise ValueError('Unsupported parameter location')
                check_schema(p['schema'])
            if 'requestBody' in op:
                op['requestBody']=deref(op['requestBody'],doc)
                for media,body in op['requestBody']['content'].items():check_schema(body['schema'])
            for status,response in list(op['responses'].items()):
                response=deref(response,doc);op['responses'][status]=response
                response['headers']={k:deref(v,doc) for k,v in response.get('headers',{}).items()}
                for target in response.get('content',{}).values():check_schema(target['schema'])
            operations[identity]=op
    assert operations
    return {'components':doc['components'],'operations':dict(sorted(operations.items()))}

def group(parameters,location):
    rows=[p for p in parameters if p['in']==location]
    fields=['readonly '+json.dumps(p['name'])+('' if p.get('required') else '?')+': '+ts(p['schema']) for p in rows]
    return '{ '+'; '.join(fields)+' }',any(p.get('required') for p in rows)

def types(doc):
    out='// Generated C07 public structural types. Refinements are enforced by Zod runtime.\nexport interface Schemas {\n'
    for name,schema in sorted(doc['components']['schemas'].items()):out+='  readonly '+json.dumps(name)+': '+ts(schema)+'\n'
    out+='}\nexport interface Operations {\n'
    for identity,op in doc['operations'].items():
        fields=[]
        for location,name in [('path','path'),('query','query'),('header','headers')]:
            value,required=group(op['parameters'],location)
            fields.append('readonly '+name+('' if required else '?')+': '+value)
        body=op.get('requestBody');variants=[]
        if body:
            for media,target in body['content'].items():
                value='Blob' if media.startswith('image/') else ts(target['schema'])
                variants.append('{ readonly mediaType?: '+json.dumps(media)+'; readonly body'+('' if body.get('required') else '?')+': '+value+' }')
        else:variants=['{ readonly body?: never; readonly mediaType?: never }']
        request='{ '+'; '.join(fields)+' } & ('+' | '.join(variants)+')'
        responses=[]
        for status,response in op['responses'].items():
            status_type=status if status.isdigit() else 'number'
            for media,target in (response.get('content') or {None:None}).items():
                value='undefined' if target is None else 'Blob' if media.startswith('image/') else ts(target['schema'])
                responses.append('{ readonly status: '+status_type+'; readonly mediaType: '+json.dumps(media)+'; readonly body: '+value+'; readonly headers: Readonly<Record<string,string>> }')
        out+='  readonly '+json.dumps(identity)+': { request: '+request+'; response: '+' | '.join(responses)+' }\n'
    return out+'}\n'

def render(include_fixtures=True, probe=None):
    source=load('contracts/generated/mcr-domain.openapi.json')
    if probe=='schema-mismatch':
        source['components']['schemas']['Bootstrap']['properties']['__c07_required']={'type':'integer'}
        source['components']['schemas']['Bootstrap']['required'].append('__c07_required')
    if probe=='missing-operation':
        path=next(iter(source['paths']));method=next(k for k in source['paths'][path] if k in {'get','post','put','patch','delete'})
        del source['paths'][path][method]
    doc=normalized(source)
    for s in source['components']['schemas'].values():check_schema(s)
    inputs={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted((ROOT/'contracts').rglob('*.json')) if not any(str(p.relative_to(ROOT)).startswith(q) for q in ('contracts/fixtures/c07/','contracts/generated/api-client/'))}
    inputs.update({str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted((HERE/'templates').iterdir()) if p.is_file()})
    for name in ('scripts/mcr/contracts/qa01-coverage/witness.py','scripts/mcr/validate-core.py','scripts/mcr/requirements-contracts.txt','frontend/package.json','frontend/pnpm-lock.yaml'):
        inputs[name]=sha((ROOT/name).read_bytes())
    inputs['scripts/mcr/generate-api-client.py']=sha(Path(__file__).read_bytes())
    pattern,pattern_sha=uri_pattern()
    inputs['dependency:rfc3986-validator@0.1.1/URI_RE']=pattern_sha
    outputs={}
    files={'types.ts':types(doc),'document.ts':'// Generated C07 public contract. No live authority.\nexport const document = '+dump(doc).rstrip()+'\n','runtime.ts':(HERE/'templates/runtime.ts').read_text(),'mock.ts':(HERE/'templates/mock.ts').read_text(),'index.ts':"export { createClient, ContractError, schema, schemas, accepts } from './runtime.js'\nexport type { ClientOptions, CallOptions } from './runtime.js'\nexport type { Schemas, Operations } from './types.js'\nexport { FixtureClock, createMock } from './mock.js'\nexport type { MockStep } from './mock.js'\n"}
    files['formats.ts']='// Generated from rfc3986-validator 0.1.1; see THIRD_PARTY_NOTICES.txt.\nexport const uriPattern = '+json.dumps(pattern)+'\n'
    files['THIRD_PARTY_NOTICES.txt']=(HERE/'templates/rfc3986-LICENSE.txt').read_text()
    for prefix in CLIENTS:
        for name,value in files.items():outputs[prefix+'/'+name]=value
    if include_fixtures:
        sys.path.insert(0,str(ROOT/'scripts/mcr/contracts/qa01-coverage'))
        from witness import Witnesses, nodes
        core=module('c07_core','scripts/mcr/validate-core.py')
        from jsonschema import FormatChecker
        validator=lambda s:core.CoreValidator({'components':source['components'],**s},format_checker=FormatChecker())
        corpus=[load(str(p.relative_to(ROOT))) for p in sorted((ROOT/'contracts/fixtures').rglob('*.json')) if '/c07/' not in str(p)]
        witnesses=Witnesses(source,validator,corpus);cases=[];compatibility=[]
        def differential(rule, extra=()):
            value=witnesses.make(rule)
            mutations=[value,None,False,42,"UNKNOWN_C07",{},[],{**value,'__c07_unknown':True} if isinstance(value,dict) else None]
            mutations += [9007199254740992, -9007199254740992, 0.5]
            mutations += list(extra)
            if isinstance(value,dict):
                mutations += [{k:v for k,v in value.items() if k!=name} for name in value]
                mutations += [{**value,name:'UNKNOWN_C07'} for name in value]
                mutations += [{**value,name:9007199254740992} for name,child in value.items() if isinstance(child,int) and not isinstance(child,bool)]
            for mutated in mutations:compatibility.append({'schema':rule,'value':mutated,'accept':validator(rule).is_valid(mutated)})
        for name in sorted(source['components']['schemas']):differential({'$ref':'#/components/schemas/'+name})
        formats={
            'date':['0000-01-01','0001-01-01','1900-02-29','2000-02-29','2026-11-07\n'],
            'date-time':['0000-01-01T00:00:00Z','0001-01-01T00:00:00Z','2026-11-07T23:59:60Z'],
            'uri':['http://','urn:example:item','custom:','https://한글.example/','https://example.test/é','https://example.test/[x]','https://example.test/%C3%A9','http://[::1]/','https://example.test/\n','https://example.test/\x01'],
        }
        for node in nodes(source['components']['schemas']):
            if isinstance(node,dict) and node.get('format') in formats:differential(node,formats[node['format']])
        for identity,op in doc['operations'].items():
            request={}
            for location,name in [('path','path'),('query','query'),('header','headers')]:
                values={p['name']:witnesses.make(p['schema']) for p in op['parameters'] if p['in']==location}
                if values:request[name]=values
            if op.get('requestBody'):
                media,target=next(iter(op['requestBody']['content'].items()));request['mediaType']=media
                request['body']=None if media.startswith('image/') else witnesses.make(target['schema'])
            for status,response in op['responses'].items():
                headers={k:witnesses.make(v['schema']) for k,v in response.get('headers',{}).items()}
                for media,target in (response.get('content') or {None:None}).items():
                    value=None if target is None or media.startswith('image/') else witnesses.make(target['schema'])
                    if target is not None and not media.startswith('image/'):
                        differential(target['schema'])
                    cases.append({'operation':identity,'request':request,'status':int(status),'mediaType':media,'body':value,'headers':headers})
        outputs['contracts/fixtures/c07/responses.json']=dump({'clock':'2030-01-01T00:00:00.000Z','seed':424242,'kind':'schema witnesses and mock transport only','cases':cases})
        outputs['contracts/fixtures/c07/compatibility.json']=dump({'cases':compatibility})
        outputs['contracts/fixtures/c07/index.json']=dump({'files':['responses.json','compatibility.json'],'operations':len(doc['operations']),'responses':len(cases),'clock':'2030-01-01T00:00:00.000Z','seed':424242})
    manifest={'generator':'scripts/mcr/generate-api-client.py','source':'contracts/generated/mcr-domain.openapi.json','operations':len(doc['operations']),'components':len(doc['components']['schemas']),'inputs':inputs,'outputs':{k:sha(v.encode()) for k,v in sorted(outputs.items())}}
    outputs['contracts/generated/api-client/manifest.json']=dump(manifest)
    return outputs

def verify(expected, root=ROOT):
    paths=[p for prefix in (*CLIENTS,'contracts/fixtures/c07') for p in (root/prefix).rglob('*')]
    if any(p.is_symlink() or (not p.is_file() and not p.is_dir()) for p in paths):raise ValueError('C07 nonregular output')
    actual={str(p.relative_to(root)) for p in paths if p.is_file()}
    if actual!=set(expected):raise ValueError('C07 missing/unexpected output: '+str(sorted(actual^set(expected))))
    for name,value in expected.items():
        if (root/name).read_bytes()!=value.encode():raise ValueError('C07 generated drift: '+name)

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--probe',choices=['schema-mismatch','missing-operation','output-drift']);a=p.parse_args()
    expected=render(probe=a.probe)
    if a.probe:
        if a.probe=='output-drift':expected['frontend/src/shared/api/generated/index.ts']='// drift\n'
    if a.check or a.probe:
        verify(expected)
        print(dump({'result':'PASS','outputs':len(expected),'operations':json.loads(expected['contracts/generated/api-client/manifest.json'])['operations']}).strip())
    else:
        for name,value in expected.items():
            path=ROOT/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(value,encoding='utf-8',newline='\n')
        print('Generated C07 public API client, schemas and deterministic fixture catalog')
if __name__=='__main__':
    try:main()
    except Exception as error: print('C07 FAIL: '+str(error),file=sys.stderr);raise SystemExit(1)
