import type { LoadedMap } from '../map/resources'
import { sceneryDepth, sceneryLimits, validateLoadedScenery } from './model'

/** Public pixels and identity only. No scene, entity, texture or decoded-image capability. */
export interface SceneryOverview {
  readonly imageUrl: string
  readonly width: number
  readonly height: number
  readonly mapId: string
  readonly mapRevision: number
  readonly mapHash: string
  readonly geometryHash: string
  readonly themeId: string
  readonly themeRevision: number
  readonly themeHash: string
  /** Separate Map-only pixels/annotations; never changes the authoritative release identity. */
  readonly presentationVersion?: string
  readonly presentationHash?: string
}
export interface SceneryOverviewPort {
  read(): SceneryOverview | null
  subscribe(listener: () => void): () => void
}

/** The caller supplies only its already hash-checked selected LoadedMap lease.
 * Draw synchronously while that lease is alive; fence asynchronous PNG encoding.
 * Canvas contains ground and static scenery only, never the incumbent game canvas.
 */
export function createSceneryOverview() {
  let snapshot: SceneryOverview | null = null, generation = 0, disposed = false
  type Raster = { canvas: HTMLCanvasElement; ticket: number; pins: Omit<SceneryOverview, 'imageUrl'> }
  let encoding = false, pending: Raster | null = null, activeCanvas: HTMLCanvasElement | null = null
  const listeners = new Set<() => void>()
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* Independent consumers. */ } } }
  function withdraw() {
    ++generation
    if (pending) { pending.canvas.width = 0; pending.canvas.height = 0; pending = null }
    // The browser already captured encoding pixels. Drop our active canvas storage now.
    if (activeCanvas) { activeCanvas.width = 0; activeCanvas.height = 0 }
    const old = snapshot; snapshot = null
    notify() // Withdraw before revoking the old URL, including reentrant observers.
    if (old) URL.revokeObjectURL(old.imageUrl)
  }
  function encode(raster: Raster) {
    encoding = true; activeCanvas = raster.canvas
    const done = (blob: Blob | null) => {
      raster.canvas.width = 0; raster.canvas.height = 0; activeCanvas = null; encoding = false
      if (!disposed && raster.ticket === generation && blob?.type === 'image/png' && blob.size > 0 && blob.size <= sceneryLimits.transferBytes) {
        try { snapshot = Object.freeze({ ...raster.pins, imageUrl: URL.createObjectURL(blob) }); notify() }
        catch { /* Schematic remains available if browser encoding/storage fails. */ }
      }
      const next = pending; pending = null
      if (next && !disposed && next.ticket === generation) encode(next)
    }
    try { raster.canvas.toBlob(done, 'image/png') } catch { done(null) }
  }
  function replace(map: LoadedMap | null) {
    if (disposed) return
    const expected = generation + 1
    withdraw()
    if (generation !== expected || disposed || !map?.scenery) return
    const ticket = generation
    let canvas: HTMLCanvasElement | null = null
    try {
      validateLoadedScenery(map)
      if (map.widthPx * map.heightPx > sceneryLimits.pixels) return
      canvas = document.createElement('canvas'); canvas.width = map.widthPx; canvas.height = map.heightPx
      const context = canvas.getContext('2d')
      if (!context) throw new Error('Overview unavailable')
      context.imageSmoothingEnabled = false
      const scenery = map.scenery, assets = new Map(scenery.assets.map(a => [a.descriptor.id, a.image]))
      context.drawImage(assets.get(scenery.manifest.ground.id)!, 0, 0)
      // Stable sort matches Phaser insertion order for equal depth.
      for (const object of [...scenery.manifest.objects].sort((a, b) => sceneryDepth(a.depth_y, map.heightPx) - sceneryDepth(b.depth_y, map.heightPx))) {
        const f = object.frame
        context.drawImage(assets.get(object.atlas_id)!, f.x, f.y, f.width, f.height, object.x, object.y, object.width, object.height)
      }
      const pins = { width: map.widthPx, height: map.heightPx, mapId: map.map.map_id, mapRevision: map.map.map_revision,
        mapHash: map.map.content_hash, geometryHash: map.map.geometry_hash,
        themeId: map.theme.theme_id, themeRevision: map.theme.theme_revision, themeHash: map.theme.content_hash }
      const raster = { canvas, ticket, pins }
      // At most one browser encoding and one latest pending raster. Superseded
      // pending canvases are cleared by withdraw; no unbounded encode queue.
      if (encoding) pending = raster
      else encode(raster)
    } catch {
      if (canvas) { canvas.width = 0; canvas.height = 0 }
    }
  }
  return {
    read: () => snapshot,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    replace,
    dispose() { if (disposed) return; disposed = true; withdraw(); listeners.clear() },
  }
}
