// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'
import { MemoryRouter } from 'react-router-dom'
import { createWorldAccessBridge, type WorldAccessBridge } from '../bridge'
import { WorldAccessFeatures, WorldNearbyPanel, WorldRoomsPanel } from '../routes'
import { authHarness } from './admitted-auth'
import { eventId, replicationPort, requestFor, userId } from './data'
import { deferred } from '../../../features/auth/__tests__/fixtures/data'
const auths: ReturnType<typeof authHarness>[] = [], bridges: WorldAccessBridge[] = []
const setup = async () => {
  const h = authHarness(); auths.push(h)
  const request = await requestFor(h.auth), replication = replicationPort(request)
  const bridge = createWorldAccessBridge({ auth: h.auth, replication, release: request }); bridges.push(bridge)
  return { ...h, request, replication, bridge }
}
afterEach(() => { cleanup(); bridges.splice(0).forEach(b => b.dispose()); auths.splice(0).forEach(h => h.auth.dispose()); vi.restoreAllMocks() })
describe('captured bootstrap to FE18 authorization bridge', () => {
  it('exposes only the validated captured actor/event/policy release without a new network owner', async () => {
    const h = await setup(); expect(await h.bridge.ready).toBe(true)
    const release = h.bridge.world.read()!
    expect(release.scope).toEqual(h.request.privacyScope); expect(release.capabilities).toEqual(h.request.bootstrap.capabilities)
    expect(Object.isFrozen(release.capabilities.meet)).toBe(true)
    expect(release.replication.read()).toBe(h.replication.read())
    h.bridge.controller.start(); await h.bridge.controller.loadRooms()
    expect(h.bridge.controller.read().nearby.map(p => p.displayName)).toContain('Authorized visitor')
    await h.bridge.controller.switchRoom('00000000-0000-4000-8000-000000000012')
    expect(h.replication.sent).toHaveLength(1); expect(h.bridge.controller.read().switch.phase).toBe('switching')
    h.bridge.dispose(); expect(h.replication.read()?.entities).toHaveLength(2); expect(h.replication.listeners.size).toBe(0)
    expect(h.bridge.world.read()).toBeNull(); expect(h.bridge.controller.read().nearby).toEqual([])
  })
  it.each(['abort', 'bootstrap teardown', 'policy', 'logout', 'event', 'identity'] as const)('withdraws synchronously on %s and never restamps stale authority', async kind => {
    const h = await setup(); await h.bridge.ready
    const abort = new AbortController()
    const bridge = createWorldAccessBridge({ auth: h.auth, replication: h.replication, release: { ...h.request, signal: abort.signal } }); bridges.push(bridge)
    await bridge.ready; bridge.controller.start(); await bridge.controller.loadRooms()
    expect(bridge.controller.read().nearby).toHaveLength(2)
    if (kind === 'abort') abort.abort()
    if (kind === 'bootstrap teardown') bridge.withdraw()
    if (kind === 'policy') h.auth.privacy.changePolicy(h.request.privacyScope, 'block')
    if (kind === 'logout') void h.auth.logout()
    if (kind === 'event' || kind === 'identity') h.auth.session.replace(h.request.scope.generation, { userId: kind === 'identity' ? '00000000-0000-4000-8000-000000000999' : userId,
      eventId: kind === 'event' ? '00000000-0000-4000-8000-000000000999' : eventId, accessToken: 'replacement-token' })
    expect(bridge.world.read()).toBeNull(); expect(bridge.controller.read().nearby).toEqual([]); expect(bridge.controller.read().rooms.data).toBeNull()
    const count = h.replication.sent.length; await bridge.controller.switchRoom('00000000-0000-4000-8000-000000000012'); expect(h.replication.sent).toHaveLength(count)
    if (kind === 'policy') { expect(h.auth.session.read().generation).toBe(h.request.scope.generation); h.replication.publish(h.replication.read()); expect(bridge.world.read()).toBeNull() }
  })
  it('projects null/foreign release as withdrawal and permits matching data only under the same surviving lease', async () => {
    const h = await setup(); await h.bridge.ready; h.bridge.controller.start(); await h.bridge.controller.loadRooms()
    const original = h.replication.read()!
    h.replication.publish(null); expect(h.bridge.controller.read().nearby).toEqual([]); expect(h.bridge.world.read()!.replication.read()).toBeNull()
    h.replication.publish({ ...original, scope: { ...original.scope, generation: 99 } }); expect(h.bridge.controller.read().nearby).toEqual([])
    h.replication.publish({ ...original, baseline: { ...original.baseline!, selfUserId: '00000000-0000-4000-8000-000000000999' } }); expect(h.bridge.controller.read().nearby).toEqual([])
    h.replication.publish(original); expect(h.bridge.controller.read().nearby).toHaveLength(2)
  })
  it('rejects changed bootstrap projections and an old captured policy without blessing the current one', async () => {
    const h = await setup(); await h.bridge.ready
    const invalid = createWorldAccessBridge({ auth: h.auth, replication: h.replication, release: { ...h.request, bootstrap: { ...h.request.bootstrap, worldAvailable: false } } }); bridges.push(invalid)
    expect(await invalid.ready).toBe(false); expect(invalid.world.read()).toBeNull()
    h.auth.privacy.changePolicy(h.request.privacyScope, 'revalidated')
    const stale = createWorldAccessBridge({ auth: h.auth, replication: h.replication, release: h.request }); bridges.push(stale)
    expect(await stale.ready).toBe(false); expect(stale.world.read()).toBeNull(); expect(h.requests).toHaveLength(0)
  })
  it('withdraws before validation completes and fences a late dispatch result', async () => {
    const h = await setup(); h.bridge.withdraw(); expect(await h.bridge.ready).toBe(false); expect(h.bridge.world.read()).toBeNull()
    const next = createWorldAccessBridge({ auth: h.auth, replication: h.replication, release: h.request }); bridges.push(next); await next.ready
    const held = deferred<boolean>(); vi.spyOn(h.replication, 'switchRoom').mockReturnValue(held.promise)
    const release = next.world.read()!, pending = release.replication.switchRoom('00000000-0000-4000-8000-000000000012')
    next.withdraw(); held.resolve(true); expect(await pending).toBe(false); expect(release.replication.read()).toBeNull()
  })
  it('does not reveal a view or dispatch if a structural port read synchronously revokes its bootstrap lease', async () => {
    const h = await setup(); await h.bridge.ready
    const release = h.bridge.world.read()!, original = h.replication.read()!
    vi.spyOn(h.replication, 'read').mockImplementation(() => { h.bridge.withdraw(); return original })
    expect(release.replication.read()).toBeNull()
    expect(await release.replication.switchRoom('00000000-0000-4000-8000-000000000012')).toBe(false)
    expect(h.replication.sent).toHaveLength(0)
  })
  it('still evicts private FE18 state when an external unsubscribe fails', async () => {
    const h = await setup(); await h.bridge.ready; h.bridge.dispose()
    const port = { ...h.replication, subscribe(listener: () => void) { const stop = h.replication.subscribe(listener); return () => { stop(); throw new Error('fixture unsubscribe failure') } } }
    const bridge = createWorldAccessBridge({ auth: h.auth, replication: port, release: h.request }); bridges.push(bridge)
    await bridge.ready; bridge.controller.start(); await bridge.controller.loadRooms(); expect(bridge.controller.read().nearby).toHaveLength(2)
    expect(() => bridge.withdraw()).not.toThrow(); expect(bridge.controller.read().nearby).toEqual([]); expect(bridge.world.read()).toBeNull(); expect(h.replication.listeners.size).toBe(0)
  })
  it('releases partial subscriptions if the external replication port throws during allocation', async () => {
    const h = await setup(); await h.bridge.ready
    const broken = { ...h.replication, subscribe() { throw new Error('fixture subscription failure') } }
    expect(() => createWorldAccessBridge({ auth: h.auth, replication: broken, release: h.request })).toThrow('fixture subscription failure')
    // Subsequent canonical cleanup still completes and reaches the existing bridge.
    h.auth.privacy.changePolicy(h.request.privacyScope, 'block'); expect(h.bridge.world.read()).toBeNull()
  })
})
describe('reusable FE18 route composition helpers', () => {
  it('renders a safe unavailable status without a captured feature provider', () => {
    render(<MemoryRouter><WorldRoomsPanel /><WorldNearbyPanel /></MemoryRouter>)
    expect(screen.getAllByRole('status')).toHaveLength(2); expect(document.querySelector('canvas,img')).toBeNull()
  })
  it('binds both panels to one captured bridge and clears DOM synchronously on privacy and teardown', async () => {
    const h = await setup(); await h.bridge.ready; h.bridge.dispose()
    const tree = render(<MemoryRouter><WorldAccessFeatures auth={h.auth} replication={h.replication} release={h.request}><WorldNearbyPanel /><WorldRoomsPanel /></WorldAccessFeatures></MemoryRouter>)
    await screen.findByRole('link', { name: 'Authorized visitor' }); await screen.findByRole('button', { name: 'Switch to SECOND ROOM' })
    expect(h.replication.listeners.size).toBe(1)
    act(() => { h.auth.privacy.changePolicy(h.request.privacyScope, 'block') })
    expect(screen.queryByRole('link', { name: 'Authorized visitor' })).toBeNull(); expect(screen.queryByText('Current fan')).toBeNull()
    tree.unmount(); expect(h.replication.listeners.size).toBe(0); await waitFor(() => expect(h.replication.sent).toHaveLength(0))
  })
  it('clears an old panel immediately on bootstrap replacement and handles StrictMode teardown without a second subscription', async () => {
    const h = await setup(); await h.bridge.ready; h.bridge.dispose()
    const tree = (release: typeof h.request) => <StrictMode><MemoryRouter><WorldAccessFeatures auth={h.auth} replication={h.replication} release={release}><WorldNearbyPanel /></WorldAccessFeatures></MemoryRouter></StrictMode>
    const mounted = render(tree(h.request)); await screen.findByRole('link', { name: 'Authorized visitor' }); expect(h.replication.listeners.size).toBe(1)
    mounted.rerender(tree({ ...h.request, bootstrap: { ...h.request.bootstrap, event: { ...h.request.bootstrap.event, title: 'Unbound replacement' } } }))
    expect(screen.queryByRole('link', { name: 'Authorized visitor' })).toBeNull()
    await waitFor(() => expect(h.replication.listeners.size).toBe(0)); mounted.unmount(); expect(h.replication.listeners.size).toBe(0)
  })

})
