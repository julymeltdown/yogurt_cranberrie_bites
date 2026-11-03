import { afterEach, describe, expect, it, vi } from 'vitest'
import { dehydrate, MutationObserver, QueryClient, QueryObserver } from '@tanstack/react-query'
import { createHttpClient, createMemorySession } from '../../api/http'
import { privateQueryKey, publicQueryKey } from '../keys'
import { createQueryPrivacy, PrivacyError, publicOnlyDehydrateOptions, type PolicyChange } from '../privacy'

const USER = '00000000-0000-4000-8000-000000000101'
const OTHER = '00000000-0000-4000-8000-000000000102'
const EVENT = '00000000-0000-4000-8000-000000000201'
const GRANT = '00000000-0000-4000-8000-000000000301'
const NEXT_EVENT = '00000000-0000-4000-8000-000000000202'
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
const disposals: (() => void)[] = []
afterEach(() => { for (const dispose of disposals.splice(0).reverse()) dispose(); vi.restoreAllMocks(); vi.unstubAllGlobals() })
function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  const session = createMemorySession()
  session.replace(0, { userId: USER, eventId: EVENT, accessToken: 'memory-token-A' })
  const privacy = createQueryPrivacy({ client, session })
  disposals.push(() => client.clear(), () => privacy.dispose())
  return { client, session, privacy }
}
const rejection = async (promise: Promise<unknown>) => {
  try { await promise; throw new Error('Expected rejection') }
  catch (error) { return error }
}
const drain = () => new Promise(resolve => setTimeout(resolve, 0))

