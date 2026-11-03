import type { SessionController } from '../../features/auth/model/session'
import { uuid } from '../../shared/media/contract'
import { MAX_IMAGE_BYTES } from '../../shared/media/image-policy'
import { createMediaPreviewController, type MediaPreviewState } from '../../shared/media/read-media'
import type { PreviewPort } from '../../shared/media/preview'
import { PrivacyError, type PrivacyScope } from '../../shared/query/privacy'

export type MediaOwnerAuth = Pick<SessionController, 'session' | 'privacy' | 'request'>
export type RetainedMediaPurpose = 'POST' | 'GIVEAWAY'
export interface MediaLease {
  read(): MediaPreviewState
  subscribe(listener: () => void): () => void
  load(): Promise<void>
  cancel(): void
  /** Terminal and idempotent; equivalent to dispose for component lifetimes. */
  release(): void
  dispose(): void
}
export interface MediaOwnerOptions {
  readonly auth: MediaOwnerAuth
  readonly scope: PrivacyScope
  /** All entries, including active/in-flight reads, count toward this cap (1–32). */
  readonly maxEntries?: number
  /** Owner-held consumer registrations, including not-yet-loaded facades (1–256). */
  readonly maxConsumers?: number
  readonly previews?: PreviewPort
}
type Reader = ReturnType<typeof createMediaPreviewController>
interface Consumer { update(state: MediaPreviewState): void; stop(): void }
interface Entry {
  readonly key: string
  readonly reader: Reader
  readonly consumers: Set<Consumer>
  removeListener(): void
  flight: Promise<void> | null
  used: number
}
const empty = (phase: MediaPreviewState['phase'], error: string | null = null): MediaPreviewState =>
  Object.freeze({ phase, error, scope: null, media: null, previewUrl: null })
function limit(value: number, upper: number) {
  if (!Number.isSafeInteger(value) || value < 1 || value > upper) throw new RangeError('Invalid media owner limit')
  return value
}

/** A private, scope-bound read owner; root must supply IDs from accessible parent rows.
 * acquire is inert. Only a consumer's explicit load may request protected media.
 * This adapter does not establish parent authorization or shell readiness. */
