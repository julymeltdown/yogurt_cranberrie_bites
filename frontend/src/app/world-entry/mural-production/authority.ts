import type { PrivacyPort } from '../../../features/mural/editor/contract'
import { toolContextSchema, WALL_TOOL_FRESH_MS, WALL_TOOL_REFRESH_MS } from '../../../features/mural/editor'
import type { WallToolPort } from '../../../features/mural/editor/tools'
import { bindingSchema, sameBinding, sameScope, clock as defaultClock } from '../../../features/mural/sync/contract'
import type { Clock, MuralBinding } from '../../../features/mural/sync/contract'
import type { MuralEntryQualification } from '../../../world/mural-entry-lifecycle'
import type { MuralQualificationContext, MuralQualificationSource } from '../../../world/mural-entry-lifecycle/source'
import type { MuralProductionConfiguration } from './configuration'

/** Shared C04 client freshness policy; not a wire TTL or independent write permission. */
export const muralCapabilityFreshnessMs = WALL_TOOL_FRESH_MS
export const muralCapabilityRefreshMs = WALL_TOOL_REFRESH_MS
export function matchesMuralPlacement(configuration: MuralProductionConfiguration, context: MuralQualificationContext): boolean {
  const map = context.release.map, pin = configuration.map, geometry = map.geometry
  return map.map_id === pin.id && map.map_revision === pin.revision && map.content_hash === pin.contentHash
    && map.geometry_hash === pin.geometryHash && geometry.width_cells * geometry.tile_width_px === pin.widthPx
    && geometry.height_cells * geometry.tile_height_px === pin.heightPx
}
/** One source per exact prepared World release; stores no private history, budget or command payload. */
export function createMuralAdmissionSource({ privacy, context, configuration, loadTools, clock = defaultClock }: {
  privacy: PrivacyPort; context: MuralQualificationContext; configuration: MuralProductionConfiguration
  loadTools: WallToolPort['load']; clock?: Clock
}): MuralQualificationSource & { dispose(): void; invalidate(): void } {
  const scope = Object.freeze({ ...context.scope }), worldRead = context.world.read.bind(context.world)
  const observers = new Set<() => void>(), authorities = new Set<() => void>()
  let stopped = false, turn = 0, lastTime = -Infinity
  let output: MuralEntryQualification | null = null, binding: MuralBinding | null = null
  let retryBinding: MuralBinding | null = null
  let capability: { editable: boolean; expiresAt: number } | null = null
  let flight: { controller: AbortController; timeout: unknown; turn: number; binding: MuralBinding } | null = null
  let refreshTimer: unknown = null, expiryTimer: unknown = null
  let stopWorld = () => {}, stopPrivacy = () => {}
  const emit = () => { for (const fn of [...authorities, ...observers]) { try { fn() } catch { /* Independent observers. */ } } }
  const clearTimer = (timer: unknown) => { if (timer !== null) clock.clear(timer) }
  const now = () => {
    const value = clock.now()
    if (!Number.isFinite(value) || value < lastTime) { dispose(); return null }
    lastTime = value; return value
  }
  function activeBinding(): MuralBinding | null {
    if (stopped || context.signal.aborted || !privacy.isCurrent(scope) || !sameScope(scope, privacy.capture())) return null
    const view = worldRead(), baseline = view?.baseline
    if (!view || !view.visible || view.phase !== 'live' || !baseline || !sameScope(scope, { ...scope, ...view.scope })
      || baseline.eventId !== scope.eventId || baseline.mapId !== configuration.map.id || baseline.mapRevision !== configuration.map.revision
      || !context.release.rooms.items.some(room => room.room_id === baseline.roomId)) return null
    // The public baseline has additional fields; decode only the exact immutable binding.
    const selected = bindingSchema.safeParse({ eventId: baseline.eventId, roomId: baseline.roomId, roomEpoch: baseline.roomEpoch, streamId: baseline.streamId })
    return selected.success ? Object.freeze(selected.data) : null
  }
  function clear() {
    ++turn; capability = null; output = null; binding = null; retryBinding = null
    clearTimer(refreshTimer); refreshTimer = null; clearTimer(expiryTimer); expiryTimer = null
    const old = flight; flight = null
    if (old) { clearTimer(old.timeout); old.controller.abort() }
    emit()
  }
  function current() {
    const time = now(), currentBinding = activeBinding()
    return time !== null && capability && time < capability.expiresAt && currentBinding && sameBinding(binding, currentBinding)
      ? currentBinding : null
  }
  function read() {
    if (output && !current()) { clear(); refresh() }
    return output
  }
  function scheduleRefresh() {
    clearTimer(refreshTimer)
    if (!stopped && activeBinding()) refreshTimer = clock.set(muralCapabilityRefreshMs, () => { refreshTimer = null; refresh() })
  }
  function refresh() {
    if (stopped || flight) return
    const candidate = activeBinding(), started = now()
    if (!candidate || started === null) { clear(); return }
    if (binding && !sameBinding(binding, candidate)) clear()
    const expectedTurn = turn, controller = new AbortController(), deadline = started + muralCapabilityFreshnessMs
    const request = { controller, timeout: null as unknown, turn: expectedTurn, binding: candidate }
    flight = request
    request.timeout = clock.set(muralCapabilityFreshnessMs, () => {
      if (flight !== request) return
      clear(); retryBinding = candidate; scheduleRefresh()
    })
    void Promise.resolve().then(() => loadTools(scope, controller.signal)).then(value => {
      if (stopped || flight !== request || turn !== expectedTurn || controller.signal.aborted) return
      const received = now(), latest = activeBinding(), parsed = toolContextSchema.safeParse(value)
      if (received === null || received >= deadline || !latest || !sameBinding(latest, candidate) || !parsed.success
        || parsed.data.actorId !== scope.userId || parsed.data.eventId !== scope.eventId) throw new Error('Mural eligibility unavailable')
      clearTimer(request.timeout); flight = null; clearTimer(expiryTimer)
      binding = candidate; retryBinding = null
      const caps = parsed.data.capabilities
      capability = { editable: parsed.data.editable && caps.backgroundIndex === 0 && (caps.pencil || caps.erase || caps.fill || caps.undo), expiresAt: deadline }
      if (!output) {
        const parentTurn = turn
        output = Object.freeze({ scope, binding: candidate, signal: context.signal, scene: context.scene,
          anchor: configuration.anchor, textureKey: configuration.textureKey,
          authority: Object.freeze({ read() {
            const value = current()
            if (parentTurn === turn && !value) { clear(); scheduleRefresh() }
            return parentTurn === turn && value ? Object.freeze({ binding: value, writable: capability?.editable === true }) : null
          }, subscribe(fn: () => void) {
            if (stopped || parentTurn !== turn) return () => {}
            authorities.add(fn); return () => { authorities.delete(fn) }
          } }) })
      }
      expiryTimer = clock.set(Math.max(0, deadline - received), () => { expiryTimer = null; clear(); refresh() })
      emit(); scheduleRefresh()
    }).catch(() => {
      if (flight !== request || turn !== expectedTurn) return
      clear(); retryBinding = candidate; scheduleRefresh()
    })
  }
  function changed() {
    if (stopped) return
    const next = activeBinding()
    if (!next) { clear(); return }
    if (!sameBinding(binding ?? flight?.binding ?? retryBinding ?? null, next)) { clear(); refresh() }
  }
  function invalidate() {
    if (stopped) return
    // A completed HTTP read/write must not revoke its own still-authorized read
    // lease before its response returns. Withdraw only editing while re-reading.
    if (capability) { capability = { ...capability, editable: false }; emit() }
    const old = flight; flight = null
    if (old) { clearTimer(old.timeout); old.controller.abort() }
    clearTimer(refreshTimer); refreshTimer = null; refresh()
  }
  function dispose() {
    if (stopped) return
    stopped = true; clear()
    try { stopWorld() } catch { /* Finish independent privacy cleanup. */ }
    try { stopPrivacy() } catch { /* Finish parent lifetime cleanup. */ }
    context.signal.removeEventListener('abort', dispose)
    observers.clear(); authorities.clear()
  }
  try {
    if (!matchesMuralPlacement(configuration, context) || context.signal.aborted || !privacy.isCurrent(scope)) dispose()
    else {
      context.signal.addEventListener('abort', dispose, { once: true })
      stopPrivacy = privacy.registerCleanup(dispose)
      stopWorld = context.world.subscribe(changed)
      if (stopped) { stopPrivacy(); stopWorld() } else changed()
    }
  } catch { dispose() }
  return Object.freeze({ read, dispose, invalidate, subscribe(fn: () => void) {
    if (stopped) return () => {}
    observers.add(fn); return () => { observers.delete(fn) }
  } })
}
