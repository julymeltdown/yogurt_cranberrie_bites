import type Phaser from 'phaser'
import type { LoadedMap } from '../../map/resources'
import { photo2DisplayDepth, type Photo2SceneDisplay } from './model'
import { createPhaserMuralScene } from '../../mural/phaser'
import type { MuralSceneObject, MuralScenePort, MuralSceneTexture } from '../../mural/sink'
import type { Point, Rect } from '../../../features/parade-ink/world-hud/photo2-world-display-v1'

type MuralAnchor = Rect & { readonly depth: number }
/** Clip each native mesh triangle to the real mural anchor before projecting its pixels. */
export function photo2MuralPlane(display: Photo2SceneDisplay, anchor: Rect) {
  if (![anchor.x, anchor.y, anchor.width, anchor.height].every(Number.isFinite) || anchor.width <= 0 || anchor.height <= 0
    || anchor.width > 2048 || anchor.height > 2048 || [[0, 0], [1, 0], [1, 1], [0, 1]].some(([x, y]) =>
      !display.projection.forward({ x: anchor.x + x! * anchor.width, y: anchor.y + y! * anchor.height }))) throw new Error('Invalid mural projection anchor')
  const clip = (polygon: Point[], axis: 'x' | 'y', edge: number, greater: boolean) => {
    const result: Point[] = []
    for (let i = 0; i < polygon.length; i++) {
      const start = polygon[i]!, end = polygon[(i + 1) % polygon.length]!
      const inside = (point: Point) => greater ? point[axis] >= edge : point[axis] <= edge
      if (inside(start)) result.push(start)
      if (inside(start) !== inside(end)) {
        const fraction = (edge - start[axis]) / (end[axis] - start[axis])
        result.push({ x: start.x + (end.x - start.x) * fraction, y: start.y + (end.y - start.y) * fraction })
      }
    }
    return result
  }
  const triangles: { native: readonly [Point, Point, Point]; display: readonly [Point, Point, Point] }[] = []
  const xs = display.mesh.columns, ys = display.mesh.rows
  for (let y = 0; y < ys.length - 1; y++) for (let x = 0; x < xs.length - 1; x++) {
    const a = { x: xs[x]!, y: ys[y]! }, b = { x: xs[x + 1]!, y: ys[y]! }
    const c = { x: xs[x]!, y: ys[y + 1]! }, d = { x: xs[x + 1]!, y: ys[y + 1]! }
    for (const triangle of [[a, b, d], [a, d, c]]) {
      let polygon = clip(triangle, 'x', anchor.x, true)
      polygon = clip(polygon, 'x', anchor.x + anchor.width, false)
      polygon = clip(polygon, 'y', anchor.y, true)
      polygon = clip(polygon, 'y', anchor.y + anchor.height, false)
      for (let i = 1; i < polygon.length - 1; i++) {
        const native = [polygon[0]!, polygon[i]!, polygon[i + 1]!] as const
        const [p, q, r] = native
        if (Math.abs((q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)) < 1e-9) continue
        const projected = native.map(point => display.projection.forward(point))
        if (projected.some(point => point === null)) throw new Error('Mural outside Photo2 projection')
        triangles.push({ native, display: projected as [Point, Point, Point] })
      }
    }
  }
  if (!triangles.length) throw new Error('Mural projection unavailable')
  const points = triangles.flatMap(t => t.display)
  const x = Math.floor(Math.min(...points.map(p => p.x))), y = Math.floor(Math.min(...points.map(p => p.y)))
  const width = Math.ceil(Math.max(...points.map(p => p.x))) - x, height = Math.ceil(Math.max(...points.map(p => p.y))) - y
  if (width < 1 || height < 1 || width > 2048 || height > 2048) throw new Error('Mural projection exceeds raster bound')
  return { triangles, bounds: { x, y, width, height } }
}

