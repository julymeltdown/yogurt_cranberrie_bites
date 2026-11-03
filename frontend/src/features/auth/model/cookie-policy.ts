import { TransportError } from '../../../shared/api/errors'

export interface CookiePolicy {
  readonly available: boolean
  onInvalidate(listener: () => void): () => void
  run<T>(kind: 'login' | 'logout' | 'refresh', work: () => Promise<T>): Promise<T>
  dispose(): void
}
/** No credentials, identities or URLs are persisted or sent across tabs.
 * Every cookie writer holds the same origin Web Lock. Peers discard memory access
 * before a writer dispatches. Unknown outcomes latch the live cohort closed.
 * This does NOT qualify automatic cookie restoration/rotation in production.
 */
export function createBrowserCookiePolicy(): CookiePolicy {
  const locks = typeof navigator === 'undefined' ? undefined : navigator.locks
  let channel: BroadcastChannel | null = null
  try { if (typeof BroadcastChannel !== 'undefined') channel = new BroadcastChannel('mcr-auth-cookie-v1') } catch { /* Unavailable browser capability: fail closed. */ }
  const listeners = new Set<() => void>()
  let closed = false, uncertain = false, active = 0
  const invalidate = () => { const snapshot = [...listeners]; snapshot.forEach(listener => listener()) }
  const send = (type: 'revoke' | 'uncertain' | 'hello') => { if (!closed) channel?.postMessage({ version: 1, type }) }
  if (channel) {
    channel.onmessage = event => {
      const value: unknown = event.data
      if (!value || typeof value !== 'object' || Array.isArray(value)) return
      const data = value as Record<string, unknown>
      if (Object.keys(data).length !== 2 || data.version !== 1) return
      if (data.type === 'hello' && uncertain) send('uncertain')
      if (data.type === 'uncertain') { uncertain = true; invalidate() }
      else if (data.type === 'revoke') invalidate()
    }
    send('hello')
  }
  return {
    get available() { return Boolean(locks && channel && !closed && !uncertain) },
    onInvalidate(listener) { listeners.add(listener); return () => { listeners.delete(listener) } },
    async run(kind, work) {
      if (closed || !locks || !channel || (uncertain && kind !== 'logout')) throw new TransportError('invalid-request')
      const waiting = new AbortController(), timeout = setTimeout(() => waiting.abort(), 5000)
      try {
        return await locks.request('mcr-auth-cookie-v1', { mode: 'exclusive', signal: waiting.signal }, async () => {
          clearTimeout(timeout)
          if (closed || (uncertain && kind !== 'logout')) throw new TransportError('invalid-request')
          send('revoke'); ++active
          try { return await work() }
          catch (error) {
            if (error instanceof TransportError && error.uncertain) { uncertain = true; send('uncertain') }
            throw error
          } finally { --active }
        })
      } catch (error) {
        if (error instanceof TransportError) throw error
        throw new TransportError('aborted')
      } finally { clearTimeout(timeout) }
    },
    dispose() { if (active) send('uncertain'); closed = true; channel?.close(); listeners.clear() },
  }
}
