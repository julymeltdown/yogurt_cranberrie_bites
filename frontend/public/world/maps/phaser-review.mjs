// Terrain-only asset qualification. Candidate manifests exist only in memory, never product pins.
import { contentHash, digest, sha256 } from '/src/world/map/json.ts'
import { loadMap, createAssetResources } from '/src/world/map/resources.ts'
import { loadPhaserEngine } from '/src/world/scenes/MapScene.ts'
const get = name => fetch(`/world/maps/${name}`).then(r => { if (!r.ok) throw Error(name); return r.json() })
const [geometry, places, bindings, textureInputs] = await Promise.all(['geometry.json', 'virtual-places.json', 'binding-inputs.json', 'phaser-texture-inputs.json'].map(get))
const source = places[0].provenance
const map = { kind: 'map_manifest', contract_version: '1.0.0', map_id: bindings.map_id, map_revision: bindings.map_revision,
  provenance: source, geometry_hash: bindings.geometry_hash, geometry, places,
  tilemap: { format: 'TILED_JSON', resource_state: 'PROVIDED', path: bindings.tilemap_path, sha256: bindings.tilemap_sha256 } }
map.content_hash = await contentHash(map)
// Required v1 identity fields are a review-only sentinel; no avatar manifest/art is loaded or bound.
const avatar = { manifest_id: 'ar02_terrain_only_unbound_avatar', manifest_version: 1, content_hash: await digest({ scope: 'AR02 TERRAIN ONLY - NOT AN AVATAR RELEASE' }) }
const themes = await Promise.all(textureInputs.themes.map(async sourceTheme => {
  const theme = { kind: 'theme_manifest', contract_version: '1.0.0', theme_id: sourceTheme.theme_id, theme_revision: sourceTheme.theme_revision,
    provenance: sourceTheme.provenance, map_id: map.map_id, map_revision: map.map_revision, map_content_hash: map.content_hash,
    geometry_hash: map.geometry_hash, collision_id: geometry.collision_id, place_ids: places.map(p => p.place_id), interaction_ids: geometry.interactions.map(p => p.interaction_id),
    avatar_manifest_id: avatar.manifest_id, avatar_manifest_version: avatar.manifest_version, avatar_content_hash: avatar.content_hash,
    texture: sourceTheme.texture, palette: sourceTheme.palette }
  theme.content_hash = await contentHash(theme); return theme
}))
const resources = createAssetResources({ origin: location.origin, allow: [{ path: map.tilemap.path, sha256: map.tilemap.sha256, kind: 'json' }, ...themes.map(t => ({ path: t.texture.path, sha256: t.texture.sha256, kind: 'png' }))] })
const load = (theme, sourceMap = map, reader = resources) => loadMap({ mapText: JSON.stringify(sourceMap), themeText: JSON.stringify(theme),
  pin: { mapHash: sourceMap.content_hash, themeHash: theme.content_hash, avatar }, resources: reader }, new AbortController().signal)