describe('identity changes cancel, scrub and evict real QueryClient data', () => {
  it.each(['logout', 'account', 'event'] as const)('clears inactive and active data on %s and discards ignored aborts', async kind => {
    const { client, session, privacy } = setup()
    const pending = deferred<string>()
    let signal!: AbortSignal
    const options = privacy.options({ resource: 'participant', read: context => { signal = context.signal; return pending.promise } })
    const results: (string | undefined)[] = []
    const observer = new QueryObserver(client, options)
    const stop = observer.subscribe(result => results.push(result.data))
    disposals.push(() => observer.destroy(), stop)
    await vi.waitFor(() => expect(signal).toBeDefined())
    // Cached inactive reveal and previous data on the active query must also disappear.
    client.setQueryData(privateQueryKey({ ...privacy.capture(), userId: USER }, 'social-reveal', [GRANT]), '@private-A')
    client.setQueryData(options.queryKey, 'cached-private-A')
    client.setQueryData(publicQueryKey(), { EMAIL: true })
    expect(observer.getCurrentResult().data).toBe('cached-private-A')
    session.replace(1, kind === 'logout' ? null : { userId: kind === 'account' ? OTHER : USER,
      eventId: kind === 'event' ? NEXT_EVENT : EVENT, accessToken: 'memory-token-B' })
    expect(signal.aborted).toBe(true)
    expect(observer.getCurrentResult().data).toBeUndefined()
    expect(results.at(-1)).toBeUndefined()
    expect(client.getQueryData(options.queryKey)).toBeUndefined()
    expect(client.getQueryCache().getAll().map(query => query.queryKey)).toEqual([publicQueryKey()])
    pending.resolve('late-private-A')
    await drain()
    expect(client.getQueryData(options.queryKey)).toBeUndefined()
    expect(observer.getCurrentResult().data).toBeUndefined()
    if (kind !== 'logout') {
      const next = privacy.options({ resource: 'participant', read: () => 'fresh-private-B' })
      expect(await client.fetchQuery(next)).toBe('fresh-private-B')
      expect(client.getQueryData(next.queryKey)).toBe('fresh-private-B')
      expect(client.getQueryData(options.queryKey)).toBeUndefined()
    }
  })

  it('composes with FE01 HttpResult and cancellation without allowing A into B cache', async () => {
    const { client, session, privacy } = setup()
    vi.stubGlobal('location', { origin: 'https://mcr.example' })
    const delayed = deferred<Response>()
    const fetch = vi.fn<typeof globalThis.fetch>(async () => delayed.promise)
    const http = createHttpClient({ session, fetch })
    const options = privacy.options({ resource: 'participant', read: async ({ signal }) => {
      const result = await http.request({ path: `/api/concert/events/${EVENT}/me`, method: 'GET',
        access: 'protected', errorFamily: 'core', signal,
        decode: (value, context) => {
          const data = value as { user_id: string; event_id: string }
          if (data.user_id !== context.scope.userId || data.event_id !== context.scope.eventId) throw new Error('Invalid context')
          return data
        },
      })
      return result.data
    } })
    const result = rejection(client.fetchQuery(options))
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
    session.replace(1, { userId: OTHER, eventId: EVENT, accessToken: 'B-token' })
    await result
    delayed.resolve(Response.json({ user_id: USER, event_id: EVENT }))
    await drain()
    expect(client.getQueryData(options.queryKey)).toBeUndefined()
    expect(privacy.capture().userId).toBe(OTHER)
    const current = privacy.options({ resource: 'participant', read: () => ({ user_id: OTHER, event_id: EVENT }) })
    expect(await client.fetchQuery(current)).toEqual({ user_id: OTHER, event_id: EVENT })
    expect(client.getQueryData(current.queryKey)).toEqual({ user_id: OTHER, event_id: EVENT })
  })

  it('keeps private queries valid during same-identity token refresh', async () => {
    const { client, session, privacy } = setup()
    const listener = vi.fn(); privacy.subscribe(listener)
    const pending = deferred<string>()
    const options = privacy.options({ resource: 'participant', read: () => pending.promise })
    const result = client.fetchQuery(options)
    session.commitRefresh(session.read(), { userId: USER, accessToken: 'rotated-token' })
    pending.resolve('same-user-data')
    expect(await result).toBe('same-user-data')
    expect(privacy.options({ resource: 'participant', read: () => 'unused' }).queryKey).toEqual(options.queryKey)
    expect(listener).not.toHaveBeenCalled()
  })

  it('refuses obsolete observer refetch without dispatch, then rebinds fresh options', async () => {
    const { client, privacy } = setup()
    const read = vi.fn(() => 'private-handle')
    const observer = new QueryObserver(client, privacy.options({ resource: 'social-reveal', entityIds: [GRANT], read }))
    const stop = observer.subscribe(() => undefined)
    disposals.push(() => observer.destroy(), stop)
    expect(read).not.toHaveBeenCalled() // Reveal is explicitly requested, never fetched on mount.
    await observer.refetch()
    expect(observer.getCurrentResult().data).toBe('private-handle')
    privacy.changePolicy(privacy.capture(), 'consent-revoked')
    expect(observer.getCurrentResult().data).toBeUndefined()
    const old = await observer.refetch()
    expect(old.data).toBeUndefined(); expect(old.error).toMatchObject({ kind: 'scope-expired' })
    expect(read).toHaveBeenCalledTimes(1)
    observer.setOptions(privacy.options({ resource: 'social-reveal', entityIds: [GRANT], read: () => 'newly-authorized-value' }))
    expect(observer.getCurrentResult().data).toBeUndefined()
    await observer.refetch()
    expect(observer.getCurrentResult().data).toBe('newly-authorized-value')
  })

  it('fences the microtask gap between queryFn settlement and Query cache commit', async () => {
    const { client, privacy } = setup()
    const scope = privacy.capture()
    const success = vi.fn()
    client.getQueryCache().config.onSuccess = success
    const options = privacy.options({ resource: 'social-reveal', entityIds: [GRANT], read: () => '@must-not-commit' })
    const originalRead = options.queryFn
    if (typeof originalRead !== 'function') throw new Error('Expected query function')
    const observer = new QueryObserver(client, {
      ...options,
      queryFn: context => {
        const pending = Promise.resolve(originalRead(context))
        // Resolves the fetch, then revokes before Query's own continuation writes its data.
        void pending.then(() => queueMicrotask(() => privacy.changePolicy(scope, 'consent-revoked'))).catch(() => undefined)
        return pending
      },
    })
    const stop = observer.subscribe(() => undefined)
    disposals.push(() => observer.destroy(), stop)
    const result = await observer.refetch()
    expect(result.data).toBeUndefined()
    expect(observer.getCurrentResult().data).toBeUndefined()
    expect(client.getQueryData(options.queryKey)).toBeUndefined()
    expect(success).not.toHaveBeenCalled()
  })

  it('allows subscriber-driven normal read refetch after synchronous privacy scrubbing', async () => {
    const { client, privacy } = setup()
    const read = vi.fn(() => 'authorized-new-data')
    const observer = new QueryObserver(client, privacy.options({ resource: 'participant', read }))
    const stop = observer.subscribe(() => undefined)
    const off = privacy.subscribe(() => observer.setOptions(privacy.options({ resource: 'participant', read })))
    disposals.push(() => observer.destroy(), stop, off)
    await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe('authorized-new-data'))
    const before = observer.options.queryKey
    privacy.changePolicy(privacy.capture(), 'block')
    expect(observer.options.queryKey).not.toEqual(before)
    await vi.waitFor(() => expect(read).toHaveBeenCalledTimes(2))
    expect(client.getQueryData(before)).toBeUndefined()
  })
})

