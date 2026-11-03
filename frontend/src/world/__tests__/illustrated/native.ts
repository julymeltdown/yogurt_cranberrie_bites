import { illustratedReleases } from './fixtures'
import { contentHash, sha256 } from '../../map/json'
import { loadMap, createAssetResources, type LoadedMap, type AssetResources } from '../../map/resources'
import { loadPhaserEngine } from '../../scenes/MapScene'
import { createWorldRuntime, type EngineLease } from '../../runtime'
import { sceneryResources, sceneryDepth } from '../../illustrated/model'
import type { ThemeManifest } from '../../map/manifest'
const { map, themes, avatar } = await illustratedReleases()
const host = document.querySelector<HTMLElement>('#world')!, status = document.querySelector('#status')!
const allow = [{ path: map.tilemap.path!, sha256: map.tilemap.sha256!, kind: 'json' as const },
  ...themes.flatMap(t => [{ path: t.texture.path!, sha256: t.texture.sha256!, kind: 'png' as const }, ...sceneryResources(t)])]
const requests: string[] = []
const resources = createAssetResources({ origin: location.origin, allow, fetch: async (input, init) => { requests.push(String(input)); return fetch(input, init) } })
const images = new Set<HTMLImageElement>()
let engine!: EngineLease, current!: LoadedMap, mounts = 0, disposals = 0
const load = async (theme: ThemeManifest, signal: AbortSignal, reader: AssetResources = resources, mapValue = map) => {
  const result = await loadMap({ mapText: JSON.stringify(mapValue), themeText: JSON.stringify(theme),
    pin: { mapHash: mapValue.content_hash, themeHash: theme.content_hash, avatar }, resources: reader }, signal)
  for (const asset of result.scenery!.assets) images.add(asset.image)
  const dispose = result.dispose
  return { ...result, dispose() { dispose(); for (const asset of result.scenery!.assets) images.delete(asset.image) } }
}
const runtime = createWorldRuntime({ host, load: async signal => current = await load(themes[0]!, signal), importEngine: async () => {
  const factory = await loadPhaserEngine()
  return (host, map) => { mounts++; engine = factory(host, map); const destroy = engine.destroy; return { ...engine, destroy() { disposals++; destroy() } } }
} })
await runtime.start()
if (runtime.read().phase !== 'ready') throw Error(JSON.stringify(runtime.read()))
const frame = () => new Promise<void>(r => requestAnimationFrame(() => requestAnimationFrame(() => r())))
const markerCanvas = document.createElement('canvas'); markerCanvas.width = 320; markerCanvas.height = 256
const markerContext = markerCanvas.getContext('2d')!
markerContext.fillStyle = '#FF00FF'; markerContext.fillRect(25, 32, 14, 28)
const markerTexture = engine.avatarScene!.texture(markerCanvas), markerSprite = engine.avatarScene!.sprite(markerTexture)
let marker: { x: number; y: number } | null = null
function moveMarker(x: number, y: number) { marker = { x, y }; markerSprite.update(x, y, 0, sceneryDepth(y, current.heightPx)) }
function focus(index: number) {
  const p = map.geometry.interactions[index]!
  engine.camera!.center({ x: (p.cell_x + .5) * 32, y: (p.cell_y + .5) * 32 }, 1)
}
async function state() {
  await frame()
  const canvas = engine.camera!.surface()!, camera = engine.camera!.read()!
  const measured = engine.readScenery!()
  return { phase: runtime.read().phase, theme: runtime.readSkin(), mounts, disposals, canvases: host.querySelectorAll('canvas').length,
    skinTextures: measured.textures, textureBytes: measured.decodedBytes, measured, decodedImages: images.size,
    transferBytes: current.scenery!.transferBytes, objects: current.scenery!.manifest.objects.length,
    geometry: map.geometry_hash, camera, signature: await sha256(new Uint8Array(canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data.buffer)), requests }
}
async function parity() {
  await frame()
  const canvas = engine.camera!.surface()!, camera = engine.camera!.read()!, expected = document.createElement('canvas')
  expected.width = canvas.width; expected.height = canvas.height
  const ctx = expected.getContext('2d')!; ctx.imageSmoothingEnabled = false; ctx.fillStyle = current.theme.palette.background; ctx.fillRect(0, 0, canvas.width, canvas.height)
  const s = current.scenery!, ground = s.assets[0]!
  ctx.drawImage(ground.image, -camera.scrollX, -camera.scrollY)
  const rows = s.manifest.objects.map(o => ({ depth: sceneryDepth(o.depth_y, current.heightPx), draw() {
    const image = s.assets.find(a => a.descriptor.id === o.atlas_id)!.image, f = o.frame
    ctx.drawImage(image, f.x, f.y, f.width, f.height, o.x - camera.scrollX, o.y - camera.scrollY, o.width, o.height)
  } }))
  if (marker) { const p = marker; rows.push({ depth: sceneryDepth(p.y, current.heightPx), draw() { ctx.drawImage(markerCanvas, 0, 0, 64, 64, p.x - 32 - camera.scrollX, p.y - 60 - camera.scrollY, 64, 64) } }) }
  rows.sort((a, b) => a.depth - b.depth).forEach(row => row.draw())
  const a = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data, b = ctx.getImageData(0, 0, canvas.width, canvas.height).data
  let different = 0, magenta = 0, maxDifference = 0
  const examples: {index:number;actual:number;expected:number}[] = []
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) { different++; maxDifference = Math.max(maxDifference, Math.abs(a[i]! - b[i]!)); if (examples.length < 12) examples.push({index:i,actual:a[i]!,expected:b[i]!}) }
  for (let i = 0; i < a.length; i += 4) if (a[i] === 255 && a[i + 1] === 0 && a[i + 2] === 255) magenta++
  return { different, maxDifference, examples, channels: a.length, magenta }
}
// Capture the exact successfully loaded candidate without creating a second renderer owner.
let awaitLoaded: LoadedMap | null = null
async function select(index: number) {
  await runtime.replaceSkin(async signal => { const next = await load(themes[index]!, signal); awaitLoaded = next; return next })
  current = awaitLoaded!; status.textContent = current.theme.theme_id
}
let index = 0
document.querySelector('#switch')!.addEventListener('click', () => { void select(index = (index + 1) % 3) })
focus(1); status.textContent = current.theme.theme_id
const api = { state, parity, focus, select, marker(x: number, y: number) { moveMarker(x, y); engine.camera!.center({ x, y }, 1) },
  async atomic() {
    const next = themes.find(t => t.theme_id !== current.theme.theme_id)!, before = await state()
    let release!: () => void
    const reader: AssetResources = { async read(resource, signal) {
      if (resource.path === next.scenery!.ground.path) await new Promise<void>(r => { release = r })
      return resources.read(resource, signal)
    } }
    let candidate!: LoadedMap
    const pending = runtime.replaceSkin(async signal => candidate = await load(next, signal, reader))
    while (!release) await new Promise(r => setTimeout(r, 5))
    const held = await state(); release(); await pending; current = candidate
    const measured = engine.readScenery!()
    return { before, held, after: await state(), peakTextures: measured.peakTextures, peakBytes: measured.peakDecodedBytes }
  },
  async negatives() {
    const rejected: { kind: string; reason: string }[] = []
    for (const kind of ['digest', 'dimension', 'budget', 'geometry', 'missing-binding']) {
      const theme = structuredClone(themes[1]!), m = structuredClone(map)
      if (kind === 'dimension') Object.assign(theme.scenery!.atlases[1]!, { width_px: 513 })
      if (kind === 'budget') Object.assign(theme.scenery!.ground, { bytes: 2_097_152 })
      if (kind === 'geometry') m.geometry.collision_grid[0]![0] = 0
      theme.content_hash = await contentHash(theme)
      const reader = { read: async (resource: Parameters<AssetResources['read']>[0], signal: AbortSignal) => {
        if (kind === 'missing-binding' && resource.path === theme.scenery!.ground.path) throw Error('Missing qualified binding')
        const bytes = await resources.read(resource, signal)
        if (kind === 'digest' && resource.path === theme.scenery!.ground.path) bytes[bytes.length - 1]! ^= 1
        return bytes
      } }
      try { await runtime.replaceSkin(signal => load(theme, signal, reader, m)); throw Error('ACCEPTED ' + kind) }
      catch (error) { if (String(error).includes('ACCEPTED')) throw error; rejected.push({ kind, reason: String(error) }) }
    }
    return rejected
  },
  async cancelled() {
    const before = await state(), controller = new AbortController(); let release!: () => void
    const reader: AssetResources = { async read(resource, signal) {
      if (resource.path === themes[1]!.scenery!.atlases[0]!.path) await new Promise<void>(r => { release = r })
      return resources.read(resource, signal)
    } }
    const pending = runtime.replaceSkin(signal => load(themes[1]!, signal, reader), controller.signal)
    const caught = pending.catch(() => {})
    while (!release) await new Promise(r => setTimeout(r, 5))
    controller.abort(); release(); await caught; await frame()
    return { before, after: await state() }
  },
  async dispose() { markerSprite.dispose(); markerTexture.dispose(); runtime.dispose(); await runtime.settled(); return { canvases: host.querySelectorAll('canvas').length, decodedImages: images.size, disposals, sceneTextures: engine.readScenery!().textures } },
}
Object.assign(window, { illustrated: api })
