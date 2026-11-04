// TEST ONLY adaptation of owned native.ts at72685ad9; actual new source/C06/Phaser, no live producer.
import { spatialV5Fixture, spatialV5FixtureResources } from './fixture'
import source from '../../spatial-v5/source-inputs.json'
import landscape from '../../spatial-v5/map-landscape-source.json'
// @ts-expect-error Independent published artist JavaScript rendering oracle.
import { drawLabels } from './label-reference.js'
import { spatialV5ThemeIds, type SpatialV5ThemeId } from '../../spatial-v5/model'
import { loadReferenceScene, requireReferenceScene, createReferenceSceneOverview } from '../..'
import { loadPhaserEngine } from '../../../scenes/MapScene'
import { createWorldRuntime, type EngineLease } from '../../../runtime'
import { decodeImage, type LoadedMap } from '../../../map/resources'
import { contentHash, sha256 } from '../../../map/json'
import { createCollision } from '../../../input/simulation'
import { nearbyTargets } from '../../../interaction/selection'
import { createSessionController } from '../../../../features/auth/model/session'
import { isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { createWholeCharacterRuntime } from '../../../../entities/avatar/whole-character'
import { createWholeCharacterWorldRenderer } from '../../../avatar/whole-character'
import { sceneryDepth } from '../../../illustrated/model'
import { validate } from '../../../network/transport/contract'
import type { Snapshot } from '../../../network/WorldTransport'
import type { ReplicationView } from '../../../network/replication'

let fixture = await spatialV5Fixture()
const allFixtures = new Map(await Promise.all(spatialV5ThemeIds.map(async id => [id, await spatialV5Fixture(id)] as const)))
const eventId = '00000000-0000-4000-8000-000000000002', userId = '00000000-0000-4000-8000-000000000001'
const auth = createSessionController({ eventId, cookiePolicy: isolatedCookiePolicy(), fetch: async () => { throw Error('No live Auth in fixture') } })
auth.session.replace(auth.session.read().generation, { userId, eventId, accessToken: 'TEST_ONLY_reference_scene' })
const abort = new AbortController(), requests: string[] = [], decoded = new Set<HTMLImageElement>()
const resources = spatialV5FixtureResources(location.origin, { signal: abort.signal,
  fetch: async (input, init) => { requests.push(String(input)); return fetch(input, init) } })
const decode: typeof decodeImage = async (bytes, signal) => {
  const lease = await decodeImage(bytes, signal); decoded.add(lease.image)
  return { image: lease.image, dispose() { lease.dispose(); decoded.delete(lease.image) } }
}
const input = () => ({ mapText: JSON.stringify(fixture.map), themeText: JSON.stringify(fixture.theme), pin: fixture.pin, resources: resources.resources, decode })
let current!: LoadedMap, engine!: EngineLease, mounts = 0, destroys = 0
const overview = createReferenceSceneOverview(), host = document.querySelector<HTMLElement>('#world')!
const runtime = createWorldRuntime({ host, createOverview: () => overview, load: async signal => current = await loadReferenceScene(input(), signal),
  importEngine: async () => { const factory = await loadPhaserEngine(); return (host, map) => {
    mounts++; engine = factory(host, map); return { ...engine, destroy() { destroys++; engine.destroy() } }
  } } })
await runtime.start(abort.signal)
if (runtime.read().phase !== 'ready') throw Error(JSON.stringify(runtime.read()))
engine.camera!.center({ x: 452, y: 305 }, 1)
const collision = createCollision(fixture.map.geometry, { speedPxPerSecond: 100, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 48 })
const positions = [[440, 389], [488, 389], [536, 389], [440, 437], [488, 437], [536, 437]].map(([x, y]) => ({ x: x!, y: y! }))
for (const p of positions) if (!collision.walkable(p)) throw Error('Fixture roster must use clear canonical feet')
const owner = createWholeCharacterRuntime({ privacy: auth.privacy, manifest: fixture.avatar, pin: fixture.avatar, origin: location.origin })
const snapshot = validate<Snapshot>('Snapshot', { protocolVersion: '1.0.0', type: 'snapshot', streamId: '00000000-0000-4000-8000-000000000004', streamSeq: 0,
  roomEpoch: '00000000-0000-4000-8000-000000000005', serverTick: 1, eventId, roomId: '00000000-0000-4000-8000-000000000003',
  mapId: fixture.map.map_id, mapRevision: fixture.map.map_revision, avatarManifestId: fixture.avatar.manifest_id, avatarManifestVersion: 1,
  cause: 'INITIAL', requestId: null, selfUserId: userId, ackInputSeq: 0, accessExpiresAt: '2026-11-05T00:00:00Z',
  players: fixture.avatar.presets.map((p, i) => ({ userId: i === 0 ? userId : `00000000-0000-4000-8000-00000000001${i}`,
    displayName: `Selected ${i + 1}`, position: positions[i], facing: 'up', moving: true, avatar: { revision: 1, config: p.config } })) })
let view: ReplicationView = { scope: auth.privacy.capture(), phase: 'live', visible: true, baseline: snapshot,
  entities: snapshot.players.map(player => ({ player, self: player.userId === userId, position: player.position })),
  pendingInputs: 0, ackInputSeq: 0, spatialPrediction: 'unavailable', problem: null }
const avatars = createWholeCharacterWorldRenderer({ resolveRuntime: () => owner }).mount({ host, map: current, camera: engine.camera!, avatarScene: engine.avatarScene!,
  release: { avatarManifest: fixture.avatar, resources: { eventId } } as never, signal: abort.signal, sample: () => view })
avatars.replace(view)
const frame = () => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
function state() { return { phase: runtime.read().phase, mounts, destroys, canvases: host.querySelectorAll('canvas').length, decodedImages: decoded.size,
  scene: engine.readScenery!(), avatars: engine.avatarScene!.read(), geometry: fixture.map.geometry_hash, mapId: fixture.map.map_id, theme: fixture.theme.theme_id,
  camera: engine.camera!.read(), requests, cache: resources.diagnostics(), overview: overview.read(), owner: owner.diagnostics() } }
async function signature() {
  await frame(); const canvas = engine.camera!.surface()!
  return sha256(new Uint8Array(canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data.buffer))
}
async function parity() {
  const leases = []
  const roster = []
  if (view.visible && owner.isCurrent()) for (const entity of view.entities) {
    const lease = await owner.acquire(entity.player.avatar.config, { signal: abort.signal })
    // Independently crop the source cell before fractional scaling, as the production texture upload does.
    const render = lease.read()!, cell = document.createElement('canvas')
    cell.width = cell.height = 64
    const crop = cell.getContext('2d')!; crop.imageSmoothingEnabled = false
    crop.drawImage(render.image, render.source.x, 0, 64, 64, 0, 0, 64, 64)
    leases.push(lease); roster.push({ entity, cell })
  }
  // Finish all asynchronous lease work before observing and composing one frame.
  const camera = engine.camera!.read()!, z = Math.fround(camera.zoom)
  await frame()
  const actual = engine.camera!.surface()!, canvas = document.createElement('canvas')
  canvas.width = actual.width; canvas.height = actual.height
  const ctx = canvas.getContext('2d')!
  // Independent source composition using the pinned Phaser 3.90 Canvas transform arithmetic.
  // Camera and sprite matrices round each stored component through Float32Array, including zoom.
  const tx = Math.fround(Math.floor(camera.width / 2 + .5) - camera.width / 2 * z)
  const ty = Math.fround(Math.floor(camera.height / 2 + .5) - camera.height / 2 * z)
  const s = current.scenery!, rows: { depth: number; draw(): void }[] = []
  const transform = (x: number, y: number, sx = 1, sy = 1) => { ctx.setTransform(
    Math.fround(z * Math.fround(sx)), 0, -0, Math.fround(z * Math.fround(sy)),
    Math.fround(z * Math.fround(Math.fround(x) - camera.scrollX) + tx),
    Math.fround(z * Math.fround(Math.fround(y) - camera.scrollY) + ty)) }
  ctx.save(); ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.globalAlpha = 1; ctx.globalCompositeOperation = 'source-over'
  ctx.imageSmoothingEnabled = false; ctx.fillStyle = current.theme.palette.background; ctx.fillRect(0, 0, canvas.width, canvas.height)
  function mask() {
    ctx.save(); ctx.save(); transform(0, 0)
    ctx.beginPath(); ctx.rect(0, 0, current.widthPx, current.heightPx)
    ctx.restore(); ctx.clip()
  }
  function paintScenery(x: number, y: number, sx: number, sy: number, draw: () => void) {
    // Phaser 3.90 applies GeometryMask once around each child and again in batchSprite.
    ctx.save(); ctx.setTransform(z, 0, -0, z, tx, ty); mask()
    ctx.save(); transform(x, y, sx, sy); mask(); draw()
    ctx.restore(); ctx.restore(); ctx.restore(); ctx.restore()
  }
  paintScenery(0, 0, 1, 1, () => ctx.drawImage(s.assets[0]!.image, 0, 0, current.widthPx, current.heightPx, 0, 0, current.widthPx, current.heightPx))
  for (const o of s.manifest.objects) rows.push({ depth: sceneryDepth(o.depth_y, current.heightPx), draw() {
    const f = o.frame
    paintScenery(o.x, o.y, o.width / f.width, o.height / f.height, () =>
      ctx.drawImage(s.assets.find(a => a.descriptor.id === o.atlas_id)!.image, f.x, f.y, f.width, f.height, 0, 0, f.width, f.height))
  } })
  for (const { entity, cell } of roster) {
    rows.push({ depth: sceneryDepth(entity.position.y, current.heightPx), draw() {
      transform(entity.position.x, entity.position.y)
      ctx.drawImage(cell, 0, 0, 64, 64, -32, -60, 64, 64)
    } })
  }
  rows.sort((a, b) => a.depth - b.depth).forEach(r => r.draw()); leases.forEach(l => l.dispose())
  roster.forEach(({ cell }) => { cell.width = cell.height = 0 })
  ctx.restore()
  const a = actual.getContext('2d')!.getImageData(0, 0, actual.width, actual.height).data, b = ctx.getImageData(0, 0, canvas.width, canvas.height).data
  let different = 0, max = 0
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) { different++; max = Math.max(max, Math.abs(a[i]! - b[i]!)) }
  if (different) Object.assign(window, { referenceExpected: canvas.toDataURL() })
  canvas.width = canvas.height = 0
  return { different, max, channels: a.length }
}
async function mapParity() {
  const pins=overview.read();if(!pins)throw Error('Overview not ready')
  const img=new Image();img.src=pins.imageUrl;await img.decode()
  const actual=document.createElement('canvas'),expected=document.createElement('canvas');actual.width=expected.width=768;actual.height=expected.height=1296
  const a=actual.getContext('2d')!,b=expected.getContext('2d')!;a.drawImage(img,0,0);b.imageSmoothingEnabled=false
  const scene=current.scenery!,data=source.scenes[fixture.theme.theme_id as SpatialV5ThemeId],textures=new Map(scene.assets.map(x=>[x.descriptor.id,x.image]))
  b.drawImage(textures.get(scene.manifest.ground.id)!,0,0);b.beginPath();source.plaza_parcel.forEach(([x,y],i)=>i?b.lineTo(x!,y!):b.moveTo(x!,y!));b.closePath();b.fillStyle='#7a8742';b.fill();b.strokeStyle='#b4a27d';b.lineWidth=6;b.stroke()
  const replacements=new Set(source.overview_representations.flatMap(p=>'replaces_prop_id'in p?[p.replaces_prop_id]:[])),frames=data.frames as Record<string,{x:number;y:number;width:number;height:number}>
  const rows=[...data.scenery.objects.filter(o=>!replacements.has(o.id)),...source.overview_representations.map(o=>({...o,atlas_id:data.scenery.atlases[0]!.id,frame:frames[o.frame]!})),...landscape.decorations.map(o=>({...o,atlas_id:data.scenery.atlases[0]!.id,frame:frames[o.frame]!}))]
  for(const o of rows.sort((a,b)=>a.depth_y-b.depth_y)){const f=o.frame;b.drawImage(textures.get(o.atlas_id)!,f.x,f.y,f.width,f.height,o.x,o.y,o.width,o.height)}
  drawLabels(b, 1)
  const x=a.getImageData(0,0,768,1296).data,y=b.getImageData(0,0,768,1296).data;let different=0;for(let i=0;i<x.length;i++)if(x[i]!==y[i])different++
  actual.width=actual.height=expected.width=expected.height=0;return{different,channels:x.length,pins}
}
const api = { state, parity, mapParity, signature,
  async occlusion() {
    const previous = view, rows = []
    try {
      for (const foot of [{ x: 344, y: 344 }, { x: 344, y: 376 }]) {
        if (!collision.walkable(foot)) throw Error('Occlusion foot must remain walkable')
        view = { ...previous, entities: previous.entities.map(e => e.self ? { ...e, position: foot } : e) }
        avatars.replace(view); await frame()
        rows.push({ foot, pixels: await parity(), signature: await signature() })
      }
    } finally { view = previous; avatars.replace(view); await frame() }
    return rows
  },
  async era(id: SpatialV5ThemeId) {
    const previous=fixture;fixture=allFixtures.get(id)!
    let next!:LoadedMap
    try{await runtime.replaceSkin(async signal=>next=await loadReferenceScene(input(),signal));current=next}
    catch(error){fixture=previous;throw error}
    return state()
  },
  async replace() { const before = await signature(); let next!: LoadedMap; await runtime.replaceSkin(async signal => next = await loadReferenceScene(input(), signal)); current = next; return { before, after: await signature(), state: state() } },
  async failures() {
    const results = []
    for (const kind of ['hash', 'geometry', 'revision', 'theme', 'frame', 'depth', 'asset'] as const) {
      const m = structuredClone(fixture.map), t = structuredClone(fixture.theme), pin = structuredClone(fixture.pin)
      if (kind === 'hash') pin.themeHash = 'sha256:' + '0'.repeat(64)
      if (kind === 'geometry') m.geometry.collision_grid[0]![0] = 0
      if (kind === 'revision') m.map_revision++
      if (kind === 'theme') t.theme_id = 'REVENGE'
      if (kind === 'frame') Object.assign(t.scenery!.objects[0]!.frame, { x: t.scenery!.objects[0]!.frame.x + 1 })
      if (kind === 'depth') Object.assign(t.scenery!.objects[0]!, { depth_y: t.scenery!.objects[0]!.depth_y + 1 })
      if (kind === 'asset') Object.assign(t.scenery!.atlases[0]!, { sha256: 'sha256:' + '0'.repeat(64) })
      if (!['hash', 'geometry'].includes(kind)) { m.content_hash = await contentHash(m); t.map_revision = m.map_revision; t.map_content_hash = m.content_hash; t.content_hash = await contentHash(t); pin.mapHash = m.content_hash; pin.themeHash = t.content_hash }
      let accepted = false
      try { await runtime.replaceSkin(signal => loadReferenceScene({ ...input(), mapText: JSON.stringify(m), themeText: JSON.stringify(t), pin }, signal)); accepted = true } catch { /* Expected strict rejection. */ }
      if (accepted) throw Error('Accepted malformed candidate ' + kind); results.push(kind)
    }
    return { results, signature: await signature(), state: state() }
  },
  async cancelled() {
    const before = await signature(), controller = new AbortController(); let release!: () => void, candidate!: LoadedMap
    const work = runtime.replaceSkin(async signal => {
      candidate = await loadReferenceScene(input(), AbortSignal.any([signal, controller.signal]))
      await new Promise<void>(resolve => { release = resolve }); return candidate
    })
    while (!release) await new Promise(resolve => setTimeout(resolve, 5))
    // A superseding replacement fences the loaded-but-late candidate without a second game.
    let next!: LoadedMap; const newer = runtime.replaceSkin(async signal => next = await loadReferenceScene(input(), signal)); release()
    await Promise.allSettled([work, newer]); current = next
    return { before, after: await signature(), state: state() }
  },
  interactions() { return fixture.map.geometry.interactions.map(i => { const foot = collision.center(i.cell_x, i.cell_y); return {
    id: i.interaction_id, placeId: i.place_id, foot, walkable: collision.walkable(foot),
    nearby: nearbyTargets(current, foot, collision, 1).map(t => t.interactionId),
    routes: fixture.map.geometry.spawns.map(s => collision.route(collision.center(s.cell_x, s.cell_y), foot) !== null),
  } }) },
  async focus(index: number) { const i = fixture.map.geometry.interactions[index]!; engine.camera!.center(collision.center(i.cell_x, i.cell_y), 1); await frame(); return state() },
  async visibility(value: boolean) { view = { ...view, visible: value }; avatars.replace(view); runtime.setVisible(value); if (value) await frame(); return state() },
  privacy() { auth.privacy.changePolicy(auth.privacy.capture(), 'block'); overview.replace(null); abort.abort(); return state() },
  async dispose() { overview.dispose(); avatars.dispose(); abort.abort(); owner.dispose(); runtime.dispose(); await runtime.settled(); resources.dispose(); auth.dispose(); return state() },
  binding: () => ({ objects: requireReferenceScene(current).loaded.manifest.objects.length, overview: requireReferenceScene(current).overview.length }),
}
Object.assign(window, { referenceWorld: api })
