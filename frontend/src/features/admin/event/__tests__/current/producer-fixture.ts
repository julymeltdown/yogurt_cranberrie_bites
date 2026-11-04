// Deterministic FE38 failure injection around the existing canonical-Auth fixture.
// Fictional role, clock, venue and transport. Never a production authority adapter.
import { createOperationsController } from '../../model/controller'
import type { PlaceWritePort } from '../../model/ports'
import { deferred, fixture, now } from '../fixtures/data'

export function placeProducerFixture(initiallyUnavailable = false) {
  const owner = fixture()
  owner.controller.dispose() // Retain the one canonical Auth; no redundant feature owner.
  let unavailable = initiallyUnavailable, hold = false, aborted = false, writeCalls = 0
  const placeListeners = new Set<() => void>(), authorityListeners = new Set<() => void>()
  let pending: { resolve(value: Awaited<ReturnType<PlaceWritePort['write']>>): void; value: Awaited<ReturnType<PlaceWritePort['write']>> } | null = null
  const places: PlaceWritePort = {
    read() { if (unavailable) throw new Error('PRIVATE place producer diagnostic'); return owner.release() },
    subscribe(listener) { placeListeners.add(listener); return () => { placeListeners.delete(listener) } },
    async write(input) {
      writeCalls++
      input.signal.addEventListener('abort', () => { aborted = true }, { once: true })
      if (hold) {
        const result = deferred<Awaited<ReturnType<PlaceWritePort['write']>>>()
        pending = { resolve: result.resolve, value: { scope: input.scope, status: 200, data: { place: input.body.place, revision: input.body.expected_revision + 1 } } }
        return result.promise
      }
      // Same registered adapter shape as ../fixtures/data.ts; only synthetic HTTP.
      const response = await owner.auth.request({ access: 'protected', errorFamily: 'domain', domainOperation: 'setConcertPlace', method: 'PUT',
        path: `/api/concert/events/${input.scope.eventId}/admin/places/${input.placeId}`,
        idempotencyKey: input.key, body: { json: input.body }, signal: input.signal,
        decode: (data, context) => ({ data, status: context.status as 200 | 202 }) })
      return { ...response.data, scope: input.scope }
    },
  }
  const controller = createOperationsController({ auth: owner.auth, places,
    authority: { read: owner.lease, subscribe(listener) { authorityListeners.add(listener); return () => { authorityListeners.delete(listener) } } },
    clock: { read: () => ({ scope: owner.scope, nowMs: now }), subscribe: () => () => {} } })
  return { controller, owner,
    state: () => ({ view: controller.read(), writeCalls, aborted, placeSubscriptions: placeListeners.size, authoritySubscriptions: authorityListeners.size,
      requests: owner.requests.map(r => ({ method: r.method, path: new URL(r.url).pathname })) }),
    fail(notify = true) { unavailable = true; if (notify) for (const listener of placeListeners) listener() },
    hold() { hold = true },
    settle() { const current = pending; pending = null; current?.resolve(current.value) },
    revoke() { owner.setLease(null); for (const listener of authorityListeners) listener() },
    dispose() { controller.dispose(); owner.dispose() },
  }
}
