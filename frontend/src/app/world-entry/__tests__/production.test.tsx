// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { StrictMode } from 'react'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { createProductionWorld } from '../production'
import { readLocalInputConfiguration, validateLocalInputConfiguration } from '../configuration'
import { createProductionPorts } from '../../ports'
import { testPreWorldPolicy } from '../../__tests__/fixtures/ports'
import { createAppRoutes } from '../../router'
import type { WorldComposition } from '../../../world/composition'
import type { WorldEntryRequest } from '../authority'
import { authHarness } from './admitted-auth'
import { requestFor, replicationPort, inputConfiguration, bundle } from './data'
import { deferred } from '../../../features/auth/__tests__/fixtures/data'

const stops: (() => void)[] = []
afterEach(() => { cleanup(); stops.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks() })
async function setup() {
  const h = authHarness(); stops.push(() => h.auth.dispose())
  const request = await requestFor(h.auth)
  const stats = { active: 0, peak: 0 }
  const replication = replicationPort(request), lease = { dispose: vi.fn(), setInputBlocked: vi.fn(), setVisible: vi.fn() }
  const composition: WorldComposition = { mount: vi.fn(() => {
    stats.active++; stats.peak = Math.max(stats.peak, stats.active); let disposed = false
    return { ...lease, dispose() { if (disposed) return; disposed = true; stats.active--; lease.dispose() } }
  }), replication: { ...replication, reconnect: async () => false },
    subscribe: replication.subscribe, read: () => ({ phase: 'ready', runtime: null, renderer: 'unavailable', problem: null }), dispose: vi.fn(), settled: async () => {} }
  const load = vi.fn(async () => composition)
  const production = createProductionWorld({ auth: h.auth, localInput: inputConfiguration, load })
  stops.push(production.dispose)
  const host = document.createElement('div'); document.body.append(host); stops.push(() => host.remove())
  const context = { ...request, signal: new AbortController().signal }
  return { ...h, request, replication, lease, composition, load, production, host, context, stats }
}
const tick = async () => { await new Promise(resolve => setTimeout(resolve, 0)) }

describe('explicit production input configuration', () => {
  it('accepts only a complete reviewed configuration and freezes a detached policy', () => {
    const source = { ...inputConfiguration, movementPolicy: { ...inputConfiguration.movementPolicy } }, config = validateLocalInputConfiguration(source)!
    source.movementPolicy.speedPxPerSecond = 999
    expect(config.movementPolicy.speedPxPerSecond).toBe(120)
    expect(Object.isFrozen(config)).toBe(true); expect(Object.isFrozen(config.movementPolicy)).toBe(true)
    expect(readLocalInputConfiguration(JSON.stringify(inputConfiguration))).toEqual(config)
  })
  it.each([undefined, null, '', '{}', 'not json', JSON.stringify({ ...inputConfiguration, reviewedBy: ' ' }),
    JSON.stringify({ ...inputConfiguration, revision: '' }), JSON.stringify({ ...inputConfiguration, zoom: 0 }),
    JSON.stringify({ ...inputConfiguration, zoom: 9 }), JSON.stringify({ ...inputConfiguration, serverTick: 50 }),
    JSON.stringify({ ...inputConfiguration, movementPolicy: { ...inputConfiguration.movementPolicy, speedPxPerSecond: 0 } }),
    JSON.stringify({ ...inputConfiguration, movementPolicy: { ...inputConfiguration.movementPolicy, footWidthPx: 129 } }),
  ])('fails closed without any input defaults for %s', value => { expect(readLocalInputConfiguration(value)).toBeNull() })
  it('rejects accessors and exotic config without invoking them', () => {
    const getter = vi.fn(() => inputConfiguration.zoom)
    const config = { ...inputConfiguration }; Object.defineProperty(config, 'zoom', { enumerable: true, get: getter })
    expect(validateLocalInputConfiguration(config)).toBeNull(); expect(getter).not.toHaveBeenCalled()
    expect(validateLocalInputConfiguration(Object.create(inputConfiguration))).toBeNull()
  })
})

