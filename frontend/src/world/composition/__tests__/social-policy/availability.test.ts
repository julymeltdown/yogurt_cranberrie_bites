import { expect, it, vi } from 'vitest'
import { createWorldSocialAvailability } from '../../../../app/world-entry/social-availability'
import type { SocialContext } from '../../../../app/social-composition/ports'
import type { WorldAccessBinding, WorldAccessBindingPort, ShellBootstrap } from '../../../../app/ports'
import type { WorldSocialPolicy } from '../../policy'

function fixture() {
  const scope = Object.freeze({ userId: 'actor', eventId: 'event', generation: 1, policyGeneration: 0, gate: 'open' as const })
  const capability = Object.freeze({ available: true as const, reason: null })
  const bootstrap: ShellBootstrap = { contractVersion: '0.1.0', event: { eventId: 'event', title: 'fixture', phase: 'LIVE' },
    participant: { userId: 'actor', eventId: 'event' }, worldAvailable: true,
    capabilities: { world: capability, feed: capability, meet: capability, gifts: capability, wall: capability,
      social_sharing: capability, mutations: capability } }
  let current = true, policy: WorldSocialPolicy | null = null
  const context: SocialContext = { scope, bootstrap,
    auth: { privacy: { isCurrent: () => current } } as unknown as SocialContext['auth'] }
  const observers = new Set<() => void>(), parents = new Set<() => void>()
  const release = { scope, privacyScope: scope, bootstrap, signal: new AbortController().signal }
  const binding = { release, socialPolicy: { read: () => policy, subscribe(fn: () => void) {
    observers.add(fn); return () => { observers.delete(fn) } } } } as unknown as WorldAccessBinding
  let active: WorldAccessBinding | null = binding
  const world: WorldAccessBindingPort = { read: () => active, subscribe(fn) { parents.add(fn); return () => { parents.delete(fn) } } }
  const reader = createWorldSocialAvailability(context, world)
  const ready = () => { policy = { scope, eventId: 'event', streamId: 'stream', roomEpoch: 'epoch', policyRevision: 1 } }
  return { context, reader, ready, binding, world, observers, parents,
    policy(value: WorldSocialPolicy | null) { policy = value },
    invalidate() { current = false }, unbind() { active = null; for (const fn of parents) fn() } }
}
it('never treats configured C03 Feed alone as a policy baseline', () => {
  const f = fixture(); expect(f.reader.read()).toBeNull(); f.ready()
  const value = f.reader.read(); expect(value?.services.feed).toBe(true)
  expect(value?.services).toEqual({ feed: true, people: false, intro: false, age: false, inbox: false, requests: false })
  expect(f.reader.read()).toBe(value)
})
it('rejects foreign full privacy scope and canonical withdrawal without issuing any reads', () => {
  const f = fixture(); f.ready(); expect(f.reader.read()).not.toBeNull()
  f.policy({ scope: { ...f.context.scope, policyGeneration: 1 }, eventId: 'event', streamId: 's', roomEpoch: 'e', policyRevision: 1 })
  expect(f.reader.read()).toBeNull(); f.ready(); f.invalidate(); expect(f.reader.read()).toBeNull()
})
it('rebinds incumbent notifications and returns all borrowed subscriptions without disposing World', () => {
  const f = fixture(), listener = vi.fn(); const stop = f.reader.subscribe(listener)
  expect(f.observers.size).toBe(1); expect(f.parents.size).toBe(1)
  f.unbind(); expect(f.observers.size).toBe(0); expect(f.reader.read()).toBeNull()
  stop(); expect(f.parents.size).toBe(0)
})
