import type { SessionController } from '../../features/auth/model/session'
import { createPlacesController, type PlaceMapPort, type PlaceCameraPort, type PlacesHttpPort } from '../../features/places'
import { TransportError } from '../../shared/api/errors'
import type { PrivacyScope } from '../../shared/query/privacy'
import type { SceneryOverviewPort } from '../../world/illustrated/overview'

/** One existing world mount's public inputs, not a bootstrap/admission substitute.
 * Publish a new immutable binding for a fresh mount; abort before withdrawing it.
 * The map snapshot and camera attachment must belong to that same mount.
 */
export interface PlacesEntryRelease {
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  readonly map: PlaceMapPort
  readonly camera?: PlaceCameraPort
  readonly overview?: SceneryOverviewPort
}
export interface PlacesEntryPort {
  /** Stable snapshot until the underlying owner changes. Never restamp an old mount. */
  read(): PlacesEntryRelease | null
  subscribe(listener: () => void): () => void
}

/** Own only Places. Never creates Auth, a world, map loader, camera or connection. */
export function createPlacesEntry({ auth, source, release }: {
  auth: SessionController; source: PlacesEntryPort; release: PlacesEntryRelease
}) {
  const scope = Object.freeze({ ...release.scope }), map = release.map.read(), camera = release.camera?.read() ?? null
  const lifetime = new AbortController(), listeners = new Set<() => void>()
  let disposed = false, cameraWithdrawn = false
  let stopSource = () => {}, stopMap = () => {}, stopPrivacy = () => {}
  const sameScope = () => !!map && auth.privacy.isCurrent(scope) && auth.session.isCurrent(map.scope)
    && map.scope.userId === scope.userId && map.scope.eventId === scope.eventId && map.scope.generation === scope.generation
  const valid = () => {
    try { return !disposed && !!scope.userId && !!scope.eventId && !release.signal.aborted && source.read() === release && sameScope() && release.map.read() === map }
    catch { return false }
  }
  if (!valid()) throw new TransportError('invalid-request')
  // Camera attachment identity is pinned. Replacing/removing it cannot redirect
  // an old button into another world, even if the old camera later reappears.
  const cameraPort: PlaceCameraPort = { read() {
    if (!valid() || cameraWithdrawn) return null
    try { if ((release.camera?.read() ?? null) !== camera || !camera?.read().camera) cameraWithdrawn = true } catch { cameraWithdrawn = true }
    return cameraWithdrawn ? null : camera
  } }
  const mapPort: PlaceMapPort = { read: () => valid() ? map : null, subscribe: () => () => {} }
  const http: PlacesHttpPort = { request(input) {
    if (!valid()) return Promise.reject(new TransportError('session-changed'))
    const signal = AbortSignal.any([lifetime.signal, release.signal, ...(input.signal ? [input.signal] : [])])
    return auth.privacy.run(scope, context => {
      if (!valid()) throw new TransportError('session-changed')
      return auth.request({ ...input, signal: context.signal })
    }, signal)
  } }
  const overview: SceneryOverviewPort | undefined = release.overview ? {
    read: () => valid() ? release.overview!.read() : null,
    subscribe: release.overview.subscribe,
  } : undefined
  const controller = createPlacesController({ session: auth.session, http, map: mapPort, camera: cameraPort, ...(overview ? { overview } : {}) })
  function dispose() {
    if (disposed) return
    disposed = true; lifetime.abort()
    release.signal.removeEventListener('abort', dispose)
    for (const stop of [stopSource, stopMap, stopPrivacy]) { try { stop() } catch { /* Always evict feature state. */ } }
    controller.dispose()
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Independent consumers. */ } }
    listeners.clear()
  }
  const check = () => { if (!valid()) dispose() }
  try {
    release.signal.addEventListener('abort', dispose, { once: true })
    stopPrivacy = auth.privacy.registerCleanup(dispose)
    stopSource = source.subscribe(check)
    if (disposed) stopSource()
    stopMap = release.map.subscribe(check)
    if (disposed) stopMap()
    check()
    if (!disposed) controller.start()
  } catch (error) { dispose(); throw error }
  return Object.freeze({ auth, source, release, controller, dispose,
    read: () => valid() ? controller : null,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    isDisposed: () => disposed,
  })
}
export type PlacesEntry = ReturnType<typeof createPlacesEntry>
