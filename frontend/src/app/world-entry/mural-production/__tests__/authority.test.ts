import { afterEach, describe, expect, it, vi } from 'vitest'
import placement from '../placement.json'
import { createMuralAdmissionSource } from '../authority'
import { validateMuralProductionConfiguration } from '../configuration'
import { fixture, ids, settle } from '../../../../world/mural-entry-lifecycle/__tests__/fixture'
import { deferred } from '../../../../features/auth/__tests__/fixtures/data'
import { toolContext } from '../../../../features/mural/editor/__tests__/wall-tools-b/fixture'
import type { WallToolContext } from '../../../../features/mural/editor/tools'
import type { PreparedWorldRelease } from '../../../../world/composition/release'
import type { MuralQualificationContext } from '../../../../world/mural-entry-lifecycle/source'
import type { PrivacyScope } from '../../../../shared/query/privacy'

const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()) })

function harness(load?: (signal: AbortSignal) => Promise<unknown>, initial = toolContext(), monotonicNow?: () => number) {
  const f = fixture(false), lifetime = new AbortController(), config = validateMuralProductionConfiguration(placement)!
  const base = f.view()
  f.setView({ ...base, baseline: { ...base.baseline!, mapId: config.map.id, mapRevision: config.map.revision } })
  // Deliberate unit-test projection: no production manifest validation or native claim.
  const release = { map: { map_id: config.map.id, map_revision: config.map.revision, content_hash: config.map.contentHash,
    geometry_hash: config.map.geometryHash, geometry: { width_cells: 48, height_cells: 81, tile_width_px: 16, tile_height_px: 16 } },
    rooms: { items: [{ room_id: ids.room }] } } as unknown as PreparedWorldRelease
  const context: MuralQualificationContext = { scope: f.scope, signal: lifetime.signal, scene: f.qualify().input.scene, world: f.world, release }
  let response = initial
  const calls: AbortSignal[] = [], loadTools = vi.fn(async (_scope: PrivacyScope, signal: AbortSignal) => {
    calls.push(signal)
    return load ? load(signal) : structuredClone(response)
  })
  const source = createMuralAdmissionSource({ privacy: f.auth.privacy, context, configuration: config, loadTools,
    clock: monotonicNow ? { ...f.clock, now: monotonicNow } : f.clock })
  stops.push(() => f.dispose(), source.dispose)
  return { ...f, context, config, source, calls, loadTools, lifetime, response(next: WallToolContext) { response = next } }
}