const host = document.querySelector('#world'), selectTheme = document.querySelector('#theme'), selectTarget = document.querySelector('#target')
const byPlace = new Map(places.map(p => [p.place_id, p]))
for (const [i, point] of geometry.interactions.entries()) {
  const label = byPlace.get(point.place_id).label, option = document.createElement('option'); option.value = i; option.textContent = label; selectTarget.append(option)
  const li = document.createElement('li'); li.textContent = label; document.querySelector('#legend').append(li)
}
selectTarget.value = '1'
let loaded = await load(themes[0]), lease = (await loadPhaserEngine())(host, loaded)
await lease.ready; lease.resize(Math.floor(host.getBoundingClientRect().width), 320)
const frame = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
const point = () => geometry.interactions[Number(selectTarget.value)]
function focus() {
  const p = point(); lease.camera.center({ x: (p.cell_x + .5)*32, y: (p.cell_y + .5)*32 }, 1)
  document.querySelector('#current').textContent = `${byPlace.get(p.place_id).label} · virtual information point`
}
async function signature() {
  await frame(); const canvas = lease.camera.surface(), pixels = canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height)
  return sha256(new Uint8Array(pixels.data.buffer))
}
async function parity() {
  await frame()
  const actual = lease.camera.surface(), camera = lease.camera.read(), expected = document.createElement('canvas')
  expected.width=actual.width; expected.height=actual.height; const ctx=expected.getContext('2d');ctx.imageSmoothingEnabled=false
  ctx.fillStyle=loaded.theme.palette.background;ctx.fillRect(0,0,expected.width,expected.height)
  for(const grid of [geometry.terrain_layer,geometry.overdraw_layer])for(let y=0;y<geometry.height_cells;y++)for(let x=0;x<geometry.width_cells;x++){
    const id=grid[y][x];if(!id)continue; const index=id-1
    ctx.drawImage(loaded.image,index%8*32,Math.floor(index/8)*32,32,32,x*32-camera.scrollX,y*32-camera.scrollY,32,32)
  }
  const a=actual.getContext('2d').getImageData(0,0,actual.width,actual.height).data,b=ctx.getImageData(0,0,expected.width,expected.height).data
  let different=0;for(let i=0;i<a.length;i++)if(a[i]!==b[i])different++
  return { differentChannels:different,totalChannels:a.length,width:actual.width,height:actual.height,camera }
}
async function state() {
  await frame(); const canvas=lease.camera.surface(); return { ready:true,theme:loaded.theme.theme_id,point:point().interaction_id,
    gameCanvases:host.querySelectorAll('canvas').length,width:canvas.width,height:canvas.height,geometryHash:map.geometry_hash,textureHash:loaded.theme.texture.sha256,
    sourceCommit:textureInputs.sourceCommit,avatarStatus:'UNBOUND_TERRAIN_REVIEW_ONLY',pixelSignature:await signature() }
}
focus(); window.ar02Phaser=await state()
selectTheme.addEventListener('change',async()=>{try{
  window.ar02Phaser.ready=false;const next=await load(themes.find(t=>t.theme_id===selectTheme.value));lease.replaceSkin(next);loaded.dispose();loaded=next
  window.ar02Phaser=await state()
}catch(error){window.ar02Phaser={error:String(error)}}})
selectTarget.addEventListener('change',async()=>{window.ar02Phaser.ready=false;focus();window.ar02Phaser=await state()})
window.ar02PhaserParity=parity
window.ar02PhaserNegatives=async()=>{
  const rejected=[]
  for(const kind of ['tiled-digest','png-digest','png-dimensions','tiled-grid']){
    const m=structuredClone(map),theme=structuredClone(loaded.theme)
    let replacement
    if(kind==='png-dimensions'){
      replacement=new Uint8Array(await fetch('/'+theme.texture.path).then(r=>r.arrayBuffer()));new DataView(replacement.buffer).setUint32(16,128)
      theme.texture.sha256=await sha256(replacement)
    }
    if(kind==='tiled-grid'){
      const tiled=await get('courtyard-tiled.json');tiled.layers[1].data[0]=0;replacement=new TextEncoder().encode(JSON.stringify(tiled));m.tilemap.sha256=await sha256(replacement)
    }
    m.content_hash=await contentHash(m);theme.map_content_hash=m.content_hash;theme.content_hash=await contentHash(theme)
    const reader={read:async(resource,signal)=>{
      if(kind==='png-dimensions'&&resource.kind==='png'||kind==='tiled-grid'&&resource.kind==='json')return replacement
      const bytes=new Uint8Array(await fetch('/'+resource.path).then(r=>r.arrayBuffer()))
      if(kind==='tiled-digest'&&resource.kind==='json'||kind==='png-digest'&&resource.kind==='png')bytes[bytes.length-1]^=1
      return bytes
    }}
    try{const bad=await load(theme,m,reader);bad.dispose();throw Error('ACCEPTED '+kind)}catch(error){if(String(error).includes('ACCEPTED'))throw error;rejected.push({kind,reason:error.reason??String(error)})}
  }
  return rejected
}
window.ar02PhaserDispose=async()=>{lease.destroy();await lease.closed;loaded.dispose();return{canvases:host.querySelectorAll('canvas').length}}
