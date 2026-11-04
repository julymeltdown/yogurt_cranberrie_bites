import assert from 'node:assert/strict'
import {readFile} from 'node:fs/promises'
import {fileURLToPath} from 'node:url'
import test from 'node:test'
import {qualifyRoofStreet,verifyArtifacts,type Layout,type Proof,type Original} from './qualifier.ts'
assert.equal(process.versions.node,'24.22.2')
const root = fileURLToPath(new URL('../../../../../',import.meta.url))
const load = async <T>(p: string): Promise<T> => JSON.parse(await readFile(root+p,'utf8')) as T
const base = await load<Layout>('frontend/public/world/map-density-derived-v1/layout.json')
const candidate = await load<Layout>('frontend/public/world/map-roof-street-derived-v2/layout.json')
const proof = await load<Proof>('frontend/public/world/map-roof-street-derived-v2/source-provenance.json')
const original = await load<Original>('frontend/public/world/map-density-derived-v1/source-provenance.json')
const bytes = new Map<string,Uint8Array>()
for (const pin of [...proof.baselinePins,...proof.sourceInputs,...proof.license.inputPins,...proof.outputs,proof.recipe]) bytes.set(pin.path,await readFile(root+pin.path))
const check = (p=proof,c=candidate) => qualifyRoofStreet(base,c,p,original)
test('real changed three-era roof/street output qualifies against immutable source and complete artifact bytes',async () => {
  assert.deepEqual(check(),[]); assert.deepEqual(await verifyArtifacts(proof,bytes),[])
  for (const era of ['revenge','parade','danger']) {
    const raw = Buffer.from(bytes.get('frontend/public/world/map-roof-street-derived-v2/'+era+'-scene.png')!)
    assert.equal(raw.readUInt32BE(16),768); assert.equal(raw.readUInt32BE(20),1296); assert.equal(raw[25],2)
  }
})
test('rejects World identity and path mutation',() => {
  const c=structuredClone(candidate); c.originalMap4={forged:true}; c.paths=[]
  assert(check(proof,c).includes('IMMUTABLE_LAYOUT:originalMap4')); assert(check(proof,c).includes('IMMUTABLE_LAYOUT:paths'))
})
test('rejects changing an incumbent facade, tree, identity or depth record',() => {
  const c=structuredClone(candidate); c.decorations[0]!.frame='invented'; c.decorations[0]!.depth_y+=1
  assert(check(proof,c).includes('INCUMBENT_DECORATION'))
})
test('rejects a roof outside its original projected illustration slot',() => {
  const p=structuredClone(proof); p.roofPlansByTheme.PARADE![0]!.renderedRect=[0,0,999,999]
  assert(check(p).includes('ORIGINAL_PROJECTION'))
})
test('rejects a detached or out-of-bounds roof volume',() => {
  const p=structuredClone(proof); p.roofPlansByTheme.PARADE![0]!.volumes[0]!.rect=[-5,0,10,10]
  assert(check(p).includes('ROOF_VOLUME'))
})
test('rejects roof or paving entering protected label reserves',() => {
  const p=structuredClone(proof),e=base.serviceLabelAnchors[0]!.exclusionRect
  p.actualByTheme.PARADE!.pavingRects[0]=[e.x,e.y,12,7]
  assert(check(p).includes('LABEL_RESERVE'))
})
test('rejects source RGBA retouching or original input substitution',() => {
  const p=structuredClone(proof); p.originalCropPins[0]!.rgbaSha256='sha256:'+'0'.repeat(64)
  assert(check(p).includes('ORIGINAL_RGBA_OR_INPUT'))
})
test('rejects era-dependent roof geometry and unsupported source coverage claims',() => {
  const p=structuredClone(proof); p.roofPlansByTheme.REVENGE!.pop(); p.actualByTheme.DANGER!.streetChangedPixels=0
  assert(check(p).includes('ERA_GEOMETRY')); assert(check(p).includes('ACTUAL_ROOF_AND_STREET_DELTA'))
})
test('rejects missing source license lineage and false label proof',() => {
  const p=structuredClone(proof); p.license.sourceRecord='invented'; p.protectedLabelRectanglesExact=0
  assert(check(p).includes('SOURCE_LICENSE_LINEAGE')); assert(check(p).includes('LABEL_RECTANGLES'))
})
test('rejects baseline alias and a changed original source hash',() => {
  const p=structuredClone(proof); p.baselinePins[0]!.sha256='sha256:'+'0'.repeat(64)
  assert(check(p).includes('BASELINE_PINS'))
})
test('rejects altered output bytes',async () => {
  const b=new Map(bytes),key='frontend/public/world/map-roof-street-derived-v2/parade-scene.png'
  const raw=Uint8Array.from(b.get(key)!); raw[100]=raw[100]!^1; b.set(key,raw)
  assert((await verifyArtifacts(proof,b)).includes('SHA256:'+key))
})
test('rejects missing era output instead of accepting a partial three-skin pack',async () => {
  const p=structuredClone(proof); p.outputs.pop()
  assert((await verifyArtifacts(p,bytes)).includes('OUTPUT_SET'))
})