describe('server-qualified production mural admission', () => {
  it('requires an actual tools read before publishing a scene qualification', async () => {
    const held = deferred<unknown>(), h = harness(() => held.promise)
    expect(h.source.read()).toBeNull()
    await settle(); expect(h.loadTools).toHaveBeenCalledOnce()
    held.resolve(toolContext()); await settle()
    const output = h.source.read()!
    expect(output.scope).toEqual(h.scope)
    expect(output.binding).toEqual(h.binding)
    expect(output.anchor).toEqual(h.config.anchor)
    expect(output.scene).toBe(h.context.scene)
    expect(output.authority.read()).toEqual({ binding: h.binding, writable: true })
  })
  it('does not cancel pending authorization on ordinary World updates', async () => {
    const held = deferred<unknown>(), h = harness(() => held.promise)
    await settle()
    for (let i = 0; i < 5; i++) h.setView({ ...h.view(), ackInputSeq: i })
    expect(h.calls).toHaveLength(1); expect(h.calls[0]!.aborted).toBe(false)
    held.resolve(toolContext()); await settle()
    expect(h.source.read()?.authority.read()?.writable).toBe(true)
  })
  it.each(['denied', 'timeout'] as const)('keeps the retry interval across ordinary World ticks after %s', async kind => {
    const never = deferred<unknown>()
    const h = harness(async () => { if (kind === 'denied') throw new Error('denied'); return never.promise })
    await settle()
    if (kind === 'timeout') await h.clock.advance(5000)
    for (let i = 0; i < 50; i++) { h.setView({ ...h.view(), ackInputSeq: i }); await settle() }
    expect(h.source.read()).toBeNull(); expect(h.calls).toHaveLength(1)
    await h.clock.advance(1999); expect(h.calls).toHaveLength(1)
    await h.clock.advance(1); expect(h.calls).toHaveLength(2)
    h.setView({ ...h.view(), baseline: { ...h.view().baseline!, roomEpoch: ids.next } }); await settle()
    expect(h.calls).toHaveLength(3) // A genuinely new binding qualifies immediately.
  })
  it('expires from request start even when the response arrives just before the deadline', async () => {
    const first = deferred<unknown>(), later = deferred<unknown>()
    let count = 0
    const h = harness(() => ++count === 1 ? first.promise : later.promise)
    await settle(); await h.clock.advance(4900)
    first.resolve(toolContext({ serverTime: '2099-01-01T00:00:00Z' })); await settle()
    const old = h.source.read()!
    expect(old.authority.read()?.writable).toBe(true)
    await h.clock.advance(100)
    expect(h.source.read()).toBeNull(); expect(old.authority.read()).toBeNull()
    expect(h.calls).toHaveLength(2)
  })
  it('rejects a response at the deadline and ignores an aborted late response', async () => {
    const held = deferred<unknown>(), h = harness(() => held.promise)
    await settle(); await h.clock.advance(5000)
    expect(h.calls[0]!.aborted).toBe(true)
    held.resolve(toolContext()); await settle()
    expect(h.source.read()).toBeNull()
  })
  it('restarts admission when a dispatch observes expiry before the delayed timer runs', async () => {
    let monotonic = 0
    const h = harness(undefined, toolContext(), () => monotonic)
    await settle(); const original = h.source.read()!
    monotonic = 5000
    expect(h.source.read()).toBeNull(); expect(original.authority.read()).toBeNull()
    await settle()
    expect(h.source.read()).not.toBeNull(); expect(h.source.read()).not.toBe(original)
    expect(h.calls).toHaveLength(2)
  })
  it('refreshes while visible and retains one qualification for the same binding', async () => {
    const h = harness(); await settle()
    const original = h.source.read()!
    h.response(toolContext({ editable: false }))
    await h.clock.advance(2000)
    expect(h.calls).toHaveLength(2)
    expect(h.source.read()).toBe(original)
    expect(original.authority.read()?.writable).toBe(false)
    h.response(toolContext()); await h.clock.advance(2000)
    expect(h.source.read()).toBe(original)
    expect(original.authority.read()?.writable).toBe(true)
  })
  it('preserves an authorized read-only view when no write capability is available', async () => {
    const h = harness(undefined, toolContext({ capabilities: { pencil: false, erase: false, fill: false, undo: false, backgroundIndex: 0 } }))
    await settle()
    expect(h.source.read()).not.toBeNull()
    expect(h.source.read()!.authority.read()?.writable).toBe(false)
  })
  it('withdraws editing during command refresh without cancelling the completed read lease', async () => {
    let pending: ReturnType<typeof deferred<unknown>> | null = null
    const h = harness(async () => pending ? pending.promise : toolContext())
    await settle(); const original = h.source.read()!, signal = original.signal
    pending = deferred<unknown>(); h.source.invalidate()
    expect(h.source.read()).toBe(original)
    expect(original.authority.read()?.writable).toBe(false)
    expect(signal.aborted).toBe(false)
    await settle(); pending.resolve(toolContext()); await settle()
    expect(h.source.read()).toBe(original)
    expect(original.authority.read()?.writable).toBe(true)
    expect(h.calls).toHaveLength(2)
  })
  it('withdraws synchronously on visibility loss and requires a fresh response on return', async () => {
    const h = harness(); await settle(); const original = h.source.read()!
    h.setView({ ...h.view(), visible: false })
    expect(h.source.read()).toBeNull(); expect(original.authority.read()).toBeNull()
    h.setView({ ...h.view(), visible: true })
    expect(h.source.read()).toBeNull(); await settle()
    expect(h.source.read()).not.toBe(original)
    expect(original.authority.read()).toBeNull()
  })
  it('withdraws an old epoch before qualifying a new one', async () => {
    const h = harness(); await settle(); const original = h.source.read()!
    h.setView({ ...h.view(), baseline: { ...h.view().baseline!, roomEpoch: ids.next } })
    expect(h.source.read()).toBeNull(); expect(original.authority.read()).toBeNull()
    await settle()
    expect(h.source.read()!.binding.roomEpoch).toBe(ids.next)
  })
  it.each(['actor', 'event', 'schema'] as const)('rejects an unqualified %s tools response', async kind => {
    const input = toolContext()
    const h = harness(async () => kind === 'actor' ? { ...input, actorId: ids.other }
      : kind === 'event' ? { ...input, eventId: ids.other } : { ...input, unexpected: true })
    await settle(); expect(h.source.read()).toBeNull()
  })
  it('withdraws immediately after a failed refresh instead of reusing previous eligibility', async () => {
    let fail = false
    const h = harness(async () => { if (fail) throw new Error('denied'); return toolContext() })
    await settle(); const original = h.source.read()!
    fail = true; await h.clock.advance(2000)
    expect(h.source.read()).toBeNull(); expect(original.authority.read()).toBeNull()
  })
  it.each(['abort', 'logout', 'policy', 'event'] as const)('disposes requests and timers on %s', async kind => {
    const held = deferred<unknown>(), h = harness(() => held.promise)
    await settle()
    if (kind === 'abort') h.lifetime.abort()
    if (kind === 'logout') void h.auth.logout()
    if (kind === 'policy') h.auth.privacy.changePolicy(h.scope, 'block')
    if (kind === 'event') h.auth.session.replace(h.scope.generation, { userId: ids.user, eventId: ids.other, accessToken: 'fixture-only' })
    expect(h.calls[0]!.aborted).toBe(true); expect(h.clock.count()).toBe(0)
    held.resolve(toolContext()); await settle(); expect(h.source.read()).toBeNull()
  })
  it('never reads tools for mismatched placement pins', async () => {
    const h = harness(); await settle()
    h.source.dispose()
    const loadTools = vi.fn(async () => toolContext())
    const source = createMuralAdmissionSource({ privacy: h.auth.privacy, context: h.context,
      configuration: { ...h.config, map: { ...h.config.map, contentHash: 'sha256:' + '0'.repeat(64) } }, loadTools, clock: h.clock })
    stops.push(source.dispose); await settle()
    expect(source.read()).toBeNull(); expect(loadTools).not.toHaveBeenCalled()
  })
})
