import native from '../spatial-v3/source-inputs.json'
import tile from '../spatial-v3/tile-pin.json'
import { canonicalJson, MapError, requireMap } from '../../map/json'
import type { LoadedMap } from '../../map/resources'
import type { LoadedScenery, Scenery } from '../../illustrated/model'
import { admitPhoto2Display, PHOTO2_DISPLAY, PHOTO2_NATIVE, PHOTO2_SOURCE_MESH, PHOTO2_VERSION,
  type DisplayBinding, type DisplayMesh, type Point, type Projection, type Rect } from '../../../features/parade-ink/world-hud/photo2-world-display-v1'

export interface Photo2PaintFrame {
  readonly id: string
  readonly atlasId: string
  readonly atlasRect: Rect
  readonly nativeFoot: Point
  readonly displayFoot: Point
  readonly width: number
  readonly height: number
}
export interface Photo2BoardFrame {
  readonly id: 'photo2-shortcut-meet' | 'photo2-shortcut-wall'
  readonly destination: '/meet' | '/wall'
  readonly atlasId: string
  readonly atlasRect: Rect
  readonly displayRect: Rect
  readonly interactionId: null
}
/** Actual CSS-client rectangles from the currently admitted scene and camera. */
export interface Photo2ShortcutSnapshot {
  readonly version: typeof PHOTO2_VERSION
  readonly viewport: Rect
  readonly boards: readonly {
    readonly id: Photo2BoardFrame['id']
    readonly destination: Photo2BoardFrame['destination']
    readonly clientRect: Rect
  }[]
}
type AdmissionPins = Omit<Parameters<typeof admitPhoto2Display>[1], 'loaded' | 'loading'>
/** Supplied by a literal reviewed release registry after ART/catalog qualification.
 * This leaf neither registers a release nor derives trusted pins from its input.
 */
export interface Photo2RegisteredRelease {
  readonly display: DisplayBinding
  readonly pins: AdmissionPins
  readonly mapHash: string
  readonly scenery: Scenery
  readonly nativeTexture: LoadedMap['theme']['texture']
  readonly groundBounds: Rect
  readonly camera: Rect
  readonly frames: readonly Photo2PaintFrame[]
  readonly boards: readonly Photo2BoardFrame[]
}
export interface Photo2SceneDisplay {
  readonly version: typeof PHOTO2_VERSION
  readonly mesh: DisplayMesh
  readonly projection: Projection
  readonly groundBounds: Rect
  readonly camera: Rect
  readonly frames: readonly Photo2PaintFrame[]
  readonly boards: readonly Photo2BoardFrame[]
}
export const PHOTO2_ASSET_PREFIX = 'world/reference-scene-v2/photo2-world-display-v1/'
/** No producer release is admitted until its literal ART/C06/catalog tuple is qualified. */
const registered: readonly Photo2RegisteredRelease[] = Object.freeze([])
export function registeredPhoto2Release(map: Pick<LoadedMap, 'map' | 'theme'>): Photo2RegisteredRelease {
  const release = registered.find(r => r.mapHash === map.map.content_hash && r.pins.themeHash === map.theme.content_hash
    && r.pins.themeRevision === map.theme.theme_revision && map.theme.theme_id === 'PARADE')
  // An absent or unqualified artifact is unavailable, never a fabricated legacy fallback.
  if (!release) throw new MapError('unavailable')
  return release
}
/** Exact registered scenery bytes and frozen native dimensions, never a prefix-only exception. */
export function registeredPhoto2Ground(scenery: Scenery, width: number, height: number) {
  return width === PHOTO2_NATIVE.width && height === PHOTO2_NATIVE.height && registered.some(r =>
    r.display.geometryHash === PHOTO2_NATIVE.geometryHash && r.pins.era === 'PARADE'
    && r.groundBounds.width === scenery.ground.width_px && r.groundBounds.height === scenery.ground.height_px
    && canonicalJson(r.scenery) === canonicalJson(scenery))
}
export function isPhoto2ReferenceClaim(value: Pick<LoadedMap, 'theme'>) {
  return [value.theme.scenery?.ground.path, ...value.theme.scenery?.atlases.map(a => a.path) ?? []]
    .some(path => path?.startsWith(PHOTO2_ASSET_PREFIX))
}
const near = (a: Point | null, b: Point) => a !== null && Math.abs(a.x - b.x) <= 1e-7 && Math.abs(a.y - b.y) <= 1e-7
const rect = (r: Rect) => [r.x, r.y, r.width, r.height].every(Number.isFinite) && r.width > 0 && r.height > 0
const copyRect = (r: Rect): Rect => Object.freeze({ x: r.x, y: r.y, width: r.width, height: r.height })
const sameRect = (a: Rect, b: Rect) => a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height
const meshKey = (m: DisplayMesh) => JSON.stringify([m.columns, m.rows, m.display.map(point => [point.x, point.y])])
/** Strict version dispatch must reach this fence before legacy geometry aliases.
 * Existing C03/C06 hashing, schema and asset loading remain prerequisites.
 */
