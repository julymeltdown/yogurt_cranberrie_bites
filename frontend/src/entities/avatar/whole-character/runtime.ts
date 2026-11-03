import type { SessionController } from '../../../features/auth/model/session'
import type { PrivacyScope } from '../../../shared/query/privacy'
import { abortable, decodeImage, pngDimensions, type DecodedImage } from '../../../world/map/resources'
import { sha256, strictJson } from '../../../world/map/json'
import { validateSelectedStaticRelease, type SelectedStaticRelease } from '../selected-static/release'
import { ensure, REVIEWED, validateWholeCharacterConfig, validateWholeCharacterManifest, WholeCharacterError,
  type WholeCharacterChoice, type WholeCharacterConfig, type WholeCharacterManifest, type WholeCharacterPin } from './contract'

export type WholeCharacterPrivacy = Pick<SessionController['privacy'], 'capture' | 'isCurrent' | 'registerCleanup' | 'run'>
export interface WholeCharacterRender {
  readonly config: WholeCharacterConfig
  readonly pose: 'front_idle'
  readonly image: HTMLImageElement
  readonly source: Readonly<{ x: number; y: 0; width: number; height: number }>
  readonly anchor: Readonly<{ x: 32; y: 60 }>
  readonly logicalCell: Readonly<{ width: 64; height: 64 }>
}
export interface WholeCharacterLease {
  /** Null immediately after caller cancellation, owner disposal or canonical scope loss. */
  read(): WholeCharacterRender | null
  subscribe(listener: () => void): () => void
  dispose(): void
}
export interface WholeCharacterRuntimeOptions {
  readonly privacy: WholeCharacterPrivacy
  readonly manifest: unknown
  readonly pin: WholeCharacterPin
  readonly origin: string
  readonly fetch?: typeof fetch
  readonly decode?: typeof decodeImage
  readonly timeoutMs?: number
}
interface Approved { readonly manifest: WholeCharacterManifest; readonly release: SelectedStaticRelease }
interface Flight<T> { controller: AbortController; promise: Promise<T>; waiters: number; ready: boolean; value?: T }
interface Slot<T> { flight?: Flight<T> | undefined; cleanup(value: T): void }
const cancelled = () => new WholeCharacterError('cancelled')
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
/** One explicit canonical privacy owner. No module/global identity or image cache. */
export function createWholeCharacterRuntime(options: WholeCharacterRuntimeOptions) {
  const { privacy } = options, scope: PrivacyScope = Object.freeze({ ...privacy.capture() })
  const origin = new URL(options.origin).origin, timeout = options.timeoutMs ?? 15000
  ensure(origin === options.origin && /^https?:$/.test(new URL(origin).protocol) && Number.isSafeInteger(timeout) && timeout >= 1 && timeout <= 15000)
  ensure(scope.gate === 'open' && typeof scope.userId === 'string' && uuid.test(scope.userId) && typeof scope.eventId === 'string' && uuid.test(scope.eventId) && privacy.isCurrent(scope))
  const owner = new AbortController(), fetcher = options.fetch ?? globalThis.fetch.bind(globalThis)
  const pin = Object.freeze({ ...options.pin }), leases = new Set<WholeCharacterLease>()
  const metadata: Slot<Approved> = { cleanup() {} }, compact: Slot<DecodedImage> = { cleanup: decoded => decoded.dispose() }, detail: Slot<DecodedImage> = { cleanup: decoded => decoded.dispose() }
  let disposed = false, pending = 0, cleanupFailures = 0, removeCleanup = () => {}
  const current = () => !disposed && !owner.signal.aborted && privacy.isCurrent(scope)
  function assertCurrent(signal?: AbortSignal) { if (!current() || signal?.aborted) throw cancelled() }
  function cleanup<T>(slot: Slot<T>, value: T) { try { slot.cleanup(value) } catch { cleanupFailures++ } }
  function clear<T>(slot: Slot<T>) { const f = slot.flight; slot.flight = undefined; f?.controller.abort(); if (f?.ready && f.value) cleanup(slot, f.value) }
  function dispose() {
    if (disposed) return
    disposed = true; owner.abort()
    for (const lease of Array.from(leases)) lease.dispose()
    clear(compact); clear(detail); clear(metadata); removeCleanup()
  }
  removeCleanup = privacy.registerCleanup(dispose)
  function shared<T>(slot: Slot<T>, signal: AbortSignal, load: (signal: AbortSignal) => Promise<T>): Promise<T> {
    assertCurrent(signal)
    let flight = slot.flight
    if (!flight) {
      const f: Flight<T> = { controller: new AbortController(), waiters: 0, ready: false, promise: undefined as unknown as Promise<T> }
      slot.flight = f; flight = f
      const combined = AbortSignal.any([owner.signal, f.controller.signal])
      f.promise = Promise.resolve().then(() => { assertCurrent(combined); return load(combined) }).then(value => {
        if (!current() || combined.aborted || slot.flight !== f) { cleanup(slot, value); throw cancelled() }
        f.value = value; f.ready = true; return value
      }).catch(error => { if (slot.flight === f) slot.flight = undefined; throw error })
    }
    const f = flight; f.waiters++
    return abortable(f.promise, signal, timeout).then(value => { assertCurrent(signal); return value }).finally(() => {
      f.waiters--
      if (!f.waiters && !f.ready) { if (slot.flight === f) slot.flight = undefined; f.controller.abort() }
    })
  }
  async function bytes(resource: { path: string; sha256: string; bytes: number }, kind: 'json' | 'png', signal: AbortSignal) {
    assertCurrent(signal)
    const controller = new AbortController(), combined = AbortSignal.any([signal, controller.signal]), url = `${origin}/${resource.path}`
    const work = async () => {
      const response = await fetcher(url, { method: 'GET', credentials: 'omit', mode: 'same-origin', cache: 'force-cache', redirect: 'error', referrerPolicy: 'no-referrer', signal: combined })
      assertCurrent(combined)
      ensure(response.ok && !response.redirected && response.url === url && response.body
        && response.headers.get('content-type')?.split(';')[0]?.trim() === (kind === 'png' ? 'image/png' : 'application/json'))
      const length = response.headers.get('content-length'); ensure(length === null || /^\d+$/.test(length) && Number(length) === resource.bytes)
      const reader = response.body.getReader(), chunks: Uint8Array[] = []; let size = 0
      try {
        while (true) { const chunk = await reader.read(); if (chunk.done) break; assertCurrent(combined); size += chunk.value.byteLength; ensure(size <= resource.bytes); chunks.push(chunk.value) }
      } finally { void reader.cancel().catch(() => {}); reader.releaseLock() }
      ensure(size === resource.bytes)
      const result = new Uint8Array(size); let offset = 0
      for (const chunk of chunks) { result.set(chunk, offset); offset += chunk.length }
      ensure(await sha256(result) === resource.sha256); assertCurrent(combined); return result
    }
    try { return await abortable(work(), combined, timeout) } finally { controller.abort() }
  }
  function approved(signal: AbortSignal) { return shared(metadata, signal, async signal => {
    const manifest = await validateWholeCharacterManifest(options.manifest, pin); assertCurrent(signal)
    const data = await bytes(REVIEWED.descriptor, 'json', signal)
    const release = validateSelectedStaticRelease(strictJson(new TextDecoder('utf-8', { fatal: true }).decode(data)))
    ensure(release.atlas.path === manifest.atlas.texture.path && release.atlas.sha256 === manifest.atlas.texture.sha256)
    manifest.presets.forEach((p, i) => ensure(release.characters[i]?.id === p.art_id && release.characters[i]?.frame.x === p.frame.x))
    return Object.freeze({ manifest, release })
  }) }
  function atlas(size: 64 | 256, signal: AbortSignal) { return shared(size === 64 ? compact : detail, signal, async signal => {
    await approved(signal)
    const resource = size === 64 ? REVIEWED.texture : REVIEWED.detail, data = await bytes(resource, 'png', signal)
    pngDimensions(data, resource.width, resource.height); assertCurrent(signal)
    const operation = (options.decode ?? decodeImage)(data, signal).then(decoded => {
      if (!current() || signal.aborted) { cleanup(size === 64 ? compact : detail, decoded); throw cancelled() }
      try { ensure(decoded.image.naturalWidth === resource.width && decoded.image.naturalHeight === resource.height); return decoded }
      catch (error) { cleanup(size === 64 ? compact : detail, decoded); throw error }
    })
    return abortable(operation, signal, timeout)
  }) }
  async function acquire(input: unknown, options: { readonly signal: AbortSignal; readonly detail?: boolean }): Promise<WholeCharacterLease> {
      assertCurrent(options.signal)
      const config = validateWholeCharacterConfig(input)
      if (pending + leases.size >= 256) throw new WholeCharacterError('capacity')
      pending++
      try {
        return await privacy.run(scope, async ({ signal }) => {
          const size = options.detail ? 256 : 64, decoded = await atlas(size, signal); assertCurrent(signal)
          const index = Number(config.preset_id.slice(-2)) - 1
          const render: WholeCharacterRender = Object.freeze({ config, pose: 'front_idle', image: decoded.image,
            source: Object.freeze({ x: index * size, y: 0 as const, width: size, height: size }),
            anchor: Object.freeze({ x: 32 as const, y: 60 as const }), logicalCell: Object.freeze({ width: 64 as const, height: 64 as const }) })
          let closed = false; const listeners = new Set<() => void>()
          const lease: WholeCharacterLease = {
            read() { return !closed && current() && !options.signal.aborted ? render : null },
            subscribe(listener) { if (closed || !current() || options.signal.aborted) { listener(); return () => {} } listeners.add(listener); return () => { listeners.delete(listener) } },
            dispose() {
              if (closed) return
              closed = true; options.signal.removeEventListener('abort', lease.dispose); leases.delete(lease)
              for (const listener of Array.from(listeners)) { try { listener() } catch { /* Every canvas/sink must still receive revocation. */ } }
              listeners.clear()
            },
          }
          options.signal.addEventListener('abort', lease.dispose, { once: true }); leases.add(lease)
          if (!current() || options.signal.aborted) { lease.dispose(); throw cancelled() }
          return lease
        }, options.signal)
      } finally { pending-- }
  }
  return {
    scope, isCurrent: current, acquire,
    validateConfig(input: unknown) { assertCurrent(); return validateWholeCharacterConfig(input) },
    choices(signal: AbortSignal): Promise<readonly WholeCharacterChoice[]> {
      assertCurrent(signal)
      return privacy.run(scope, async ({ signal }) => {
        const { manifest, release } = await approved(signal); assertCurrent(signal)
        return Object.freeze(manifest.presets.map((p, i) => Object.freeze({ id: p.preset_id, label: release.characters[i]!.label, config: p.config })))
      }, signal)
    },
    dispose,
    /** Bounded resource counts only; never exposes another identity/config or mutable caches. */
    diagnostics: () => Object.freeze({ disposed, leases: leases.size, pending, cleanupFailures, decodedAtlases: Number(!!compact.flight?.ready) + Number(!!detail.flight?.ready) }),
  }
}
export type WholeCharacterRuntime = ReturnType<typeof createWholeCharacterRuntime>