export function createMediaOwner(options: MediaOwnerOptions) {
  const { auth } = options, scope = Object.freeze({ ...options.scope })
  const maxEntries = limit(options.maxEntries ?? 8, 32), maxConsumers = limit(options.maxConsumers ?? 64, 256)
  if (!auth.privacy.isCurrent(scope) || !uuid(scope.userId) || !uuid(scope.eventId)) throw new PrivacyError('scope-expired')
  const scopeKey = JSON.stringify([scope.generation, scope.userId, scope.eventId, scope.policyGeneration, scope.gate])
  const entries = new Map<string, Entry>(), consumers = new Set<Consumer>()
  let disposed = false, tick = 0
  const current = () => !disposed && auth.privacy.isCurrent(scope)
  function evict(entry: Entry) {
    entries.delete(entry.key)
    entry.removeListener()
    entry.reader.dispose()
  }
  function dispose() {
    if (disposed) return
    disposed = true
    unregister()
    // Revoke every URL and abort every reader before notifying consumer observers.
    for (const entry of [...entries.values()]) evict(entry)
    for (const consumer of [...consumers]) consumer.stop()
    consumers.clear()
  }
  const unregister = auth.privacy.registerCleanup(dispose)
  function getEntry(mediaId: string, purpose: RetainedMediaPurpose): Entry | null {
    const key = JSON.stringify([scopeKey, purpose, mediaId])
    const cached = entries.get(key)
    if (cached) { cached.used = ++tick; return cached }
    if (entries.size >= maxEntries) {
      const oldest = [...entries.values()].filter(entry => entry.consumers.size === 0).sort((a, b) => a.used - b.used)[0]
      if (!oldest) return null // Never evict a preview still held by an active consumer.
      evict(oldest)
    }
    const reader = createMediaPreviewController({ session: auth.session, mediaId, purpose,
      ...(options.previews ? { previews: options.previews } : {}),
      request: input => auth.privacy.run(scope, ({ signal }) => auth.request({ ...input, signal }), input.signal),
    })
    const entry: Entry = { key, reader, consumers: new Set(), removeListener: () => undefined, flight: null, used: ++tick }
    entry.removeListener = reader.subscribe(() => {
      if (!current()) return
      for (const consumer of [...entry.consumers]) consumer.update(reader.read())
    })
    entries.set(key, entry)
    return entry
  }
  return {
    scope,
    acquire({ mediaId, purpose, signal }: { readonly mediaId: string; readonly purpose: RetainedMediaPurpose; readonly signal?: AbortSignal }): MediaLease {
      if (!current()) throw new PrivacyError('scope-expired')
      if (!uuid(mediaId) || (purpose !== 'POST' && purpose !== 'GIVEAWAY')) throw new Error('Invalid media lease')
      if (consumers.size >= maxConsumers) throw new RangeError('Media owner consumer limit reached')
      let state = empty('idle'), attached: Entry | null = null, released = false
      let resolveStopped: () => void = () => undefined
      let stopped = new Promise<void>(resolve => { resolveStopped = resolve })
      const listeners = new Set<() => void>()
      const notify = () => { for (const listener of [...listeners]) { try { listener() } catch { /* Isolate observers during private cleanup. */ } } }
      function detach() {
        const entry = attached; attached = null
        if (!entry) return
        entry.consumers.delete(consumer)
        if (entries.get(entry.key) !== entry || entry.consumers.size) return
        if (current() && entry.reader.read().phase === 'ready') entry.used = ++tick
        else evict(entry)
      }
      function cancel() {
        resolveStopped()
        stopped = new Promise<void>(resolve => { resolveStopped = resolve })
        detach(); state = empty('cancelled'); notify()
      }
      function release() {
        if (released) return
        released = true
        signal?.removeEventListener('abort', release)
        consumers.delete(consumer)
        cancel()
        listeners.clear()
      }
      const consumer: Consumer = {
        update(next) { if (!released && current() && state !== next) { state = next; notify() } },
        stop: release,
      }
      consumers.add(consumer)
      signal?.addEventListener('abort', release, { once: true })
      if (signal?.aborted) release()
      return {
        read: () => state,
        subscribe(listener) { if (released) return () => undefined; listeners.add(listener); return () => { listeners.delete(listener) } },
        async load() {
          if (released || !current() || signal?.aborted) return
          const cancellation = stopped
          if (!attached) {
            const entry = getEntry(mediaId, purpose)
            if (!entry) { state = empty('failed', 'media-capacity'); notify(); return }
            attached = entry
            entry.consumers.add(consumer)
          }
          const entry = attached
          entry.used = ++tick
          if (!entry.flight && entry.reader.read().phase !== 'ready') {
            // Install the flight before reader notifications can re-enter load.
            const flight = Promise.resolve().then(() => entry.reader.load())
            entry.flight = flight
            const finish = () => { if (entry.flight === flight) entry.flight = null }
            void flight.then(finish, finish)
          }
          consumer.update(entry.reader.read())
          if (entry.flight) await Promise.race([entry.flight, cancellation])
        },
        cancel,
        release,
        dispose: release,
      }
    },
    /** Counts and worst-case private Blob ceiling, not measured JS/decoder/GPU memory. */
    inspect: () => Object.freeze({ disposed, entries: entries.size, consumers: consumers.size,
      activeEntries: [...entries.values()].filter(entry => entry.consumers.size > 0).length,
      maxEntries, maxConsumers, maxResidentBlobBytes: maxEntries * MAX_IMAGE_BYTES }),
    dispose,
  }
}
export type MediaOwner = ReturnType<typeof createMediaOwner>
