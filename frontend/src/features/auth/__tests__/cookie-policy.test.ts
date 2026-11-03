import { afterEach, expect, it, vi } from 'vitest'
import { TransportError } from '../../../shared/api/errors'
import { createBrowserCookiePolicy, type CookiePolicy } from '../model/cookie-policy'
import { deferred } from './fixtures/data'
const policies: CookiePolicy[] = []
const messages: unknown[] = []
class Channel {
  static instances = new Set<Channel>()
  onmessage: ((event: { data: unknown }) => void) | null = null
  constructor() { Channel.instances.add(this) }
  postMessage(data: unknown) { messages.push(structuredClone(data)); for (const peer of Channel.instances) if (peer !== this) peer.onmessage?.({ data }) }
  close() { Channel.instances.delete(this) }
}
function setup() {
  let tail = Promise.resolve()
  const locks = { request: async (_name: string, options: { signal: AbortSignal }, work: () => Promise<unknown>) => {
    const prior = tail, next = deferred<void>(); tail = next.promise
    try { await prior; if (options.signal.aborted) throw new Error('Aborted'); return await work() }
    finally { next.resolve() }
  } }
  vi.stubGlobal('navigator', { locks }); vi.stubGlobal('BroadcastChannel', Channel)
  const policy = createBrowserCookiePolicy(); policies.push(policy); return policy
}
afterEach(() => { policies.splice(0).forEach(p => p.dispose()); Channel.instances.clear(); messages.length = 0; vi.unstubAllGlobals(); vi.useRealTimers() })
it('sends only exact non-secret revocation protocol and discards malformed messages', async () => {
  const policy = setup(), changed = vi.fn(); policy.onInvalidate(changed)
  const sender = new Channel()
  for (const invalid of [null, {}, { version: 2, type: 'revoke' }, { version: 1, type: 'revoke', accessToken: 'secret' }, { version: 1, type: 'unknown' }]) sender.postMessage(invalid)
  expect(changed).not.toHaveBeenCalled()
  sender.postMessage({ version: 1, type: 'revoke' }); expect(changed).toHaveBeenCalledTimes(1)
  messages.length = 0
  await policy.run('login', async () => undefined)
  expect(messages).toEqual([{ version: 1, type: 'revoke' }])
})
it('serializes writers and fails closed on lock starvation without dispatch', async () => {
  const policy = setup(), hold = deferred<void>(), work = vi.fn(async () => undefined)
  const first = policy.run('login', () => hold.promise)
  await Promise.resolve(); vi.useFakeTimers()
  const second = policy.run('logout', work).catch(e => e)
  await vi.advanceTimersByTimeAsync(5001); hold.resolve(); await first
  expect(await second).toMatchObject({ kind: 'aborted' }); expect(work).not.toHaveBeenCalled()
})
it('latches uncertain rotations, informs peers/newcomers, allows only explicit server logout', async () => {
  const policy = setup()
  await expect(policy.run('refresh', async () => { throw new TransportError('timeout', { uncertain: true }) })).rejects.toMatchObject({ uncertain: true })
  expect(policy.available).toBe(false)
  const work = vi.fn(async () => undefined)
  await expect(policy.run('login', work)).rejects.toMatchObject({ kind: 'invalid-request' }); expect(work).not.toHaveBeenCalled()
  const newcomer = createBrowserCookiePolicy(); policies.push(newcomer)
  expect(newcomer.available).toBe(false)
  await policy.run('logout', work); expect(work).toHaveBeenCalledTimes(1)
  expect(policy.available).toBe(false) // A 204 cannot retroactively qualify an unknown earlier cookie writer.
})
it('does not latch an ordinary rejected credential response', async () => {
  const policy = setup()
  await expect(policy.run('login', async () => { throw new TransportError('http', { status: 401 }) })).rejects.toMatchObject({ status: 401 })
  expect(policy.available).toBe(true)
})
it('announces uncertainty when disposed with a dispatched cookie writer', async () => {
  const policy = setup(), hold = deferred<void>()
  const writing = policy.run('login', () => hold.promise)
  await Promise.resolve(); policy.dispose(); hold.resolve(); await writing
  expect(messages).toContainEqual({ version: 1, type: 'uncertain' })
})
it('keeps cookie auth unavailable if browser coordination is missing or denied', async () => {
  vi.stubGlobal('navigator', {}); vi.stubGlobal('BroadcastChannel', undefined)
  const absent = createBrowserCookiePolicy(); policies.push(absent)
  expect(absent.available).toBe(false)
  await expect(absent.run('login', async () => undefined)).rejects.toMatchObject({ kind: 'invalid-request' })
  vi.stubGlobal('BroadcastChannel', class { constructor() { throw new Error('Denied') } })
  expect(() => { const denied = createBrowserCookiePolicy(); policies.push(denied); expect(denied.available).toBe(false) }).not.toThrow()
})