describe('policy/reveal generations and fail-closed gates', () => {
  it.each(['block', 'consent-revoked', 'consent-expired', 'handle-replaced', 'suspended', 'event-closed'] satisfies PolicyChange[])('rejects late reveals and clears all private representations on %s', async reason => {
    const { client, privacy } = setup()
    const pending = deferred<string>()
    let signal!: AbortSignal
    const previous = privacy.capture()
    const options = privacy.options({ resource: 'social-reveal', entityIds: [GRANT], read: context => { signal = context.signal; return pending.promise } })
    const result = rejection(client.fetchQuery(options))
    await vi.waitFor(() => expect(signal).toBeDefined())
    client.setQueryData(['unclassified-private-projection'], '@cached-handle')
    client.setQueryData(publicQueryKey(), { providers: [] })
    expect(privacy.changePolicy(previous, reason)).toBe(true)
    expect(privacy.capture().policyGeneration).toBe(previous.policyGeneration + 1)
    expect(signal.aborted).toBe(true)
    await result
    pending.resolve('@late-revoked-handle')
    await drain()
    expect(client.getQueryData(options.queryKey)).toBeUndefined()
    expect(client.getQueryData(['unclassified-private-projection'])).toBeUndefined()
    expect(client.getQueryData(publicQueryKey())).toEqual({ providers: [] })
    expect(privacy.isCurrent(previous)).toBe(false)
    expect(privacy.changePolicy(previous, 'revalidated')).toBe(false)
    if (reason === 'suspended' || reason === 'event-closed') {
      expect(() => privacy.options({ resource: 'participant', read: () => 'blocked' })).toThrow(PrivacyError)
      expect(await rejection(privacy.run(privacy.capture(), () => 'blocked'))).toMatchObject({ kind: 'unavailable' })
      expect(privacy.changePolicy(privacy.capture(), 'revalidated')).toBe(true)
    }
    const next = privacy.options({ resource: 'participant', read: () => 'fresh-server-authorized' })
    expect(await client.fetchQuery(next)).toBe('fresh-server-authorized')
  })

  it('accepts successive same-session revocations but never a stale reopening decision', () => {
    const { client, privacy } = setup()
    const streamScope = privacy.capture()
    expect(privacy.changePolicy(streamScope, 'block')).toBe(true)
    const options = privacy.options({ resource: 'participant', read: () => 'new-private-data' })
    client.setQueryData(options.queryKey, 'new-private-data')
    expect(privacy.changePolicy(streamScope, 'consent-revoked')).toBe(true)
    expect(client.getQueryData(options.queryKey)).toBeUndefined()
    expect(privacy.capture().policyGeneration).toBe(streamScope.policyGeneration + 2)
    expect(privacy.changePolicy(streamScope, 'revalidated')).toBe(false)
  })

  it('ignores a delayed policy message from the previous identity', () => {
    const { client, privacy, session } = setup()
    const old = privacy.capture()
    session.replace(1, { userId: OTHER, eventId: EVENT, accessToken: 'new-token' })
    const next = privacy.options({ resource: 'participant', read: () => 'B' })
    client.setQueryData(next.queryKey, 'private-B')
    expect(privacy.changePolicy(old, 'suspended')).toBe(false)
    expect(client.getQueryData(next.queryKey)).toBe('private-B')
    expect(privacy.capture().gate).toBe('open')
  })

  it('never automatically reveals on mount, focus/reconnect defaults or generation rebinding', () => {
    const { privacy } = setup()
    const options = privacy.options({ resource: 'social-reveal', entityIds: [GRANT], read: vi.fn() })
    expect(options).toMatchObject({ enabled: false, refetchOnMount: false, refetchOnWindowFocus: false,
      refetchOnReconnect: false, refetchInterval: false, retry: false, retryOnMount: false, gcTime: 0, staleTime: 0 })
  })
})

