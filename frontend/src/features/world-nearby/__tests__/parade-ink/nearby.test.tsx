// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { WorldNearby } from '../../WorldNearby'
import { projectNearby, type NearbyPerson } from '../../projection'
import { harness, initial, uid } from '../../../world-rooms/__tests__/fixtures/harness'
import { projectCapabilities } from '../../../../app/capabilities'

const owned: ReturnType<typeof harness>[] = []
afterEach(() => { cleanup(); owned.splice(0).forEach(value => value.dispose()) })
const portrait = (person: NearbyPerson) => <span role="img" aria-label={`${person.displayName} supplied portrait`}>Revision {person.avatar?.revision ?? 'unavailable'}</span>
async function setup(renderAvatar: typeof portrait | (() => null) = portrait) {
  const h = harness(); owned.push(h); await h.live()
  const view = render(<MemoryRouter><WorldNearby controller={h.controller} renderAvatar={renderAvatar} /></MemoryRouter>)
  return { ...h, view }
}
function open() { const trigger = screen.getByRole('button', { name: 'View Ghost avatar' }); trigger.focus(); fireEvent.click(trigger); return trigger }
it('copies only the already authorized avatar into an immutable nearby projection', async () => {
  const h = await setup(), view = h.replication.read(), scope = h.auth.privacy.capture()
  const person = projectNearby(view, scope).find(value => value.userId === uid(2))!
  const source = view.entities.find(value => value.player.userId === uid(2))!.player.avatar
  expect(person.avatar).toEqual(source); expect(person.avatar).not.toBe(source)
  expect(Object.isFrozen(person.avatar)).toBe(true)
  expect(person).not.toHaveProperty('position'); expect(person).not.toHaveProperty('distance')
  expect(projectNearby({ ...view, visible: false }, scope)).toEqual([])
  expect(projectNearby(view, { ...scope, generation: scope.generation + 1 })).toEqual([])
})
it('mounts the single popover with real profile action, closes on Escape and restores its trigger without requests', async () => {
  const h = await setup(), requests = h.requests.length, trigger = open()
  const dialog = screen.getByRole('dialog', { name: 'Ghost' })
  expect(within(dialog).getByRole('link', { name: 'View profile' }).getAttribute('href')).toBe(`/people/${uid(2)}`)
  const close = within(dialog).getByRole('button', { name: 'Close avatar details' })
  expect(document.activeElement).toBe(close)
  fireEvent.keyDown(close, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).toBeNull(); expect(document.activeElement).toBe(trigger)
  expect(h.requests).toHaveLength(requests)
})
it('removes the selected name and portrait in the same authorized delta removal render', async () => {
  const h = await setup(); open()
  act(() => h.receive({ protocolVersion: '1.0.0', type: 'delta', streamId: initial.streamId, roomEpoch: initial.roomEpoch, streamSeq: 1, serverTick: 2, ackInputSeq: 0, upserts: [], removes: [uid(2)] }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.queryByRole('img', { name: 'Ghost supplied portrait' })).toBeNull()
  expect(screen.queryByRole('link', { name: 'Ghost' })).toBeNull()
  expect(document.activeElement).toBe(screen.getByRole('region', { name: 'Other people in this world' }))
})
it.each(['policy', 'identity', 'event', 'suspension', 'access', 'resync', 'replacement'] as const)('withdraws selected private content on %s before retaining a stale popover', async kind => {
  const h = await setup(); open(); const release = h.release()!, scope = h.auth.privacy.capture()
  act(() => {
    if (kind === 'policy') h.auth.privacy.changePolicy(scope, 'block')
    else if (kind === 'identity' || kind === 'event') h.auth.session.replace(scope.generation, { userId: kind === 'identity' ? uid(20) : scope.userId!, eventId: kind === 'event' ? uid(99) : scope.eventId, accessToken: 'replacement-test-token' })
    else if (kind === 'suspension') h.replication.setVisible(false)
    else if (kind === 'access') h.publish({ ...release, capabilities: projectCapabilities({ ...release.capabilities, world: { available: false, reason: 'FEATURE_DISABLED' } }) })
    else if (kind === 'replacement') h.receive({ ...initial, cause: 'EPOCH_CHANGED', streamId: uid(501), roomEpoch: uid(502), players: [initial.players[0]] })
    else h.receive({ protocolVersion: '1.0.0', type: 'delta', streamId: initial.streamId, roomEpoch: initial.roomEpoch, streamSeq: 10, serverTick: 20, ackInputSeq: 0, upserts: [], removes: [] })
  })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.queryByRole('img', { name: 'Ghost supplied portrait' })).toBeNull()
  expect(screen.queryByRole('link', { name: 'Ghost' })).toBeNull()
})
it('does not resurrect selection when access is reset and rebound within one React batch', async () => {
  const h = await setup(), release = h.release()!; open()
  act(() => { h.publish(null); h.publish(release) })
  expect(screen.getByRole('link', { name: 'Ghost' })).toBeTruthy()
  expect(screen.queryByRole('dialog')).toBeNull()
})
it('derives updated names and avatar revisions from the current roster without storing the former identity payload', async () => {
  const h = await setup(); open(); const person = initial.players.find(value => value.userId === uid(2))!
  act(() => h.receive({ protocolVersion: '1.0.0', type: 'delta', streamId: initial.streamId, roomEpoch: initial.roomEpoch, streamSeq: 1, serverTick: 2, ackInputSeq: 0, upserts: [{ ...person, displayName: 'Current Ghost', avatar: { ...person.avatar, revision: person.avatar.revision + 1 } }], removes: [] }))
  const dialog = screen.getByRole('dialog', { name: 'Current Ghost' })
  expect(within(dialog).getByRole('img', { name: 'Current Ghost supplied portrait' }).textContent).toBe(`Revision ${person.avatar.revision + 1}`)
  expect(screen.queryByRole('img', { name: 'Ghost supplied portrait' })).toBeNull()
})
it('shows an explicit unavailable portrait when the caller has no compatible current character', async () => {
  await setup(() => null); open()
  expect(within(screen.getByRole('dialog', { name: 'Ghost' })).getByRole('status').textContent).toBe('Portrait unavailable')
  expect(document.querySelector('canvas,img')).toBeNull()
})

it('does not restore an old selection after a controller replacement and return', async () => {
  const h = await setup(); open()
  const next = harness(); owned.push(next); await next.live()
  h.view.rerender(<MemoryRouter><WorldNearby controller={next.controller} renderAvatar={portrait} /></MemoryRouter>)
  expect(screen.queryByRole('dialog')).toBeNull()
  h.view.rerender(<MemoryRouter><WorldNearby controller={h.controller} renderAvatar={portrait} /></MemoryRouter>)
  expect(screen.queryByRole('dialog')).toBeNull()
})
