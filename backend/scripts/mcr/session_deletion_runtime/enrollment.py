"""Actual enrollment provisioning for the Sessions-owned seven-service fixture.

The operator case publishes only a synthetic isolated policy. All identities,
challenges, acceptances, sessions and profiles still come from public commands.
No work starts on import, and no canonical source file is modified.
"""
from __future__ import annotations
import json, os, secrets, time
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import parse_qsl, urlsplit
from uuid import UUID, uuid4


def adapters(prep, world, dev, publisher_binary, publisher_sha256, source_revision, frontend_origin):
    """Return exact local test seams for the owning runtime's scoped patch context."""
    import hashlib
    if publisher_binary.is_symlink() or hashlib.sha256(publisher_binary.read_bytes()).hexdigest() != publisher_sha256:
        raise ValueError('Sessions operator executable identity mismatch')
    def ensure_policy(stack):
        output = stack.cluster.root/'session-policy.json'
        if not output.exists():
            env = {**stack.cluster.env,
                   'MCR_SESSION_DELETION_OWNER_ROOT':str(stack.cluster.root),
                   'MCR_SESSION_DELETION_OWNER_TOKEN':stack.cluster.token,
                   'MCR_SESSION_DELETION_OWNER_IDENTITY':f'{stack.cluster.root.stat().st_dev}:{stack.cluster.root.stat().st_ino}',
                   'MCR_SESSION_DELETION_AUTH_URL':stack.cluster.url('mcr_auth'),
                   'MCR_SESSION_DELETION_POLICY_OUTPUT':str(output)}
            dev.migration.execute([str(publisher_binary),'--ignored','--exact',
                'publish_isolated_policy_for_actual_serving_sessions','--nocapture','--test-threads=1'],
                env=env,cwd=stack.cluster.root,phase='sessions-isolated-policy-operator',timeout=15)
        info = output.lstat()
        prep.auth.require(not output.is_symlink() and info.st_uid == os.getuid() and info.st_mode&0o077 == 0,
                          'private-session-policy-custody')
        value = json.loads(output.read_bytes())
        prep.auth.require(value['productionApproval'] is False, 'isolated-policy-only')
        return value

    return http_adapters(prep, world, dev, ensure_policy, frontend_origin)


def owned_auth_adapters(prep, world, dev, publisher_binary, publisher_sha256, frontend_origin):
    """Use the finite Auth operator EXE in this existing stack, before signup."""
    import hashlib
    publisher_binary = Path(publisher_binary)
    if publisher_binary.is_symlink() or hashlib.sha256(publisher_binary.read_bytes()).hexdigest() != publisher_sha256:
        raise ValueError('Auth operator executable identity mismatch')

    def ensure_policy(stack):
        stack.cluster.assert_owned()
        output = stack.cluster.root/'enrollment-operator-policy.json'
        if not output.exists():
            info = stack.cluster.root.stat()
            env = {**stack.cluster.env,
                'MCR_ENROLLMENT_OPERATOR_ROOT':str(stack.cluster.root),
                'MCR_ENROLLMENT_OPERATOR_TOKEN':stack.cluster.token,
                'MCR_ENROLLMENT_OPERATOR_IDENTITY':f'{info.st_dev}:{info.st_ino}',
                'MCR_ENROLLMENT_OPERATOR_AUTH_URL':stack.cluster.url('mcr_auth'),
                'MCR_ENROLLMENT_OPERATOR_OUTPUT':str(output)}
            dev.migration.execute([str(publisher_binary),'--ignored','--exact',
                'publish_isolated_policy_into_owned_auth','--nocapture','--test-threads=1'],
                env=env,cwd=stack.cluster.root,phase='isolated-auth-policy-operator',timeout=15)
        info = output.lstat()
        prep.auth.require(not output.is_symlink() and info.st_uid == os.getuid()
            and info.st_mode&0o077 == 0 and info.st_size <= 4096,'private-auth-policy-custody')
        value = json.loads(output.read_bytes())
        prep.auth.require(value['productionApproval'] is False and value['databaseCreated'] is False
            and value['authMigrations']==19,'isolated-existing-auth-policy-only')
        return value

    return http_adapters(prep, world, dev, ensure_policy, frontend_origin)


