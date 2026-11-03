import { vi } from 'vitest'
import movement from '../../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { createSessionController } from '../../../features/auth/model/session'
import type { ReplicationView } from '../../../world/network/replication'
import type { Snapshot } from '../../../world/network/WorldTransport'
import { MockWorldClock } from '../../../world/network/mock'
import { bootstrapFor } from '../../__tests__/fixtures/ports'
import type { WorldAccessBinding } from '../../ports'
import { createConnectionObserver, type ServiceDecision, type RecoveryAction } from '../observer'
export function port<T>(initial: T) {
  let value = initial
  const listeners = new Set<() => void>()
  return { read: () => value, listeners, subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn) } },
    set(next: T) { value = next; for (const fn of Array.from(listeners)) fn() } }
}
export function harness() {
  const snapshot = structuredClone(movement.messages[1]!.message) as Snapshot
  const auth = createSessionController({ eventId: snapshot.eventId, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_k, w) => w(), dispose() {} } })
  auth.session.replace(0, { userId: snapshot.selfUserId, eventId: snapshot.eventId, accessToken: 'test-only-memory' })
  const scope = { ...auth.session.read(), userId: snapshot.selfUserId, eventId: snapshot.eventId }
  const { players: _players, ...baseline } = snapshot
  const view = port<ReplicationView | null>({ scope, phase: 'live', visible: true, baseline, entities: [], pendingInputs: 0, ackInputSeq: 0, spatialPrediction: 'unavailable', problem: null })
  const lifetime = new AbortController(), release = { scope, privacyScope: auth.privacy.capture(), bootstrap: bootstrapFor(scope), signal: lifetime.signal }
  const binding: WorldAccessBinding = { release, replication: { read: view.read, subscribe: view.subscribe, switchRoom: async () => false } }
  const world = port<WorldAccessBinding | null>(binding), connectivity = port<'online' | 'offline' | 'unknown'>('online')
  const service = port<ServiceDecision | null>(null), reconnect = vi.fn(async () => false)
  const recovery = port<RecoveryAction | null>({ release, reconnect }), clock = new MockWorldClock()
  const controller = createConnectionObserver({ auth, world, connectivity, service, recovery, scheduler: clock, pendingMs: 1000 })
  controller.start()
  const patch = (next: Partial<ReplicationView>) => view.set({ ...view.read()!, ...next })
  return { auth, view, world, binding, lifetime, connectivity, service, reconnect, recovery, clock, controller, patch,
    dispose() { controller.dispose(); auth.dispose() } }
}
