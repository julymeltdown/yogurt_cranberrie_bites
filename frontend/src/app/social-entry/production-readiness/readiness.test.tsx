// @vitest-environment jsdom
import { StrictMode, act, useLayoutEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createProductionSocialReadiness } from './index'
import { createSocialAvailabilityLifetime } from '../availability-lifetime'
import { deferred, event, id, setup, success, user } from './fixtures'

const cleanups: (() => void)[] = []
beforeEach(() => { vi.stubGlobal('location', { origin: 'https://mcr.example' }); vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true) })
afterEach(() => { cleanups.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks(); vi.unstubAllGlobals() })
const all = { feed: true, people: true, intro: true, age: true, inbox: true, requests: true }
function owner(handler?: Parameters<typeof setup>[0]) {
  const f = setup(handler), port = createProductionSocialReadiness(f.context, f.world), caller = new AbortController()
  cleanups.push(f.dispose, port.dispose)
  return { ...f, port, caller, prepare: () => port.prepare!(caller.signal) }
}
async function ready(f: ReturnType<typeof owner>) { await f.prepare(); f.policy(f.baseline()); await vi.waitFor(() => expect(f.port.read()?.services).toEqual(all)) }

it('construction/subscription are inert and prepare makes no read before the matching baseline', async () => {
  const f = owner(), worldRead = vi.spyOn(f.world, 'read'), policyRead = vi.spyOn(f.policyPort, 'read')
  const stop = f.port.subscribe(() => {}); cleanups.push(stop)
  expect(f.port.read()).toBeNull(); expect(worldRead).not.toHaveBeenCalled(); expect(policyRead).not.toHaveBeenCalled()
  expect(f.listenerCount()).toBe(0); expect(f.requests).toHaveLength(0)
  await f.prepare(); expect(f.port.read()).toBeNull(); expect(f.requests).toHaveLength(0)
  f.policy(f.baseline()); await vi.waitFor(() => expect(f.port.read()?.services).toEqual(all))
  expect(f.requests).toHaveLength(6); expect(f.requests.every(request => request.method === 'GET')).toBe(true)
  expect(f.requests.some(request => new URL(request.url).pathname.endsWith('/feed'))).toBe(false)
  expect(f.port.read()).toEqual({ scope: f.scope, bootstrap: f.bootstrap, services: all })
  expect(f.port.read()!.bootstrap).toBe(f.bootstrap)
})

it('Feed needs configured C03 and policy only; other producers do not wait for Feed availability', async () => {
  const f = owner(), bootstrap = { ...f.bootstrap, capabilities: { ...f.bootstrap.capabilities,
    feed: { available: false, reason: 'FEATURE_DISABLED' } as const,
    social_sharing: { available: false, reason: 'FEATURE_DISABLED' } as const } }
  const port = createProductionSocialReadiness({ ...f.context, bootstrap }, f.world); cleanups.push(port.dispose!)
  f.binding({ ...f.valid, release: { ...f.valid.release, bootstrap } }); f.policy(f.baseline())
  await port.prepare!(f.caller.signal)
  await vi.waitFor(() => expect(port.read()?.services).toEqual({ ...all, feed: false, requests: false }))
  expect(f.requests).toHaveLength(4); expect(f.requests.some(request => /feed|social-handles|social-requests/.test(new URL(request.url).pathname))).toBe(false)
})

it.each([
  ['people', '/intros'], ['intro', '/me/intro'], ['age', '/me/age-status'], ['inbox', '/me/notifications'],
  ['requests', '/me/social-handles'], ['requests', '/me/social-requests'],
] as const)('malformed strict %s body at %s fails only that producer', async (service, suffix) => {
  const f = owner(request => new URL(request.url).pathname.endsWith(suffix) ? Response.json({ items: [], private_extra: 'discard' }) : success(request))
  await f.prepare(); f.policy(f.baseline())
  await vi.waitFor(() => expect(f.port.read()?.services).toEqual({ ...all, [service]: false }))
})

it('a denied Intro does not suppress current independent services or synthesize an Intro', async () => {
  const f = owner(request => new URL(request.url).pathname.endsWith('/me/intro') ? Response.json({ code: 'RESOURCE_UNAVAILABLE', message: 'unit', details: { reason: 'RESOURCE_UNAVAILABLE' } }, { status: 404 }) : success(request))
  await f.prepare(); f.policy(f.baseline())
  await vi.waitFor(() => expect(f.port.read()?.services).toEqual({ ...all, intro: false }))
})

it.each(['userId', 'eventId', 'generation', 'policyGeneration', 'gate'] as const)('foreign baseline %s cannot start a probe', async field => {
  const f = owner(), changes = { userId: id(999), eventId: id(998), generation: f.scope.generation + 1, policyGeneration: f.scope.policyGeneration + 1, gate: 'suspended' as const }
  await f.prepare(); f.policy({ ...f.baseline(), scope: { ...f.scope, [field]: changes[field] } })
  await Promise.resolve(); expect(f.requests).toHaveLength(0); expect(f.port.read()).toBeNull()
})

