import type { HttpRequest, HttpResult, SessionPort, SessionScope } from '../../../shared/api/http'
import { TransportError } from '../../../shared/api/errors'
import type { MapManifest } from '../../../world/map/manifest'
import { schemaValid } from '../../../world/map/schema'
import { canonicalJson } from '../../../world/map/json'
import type { createCameraControl } from '../../../world/camera/port'
import { decodePlace, decodePlaces, placeId, type PlaceList, type PlaceResponse } from './contract'

/** Already validated by FE14's asset owner. No map/image/network ownership here. */
export interface PlaceMapRelease {
  readonly scope: SessionScope
  readonly manifest: MapManifest
  readonly expected: { readonly mapId: string; readonly revision: number; readonly contentHash: string; readonly geometryHash: string }
}
export interface PlaceMapPort {
  read(): PlaceMapRelease | null
  /** Publish every release replacement/removal; read() must return the current snapshot. */
  subscribe(listener: () => void): () => void
}
export interface PlaceCameraPort {
  /** Return the existing FE15 attachment's camera, never create a second camera/game. */
  read(): Pick<ReturnType<typeof createCameraControl>, 'lookAt' | 'recenter' | 'setZoom' | 'read'> | null
}
/** Inject the canonical SessionController.request. Refresh/expiry policy stays with FE09. */
export interface PlacesHttpPort {
  request<T>(input: Omit<HttpRequest<T>, 'refresh'>): Promise<HttpResult<T>>
}
export type PlaceFailure = 'signed-out' | 'denied' | 'missing' | 'conflict' | 'map-mismatch' | 'rate-limited' | 'invalid' | 'unavailable'
export interface Resource<T> { readonly phase: 'idle' | 'loading' | 'ready' | 'empty' | 'error'; readonly data: T | null; readonly reason: PlaceFailure | null }
export interface PlacesState {
  readonly list: Resource<PlaceList>
  readonly detail: Resource<PlaceResponse>
  readonly selectedId: string | null
  readonly map: MapManifest | null
  readonly mapMismatch: boolean
  readonly unavailablePlaceIds: readonly string[]
  readonly notice: string | null
}
const resource = <T>(phase: Resource<T>['phase'], data: T | null = null, reason: PlaceFailure | null = null): Resource<T> => ({ phase, data, reason })
function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { for (const child of Object.values(value)) freeze(child); Object.freeze(value) }
  return value
}
function key(release: PlaceMapRelease | null) {
  if (!release) return ''
  const m = release.manifest, p = release.expected
  return JSON.stringify([release.scope.generation, release.scope.userId, release.scope.eventId,
    m.map_id, m.map_revision, m.content_hash, m.geometry_hash, p.mapId, p.revision, p.contentHash, p.geometryHash])
}
function validatedMap(release: PlaceMapRelease | null, scope: SessionScope): MapManifest | null {
  if (!release) return null
  if (release.scope.generation !== scope.generation || release.scope.userId !== scope.userId || release.scope.eventId !== scope.eventId) return null
  const m = release.manifest, p = release.expected, g = m.geometry
  if (!schemaValid(m, 'map') || m.map_id !== p.mapId || m.map_revision !== p.revision
    || m.content_hash !== p.contentHash || m.geometry_hash !== p.geometryHash
    || g.width_cells > 128 || g.height_cells > 128 || g.tile_width_px > 128 || g.tile_height_px > 128
    || g.collision_grid.length !== g.height_cells || g.collision_grid.some(row => row.length !== g.width_cells)
    || new Set(m.places.map(p => p.place_id)).size !== m.places.length
    || g.interactions.some(p => !m.places.some(place => place.place_id === p.place_id)
      || p.cell_x >= g.width_cells || p.cell_y >= g.height_cells || g.collision_grid[p.cell_y]?.[p.cell_x] !== 0)) return null
  return freeze(structuredClone(m))
}
function failure(error: unknown): PlaceFailure {
  if (!(error instanceof TransportError)) return 'unavailable'
  if (error.server?.reason === 'MAP_VERSION_MISMATCH') return 'map-mismatch'
  if (error.status === 401 || error.kind === 'authentication-required' || error.kind === 'session-changed') return 'signed-out'
  if (error.status === 403) return 'denied'
  if (error.status === 404 || error.status === 410) return 'missing'
  if (error.status === 409) return 'conflict'
  if (error.status === 429) return 'rate-limited'
  if (error.kind === 'invalid-response' || error.kind === 'invalid-request') return 'invalid'
  return 'unavailable'
}
/** Catalog revision is NOT map_revision. Geometry targets use the separately pinned C06 release. */
export function createPlacesController(options: { session: SessionPort; http: PlacesHttpPort; map: PlaceMapPort; camera?: PlaceCameraPort }) {
  let state: PlacesState = freeze({ list: resource<PlaceList>('idle'), detail: resource<PlaceResponse>('idle'),
    selectedId: null, map: null, mapMismatch: false, unavailablePlaceIds: [], notice: null })
  let started = false, disposed = false, revision = 0, detailRevision = 0
  let activeReleaseKey = ''
  let listAbort: AbortController | null = null, detailAbort: AbortController | null = null
  let removeScope = () => {}, removeMap = () => {}
  const listeners = new Set<() => void>()
  const publish = (patch: Partial<PlacesState>) => {
    if (disposed) return
    state = freeze({ ...state, ...patch })
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Snapshot prevents reentrant observers changing this publication. */ } }
  }
  const scope = () => options.session.read()
  const sameScope = (snapshot: SessionScope) => options.session.isCurrent(snapshot)
  const unavailableTargets = new Set<string>()
  let invalidationScope: SessionScope = scope(), invalidReleaseKey: string | null = null
  function cancel() { ++revision; ++detailRevision; listAbort?.abort(); detailAbort?.abort(); listAbort = null; detailAbort = null }
  function bindScope() {
    removeScope()
    const signal = scope().signal, changed = () => { if (!disposed && started) { bindScope(); void refresh() } }
    signal.addEventListener('abort', changed, { once: true })
    removeScope = () => signal.removeEventListener('abort', changed)
  }
  function mapped(list: PlaceList, map: MapManifest | null) {
    // Dynamic closure/verification may become stricter without moving an interaction.
    // Stable identity/provenance changes require a fresh map release, never a guessed target.
    return Boolean(map && list.items.every(place => map.places.some(p => p.place_id === place.place_id
      && p.place_kind === place.place_kind && p.label === place.label
      && p.provenance.source_id === place.provenance.source_id && p.provenance.source_revision === place.provenance.source_revision)))
  }
  async function loadDetail(id: string) {
    ++detailRevision; detailAbort?.abort()
    const ticket = detailRevision, listTicket = revision, snapshot = scope(), releaseKey = key(options.map.read()), catalog = state.list.data
    if (disposed || !started) return
    if (!placeId.safeParse(id).success) { publish({ detail: resource<never>('error', null, 'missing') }); return }
    if (state.list.phase === 'empty') { publish({ detail: resource<never>('error', null, 'missing') }); return }
    if (!catalog || state.list.phase !== 'ready') return
    const abort = new AbortController(); detailAbort = abort
    const current = () => !disposed && started && ticket === detailRevision && listTicket === revision && !abort.signal.aborted
      && sameScope(snapshot) && key(options.map.read()) === releaseKey && state.selectedId === id
    publish({ detail: resource('loading') })
    if (!current()) return
    try {
      const result = await options.http.request({ path: `/api/concert/events/${snapshot.eventId}/places/${id}`,
        method: 'GET', access: 'protected', errorFamily: 'core', decode: decodePlace, signal: abort.signal })
      if (!current()) return
      if (result.data.revision !== catalog.revision) { publish({ detail: resource<never>('error', null, 'conflict'), mapMismatch: true }); return }
      const listed = catalog.items.find(p => p.place_id === id)
      if (!listed || canonicalJson(listed) !== canonicalJson(result.data.place)) {
        invalidReleaseKey = releaseKey
        publish({ detail: resource<never>('error', null, 'map-mismatch'), mapMismatch: true }); return
      }
      unavailableTargets.delete(id)
      publish({ detail: resource('ready', result.data), unavailablePlaceIds: Array.from(unavailableTargets) })
    } catch (error) {
      if (!current()) return
      const reason = failure(error)
      if (reason === 'missing' && catalog.items.some(p => p.place_id === id)) unavailableTargets.add(id)
      if (reason === 'map-mismatch') invalidReleaseKey = releaseKey
      publish({ detail: resource<never>('error', null, reason), unavailablePlaceIds: Array.from(unavailableTargets),
        ...(reason === 'map-mismatch' ? { mapMismatch: true } : {}) })
    }
  }
  async function refresh() {
    if (disposed || !started) return
    cancel()
    const ticket = revision, snapshot = scope(), release = options.map.read(), releaseKey = key(release)
    if (invalidationScope.generation !== snapshot.generation || invalidationScope.userId !== snapshot.userId || invalidationScope.eventId !== snapshot.eventId) {
      unavailableTargets.clear(); invalidReleaseKey = null; invalidationScope = snapshot
    }
    activeScope = snapshot; activeReleaseKey = releaseKey
    if (!snapshot.userId || !snapshot.eventId || !snapshot.accessToken || snapshot.signal.aborted) {
      publish({ list: resource<never>('error', null, 'signed-out'), detail: resource('idle'), map: null, mapMismatch: true, unavailablePlaceIds: [], notice: null }); return
    }
    const abort = new AbortController(); listAbort = abort
    const current = () => !disposed && started && ticket === revision && !abort.signal.aborted && sameScope(snapshot) && key(options.map.read()) === releaseKey
    let map: MapManifest | null = null
    try { map = validatedMap(release, snapshot) } catch { /* Invalid or withdrawn release cannot supply targets. */ }
    publish({ list: resource('loading'), detail: resource('idle'), map, mapMismatch: !map || invalidReleaseKey !== null,
      unavailablePlaceIds: Array.from(unavailableTargets), notice: null })
    if (!current()) return
    try {
      const result = await options.http.request({ path: `/api/concert/events/${snapshot.eventId}/places`,
        method: 'GET', access: 'protected', errorFamily: 'core', decode: decodePlaces, signal: abort.signal })
      if (!current()) return
      const consistent = mapped(result.data, map)
      // A new, matching map/catalog pair can reconcile a rejected release. Re-reading
      // the same old catalog/pin must not silently undo an authoritative mismatch.
      if (consistent && invalidReleaseKey !== releaseKey) invalidReleaseKey = null
      for (const id of unavailableTargets) if (!result.data.items.some(p => p.place_id === id)) unavailableTargets.delete(id)
      publish({ list: resource(result.data.items.length ? 'ready' : 'empty', result.data),
        mapMismatch: !consistent || invalidReleaseKey !== null, unavailablePlaceIds: Array.from(unavailableTargets) })
      if (current() && state.selectedId) {
        if (result.data.items.length) await loadDetail(state.selectedId)
        else publish({ detail: resource<never>('error', null, 'missing') })
      }
    } catch (error) {
      if (!current()) return
      const reason = failure(error)
      if (reason === 'map-mismatch') invalidReleaseKey = releaseKey
      publish({ list: resource<never>('error', null, reason), detail: resource('idle'), mapMismatch: true })
    }
  }
  function cameraAction(action: (camera: NonNullable<ReturnType<PlaceCameraPort['read']>>) => void) {
    const camera = options.camera?.read()
    if (disposed || !started || !camera || state.mapMismatch || !state.map || !sameScope(activeScope)
      || key(options.map.read()) !== activeReleaseKey) return false
    try { action(camera); publish({ notice: 'Camera updated in the virtual world. Your avatar has not moved.' }); return true }
    catch { publish({ notice: 'Camera controls are unavailable. The place list remains accessible.' }); return false }
  }
  let activeScope: SessionScope = scope()
  function canShow(id: string) {
    return !disposed && started && sameScope(activeScope) && !state.mapMismatch && state.list.phase === 'ready'
      && !unavailableTargets.has(id)
      && key(options.map.read()) === activeReleaseKey
      && Boolean(options.camera?.read()) && Boolean(state.map?.geometry.interactions.some(p => p.place_id === id))
      && state.map?.places.some(p => p.place_id === id && p.closure_state === 'OPEN') === true
      && state.list.data?.items.some(p => p.place_id === id && p.closure_state === 'OPEN') === true
      && (state.selectedId !== id || state.detail.phase === 'ready')
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    start() {
      if (disposed || started) return
      started = true; activeScope = scope(); bindScope()
      removeMap = options.map.subscribe(() => { activeScope = scope(); void refresh() })
      void refresh()
    },
    refresh() { activeScope = scope(); return refresh() },
    select(id: string | null) {
      if (disposed) return
      ++detailRevision; detailAbort?.abort()
      publish({ selectedId: id, detail: resource('idle'), notice: null })
      if (id && started) void loadDetail(id)
    },
    canShow,
    showOnMap(id: string) {
      if (!canShow(id)) return false
      const map = state.map!, target = map.geometry.interactions.find(p => p.place_id === id)!
      return cameraAction(camera => camera.lookAt({ x: (target.cell_x + .5) * map.geometry.tile_width_px, y: (target.cell_y + .5) * map.geometry.tile_height_px }))
    },
    cameraAvailable: () => !disposed && started && Boolean(options.camera?.read()) && !state.mapMismatch && sameScope(activeScope) && key(options.map.read()) === activeReleaseKey,
    recenter: () => cameraAction(camera => camera.recenter()),
    zoom(delta: -1 | 1) { return cameraAction(camera => { const zoom = camera.read().camera?.zoom; if (zoom !== undefined) camera.setZoom(Math.max(1, Math.min(8, zoom + delta))) }) },
    dispose() {
      if (disposed) return
      disposed = true; started = false; cancel(); removeScope(); removeMap()
      unavailableTargets.clear(); invalidReleaseKey = null
      state = freeze({ list: resource<PlaceList>('idle'), detail: resource<PlaceResponse>('idle'), selectedId: null, map: null, mapMismatch: true, unavailablePlaceIds: [], notice: null })
      for (const listener of Array.from(listeners)) { try { listener() } catch { /* Best-effort final eviction notification. */ } }
      listeners.clear()
    },
  }
}
export type PlacesController = ReturnType<typeof createPlacesController>
