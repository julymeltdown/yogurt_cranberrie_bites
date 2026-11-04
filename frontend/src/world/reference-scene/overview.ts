import type { LoadedMap } from '../map/resources'
import type { SceneryOverview } from '../illustrated/overview'
import { limits } from '../map/manifest'
import { requireReferenceScene, type ReferenceSceneBinding } from './model'
import { isSpatialV4ReferenceClaim } from './spatial-v4/model'
import { createSpatialV4Labels } from './spatial-v4/map-labels'
import { isSpatialV5ReferenceClaim } from './spatial-v5/model'
import { createSpatialV5Labels } from './spatial-v5/map-labels'

/** MAP-only semantic projection; never mutates the shared WORLD collision or actor canvas. */
export function drawReferenceOverview(context: CanvasRenderingContext2D, binding: ReferenceSceneBinding) {
  const { loaded, overview, plaza } = binding
  const images = new Map(loaded.assets.map(a => [a.descriptor.id, a.image]))
  context.imageSmoothingEnabled = false
  context.drawImage(images.get(loaded.manifest.ground.id)!, 0, 0)
  context.save()
  try {
    context.beginPath()
    plaza.forEach(([x, y], i) => { if (i === 0) context.moveTo(x, y); else context.lineTo(x, y) })
    context.closePath(); context.fillStyle = '#7a8742'; context.fill()
    context.strokeStyle = '#b4a27d'; context.lineWidth = 6; context.stroke()
  } finally { context.restore() }
  // The destination canvas clips cropped edge illustrations. Only WORLD prop footprints are solid.
  for (const prop of [...overview].sort((a, b) => a.depth_y - b.depth_y)) {
    const f = prop.frame
    context.drawImage(images.get(prop.atlas_id)!, f.x, f.y, f.width, f.height, prop.x, prop.y, prop.width, prop.height)
  }
}

/** Same public pixel/identity port as the incumbent overview. Root owns release and privacy binding. */
export function createReferenceSceneOverview() {
  let labels = createSpatialV4Labels(), labelsVersion = 4
  type Raster = { canvas: HTMLCanvasElement; ticket: number; pins: Omit<SceneryOverview, 'imageUrl'> }
  let disposed = false, generation = 0, encoding = false, active: Raster | null = null, pending: Raster | null = null
  let snapshot: SceneryOverview | null = null
  const listeners = new Set<() => void>()
  const clear = (raster: Raster | null) => { if (raster) raster.canvas.width = raster.canvas.height = 0 }
  const notify = () => { for (const listener of Array.from(listeners)) { try { listener() } catch { /* Independent consumers. */ } } }
  function withdraw() {
    generation++; clear(pending); pending = null; clear(active)
    const old = snapshot; snapshot = null
    notify() // Withdraw identity before revoking the previous pixel capability.
    if (old) URL.revokeObjectURL(old.imageUrl)
  }
  function encode(raster: Raster) {
    encoding = true; active = raster
    let completed = false
    const done = (blob: Blob | null) => {
      if (completed) return; completed = true; clear(raster); active = null; encoding = false
      const next = pending; pending = null
      let published = false
      // This local encoded projection is one PNG, not the C06 combined source transfer budget.
      if (!disposed && raster.ticket === generation && blob?.type === 'image/png' && blob.size > 0 && blob.size <= limits.pngBytes) {
        try { snapshot = Object.freeze({ ...raster.pins, imageUrl: URL.createObjectURL(blob) }); published = true } catch { /* Remain unavailable. */ }
      }
      if (!disposed && next?.ticket === generation) encode(next)
      else clear(next)
      if (published) notify()
    }
    try { raster.canvas.toBlob(done, 'image/png') } catch { done(null) }
  }
  function replace(map: LoadedMap | null) {
    if (disposed) return
    const expected = generation + 1; withdraw()
    if (disposed || generation !== expected || !map) return
    const version = isSpatialV5ReferenceClaim(map) ? 5 : isSpatialV4ReferenceClaim(map) ? 4 : 0
    if (version && labelsVersion !== version) {
      labels.dispose(); labels = version === 5 ? createSpatialV5Labels() : createSpatialV4Labels(); labelsVersion = version
    }
    const render = () => {
     if (disposed || generation !== expected) return
     let canvas: HTMLCanvasElement | null = null
     try {
      const binding = requireReferenceScene(map), ticket = generation
      canvas = document.createElement('canvas'); canvas.width = map.widthPx; canvas.height = map.heightPx
      const context = canvas.getContext('2d'); if (!context) throw new Error('Reference overview unavailable')
      drawReferenceOverview(context, binding)
      if (version) labels.draw(context)
      if (disposed || generation !== ticket) { canvas.width = canvas.height = 0; return }
      const raster = { canvas, ticket, pins: { width: map.widthPx, height: map.heightPx,
        mapId: map.map.map_id, mapRevision: map.map.map_revision, mapHash: map.map.content_hash, geometryHash: map.map.geometry_hash,
        themeId: map.theme.theme_id, themeRevision: map.theme.theme_revision, themeHash: map.theme.content_hash } }
      if (encoding) pending = raster
      else encode(raster)
     } catch { if (canvas) canvas.width = canvas.height = 0 }
    }
    if (version) void labels.prepare().then(render, () => { /* Exact font unavailable: retain schematic/list fallback. */ })
    else render()
  }
  return { read: () => snapshot,
    subscribe(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } },
    replace, dispose() { if (disposed) return; disposed = true; labels.dispose(); withdraw(); listeners.clear() } }
}
