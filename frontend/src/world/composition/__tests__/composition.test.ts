// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { waitFor } from '@testing-library/react'
import { createSessionController } from '../../../features/auth/model/session'
import { capabilities, authResponse, isolatedCookiePolicy, deferred } from '../../../features/auth/__tests__/fixtures/data'
import { MockSocket } from '../../network/mock'
import type { EngineFactory } from '../../runtime'
import type { WorldCamera } from '../../camera/port'
import type { WorldTicketRequest } from '../../network/WorldTransport'
import { createWorldComposition, type WorldCompositionOptions, type WorldRendererPort } from '../index'
import { prepareRelease } from '../release'
import { createTicketAcquirer } from '../ticket'
import { currentScope, eventId, project, resolverFor, snapshot, testData, ticket, userId } from './fixtures/data'

const decoded = vi.hoisted(() => ({ releases: 0, calls: 0 }))
vi.mock('../../map/resources', async original => {
  const module = await original<typeof import('../../map/resources')>()
  return { ...module, loadMap: (input: Parameters<typeof module.loadMap>[0], signal: AbortSignal) => module.loadMap({ ...input,
    decode: async () => { decoded.calls++; const image = new Image(); Object.defineProperties(image, { naturalWidth: { value: 96 }, naturalHeight: { value: 32 } })
      let disposed = false; return { image, dispose() { if (!disposed) { disposed = true; decoded.releases++ } } } },
  }, signal) }
})
const releases: (() => void)[] = []
afterEach(() => { releases.splice(0).reverse().forEach(release => release()); vi.restoreAllMocks(); vi.unstubAllGlobals(); document.body.replaceChildren() })
async function fixture(options: { resolve?: WorldCompositionOptions['resolveRelease']; renderer?: WorldRendererPort; closed?: Promise<void>;
  handler?: (request: Request) => Promise<Response> | Response } = {}) {
  decoded.calls = decoded.releases = 0
  const data = await testData(), requests: Request[] = [], sockets: MockSocket[] = []
  const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    const request = input as Request; requests.push(request.clone())
    const path = new URL(request.url).pathname
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    if (options.handler) return options.handler(request)
    if (path.endsWith('/world-tickets')) return Response.json(ticket(await request.clone().json() as WorldTicketRequest, requests.length))
    return new Response(null, { status: 204 })
  } })
  releases.push(() => auth.dispose())
  auth.start(); await waitFor(() => expect(auth.read().emailEnabled).toBe(true))
  expect(await auth.login('fixture@example.test', 'fixture-password')).toBe(true)
  class Socket extends MockSocket {
    readonly accessExpiresAt = new Date(Date.now() + 60000).toISOString()
    constructor() { super(); sockets.push(this); queueMicrotask(() => { if (this.readyState === 0) this.open() }) }
    override send(text: string) {
      super.send(text); const message = JSON.parse(text)
      if (message.type === 'authenticate') queueMicrotask(() => this.receive(JSON.stringify(snapshot(message.roomId, undefined, { accessExpiresAt: this.accessExpiresAt }))))
      if (message.type === 'switchRoom') queueMicrotask(() => this.receive(JSON.stringify(snapshot(message.roomId,
        '00000000-0000-4000-8000-000000000199', { cause: 'ROOM_SWITCH', requestId: message.requestId, accessExpiresAt: this.accessExpiresAt }))))
    }
  }
  vi.stubGlobal('WebSocket', Socket)
  const stats = { engines: 0, peak: 0, mounts: 0, destroyed: 0, visible: true, center: [] as { x: number; y: number }[] }
  const engine: EngineFactory = host => {
    stats.engines++; stats.mounts++; stats.peak = Math.max(stats.peak, stats.engines)
    const canvas = document.createElement('canvas'); host.append(canvas)
    const camera: WorldCamera = { surface: () => canvas, center: point => stats.center.push(point), project: point => point, unproject: point => point,
      read: () => ({ zoom: 1, scrollX: 0, scrollY: 0, width: 390, height: 400 }) }
    let destroyed = false
    return { ready: Promise.resolve(), closed: options.closed ?? Promise.resolve(), camera,
      resize() {}, setVisible(value) { stats.visible = value }, destroy() { if (destroyed) return; destroyed = true; stats.engines--; stats.destroyed++; canvas.remove() } }
  }
  const resolveRelease = options.resolve ?? resolverFor(auth, data)
  const composition = createWorldComposition({ auth, resolveRelease, importEngine: async () => engine,
    ...(options.renderer ? { renderer: options.renderer } : {}) })
  releases.push(() => composition.dispose())
  const host = document.createElement('div'); document.body.append(host)
  const controller = new AbortController(), bootstrap = await project(auth, data)
  const context = { scope: currentScope(auth), bootstrap, privacyScope: auth.privacy.capture(), signal: controller.signal }
  const mount = () => composition.mount(host, context)
  return { auth, data, requests, sockets, composition, stats, host, controller, context, mount, resolveRelease, engine }
}
const live = async (f: Awaited<ReturnType<typeof fixture>>) => { const lease = f.mount(); await waitFor(() => expect(f.composition.replication.read()?.phase, JSON.stringify({ state: f.composition.read(), stats: f.stats })).toBe('live')); return lease }