describe('one lazy production owner and exact shell lifetime', () => {
  it('allocates nothing before mount, publishes the exact bootstrap, and caches controls while loading', async () => {
    const h = await setup(), held = deferred<WorldComposition>(); h.load.mockReturnValue(held.promise)
    expect(h.load).not.toHaveBeenCalled(); expect(h.production.access.read()).toBeNull()
    const lease = h.production.world.mount(h.host, h.context)
    lease.setInputBlocked(false); lease.setVisible(false)
    const binding = h.production.access.read()!
    expect(binding.release.bootstrap).toBe(h.request.bootstrap); expect(binding.release.privacyScope).toEqual(h.request.privacyScope)
    expect(binding.release.signal.aborted).toBe(false)
    expect(() => h.production.world.mount(h.host, h.context)).toThrow()
    held.resolve(h.composition); await tick()
    expect(h.composition.mount).toHaveBeenCalledOnce(); expect(h.lease.setInputBlocked).toHaveBeenLastCalledWith(false); expect(h.lease.setVisible).toHaveBeenLastCalledWith(false)
    const mounted = vi.mocked(h.composition.mount).mock.calls[0]![1]
    expect(mounted.signal).toBe(binding.release.signal); expect(mounted.bootstrap).toBe(h.request.bootstrap)
    lease.dispose(); expect(binding.release.signal.aborted).toBe(true); expect(h.production.access.read()).toBeNull(); expect(h.lease.dispose).toHaveBeenCalledOnce()
    h.production.world.mount(h.host, h.context); await tick()
    expect(h.load).toHaveBeenCalledOnce(); expect(h.composition.mount).toHaveBeenCalledTimes(2)
    expect(binding.replication.read()).toBeNull(); expect(await binding.replication.switchRoom('old-room')).toBe(false)
    expect(h.replication.sent).toHaveLength(0)
  })
  it.each(['absent', 'invalid'] as const)('never imports or opens the world for %s config', async kind => {
    const h = await setup(), world = createProductionWorld({ auth: h.auth, localInput: kind === 'absent' ? null : { ...inputConfiguration, reviewedBy: '' }, load: h.load }); stops.push(world.dispose)
    world.world.mount(h.host, h.context); await tick()
    expect(h.load).not.toHaveBeenCalled(); expect(h.host).toHaveTextContent('The world is unavailable. You can keep browsing.')
    expect(world.access.read()!.replication.read()).toBeNull()
  })
  it.each(['abort', 'policy', 'identity', 'event', 'logout', 'dispose'] as const)('withdraws synchronously on %s and fences delayed allocation', async kind => {
    const h = await setup(), held = deferred<WorldComposition>(); h.load.mockReturnValue(held.promise)
    const controller = new AbortController(); h.production.world.mount(h.host, { ...h.context, signal: controller.signal })
    const binding = h.production.access.read()!
    if (kind === 'abort') controller.abort()
    if (kind === 'policy') h.auth.privacy.changePolicy(h.request.privacyScope, 'block')
    if (kind === 'identity' || kind === 'event') h.auth.session.replace(h.request.scope.generation, { userId: kind === 'identity' ? '00000000-0000-4000-8000-000000000999' : h.request.scope.userId,
      eventId: kind === 'event' ? '00000000-0000-4000-8000-000000000999' : h.request.scope.eventId, accessToken: 'replacement-test-token' })
    if (kind === 'logout') void h.auth.logout()
    if (kind === 'dispose') h.production.dispose()
    expect(binding.release.signal.aborted).toBe(true); expect(h.production.access.read()).toBeNull(); expect(binding.replication.read()).toBeNull()
    held.resolve(h.composition); await tick(); expect(h.composition.mount).not.toHaveBeenCalled()
    if (kind === 'dispose') expect(h.composition.dispose).toHaveBeenCalledOnce()
  })
  it.each(['loader throw', 'loader reject', 'mount throw'] as const)('shows safe unavailable content after %s', async kind => {
    const h = await setup()
    if (kind === 'loader throw') h.load.mockImplementation(() => { throw new Error('private diagnostic') })
    if (kind === 'loader reject') h.load.mockRejectedValue(new Error('private diagnostic'))
    if (kind === 'mount throw') vi.mocked(h.composition.mount).mockImplementation(() => { throw new Error('private diagnostic') })
    h.production.world.mount(h.host, h.context); await tick()
    expect(h.host).toHaveTextContent('The world is unavailable. You can keep browsing.'); expect(h.host).not.toHaveTextContent('private diagnostic')
  })
  it('rolls back partial mount if privacy registration fails', async () => {
    const h = await setup(); vi.spyOn(h.auth.privacy, 'registerCleanup').mockImplementationOnce(() => { throw new Error('failure') })
    expect(() => h.production.world.mount(h.host, h.context)).toThrow('World authority unavailable')
    expect(h.production.access.read()).toBeNull(); expect(h.host).toBeEmptyDOMElement(); expect(h.load).not.toHaveBeenCalled()
  })
})

