import { QueryClient } from '@tanstack/react-query'
import { createHttpClient, createMemorySession } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createMuralSync } from '../controller'
import { createMuralHttp } from '../http'
import { createMuralTextureSink } from '../../../../world/mural/sink'
import type { MuralScenePort } from '../../../../world/mural/sink'
import type { Clock, MuralAuthority, MuralAuthorityPort, MuralDelta, PublicPixel } from '../contract'
import type { MuralCommandPort, MuralSnapshot, PixelCommand, PixelOperation } from '../../editor/contract'
import type { WallToolPort } from '../../editor/tools'
import { snapshot as editorSnapshot, ids as editorIds } from '../../editor/__tests__/fixture'
export const ids = { ...editorIds, other: '00000000-0000-4000-8000-000000000005', room: '00000000-0000-4000-8000-000000000006',
  epoch: '00000000-0000-4000-8000-000000000007', stream: '00000000-0000-4000-8000-000000000008', next: '00000000-0000-4000-8000-000000000009' }
export const time = '2026-11-04T00:00:00Z', timestamp = Date.parse(time)
export const snapshot = (patch: Partial<MuralSnapshot> = {}) => editorSnapshot({ cooldown_ms: 0, ...patch })
export const pixel = (revision: number, patch: Partial<PublicPixel> = {}): PublicPixel => ({ operation_id: `00000000-0000-4000-8000-${String(1000 + revision).padStart(12, '0')}`,
  event_id: ids.event, revision, x: revision % 128, y: 1, color_index: revision % 16, committed_at: time, ...patch })
export const delta = (from: number, operations: PublicPixel[], reset = false, to = from + operations.length): MuralDelta => ({ event_id: ids.event,
  from_revision: from, to_revision: to, reset_required: reset, operations })
export const drain = async () => { for (let i = 0; i < 20; i++) await Promise.resolve() }
export function testClock(): Clock & { advance(ms: number): Promise<void>; count(): number } {
  let at = 0, serial = 0; const pending = new Map<number, { at: number; run: () => void }>()
  return { now: () => at, set(ms, run) { const id = ++serial; pending.set(id, { at: at + ms, run }); return id }, clear(id) { pending.delete(id as number) }, count: () => pending.size,
    async advance(ms) {
      const until = at + ms
      for (;;) { const next = Array.from(pending).sort((a, b) => a[1].at - b[1].at)[0]; if (!next || next[1].at > until) break
        at = next[1].at; pending.delete(next[0]); next[1].run(); await drain() }
      at = until; await drain()
    } }
}
export function authorityPort() {
  let value: MuralAuthority | null = { binding: { eventId: ids.event, roomId: ids.room, roomEpoch: ids.epoch, streamId: ids.stream }, writable: true }
  const listeners = new Set<() => void>()
  return { read: () => value, subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    set(next: MuralAuthority | null) { value = next; Array.from(listeners).forEach(f => f()) }, count: () => listeners.size }
}
export function memoryScene() {
  let textures = 0, objects = 0, uploads = 0, rgba: Uint8ClampedArray | null = null
  const listeners = new Set<() => void>()
  const port: MuralScenePort = { texture() { textures++; return { replace(next) { uploads++; rgba = new Uint8ClampedArray(next) }, dispose() { textures--; rgba = null } } },
    object() { objects++; return { dispose() { objects-- } } }, onFrame(listener) { listeners.add(listener); return () => { listeners.delete(listener) } } }
  return { port, frame() { Array.from(listeners).forEach(f => f()) }, read: () => ({ textures, objects, uploads, listeners: listeners.size, rgba }) }
}
export function fixture(options: { reads?: { snapshot: () => Promise<unknown>; delta: () => Promise<unknown> }; commands?: MuralCommandPort; actor?: string; tools?: WallToolPort } = {}) {
  const session = createMemorySession(), client = new QueryClient(), clock = testClock(), authority = authorityPort(), scene = memoryScene(), sink = createMuralTextureSink(scene.port)
  session.replace(0, { userId: options.actor ?? ids.user, eventId: ids.event, accessToken: 'fixture-only-memory-token' })
  const privacy = createQueryPrivacy({ client, session }), requests: { kind: string; signal: AbortSignal; command?: PixelCommand }[] = []
  let server = snapshot(), key = options.actor === ids.other ? 200 : 100
  const sync = createMuralSync({ privacy, authority, clock, sink, newKey: () => `00000000-0000-4000-8000-${String(++key).padStart(12, '0')}`,
    ...(options.tools ? { tools: options.tools } : {}),
    reads: { snapshot: async r => { requests.push({ kind: 'snapshot', signal: r.signal }); return options.reads ? options.reads.snapshot() : structuredClone(server) },
      delta: async r => { requests.push({ kind: 'delta', signal: r.signal }); return options.reads ? options.reads.delta() : delta(r.afterRevision, []) } },
    commands: { place: async (c, s) => { requests.push({ kind: 'place', command: c, signal: s }); return options.commands ? options.commands.place(c, s) : { kind: 'unknown' } },
      check: async (c, s) => { requests.push({ kind: 'check', command: c, signal: s }); return options.commands ? options.commands.check(c, s) : { kind: 'unknown' } } } })
  return { session, privacy, clock, authority, scene, sink, sync, requests, server(value: MuralSnapshot) { server = value },
    async start() { sync.start(); await drain(); scene.frame() }, dispose() { sync.dispose(); privacy.dispose(); client.clear() } }
}

