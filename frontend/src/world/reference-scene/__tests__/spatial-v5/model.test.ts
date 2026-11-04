// Adapted from published spatial-v4 tests at93ef0d89; new source/version pins, no edits to frozen tests.
import { expect, it, vi } from 'vitest'
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import source from '../../spatial-v5/source-inputs.json'
import tile from '../../spatial-v5/tile-pin.json'
import architecture from '../../spatial-v5/architecture-source.json'
import { spatialV5Fixture } from './fixture'
import { loadReferenceScene, requireReferenceScene, referenceSceneFor, getReferenceOverviewMetadata, spatialV5ThemeIds } from '../..'
import { contentHash } from '../../../map/json'
import { createCollision } from '../../../input/simulation'
import { nearbyTargets } from '../../../interaction/selection'
import type { DecodedImage } from '../../../map/resources'

const base = new URL('../../../../../public/', import.meta.url)
const bytes = async (path: string) => new Uint8Array(await readFile(new URL(path, base)))
const hash = (b: Uint8Array) => 'sha256:'+createHash('sha256').update(b).digest('hex')
async function setup(id: typeof spatialV5ThemeIds[number] = 'PARADE') {
  const f = await spatialV5Fixture(id), images = new Set<HTMLImageElement>()
  const read = vi.fn(async (r: { path: string }) => bytes(r.path))
  const decode = vi.fn(async (png: Uint8Array<ArrayBuffer>) => {
    const data = new DataView(png.buffer, png.byteOffset, png.byteLength), image = { naturalWidth: data.getUint32(16), naturalHeight: data.getUint32(20) } as HTMLImageElement
    images.add(image); return { image, dispose() { images.delete(image) } }
  })
  return { f, images, read, decode, input: { mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin, resources: { read }, decode } }
}
it('preserves frozen base pins and projects exact architecture resources with public Tiled bytes', async () => {
  for (const [path, pin] of Object.entries(source.inputs)) expect(hash(await bytes('world/reference-scene-v2/'+path)),path).toBe(pin)
  const seen = new Set<string>()
  for (const id of spatialV5ThemeIds) {
    const f = await spatialV5Fixture(id)
    expect(f.tiled).toEqual(await bytes(tile.path)); expect(hash(f.tiled)).toBe(tile.sha256)
    const s = f.theme.scenery!, resources = [s.ground, ...s.atlases]
    for (const r of resources) { const raw = await bytes(r.path); expect(hash(raw)).toBe(r.sha256); expect(raw.length).toBe(r.bytes) }
    expect(resources.reduce((a,r)=>a+r.bytes,0)).toBeLessThanOrEqual(2097152)
    expect(resources.reduce((a,r)=>a+r.width_px*r.height_px,0)).toBe(2043904)
    seen.add(s.atlases[0]!.sha256)
  }
  expect(seen.size).toBe(3)
})
it.each(spatialV5ThemeIds)('loads %s on one new canonical geometry, exact packed frames and separate MAP semantics', async id => {
  const s = await setup(id), map = await loadReferenceScene(s.input, new AbortController().signal), binding = requireReferenceScene(map)
  expect(referenceSceneFor(map)).not.toBeNull(); expect(binding.loaded.manifest).toEqual(architecture.scenes[id].scenery)
  expect(binding.loaded.manifest.objects).toHaveLength(34); expect(binding.overview).toHaveLength(49)
  expect(binding.worldCamera.y).toBe(13); expect(binding.loaded.manifest.objects.find(o=>o.id==='northern_hall')!.y).toBe(-94)
  expect(binding.overview.find(o=>o.id==='stage_venue_representation')).toMatchObject({x:185,y:163,width:300,height:210,depth_y:373})
  expect(binding.loaded.manifest.objects.some(o=>o.id==='stage_venue_representation')).toBe(false)
  expect(getReferenceOverviewMetadata({ geometryHash: map.map.geometry_hash, width:768,height:1296,themeId:id,mapId:map.map.map_id,mapRevision:map.map.map_revision })!.mapLabels[0]).toMatchObject({x:422,y:201})
  const collision = createCollision(map.map.geometry,{speedPxPerSecond:100,footWidthPx:8,footHeightPx:6,interactionRadiusPx:48})
  for (const interaction of map.map.geometry.interactions) {
    const foot=collision.center(interaction.cell_x,interaction.cell_y)
    expect(collision.walkable(foot)).toBe(true);expect(nearbyTargets(map,foot,collision,1).map(t=>t.interactionId)).toEqual([interaction.interaction_id])
    for(const spawn of map.map.geometry.spawns)expect(collision.route(collision.center(spawn.cell_x,spawn.cell_y),foot)).not.toBeNull()
  }
  expect(map.map.places).toEqual(source.places);expect(map.map.places.every(p=>p.place_kind==='VIRTUAL_CONCEPT'&&p.verification_state==='CONCEPT_ONLY'&&p.provenance.review_state==='UNREVIEWED')).toBe(true)
  expect(s.images.size).toBe(3);map.dispose();expect(s.images.size).toBe(0)
})
it.each(['old-scene','cross-era','old-grid','v8-geometry','v8-tile','v8-scene','foreign-id','revision','tiled-pin','compatibility-png','place-authority','frame','foot-depth','spawn'] as const)('rejects %s before any bytes or decode, even with recomputed manifest hashes',async kind=>{
  const s=await setup(),{map,theme}=s.f
  if(kind==='old-scene')theme.scenery=(await import('../../source-inputs.json')).default.scenery as NonNullable<typeof theme.scenery>
  if(kind==='cross-era')theme.scenery=structuredClone(source.scenes.DANGER.scenery) as NonNullable<typeof theme.scenery>
  if(kind==='old-grid')map.geometry=(await import('../../spatial-v3/source-inputs.json')).default.geometry
  if(kind==='v8-geometry'){const old=(await import('../../spatial-v4/source-inputs.json')).default;map.geometry=structuredClone(old.geometry);map.geometry_hash=old.geometry_hash;theme.geometry_hash=old.geometry_hash}
  if(kind==='v8-tile'){const old=(await import('../../spatial-v4/tile-pin.json')).default;map.tilemap.path=old.path;map.tilemap.sha256=old.sha256}
  if(kind==='v8-scene'){const old=(await import('../../spatial-v4/source-inputs.json')).default;theme.scenery=structuredClone(old.scenes.PARADE.scenery) as NonNullable<typeof theme.scenery>;theme.texture.path=theme.scenery.ground.path;theme.texture.sha256=theme.scenery.ground.sha256}
  if(kind==='foreign-id'){map.map_id='foreign_scene';theme.map_id=map.map_id}
  if(kind==='revision'){map.map_revision=3;theme.map_revision=3}
  if(kind==='tiled-pin')map.tilemap.sha256='sha256:'+'0'.repeat(64)
  if(kind==='compatibility-png')theme.texture.path='world/reference-scene-v2/runtime-v4/parade-ground.png'
  if(kind==='place-authority')map.places[0]!.label='Claimed physical destination'
  if(kind==='frame')Object.assign(theme.scenery!.objects[0]!.frame,{x:theme.scenery!.objects[0]!.frame.x+1})
  if(kind==='foot-depth')Object.assign(theme.scenery!.objects[0]!,{depth_y:theme.scenery!.objects[0]!.depth_y+1})
  if(kind==='spawn')map.geometry.spawns[0]!.cell_y++
  map.content_hash=await contentHash(map);theme.map_content_hash=map.content_hash;theme.content_hash=await contentHash(theme)
  await expect(loadReferenceScene({...s.input,mapText:JSON.stringify(map),themeText:JSON.stringify(theme),pin:{...s.f.pin,mapHash:map.content_hash,themeHash:theme.content_hash}},new AbortController().signal)).rejects.toThrow()
  expect(s.read).not.toHaveBeenCalled();expect(s.decode).not.toHaveBeenCalled()
})
it('releases an abort-resistant decoded image and cannot publish a spatial lease after cancellation',async()=>{
  const s=await setup(),abort=new AbortController();let finish!:(d:DecodedImage)=>void
  const load=loadReferenceScene({...s.input,decode:()=>new Promise<DecodedImage>(r=>{finish=r})},abort.signal)
  const rejected=expect(load).rejects.toThrow();await vi.waitFor(()=>expect(finish).toBeTypeOf('function'));abort.abort();await rejected
  const dispose=vi.fn();finish({image:{} as HTMLImageElement,dispose});await Promise.resolve();await Promise.resolve();expect(dispose).toHaveBeenCalledOnce()
})

