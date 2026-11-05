/** Source-artifact checks only. The spatial owner's contract interprets this ABI. */
import {createHash} from 'node:crypto'
import {readFile, readdir} from 'node:fs/promises'
import path from 'node:path'

export const photo5={id:'photo-5',sha256:'d0713633289f87fc98603927f7cbc02df2054e989f6360332a93476d6c9da4e0',bytes:328720}
export const photo10={id:'photo-10',sha256:'1f0e133be3a4bf999fb7871188cce67086d09106a095c266b74e648a67777f63',bytes:320653}
export const assetPath='venue/paradise-city/v1/siteplan-photo-5.jpg'
const required=['stage','standing-a','standing-b','foh','main-gate','sub-gate','station','chroma','studio-paradise','spa-cimer','wonderbox','art-paradiso-plaza','photo-zone']
const object=(v:unknown):v is Record<string,any>=>!!v&&typeof v==='object'&&!Array.isArray(v)
const keys=(v:unknown,k:string[])=>object(v)&&Object.keys(v).sort().join('|')===[...k].sort().join('|')
const id=(v:unknown)=>typeof v==='string'&&v.length<=64&&/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(v)
const integer=(v:unknown)=>typeof v==='number'&&Number.isFinite(v)&&Number.isInteger(v)
const same=(a:unknown,b:unknown):boolean=>{
 if(typeof a!==typeof b)return false
 if(Array.isArray(a)||Array.isArray(b))return Array.isArray(a)&&Array.isArray(b)&&a.length===b.length&&a.every((v,i)=>same(v,b[i]))
 if(object(a)||object(b))return object(a)&&object(b)&&Object.keys(a).length===Object.keys(b).length&&Object.keys(a).every(k=>Object.hasOwn(b,k)&&same(a[k],b[k]))
 return a===b
}
export const digest=(v:Uint8Array)=>createHash('sha256').update(v).digest('hex')
export function jpegDimensions(input:Uint8Array) {
 const raw=Buffer.from(input)
 if(raw[0]!==255||raw[1]!==216)throw Error('JPEG_SIGNATURE')
 let at=2
 while(at<raw.length) {
  if(raw[at++]!==255)throw Error('JPEG_MARKER')
  while(raw[at]===255)at++
  const marker=raw[at++]!
  if(marker===216||marker===217||marker===1||(marker>=208&&marker<=215))continue
  if(at+2>raw.length)throw Error('JPEG_LENGTH')
  const length=raw.readUInt16BE(at)
  if(length<2||at+length>raw.length)throw Error('JPEG_LENGTH')
  if([192,193,194].includes(marker)) {
   if(length<8||raw[at+2]!==8)throw Error('JPEG_PRECISION')
   return {width:raw.readUInt16BE(at+5),height:raw.readUInt16BE(at+3)}
  }
  if(marker===218)throw Error('JPEG_SOF_MISSING')
  at+=length
 }
 throw Error('JPEG_SOF_MISSING')
}
export function qualifyInterface(value:unknown):string[] {
 const errors=new Set<string>();const fail=(reason:string)=>errors.add(reason)
 if(!keys(value,['schemaVersion','venueId','siteplanId','revision','variantId','sourcePixelCoordinateSpace','imageOrigin','image','sourceRefs','landmarks']))return ['ENVELOPE']
 const v=value as Record<string,any>
 if(v.schemaVersion!=='paradise-city-siteplan-v1'||v.venueId!=='paradise-city'||v.siteplanId!=='paradise-city-siteplan'||v.revision!==1||v.variantId!=='photo-5-provisional')fail('VARIANT_IDENTITY')
 if(v.sourcePixelCoordinateSpace!=='image-px'||v.imageOrigin!=='top-left')fail('PRESENTATION_COORDINATES')
 const image={path:assetPath,mime:'image/jpeg',sha256:photo5.sha256,bytes:photo5.bytes,width:1024,height:1280}
 if(!same(v.image,image))fail('EXACT_IMAGE')
 if(!same(v.sourceRefs,[photo5,photo10]))fail('EXACT_SOURCE_REFS')
 if(!Array.isArray(v.landmarks)||!v.landmarks.length||v.landmarks.length>128)return [...errors,'LANDMARK_ARRAY']
 const ids=new Set<string>()
 for(const a of v.landmarks) {
  if(!keys(a,['id','displayName','anchor','hitRegion','sourceRefs','review'])){fail('CANDIDATE_FIELDS');continue}
  if(!id(a.id)||ids.has(a.id))fail('CANDIDATE_ID');ids.add(a.id)
  if(typeof a.displayName!=='string'||a.displayName.length<1||a.displayName.length>120||a.displayName.trim()!==a.displayName||/[\u0000-\u001f\u007f]/.test(a.displayName))fail('SOURCE_LABEL')
  if(a.review!=='ILLUSTRATED_UNCALIBRATED')fail('SOURCE_REVIEW')
  if(!Array.isArray(a.sourceRefs)||!a.sourceRefs.length||a.sourceRefs.length>2||new Set(a.sourceRefs).size!==a.sourceRefs.length||a.sourceRefs.some((r:unknown)=>r!=='photo-5'&&r!=='photo-10')||!a.sourceRefs.includes('photo-5'))fail('CANDIDATE_SOURCE_REFS')
  if(a.id==='photo-zone'&&(!Array.isArray(a.sourceRefs)||!a.sourceRefs.includes('photo-10')))fail('PHOTO_ZONE_LABEL_CORROBORATION')
  if(!keys(a.anchor,['x','y'])||!keys(a.hitRegion,['x','y','width','height'])){fail('PIXEL_GEOMETRY');continue}
  const p=a.anchor,r=a.hitRegion
  if(![p.x,p.y,r.x,r.y,r.width,r.height].every(integer)||p.x<0||p.x>=1024||p.y<0||p.y>=1280||r.x<0||r.y<0||r.width<=0||r.height<=0||r.x+r.width>1024||r.y+r.height>1280||p.x<r.x||p.x>=r.x+r.width||p.y<r.y||p.y>=r.y+r.height)fail('PIXEL_BOUNDS')
 }
 if(required.some(k=>!ids.has(k)))fail('COMPLETE_PICTURED_PLAN_ANCHORS')
 return [...errors]
}
export function qualifyImage(raw:Uint8Array) {
 const errors=[]
 if(raw.length!==photo5.bytes||digest(raw)!==photo5.sha256)errors.push('EXACT_JPEG_BYTES')
 try{if(!same(jpegDimensions(raw),{width:1024,height:1280}))errors.push('JPEG_DIMENSIONS')}catch{errors.push('JPEG_DECODE_HEADER')}
 return errors
}
export async function qualifyPack(root:string) {
 const pack=path.join(root,'frontend/public/venue/paradise-city/v1')
 const names=(await readdir(pack)).sort()
 if(!same(names,['asset-interface.json','siteplan-photo-5.jpg','source-provenance.json']))throw Error('CLOSED_PACK_FILES')
 const raw=await readFile(path.join(pack,'siteplan-photo-5.jpg'))
 const source=await readFile(path.join(pack,'asset-interface.json'));const v=JSON.parse(source.toString())
 const errors=[...qualifyImage(raw),...qualifyInterface(v)]
 const provenance=JSON.parse(await readFile(path.join(pack,'source-provenance.json'),'utf8'))
 if(provenance.format!=='PARADISE_CITY_ILLUSTRATED_SOURCE_PACK_V1'||!provenance.originalBytesUnchanged||provenance.imageTransform!=='NONE_FULL_1024x1280_BOARD_RETAINED'||provenance.variantId!=='photo-5-provisional'||provenance.primaryState!=='PROVISIONAL')errors.push('PROVENANCE_AUTHORITY')
 if(provenance.sourceBase!=='8ac5dc2e2b9b336885da3ec85e82641a9f8cebe1'||provenance.publicationBaseline!=='ac24131a69b4d4779a0d588582a7198def466846')errors.push('SOURCE_BASE')
 const anchors=JSON.parse(await readFile(path.join(root,'frontend/src/features/paradise-city-map/art-source/source-anchors.json'),'utf8'))
 if(!same(v.landmarks,anchors))errors.push('SINGLE_SOURCE_ANCHOR_GEOMETRY')
 for(const pin of [provenance.anchorSource,provenance.recipe,...provenance.outputs]) {
  if(!object(pin)||typeof pin.path!=='string'||path.isAbsolute(pin.path)||pin.path.split('/').includes('..')){errors.push('PIN_PATH');continue}
  const bytes=await readFile(path.join(root,pin.path))
  if(bytes.length!==pin.bytes||digest(bytes)!==pin.sha256)errors.push('SOURCE_PIN:'+pin.path)
 }
 if(!same(provenance.outputs.map((p:any)=>p.path).sort(),['frontend/public/venue/paradise-city/v1/asset-interface.json','frontend/public/venue/paradise-city/v1/siteplan-photo-5.jpg']))errors.push('PROVENANCE_OUTPUT_CLOSURE')
 if(!same(provenance.original,{file:'5-Photo-5.jpg',...photo5,width:1024,height:1280})||!same(provenance.supplementaryLabelReference,{...photo10,use:'PHOTO ZONE label corroboration only; all source anchor geometry is measured on Photo 5; no variant geometry blending'}))errors.push('ORIGINAL_REFERENCE_PROVENANCE')
 if(errors.length)throw Error([...new Set(errors)].join(','))
 return {status:'PASS',imageBytes:raw.length,imageSHA256:digest(raw),width:1024,height:1280,sourceCandidates:v.landmarks.length,interfaceSHA256:digest(source),provisional:true,coordinateSpace:'image-px'}
}