async function routed(path = '/world', strict = false) {
  const h = await setup(), ports = createProductionPorts(h.auth, { localInput: inputConfiguration, load: h.load })
  stops.push(() => ports.dispose?.())
  // Use the actual root/router/WorldHost with a deterministic authenticated boundary.
  const router = createMemoryRouter(createAppRoutes({ ...ports, readiness: { read: () => 'ready', subscribe: () => () => {} }, preWorldPolicy: testPreWorldPolicy, bootstrap: { load: async () => h.request.bootstrap } }), { initialEntries: [path] })
  stops.push(() => router.dispose())
  const node = <RouterProvider router={router} />
  const view = render(strict ? <StrictMode>{node}</StrictMode> : node)
  await screen.findByRole('navigation', { name: 'Main navigation' })
  return { ...h, ports, router, ...view }
}
describe('default production rooms/nearby routes', () => {
  it('keeps the host, composition and bridge across route changes under StrictMode', async () => {
    const h = await routed('/world/nearby', true)
    expect(await screen.findByRole('link', { name: 'Authorized visitor' })).toBeVisible()
    const host = document.querySelector('[data-world-host]'), binding = h.ports.worldAccess!.read()!, mounts = vi.mocked(h.composition.mount).mock.calls.length
    for (const path of ['/world/rooms', '/world/nearby', '/world', '/feed', '/world/nearby']) {
      await act(() => h.router.navigate(path))
      expect(document.querySelector('[data-world-host]')).toBe(host)
      expect(h.ports.worldAccess!.read()).toBe(binding)
    }
    expect(h.load).toHaveBeenCalledOnce(); expect(h.composition.mount).toHaveBeenCalledTimes(mounts)
    expect(h.stats).toEqual({ active: 1, peak: 1 }); expect(h.replication.listeners.size).toBe(1)
    h.unmount(); expect(binding.release.signal.aborted).toBe(true)
    expect(h.replication.listeners.size).toBe(0); expect(h.stats.active).toBe(0)
    h.ports.dispose?.(); expect(h.replication.listeners.size).toBe(0)
    expect(h.lease.dispose).toHaveBeenCalledTimes(mounts)
  })
  it('routes room switches through the same replication owner', async () => {
    const h = await routed('/world/rooms')
    const button = await screen.findByRole('button', { name: /Switch to/ })
    await waitFor(() => expect(button).toBeEnabled())
    await act(async () => { button.click() })
    expect(h.replication.sent).toHaveLength(1)
    expect(h.load).toHaveBeenCalledOnce(); expect(h.composition.mount).toHaveBeenCalledOnce()
  })
  it.each(['policy', 'logout'] as const)('removes previously rendered people on %s', async kind => {
    const h = await routed('/world/nearby')
    await screen.findByRole('link', { name: 'Authorized visitor' })
    const binding = h.ports.worldAccess!.read()!
    act(() => { if (kind === 'policy') h.auth.privacy.changePolicy(binding.release.privacyScope, 'block'); else void h.auth.logout() })
    expect(screen.queryByText('Authorized visitor')).not.toBeInTheDocument()
    expect(binding.release.signal.aborted).toBe(true); expect(h.ports.worldAccess!.read()).toBeNull()
  })
  it('never loads world composition on a public route with the actual production ports', async () => {
    const h = await setup(), ports = createProductionPorts(h.auth, { localInput: inputConfiguration, load: h.load }); stops.push(() => ports.dispose?.())
    const router = createMemoryRouter(createAppRoutes(ports), { initialEntries: ['/legal/privacy'] }); stops.push(() => router.dispose())
    render(<RouterProvider router={router} />)
    await screen.findByRole('heading', { name: /Privacy/ }); expect(h.load).not.toHaveBeenCalled(); expect(ports.worldAccess!.read()).toBeNull()
  })
  it('replaces bootstrap authority without remounting the enclosing provider', async () => {
    const h = await setup(), ports = createProductionPorts(h.auth, { localInput: inputConfiguration, load: h.load }); stops.push(() => ports.dispose?.())
    const [{ PortsProvider }, { WorldHost }, { default: Provider }] = await Promise.all([import('../../access'), import('../../WorldHost'), import('../root-features')])
    const mount = (request: WorldEntryRequest) => <PortsProvider ports={ports}><Provider><WorldHost scope={request.scope} bootstrap={request.bootstrap} privacyScope={h.request.privacyScope} blocked={false} covered={false} /></Provider></PortsProvider>
    const view = render(mount(h.request)); await waitFor(() => expect(h.composition.mount).toHaveBeenCalledOnce())
    const binding = ports.worldAccess!.read()!, second = await requestFor(h.auth, bundle())
    view.rerender(mount(second)); await waitFor(() => expect(h.composition.mount).toHaveBeenCalledTimes(2))
    expect(binding.release.signal.aborted).toBe(true); expect(ports.worldAccess!.read()!.release.bootstrap).toBe(second.bootstrap)
    expect(h.load).toHaveBeenCalledOnce(); expect(h.lease.dispose).toHaveBeenCalledOnce()
  })
})
