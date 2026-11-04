import { afterEach, expect, it, vi } from 'vitest'
import { fixture, deferred, id } from '../../../social-composition/__tests__/fixtures'
import { productionAvailability } from '../../../in03-entry/availability'
import { createSocialAvailabilityLifetime } from '../../availability-lifetime'
import type { SocialContext } from '../../../social-composition/ports'

const cleanups: (() => void)[] = []
afterEach(() => { cleanups.splice(0).reverse().forEach(stop => stop()) })
function setup() {
  const f = fixture(), context: SocialContext = { auth: f.auth, bootstrap: f.bootstrap, scope: f.scope }
  cleanups.push(f.dispose)
  return { ...f, context }
}

it('constructs without factory/read/subscribe/HTTP and prepares only in the started lifetime', async () => {
  const f = setup(), held = deferred<void>(), dispose = vi.fn(), prepare = vi.fn((_signal: AbortSignal) => held.promise)
  const factory = vi.fn(() => ({ ...f.availability, prepare, dispose }))
  const port = createSocialAvailabilityLifetime(f.context, factory); cleanups.push(port.dispose)
  expect(factory).not.toHaveBeenCalled(); expect(port.read()).toBeNull(); expect(f.listenerCount()).toBe(0)
  port.start(); port.start(); expect(factory).toHaveBeenCalledTimes(1); expect(port.read()).toBeNull()
  await Promise.resolve(); expect(prepare).toHaveBeenCalledTimes(1)
  held.resolve(); await vi.waitFor(() => expect(port.read()?.services.feed).toBe(true))
  expect(f.requests).toHaveLength(0)
  port.dispose(); expect(port.read()).toBeNull(); expect(dispose).toHaveBeenCalledTimes(1)
  expect(prepare.mock.calls[0]![0].aborted).toBe(true)
})

it('production remains unavailable without the real reader; borrowing never sends a duplicate Feed probe', () => {
  const f = setup()
  expect(productionAvailability(f.context).read()).toBeNull()
  const port = productionAvailability(f.context, f.availability)
  expect(port.read()?.services.feed).toBe(true); expect(port.read()).toBe(port.read())
  expect(f.listenerCount()).toBe(0); expect(f.requests).toHaveLength(0)
  f.setLease(null); expect(port.read()).toBeNull()
})

it.each(['userId', 'eventId', 'generation', 'policyGeneration', 'gate'] as const)('rejects a reader with foreign %s before service use', field => {
  const f = setup(), changes = { userId: id(999), eventId: id(998), generation: f.scope.generation + 1,
    policyGeneration: f.scope.policyGeneration + 1, gate: 'suspended' as const }
  f.setLease({ bootstrap: f.bootstrap, scope: { ...f.scope, [field]: changes[field] }, services: { feed: true } })
  expect(productionAvailability(f.context, f.availability).read()).toBeNull(); expect(f.requests).toHaveLength(0)
})

it('rejects foreign bootstrap/malformed service flags and honors C03 Feed/sharing denials', () => {
  const f = setup(), port = productionAvailability(f.context, f.availability)
  f.setLease({ bootstrap: { ...f.bootstrap }, scope: f.scope, services: { feed: true } }); expect(port.read()).toBeNull()
  f.setLease({ bootstrap: f.bootstrap, scope: f.scope, services: { feed: true, unknown: true } as { feed: boolean } }); expect(port.read()).toBeNull()
  const bootstrap = { ...f.bootstrap, capabilities: { ...f.bootstrap.capabilities,
    feed: { available: false, reason: 'FEATURE_DISABLED' } as const,
    social_sharing: { available: false, reason: 'FEATURE_DISABLED' } as const } }
  f.setLease({ bootstrap, scope: f.scope, services: { feed: true, requests: true } })
  expect(productionAvailability({ ...f.context, bootstrap }, f.availability).read()?.services).toEqual({ feed: false, requests: false })
})

it('withdraws synchronously on policy loss before producer disposal and ignores delayed preparation', async () => {
  const f = setup(), held = deferred<void>(); let signal: AbortSignal | undefined
  const dispose = vi.fn(() => { expect(port.read()).toBeNull() })
  const port = createSocialAvailabilityLifetime(f.context, () => ({ ...f.availability,
    prepare(input) { signal = input; return held.promise }, dispose }))
  port.start(); await Promise.resolve()
  f.auth.privacy.changePolicy(f.scope, 'consent-revoked')
  expect(port.read()).toBeNull(); expect(signal?.aborted).toBe(true); expect(dispose).toHaveBeenCalledTimes(1)
  held.resolve(); await Promise.resolve(); await Promise.resolve(); expect(port.read()).toBeNull(); expect(f.listenerCount()).toBe(0)
})

it('signout invalidates the old lease and a later start cannot revive disposed ownership', () => {
  const f = setup(), dispose = vi.fn(), factory = vi.fn(() => ({ ...f.availability, dispose }))
  const port = createSocialAvailabilityLifetime(f.context, factory); port.start()
  expect(port.read()?.services.feed).toBe(true)
  f.auth.session.replace(f.scope.generation, null); expect(port.read()).toBeNull(); expect(dispose).toHaveBeenCalledTimes(1)
  port.start(); expect(factory).toHaveBeenCalledTimes(1); expect(f.listenerCount()).toBe(0)
})

it('preparation rejection withdraws and releases the candidate without an unhandled promise', async () => {
  const f = setup(), dispose = vi.fn(), port = createSocialAvailabilityLifetime(f.context, () => ({ ...f.availability,
    prepare: () => Promise.reject(Error('unit-private-detail')), dispose }))
  port.start(); await vi.waitFor(() => expect(dispose).toHaveBeenCalledTimes(1))
  expect(port.read()).toBeNull(); expect(f.listenerCount()).toBe(0)
})

it('producer readiness changes notify current consumers and borrowed-reader cleanup stays with its owner', () => {
  const f = setup(), ownDispose = vi.fn(), reader = { ...f.availability, dispose: ownDispose }
  const port = createSocialAvailabilityLifetime(f.context, () => productionAvailability(f.context, reader))
  const notify = vi.fn(); port.subscribe(notify); port.start()
  f.setLease(null); expect(port.read()).toBeNull(); expect(notify).toHaveBeenCalled()
  port.dispose(); expect(f.listenerCount()).toBe(0); expect(ownDispose).not.toHaveBeenCalled()
})
