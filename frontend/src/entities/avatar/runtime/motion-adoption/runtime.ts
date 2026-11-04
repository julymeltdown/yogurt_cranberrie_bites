import type { PrivacyScope } from '../../../../shared/query/privacy'
import { abortable } from '../../../../world/map/resources'
import { MOTION_LAYERS, loadMotionArtwork, requireMotion, validateMotionSelection, type MotionArt, type MotionSelection } from './art'
import { createMotionCompositor, type MotionComposition, type MotionCompositor } from './compositor'
import { isReviewedProductionAuthority, type ReviewedProductionLifetime } from './reviewed-production'

/** Operations do not themselves admit a release. Production additionally needs a minted public grant. */
export interface MotionAuthorityOperations<C> {
  readonly manifestId: string; readonly manifestVersion: number; readonly manifestHash: string
  decodeConfig(value: unknown): C
  selection(config: C): MotionSelection
  presets(art: MotionArt): readonly { readonly id: string; readonly label: string; readonly config: C }[]
  replace(config: C, selection: MotionSelection): C
}
export type MotionAuthority<C> = MotionAuthorityOperations<C> & ({ readonly qualification: 'TEST_ONLY' } | ReviewedProductionLifetime)
export interface MotionLease<C> {
  read(): Readonly<{ config: C; selection: MotionSelection; canvas: HTMLCanvasElement }> | null
  subscribe(listener: () => void): () => void
  dispose(): void
}
/** Sole per-user/event owner of decoded bytes, composition cache, leases and scene time. */
export function createMotionRuntime<C>(options: {
  readonly authority: MotionAuthority<C>; readonly scope: PrivacyScope
  readonly privacy: { isCurrent(scope: PrivacyScope): boolean; registerCleanup(cleanup: () => void): () => void }
  readonly origin: string; readonly fetch?: typeof fetch
  readonly load?: typeof loadMotionArtwork
}) {
  const scope = Object.freeze({ ...options.scope }), authority = options.authority
  const production = authority.qualification === 'REVIEWED_PRODUCTION' ? authority : null
  requireMotion(!production || isReviewedProductionAuthority(production) && production.isCurrent() && !options.load
    && scope.userId === production.scope.userId && scope.eventId === production.scope.eventId
    && scope.generation === production.scope.generation && scope.policyGeneration === production.scope.policyGeneration && scope.gate === production.scope.gate)
  requireMotion(scope.userId && scope.eventId && scope.gate === 'open' && (production || authority.qualification === 'TEST_ONLY')
    && authority.manifestId.length > 0 && Number.isSafeInteger(authority.manifestVersion) && authority.manifestVersion > 0
    && /^sha256:[a-f0-9]{64}$/.test(authority.manifestHash))
  const controller = new AbortController(), leases = new Set<() => void>(), clocks = new Set<(time: number) => void>()
  let disposed = false, flight: Promise<{ art: MotionArt; image: HTMLImageElement; dispose(): void }> | null = null
  let loaded: Awaited<ReturnType<typeof loadMotionArtwork>> | null = null, compositor: MotionCompositor | null = null, time = 0
  const current = () => !disposed && !controller.signal.aborted && options.privacy.isCurrent(scope) && (!production || production.isCurrent())
  const assertCurrent = (signal?: AbortSignal) => requireMotion(current() && !signal?.aborted)
  function dispose() {
    if (disposed) return
    disposed = true; controller.abort(); unregister(); unregisterAuthority(); production?.dispose(); clocks.clear()
    for (const close of Array.from(leases)) close()
    compositor?.dispose(); compositor = null; loaded?.dispose(); loaded = null; flight = null
  }
  let unregister = () => {}, unregisterAuthority = () => {}
  unregister = options.privacy.registerCleanup(dispose)
  if (production) unregisterAuthority = production.onInvalidate(dispose)
  if (!current()) dispose()
  async function ready(signal: AbortSignal) {
    assertCurrent(signal)
    if (!flight) {
      const next = (options.load ?? loadMotionArtwork)({ origin: options.origin, signal: controller.signal,
        ...(options.fetch ? { fetch: options.fetch } : {}) }).then(value => {
          if (!current()) { value.dispose(); throw new Error('Motion owner expired') }
          loaded = value; compositor = createMotionCompositor({ art: value.art, image: value.image, isCurrent: current }); return value
        }).catch(error => { if (flight === next) flight = null; if (production) dispose(); throw error })
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
  return { kind: 'motion-adoption' as const, authority, scope, isCurrent: current, validateConfig: decode, acquire,
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
    diagnostics: () => ({ decodedAtlases: loaded ? 1 : 0, leases: leases.size, clockListeners: clocks.size, composition: compositor?.read() ?? null, disposed }),
    dispose,
  }
}
export type MotionRuntime<C = unknown> = ReturnType<typeof createMotionRuntime<C>>