export function validatePhoto2Metadata(map: Pick<LoadedMap, 'map' | 'theme' | 'widthPx' | 'heightPx' | 'spawn'>,
  release: Photo2RegisteredRelease) {
  const g = native.geometry, theme = map.theme, nativeGround = native.scenes.PARADE.scenery.ground
  requireMap(release.display.era === 'PARADE' && release.pins.era === 'PARADE'
    && theme.theme_id === 'PARADE' && theme.theme_revision === release.pins.themeRevision
    && theme.content_hash === release.pins.themeHash && map.map.content_hash === release.mapHash
    && map.map.map_id === PHOTO2_NATIVE.mapId && map.map.map_revision === PHOTO2_NATIVE.revision
    && map.map.geometry_hash === PHOTO2_NATIVE.geometryHash
    && canonicalJson(map.map.geometry) === canonicalJson(g) && canonicalJson(map.map.places) === canonicalJson(native.places)
    && map.widthPx === PHOTO2_NATIVE.width && map.heightPx === PHOTO2_NATIVE.height
    && map.map.tilemap.resource_state === 'PROVIDED' && map.map.tilemap.path === tile.path && map.map.tilemap.sha256 === tile.sha256
    && theme.map_id === map.map.map_id && theme.map_revision === map.map.map_revision
    && theme.map_content_hash === map.map.content_hash && theme.geometry_hash === map.map.geometry_hash
    && theme.collision_id === g.collision_id && canonicalJson(map.spawn) === canonicalJson(g.spawns.find(s => s.spawn_id === g.default_spawn_id))
    && canonicalJson(theme.texture) === canonicalJson(release.nativeTexture)
    && theme.texture.image_state === 'PROVIDED' && theme.texture.path === nativeGround.path && theme.texture.sha256 === nativeGround.sha256
    && theme.texture.width_px === PHOTO2_NATIVE.width && theme.texture.height_px === PHOTO2_NATIVE.height
    && canonicalJson(theme.scenery) === canonicalJson(release.scenery))
  const s = release.scenery, bounds = release.groundBounds
  // Renderer source and release pins must name the same mesh, camera and ground frame.
  requireMap(sameRect(release.groundBounds, PHOTO2_DISPLAY.groundBounds) && sameRect(release.camera, PHOTO2_DISPLAY.camera)
    && meshKey(release.display.mesh) === meshKey(PHOTO2_SOURCE_MESH))
  requireMap(s.objects.length === release.display.anchors.length && release.display.anchors.every(anchor => {
    const matches = s.objects.filter(object => object.id === anchor.id)
    return matches.length === 1 && ['x', 'y', 'width', 'height', 'depth_y'].every(key => matches[0]![key as 'x'] === anchor[key as 'x'])
  }))
  requireMap(rect(bounds) && rect(release.camera) && Number.isSafeInteger(bounds.width) && Number.isSafeInteger(bounds.height)
    && s.ground.path.startsWith(PHOTO2_ASSET_PREFIX) && s.ground.width_px === bounds.width && s.ground.height_px === bounds.height
    && release.display.assets.length === s.atlases.length + 1
    && [s.ground, ...s.atlases].every(asset => release.display.assets.some(a => a.path === asset.path && a.sha256 === asset.sha256
      && a.bytes === asset.bytes && a.width === asset.width_px && a.height === asset.height_px)))
  requireMap(admitPhoto2Display(release.display, { ...release.pins, loaded: release.pins.assets, loading: false }))
}
export function requirePhoto2Display(map: LoadedMap, release: Photo2RegisteredRelease): Photo2SceneDisplay {
  validatePhoto2Metadata(map, release)
  const loaded: LoadedScenery | undefined = map.scenery
  requireMap(loaded && canonicalJson(loaded.manifest) === canonicalJson(release.scenery))
  const projection = admitPhoto2Display(release.display, { ...release.pins, loading: false,
    loaded: loaded.assets.map(a => ({ path: a.descriptor.path, sha256: a.descriptor.sha256, bytes: a.descriptor.bytes,
      width: a.image.naturalWidth, height: a.image.naturalHeight })) })
  requireMap(projection && release.frames.length === release.display.anchors.length && release.boards.length === 2)
  const atlases = new Map(release.scenery.atlases.map(a => [a.id, a])), ids = new Set<string>()
  const packed: { atlasId: string; area: Rect }[] = []
  const take = (id: string, atlasId: string, area: Rect) => {
    const atlas = atlases.get(atlasId)
    requireMap(atlas && !ids.has(id) && rect(area) && [area.x, area.y, area.width, area.height].every(Number.isSafeInteger)
      && area.x >= 0 && area.y >= 0 && area.x + area.width <= atlas.width_px && area.y + area.height <= atlas.height_px)
    requireMap(packed.every(p => p.atlasId !== atlasId || area.x + area.width <= p.area.x || p.area.x + p.area.width <= area.x
      || area.y + area.height <= p.area.y || p.area.y + p.area.height <= area.y))
    packed.push({ atlasId, area }); ids.add(id)
  }
  const frames = release.frames.map(frame => {
    take(frame.id, frame.atlasId, frame.atlasRect)
    const anchor = release.display.anchors.find(a => a.id === frame.id)
    const object = release.scenery.objects.find(o => o.id === frame.id)
    requireMap(object && object.atlas_id === frame.atlasId && object.frame.x === frame.atlasRect.x && object.frame.y === frame.atlasRect.y
      && object.frame.width === frame.atlasRect.width && object.frame.height === frame.atlasRect.height)
    requireMap(anchor && Number.isSafeInteger(frame.width) && Number.isSafeInteger(frame.height)
      && frame.width > 0 && frame.height > 0 && frame.width <= 1024 && frame.height <= 1024
      && near(frame.nativeFoot, { x: anchor.x + anchor.width / 2, y: anchor.depth_y })
      && near(projection.forward(frame.nativeFoot), frame.displayFoot))
    return Object.freeze({ ...frame, nativeFoot: Object.freeze({ ...frame.nativeFoot }),
      displayFoot: Object.freeze({ ...frame.displayFoot }), atlasRect: copyRect(frame.atlasRect) })
  })
  const boards = release.boards.map(board => {
    requireMap(((board.id === 'photo2-shortcut-meet' && board.destination === '/meet') || (board.id === 'photo2-shortcut-wall' && board.destination === '/wall'))
      && board.interactionId === null && rect(board.displayRect) && board.displayRect.width <= 1024 && board.displayRect.height <= 1024)
    take(board.id, board.atlasId, board.atlasRect)
    return Object.freeze({ ...board, atlasRect: copyRect(board.atlasRect), displayRect: copyRect(board.displayRect) })
  })
  const mesh = Object.freeze({ columns: Object.freeze([...release.display.mesh.columns]), rows: Object.freeze([...release.display.mesh.rows]),
    display: Object.freeze(release.display.mesh.display.map(point => Object.freeze({ ...point }))) })
  return Object.freeze({ version: PHOTO2_VERSION, mesh, projection, groundBounds: copyRect(release.groundBounds),
    camera: copyRect(release.camera), frames: Object.freeze(frames), boards: Object.freeze(boards) })
}
/** The same visual foot order applies to billboards and actors. Never feed it to physics. */
export function photo2DisplayDepth(display: Photo2SceneDisplay, footY: number) {
  requireMap(Number.isFinite(footY))
  return 1 + (footY - display.groundBounds.y) / (display.groundBounds.height + 1) * .9
}