/** Pixel centers use the same affine triangles as input inversion; no clip-antialias seams. */
export function paintPhoto2Mural(plane: ReturnType<typeof photo2MuralPlane>, anchor: Rect, rgba: Uint8ClampedArray, output: Uint8ClampedArray) {
  const bounds = plane.bounds
  if (rgba.length !== 65536 || output.length !== bounds.width * bounds.height * 4) throw new Error('Invalid mural raster')
  output.fill(0)
  for (const triangle of plane.triangles) {
    const [a, b, c] = triangle.display, [p, q, r] = triangle.native
    const area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    const left = Math.max(0, Math.floor(Math.min(a.x, b.x, c.x) - bounds.x))
    const right = Math.min(bounds.width, Math.ceil(Math.max(a.x, b.x, c.x) - bounds.x))
    const top = Math.max(0, Math.floor(Math.min(a.y, b.y, c.y) - bounds.y))
    const bottom = Math.min(bounds.height, Math.ceil(Math.max(a.y, b.y, c.y) - bounds.y))
    for (let y = top; y < bottom; y++) for (let x = left; x < right; x++) {
      const px = bounds.x + x + .5, py = bounds.y + y + .5
      const u = ((b.x - px) * (c.y - py) - (b.y - py) * (c.x - px)) / area
      const v = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) / area
      const w = 1 - u - v
      if (Math.min(u, v, w) < -1e-9) continue
      const nativeX = u * p.x + v * q.x + w * r.x, nativeY = u * p.y + v * q.y + w * r.y
      const sx = Math.max(0, Math.min(127, Math.floor((nativeX - anchor.x) * 128 / anchor.width)))
      const sy = Math.max(0, Math.min(127, Math.floor((nativeY - anchor.y) * 128 / anchor.height)))
      const source = (sy * 128 + sx) * 4, target = (y * bounds.width + x) * 4
      for (let channel = 0; channel < 4; channel++) output[target + channel] = rgba[source + channel]!
    }
  }
}

/** One bounded display raster; mesh triangles preserve the camera inverse used for hits. */
function createProjectedMuralScene(scene: Phaser.Scene, anchor: MuralAnchor, key: string, display: Photo2SceneDisplay): MuralScenePort {
  const plane = photo2MuralPlane(display, anchor), bounds = plane.bounds
  let owned: MuralSceneTexture | null = null
  return {
    texture() {
      if (owned || scene.textures.exists(key)) throw new Error('Mural texture already owned')
      const texture = scene.textures.createCanvas(key, bounds.width, bounds.height)
      if (!texture) throw new Error('Mural texture unavailable')
      texture.setFilter(1)
      let disposed = false
      const lease: MuralSceneTexture = {
        replace(rgba) {
          if (disposed || rgba.length !== 65536) throw new Error('Mural texture unavailable')
          const context = texture.getContext()
          const pixels = context.createImageData(bounds.width, bounds.height)
          paintPhoto2Mural(plane, anchor, rgba, pixels.data); context.putImageData(pixels, 0, 0)
          texture.refresh()
        },
        dispose() { if (disposed) return; disposed = true; owned = null; scene.textures.remove(key) },
      }
      owned = lease; return lease
    },
    object(texture) {
      if (texture !== owned || !owned) throw new Error('Unowned mural texture')
      const foot = display.projection.forward({ x: anchor.x + anchor.width / 2, y: anchor.y + anchor.height })
      if (!foot) throw new Error('Mural depth unavailable')
      const image = scene.add.image(bounds.x, bounds.y, key).setOrigin(0).setDepth(photo2DisplayDepth(display, foot.y))
      let disposed = false
      return { dispose: () => { if (disposed) return; disposed = true; image.destroy() } }
    },
    onFrame(listener) { scene.events.on('update', listener); return () => { scene.events.off('update', listener) } },
  }
}

/** Stable public mural leases retain their RGBA while the visual profile changes atomically. */
export function createProfiledMuralScene(scene: Phaser.Scene, anchor: MuralAnchor, key: string | undefined,
  current: () => Photo2SceneDisplay | null, refreshers: Set<() => void>): MuralScenePort {
  const textureKey = key ?? 'mcr-mural'
  if (![anchor.x, anchor.y, anchor.width, anchor.height, anchor.depth].every(Number.isFinite) || anchor.width <= 0 || anchor.height <= 0
    || anchor.width > 2048 || anchor.height > 2048 || !/^[a-zA-Z0-9_-]{1,80}$/.test(textureKey)) throw new Error('Invalid mural anchor')
  let profile = current(), raster: MuralSceneTexture | null = null, publicTexture: MuralSceneTexture | null = null
  let rgba: Uint8ClampedArray | null = null
  const make = () => profile ? createProjectedMuralScene(scene, anchor, textureKey, profile) : createPhaserMuralScene(scene, anchor, textureKey)
  let port = make()
  const objects = new Map<MuralSceneObject, MuralSceneObject>()
  const refresh = () => {
    const next = current()
    if (next === profile || !publicTexture) return
    for (const object of objects.values()) object.dispose()
    raster?.dispose(); raster = null; profile = next; port = make(); raster = port.texture()
    if (rgba) raster.replace(rgba)
    for (const object of objects.keys()) objects.set(object, port.object(raster))
  }
  return {
    texture() {
      if (publicTexture) throw new Error('Mural texture already owned')
      raster = port.texture(); refreshers.add(refresh)
      const lease: MuralSceneTexture = {
        replace(pixels) {
          if (publicTexture !== lease || pixels.length !== 65536) throw new Error('Mural texture unavailable')
          rgba = new Uint8ClampedArray(pixels); refresh(); raster!.replace(rgba)
        },
        dispose() {
          if (publicTexture !== lease) return
          publicTexture = null; refreshers.delete(refresh)
          for (const object of objects.values()) object.dispose()
          objects.clear(); raster?.dispose(); raster = null; rgba = null
        },
      }
      publicTexture = lease; return lease
    },
    object(texture) {
      if (texture !== publicTexture || !raster) throw new Error('Unowned mural texture')
      const lease: MuralSceneObject = { dispose() { objects.get(lease)?.dispose(); objects.delete(lease) } }
      objects.set(lease, port.object(raster)); return lease
    },
    onFrame(listener) { scene.events.on('update', listener); return () => { scene.events.off('update', listener) } },
    onClose(listener) {
      let closed = false
      const close = () => { if (closed) return; closed = true; listener() }
      scene.events.once('shutdown', close); scene.events.once('destroy', close)
      return () => { scene.events.off('shutdown', close); scene.events.off('destroy', close) }
    },
  }
}

