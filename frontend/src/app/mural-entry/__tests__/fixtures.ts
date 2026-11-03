import { createSessionController } from '../../../features/auth/model/session'
import { isolatedCookiePolicy, deferred } from '../../../features/auth/__tests__/fixtures/data'
import { authorityPort, ids, memoryScene, serverFixture, testClock, timestamp, drain } from '../../../features/mural/sync/__tests__/fixture'
import { createMuralEntry } from '../owner'
import type { MuralEntryRelease } from '../ports'

export { ids, deferred, drain }
/** Canonical Auth/HTTP/privacy; synthetic in-process C04 server and incumbent scene. No product import. */
export function entryFixture() {
  const server = serverFixture(), clock = testClock(), requests: Request[] = []
  let hold: { match: (r: Request) => boolean; response: ReturnType<typeof deferred<Response>>; arrived: ReturnType<typeof deferred<Response>> } | null = null
  const auth = createSessionController({ eventId: ids.event, cookiePolicy: isolatedCookiePolicy(), fetch: async input => {
    const req = input as Request; requests.push(req.clone())
    const result = await server.fetch(input)
    if (hold?.match(req)) { hold.arrived.resolve(result); return hold.response.promise }
    return result
  } })
  // Test setup installs a synthetic authenticated session in the actual canonical Auth owner.
  auth.session.replace(0, { userId: ids.user, eventId: ids.event, accessToken: ids.user })
  const scope = auth.privacy.capture(), owners: ReturnType<typeof createMuralEntry>[] = []
  function release(faulty = false): ReturnType<typeof makeRelease> { return makeRelease(faulty) }
  function makeRelease(faulty: boolean) {
    const authority = authorityPort(), scene = memoryScene(), lifetime = new AbortController(), close = new Set<() => void>()
    const value: MuralEntryRelease = { scope, binding: { ...authority.read()!.binding }, signal: lifetime.signal,
      authority: { read: authority.read, subscribe(fn) { const stop = authority.subscribe(fn); return () => { stop(); if (faulty) throw new Error('synthetic authority cleanup fault') } } },
      scene: { ...scene.port,
        onFrame(fn) { const stop = scene.port.onFrame(fn); return () => { stop(); if (faulty) throw new Error('synthetic frame cleanup fault') } },
        onClose(fn) { close.add(fn); return () => { close.delete(fn); if (faulty) throw new Error('synthetic close cleanup fault') } },
      } }
    return { value, authority, scene, lifetime, close() { for (const fn of Array.from(close)) fn() }, count: () => close.size }
  }
  return { auth, scope, server, clock, requests, release,
    create(value: MuralEntryRelease | null = null, receiptNow = () => timestamp) {
      const owner = createMuralEntry({ auth, scope, clock, receiptNow }); owners.push(owner); owner.bind(value); return owner
    },
    hold(match: (r: Request) => boolean) { hold = { match, response: deferred<Response>(), arrived: deferred<Response>() }; return hold },
    stopHold() { hold = null },
    dispose() { owners.forEach(o => o.dispose()); auth.dispose() },
  }
}