describe('mutation and draft ownership', () => {
  it('runs registered cleanup, resets mutation observers and drops late mutation success callbacks', async () => {
    const { client, privacy } = setup()
    const pending = deferred<string>()
    const scope = privacy.capture()
    let requestSignal!: AbortSignal
    let draft = 'private-draft-A'
    const success = vi.fn()
    const observer = new MutationObserver(client, {
      mutationKey: privateQueryKey({ ...scope, userId: USER }, 'participant'),
      mutationFn: (_input: { privateInput: string }) => privacy.run(scope, ({ signal }) => { requestSignal = signal; return pending.promise }),
      onSuccess: success, retry: false,
    })
    const stop = observer.subscribe(() => undefined)
    const unregister = privacy.registerCleanup(() => { draft = ''; observer.reset() })
    disposals.push(stop, unregister)
    const result = rejection(observer.mutate({ privateInput: 'mutation-input' }))
    await vi.waitFor(() => expect(requestSignal).toBeDefined())
    privacy.changePolicy(scope, 'consent-revoked')
    expect(requestSignal.aborted).toBe(true)
    expect(draft).toBe('')
    expect(observer.getCurrentResult()).toMatchObject({ status: 'idle', variables: undefined, data: undefined })
    expect(client.getMutationCache().getAll()).toHaveLength(0)
    expect(await result).toMatchObject({ kind: 'scope-expired' })
    pending.resolve('late-private-result')
    await drain()
    expect(success).not.toHaveBeenCalled()
    expect(client.getMutationCache().getAll()).toHaveLength(0)
  })

  it('does not claim raw mutation-cache removal cancels an unguarded mutation', async () => {
    const { client, privacy } = setup()
    const pending = deferred<string>()
    const success = vi.fn()
    const observer = new MutationObserver(client, { mutationFn: () => pending.promise, onSuccess: success, retry: false })
    const result = observer.mutate(undefined)
    privacy.changePolicy(privacy.capture(), 'block')
    pending.resolve('server-side-work-completed')
    expect(await result).toBe('server-side-work-completed')
    expect(success).toHaveBeenCalledTimes(1) // Feature must use run + cleanup, not only remove a cache row.
  })

  it('keeps scrubbing other active observers and inactive queries if one observer throws', async () => {
    const { client, privacy } = setup()
    const options = privacy.options({ resource: 'participant', read: () => 'private-result' })
    let armed = false
    const first = new QueryObserver(client, options)
    const second = new QueryObserver(client, options)
    const offFirst = first.subscribe(result => { if (armed && result.data === undefined) throw new Error('consumer failure') })
    const offSecond = second.subscribe(() => undefined)
    disposals.push(() => first.destroy(), () => second.destroy(), offFirst, offSecond)
    await vi.waitFor(() => expect(second.getCurrentResult().data).toBe('private-result'))
    client.setQueryData(['another-private-projection'], 'inactive-private')
    armed = true
    expect(() => privacy.changePolicy(privacy.capture(), 'block')).not.toThrow()
    expect(first.getCurrentResult().data).toBeUndefined()
    expect(second.getCurrentResult().data).toBeUndefined()
    expect(client.getQueryData(['another-private-projection'])).toBeUndefined()
  })

  it('isolates failing cleanup, purges data anyway and reports only a bounded failure count', () => {
    const { client, privacy } = setup()
    const events = vi.fn(); const second = vi.fn()
    privacy.registerCleanup(() => { throw new Error('private failure detail') })
    privacy.registerCleanup(second)
    privacy.subscribe(() => { throw new Error('private subscriber') })
    privacy.subscribe(events)
    const key = privacy.options({ resource: 'participant', read: () => 'secret' }).queryKey
    client.setQueryData(key, 'secret')
    privacy.changePolicy(privacy.capture(), 'block')
    expect(second).toHaveBeenCalledTimes(1)
    expect(client.getQueryData(key)).toBeUndefined()
    expect(events.mock.calls[0]![0]).toMatchObject({ cleanupFailures: 1 })
    expect(JSON.stringify(events.mock.calls)).not.toMatch(/private failure|private subscriber|memory-token|secret/)
  })
})