it('composes existing runtime, canonical login/ticket HTTP, real transport engine and replication with one owner', async () => {
  const f = await fixture(), lease = await live(f)
  expect(f.auth.session.read().userId).toBe(userId)
  expect(f.sockets).toHaveLength(1); expect(f.stats).toMatchObject({ engines: 1, peak: 1, mounts: 1 })
  const request = f.requests.find(r => new URL(r.url).pathname.endsWith('/world-tickets'))!
  expect(request.method).toBe('POST'); expect(request.cache).toBe('no-store'); expect(request.headers.get('X-MCR-Intent')).toBe('browser')
  expect(request.headers.get('Authorization')).toBe('Bearer fixture-not-a-real-credential')
  expect(request.headers.has('Idempotency-Key')).toBe(false)
  expect(await request.clone().json()).toMatchObject({ room_id: f.data.rooms.default_room_id, map_id: f.data.map.map_id,
    avatar_manifest_id: f.data.pin.avatar.manifest_id })
  expect(f.composition.replication.read()).toMatchObject({ spatialPrediction: 'unavailable', entities: expect.any(Array) })
  expect(f.composition.read().renderer).toBe('unavailable')
  expect(f.host.textContent).not.toContain('fixture-not-a-real-credential')
  lease.setInputBlocked(false); lease.setInputBlocked(true); lease.setInputBlocked(false)
  expect(f.stats.mounts).toBe(1); expect(f.sockets).toHaveLength(1)
  lease.dispose(); expect(f.stats.engines).toBe(0); expect(f.composition.replication.read()).toBeNull()
  expect(f.sockets[0]!.readyState).toBe(3); expect([...f.sockets[0]!.listeners.values()].every(set => set.size === 0)).toBe(true)
  await f.composition.settled(); expect(decoded.releases).toBe(decoded.calls)
})

it.each(['missing', 'obsolete', 'suspended'] as const)('fails closed for %s bootstrap authority without allocating a game/socket', async kind => {
  const f = await fixture()
  if (kind === 'missing') f.context.privacyScope = null as never
  if (kind === 'obsolete') f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
  if (kind === 'suspended') f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'suspended')
  expect(f.mount).toThrow('World authority unavailable')
  expect(f.stats.mounts).toBe(0); expect(f.sockets).toHaveLength(0)
})

it.each(['block', 'logout', 'abort'] as const)('synchronously withdraws every resource on %s', async kind => {
  const f = await fixture(), lease = await live(f)
  if (kind === 'block') f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
  if (kind === 'logout') void f.auth.logout()
  if (kind === 'abort') f.controller.abort()
  expect(f.stats.engines).toBe(0); expect(f.sockets[0]!.readyState).toBe(3)
  expect(f.host.childElementCount).toBe(0); expect(f.composition.replication.read()).toBeNull()
  lease.dispose(); await f.composition.settled(); expect(decoded.releases).toBe(decoded.calls)
})

