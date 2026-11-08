import type { PrivacyScope } from '../../shared/query/privacy'
import { abortable } from '../../world/map/resources'
import { MOTION_LAYERS, loadMotionArtwork, requireMotion, validateMotionSelection, type MotionArt, type MotionSelection } from './art'
import { createMotionCompositor, type MotionComposition, type MotionCompositor, type MotionPart } from './compositor'

/**
 * WP-08 T-704: the profile avatar runtime.
 *
 * Carried over from the concert editor: one scoped owner of decoded bytes, the
 * composition cache, leases and scene time; shared atlases; bounded cleanup.
 *
 * Re-keyed: the owner is the *account*, not an event. The concert runtime
 * required `scope.eventId` and refused to run without it; a profile avatar has
 * no event, so the gate is the profile scope alone. A runtime that is still
 * given an event identity is not rejected — it simply does not depend on it —
 * but the scope's account must be present, and the runtime never reads or
 * forwards an event id.
 */
export interface MotionAuthorityOperations<C> {
  readonly manifestId: string; readonly manifestVersion: number; readonly manifestHash: string
  decodeConfig(value: unknown): C
  selection(config: C): MotionSelection
  presets(art: MotionArt): readonly { readonly id: string; readonly label: string; readonly config: C }[]
  replace(config: C, selection: MotionSelection): C
}
/**
 * A runtime authority. `TEST_ONLY` is the fixture qualification; a
 * `REVIEWED_PRODUCTION` authority is the same operations plus the production
 * lifetime (see `reviewed-production.ts`). Only the production variant can
 * carry `canSave`, so the editor's save gate cannot be satisfied by a fixture.
 */
export type MotionAuthority<C> = MotionAuthorityOperations<C> & (
  | { readonly qualification: 'TEST_ONLY' }
  | { readonly qualification: 'REVIEWED_PRODUCTION'; readonly canSave: boolean }
)
export interface MotionLease<C> {
  read(): Readonly<{ config: C; selection: MotionSelection; canvas: HTMLCanvasElement }> | null
  subscribe(listener: () => void): () => void
  dispose(): void
}
export type { MotionPart } from './compositor'
export interface MotionPartLease {
  read(): Readonly<MotionPart & { canvas: HTMLCanvasElement; representation: 'isolated-part'; direction: 'down'; phase: 0; composition: 'raw-item-cell' }> | null
  subscribe(listener: () => void): () => void
  dispose(): void
}

/** The profile scope a runtime is bound to. `userId` is the account; there is no event. */
export type ProfileAvatarScope = Pick<PrivacyScope, 'userId' | 'generation' | 'policyGeneration' | 'gate'>

