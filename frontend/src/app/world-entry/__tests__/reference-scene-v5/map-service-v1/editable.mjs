// New service derivative of immutable, qualified architecture SVGs. Old qualification is not inherited.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const hash = raw => createHash('sha256').update(raw).digest('hex')
const escape = value => String(value).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;')
function labelMarkup(l, labels) {
 const s=labels.style,r=l.rect,b=s.border_width,x=r.x+s.padding,y=r.y+(r.height-s.icon_size)/2,k=s.icon_size/24
 const pieces=[`<g data-label="${escape(l.label)}" data-anchor="${l.x},${l.y}"><rect data-paper="" x="${r.x}" y="${r.y}" width="${r.width}" height="${r.height}" fill="${s.border_fill}"/><rect x="${r.x+b}" y="${r.y+b}" width="${r.width-2*b}" height="${r.height-2*b}" fill="${s.paper_fill}"/>`]
 if(l.icon){
  pieces.push(`<rect x="${x-3}" y="${y-3}" width="${s.icon_size+6}" height="${s.icon_size+6}" fill="${s.icon_paper}"/>`)
  for(const [shapes,color] of [[labels.icons[l.icon],s.icon_fill],[labels.icon_cutouts[l.icon]??[],s.icon_paper]])for(const shape of shapes)pieces.push(`<polygon points="${shape.map(([xx,yy])=>[x+xx*k,y+yy*k].join(',')).join(' ')}" fill="${color}"/>`)
 }
 l.lines.forEach((line,i)=>pieces.push(`<text x="${r.x+s.padding+(l.icon?s.icon_cell:0)}" y="${r.y+(r.height-l.lines.length*s.line_height)/2+32+i*s.line_height}" font-family="MCR Map Labels" font-size="${s.font_size}" fill="${s.text_fill}">${escape(line)}</text>`))
 return pieces.join('')+'</g>'
}
function objectMarkup(o,frame){return `<svg data-object="${escape(o.id)}" data-depth="${o.depth_y}" x="${o.x}" y="${o.y}" width="${o.width}" height="${o.height}" viewBox="${frame.x} ${frame.y} ${frame.width} ${frame.height}" preserveAspectRatio="none" overflow="hidden"><use href="#atlas"/></svg>`}
export async function writeEditable(frontend,output,era){
 const repo=resolve(frontend,'..'),read=async p=>JSON.parse(await readFile(resolve(repo,p),'utf8'))
 const prefix='docs/mcr/evidence/reference-scene-v5/architecture-v2/'
 const path=prefix+'native-04/editable-'+era.toLowerCase()+'.svg'
 const packet=await read(prefix+'capture-packet.json'),original=await readFile(resolve(repo,path)),pin=packet.artifacts['native-04/editable-'+era.toLowerCase()+'.svg']
 assert.equal(original.length,pin.bytes);assert.equal(hash(original),pin.sha256)
 const document=await read('frontend/src/world/reference-scene/spatial-v5/map-service-source.json'),pins=await read('frontend/src/world/reference-scene/spatial-v5/map-service-pins.json')
 const base=await read('frontend/src/world/reference-scene/spatial-v5/source-inputs.json'),labels=document.presentation
 const before=original.toString(),objects=[...before.matchAll(/<svg data-object="[^"]+"[^>]*><use href="#atlas"\/><\/svg>/g)].map(m=>m[0])
 assert.equal(objects.length,49);const oldBlock=objects.join('');assert(before.includes(oldBlock))
 const replacement=document.display_replacements[era],oldFood=objects.find(s=>s.includes('data-object="food_counters"'));assert(oldFood)
 const mapped=objects.map((text,index)=>({text:text===oldFood?objectMarkup(replacement,replacement.frame):text,depth:Number(text.match(/data-depth="([^"]+)"/)[1]),index}))
 document.annexes.forEach((row,i)=>mapped.push({text:objectMarkup(row,base.scenes[era].frames[row.frame]),depth:row.depth_y,index:49+i}))
 const newBlock=mapped.sort((a,b)=>a.depth-b.depth||a.index-b.index).map(o=>o.text).join('')
 let svg=before.replace(oldBlock,newBlock);const oldLabels=[...before.matchAll(/<g data-label="[^"]+"[\s\S]*?<\/g>/g)].map(m=>m[0]);assert.equal(oldLabels.length,6)
 const newLabels=labels.labels.map(l=>labelMarkup(l,labels));oldLabels.forEach((old,i)=>{svg=svg.replace(old,newLabels[i])})
 const metadata={format:'MCR_MAP_SERVICE_V1_EDITABLE_SVG',era,geometryHash:base.geometry_hash,objects:51,labels:6,
  sourceHandoff:pins.handoff,effectiveConsumerTuple:pins.effective_consumer_tuple,immutableBase:{path,bytes:pin.bytes,sha256:pin.sha256},
  authority:'NEW unbound virtual feature labels, two MAP wings and one neutral MAP display replacement. No canonical place, World, physical or release authority. Previous qualification not inherited.'}
 svg=svg.replace(/<metadata>[\s\S]*?<\/metadata>/,`<metadata>${escape(JSON.stringify(metadata))}</metadata>`)
 let normalized=svg.replace(newBlock,oldBlock);newLabels.forEach((s,i)=>{normalized=normalized.replace(s,oldLabels[i])})
 const strip=s=>s.replace(/<metadata>[\s\S]*?<\/metadata>/,'')
 assert.equal(strip(normalized),strip(before),'Only explicit new objects/display/labels/provenance may change; embedded assets and48 originals exact')
 await writeFile(resolve(output,`editable-${era.toLowerCase()}.svg`),svg)
 return {svg,metadata}
}