describe('memory lifetime, Query defaults and disposal', () => {
  it('excludes all private/unclassified queries and every mutation from dehydration', async () => {
    const { client, privacy } = setup()
    const key = privacy.options({ resource: 'participant', read: () => 'private-payload' }).queryKey
    client.setQueryData(key, 'private-payload')
    client.setQueryData(['unknown'], 'private-unknown')
    client.setQueryData(publicQueryKey(), { providers: ['EMAIL'] })
    const mutation = client.getMutationCache().build(client, { mutationFn: async () => 'private-mutation' })
    await mutation.execute(undefined)
    const saved = dehydrate(client, publicOnlyDehydrateOptions)
    expect(saved.queries.map(query => query.queryKey)).toEqual([publicQueryKey()])
    expect(saved.mutations).toHaveLength(0)
    expect(JSON.stringify(saved)).not.toMatch(/private-payload|private-unknown|private-mutation/)
  })

  it('overrides client-wide placeholders, initial data, persisters and read retries', async () => {
    const { client, privacy } = setup()
    const persister = vi.fn(() => 'private-disk-cache')
    client.setDefaultOptions({ queries: { initialData: 'old-private-default', placeholderData: 'old-private-placeholder',
      persister, retry: 3, gcTime: Infinity } })
    const read = vi.fn(() => { throw new Error('bounded adapter failure') })
    const observer = new QueryObserver(client, privacy.options({ resource: 'participant', read }))
    const stop = observer.subscribe(() => undefined)
    disposals.push(() => observer.destroy(), stop)
    expect(observer.getCurrentResult().data).toBeUndefined()
    await vi.waitFor(() => expect(observer.getCurrentResult().isError).toBe(true))
    expect(read).toHaveBeenCalledTimes(1)
    expect(persister).not.toHaveBeenCalled()
    expect(observer.getCurrentResult().data).toBeUndefined()
  })

  it('does not touch browser persistence and does not retain credentials in scopes or events', async () => {
    const storage = { getItem: vi.fn(), setItem: vi.fn(), removeItem: vi.fn() }
    vi.stubGlobal('localStorage', storage); vi.stubGlobal('sessionStorage', storage)
    const { client, privacy, session } = setup()
    const listener = vi.fn(); privacy.subscribe(listener)
    await client.fetchQuery(privacy.options({ resource: 'participant', read: () => '@private-result' }))
    session.replace(1, null)
    expect(JSON.stringify(privacy.capture())).not.toContain('memory-token')
    expect(JSON.stringify(listener.mock.calls)).not.toMatch(/memory-token|@private-result/)
    expect(storage.getItem).not.toHaveBeenCalled(); expect(storage.setItem).not.toHaveBeenCalled(); expect(storage.removeItem).not.toHaveBeenCalled()
  })

  it('cancels a reader promptly even if its work ignores its signal', async () => {
    const { privacy } = setup()
    const caller = new AbortController()
    const result = rejection(privacy.run(privacy.capture(), () => new Promise<string>(() => undefined), caller.signal))
    caller.abort()
    expect(await result).toMatchObject({ kind: 'cancelled' })
  })

  it('does not start pre-cancelled work and detaches per-run abort listeners', async () => {
    const { privacy, session } = setup()
    const signal = session.read().signal
    const add = vi.spyOn(signal, 'addEventListener'); const remove = vi.spyOn(signal, 'removeEventListener')
    const read = vi.fn(() => 'done')
    const caller = new AbortController(); caller.abort()
    expect(await rejection(privacy.run(privacy.capture(), read, caller.signal))).toMatchObject({ kind: 'cancelled' })
    expect(read).not.toHaveBeenCalled()
    expect(await privacy.run(privacy.capture(), read)).toBe('done')
    expect(add.mock.calls.filter(([type]) => type === 'abort')).toHaveLength(2)
    expect(remove.mock.calls.filter(([type]) => type === 'abort')).toHaveLength(2)
  })

  it('unsubscribes listeners/cleanup and disposes idempotently without affecting a later shell', async () => {
    const { client, privacy, session } = setup()
    const listener = vi.fn(); const cleanup = vi.fn()
    const off = privacy.subscribe(listener); const unregister = privacy.registerCleanup(cleanup)
    off(); unregister()
    const scope = privacy.capture()
    const result = rejection(privacy.run(scope, () => new Promise(() => undefined)))
    privacy.dispose(); privacy.dispose()
    expect(await result).toMatchObject({ kind: 'disposed' })
    expect(listener).not.toHaveBeenCalled(); expect(cleanup).not.toHaveBeenCalled()
    expect(privacy.changePolicy(scope, 'revalidated')).toBe(false)
    expect(() => privacy.subscribe(listener)).toThrow(PrivacyError)
    expect(() => privacy.registerCleanup(cleanup)).toThrow(PrivacyError)
    client.setQueryData(['later-shell'], 'retain')
    session.replace(1, { userId: OTHER, eventId: NEXT_EVENT, accessToken: 'new-shell-token' })
    expect(client.getQueryData(['later-shell'])).toBe('retain')
  })

  it('cannot produce private options or execute private work while anonymous', async () => {
    const { privacy, session } = setup()
    session.replace(1, null)
    expect(() => privacy.options({ resource: 'participant', read: () => 'private' })).toThrow(PrivacyError)
    expect(await rejection(privacy.run(privacy.capture(), () => 'private'))).toMatchObject({ kind: 'unavailable' })
  })
})