it.each(['old-atlas', 'old-objects', 'cross-era-atlas', 'foreign-geometry-claim'] as const)('rejects architecture %s before resources even after rehashing manifests', async fault => {
  const s = await setup(), { map, theme } = s.f
  if (fault === 'old-atlas') Object.assign(theme.scenery!, { atlases: structuredClone(source.scenes.PARADE.scenery.atlases) })
  if (fault === 'old-objects') Object.assign(theme.scenery!, { objects: structuredClone(source.scenes.PARADE.scenery.objects) })
  if (fault === 'cross-era-atlas') Object.assign(theme.scenery!, { atlases: structuredClone(architecture.scenes.DANGER.scenery.atlases) })
  if (fault === 'foreign-geometry-claim') {
    const old = await import('../../spatial-v4/source-inputs.json')
    map.geometry = structuredClone(old.default.geometry); map.geometry_hash = old.default.geometry_hash; theme.geometry_hash = map.geometry_hash
    map.tilemap.path = 'world/foreign/canonical.json'
    Object.assign(theme.scenery!.ground, { path: 'world/foreign/ground.png' })
  }
  map.content_hash = await contentHash(map); theme.map_content_hash = map.content_hash; theme.content_hash = await contentHash(theme)
  await expect(loadReferenceScene({ ...s.input, mapText: JSON.stringify(map), themeText: JSON.stringify(theme), pin: { ...s.f.pin, mapHash: map.content_hash, themeHash: theme.content_hash } }, new AbortController().signal)).rejects.toThrow()
  expect(s.read).not.toHaveBeenCalled(); expect(s.decode).not.toHaveBeenCalled()
})

it('preserves all v8 non-terrain authority while deriving exactly560 changed terrain cells', async () => {
  const old=(await import('../../spatial-v4/source-inputs.json')).default
  const {terrain_layer: before,...oldAuthority}=old.geometry, {terrain_layer: after,...newAuthority}=source.geometry
  expect(newAuthority).toEqual(oldAuthority);expect(source.places).toEqual(old.places)
  expect(after.flat().filter((cell,i)=>cell!==before.flat()[i])).toHaveLength(560)
  for(const revision of [5,6,6.5]) {
    const identity={geometryHash:source.geometry_hash,width:768,height:1296,themeId:'PARADE',mapId:source.map_id,mapRevision:revision}
    expect(getReferenceOverviewMetadata(identity)!==null).toBe(revision===6)
  }
})
