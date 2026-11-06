// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createProductionWorld } from '../../world-entry/production'
import { authHarness } from '../../world-entry/__tests__/admitted-auth'
import { requestFor, replicationPort, inputConfiguration } from '../../world-entry/__tests__/data'
import { deferred } from '../../../features/auth/__tests__/fixtures/data'
import type { WorldComposition } from '../../../world/composition'

const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()) })
async function setup() {
  const h = authHarness(), request = await requestFor(h.auth), replication = replicationPort(request)
  stops.push(h.auth.dispose)
  const reconnect = vi.fn(async () => true), loaded = deferred<WorldComposition>()
  const composition: WorldComposition = {
    mount: () => ({ dispose() {}, setInputBlocked() {}, setVisible() {} }),
    replication: { ...replication, reconnect }, subscribe: replication.subscribe,
    read: () => ({ phase: 'ready', runtime: null, renderer: 'unavailable', problem: null }), dispose() {}, async settled() {},
  }
  const production = createProductionWorld({ auth: h.auth, localInput: inputConfiguration, load: () => loaded.promise })
  stops.push(production.dispose)
  const host = document.createElement('div'), abort = new AbortController()
  const lease = production.world.mount(host, { ...request, signal: abort.signal })
  const binding = production.access.read()!
  return { ...h, request, replication, reconnect, loaded, composition, production, host, abort, lease, binding,
    async admit() { loaded.resolve(composition); await new Promise(resolve => setTimeout(resolve, 0)) } }
}
it('uses the existing lazy composition only, never allocates for an early reconnect, and rejects an obsolete lease', async () => {
  const h = await setup()
  expect(await h.binding.reconnect!()).toBe(false); expect(h.reconnect).not.toHaveBeenCalled()
  await h.admit(); expect(await h.binding.reconnect!()).toBe(true); expect(h.reconnect).toHaveBeenCalledOnce()
  h.lease.dispose(); h.production.world.mount(h.host, h.request)
  expect(await h.binding.reconnect!()).toBe(false); expect(h.reconnect).toHaveBeenCalledOnce()
})
it.each(['abort', 'policy', 'identity', 'logout', 'dispose'] as const)('fences late production reconnect on %s', async cause => {
  const h = await setup(); await h.admit()
  const held = deferred<boolean>(); h.reconnect.mockReturnValue(held.promise)
  const result = h.binding.reconnect!()
  if (cause === 'abort') h.abort.abort()
  if (cause === 'policy') h.auth.privacy.changePolicy(h.request.privacyScope, 'block')
  if (cause === 'identity') h.auth.session.replace(h.request.scope.generation, { userId: '00000000-0000-4000-8000-000000000999', eventId: h.request.scope.eventId, accessToken: 'test-memory-only' })
  if (cause === 'logout') void h.auth.logout()
  if (cause === 'dispose') h.production.dispose()
  held.resolve(true); expect(await result).toBe(false)
  expect(h.production.access.read()).toBeNull(); expect(await h.binding.reconnect!()).toBe(false)
  expect(h.reconnect).toHaveBeenCalledOnce()
})
