import { afterEach, expect, it, vi } from 'vitest'
import { QueryClient } from '@tanstack/react-query'
import { createMemorySession } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import type { WorldEvent, ServerMessage } from '../../../network/WorldTransport'
import { decode } from '../../../network/transport/contract'
import trace from '../../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { attachWorldSocialPolicy } from '../../policy'

const snapshot = decode(JSON.stringify(trace.messages[1]!.message), 'server')
if (snapshot.type !== 'snapshot') throw new Error('Snapshot fixture missing')
const initial = snapshot
const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()) })
function fixture() {
  const session = createMemorySession(), client = new QueryClient()
  session.replace(0, { userId: initial.selfUserId, eventId: initial.eventId, accessToken: 'memory-only' })
  const privacy = createQueryPrivacy({ session, client }), scope = privacy.capture(), lifetime = new AbortController()
  const listeners = new Set<(event: WorldEvent) => void>()
  const subscribe = vi.fn((fn: (event: WorldEvent) => void) => { listeners.add(fn); return () => { listeners.delete(fn) } })
  const port = attachWorldSocialPolicy({ auth: { session, privacy }, scope, signal: lifetime.signal, transport: { subscribe } })
  const emit = (frame: ServerMessage) => { for (const fn of Array.from(listeners)) fn({ type: 'frame', frame }) }
  const baseline = { protocolVersion: '1.0.0', type: 'policy.baseline', streamId: initial.streamId,
    eventId: initial.eventId, scope: 'SOCIAL', policyRevision: 1 } as const
  stops.push(() => { port.dispose(); privacy.dispose(); client.clear() })
  return { session, privacy, scope, lifetime, port, emit, baseline, listeners, subscribe }
}
it('requires a real matching snapshot and separate SOCIAL baseline, with one borrowed subscription', () => {
  const f = fixture(); f.emit(f.baseline); expect(f.port.read()).toBeNull()
  f.emit(initial); expect(f.port.read()).toBeNull()
  f.emit(f.baseline)
  expect(f.port.read()).toMatchObject({ scope: f.scope, streamId: initial.streamId, roomEpoch: initial.roomEpoch, policyRevision: 1 })
  expect(f.subscribe).toHaveBeenCalledOnce()
})
it.each(['eventId', 'streamId'] as const)('rejects foreign baseline %s', key => {
  const f = fixture(); f.emit(initial); f.emit({ ...f.baseline, [key]: '00000000-0000-4000-8000-000000000199' })
  expect(f.port.read()).toBeNull()
})
it('withdraws before publishing a replacement snapshot and rejects backwards policy revisions', () => {
  const f = fixture(); f.emit(initial); f.emit({ ...f.baseline, policyRevision: 3 })
  const changes: unknown[] = []; f.port.subscribe(() => changes.push(f.port.read()))
  f.emit(initial); expect(changes).toEqual([null])
  f.emit({ ...f.baseline, policyRevision: 2 }); expect(f.port.read()).toBeNull()
  f.emit({ ...f.baseline, policyRevision: 3 }); expect(f.port.read()?.policyRevision).toBe(3)
})
it.each(['SOURCE_CHANGED', 'CONSENT_CHANGED', 'BLOCK_CHANGED', 'EXPIRED', 'CANONICAL_POLICY_CHANGED'] as const)(
  'withdraws and clears the same canonical privacy lifetime for %s', reason => {
    const f = fixture(); f.emit(initial); f.emit(f.baseline)
    const order: string[] = []
    f.port.subscribe(() => { expect(f.port.read()).toBeNull(); order.push('withdrawn') })
    f.privacy.registerCleanup(() => { expect(f.port.read()).toBeNull(); order.push('canonical-cleanup') })
    f.emit({ ...f.baseline, type: 'policy.invalidate', policyRevision: 2, reason })
    expect(f.privacy.isCurrent(f.scope)).toBe(false)
    expect(order[0]).toBe('withdrawn'); expect(order).toContain('canonical-cleanup')
    expect(f.listeners.size).toBe(0)
  })
it('withdraws on transport loss and requires a fresh snapshot before another baseline', () => {
  const f = fixture(); f.emit(initial); f.emit(f.baseline)
  for (const fn of f.listeners) fn({ type: 'state', state: 'closed' })
  expect(f.port.read()).toBeNull(); f.emit(f.baseline); expect(f.port.read()).toBeNull()
  f.emit(initial); f.emit(f.baseline); expect(f.port.read()).not.toBeNull()
})
it('isolates throwing observers and never restores a disposed privacy owner', () => {
  const f = fixture(); f.port.subscribe(() => { throw new Error('observer') }); const other = vi.fn(); f.port.subscribe(other)
  f.emit(initial); f.emit(f.baseline); expect(other).toHaveBeenCalledOnce()
  f.lifetime.abort(); expect(f.port.read()).toBeNull(); expect(f.listeners.size).toBe(0)
  f.emit(initial); f.emit(f.baseline); expect(f.port.read()).toBeNull()
})