it('requires the exact bootstrap reference and release scope, not a cloned or foreign C03', async () => {
  const f = owner(); f.policy(f.baseline())
  f.binding({ ...f.valid, release: { ...f.valid.release, bootstrap: { ...f.bootstrap } } }); await f.prepare()
  expect(f.requests).toHaveLength(0); expect(f.port.read()).toBeNull()
  f.binding({ ...f.valid, release: { ...f.valid.release, scope: { ...f.valid.release.scope, userId: id(999) } } })
  expect(f.requests).toHaveLength(0); expect(f.port.read()).toBeNull()
  f.binding(f.valid); await vi.waitFor(() => expect(f.port.read()?.services).toEqual(all))
})

it.each(['status', 'path', 'userId', 'eventId', 'generation'] as const)('rejects otherwise valid HTTP body with foreign %s metadata', async field => {
  const f = owner(), original = f.auth.request
  const intercepted: typeof original = request => {
    if (!request.path.endsWith('/me/age-status')) return original(request)
    const response = { status: field === 'status' ? 202 : 200, path: field === 'path' ? request.path + '/foreign' : request.path,
      scope: { userId: field === 'userId' ? id(999) : user, eventId: field === 'eventId' ? id(998) : event, generation: f.scope.generation + (field === 'generation' ? 1 : 0) } }
    return Promise.resolve().then(() => ({ data: request.decode({ user_id: user, event_id: event, status: 'UNKNOWN', verified_at: null, expires_at: null, verification_available: false }, response), scope: f.scope }))
  }
  vi.spyOn(f.auth, 'request').mockImplementation(intercepted)
  await f.prepare(); f.policy(f.baseline())
  await vi.waitFor(() => expect(f.port.read()?.services).toEqual({ ...all, age: false }))
})

it('policy replacement withdraws first, cancels the old cohort, and rejects its late success', async () => {
  const held = deferred<Response>(); let hold = true
  const f = owner(request => hold && new URL(request.url).pathname.endsWith('/me/intro') ? held.promise : success(request))
  await f.prepare(); f.policy(f.baseline()); await vi.waitFor(() => expect(f.requests).toHaveLength(6))
  const withdrawn: boolean[] = [], stop = f.port.subscribe(() => { withdrawn.push(f.port.read() === null) }); cleanups.push(stop)
  hold = false; f.policy({ ...f.baseline(), policyRevision: 2 })
  expect(withdrawn).toContain(true)
  held.resolve(Response.json({ invalid: 'late-private-body' }))
  await vi.waitFor(() => expect(f.port.read()?.services).toEqual(all))
  f.policy(null); expect(f.port.read()).toBeNull(); await Promise.resolve(); expect(f.port.read()).toBeNull()
})

it('response decode and final commit both fence a silently replaced incumbent policy', async () => {
  const held = deferred<Response>(), f = owner(request => new URL(request.url).pathname.endsWith('/me/intro') ? held.promise : success(request))
  await f.prepare(); f.policy(f.baseline()); await vi.waitFor(() => expect(f.requests.length).toBeGreaterThan(0))
  f.policy({ ...f.baseline(), policyRevision: 2 }, false)
  held.resolve(await success(new Request(`https://mcr.example/api/concert/events/${event}/me/intro`)))
  await Promise.resolve(); expect(f.port.read()).toBeNull()
})

it.each(['privacy', 'release', 'caller', 'world', 'dispose'] as const)('%s withdrawal is synchronous and late responses cannot resurrect readiness', async kind => {
  const f = owner(); await ready(f)
  const count = f.requests.length
  if (kind === 'privacy') f.auth.privacy.changePolicy(f.scope, 'consent-expired')
  if (kind === 'release') f.release.abort()
  if (kind === 'caller') f.caller.abort()
  if (kind === 'world') f.binding(null)
  if (kind === 'dispose') f.port.dispose!()
  expect(f.port.read()).toBeNull(); await Promise.resolve(); expect(f.requests).toHaveLength(count)
  if (kind !== 'world' && kind !== 'release') expect(f.listenerCount()).toBe(0)
})

it('a prepared caller remains attached; repeated prepare never repeats queries', async () => {
  const f = owner(); await ready(f); await f.prepare(); expect(f.requests).toHaveLength(6)
  f.caller.abort(); expect(f.port.read()).toBeNull(); expect(f.listenerCount()).toBe(0)
})

it('the committed StrictMode lifecycle abandons the first constructor before any probes', async () => {
  const f = owner(), constructed: ReturnType<typeof createProductionSocialReadiness>[] = []
  f.policy(f.baseline())
  function Committed() {
    useLayoutEffect(() => {
      const lifetime = createSocialAvailabilityLifetime(f.context, context => {
        const port = createProductionSocialReadiness(context, f.world); constructed.push(port); return port
      })
      lifetime.start(); return lifetime.dispose
    }, [])
    return null
  }
  const host = document.createElement('div'), root = createRoot(host)
  try {
    await act(async () => { root.render(<StrictMode><Committed /></StrictMode>) })
    await vi.waitFor(() => expect(constructed[1]?.read()?.services).toEqual(all))
    expect(f.requests).toHaveLength(6)
    expect(constructed).toHaveLength(2); expect(constructed[0]!.read()).toBeNull()
    expect(constructed[1]!.read()?.services).toEqual(all)
  } finally { await act(async () => { root.unmount() }) }
  expect(f.listenerCount()).toBe(0); expect(constructed.every(port => port.read() === null)).toBe(true)
})
