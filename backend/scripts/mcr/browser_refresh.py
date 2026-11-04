#!/usr/bin/env python3
"""Qualify the actual Gateway/Auth refresh port in an owned HTTPS Chrome fixture.

No browser profile, database, certificate trust store, or running service is reused.
The proxy only terminates fixture TLS and schedules/drops selected real responses;
it never invents an Auth response. All credentials remain in private runtime memory.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import secrets
import shutil
import selectors
import signal
import ssl
import subprocess
import threading
import time
from unittest.mock import patch
from urllib.parse import urlsplit
from uuid import UUID

import browser_auth_smoke as auth
import dev_stack as dev

ROOT = Path(__file__).resolve().parents[3]
CHROME = Path('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome')


def require(condition, label):
    if not condition:
        raise auth.CheckFailure(label)


def current_readiness(status, body, service):
    """Strict current readyz schema; retained helper predates the new owner probe."""
    try:
        value = json.loads(body)
        expected = {"database": True, **{
            f"{dev.runtime.PREFIX[service].lower()}->{dev.runtime.PREFIX[peer].lower()}":
            service != "auth-service" and not (service == "post-service" and peer in ("profile-service", "notification-service"))
            for peer in dev.runtime.PEERS[service]}}
        if service == "api-gateway":
            expected["signing-keys"] = True
        if service in ("auth-service", "friend-service"):
            expected["concert-owner-authority"] = True
        require(all(type(value[k]) is bool for k in ("ready", "degraded", "draining")), "readyz-flags")
        require(isinstance(value['checks'], dict) and set(value['checks']) == set(expected), "readyz-exact-current-checks")
        for name, check in value['checks'].items():
            require(type(check['required']) is bool and check['required'] == expected[name]
                    and check['status'] in ('up', 'down', 'timed_out', 'not_checked'), "readyz-check-policy")
        computed = not value['draining'] and all(not c['required'] or c['status'] == 'up' for c in value['checks'].values())
        require(value['ready'] == computed and status == (200 if computed else 503)
                and value['degraded'] == any(c['status'] != 'up' for c in value['checks'].values()), "readyz-consistency")
        return {'http_status': status, **{k: value[k] for k in ('ready', 'degraded', 'draining')},
                'checks': {name: {'required': c['required'], 'status': c['status']} for name, c in value['checks'].items()}}
    except (ValueError, KeyError, TypeError):
        raise auth.CheckFailure('readyz-malformed-current-schema') from None


class FixtureTLS:
    """New loopback listener; actual upstream requests only, bounded scheduling."""
    def __init__(self):
        self.upstream = None
        self.lock = threading.Lock()
        self.barrier = None
        self.cookies = []
        self.same_cookie = None
        self.drop_next = False
        self.requests = 0
        self.active = self.peak = 0
        self.conflict_set_cookie = False
        self.cross_site_cookie_absent = None
        self.failure = None
        owner = self
        class Handler(BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'
            def log_message(self, *_):
                pass  # Never log a Cookie, URL query, credential, or response body.
            def do_GET(self):
                self.forward()
            def do_POST(self):
                self.forward()
            def do_OPTIONS(self):
                self.forward()
            def forward(self):
                connection = None
                counted = False
                try:
                    if self.path == '/favicon.ico':
                        self.send_response(204)
                        self.send_header('Content-Length', '0')
                        self.end_headers(); return
                    if self.path == '/':
                        body = b'<!doctype html><title>Owned refresh qualification</title><p>Native Auth port fixture</p>'
                        self.send_response(200)
                        self.send_header('Content-Type', 'text/html')
                        self.send_header('Cache-Control', 'no-store')
                        self.send_header('Content-Security-Policy', "default-src 'none'; connect-src 'self'; form-action https://127.0.0.1:*; script-src 'none'")
                        self.send_header('Content-Length', str(len(body)))
                        self.end_headers(); self.wfile.write(body); return
                    require(owner.upstream is not None, 'proxy-not-ready')
                    require('?' not in self.path and self.path in {
                        '/api/auth/email/signup', '/api/auth/email/verify', '/api/auth/login',
                        '/api/auth/refresh', '/api/auth/logout', '/api/auth/account'}, 'proxy-path-refused')
                    size = int(self.headers.get('Content-Length', '0'))
                    require(0 <= size <= 16384, 'proxy-body-limit')
                    body = self.rfile.read(size)
                    headers = {key: value for key, value in self.headers.items()
                               if key.lower() not in {'host', 'connection', 'content-length'}}
                    refresh = self.command == 'POST' and self.path == '/api/auth/refresh'
                    gate = None
                    if refresh:
                        with owner.lock:
                            counted = True
                            owner.requests += 1
                            owner.active += 1; owner.peak = max(owner.peak, owner.active)
                            if self.headers.get('Origin', '').startswith('https://localhost:'):
                                owner.cross_site_cookie_absent = not self.headers.get('Cookie')
                            gate = owner.barrier
                            if gate is not None:
                                owner.cookies.append(self.headers.get('Cookie'))
                        if gate is not None:
                            gate.wait(timeout=8)
                            with owner.lock:
                                if len(owner.cookies) == 2:
                                    owner.same_cookie = bool(owner.cookies[0]) and owner.cookies[0] == owner.cookies[1]
                                    owner.barrier = None
                                    owner.cookies = []
                    connection = http.client.HTTPConnection('127.0.0.1', owner.upstream, timeout=10)
                    connection.request(self.command, self.path, body=body, headers=headers)
                    response = connection.getresponse(); payload = response.read(65537)
                    require(len(payload) <= 65536, 'proxy-response-limit')
                    observed = response.getheaders()
                    if refresh:
                        with owner.lock:
                            # Count native upstream overlap, not downstream socket flushing.
                            owner.active -= 1
                            counted = False
                            if response.status == 409:
                                owner.conflict_set_cookie |= any(k.lower() == 'set-cookie' for k, _ in observed)
                            drop = owner.drop_next and response.status == 200
                            if drop: owner.drop_next = False
                        if drop:
                            self.close_connection = True
                            self.connection.shutdown(2)
                            return
                    self.send_response(response.status)
                    for key, value in observed:
                        if key.lower() not in {'transfer-encoding', 'connection', 'content-length'}:
                            self.send_header(key, value)
                    self.send_header('Content-Length', str(len(payload)))
                    self.end_headers(); self.wfile.write(payload)
                except (BrokenPipeError, ConnectionResetError):
                    pass
                except BaseException as error:
                    owner.failure = str(error) if isinstance(error, auth.CheckFailure) else type(error).__name__
                    self.close_connection = True
                finally:
                    if connection is not None: connection.close()
                    if counted:
                        with owner.lock: owner.active -= 1
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = True
        self.thread = None
        self.origin = f'https://127.0.0.1:{self.server.server_port}'

    def start(self, stack):
        self.upstream = stack.ports.ports['api-gateway:http']
        root = stack.cluster.root
        key, cert = root/'browser-refresh.key', root/'browser-refresh.crt'
        dev.migration.execute(['/opt/homebrew/bin/openssl', 'req', '-x509', '-newkey', 'rsa:2048',
            '-nodes', '-days', '1', '-subj', '/CN=127.0.0.1',
            '-addext', 'subjectAltName=IP:127.0.0.1,DNS:localhost', '-keyout', str(key), '-out', str(cert)],
            env=stack.cluster.env, cwd=root, phase='fixture-cert')
        key.chmod(0o600); cert.chmod(0o600)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); context.load_cert_chain(cert, key)
        self.server.socket = context.wrap_socket(self.server.socket, server_side=True)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True); self.thread.start()
        public = dev.migration.execute(['/opt/homebrew/bin/openssl', 'x509', '-in', str(cert), '-pubkey', '-noout'],
            env=stack.cluster.env, cwd=root, phase='fixture-cert-public').stdout.encode()
        der = subprocess.run(['/opt/homebrew/bin/openssl', 'pkey', '-pubin', '-outform', 'DER'],
            input=public, capture_output=True, check=True).stdout
        self.spki = base64.b64encode(hashlib.sha256(der).digest()).decode()

    def close(self):
        if self.thread is not None:
            self.server.shutdown(); self.thread.join(timeout=5)
            require(not self.thread.is_alive(), 'fixture-TLS-stop')
        self.server.server_close()


BROWSER = r'''
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { createInterface } from 'node:readline';
const lines=createInterface({input:process.stdin}); const incoming=[]; const waiting=[];
lines.on('line',line=>{const value=JSON.parse(line);const next=waiting.shift();next?next(value):incoming.push(value)});
const receive=()=>incoming.length?Promise.resolve(incoming.shift()):new Promise(resolve=>waiting.push(resolve));
const cfg=await receive();
const {chromium}=createRequire(import.meta.url)(cfg.playwright);
const rpc=async(kind,data={})=>{console.log(JSON.stringify({kind,...data}));const r=await receive();assert.equal(r.ok,true,kind);return r.value};
const done=[]; const pass=label=>{done.push(label);console.log(JSON.stringify({kind:'progress',label}))};
let browser;
try {
 browser=await chromium.launch({executablePath:cfg.chrome,headless:true,args:[`--ignore-certificate-errors-spki-list=${cfg.spki}`]});
 const context=await browser.newContext(); const page=await context.newPage(); await page.goto(cfg.origin);
 assert.equal(await page.evaluate(()=>isSecureContext),true);
 async function call(p,path,body,extra={}) {
   return p.evaluate(async({path,body,extra})=>{
    const r=await fetch(path,{method:extra.method??'POST',credentials:extra.credentials??'include',
      headers:{'Content-Type':'application/json',...(extra.intent===false?{}:{'X-MCR-Intent':extra.intent??'browser'}),...(extra.token?{Authorization:`Bearer ${extra.token}`}:{})},
      ...(body===undefined?{}:{body:JSON.stringify(body)})});
    return {status:r.status,cache:r.headers.get('cache-control'),hiddenCookie:r.headers.get('set-cookie')===null,body:r.status===204?null:await r.json()};
   },{path,body,extra});
 }
 function authenticated(result) {assert.equal(result.status,200);assert.equal(result.cache,'no-store');assert(result.hiddenCookie);
   assert.equal(result.body.refreshToken,'');assert.equal(result.body.tokenType,'Bearer');assert.equal(result.body.userId,user);return result.body.accessToken}
 const credentials={email:cfg.email,password:cfg.password};
 let result=await call(page,'/api/auth/email/signup',credentials);assert.equal(result.status,200);
 const code=await rpc('mail'); result=await call(page,'/api/auth/email/verify',{email:cfg.email,code});assert.equal(result.status,200);
 result=await call(page,'/api/auth/login',credentials);const user=result.body.userId;
 assert.match(user,/^[0-9a-f-]{36}$/);let access=authenticated(result); const initialAccess=access;
 await rpc('bind-user',{user});
 const cookie=async(c=context)=>(await c.cookies()).find(x=>x.name==='refresh_token');
 let old=await cookie();assert(old?.httpOnly&&old.secure&&old.sameSite==='Lax'&&old.path==='/');
 assert.equal(old.domain,'127.0.0.1');assert(old.expires>Date.now()/1000);
 assert.equal(await page.evaluate(()=>document.cookie.includes('refresh_token')),false);
 assert.equal(await page.evaluate(()=>localStorage.length+sessionStorage.length),0);pass('actual-HTTPS-Secure-HttpOnly-cookie');
 let snapshot=await rpc('snapshot');
 assert.equal((await call(page,'/api/auth/refresh',undefined,{credentials:'omit'})).status,401);
 assert.equal((await call(page,'/api/auth/refresh',undefined,{intent:false})).status,403);
 assert.equal(await rpc('snapshot'),snapshot);assert.equal((await cookie()).value,old.value);pass('credentials-and-intent-denials-no-effects');
 const hostile=await context.newPage();await hostile.goto(cfg.origin.replace('127.0.0.1','localhost'));
 const navigation=hostile.waitForNavigation();
 await hostile.evaluate(action=>{const f=document.createElement('form');f.method='POST';f.action=action;document.body.append(f);f.submit()},cfg.origin+'/api/auth/refresh');
 assert.equal((await navigation).status(),403);await hostile.close();
 assert.equal(await rpc('snapshot'),snapshot);assert.equal(await rpc('cross-site-cookie'),true);pass('SameSite-cross-site-form-CSRF-no-effects');
 access=authenticated(await call(page,'/api/auth/refresh'));assert.notEqual((await cookie()).value,old.value);
 let state=await rpc('sessions');assert.equal(state.live,1);assert.equal(state.families,1);assert.equal(state.rotated,1);const generation=state.generation;assert(Number.isSafeInteger(generation));pass('actual-rotation-one-live-successor');
 const b=await context.newPage();await b.goto(cfg.origin);const predecessor=await cookie();
 await rpc('arm-race');
 const race=await Promise.all([call(page,'/api/auth/refresh'),call(b,'/api/auth/refresh')]);
 assert.deepEqual(race.map(x=>x.status).sort(),[200,409]);access=authenticated(race.find(x=>x.status===200));
 assert.notEqual((await cookie()).value,predecessor.value);assert.equal(await rpc('race-proof'),true);
 state=await rpc('sessions');assert.equal(state.live,1);assert.equal(state.families,1);assert.equal(state.rotated,2);assert.equal(state.generation,generation);pass('two-real-tabs-same-predecessor-one-winner-benign-409');
 // Create a separate actual family to prove late reuse never revokes unrelated sessions.
 const independent=await browser.newContext();const c=await independent.newPage();await c.goto(cfg.origin);
 const independentAccess=authenticated(await call(c,'/api/auth/login',credentials));
 const expiry=Math.max(...[initialAccess,independentAccess].map(token=>JSON.parse(Buffer.from(token.split('.')[1],'base64url').toString()).exp));
 // Actual JWT clock expiry, including the pinned verifier's default clock-skew leeway.
 const until=Math.max(Date.now()+31000,(expiry+62)*1000);
 pass('waiting-real-JWT-expiry-and-reuse-grace');await new Promise(resolve=>setTimeout(resolve,Math.max(0,until-Date.now())));
 assert.equal((await call(page,'/api/auth/account',undefined,{method:'GET',token:initialAccess})).status,401);
 let before=await rpc('sessions');
 const newAccess=authenticated(await call(c,'/api/auth/refresh'));
 assert.equal((await call(c,'/api/auth/account',undefined,{method:'GET',token:newAccess})).status,404);
 assert.equal((await call(c,'/api/auth/account',undefined,{method:'GET',token:independentAccess})).status,401);
 state=await rpc('sessions');assert.equal(state.rotated,before.rotated+1);
 const current=JSON.stringify(state);
 assert.equal((await call(c,'/api/auth/account',undefined,{method:'GET',token:independentAccess})).status,401);
 assert.equal(JSON.stringify(await rpc('sessions')),current);pass('real-expired-and-stale-401-no-implicit-rotation');
 const stale=await browser.newContext();await stale.addCookies([{name:predecessor.name,value:predecessor.value,url:cfg.origin,httpOnly:true,secure:true,sameSite:'Lax'}]);
 const d=await stale.newPage();await d.goto(cfg.origin);assert.equal((await call(d,'/api/auth/refresh')).status,401);
 assert.equal((await call(page,'/api/auth/refresh')).status,401);
 state=await rpc('sessions');assert.equal(state.live,1);assert(state.generation>generation);
 const afterReuseGeneration=state.generation;
 authenticated(await call(c,'/api/auth/refresh'));pass('reuse-after-real-grace-revokes-only-own-family');
 // Native Web Locks serialize the real shared-cookie writers, without fake HTTP.
 await rpc('reset-peak');
 const second=await independent.newPage();await second.goto(cfg.origin);
 async function locked(p){return p.evaluate(()=>navigator.locks.request('mcr-auth-cookie-v1',{mode:'exclusive'},async()=>{
   const r=await fetch('/api/auth/refresh',{method:'POST',credentials:'include',headers:{'X-MCR-Intent':'browser'}});return r.status}));}
 assert.deepEqual(await Promise.all([locked(c),locked(second)]),[200,200]);assert.equal(await rpc('peak'),1);pass('native-Web-Locks-real-cookie-writers-serialized');
 before=await rpc('sessions');const count=await rpc('refresh-count');const beforeDrop=await cookie(independent);
 await rpc('drop-response');let failed=false;let lostStatus=null;
 // One application fetch. Chromium can internally resend after pre-header EOF.
 try{lostStatus=(await call(c,'/api/auth/refresh')).status}catch{failed=true}
 assert(failed||lostStatus===409);
 const attempts=(await rpc('refresh-count'))-count;assert(attempts===1||attempts===2);
 if(lostStatus===409)assert.equal(attempts,2);
 assert.equal((await cookie(independent)).value,beforeDrop.value);
 state=await rpc('sessions');assert.equal(state.rotated,before.rotated+1);assert.equal(state.live,1);
 await rpc('loss-proof',{transportAttempts:attempts,fetchOutcome:failed?'network_error':'conflict409'});
 assert.equal((await call(c,'/api/auth/refresh')).status,409);pass('lost-response-no-cookie-adoption-or-second-rotation');
 assert.equal((await rpc('sessions')).generation,afterReuseGeneration);pass('real-rotation-security-generation-stable');
 assert.equal((await call(c,'/api/auth/logout')).status,204);assert.equal(await cookie(independent),undefined);
 state=await rpc('sessions');assert.equal(state.live,0);assert(state.generation>afterReuseGeneration);assert.equal((await call(c,'/api/auth/refresh')).status,401);pass('logout-via-ancestor-revokes-successor-and-clears-cookie');
 for(const p of [page,b,c,second]){assert.equal(await p.evaluate(()=>document.cookie.includes('refresh_token')),false);assert.equal(await p.evaluate(()=>localStorage.length+sessionStorage.length),0)}
 console.log(JSON.stringify({kind:'done',checks:done,chrome:browser.version(),userCanonical:true,secretsRecorded:false}));
} catch(error) {console.log(JSON.stringify({kind:'failure',failure:(error?.name??'Error')+' '+String(error?.stack??'').split('\n').filter(x=>x.trim().startsWith('at ')).slice(0,2).join(' ')}));process.exitCode=1}
finally{if(browser)await browser.close();lines.close()}
'''


def browser_run(stack, fixture, report, args):
    email = 'refresh-' + secrets.token_hex(10) + '@example.test'
    password = secrets.token_urlsafe(24)
    user = None
    config = {'origin': fixture.origin, 'spki': fixture.spki, 'chrome': str(args.chrome),
              'playwright': str(args.playwright), 'email': email, 'password': password}
    node = subprocess.Popen([args.node, '--input-type=module', '-e', BROWSER], cwd=stack.cluster.root,
        env={**stack.cluster.env, 'XDG_CONFIG_HOME': str(stack.cluster.root/'browser-config'),
             'XDG_CACHE_HOME': str(stack.cluster.root/'browser-cache')}, stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1, start_new_session=True)
    # Playwright creates an ephemeral profile; TMPDIR and browser caches are fixture-owned.
    report['browser_pid'] = node.pid
    node.stdin.write(json.dumps(config)+'\n');node.stdin.flush()
    selector=selectors.DefaultSelector();selector.register(node.stdout,selectors.EVENT_READ)
    deadline=time.monotonic()+240;completed=False
    buffered=b'';pending=[]
    try:
        while time.monotonic()<deadline:
            if not pending:
                events=selector.select(timeout=1)
                if not events:
                    require(node.poll() is None, 'browser-driver-exited');continue
                chunk=os.read(node.stdout.fileno(),65536)
                require(bool(chunk),'browser-driver-output-ended')
                buffered += chunk
                require(len(buffered)<=131072,'browser-control-output-limit')
                pending=buffered.split(b'\n');buffered=pending.pop()
                if not pending:continue
            request=json.loads(pending.pop(0));kind=request.get('kind')
            if kind=='progress':
                label=request.get('label');require(isinstance(label,str) and len(label)<100,'browser-progress-label')
                report.setdefault('completed_phases', []).append(label)
                print(json.dumps({'phase':label}),flush=True);continue
            if kind=='failure':raise auth.CheckFailure('browser-assertion:'+request.get('failure','Error'))
            if kind=='done':
                report['browser']=request;completed=True;break
            value=None
            if kind=='mail':
                until=time.monotonic()+20
                while time.monotonic()<until:
                    for path in (stack.cluster.root/'mail').glob('*.json'):
                        require(not path.is_symlink() and path.stat().st_mode&0o077==0,'private-mail')
                        row=json.loads(path.read_bytes())
                        if row['kind']=='VERIFY_EMAIL' and row['payload']['email']==email:
                            value=row['payload']['verification_code'];break
                    if value is not None:break
                    time.sleep(.05)
                require(value is not None,'actual-verification-delivery-timeout')
            elif kind=='bind-user':
                user=str(UUID(request['user']));value=True
            elif kind=='snapshot':value=auth.auth_snapshot(stack.cluster)
            elif kind=='sessions':
                require(user is not None,'missing-canonical-user')
                stack.cluster.assert_owned()
                value=json.loads(stack.cluster.sql('mcr_auth',f"select json_build_object('live',count(*) filter(where revoked_at is null and expires_at>clock_timestamp()),'families',count(distinct family_id),'rotated',count(*) filter(where revoked_reason='ROTATED'),'generation',(select generation from auth_concert_user_security_generation where user_id='{user}')) from refresh_tokens where user_id='{user}';"))
            elif kind=='arm-race':
                with fixture.lock:fixture.barrier=threading.Barrier(2);fixture.cookies=[];fixture.same_cookie=None
                value=True
            elif kind=='race-proof':value=fixture.same_cookie is True and not fixture.conflict_set_cookie and fixture.failure is None
            elif kind=='cross-site-cookie':value=fixture.cross_site_cookie_absent
            elif kind=='reset-peak':
                with fixture.lock:require(fixture.active==0,'proxy-active-at-reset');fixture.peak=0
                value=True
            elif kind=='peak':value=fixture.peak
            elif kind=='refresh-count':value=fixture.requests
            elif kind=='drop-response':fixture.drop_next=True;value=True
            elif kind=='loss-proof':
                require(request.get('transportAttempts') in (1,2) and request.get('fetchOutcome') in ('network_error','conflict409'),'loss-proof-shape')
                report['loss_probe']={'application_fetch_calls':1,'transport_attempts':request['transportAttempts'],'outcome':request['fetchOutcome'],'committed_rotations':1,'cookie_adopted':False}
                value=True
            else:raise auth.CheckFailure('unknown-browser-control')
            node.stdin.write(json.dumps({'ok':True,'value':value})+'\n');node.stdin.flush()
        require(completed,'bounded-browser-timeout')
        code=node.wait(timeout=15);report['browser_exit_code']=code;require(code==0,'browser-process-exit')
        require(fixture.failure is None,'proxy-failure')
    finally:
        selector.close()
        if node.poll() is None:
            os.killpg(node.pid, signal.SIGTERM)
            try:node.wait(timeout=10)
            except subprocess.TimeoutExpired:os.killpg(node.pid, signal.SIGKILL);node.wait(timeout=5)
        node.stdin.close();node.stdout.close();node.stderr.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir',type=Path,required=True)
    parser.add_argument('--report',type=Path,required=True)
    parser.add_argument('--chrome',type=Path,default=CHROME)
    parser.add_argument('--playwright',type=Path,required=True)
    parser.add_argument('--node',default=shutil.which('node') or 'node')
    args=parser.parse_args()
    if not args.bin_dir.is_absolute() or not args.report.is_absolute():parser.error('new absolute binary directory/report required')
    for executable in (args.chrome, Path(args.node)):
        if not executable.is_absolute() or not executable.is_file() or not os.access(executable, os.X_OK):
            parser.error('explicit existing Chrome and Node executables required')
    if not args.playwright.is_absolute() or not args.playwright.exists():
        parser.error('explicit existing Playwright module required; this script never installs dependencies')
    fd=os.open(args.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    report={'scope':'BE03-BROWSER-REFRESH actual public cookie/rotation criterion for FE09','result':'FAIL',
        'source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        'script_sha256':dev.runtime.digest(Path(__file__)), 'production_deployment':'NOT_RUN',
        'frontend_production_wiring':'NOT_RUN','OAuth':'NOT_RUN','secrets_recorded':False}
    source_paths=[p for service in ['api-gateway','auth-service'] for p in (ROOT/'backend/services'/service/'src').rglob('*.rs')]
    source_paths += [p for p in (ROOT/'backend/crates/auth-application/src').rglob('*.rs')]
    report['source_sha256']={str(p.relative_to(ROOT)):dev.runtime.digest(p) for p in sorted(source_paths)}
    fixture=FixtureTLS()
    stack=dev.DevStack(args.bin_dir,Path('/opt/homebrew/bin'),Path('/opt/homebrew/bin/redis-server'),Path('/opt/homebrew/bin/openssl'))
    original=dev.development_environments
    def environments(*values,**kwargs):
        envs=original(*values,**kwargs)
        envs['api-gateway'].update(GATEWAY_CORS_ALLOWED_ORIGINS=fixture.origin,GATEWAY_REFRESH_COOKIE_SECURE='true',GATEWAY_REFRESH_COOKIE_DOMAIN='')
        envs['auth-service']['AUTH_ACCESS_TTL_SECONDS']='60'
        return envs
    previous={sig:signal.signal(sig,lambda *_:(_ for _ in ()).throw(KeyboardInterrupt())) for sig in (signal.SIGINT,signal.SIGTERM)}
    try:
        with patch.object(dev,'development_environments',environments), patch.object(dev.runtime,'readiness_value',current_readiness):
            stack.start()
        fixture.start(stack)
        stack.report['configuration'].update(cookie_secure=True, cookie_scope='owned HTTPS terminator to native loopback Gateway only')
        report['fixture']={'origin':fixture.origin,'HTTPS':'local ephemeral certificate; Chrome trusts only this SPKI',
            'refresh_cookie':'Secure HttpOnly SameSite=Lax Path=/ host-only; real issuance',
            'access_ttl_seconds':60,'grace_seconds':30,'readiness':'strict current schema including Auth/Friend concert-owner-authority','other_environment':'owned native DevStack only'}
        browser_run(stack,fixture,report,args)
        report['result']='PASS'
    except BaseException as error:
        report['failure']=(str(error) if isinstance(error,auth.CheckFailure) else error.detail if isinstance(error,dev.SmokeError) else type(error).__name__)
    finally:
        report['proxy_observations'] = {'failure': fixture.failure, 'refresh_requests': fixture.requests,
            'peak_native_refresh_overlap': fixture.peak, 'race_same_cookie': fixture.same_cookie,
            'conflict_set_cookie': fixture.conflict_set_cookie, 'cross_site_cookie_absent': fixture.cross_site_cookie_absent}
        try:fixture.close()
        except BaseException:report['result']='FAIL';report['TLS_cleanup']='FAIL'
        try:stack.close()
        except BaseException:stack.report['cleanup']='FAIL'
        report['owned_stack']=stack.report
        if stack.report.get('cleanup')!='PASS':report['result']='FAIL'
        for sig,handler in previous.items():signal.signal(sig,handler)
        dev.write_fd(fd,report)
    print(json.dumps({'result':report['result'],'cleanup':stack.report.get('cleanup'),'checks':len(report.get('browser',{}).get('checks',[]))}),flush=True)
    return 0 if report['result']=='PASS' else 1

if __name__=='__main__':raise SystemExit(main())
