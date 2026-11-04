#!/usr/bin/env python3
"""Scoped actual seven-service Sessions owner, with inherited guard/cleanup.

Requires a separately admitted original-clock seven-service resource round. The
browser-fixture compile/boot allocation does NOT authorize this controller.
Existing canonical helper files stay unchanged; each adaptation is scoped below.
"""
from __future__ import annotations
import argparse
import asyncio
from contextlib import ExitStack
import json
from pathlib import Path
import sys
import socket
import time
from unittest.mock import patch
from uuid import uuid4

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import in03_stack_runtime as inherited
from session_deletion_runtime.enrollment import adapters
from session_deletion_runtime.scenario import exercise, survivors_after_restart
from session_deletion_runtime.runtime_sql import RuntimeSqlPins


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--publisher-binary', type=Path, required=True)
    parser.add_argument('--publisher-sha256', required=True)
    parser.add_argument('--sessions-report', type=Path, required=True)
    parser.add_argument('--runtime-sql-pins', type=Path, required=True)
    parser.add_argument('--scenario-seconds', type=int, default=60)
    options, forwarded = parser.parse_known_args(argv)
    # Parse only fixed inherited arguments here; its main validates the entire
    # original admission, exact seven binary pins and source/migration closure.
    def argument(name):
        inherited.require(forwarded.count(name) == 1, 'sessions-explicit-input-required')
        return forwarded[forwarded.index(name) + 1]
    inherited.require(1 <= options.scenario_seconds <= 120, 'sessions-bounded-scenario-required')
    inherited.require(options.sessions_report.is_absolute()
        and options.sessions_report.parent.is_dir()
        and not options.sessions_report.exists(), 'sessions-new-private-report-required')
    inputs = inherited.json_object(inherited.private_input(Path(argument('--inputs'))))
    root = Path(inputs['helper_source']['root'])
    revision = inherited.source_revision(inputs['helper_source']['revision'])
    inherited.validate_sources(root, revision)
    sql_pins = RuntimeSqlPins(inherited,root,inputs,options.runtime_sql_pins)
    world, prep, dev = inherited.load_helpers(root)
    mail, principal, provenance = adapters(prep, world, dev,
        options.publisher_binary, options.publisher_sha256, revision, argument('--browser-origin'))
    context, principals, outcome, launches = {}, [], {}, {}
    original_features, original_control = inherited.managed_features, dev.Control
    original_child = dev.runtime.Child
    original_guard = world.QualificationGuard

    class SessionsGuard(original_guard):
        def __init__(self,*args,**kwargs):
            super().__init__(*args,**kwargs)
            context['guard'] = self

        def check(self):
            super().check()
            sql_pins.verify()

    class RestartPorts(dev.runtime.Ports):
        def reserve(self,name,port=0):
            inherited.require(name.split(':',1)[0] in inherited.SERVICES and name not in self.ports
                and type(port) is int and 0<port<65536 and port not in dev.runtime.DEFAULT_PORTS,
                'sessions-exact-restart-port-required')
            handle = socket.socket(socket.AF_INET,socket.SOCK_STREAM)
            try:
                handle.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
                handle.bind(('127.0.0.1',port))
                self.held[name],self.ports[name]=handle,port
                self.used.add(port)
                return port
            except BaseException:
                handle.close()
                raise

    class CapturedChild(original_child):
        def __init__(self, cluster, name, argv, env, ports, reservations, secrets_to_redact=()):
            super().__init__(cluster,name,argv,env,ports,reservations,secrets_to_redact)
            if name in inherited.SERVICES:
                # This remains in RAM only: do not persist the environments,
                # bearer/credential values, signing keys or TLS private bytes.
                launches[name] = (list(argv),dict(env),tuple(reservations))

    def features(envs, concert, urls, signing_pairs):
        original_features(envs, concert, urls, signing_pairs)
        concert['CONCERT_ACCOUNT_DELETION_MODE'] = 'managed'
        envs['auth-service'].update(AUTH_CONCERT_DELETION_MODE='managed',
            AUTH_CONCERT_GRPC_ENDPOINT=urls['concert'], AUTH_CONCERT_DELETION_DEADLINE_MS='5000',
            AUTH_CONCERT_DELETION_SOURCE_ENABLED='true', AUTH_USER_DELETION_WORKER_ENABLED='true',
            AUTH_USER_DELETION_WORKER_INTERVAL_MS='100',
            AUTH_ENROLLMENT_VERIFICATION_URL=argument('--browser-origin') + '/verify-email')
        for name, maximum in (('auth',3),('friend',3),('profile',2),('post',3),('notification',2)):
            envs[name + '-service'][name.upper() + '_DB_POOL_MAX_CONNECTIONS'] = str(maximum)
        envs['api-gateway'].update(GATEWAY_DB_POOL_MAX_CONNECTIONS='2',
            GATEWAY_JOURNAL_DB_POOL_MAX_CONNECTIONS='2', GATEWAY_BACKGROUND_DB_POOL_MAX_CONNECTIONS='1')
        context.update(event=concert['CONCERT_WORLD_EVENT_ID'],
                       rooms=json.loads(concert['CONCERT_WORLD_ROOMS_JSON']),
                       auth_env=envs['auth-service'], concert_endpoint=urls['concert'])

    def provision(*args, **kwargs):
        value = principal(*args, **kwargs)
        principals.append(value)
        return value

    class SessionsControl(original_control):
        def __init__(self, stack):
            # Actual three principals and C03 are already provisioned by the
            # inherited controller. This driver keeps bearer values in RAM.
            browser = prep.auth.Browser(stack.ports.url('api-gateway:http'), {'statuses': {}})
            browser.origin = argument('--browser-origin')
            deadline = min(time.monotonic() + options.scenario_seconds,
                time.monotonic() + float(argument('--allocation-work-deadline-epoch')) - time.time())
            context['guard'].phase('actual-serving-sessions-source-restart',options.scenario_seconds)
            probe_report = {'statuses': {}}
            async def bounded():
                async with asyncio.timeout(max(.001, deadline-time.monotonic())):
                    return await exercise(stack, browser, probe_report, context['event'],
                        context['rooms'], principals, browser.origin, argument('--allocation-id'), deadline)
            outcome.update(asyncio.run(bounded()))
            def replay(phase):
                stack.cluster.assert_owned()
                inherited.require(time.monotonic()<deadline, 'sessions-original-replay-budget')
                env = {**stack.cluster.env,
                    'MCR_SESSION_DELETION_OWNER_ROOT':str(stack.cluster.root),
                    'MCR_SESSION_DELETION_OWNER_TOKEN':stack.cluster.token,
                    'MCR_SESSION_DELETION_OWNER_IDENTITY':f'{stack.cluster.root.stat().st_dev}:{stack.cluster.root.stat().st_ino}',
                    'MCR_SESSION_DELETION_AUTH_URL':stack.cluster.url('mcr_auth'),
                    'MCR_SESSION_DELETION_CONCERT_URL':stack.cluster.url('mcr_concert'),
                    'MCR_SESSION_DELETION_CONCERT_ENDPOINT':context['concert_endpoint'],
                    'MCR_SESSION_DELETION_SUBJECT':principals[0][0],
                    'MCR_SESSION_DELETION_REPLAY_PHASE':phase,
                    'AUTH_SERVICE_CREDENTIAL':context['auth_env']['AUTH_SERVICE_CREDENTIAL']}
                dev.migration.execute([str(options.publisher_binary),'--ignored','--exact',
                    'real_serving_sessions_exact_replay_and_cancelled_lock_wait','--nocapture','--test-threads=1'],
                    env=env,cwd=stack.cluster.root,phase='sessions-actual-replay-'+phase,
                    timeout=max(.001,min(15,deadline-time.monotonic())))
                return json.loads(inherited.private_input(stack.cluster.root/f'sessions-replay-{phase}.json'))
            source_receipt = replay('source')
            inherited.require(set(launches)==inherited.SERVICES,'sessions-seven-captured-children-required')
            inherited.require(stack.cluster.sql('postgres','select count(*) from pg_prepared_xacts')=='0',
                              'sessions-pre-restart-prepared-zero')
            before = int((stack.cluster.data/'postmaster.pid').read_text().splitlines()[0])
            identity = stack.cluster.sql('postgres','select system_identifier::text from pg_control_system()')
            stops = {}
            stop_started = time.monotonic()
            for name in reversed([*dev.runtime.START_ORDER,'concert-service']):
                inherited.require(time.monotonic()<deadline,'sessions-original-restart-budget')
                stops[name] = stack.children[name].stop()
                inherited.require(stops[name].get('exit_code')==0 and not stops[name].get('forced')
                    and not stops[name].get('unexpected_exit'),'sessions-restart-direct-child-stop')
            stack.cluster.stop();stack.cluster.start()
            after = int((stack.cluster.data/'postmaster.pid').read_text().splitlines()[0])
            inherited.require(before!=after and identity==stack.cluster.sql('postgres',
                'select system_identifier::text from pg_control_system()'),'sessions-physical-pg-restart-required')
            # Natural incumbent World lease expiry; no SQL lease mutation.
            time.sleep(max(0,5.1-(time.monotonic()-stop_started)))
            reservations = RestartPorts()
            try:
                for name in [*dev.runtime.START_ORDER,'concert-service']:
                    argv, env, names = launches[name]
                    env = dict(env)
                    if name=='concert-service':
                        env.update(CONCERT_WORLD_OWNER_ID=str(uuid4()),CONCERT_PRODUCER_INSTANCE_ID=str(uuid4()))
                        env['CONCERT_WORLD_INVALIDATION_CONSUMER_ID']=env['CONCERT_WORLD_OWNER_ID']
                    for key in names:reservations.reserve(key,stack.ports.ports[key])
                    child = CapturedChild(stack.cluster,name,argv,env,reservations,names,tuple(env.values()))
                    stack.children[name] = child
                    child.wait_listeners(timeout=max(.001,min(20,deadline-time.monotonic())))
                with patch.object(dev.runtime,'readiness_value',lambda status,body,name:
                    inherited.seven_readiness(status,body,name,prep.current_readiness)):
                    stack.health()
            finally:
                reservations.close()
            restarted_receipt = replay('restart')
            inherited.require(all(source_receipt[key]==restarted_receipt[key] for key in
                ('command','snapshot','sourceBinding','fenceRevision')),'sessions-restart-exact-durable-receipt')
            async def restarted_clients():
                async with asyncio.timeout(max(.001,deadline-time.monotonic())):
                    return await survivors_after_restart(stack,browser,probe_report,context['event'],
                        context['rooms'],principals,browser.origin,argument('--allocation-id'))
            restart_clients = asyncio.run(restarted_clients())
            outcome.update(result='SOURCE_AND_PHYSICAL_RESTART_PASS',physical_restart={
                'before_postmaster_pid':before,'after_postmaster_pid':after,'same_system_identifier':True,
                'direct_child_stops':stops,'exact_receipt_replay':'PASS','native_clients':restart_clients},
                cancellation='ACTUAL_NATIVE_LOCK_WAIT_NO_ACK_EXPIRED_RETRY_AND_FRESH_REPLAY_PASS',
                source_replay=source_receipt,restart_replay=restarted_receipt)
            super().__init__(stack)
            # Do not hold a producer after its assigned native scenario ends.
            self.stopping = True

    result = 1
    try:
        with ExitStack() as scoped:
            for obj, name, replacement in (
                (inherited,'load_helpers',lambda _: (world,prep,dev)),
                (inherited,'managed_features',features),
                (inherited,'auth_provenance',provenance),
                (prep,'principal',provision),(dev,'MailSink',mail),(dev,'Control',SessionsControl),
                (dev.runtime,'Child',CapturedChild),(world,'QualificationGuard',SessionsGuard)):
                scoped.enter_context(patch.object(obj,name,replacement))
            result = inherited.main(forwarded)
        inherited.require(result == 0 and outcome.get('result') == 'SOURCE_AND_PHYSICAL_RESTART_PASS',
                          'sessions-source-round-not-qualified')
    finally:
        # The inherited report separately owns actual cleanup/prepared/guard
        # disposition. A scenario source pass never overrides its failure.
        value = {'scope':'ACTUAL_SERVING_SESSIONS_SOURCE_ROUND','inherited_exit_code':result,
            'source_revision':revision,'scenario':outcome,'whole_be42':'NOT_CLAIMED',
            'runtime_sql':sql_pins.services,
            'physical_restart':outcome.get('physical_restart','NOT_RUN'),
            'cancellation':outcome.get('cancellation','NOT_RUN')}
        prep.private_json(options.sessions_report,value)
    return result


if __name__ == '__main__':
    raise SystemExit(main())
