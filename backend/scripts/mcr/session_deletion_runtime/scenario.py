"""Actual populated serving-World deletion scenario. Never starts on import.

This is one source-round scenario, not full BE42 qualification. Physical restart,
uncertain replies and cancellation require additional original-clock rounds.
"""
from __future__ import annotations
import asyncio
import json
import time
from uuid import UUID


async def exercise(stack, browser, report, event, rooms, principals, origin, allocation, deadline):
    from world_capacity_support.scenarios import Runner
    from world_capacity_support.report import check

    def budget():
        stack.cluster.assert_owned()
        check(time.monotonic() < deadline, 'sessions-original-work-deadline')

    def read(database, query):
        budget()
        return json.loads(stack.cluster.sql(database, query))

    def subject(value):
        # SQL observations accept only canonical UUIDs, never arbitrary input.
        parsed = UUID(value)
        check(parsed.int and str(parsed) == value, 'sessions-uuid')
        return "'" + str(parsed) + "'::uuid"

    victim_user, _, victim_token = principals[0]
    survivor_user, _, _ = principals[1]
    actor = subject(victim_user)
    _, bootstrap = await asyncio.to_thread(browser.request, 'sessions-source-bootstrap',
        f'/api/concert/events/{event}/bootstrap', method='GET', bearer=victim_token)
    config = {
        'allocation_id': allocation, 'gateway_url': stack.ports.url('api-gateway:http'),
        'ws_url': 'ws://' + stack.ports.address('concert-service:world') + '/world-ws',
        'browser_origin': origin, 'event_id': event, 'bootstrap': bootstrap, 'rooms': rooms,
        'principals': [{'bearer': item[2], 'room_index': 0} for item in principals[:2]],
        'schedule': {'arrival_concurrency': 2, 'operation_timeout_seconds': 20,
                     'measurement_seconds': 1, 'total_timeout_seconds': 60, 'input_hz': 1},
    }
    runner = Runner(config, False)
    outcome = {'scope': 'ACTUAL_SERVING_SESSIONS_SOURCE_ROUND', 'result': 'FAIL',
               'physical_restart': 'NOT_RUN', 'cancellation': 'NOT_RUN',
               'whole_be42': 'NOT_CLAIMED', 'positive_identity_or_receipt_sql': False}
    try:
        budget()
        await runner.prepare()
        victim, survivor = runner.clients
        check(victim.actor == victim_user and survivor.actor == survivor_user,
              'sessions-real-principal-bindings')
        check(victim_user in survivor.state.players and survivor_user in victim.state.players,
              'sessions-populated-native-rosters')
        before = await asyncio.to_thread(read, 'mcr_concert',
            'select json_build_object(\'memberships\',count(*)) from concert_world_memberships '
            'where user_id=' + actor + ' and lease_until>clock_timestamp()')
        check(before['memberships'] > 0, 'sessions-nonempty-real-membership-required')
        # The expected close is still observed and acknowledged by the existing
        # real RFC6455 reader. This suppresses only its generic unexpected-close
        # diagnostic; it never creates a close or a native completion receipt.
        victim.expected_restart = True
        _, accepted = await asyncio.to_thread(browser.request, 'sessions-account-delete',
            '/api/auth/account', method='DELETE', expected=202, bearer=victim_token)
        check(isinstance(accepted, dict), 'sessions-actual-delete-response')
        await runner.wait(lambda: victim.peer_disconnected, 'sessions-victim-native-close')
        check(victim.ws.close_ack and victim.close_code not in (None, 1006),
              'sessions-real-close-acknowledgement')
        await runner.wait(lambda: victim_user not in survivor.state.players,
                          'sessions-survivor-observed-removal')
        runner.check_errors()
        saga = await asyncio.to_thread(read, 'mcr_auth',
            'select row_to_json(s) from (select id::text,user_id::text,actor_id::text,'
            'retention_until::text,status from user_deletion_sagas where user_id=' + actor + ') s')
        check(isinstance(saga, dict) and saga['user_id'] == victim_user,
              'sessions-canonical-auth-saga')
        saga_id = subject(saga['id'])
        stop = min(deadline, time.monotonic() + 20)
        step = None
        while time.monotonic() < stop:
            step = await asyncio.to_thread(read, 'mcr_auth',
                'select row_to_json(s) from (select status,attempt_count from user_deletion_steps '
                'where saga_id=' + saga_id + " and target='CONCERT_SESSIONS') s")
            if step and step['status'] == 'SUCCEEDED':
                break
            await asyncio.sleep(.1)
        check(step and step['status'] == 'SUCCEEDED' and step['attempt_count'] > 0,
              'sessions-real-worker-target-ack')
        native = await asyncio.to_thread(read, 'mcr_concert',
            'select json_build_object('
            "'progress',(select row_to_json(p) from (select saga_id::text,"
            'encode(command_binding,\'hex\') as command_binding,encode(source_binding,\'hex\') as source_binding,'
            'removed_count,progress_revision,completed_at::text from concert_world_session_deletion_progress '
            'where user_id=' + actor + ') p),'
            "'victims',(select count(*) from concert_world_session_deletion_victims where user_id=" + actor + '),'
            "'pending',(select count(*) from concert_world_session_deletion_victims where user_id=" + actor + ' and completed_at is null),'
            "'memberships',(select count(*) from concert_world_memberships where user_id=" + actor + '))')
        progress = native['progress']
        check(progress and progress['saga_id'] == saga['id'] and progress['completed_at']
              and progress['progress_revision'] > 0 and progress['removed_count'] > 0
              and len(progress['command_binding']) == len(progress['source_binding']) == 64
              and progress['source_binding'] != '00' * 32 and native['victims'] > 0
              and native['pending'] == native['memberships'] == 0,
              'sessions-nonempty-durable-native-terminal-required')
        # A deleted actor must not get another public World ticket. Keep the
        # exact trusted Auth denial; no mock or refreshed bearer is substituted.
        status, _ = await runner.http('world-tickets', victim_token,
            {'room_id': rooms[0]['room_id'], **dict(zip(
                ('map_id','map_revision','avatar_manifest_id','avatar_manifest_version'), runner.pins))})
        check(status in (401, 403), 'sessions-deleted-reconnect-denied')
        await runner.reconnect([1])
        check(survivor.state.players.keys() == {survivor_user}, 'sessions-survivor-reconnect')
        outcome.update(result='SOURCE_PASS_RESTART_AND_CANCELLATION_OPEN',
            actual_auth_saga=saga, actual_worker_step=step, native=native,
            native_close_code=victim.close_code, native_close_ack=True,
            survivor_reconnect='PASS', deleted_reconnect_status=status)
    finally:
        await asyncio.gather(*(client.stop() for client in runner.clients), return_exceptions=True)
        report['sessions_source_round'] = outcome
    return outcome
