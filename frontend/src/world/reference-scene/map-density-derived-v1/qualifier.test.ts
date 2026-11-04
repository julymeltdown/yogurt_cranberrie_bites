import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { qualifyDensity, verifyArtifacts, type DensityLayout, type DensityProvenance, type OriginalProvenance } from './qualifier.ts'

assert.equal(process.versions.node, '24.22.2')
console.log('Pinned Node '+process.versions.node+'; pure qualifier, no consumer process')
const root = fileURLToPath(new URL('../../../../../', import.meta.url))
const load = async <T>(path: string): Promise<T> => JSON.parse(await readFile(root+path, 'utf8')) as T
const base = await load<DensityLayout>('frontend/public/world/map-reference-composition-v1/layout.json')
const candidate = await load<DensityLayout>('frontend/public/world/map-density-derived-v1/layout.json')
const proof = await load<DensityProvenance>('frontend/public/world/map-density-derived-v1/source-provenance.json')
const original = await load<OriginalProvenance>('frontend/public/world/map-reference-composition-v1/source-provenance.json')
const readset = new Map<string, Uint8Array>()
for (const pin of [...proof.baselinePins, ...proof.sourceInputs, ...proof.outputs, proof.recipe]) readset.set(pin.path, await readFile(root+pin.path))
const addedIndex = candidate.decorations.findIndex(row => row.id.startsWith('density_'))
assert(addedIndex >= 0)

test('real generated derivative qualifies without changing the entire frozen baseline or original input bytes', async () => {
  assert.deepEqual(qualifyDensity(base, candidate, proof, original), [])
  assert.deepEqual(await verifyArtifacts(proof, readset), [])
  for (const era of ['revenge','parade','danger']) {
    const raw = readset.get('frontend/public/world/map-density-derived-v1/'+era+'-scene.png')!
    assert.equal(Buffer.from(raw).readUInt32BE(16), 768)
    assert.equal(Buffer.from(raw).readUInt32BE(20), 1296)
    assert.equal(raw[25], 2, 'RGB raster authority')
  }
})
test('rejects a changed canonical identity or movement corridor instead of treating art as geometry authority', () => {
  const changed = structuredClone(candidate)
  changed.originalMap4 = {tampered: true}
  changed.paths[0]![0]![0] = 0
  const failures = qualifyDensity(base, changed, proof, original)
  assert(failures.includes('IMMUTABLE_LAYOUT:originalMap4'))
  assert(failures.includes('IMMUTABLE_LAYOUT:paths'))
})
test('rejects replacement or duplicate incumbent decorations', () => {
  const changed = structuredClone(candidate)
  const old = changed.decorations.find(row => !row.id.startsWith('density_'))!
  old.frame = 'tree_green'
  changed.decorations.push(structuredClone(old))
  assert(qualifyDensity(base, changed, proof, original).includes('INCUMBENT_DECORATION'))
})
test('rejects new decorative interaction authority', () => {
  const changed = structuredClone(candidate)
  changed.decorations[addedIndex]!.interactive = true
  changed.decorations[addedIndex]!.interaction_id = 'stage_info'
  assert(qualifyDensity(base, changed, proof, original).includes('DECORATION_AUTHORITY'))
})
test('rejects a new crop moved outside global foot order', () => {
  const changed = structuredClone(candidate)
  const [row] = changed.decorations.splice(addedIndex, 1)
  changed.decorations.unshift(row!)
  assert(qualifyDensity(base, changed, proof, original).includes('GLOBAL_FOOT_ORDER'))
})
test('rejects foliage moved into an exact service label reserve', () => {
  const changed = structuredClone(candidate), row = changed.decorations[addedIndex]!
  const rect = base.serviceLabelAnchors[0]!.exclusionRect
  row.rect = [rect.x+10, rect.y+10, 36, 30]; row.depth_y = rect.y+40
  assert(qualifyDensity(base, changed, proof, original).includes('LABEL_RESERVE'))
})
test('rejects foliage painted on a retained path centerline', () => {
  const changed = structuredClone(candidate), row = changed.decorations[addedIndex]!
  row.rect = [315, 657, 20, 20]; row.depth_y = 677
  assert(qualifyDensity(base, changed, proof, original).includes('PATH_CORRIDOR'))
})
test('rejects retouched source RGBA and stale original provenance', () => {
  const changed = structuredClone(proof)
  changed.additionCropPins[0]!.rgbaSha256 = 'sha256:'+'0'.repeat(64)
  changed.sourceInputs[0]!.sha256 = 'sha256:'+'0'.repeat(64)
  const failures = qualifyDensity(base, candidate, changed, original)
  assert(failures.includes('WHOLE_SOURCE_CROP'))
  assert(failures.includes('ORIGINAL_INPUT_OR_CROP_PINS'))
})
test('rejects corrupted raster bytes and a missing era output', async () => {
  const changed = new Map(readset), key = 'frontend/public/world/map-density-derived-v1/parade-scene.png'
  const raw = Uint8Array.from(changed.get(key)!); raw[100] = raw[100]! ^ 1; changed.set(key, raw)
  assert((await verifyArtifacts(proof, changed)).includes('SHA256:'+key))
  const missing = structuredClone(proof); missing.outputs = missing.outputs.filter(pin => pin.path !== key)
  assert((await verifyArtifacts(missing, readset)).includes('OUTPUT_SET'))
})
