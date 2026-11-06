// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { waitFor } from '@testing-library/react'
import { createProductionWorld } from '../../world-entry/production'
import { createDefaultConnectionStatus } from '../default-entry'
import { createWorldComposition } from '../../../world/composition'
import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../features/auth/__tests__/fixtures/data'
import { withAdmission } from '../../../features/auth/__tests__/fixtures/admission'
import { project, currentScope, eventId, userId, resolverFor, testData, ticket, snapshot } from '../../../world/composition/__tests__/fixtures/data'
import { MockSocket } from '../../../world/network/mock'
import type { WorldTicketRequest } from '../../../world/network/WorldTransport'
import type { EngineFactory } from '../../../world/runtime'
import { projectWorldLifecycle } from './status'

// Only image decoding and the engine port are isolated; HTTP validation, ticket
// adapter, transport, replication, composition and default observer are actual.
vi.mock('../../../world/map/resources', async original => {
  const module = await original<typeof import('../../../world/map/resources')>()
  return { ...module, loadMap: (input: Parameters<typeof module.loadMap>[0], signal: AbortSignal) => module.loadMap({ ...input,
    decode: async () => {
      const image = new Image(); Object.defineProperties(image, { naturalWidth: { value: 96 }, naturalHeight: { value: 32 } })
      return { image, dispose() {} }
    },
  }, signal) }
})
const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals(); document.body.replaceChildren() })
async function setup(failure?: { status: number; reason: string }) {
  const data = await testData(), requests: Request[] = [], sockets: Socket[] = []
  const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: withAdmission(async input => {
    const request = input as Request; requests.push(request)
    if (!new URL(request.url).pathname.endsWith('/world-tickets')) throw new Error('Unexpected fixture request')
    if (failure) return Response.json({ code: `GATEWAY_${failure.status}`, message: 'private raw server text must never enter status',
      details: { reason: failure.reason } }, { status: failure.status })
    return Response.json(ticket(await request.json() as WorldTicketRequest, requests.length))
  }) })
  stops.push(auth.dispose)
  auth.session.replace(0, { userId, eventId, accessToken: 'memory-only-version-fixture' })
  class Socket extends MockSocket {
    constructor() { super(); sockets.push(this); queueMicrotask(() => { if (this.readyState === 0) this.open() }) }
    override send(text: string) {
      super.send(text); const frame = JSON.parse(text)
      if (frame.type === 'authenticate') queueMicrotask(() => this.receive(JSON.stringify(snapshot(frame.roomId))))
    }
  }
  vi.stubGlobal('WebSocket', Socket)
  const stats = { active: 0, peak: 0, mounts: 0 }
  const engine: EngineFactory = host => {
    stats.active++; stats.mounts++; stats.peak = Math.max(stats.peak, stats.active)
    const canvas = document.createElement('canvas'); host.append(canvas); let retired = false
    return { ready: Promise.resolve(), closed: Promise.resolve(), camera: { surface: () => canvas, center() {}, project: p => p, unproject: p => p,
      read: () => ({ zoom: 1, scrollX: 0, scrollY: 0, width: 390, height: 400 }) }, resize() {}, setVisible() {},
      destroy() { if (!retired) { retired = true; stats.active--; canvas.remove() } } }
  }
  const composition = createWorldComposition({ auth, resolveRelease: resolverFor(auth, data), importEngine: async () => engine })
  const production = createProductionWorld({ auth, localInput: { revision: 'TEST_ONLY', reviewedBy: 'test fixture',
    movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 }, zoom: 1 }, load: async () => composition })
  stops.push(production.dispose)
  const status = createDefaultConnectionStatus({ auth, world: production.access, browser: window, reload() {} })
  stops.push(status.dispose); status.start()
  const host = document.createElement('div'); document.body.append(host)
  const lifetime = new AbortController(), bootstrap = await project(auth, data)
  const lease = production.world.mount(host, { scope: currentScope(auth), bootstrap, signal: lifetime.signal, privacyScope: auth.privacy.capture() })
  const binding = production.access.read()!
  return { auth, production, composition, status, host, lifetime, lease, binding, stats, requests, sockets }
}
it('carries canonical HTTP409 map mismatch through actual admission and terminal destruction to reload-only default status', async () => {
  const h = await setup({ status: 409, reason: 'MAP_VERSION_MISMATCH' })
  await waitFor(() => expect(h.status.controller.read().kind).toBe('version'))
  expect(h.composition.read()).toMatchObject({ phase: 'unavailable', problem: 'map-version' })
  expect(h.composition.replication.read()).toBeNull()
  expect(h.binding.lifecycle?.read()).toMatchObject({ release: h.binding.release, kind: 'version' })
  expect(h.status.controller.read()).toMatchObject({ pending: false, canRetry: false })
  expect(await h.status.controller.retry()).toBe(false)
  expect(h.requests).toHaveLength(1); expect(h.sockets).toHaveLength(0); expect(h.stats).toEqual({ active: 0, peak: 1, mounts: 1 })
  expect([...h.host.querySelectorAll('button')].filter(button => !button.hidden)).toHaveLength(0)
  expect(h.host.textContent).toContain('Reload the page')
  expect(h.host.textContent).not.toMatch(/private raw|memory-only/)
})
it('retains a typed server map mismatch after a previously live world withdraws its roster/game/socket', async () => {
  const h = await setup(); await waitFor(() => expect(h.status.controller.read().kind).toBe('ready'))
  const streamId = h.composition.replication.read()!.baseline!.streamId
  h.sockets[0]!.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'resyncRequired', streamId, reason: 'MAP_VERSION_MISMATCH' }))
  expect(h.status.controller.read()).toMatchObject({ kind: 'version', stale: true, pending: false, canRetry: false })
  expect(h.composition.replication.read()).toBeNull(); expect(h.stats.active).toBe(0); expect(h.sockets[0]!.readyState).toBe(3)
  expect(await h.status.controller.retry()).toBe(false); expect(h.requests).toHaveLength(1)
})
for (const failure of [{ status: 503, reason: 'MAP_VERSION_MISMATCH' }, { status: 503, reason: 'DEPENDENCY_UNAVAILABLE' },
  { status: 409, reason: 'VERSION_CONFLICT' }]) it(`never invents map maintenance/version from unsupported ${failure.status}/${failure.reason}`, async () => {
  const h = await setup(failure)
  await waitFor(() => expect(h.status.controller.read().kind).toBe('unavailable'))
  expect(h.composition.read().problem).not.toBe('map-version')
  expect(h.status.controller.read().pending).toBe(false); expect(h.sockets).toHaveLength(0)
})
it('does not infer map mismatch from close4409 alone', async () => {
  const h = await setup(); await waitFor(() => expect(h.status.controller.read().kind).toBe('ready'))
  h.sockets[0]!.serverClose(4409)
  expect(h.status.controller.read()).toMatchObject({ kind: 'unavailable', canRetry: true })
  expect(h.binding.lifecycle?.read()?.kind).toBe('ready')
})
for (const cause of ['abort', 'policy', 'dispose'] as const) it(`withdraws terminal version and its captured lifecycle on ${cause}`, async () => {
  const h = await setup({ status: 409, reason: 'MAP_VERSION_MISMATCH' })
  await waitFor(() => expect(h.status.controller.read().kind).toBe('version'))
  if (cause === 'abort') h.lifetime.abort()
  if (cause === 'policy') h.auth.privacy.changePolicy(h.binding.release.privacyScope, 'block')
  if (cause === 'dispose') h.production.dispose()
  expect(h.binding.lifecycle?.read()).toBeNull(); expect(h.production.access.read()).toBeNull()
  expect(h.status.controller.read()).toEqual({ kind: 'unavailable', stale: false, pending: false, canRetry: false })
})
it('keeps generic release failure and wire negotiation distinct from maintenance/application version', () => {
  const release = {} as Parameters<typeof projectWorldLifecycle>[0]
  expect(projectWorldLifecycle(release, { phase: 'unavailable', problem: 'release' }).kind).toBe('unavailable')
  expect(projectWorldLifecycle(release, { phase: 'loading', problem: null }).kind).toBe('loading')
})