/** Sole per-account owner of decoded bytes, composition cache, leases and scene time. */
export function createMotionRuntime<C>(options: {
  readonly authority: MotionAuthority<C>; readonly scope: ProfileAvatarScope
  readonly privacy: { isCurrent(scope: PrivacyScope): boolean; registerCleanup(cleanup: () => void): () => void }
  readonly origin: string; readonly fetch?: typeof fetch
  readonly load?: typeof loadMotionArtwork
}) {
  const scope = Object.freeze({ ...options.scope }), authority = options.authority
  // The avatar is profile-owned: the account must be present and the gate open.
  // No event is required, read, or forwarded.
  requireMotion(!!scope.userId && scope.gate === 'open'
    && authority.manifestId.length > 0 && Number.isSafeInteger(authority.manifestVersion) && authority.manifestVersion > 0
    && /^sha256:[a-f0-9]{64}$/.test(authority.manifestHash))
  const controller = new AbortController(), leases = new Set<() => void>(), clocks = new Set<(time: number) => void>()
  const partPreviews = new Set<{ clear(): void; notify(): void }>()
  let disposed = false, flight: Promise<{ art: MotionArt; image: HTMLImageElement; dispose(): void }> | null = null
  let loaded: Awaited<ReturnType<typeof loadMotionArtwork>> | null = null, compositor: MotionCompositor | null = null, time = 0
  const current = () => !disposed && !controller.signal.aborted && options.privacy.isCurrent(scope as PrivacyScope)
  const assertCurrent = (signal?: AbortSignal) => requireMotion(current() && !signal?.aborted)
  function dispose() {
    if (disposed) return
    disposed = true
    // Clear every isolated canvas and retained reference before any withdrawal observer.
    const closingParts = Array.from(partPreviews)
    for (const part of closingParts) part.clear()
    controller.abort(); unregister(); clocks.clear()
    for (const part of closingParts) part.notify()
    for (const close of Array.from(leases)) close()
    compositor?.dispose(); compositor = null; loaded?.dispose(); loaded = null; flight = null
  }
  let unregister = () => {}
  unregister = options.privacy.registerCleanup(dispose)
  if (!current()) dispose()
  async function ready(signal: AbortSignal) {
    assertCurrent(signal)
    if (!flight) {
      const next = (options.load ?? loadMotionArtwork)({ origin: options.origin, signal: controller.signal,
        ...(options.fetch ? { fetch: options.fetch } : {}) }).then(value => {
          if (!current()) { value.dispose(); throw new Error('Motion owner expired') }
          loaded = value; compositor = createMotionCompositor({ art: value.art, image: value.image, isCurrent: current }); return value
        }).catch(error => { if (flight === next) flight = null; throw error })
      flight = next
    }
    const value = await abortable(flight, signal); assertCurrent(signal); return value
  }
  function decode(input: unknown) { assertCurrent(); return authority.decodeConfig(input) }
  function selection(config: C, art: MotionArt) { return validateMotionSelection(authority.selection(config), art) }
  async function acquire(input: unknown, request: { signal: AbortSignal }): Promise<MotionLease<C>> {
    const config = decode(input), { art } = await ready(request.signal), selected = selection(config, art)
    assertCurrent(request.signal)
    let acquired: MotionComposition | null = compositor!.acquire(selected), closed = false
    const listeners = new Set<() => void>(), canvas = acquired.canvas
    function close() {
      if (closed) return
      closed = true; leases.delete(close); request.signal.removeEventListener('abort', close); acquired?.release(); acquired = null
      for (const listener of Array.from(listeners)) { try { listener() } catch { /* Complete independent withdrawal. */ } }
      listeners.clear()
    }
    leases.add(close); request.signal.addEventListener('abort', close, { once: true })
    if (!current() || request.signal.aborted) close()
    return { read: () => !closed && current() ? Object.freeze({ config, selection: selected, canvas }) : null,
      subscribe(listener) { if (closed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } }, dispose: close }
  }
  async function acquirePartPreview(input: MotionPart, request: { signal: AbortSignal }): Promise<MotionPartLease> {
    assertCurrent(request.signal)
    requireMotion(input && MOTION_LAYERS.includes(input.kind) && typeof input.key === 'string'
      && Object.keys(input).sort().join(',') === 'key,kind' && partPreviews.size < 4 && flight)
    const part = Object.freeze({ kind: input.kind, key: input.key }), pending = flight
    let acquired: ReturnType<MotionCompositor['acquirePartPreview']> | null = null, closed = false
    const listeners = new Set<() => void>()
    const reservation = { clear, notify }
    function clear() {
      if (closed) return
      closed = true; partPreviews.delete(reservation); request.signal.removeEventListener('abort', close)
      acquired?.release(); acquired = null
    }
    function notify() {
      const pendingListeners = Array.from(listeners)
      listeners.clear()
      for (const listener of pendingListeners) { try { listener() } catch { /* Independent withdrawal. */ } }
    }
    function close() { if (closed) return; clear(); notify() }
    partPreviews.add(reservation); request.signal.addEventListener('abort', close, { once: true })
    try {
      // Borrow an existing owner flight. A preview never starts an atlas load.
      const { art } = await new Promise<Awaited<ReturnType<typeof loadMotionArtwork>>>((resolve, reject) => {
        // Borrow the flight without adding the deadline timer used by ready().
        function detach() { request.signal.removeEventListener('abort', cancel); controller.signal.removeEventListener('abort', cancel) }
        function cancel() { detach(); reject(new Error('Motion part request expired')) }
        request.signal.addEventListener('abort', cancel, { once: true }); controller.signal.addEventListener('abort', cancel, { once: true })
        pending.then(value => { detach(); resolve(value) }, error => { detach(); reject(error) })
        if (request.signal.aborted || controller.signal.aborted) cancel()
      })
      assertCurrent(request.signal); requireMotion(!closed && compositor)
      const items = art.descriptor.wardrobe.items
      requireMotion(items.some(item => item.kind === part.kind && item.key === part.key))
      const first = (kind: 'body' | 'eyes' | 'hair') => { const item = items.find(item => item.kind === kind); requireMotion(item); return item.key }
      const selected = validateMotionSelection({ mode: 'wardrobe', body: first('body'), eyes: first('eyes'), hair: first('hair'),
        outfit: null, accessory: null, [part.kind]: part.key }, art)
      const seed = authority.presets(art)[0]; requireMotion(seed)
      // Pair membership in the verified descriptor is necessary but not authority:
      // the current scoped authority must also accept and round-trip this choice.
      const accepted = decode(authority.replace(decode(seed.config), selected))
      requireMotion(JSON.stringify(selection(accepted, art)) === JSON.stringify(selected))
      assertCurrent(request.signal); requireMotion(!closed)
      const next = compositor.acquirePartPreview(part)
      if (!current() || request.signal.aborted || closed) { next.release(); close(); throw new Error('Motion part owner expired') }
      acquired = next
      return { read: () => !closed && current() && acquired ? Object.freeze({ ...part, canvas: acquired.canvas,
        representation: 'isolated-part' as const, direction: 'down' as const, phase: 0 as const, composition: 'raw-item-cell' as const }) : null,
      subscribe(listener) { if (closed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } }, dispose: close }
    } catch (error) { close(); throw error }
  }
  return { kind: 'emo-avatar-runtime' as const, authority, scope, isCurrent: current, validateConfig: decode, acquire, acquirePartPreview,
    async catalog(signal: AbortSignal) {
      const { art } = await ready(signal)
      const presets = authority.presets(art).map(p => { const config = decode(p.config); selection(config, art); return Object.freeze({ ...p, config }) })
      requireMotion(presets.length === 6 && new Set(presets.map(p => p.id)).size === 6)
      return Object.freeze({ presets: Object.freeze(presets), items: art.descriptor.wardrobe.items,
        layers: MOTION_LAYERS, qualification: authority.qualification })
    },
    async choose(input: unknown, selected: MotionSelection, signal: AbortSignal) {
      const config = decode(input), { art } = await ready(signal), checked = validateMotionSelection(selected, art)
      const next = decode(authority.replace(config, checked)); requireMotion(JSON.stringify(selection(next, art)) === JSON.stringify(checked)); return next
    },
    /** The incumbent Phaser onFrame is the only clock producer. Views only subscribe. */
    sampleFrame(next: number) {
      if (!current() || !Number.isFinite(next) || next < 0) return
      time = next; for (const fn of Array.from(clocks)) { if (!current()) break; try { fn(time) } catch { /* Independent presentation. */ } }
    },
    readTime: () => time,
    onFrame(fn: (time: number) => void) { if (!current()) return () => {}; clocks.add(fn); return () => { clocks.delete(fn) } },
    diagnostics: () => ({ decodedAtlases: loaded ? 1 : 0, leases: leases.size, partPreviews: partPreviews.size, clockListeners: clocks.size, composition: compositor?.read() ?? null, disposed }),
    dispose,
  }
}
export type MotionRuntime<C = unknown> = ReturnType<typeof createMotionRuntime<C>>