it('fences an abort-resistant release resolver and never allocates its late map or socket', async () => {
  const pending = deferred<Awaited<ReturnType<WorldCompositionOptions['resolveRelease']>>>()
  const f = await fixture({ resolve: () => pending.promise }), lease = f.mount()
  const resolved = await resolverFor(f.auth, f.data)()
  f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'consent-revoked')
  pending.resolve(resolved); await pending.promise; await new Promise(resolve => setTimeout(resolve, 30))
  expect(f.stats.mounts).toBe(0); expect(f.sockets).toHaveLength(0); expect(f.composition.replication.read()).toBeNull()
  lease.dispose()
})

it('releases a decoded map that completes after an abort-resistant asset load is withdrawn', async () => {
  const pending = deferred<void>(); let reads = 0
  const f = await fixture({ resolve: async request => {
    const resolved = await resolverFor(f.auth, f.data)()
    return { ...resolved, privacyScope: request.privacyScope, resources: { read: async resource => {
      if (++reads === 1) await pending.promise
      return resolved.resources.read(resource, new AbortController().signal)
    } } }
  } })
  const lease = f.mount(); await waitFor(() => expect(reads).toBe(1))
  lease.dispose(); pending.resolve()
  await waitFor(() => expect(decoded.releases).toBe(1))
  expect(decoded.calls).toBe(1); expect(f.stats.mounts).toBe(0); expect(f.sockets).toHaveLength(0)
})

it('rejects a duplicate mount and serializes replacement allocation behind the old engine closure', async () => {
  const closed = deferred<void>(), f = await fixture({ closed: closed.promise }), lease = await live(f)
  expect(f.mount).toThrow(); lease.dispose()
  const next = f.mount(); await new Promise(resolve => setTimeout(resolve, 30))
  expect(f.stats.mounts).toBe(1); expect(f.sockets).toHaveLength(1)
  closed.resolve(); await waitFor(() => expect(f.composition.replication.read()?.phase).toBe('live'))
  expect(f.stats).toMatchObject({ mounts: 2, peak: 1 }); expect(f.sockets).toHaveLength(2)
  next.dispose()
})

it('shares the canonical-session resource claim and closure fence across adapter instances', async () => {
  const closed = deferred<void>(), f = await fixture({ closed: closed.promise }), lease = await live(f)
  const second = createWorldComposition({ auth: f.auth, resolveRelease: f.resolveRelease, importEngine: async () => f.engine })
  releases.push(() => second.dispose())
  expect(() => second.mount(f.host, f.context)).toThrow()
  lease.dispose(); const next = second.mount(f.host, f.context)
  await new Promise(resolve => setTimeout(resolve, 20)); expect(f.stats.mounts).toBe(1)
  closed.resolve(); await waitFor(() => expect(second.replication.read()?.phase).toBe('live'))
  expect(f.stats).toMatchObject({ mounts: 2, peak: 1 }); next.dispose()
})

it('hides by disconnecting/evicting, resumes with a fresh ticket and does not create another game', async () => {
  const f = await fixture(), lease = await live(f)
  lease.setVisible(false)
  expect(f.composition.replication.read()).toMatchObject({ phase: 'suspended', entities: [] })
  expect(f.sockets[0]!.readyState).toBe(3)
  lease.setVisible(true); await waitFor(() => expect(f.sockets).toHaveLength(2))
  await waitFor(() => expect(f.composition.replication.read()?.phase).toBe('live'))
  expect(f.stats.mounts).toBe(1)
  expect(f.requests.filter(r => new URL(r.url).pathname.endsWith('/world-tickets'))).toHaveLength(2)
})