/** Ordinary Canvas-compatible images over a producer-painted display plane.
 * Every actor/input consumer must use display.projection; this does not move physics.
 * Nonphysical board paint has no Phaser input handler or proximity registration.
 */
export function preparePhoto2SceneSkin(scene: Phaser.Scene, map: LoadedMap, display: Photo2SceneDisplay,
  prefix: string, nearest: Phaser.Textures.FilterMode, uploaded: () => void, alive: () => boolean) {
  const objects: Phaser.GameObjects.Image[] = [], keys: string[] = []
  let removed = false, graphics: Phaser.GameObjects.Graphics | null = null, mask: Phaser.Display.Masks.GeometryMask | null = null
  const check = () => { if (removed || !alive()) throw new Error('Photo2 scene unavailable') }
  const remove = () => {
    if (removed) return
    removed = true
    for (const object of objects) { try { object.destroy() } catch { /* Release independent owned objects. */ } }
    try { mask?.destroy() } catch { /* Continue releasing storage. */ }
    try { graphics?.destroy() } catch { /* Continue releasing storage. */ }
    for (const key of keys) { try { if (scene.textures.exists(key)) scene.textures.remove(key) } catch { /* Continue cleanup. */ } }
  }
  try {
    check()
    if (!map.scenery) throw new Error('Photo2 assets unavailable')
    const textures = new Map<string, string>()
    for (const asset of map.scenery.assets) {
      const key = `${prefix}-${asset.descriptor.id}`
      keys.push(key)
      const texture = scene.textures.addImage(key, asset.image)
      if (!texture) throw new Error('Photo2 texture unavailable')
      texture.setFilter(nearest); textures.set(asset.descriptor.id, key); uploaded(); check()
    }
    const bounds = display.groundBounds
    graphics = scene.make.graphics({ x: 0, y: 0 }, false)
    graphics.fillStyle(0xffffff).fillRect(bounds.x, bounds.y, bounds.width, bounds.height)
    mask = graphics.createGeometryMask(); check()
    const image = (x: number, y: number, key: string, frame?: string) => {
      check()
      const object = scene.add.image(x, y, key, frame); objects.push(object)
      object.setOrigin(0).setVisible(false).setMask(mask!); check()
      return object
    }
    const groundKey = textures.get(map.scenery.manifest.ground.id)
    if (!groundKey) throw new Error('Photo2 ground unavailable')
    image(bounds.x, bounds.y, groundKey).setDepth(0)
    for (const prop of [...display.frames].sort((a, b) => a.id < b.id ? -1 : a.id > b.id ? 1 : 0)) {
      const key = textures.get(prop.atlasId), f = prop.atlasRect
      if (!key || !scene.textures.get(key).add(prop.id, 0, f.x, f.y, f.width, f.height)) throw new Error('Photo2 prop frame unavailable')
      image(prop.displayFoot.x - prop.width / 2, prop.displayFoot.y - prop.height, key, prop.id)
        .setDisplaySize(prop.width, prop.height).setDepth(photo2DisplayDepth(display, prop.displayFoot.y))
    }
    for (const board of display.boards) {
      const key = textures.get(board.atlasId), f = board.atlasRect, r = board.displayRect
      if (!key || !scene.textures.get(key).add(board.id, 0, f.x, f.y, f.width, f.height)) throw new Error('Photo2 board frame unavailable')
      image(r.x, r.y, key, board.id).setDisplaySize(r.width, r.height).setDepth(photo2DisplayDepth(display, r.y + r.height))
    }
    check()
    return { show() {
      try { check(); for (const object of objects) { object.setVisible(true); check() } }
      catch (error) { remove(); throw error }
    }, remove }
  } catch (error) { remove(); throw error }
}
