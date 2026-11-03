import { createMeetingBeaconController, type MeetingBeaconController, type MeetingAuthorityPort, type MeetingPhotoPort } from '../../features/meet-beacon'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { MeetingEntryAuth, MeetingEntryRelease } from './ports'
import type { PlacesState } from '../../features/places/model/controller'

export interface MeetingEntryView {
  readonly phase: 'unavailable' | 'available' | 'withdrawn'
  readonly controller: MeetingBeaconController | null
}
export const unavailableMeetingEntry: MeetingEntryView = Object.freeze({ phase: 'unavailable', controller: null })
const unavailablePlaces = Object.freeze<PlacesState>({ list: { phase: 'error', data: null, reason: 'unavailable' },
  detail: { phase: 'idle', data: null, reason: null }, selectedId: null, map: null, mapMismatch: true, unavailablePlaceIds: [], notice: null })
const same = (a: PrivacyScope, b: PrivacyScope) => a.userId === b.userId && a.eventId === b.eventId && a.generation === b.generation
  && a.policyGeneration === b.policyGeneration && a.gate === b.gate
const uuid = (s: unknown): s is string => typeof s === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(s)
const validScope = (s: PrivacyScope) => uuid(s?.userId) && uuid(s?.eventId) && Number.isSafeInteger(s?.generation) && s.generation >= 0
  && Number.isSafeInteger(s?.policyGeneration) && s.policyGeneration >= 0 && s.gate === 'open'

/** One captured shell lease. It never adopts a replacement identity or manufactures policy/clock/place authority. */
export function createMeetingEntry({ auth, release }: { auth: MeetingEntryAuth | null; release: MeetingEntryRelease | null }) {
  let view = unavailableMeetingEntry, disposed = false, withdrawn = false, broken = false, controller: MeetingBeaconController | null = null
  const listeners = new Set<() => void>(), stops: (() => void)[] = []
  const scope = release && validScope(release.scope) ? Object.freeze({ userId: release.scope.userId, eventId: release.scope.eventId,
    generation: release.scope.generation, policyGeneration: release.scope.policyGeneration, gate: release.scope.gate }) : null
  const sources = release ? Object.freeze({ places: release.places, authority: release.authority, clock: release.clock,
    photos: release.photos, invalidations: release.invalidations }) : null
  const signal = release?.signal
  function notify() { for (const listener of Array.from(listeners)) { try { listener() } catch { /* All presentation owners receive withdrawal. */ } } }
  function live() {
    return !disposed && !withdrawn && !!auth && !!scope && !!release && !!signal && !signal.aborted
      && auth.session.isCurrent(scope) && auth.privacy.isCurrent(scope)
  }
  function time(): number | null {
    if (!live()) return null
    try {
      const clock = sources!.clock.read()
      return clock && same(clock.scope, scope!) && Number.isFinite(clock.nowMs) && clock.nowMs >= 0 ? clock.nowMs : null
    } catch { return null }
  }
  function update() {
    const feature = controller?.read()
    const next = live() && feature?.canRead && feature.places.length > 0 && time() !== null ? controller : null
    const phase = withdrawn || disposed ? 'withdrawn' : next ? 'available' : 'unavailable'
    if (view.phase === phase && view.controller === next) return
    view = Object.freeze({ phase, controller: next }); notify()
  }
  function withdraw() {
    if (withdrawn) return
    withdrawn = true
    signal?.removeEventListener('abort', withdraw)
    for (const stop of stops.splice(0).reverse()) { try { stop() } catch { /* A failed port cleanup cannot retain private UI. */ } }
    // Invalidate the lease before disposal notifications so callbacks cannot reopen it.
    const previous = controller; controller = null
    try { previous?.dispose() } catch { /* Presentation withdrawal cannot depend on a port's cleanup behavior. */ }
    view = Object.freeze({ phase: 'withdrawn', controller: null }); notify()
  }
  function subscribe(source: (listener: () => void) => () => void, listener: () => void) {
    try {
      const stop = source(() => { if (live()) listener() }); let active = true
      return () => { if (!active) return; active = false; try { stop() } catch { /* Continue disposing the other feature sources. */ } }
    }
    catch { broken = true; if (controller) withdraw(); return () => {} }
  }
  function register(make: () => () => void) {
    if (withdrawn) return
    const stop = make()
    if (withdrawn) stop(); else stops.push(stop)
  }
  if (auth && release && scope && signal && live()) {
    try {
      const authority: MeetingAuthorityPort = {
        read() {
          if (!live() || time() === null) return null
          try { const a = sources!.authority.read(); return a && same(a.scope, scope) ? a : null } catch { return null }
        },
        subscribe(listener) { return subscribe(fn => sources!.authority.subscribe(fn), listener) },
      }
      const photos: MeetingPhotoPort | undefined = sources!.photos ? {
        read() {
          if (!live()) return null
          try { const p = sources!.photos!.read(); return p && same(p.scope, scope) ? p : null } catch { return null }
        },
        subscribe(listener) { return subscribe(fn => sources!.photos!.subscribe(fn), listener) },
      } : undefined
      const places = {
        read() {
          if (!live()) return unavailablePlaces
          try { return sources!.places.read() } catch { broken = true; if (controller) withdraw(); return unavailablePlaces }
        },
        subscribe(listener: () => void) { return subscribe(fn => sources!.places.subscribe(fn), listener) },
      }
      controller = createMeetingBeaconController({ auth, places, authority, now: time, ...(photos ? { photos } : {}) })
      if (broken || !live()) { controller.dispose(); controller = null; withdraw() }
      else {
        register(() => controller!.subscribe(update))
        register(() => sources!.clock.subscribe(() => { if (live()) { controller?.refreshTime(); update() } }))
        register(() => auth.privacy.registerCleanup(withdraw))
        if (sources!.invalidations) register(() => sources!.invalidations!.subscribe(event => {
          if (live() && same(event.scope, scope)) controller?.invalidate(event)
        }))
        if (!live()) withdraw()
        else { signal.addEventListener('abort', withdraw, { once: true }); controller?.start(); update() }
      }
    } catch { withdraw() }
  }
  return {
    read: () => view,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    withdraw,
    dispose() { if (disposed) return; withdraw(); disposed = true; listeners.clear() },
  }
}
export type MeetingEntry = ReturnType<typeof createMeetingEntry>