it('publishes the stable FE18 release and permits only listed room targets', async () => {
  const f = await fixture(), listener = vi.fn(); f.composition.replication.subscribe(listener)
  await live(f)
  expect(await f.composition.replication.switchRoom('00000000-0000-4000-8000-000000000999')).toBe(false)
  await f.composition.replication.switchRoom(f.data.rooms.items[1]!.room_id)
  await waitFor(() => expect(f.composition.replication.read()?.baseline?.roomId).toBe(f.data.rooms.items[1]!.room_id))
  expect(f.sockets).toHaveLength(1); expect(f.stats.mounts).toBe(1); expect(listener).toHaveBeenCalled()
})

it('preserves current membership and game when C05 rejects a full target room ticket', async () => {
  let issued = 0
  const f = await fixture({ handler: async request => {
    const body = await request.clone().json() as WorldTicketRequest
    return body.room_id === f.data.rooms.default_room_id ? Response.json(ticket(body, ++issued))
      : Response.json({ code: 'TEST', message: 'Test-only full target.', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 409 })
  } })
  await live(f)
  expect(await f.composition.replication.switchRoom(f.data.rooms.items[1]!.room_id)).toBe(false)
  expect(f.composition.replication.read()).toMatchObject({ phase: 'live', baseline: { roomId: f.data.rooms.default_room_id }, problem: 'room-unavailable' })
  expect(f.sockets).toHaveLength(1); expect(f.stats).toMatchObject({ engines: 1, mounts: 1 })
})

it('clears a renderer synchronously and releases a renderer returned after reentrant revocation', async () => {
  let revoke: () => void = () => {}, disposals = 0
  const f = await fixture({ renderer: { mount() { revoke(); return { replace() {}, dispose() { disposals++ } } } } })
  revoke = () => f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
  f.mount(); await waitFor(() => expect(disposals).toBe(1))
  expect(f.stats.engines).toBe(0); expect(f.sockets).toHaveLength(0); expect(f.host.childElementCount).toBe(0)
})

it('treats a C05 server revoke as terminal and releases the game as well as the socket/roster', async () => {
  const f = await fixture(); await live(f)
  f.sockets[0]!.receive(JSON.stringify({ protocolVersion: '1.0.0', type: 'revoke', streamId: f.composition.replication.read()!.baseline!.streamId, reason: 'SUSPENDED' }))
  expect(f.stats.engines).toBe(0); expect(f.host.childElementCount).toBe(0)
  expect(f.sockets[0]!.readyState).toBe(3); expect(f.composition.replication.read()).toBeNull()
})

it('returns an unavailable state and tears down partial resources when the renderer throws', async () => {
  const f = await fixture({ renderer: { mount() { throw new Error('Test renderer failure') } } })
  f.mount(); await waitFor(() => expect(f.composition.read().phase).toBe('unavailable'))
  expect(f.stats.engines).toBe(0); expect(f.sockets).toHaveLength(0)
  expect(f.host.querySelector('.world-controls')).toBeNull()
})

it('offers an explicit resource retry without changing the captured lease or duplicating ownership', async () => {
  let attempts = 0
  const f = await fixture({ resolve: async request => {
    const resolved = await resolverFor(f.auth, f.data)()
    return { ...resolved, privacyScope: request.privacyScope, ...(++attempts === 1 ? { resources: { read: async () => { throw new Error('Test-only first asset failure') } } } : {}) }
  } })
  const lease = f.mount(), generation = f.auth.session.read().generation
  lease.setInputBlocked(false)
  await waitFor(() => expect(f.composition.read().phase).toBe('unavailable'))
  expect(f.stats.mounts).toBe(0)
  lease.setInputBlocked(true); lease.setVisible(false)
  f.host.querySelector<HTMLButtonElement>('button')!.click()
  await waitFor(() => expect(f.composition.read().phase).toBe('ready'))
  expect(f.stats.visible).toBe(false); expect(f.sockets).toHaveLength(0)
  lease.setVisible(true)
  await waitFor(() => expect(f.composition.replication.read()?.phase).toBe('live'))
  expect(f.host.querySelector<HTMLButtonElement>('.world-joystick')!.disabled).toBe(true)
  expect(f.stats).toMatchObject({ mounts: 1, peak: 1 }); expect(attempts).toBe(2)
  expect(f.auth.session.read().generation).toBe(generation)
  lease.dispose()
})

