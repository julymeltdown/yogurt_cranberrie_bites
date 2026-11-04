// New architecture derivative of immutable published layout-v2 SVGs. No prior qualification is inherited.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;')
const hash = raw => createHash('sha256').update(raw).digest('hex')
const bases = {
 REVENGE: '4ff8f0e6a987eb1b917ced7404b77b429e5e787a79defd18d918e2180f9ee8dc',
 PARADE: 'ce7527927525bbf3339d7880a67ddd7dd955832fa50a4f56a605c18b5bdc4566',
 DANGER: '39b7de9a3b2f10fd5e9f3eca77587b17c124b3cd685c6da6c002fdfdd46262aa',
}
export async function writeEditable(frontend, output, era) {
 const read = async p => JSON.parse(await readFile(resolve(frontend,p),'utf8'))
 const path = 'docs/mcr/evidence/reference-scene-v5/map-layout-v2/qualified-v2/editable-' + era.toLowerCase() + '.svg'
 const original = await readFile(resolve(frontend,'..',path)); assert.equal(hash(original),bases[era])
 const architecture = await read('src/world/reference-scene/spatial-v5/architecture-pin.json')
 const scenery = await read('public/world/reference-scene-v2/architecture-v2/'+era.toLowerCase()+'/scenery.json')
 const atlas = scenery.atlases[0], bytes = await readFile(resolve(frontend,'public',atlas.path))
 assert.equal(bytes.length,atlas.bytes);assert.equal('sha256:'+hash(bytes),atlas.sha256)
 const metadata = { format:'MCR_ARCHITECTURE_V2_EDITABLE_SVG', era, geometryHash:'sha256:ef52c8b05af6a882f2e82285c9879c9a4cb78ab513336e4bb3cd483c9b123395',
  architecture:{handoff:architecture.handoff,publishedRevision:architecture.published_revision,atlas},
  immutableBase:{revision:'2e1e01228bb8129c5e44c8741540b2206d29739b',path,sha256:bases[era],bytes:original.length},
  authority:'NEW architecture derivative. Original geometry preserved; prior qualification/admission NOT inherited.',objects:49,labels:6 }
 const before=original.toString(),pattern=/(<image id="atlas"[^>]* href=")[^"]+("[^>]*\/?>)/
 assert.equal((before.match(/id="atlas"/g)??[]).length,1);assert(pattern.test(before))
 const svg=before.replace(/<metadata>[\s\S]*?<\/metadata>/,`<metadata>${escape(JSON.stringify(metadata))}</metadata>`)
  .replace(pattern,(_all,prefix,suffix)=>prefix+'data:image/png;base64,'+bytes.toString('base64')+suffix)
 const geometry=text=>text.replace(/<metadata>[\s\S]*?<\/metadata>/,'').replace(pattern,'$1RESOURCE$2')
 assert.equal(geometry(svg),geometry(before),'Only atlas pixels and new provenance may change')
 await writeFile(resolve(output,`editable-${era.toLowerCase()}.svg`),svg)
 return {svg,metadata}
}
