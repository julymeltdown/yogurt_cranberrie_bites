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
import time
from unittest.mock import patch

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import in03_stack_runtime as inherited
from session_deletion_runtime.enrollment import adapters
from session_deletion_runtime.scenario import exercise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--publisher-binary', type=Path, required=True)
    parser.add_argument('--publisher-sha256', required=True)
    parser.add_argument('--sessions-report', type=Path, required=True)
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
    world, prep, dev = inherited.load_helpers(root)
    mail, principal, provenance = adapters(prep, world, dev,
        options.publisher_binary, options.publisher_sha256, revision, argument('--browser-origin'))
    context, principals, outcome = {}, [], {}
    original_features, original_control = inherited.managed_features, dev.Control

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
                       rooms=json.loads(concert['CONCERT_WORLD_ROOMS_JSON']))

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
            probe_report = {'statuses': {}}
            async def bounded():
                async with asyncio.timeout(max(.001, deadline-time.monotonic())):
                    return await exercise(stack, browser, probe_report, context['event'],
                        context['rooms'], principals, browser.origin, argument('--allocation-id'), deadline)
            outcome.update(asyncio.run(bounded()))
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
                (prep,'principal',provision),(dev,'MailSink',mail),(dev,'Control',SessionsControl)):
                scoped.enter_context(patch.object(obj,name,replacement))
            result = inherited.main(forwarded)
        inherited.require(result == 0 and outcome.get('result') == 'SOURCE_PASS_RESTART_AND_CANCELLATION_OPEN',
                          'sessions-source-round-not-qualified')
    finally:
        # The inherited report separately owns actual cleanup/prepared/guard
        # disposition. A scenario source pass never overrides its failure.
        value = {'scope':'ACTUAL_SERVING_SESSIONS_SOURCE_ROUND','inherited_exit_code':result,
            'source_revision':revision,'scenario':outcome,'whole_be42':'NOT_CLAIMED',
            'physical_restart':'NOT_RUN','cancellation':'NOT_RUN'}
        prep.private_json(options.sessions_report,value)
    return result


if __name__ == '__main__':
    raise SystemExit(main())