it('keeps lifetime revocation wired while an unavailable resource is awaiting explicit retry', async () => {
  const f = await fixture({ renderer: { mount() { throw new Error('Test-only unavailable renderer') } } })
  f.mount(); await waitFor(() => expect(f.composition.read().phase).toBe('unavailable'))
  f.auth.privacy.changePolicy(f.auth.privacy.capture(), 'block')
  expect(f.host.childElementCount).toBe(0); expect(f.composition.replication.read()).toBeNull()
})

it('withdraws game/socket/renderer after an active renderer rejects a live projection', async () => {
  let disposals = 0, evictions = 0
  const f = await fixture({ renderer: { mount() { return { replace(view) {
    if (view.phase === 'live') throw new Error('Test-only sink failure')
    if (!view.entities.length) evictions++
  }, dispose() { disposals++ } } } } })
  f.mount(); await waitFor(() => expect(disposals).toBe(1))
  expect(f.stats.engines).toBe(0); expect(f.sockets[0]!.readyState).toBe(3)
  expect(f.composition.replication.read()).toBeNull(); expect(evictions).toBeGreaterThan(0)
})

it('forwards FE15 input without moving the camera to a fabricated local position', async () => {
  const f = await fixture(), lease = await live(f); lease.setInputBlocked(false)
  const canvas = f.host.querySelector('canvas')!; canvas.focus()
  canvas.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, code: 'ArrowRight' }))
  await waitFor(() => expect(f.sockets[0]!.sent.some(text => { const m = JSON.parse(text); return m.type === 'input' && m.axisX > 0 })).toBe(true))
  expect(f.stats.center.every(p => p.x === 80 && p.y === 112)).toBe(true)
  lease.setInputBlocked(true)
})

it('rejects stale/mismatched release bindings before any private resource is opened', async () => {
  const f = await fixture(), resolved = await f.resolveRelease({ ...f.context, signal: f.controller.signal })
  const expected = { scope: f.context.scope, privacyScope: f.context.privacyScope, bootstrap: f.context.bootstrap }
  await expect(prepareRelease({ ...resolved, privacyScope: { ...resolved.privacyScope, policyGeneration: 999 } }, expected)).rejects.toThrow()
  await expect(prepareRelease({ ...resolved, roomId: '00000000-0000-4000-8000-000000000999' }, expected)).rejects.toThrow()
  await expect(prepareRelease({ ...resolved, bootstrap: { ...f.data.bootstrap, map_manifest: { ...f.data.map, map_revision: 99 } } }, expected)).rejects.toThrow()
  await expect(prepareRelease({ ...resolved, bootstrap: { ...f.data.bootstrap, server_time: '2026-11-03T01:00:00Z' } }, expected)).rejects.toThrow()
})

it.each([401, 429, 503])('never refreshes or automatically replays a failed C05 ticket POST (%s)', async status => {
  const reason = status === 401 ? 'AUTHENTICATION_REQUIRED' : status === 429 ? 'RATE_LIMITED' : 'DEPENDENCY_UNAVAILABLE'
  const f = await fixture({ handler: () => Response.json({ code: 'FIXTURE', message: 'Test-only ticket rejection.', details: { reason } }, { status }) })
  const resolved = await resolverFor(f.auth, f.data)(), body = { room_id: resolved.roomId, map_id: f.data.map.map_id,
    map_revision: f.data.map.map_revision, avatar_manifest_id: f.data.pin.avatar.manifest_id, avatar_manifest_version: f.data.pin.avatar.manifest_version }
  await expect(createTicketAcquirer(f.auth, f.context.privacyScope)({ scope: f.context.scope, body, signal: f.controller.signal })).rejects.toThrow()
  expect(f.requests.filter(request => new URL(request.url).pathname.endsWith('/world-tickets'))).toHaveLength(1)
  expect(f.requests.filter(request => new URL(request.url).pathname.endsWith('/refresh'))).toHaveLength(0)
})
