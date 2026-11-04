#!/usr/bin/env python3
"""Small real-Auth World pair; reuse the owned IN03 launcher, never its journey claim.

Run with the existing in03_stack_runtime.py CLI and a freshly admitted seven-service
cohort. The motion Concert binary must have its own source/build receipt. Canonical
Auth signup/mail/verify/login happens in the launcher; tokens and full C03 stay in
process memory. No benchmark, fixture authority, browser, or raw wire retention.
"""
import asyncio
import json
import math
import sys
import time
from pathlib import Path
from unittest.mock import patch

# This file is installed under backend/scripts/mcr/world_motion/.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import in03_stack_runtime as launcher
from world_capacity_support.report import check
from world_capacity_support.scenarios import PIN_HTTP, PIN_WS, StreamState, release_pins
from world_capacity_support.transport import Contracts, Network


async def pair(config, principals):
    network, contracts = Network(config), Contracts()
    clients, tasks = [], []
    end = time.monotonic() + 45
    failures = []

    async def until(predicate, label):
        while not predicate():
            check(not failures, failures[0] if failures else label)
            check(time.monotonic() < end, label)
            await asyncio.sleep(.02)

    async def reader(client):
        try:
            while True:
                frame = await client['ws'].receive()
                response = client['state'].accept(frame, time.monotonic())
                if response:
                    await client['ws'].send(response)
                if frame['type'] not in ('snapshot', 'delta'):
                    continue
                if client['opted']:
                    anchor = frame.get('selfMotion')
                    check(isinstance(anchor, dict), 'negotiated-anchor-missing')
                    if frame['type'] == 'snapshot':
                        profile = frame.get('motionProfile')
                        check(profile and profile['version'] == '1.0.0'
                              and profile['geometryHash'] == client['geometry'], 'actual-c06-profile')
                        client['profile'] = profile
                    check(anchor['serverTick'] == frame['serverTick']
                          and frame['ackInputSeq'] <= anchor['consumedInputSeq'] <= client['state'].sent,
                          'anchor-clock-input-binding')
                    position = client['state'].players[client['actor']]['position']
                    if frame['type'] == 'snapshot' or any(row['userId'] == client['actor'] for row in frame['upserts']):
                        check(anchor['position'] == position, 'full-self-anchor-position')
                    client['state'].players[client['actor']]['position'] = dict(anchor['position'])
                    if client['anchor']:
                        check(anchor['consumedInputSeq'] >= client['anchor']['consumedInputSeq']
                              and anchor['droppedTicks'] >= client['anchor']['droppedTicks'],
                              'anchor-counters-regressed')
                    client['anchor'] = anchor
                else:
                    check('motionProfile' not in frame and 'selfMotion' not in frame, 'legacy-shape-changed')
                client['frames'] += 1
                check(client['frames'] <= 1000, 'bounded-pair-frame-count')
        except asyncio.CancelledError:
            raise
        except BaseException as error:
            if not client.get('closing'):
                failures.append(type(error).__name__ + ':' + str(error)[:120])

    try:
        for ordinal, (user, _private_login, token) in enumerate(principals[:2]):
            status, doc = await network.http(f"/api/concert/events/{config['event']}/bootstrap", token)
            check(status == 200 and contracts.bootstrap(doc, config['event']) == user, 'canonical-bootstrap')
            pins = release_pins(doc)
            geometry = doc['map_manifest']['geometry_hash']
            request = {'room_id': config['room'], **dict(zip(PIN_HTTP, pins))}
            status, ticket = await network.http(f"/api/concert/events/{config['event']}/world-tickets", token, request)
            check(status == 200, 'canonical-ticket')
            contracts.validate(ticket, 'WorldTicketResponse')
            auth = {'protocolVersion': '1.0.0', 'type': 'authenticate', 'ticket': ticket['ticket'],
                    'eventId': config['event'], 'roomId': config['room'], **dict(zip(PIN_WS, pins))}
            if ordinal == 0:
                auth['motionProfileVersion'] = '1.0.0'
            ws = await network.websocket()
            client = {'ws': ws, 'state': StreamState(user, config['event'], config['room'], pins, contracts),
                      'actor': user, 'opted': ordinal == 0, 'geometry': geometry,
                      'anchor': None, 'profile': None, 'frames': 0}
            clients.append(client)
            await ws.send(auth)
            tasks.append(asyncio.create_task(reader(client)))
            del doc, ticket, auth
            await until(lambda: client['state'].stream is not None, 'initial-authoritative-snapshot')
        a, b = clients
        await until(lambda: len(a['state'].players) == len(b['state'].players) == 2, 'two-canonical-actors')
        start = dict(a['anchor']['position'])
        moved = False
        for axis in ((1, 0), (0, 1), (-1, 0), (0, -1)):
            packet = a['state'].input(time.monotonic())
            packet.update(axisX=axis[0], axisY=axis[1])
            sequence = packet['seq']
            await a['ws'].send(packet)
            await until(lambda: a['anchor']['consumedInputSeq'] >= sequence
                        and a['state'].ack == sequence and a['anchor']['inputAppliedTicks'] >= 2,
                        'actual-input-application')
            position = a['anchor']['position']
            if math.hypot(position['x'] - start['x'], position['y'] - start['y']) > .001:
                moved = True
                break
        check(moved, 'actual-authoritative-movement')
        stop = a['state'].input(time.monotonic())
        await a['ws'].send(stop)
        sequence = stop['seq']
        await until(lambda: a['state'].ack == sequence and a['anchor']['inputAppliedTicks'] >= 2,
                    'actual-neutral-stop')
        stopped = dict(a['anchor']['position'])
        tick = a['anchor']['serverTick']
        await until(lambda: a['anchor']['serverTick'] >= tick + 2, 'post-stop-anchor')
        check(a['anchor']['position'] == stopped, 'neutral-position-drift')
        await until(lambda: b['state'].players[a['actor']]['position'] == stopped, 'legacy-observes-authority')
        a['closing'] = True
        await a['ws'].abort()
        tasks[0].cancel()
        await asyncio.gather(tasks[0], return_exceptions=True)
        await until(lambda: a['actor'] not in b['state'].players, 'disconnected-peer-removed')
        return {'result': 'PASS', 'canonical_active_users': 2, 'transport': 'REAL_HTTP_AND_WEBSOCKET',
                'authority': 'CANONICAL_AUTH_FRIEND_PROFILE_CONCERT', 'motion_version': '1.0.0',
                'actual_c06_hash_matched': True, 'complete_self_anchor': True,
                'authoritative_movement': True, 'neutral_stop': True, 'legacy_wire_unchanged': True,
                'legacy_observed_motion': True, 'peer_removal': True,
                'frame_counts': [c['frames'] for c in clients],
                'full_c03_retained_or_hashed': False, 'tokens': 'MEMORY_ONLY',
                'performance_qualification': 'DEFERRED_BY_USER', 'original_in03_journey': 'NOT_RUN'}
    finally:
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        await asyncio.gather(*(c['ws'].abort() for c in clients), return_exceptions=True)


def main(argv=None):
    original = launcher.auth_provenance

    def motion_proof(stack, browser, report, principals, event, revision, auth_source, expires_at, world):
        provenance = original(stack, browser, report, principals, event, revision, auth_source, expires_at, world)
        room = stack.cluster.sql('mcr_concert', 'SELECT room_id FROM concert_world_rooms WHERE is_default ORDER BY room_id LIMIT 1;').strip()
        check(room, 'owned-default-room')
        config = {'gateway_url': stack.descriptor()['gateway_url'],
                  'ws_url': 'ws://' + stack.ports.address('concert-service:world') + '/world-ws',
                  'browser_origin': browser.origin, 'event': event, 'room': room}
        report['world_motion_public_pair'] = asyncio.run(pair(config, principals))
        return provenance

    with patch.object(launcher, 'auth_provenance', motion_proof), patch.object(launcher, '__doc__', __doc__):
        return launcher.main(argv)


if __name__ == '__main__':
    raise SystemExit(main())
