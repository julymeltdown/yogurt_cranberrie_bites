import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createHttpClient } from '../../../../shared/api/http'
import { createMuralHttp } from '../http'
import { authorityPort, httpClientFixture, ids, serverFixture, snapshot, timestamp } from './fixture'
import type { PixelCommand } from '../../editor/contract'
const cleanups: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { cleanups.splice(0).reverse().forEach(f => f()); vi.unstubAllGlobals() })
function setup() { const server = serverFixture(), authority = authorityPort(), f = httpClientFixture(server, ids.user, authority); cleanups.push(f.dispose)
  const command: PixelCommand = { scope: f.privacy.capture(), operation: 'placeConcertPixel', key: ids.next,
    body: { expected_revision: 10, x: 1, y: 2, color_index: 2 } }
  const lease = { scope: command.scope, binding: authority.read()!.binding, generation: 1 }, signal = new AbortController().signal
  return { ...f, server, authority, command, lease, signal }
}
describe('FE33 canonical HTTP adapter and no-replay receipt boundary', () => {
  it('reads only frozen C04 snapshot/delta operations with exact event/baseline binding', async () => {
    const f = setup(), adapter = f.adapter(false)
    const snapshot = await adapter.reads.snapshot({ lease: f.lease, signal: f.signal }) as { revision: number }
    expect(snapshot.revision).toBe(10)
    await adapter.reads.delta({ lease: f.lease, signal: f.signal, afterRevision: 10 })
    expect(f.server.requests.map(r => r.method)).toEqual(['GET', 'GET'])
    expect(f.server.requests[1]!.url).toContain('/mural/operations?after_revision=10')
  })
  it('places exactly once with original key/body, verifies ack, and returns pending metadata as pending', async () => {
    const f = setup(), adapter = f.adapter(false), result = await adapter.commands.place(f.command, f.signal)
    expect(result.kind).toBe('acknowledged'); expect(f.server.requests).toHaveLength(1)
    expect(f.server.requests[0]!.headers.get('Idempotency-Key')).toBe(f.command.key)
    expect(await f.server.requests[0]!.clone().json()).toEqual(f.command.body)
    const g = setup(); g.server.mode('pending'); expect((await g.adapter(false).commands.place(g.command, g.signal)).kind).toBe('pending')
  })
  it('lost response followed by SUCCEEDED metadata stays unknown without original result; POST is never replayed', async () => {
    const f = setup(), adapter = f.adapter(false); f.server.mode('lost')
    expect((await adapter.commands.place(f.command, f.signal)).kind).toBe('unknown'); f.server.mode('ack')
    expect((await adapter.commands.check(f.command, f.signal)).kind).toBe('unknown')
    expect(f.server.requests.map(r => r.method)).toEqual(['POST', 'GET', 'GET'])
  })
  it('C04 0.1.2 own-result HTTP read resolves exact aggregate receipt, without mutation replay', async () => {
    const f = setup(), adapter = f.adapter(true); f.server.mode('lost'); await adapter.commands.place(f.command, f.signal); f.server.mode('ack')
    expect((await adapter.commands.check(f.command, f.signal)).kind).toBe('acknowledged')
    expect(f.server.requests.map(r => r.method)).toEqual(['POST', 'GET', 'GET'])
    expect(f.server.requests[2]!.url).toBe(`https://mcr.example/api/concert/events/${ids.event}/mural/commands/${f.command.key}/result`)
    expect(f.server.requests[2]!.headers.get('Idempotency-Key')).toBeNull()
  })
  it.each([{ actor_id: ids.other }, { event_id: ids.other }, { operation_id: 'anotherOperation' }, { idempotency_key: ids.other },
    { resource_revision: 12 }, { resource_id: ids.other }, { result_status: 202 }, { expires_at: '2026-11-03T00:00:00Z' }, { author_id: ids.other }])('forged or expired receipt %j cannot confirm', async patch => {
    const f = setup(); f.server.mode('lost'); await f.adapter(false).commands.place(f.command, f.signal); f.server.mode('ack')
    const http = createHttpClient({ session: f.session, fetch: async input => {
      const response = await f.server.fetch(input)
      return (input as Request).url.endsWith('/commands') || (input as Request).url.includes('/commands?')
        ? Response.json({ ...await response.json() as object, ...patch }) : response
    } })
    f.server.resultAvailable(true)
    const adapter = createMuralHttp({ http, privacy: f.privacy, authority: f.authority, now: () => timestamp })
    expect((await adapter.commands.check(f.command, f.signal)).kind).toBe('unknown')
  })
  it('public delta with private cooldown/author fields is rejected', async () => {
    const f = setup(); await f.adapter(false).commands.place(f.command, f.signal)
    const http = createHttpClient({ session: f.session, fetch: async () => Response.json({ event_id: ids.event, from_revision: 10, to_revision: 11, reset_required: false,
      operations: [{ ...f.server.operations[0], next_allowed_at: '2026-11-04T00:00:00Z' }] }) })
    const adapter = createMuralHttp({ http, privacy: f.privacy, authority: f.authority })
    await expect(adapter.reads.delta({ lease: f.lease, signal: f.signal, afterRevision: 10 })).rejects.toThrow()
  })
  it('authority change discards a late acknowledged response', async () => {
    const f = setup(); let release!: () => void
    const delay = new Promise<void>(resolve => { release = resolve })
    const http = createHttpClient({ session: f.session, fetch: async input => { const response = await f.server.fetch(input); await delay; return response } })
    const adapter = createMuralHttp({ http, privacy: f.privacy, authority: f.authority }), pending = adapter.commands.place(f.command, f.signal)
    await vi.waitFor(() => expect(f.server.requests).toHaveLength(1)); f.authority.set({ ...f.authority.read()!, binding: { ...f.lease.binding, roomEpoch: ids.next } }); release()
    expect((await pending).kind).toBe('unknown')
  })
  it('missing authority or writable capability sends no HTTP command', async () => {
    const f = setup(); f.authority.set({ ...f.authority.read()!, writable: false })
    expect((await f.adapter(false).commands.place(f.command, f.signal)).kind).toBe('unknown'); expect(f.server.requests).toHaveLength(0)
    f.authority.set(null); await expect(f.adapter(false).reads.snapshot({ lease: f.lease, signal: f.signal })).rejects.toThrow()
  })
  it.each(['receipt', 'key', 'event', 'aggregate-operation', 'revision', 'x', 'y', 'color', 'cooldown', 'author', 'extra', 'raw'] as const)('rejects mismatched closed own-result envelope: %s', async kind => {
    const f = setup(); f.server.mode('lost'); await f.adapter(true).commands.place(f.command, f.signal); f.server.mode('ack')
    const http = createHttpClient({ session: f.session, fetch: async input => {
      const response = await f.server.fetch(input)
      if (!(input as Request).url.endsWith('/result')) return response
      const value = await response.json() as { receipt_id: string; idempotency_key: string; result: Record<string, unknown> } & Record<string, unknown>
      if (kind === 'receipt') value.receipt_id = ids.other
      if (kind === 'key') value.idempotency_key = ids.other
      if (kind === 'event') value.result.event_id = ids.other
      if (kind === 'aggregate-operation') value.result.operation_id = ids.event
      if (kind === 'revision') value.result.revision = 12
      if (kind === 'x') value.result.x = 9
      if (kind === 'y') value.result.y = 9
      if (kind === 'color') value.result.color_index = 9
      if (kind === 'cooldown') value.result.next_allowed_at = '2026-11-03T00:00:00Z'
      if (kind === 'author') value.result.author_id = ids.user
      if (kind === 'extra') value.secret = 'unapproved'
      return Response.json(kind === 'raw' ? value.result : value)
    } })
    const adapter = createMuralHttp({ http, privacy: f.privacy, authority: f.authority, now: () => timestamp })
    expect((await adapter.commands.check(f.command, f.signal)).kind).toBe('unknown')
    expect(f.server.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  })
  it.each([401, 403, 404, 429, 500, 503, 504])('own-result HTTP %s remains unknown and never replays', async status => {
    const f = setup(); f.server.mode('lost'); await f.adapter(true).commands.place(f.command, f.signal); f.server.mode('ack')
    const http = createHttpClient({ session: f.session, fetch: async input => (input as Request).url.endsWith('/result')
      ? Response.json({ code: 'UNAVAILABLE', message: 'Unavailable.', details: { reason: status >= 500 ? 'DEPENDENCY_UNAVAILABLE' : 'RESOURCE_UNAVAILABLE' } }, { status }) : f.server.fetch(input) })
    expect((await createMuralHttp({ http, privacy: f.privacy, authority: f.authority, now: () => timestamp }).commands.check(f.command, f.signal)).kind).toBe('unknown')
    expect(f.server.requests.filter(r => r.method === 'POST')).toHaveLength(1)
  })
  it('pending status never calls the own-result route', async () => {
    const f = setup(); f.server.mode('pending'); await f.adapter(true).commands.place(f.command, f.signal)
    expect((await f.adapter(true).commands.check(f.command, f.signal)).kind).toBe('pending')
    expect(f.server.requests.some(r => r.url.endsWith('/result'))).toBe(false)
  })
  it('original server cooldown is recovered without deriving it from snapshot or public delta', async () => {
    const f = setup(); f.server.restore(snapshot({ cooldown_ms: 30000 })); f.server.mode('lost'); await f.adapter(true).commands.place(f.command, f.signal); f.server.mode('ack')
    expect(f.server.read().next_allowed_at).toBeNull()
    const result = await f.adapter(true).commands.check(f.command, f.signal)
    expect(result.kind).toBe('acknowledged')
    if (result.kind === 'acknowledged') expect((result.pixel as { next_allowed_at: string }).next_allowed_at).toBe('2026-11-04T00:00:30.000Z')
  })
})
