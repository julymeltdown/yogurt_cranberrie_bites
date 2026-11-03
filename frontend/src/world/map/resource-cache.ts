import { MapError, requireMap } from './json'
import { abortable } from './resources'

interface Entry { readonly promise: Promise<Uint8Array<ArrayBuffer>>; bytes?: Uint8Array<ArrayBuffer> }
export interface ResourceByteCache {
  /** The reader must recheck its own allowlist/authority before every call. Loader returns verified public bytes only. */
  read(key: string, signal: AbortSignal, load: (signal: AbortSignal) => Promise<Uint8Array<ArrayBuffer>>): Promise<Uint8Array<ArrayBuffer>>
  dispose(): void
  diagnostics(): Readonly<{ loads: number; hits: number; coalesced: number; residentBytes: number; entries: number; disposed: boolean }>
}
/** One authorized release owner, never a global/private-response cache. Consumer aborts are independent. */
export function createResourceByteCache(options: { signal?: AbortSignal; maxBytes?: number; maxEntries?: number } = {}): ResourceByteCache {
  const limit = options.maxBytes ?? 8_388_608, count = options.maxEntries ?? 32
  requireMap(Number.isSafeInteger(limit) && limit > 0 && limit <= 33_554_432 && Number.isSafeInteger(count) && count > 0 && count <= 64)
  const owner = new AbortController(), entries = new Map<string, Entry>()
  let closed = false, residentBytes = 0, loads = 0, hits = 0, coalesced = 0
  function remove(key: string, entry: Entry) {
    if (entries.get(key) !== entry) return
    entries.delete(key); residentBytes -= entry.bytes?.byteLength ?? 0
  }
  function trim() {
    for (const [key, entry] of entries) {
      if (residentBytes <= limit && entries.size <= count) break
      if (entry.bytes) remove(key, entry)
    }
  }
  function dispose() {
    if (closed) return
    closed = true; options.signal?.removeEventListener('abort', dispose); owner.abort(); entries.clear(); residentBytes = 0
  }
  options.signal?.addEventListener('abort', dispose, { once: true })
  if (options.signal?.aborted) dispose()
  return {
    async read(key, signal, load) {
      if (closed || signal.aborted) throw new MapError('cancelled')
      let entry = entries.get(key)
      if (entry) {
        if (entry.bytes) hits++; else coalesced++
        entries.delete(key); entries.set(key, entry)
      } else {
        trim()
        if (entries.size >= count) {
          const evict = Array.from(entries).find(([, value]) => value.bytes)
          if (evict) remove(...evict); else throw new MapError('unavailable')
        }
        loads++
        const created: Entry = { promise: Promise.resolve().then(() => {
          if (closed) throw new MapError('cancelled')
          return load(owner.signal)
        }).then(bytes => {
          if (closed) throw new MapError('cancelled')
          requireMap(bytes.byteLength > 0)
          const retained = bytes.slice()
          if (entries.get(key) === created && retained.byteLength <= limit) {
            created.bytes = retained; residentBytes += retained.byteLength; trim()
          } else remove(key, created)
          return retained
        }).catch(error => { remove(key, created); throw error }) }
        entry = created; entries.set(key, entry)
      }
      const bytes = await abortable(entry.promise, signal)
      if (closed || signal.aborted) throw new MapError('cancelled')
      // A decoder/caller cannot mutate the resident copy used by a different screen.
      return bytes.slice()
    },
    dispose,
    diagnostics: () => Object.freeze({ loads, hits, coalesced, residentBytes, entries: entries.size, disposed: closed }),
  }
}
