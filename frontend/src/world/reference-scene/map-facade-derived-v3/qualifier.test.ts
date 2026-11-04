import assert from 'node:assert/strict'
import {readFile} from 'node:fs/promises'
import {fileURLToPath} from 'node:url'
import test from 'node:test'
import {qualifyFacade,verifyArtifacts,type Layout,type Proof,type Ground,type Pin} from './qualifier.ts'
import {decodePng} from './png.ts'
assert.equal(process.versions.node,'24.22.2')
const root=fileURLToPath(new URL('../../../../../',import.meta.url))
const load=async<T>(p:string):Promise<T>=>JSON.parse(await readFile(root+p,'utf8')) as T
const base=await load<Layout>('frontend/public/world/map-roof-street-derived-v2/layout.json')
const original=await load<Proof>('frontend/public/world/map-roof-street-derived-v2/source-provenance.json')
const candidate=await load<Layout>('frontend/public/world/map-facade-derived-v3/layout.json')
const proof=await load<Proof>('frontend/public/world/map-facade-derived-v3/source-provenance.json')
const ground=await load<Ground>(proof.groundBinding.path)
const bytes=new Map<string,Uint8Array>()
async function collect(value:unknown):Promise<void> {
 if(!value||typeof value!=='object')return
 if('path' in value&&'bytes' in value&&'sha256' in value) {
  const pin=value as Pin
  if(!bytes.has(pin.path))bytes.set(pin.path,await readFile(root+pin.path))
  return
 }
 for(const child of Object.values(value))await collect(child)
}
await collect(proof);await collect(ground)
await collect(JSON.parse(Buffer.from(bytes.get(ground.provenancePin.path)!).toString()))
const check=(p=proof,c=candidate,g=ground)=>qualifyFacade(base,c,p,original,g)
test('real composed three-era pixels qualify, with independently reconstructed original alpha and all file bytes',async()=>{
 assert.deepEqual(check(),[]);assert.deepEqual(await verifyArtifacts(proof,ground,bytes),[])
})
test('rejects changing World content, geometry or layout authority',()=>{
 const c=structuredClone(candidate);c.originalMap4={forged:true};c.paths=[]
 assert(check(proof,c).includes('IMMUTABLE_LAYOUT:originalMap4'));assert(check(proof,c).includes('IMMUTABLE_LAYOUT:paths'))
})
test('rejects moving an incumbent crop, depth or decoration identity',()=>{
 const c=structuredClone(candidate);c.decorations[0]!.depth_y++
 assert(check(proof,c).includes('INCUMBENT_DECORATION'))
})
test('rejects extending the explicit Root facade bounds',()=>{
 const p=structuredClone(proof);p.allowedBounds.map_only_stage_west_wing=[0,0,768,1296]
 assert(check(p).includes('ROOT_PAINT_BOUNDS'))
})
test('rejects losing original source RGBA provenance',()=>{
 const p=structuredClone(proof);p.originalCropPins[0]!.rgbaSha256='sha256:'+'0'.repeat(64)
 assert(check(p).includes('ORIGINAL_RGBA_OR_INPUT'))
})
test('rejects a fake gap painted over the original layer',()=>{
 const p=structuredClone(proof),c=p.componentsByTheme.PARADE!.map_only_stage_central_hall!.find(c=>c.role==='negative_gap')!
 c.operations=[{kind:'rect',rect:c.rect,rgba:[200,200,200,255]}]
 assert(check(p).includes('TRUE_RECOMPOSED_GAP'))
})
test('rejects a primitive beyond its authored component and translucent facade cover',()=>{
 const p=structuredClone(proof),c=p.componentsByTheme.PARADE!.map_only_stage_west_wing!.find(c=>c.role==='recessed_bay')!
 c.operations=[{kind:'rect',rect:[0,0,20,20],rgba:[200,200,200,128]}]
 assert(check(p).includes('COMPONENT_OPERATION'));assert(check(p).includes('COMPONENT_RGBA'))
})
test('rejects ground without actual qualification or a different frozen v2 baseline',()=>{
 const g=structuredClone(ground);g.qualification.status='NOT_RUN';g.baselinePins[0]!.sha256='sha256:'+'0'.repeat(64)
 assert(check(proof,candidate,g).includes('QUALIFIED_GROUND'));assert(check(proof,candidate,g).includes('FROZEN_V2'))
})
test('rejects era-dependent facade geometry',()=>{
 const p=structuredClone(proof);p.componentsByTheme.REVENGE!.map_only_stage_east_wing!.pop()
 assert(check(p).includes('ERA_GEOMETRY'))
})
test('rejects source-alpha edits inside the nominal bound but inside the canonical anchor disc',async()=>{
 const p=structuredClone(proof),c=p.componentsByTheme.PARADE!.map_only_stage_central_hall!.find(c=>c.role==='recessed_bay')!
 c.rect=[398,375,6,12];c.operations=[{kind:'rect',rect:c.rect,rgba:[255,200,160,255]}]
 assert((await verifyArtifacts(p,ground,bytes)).includes('SOURCE_ALPHA_OR_FOREGROUND:'+c.id))
})
test('rejects a changed source PNG byte and a missing era output',async()=>{
 const b=new Map(bytes),key='frontend/public/world/map-facade-derived-v3/parade-scene.png',raw=Uint8Array.from(b.get(key)!)
 raw[100]=raw[100]!^1;b.set(key,raw)
 assert((await verifyArtifacts(proof,ground,b)).includes('SHA256:'+key))
 const p=structuredClone(proof);p.outputs.pop()
 assert((await verifyArtifacts(p,ground,bytes)).includes('OUTPUT_SET'))
})
test('rejects a ground object that is not the exact pinned qualified interface bytes',async()=>{
 const g=structuredClone(ground);g.qualification.sourceCommit='forged'
 assert((await verifyArtifacts(proof,g,bytes)).includes('GROUND_INTERFACE_BYTES'))
})
test('rejects a facade mask hash and false actual changed-pixel counts',async()=>{
 const p=structuredClone(proof);p.facadeMaskPins.REVENGE!.map_only_stage_west_wing!.sha256='sha256:'+'0'.repeat(64);p.actualByTheme.REVENGE!.changedPixels=1
 const reasons=await verifyArtifacts(p,ground,bytes)
 assert(reasons.includes('INDEPENDENT_FACADE_ALPHA_MASK:REVENGE:map_only_stage_west_wing'));assert(reasons.includes('ACTUAL_DELTA_COUNTS'))
})
test('PNG decoder refuses malformed or unsupported source data',()=>{
 assert.throws(()=>decodePng(new Uint8Array([1,2,3])))
 const raw=Uint8Array.from(bytes.get('frontend/public/world/map-facade-derived-v3/parade-scene.png')!);raw[24]=16
 assert.throws(()=>decodePng(raw))
})

