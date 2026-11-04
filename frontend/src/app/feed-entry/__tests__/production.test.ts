import { afterEach, describe, expect, it } from 'vitest'
import { fixture } from '../../social-composition/__tests__/fixtures'
import { createFeedSocialPorts } from '../production'
import type { SocialAvailability, SocialContext } from '../../social-composition/ports'

const owned: ReturnType<typeof fixture>[] = []
afterEach(() => { owned.splice(0).forEach(value => value.dispose()) })
function setup() {
  const value = fixture(); owned.push(value)
  const context: SocialContext = { auth: value.auth, scope: value.scope, bootstrap: value.bootstrap }
  return { ...value, context }
}
describe('Feed production source admission and lifetime', () => {
  it('does not infer a producer from a capable bootstrap', () => {
    const value = setup()
    expect(value.bootstrap.capabilities.feed.available).toBe(true)
    expect(createFeedSocialPorts().availability(value.context)).toBeNull()
    expect(createFeedSocialPorts({ readiness: () => { throw Error('Unavailable') } }).availability(value.context)).toBeNull()
  })
  it('borrows explicit Feed readiness without admitting unrelated services', () => {
    const value = setup(), leaf = createFeedSocialPorts({ readiness: () => value.availability }).availability(value.context)!
    const first = leaf.read()
    expect(first).toMatchObject({ services: { feed: true, people: false, intro: false, age: false, inbox: false, requests: false } })
    expect(leaf.read()).toBe(first)
    value.setLease({ scope: value.scope, bootstrap: value.bootstrap, services: { feed: false } })
    expect(leaf.read()?.services.feed).toBe(false)
  })
  it('requires the exact bootstrap and every privacy scope field', () => {
    const value = setup(), leaf = createFeedSocialPorts({ readiness: () => value.availability }).availability(value.context)!
    value.setLease({ scope: value.scope, bootstrap: { ...value.bootstrap }, services: { feed: true } })
    expect(leaf.read()).toBeNull()
    const changes = { userId: 'other-user', eventId: 'other-event', generation: value.scope.generation + 1,
      policyGeneration: value.scope.policyGeneration + 1, gate: 'suspended' } as const
    for (const [key, next] of Object.entries(changes)) {
      value.setLease({ scope: { ...value.scope, [key]: next }, bootstrap: value.bootstrap, services: { feed: true } })
      expect(leaf.read()).toBeNull()
    }
  })
  it('rejects malformed flags and a lost source, while respecting C03 denial', () => {
    const value = setup(), leaf = createFeedSocialPorts({ readiness: () => value.availability }).availability(value.context)!
    for (const services of [{ feed: 'yes' }, { feed: true, invented: true }]) {
      value.setLease({ scope: value.scope, bootstrap: value.bootstrap, services } as unknown as SocialAvailability)
      expect(leaf.read()).toBeNull()
    }
    value.setLease(null); expect(leaf.read()).toBeNull()
    const bootstrap = { ...value.bootstrap, capabilities: { ...value.bootstrap.capabilities, feed: { available: false, reason: 'FEATURE_DISABLED' } as const } }
    expect(createFeedSocialPorts({ readiness: context => ({ read: () => ({ scope: context.scope, bootstrap, services: { feed: true } }), subscribe: () => () => {} }) })
      .availability({ ...value.context, bootstrap })?.read()?.services.feed).toBe(false)
  })
  it('withdraws old leases on policy change and releases borrowed observers', () => {
    const value = setup(), leaf = createFeedSocialPorts({ readiness: () => value.availability }).availability(value.context)!
    let changes = 0
    const stop = leaf.subscribe(() => { changes++ })
    expect(value.listenerCount()).toBe(1)
    value.auth.privacy.changePolicy(value.scope, 'block')
    expect(changes).toBeGreaterThan(0); expect(leaf.read()).toBeNull()
    stop(); stop(); expect(value.listenerCount()).toBe(0)
  })
  it('fails closed if observation cannot be established', () => {
    const value = setup()
    const leaf = createFeedSocialPorts({ readiness: () => ({ read: value.availability.read, subscribe() { throw Error('Lost observer') } }) }).availability(value.context)!
    leaf.subscribe(() => {})()
    expect(leaf.read()).toBeNull()
  })
})
