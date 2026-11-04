import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createSafetyEntry } from '../../../safety-entry/owner'
import { reportAuthorization } from '../../../safety-entry/authorization'
import { createSocialOwner } from '../../owner'
import { canonicalSharingRequests } from '../../sharing'
import { fixture, event, user, deferred } from '../fixtures'
import { scenarioService } from './service'
const stops: (() => void)[] = []
beforeEach(() => vi.stubGlobal('location', { origin: 'https://mcr.example' }))
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()); vi.unstubAllGlobals() })
function setup(handler?: (r: Request, fallback: (r: Request) => Promise<Response>) => Promise<Response>) {
  const service = scenarioService(user, event), f = fixture(r => handler ? handler(r, service.respond) : service.respond(r))
  let path = `/people/${service.recipient}`
  const destinations: unknown[] = [], owner = createSafetyEntry(f.auth, () => path, scope => destinations.push(scope))
  stops.push(() => { owner.dispose(); f.dispose() })
  return { ...f, service, owner, destinations, route(value: string) { path = value; owner.changeRoute(value) } }
}
it('authorizes only fresh registered targets and rejects unqualified report media and unsupported target lookups', async () => {
  const f = setup(), port = reportAuthorization(f.auth, () => 100), target = { target_type: 'USER' as const, target_id: f.service.recipient }, signal = new AbortController().signal
  expect(await port.check(target, [], f.scope, signal)).toMatchObject({ allowed: true, target, scope: f.scope, evidenceMediaIds: [], expiresAt: 10100 })
  const count = f.requests.length
  await expect(port.check(target, [f.service.recipient], f.scope, signal)).rejects.toThrow('attachments unavailable')
  await expect(port.check({ ...target, target_type: 'COMMENT' }, [], f.scope, signal)).rejects.toThrow('No qualified target lookup')
  expect(f.requests).toHaveLength(count)
  f.service.mode('target-denied'); await expect(port.check(target, [], f.scope, signal)).rejects.toBeDefined()
  expect(f.requests.every(r => r.headers.get('Authorization')?.startsWith('Bearer ') && r.cache === 'no-store')).toBe(true)
})
it('aborts and ignores a late block-target response after route change', async () => {
  const gate = deferred<Response>(); let targetRead: Request | undefined
  const f = setup(async (r, fallback) => { if (r.url.includes('/people/')) { targetRead = r; return gate.promise }; return fallback(r) })
  const opening = f.owner.beginBlock(f.service.recipient, f.scope)
  while (!targetRead) await new Promise(resolve => setTimeout(resolve, 0))
  f.route('/me/blocks'); expect(targetRead.signal.aborted).toBe(true)
  gate.resolve(Response.json(f.service.intro.author)); expect(await opening).toBe(false)
  expect(f.owner.read().target).toBeNull(); expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
it('cannot confirm an old block selection after canonical privacy withdrawal', async () => {
  const f = setup(); expect(await f.owner.beginBlock(f.service.recipient, f.scope)).toBe(true)
  f.auth.privacy.changePolicy(f.scope, 'block'); await f.owner.confirmBlock()
  expect(f.owner.read().controller).toBeNull(); expect(f.owner.read().target).toBeNull()
  expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
it('preserves one original safety intent across route changes and retry without replay', async () => {
  const f = setup(); expect(await f.owner.beginBlock(f.service.recipient, f.scope)).toBe(true)
  f.service.mode('unknown'); await f.owner.confirmBlock()
  const safety = f.owner.read().controller!, key = f.service.last()!.key
  expect(safety.read().phase).toBe('unknown'); f.route('/me/blocks'); expect(await f.owner.retry()).toBe(false)
  expect(f.owner.read().controller).toBe(safety); await safety.loadBlocks(); await safety.checkStatus()
  expect(f.requests.filter(r => r.method !== 'GET')).toHaveLength(1)
  const status = f.requests.find(r => new URL(r.url).pathname.endsWith('/commands'))!
  expect(new URL(status.url).searchParams.get('idempotency_key')).toBe(key)
  expect(new URL(status.url).searchParams.get('operation_id')).toBe('blockConcertUser')
})
it('does not send a report when the target is withdrawn between open and submit', async () => {
  const f = setup(); await f.owner.prepare(); const safety = f.owner.read().controller!
  await safety.open(`?targetType=USER&targetId=${f.service.recipient}`); expect(safety.read().phase).toBe('ready')
  f.service.mode('target-denied'); await safety.report()
  expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
it('does not navigate from a forged or freshly denied introduction', async () => {
  const f = setup(); f.setLease({ scope: f.scope, bootstrap: f.bootstrap, services: { people: true, requests: true } })
  const owner = createSocialOwner({ ...f.options, requests: canonicalSharingRequests }); stops.push(owner.dispose)
  owner.enter(`/people/${f.service.recipient}`); await owner.people.load(); await owner.people.openProfile(f.service.recipient)
  expect(owner.canShare(f.service.recipient)).toBe(true); f.service.mode('intro-denied')
  expect(await owner.openSharing(f.service.recipient)).toBe(false); expect(f.navigations).toEqual([])
  expect(await owner.openSharing(user)).toBe(false); expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
it('aborts and ignores a late authorized introduction after leaving its Profile', async () => {
  const gate = deferred<Response>(); let introRead: Request | undefined
  const f = setup(async (r, fallback) => { if (/\/intros\/[^/]+$/.test(new URL(r.url).pathname)) { introRead = r; return gate.promise }; return fallback(r) })
  f.setLease({ scope: f.scope, bootstrap: f.bootstrap, services: { people: true, requests: true } })
  const owner = createSocialOwner({ ...f.options, requests: canonicalSharingRequests }); stops.push(owner.dispose)
  owner.enter(`/people/${f.service.recipient}`); await owner.people.load(); await owner.people.openProfile(f.service.recipient)
  const opening = owner.openSharing(f.service.recipient)
  while (!introRead) await new Promise(resolve => setTimeout(resolve, 0))
  owner.enter('/meet/people')
  expect(introRead?.signal.aborted).toBe(true); gate.resolve(Response.json(f.service.intro))
  expect(await opening).toBe(false); expect(f.navigations).toEqual([]); expect(owner.requests()).toBeNull()
})
it.each(['reports', 'blocks'] as const)('rejects duplicate own %s rows without granting that service', async service => {
  const f = setup(async (r, fallback) => {
    if (new URL(r.url).pathname.endsWith(`/me/${service}`)) {
      const row = service === 'blocks' ? { target_user_id: '00000000-0000-4000-8000-000000000099', blocked_by_me: true, revision: 1 }
        : { report_id: '00000000-0000-4000-8000-000000000095', event_id: event, target_type: 'USER', target_id: '00000000-0000-4000-8000-000000000099', state: 'OPEN', revision: 1, created_at: '2026-11-01T00:00:00Z' }
      return Response.json({ items: [row, row], next_cursor: null })
    }
    return fallback(r)
  })
  await f.owner.prepare(); expect(f.owner.read()[service]).toBe(false)
  expect(f.requests.every(r => r.method === 'GET')).toBe(true)
})