/** In-process deterministic C04 HTTP server for tests only; no production import or listener/socket. */
export function serverFixture() {
  let state = snapshot(), mode: 'ack' | 'lost' | 'pending' | 'rejected' = 'ack'
  let resultAvailable = false, receiptExpiresAt = '2099-01-01T00:00:00Z'
  const operations: PublicPixel[] = [], commands = new Map<string, { command: PixelCommand; own: PixelOperation }>(), requests: Request[] = []
  const paint = (p: PublicPixel) => { const pixels = [...state.pixels]; pixels[p.y * 128 + p.x] = p.color_index; state = { ...state, revision: p.revision, pixels }; operations.push(p) }
  const receipt = (c: PixelCommand) => {
    const stored = commands.get(c.key)
    return { receipt_id: ids.snapshot, event_id: ids.event, actor_id: c.scope.userId, operation_id: c.operation, idempotency_key: c.key,
      state: mode === 'pending' ? 'PENDING' : 'SUCCEEDED', resource_id: mode === 'pending' ? null : ids.event,
      resource_revision: mode === 'pending' ? null : stored?.own.revision ?? null, failure_reason: null, result_status: mode === 'pending' ? null : 200,
      created_at: time, expires_at: receiptExpiresAt }
  }
  const error = (reason: string, status: number) => Response.json({ code: `MCR_${status}`, message: 'Unavailable.', details: { reason } }, { status })
  const fetch: typeof globalThis.fetch = async input => {
    const request = input as Request; requests.push(request.clone()); const url = new URL(request.url)
    const user = request.headers.get('Authorization')?.slice(7) ?? ids.user
    if (url.pathname.endsWith('/mural/snapshot')) return Response.json(state)
    if (url.pathname.endsWith('/mural/operations')) {
      const after = Number(url.searchParams.get('after_revision')), found = operations.filter(p => p.revision > after)
      return Response.json(after > state.revision || found.length > 512 || found.length !== state.revision - after ? delta(after, [], true, state.revision) : delta(after, found))
    }
    if (url.pathname.endsWith('/result')) {
      const key = url.pathname.split('/').at(-2)!, stored = commands.get(key)
      return resultAvailable && mode !== 'pending' && stored && stored.command.scope.userId === user
        ? Response.json({ receipt_id: receipt(stored.command).receipt_id, idempotency_key: key, result: stored.own }) : error('RESOURCE_UNAVAILABLE', 404)
    }
    if (url.pathname.endsWith('/commands')) {
      const stored = commands.get(url.searchParams.get('idempotency_key') ?? '')
      return stored && stored.command.scope.userId === user ? Response.json(receipt(stored.command)) : error('RESOURCE_UNAVAILABLE', 404)
    }
    if (url.pathname.endsWith('/mural/pixels')) {
      if (mode === 'rejected') return error('INVALID_INPUT', 400)
      const body = await request.json() as PixelCommand['body'], key = request.headers.get('Idempotency-Key')!
      if (body.expected_revision !== state.revision) return error('VERSION_CONFLICT', 409)
      const command: PixelCommand = { key, operation: 'placeConcertPixel', scope: { userId: user, eventId: ids.event, generation: 1, policyGeneration: 0, gate: 'open' }, body }
      const p = pixel(state.revision + 1, { x: body.x, y: body.y, color_index: body.color_index }), own = { ...p, next_allowed_at: new Date(timestamp + state.cooldown_ms).toISOString() }
      paint(p); commands.set(key, { command, own })
      if (mode === 'lost') throw new Error('Fixture lost response after durable commit')
      return mode === 'pending' ? Response.json(receipt(command), { status: 202 }) : Response.json(own)
    }
    return error('RESOURCE_UNAVAILABLE', 404)
  }
  return { fetch, requests, read: () => structuredClone(state), paint, delta, operations, mode(value: typeof mode) { mode = value },
    resultAvailable(value: boolean) { resultAvailable = value }, receiptExpiry(value: string) { receiptExpiresAt = value },
    restore(next: MuralSnapshot) { state = structuredClone(next); operations.length = 0 }, recover(command: PixelCommand) { return commands.get(command.key)?.own ?? null } }
}
export function httpClientFixture(server: ReturnType<typeof serverFixture>, actor: string = ids.user, authority: MuralAuthorityPort = authorityPort()) {
  const session = createMemorySession(), client = new QueryClient()
  session.replace(0, { userId: actor, eventId: ids.event, accessToken: actor })
  const privacy = createQueryPrivacy({ client, session }), http = createHttpClient({ session, fetch: server.fetch })
  return { session, privacy, http, authority,
    adapter: (withOriginal: boolean) => { server.resultAvailable(withOriginal); return createMuralHttp({ http, privacy, authority, now: () => timestamp }) },
    dispose() { privacy.dispose(); client.clear() } }
}