def http_adapters(prep, world, dev, ensure_policy, frontend_origin):
    """Public HTTP provisioning; ensure_policy(stack) must publish real operator input.

    The callback returns policyRevision/acknowledgementIds/productionApproval.
    It must use the owning stack, not create another fixture or insert identities.
    This factory does not claim the callback or its executable is qualified.
    """
    expected_origin = urlsplit(dev.local_origin(frontend_origin))

    class EnrollmentMailSink(dev.MailSink):
        def accept(self, path, delivery_id, payload):
            if path != '/verify' or not isinstance(payload, dict) or 'enrollment_attempt_id' not in payload:
                return super().accept(path, delivery_id, payload)
            self.cluster.assert_owned()
            if set(payload) != {'email','verification_code','expires_at','enrollment_attempt_id','verification_link'}:
                raise ValueError('closed enrollment mail payload required')
            attempt = str(UUID(payload['enrollment_attempt_id']))
            link = urlsplit(payload['verification_link'])
            pairs = parse_qsl(link.query, keep_blank_values=True)
            if not (payload['enrollment_attempt_id'] == attempt and UUID(attempt).int
                    and len(payload['verification_code']) == 6 and payload['verification_code'].isascii()
                    and payload['verification_code'].isdigit() and link.path == '/verify-email'
                    and len(pairs) == 2 and dict(pairs) == {'attemptId':attempt,'code':payload['verification_code']}
                    and (link.scheme,link.netloc) == (expected_origin.scheme,expected_origin.netloc)
                    and not link.username and not link.password and not link.fragment):
                raise ValueError('bound enrollment link required')
            if self.directory.is_symlink() or not self.directory.is_dir():
                raise ValueError('owned mail directory replaced')
            target = self.directory/(str(UUID(delivery_id))+'.json')
            value = {'kind':'VERIFY_EMAIL','delivery_id':str(UUID(delivery_id)),'payload':payload}
            data = dev.encode(value)
            if target.exists() or target.is_symlink():
                return 204 if not target.is_symlink() and target.read_bytes() == data else 409
            if self.count >= 1000:
                return 507
            dev.runtime.private_file(target, data)
            self.count += 1
            return 204

    def principal(stack, browser, ordinal, *, deadline=None, cancel_event=None, signup_gate=None):
        def budget():
            prep.auth.require(cancel_event is None or not cancel_event.is_set(), 'sessions-provision-cancelled')
            prep.auth.require(deadline is None or time.monotonic()<deadline, 'sessions-provision-deadline')
        budget(); operator = ensure_policy(stack)
        prep.auth.require(operator['productionApproval'] is False, 'isolated-policy-only')
        _, policy = browser.request(f'principal-{ordinal}-policy','/api/auth/enrollment/policy',method='GET')
        prep.auth.require(policy['revision']==operator['policyRevision'],'actual-published-policy')
        headers, attempt = browser.request(f'principal-{ordinal}-attempt','/api/auth/enrollment/attempts',
            {'policyRevision':policy['revision']},expected=201)
        cookie = headers.get('set-cookie','')
        prep.auth.require('mcr-enrollment-dev=' in cookie and 'HttpOnly' in cookie and 'SameSite=Strict' in cookie
                          and 'Path=/api/auth/enrollment' in cookie, 'actual-bound-cookie')
        cookie = cookie.split(';',1)[0]
        email = f'mcr-sessions-{secrets.token_hex(12)}@example.test'
        password = secrets.token_urlsafe(24)
        budget()
        _, accepted = browser.request(f'principal-{ordinal}-signup','/api/auth/enrollment/signup',
            {'attemptId':attempt['attemptId'],'policyRevision':policy['revision'],
             'acknowledgementIds':operator['acknowledgementIds'],'idempotencyKey':str(uuid4()),
             'email':email,'password':password},expected=202,cookie=cookie)
        prep.auth.require(set(accepted)=={'status','attemptId','expiresAt','resendAvailableAt','serverTime'}
                          and accepted['attemptId']==attempt['attemptId'],'generic-accepted-attempt')
        message = None; mail_stop = min(deadline or time.monotonic()+20,time.monotonic()+20)
        while message is None and time.monotonic()<mail_stop:
            budget(); stack.cluster.assert_owned()
            for path in stack.mail.directory.glob('*.json'):
                prep.auth.require(not path.is_symlink() and path.stat().st_mode&0o077 == 0,'private-session-mail')
                candidate = json.loads(path.read_bytes())
                if candidate['kind']=='VERIFY_EMAIL' and candidate['payload']['email']==email:
                    message = candidate['payload']; break
            if message is None:time.sleep(.1)
        prep.auth.require(message is not None and message['enrollment_attempt_id']==attempt['attemptId'],
                          'actual-matching-enrollment-mail')
        _, verified = browser.request(f'principal-{ordinal}-verify','/api/auth/enrollment/verify',
            {'attemptId':attempt['attemptId'],'code':message['verification_code']},cookie=cookie)
        prep.auth.require(verified['status']=='VERIFIED' and verified['attemptId']==attempt['attemptId']
                          and 'userId' not in verified,'bound-canonical-verification')
        budget()
        headers, login = browser.request(f'principal-{ordinal}-login','/api/auth/login',{'email':email,'password':password})
        user = str(UUID(login['userId']))
        prep.auth.require(UUID(user).int and user==login['userId'] and login['accessToken']
                          and login['refreshToken']=='','actual-login-identity')
        prep.auth.refresh_cookie(headers)
        _, admitted = browser.request(f'principal-{ordinal}-consent','/api/auth/enrollment/admission',
            method='GET',bearer=login['accessToken'])
        if admitted['decision']=='CONSENT_REQUIRED':
            budget()
            _, policy = browser.request(f'principal-{ordinal}-renewed-policy',
                '/api/auth/enrollment/policy',method='GET')
            acknowledgements = [document['requiredAcknowledgementId'] for document in policy['documents']]
            prep.auth.require(acknowledgements and len(set(acknowledgements))==len(acknowledgements),
                              'current-required-acknowledgements')
            _, consent = browser.request(f'principal-{ordinal}-renewed-consent',
                '/api/auth/enrollment/consent',{'policyRevision':policy['revision'],
                'acknowledgementIds':acknowledgements,'idempotencyKey':str(uuid4())},
                bearer=login['accessToken'])
            prep.auth.require(consent['decision']=='ADMITTED'
                              and consent['acceptedRevision']==policy['revision'],'explicit-renewed-consent')
            budget()
            _, admitted = browser.request(f'principal-{ordinal}-renewed-admission',
                '/api/auth/enrollment/admission',method='GET',bearer=login['accessToken'])
        prep.auth.require(admitted['decision']=='ADMITTED'
                          and admitted['policyRevision']==policy['revision']
                          and admitted['acceptedRevision']==policy['revision'],'actual-current-consent')
        path = stack.cluster.root/f'in01-principal-{ordinal}.json'
        prep.private_json(path,{'user_id':user,'email':email,'password':password,
            'login_path':'/api/auth/login','access_token':'MEMORY_ONLY'})
        budget()
        return user,path,login['accessToken']

    def provenance(stack,browser,report,principals,event,revision,auth_source,expires_at,world_module):
        from world_capacity_support.transport import Contracts
        from world_capacity_support.report import json_value
        actors = {}
        for ordinal,(role,(user,path,token)) in enumerate(zip(('requester','recipient','restricted'),principals),1):
            raw = world_module.public_json(browser,report,f'principal-{ordinal}-canonical-bootstrap',
                f'/api/concert/events/{event}/bootstrap',token,response_bytes=True)
            prep.auth.require(Contracts().bootstrap(json_value(raw),event)==user,'actual-sessions-bootstrap-owner')
            login = json.loads(path.read_bytes())
            prep.auth.require(login['user_id']==user and login['access_token']=='MEMORY_ONLY','sessions-credential-custody')
            statuses = {name:report['statuses'].get(f'principal-{ordinal}-{suffix}')
                for name,suffix in [('signupStatus','signup'),('verifyStatus','verify'),('loginStatus','login')]}
            prep.auth.require(statuses=={'signupStatus':202,'verifyStatus':200,'loginStatus':200},'actual-enrollment-http-status')
            actors[role]={'userId':user,'email':login['email'],'eventId':event,
                'method':'GATEWAY_ENROLLMENT_COOKIE_HTTP_MAIL_VERIFY_LOGIN',**statuses,'bootstrapStatus':200}
        return {'version':'mcr-local-real-auth/v1','kind':'CANONICAL_ENROLLMENT_GATEWAY_PRIVATE_HTTP_MAIL',
            'sourceRevision':revision,'binaryRevision':auth_source,'tool':'session_deletion_runtime/enrollment.py',
            'mailOwner':stack.cluster.token,'controlOwner':stack.cluster.token,'origin':browser.origin,
            'eventId':event,'issuedAt':datetime.now(timezone.utc).isoformat(),
            'expiresAt':datetime.fromtimestamp(expires_at,timezone.utc).isoformat(),
            'custody':'PRIVATE_MODE_0600','actors':actors,'operatorPolicyProductionApproval':False}
    return EnrollmentMailSink,principal,provenance
