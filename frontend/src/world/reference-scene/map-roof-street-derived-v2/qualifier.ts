/** Pure source admission. No consumer, registry, renderer or resource loader. */
export type Rect = readonly [number, number, number, number]
type Pin = {path: string; bytes: number; sha256: string}
type Row = {id: string; frame: string; rect: Rect; depth_y: number; crop_role: string}
type Crop = {themeId: string; frame: string; sourceRect: {x: number; y: number; width: number; height: number}; rgbaSha256: string; atlas: Pin}
type Plan = {id: string; renderedRect: Rect; roofDomain: Rect; volumes: {id: string; rect: Rect; profile: string}[]}
export interface Layout {
  [key: string]: unknown
  version: string; decorations: Row[]
  serviceLabelAnchors: {exclusionRect: {x: number; y: number; width: number; height: number}}[]
  roofStreetDerivation?: {sourceCommit: string; baselinePins: Pin[]; roofPlansByTheme: Record<string, Plan[]>}
}
export interface Proof {
  version: string; sourceCommit: string; baselinePins: Pin[]; sourceInputs: Pin[]
  originalCropPins: Crop[]; additionCropPins: unknown[]; outputs: Pin[]; recipe: Pin
  roofPlansByTheme: Record<string, Plan[]>
  actualByTheme: Record<string, {changedPixels: number; roofChangedPixels: number; streetChangedPixels: number; roofs: number; volumes: number; pavingFaces: number; pavingRects: Rect[]}>
  protectedLabelRectanglesExact: number; incumbentDecorationsExact: number; sourceRgbaPolicy: string
  helperReuse: {path: string; sha256: string}
  license: {newVectors: string; inheritedSources: string; sourceRecord: string; inputPins: Pin[]}
}
export type Original = Pick<Proof, 'sourceInputs' | 'originalCropPins' | 'additionCropPins'>
const PREFIX = 'frontend/public/world/map-roof-street-derived-v2/'
const BASE_PREFIX = 'frontend/public/world/map-density-derived-v1/'
const SOURCE = '7acfac3aed91f1eb06fa48eee63ce3da1431fadc'
const SAME = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)
const ERAS = ['REVENGE','PARADE','DANGER']
const overlap = (a: Rect,b: Rect) => a[0] < b[0]+b[2] && b[0] < a[0]+a[2] && a[1] < b[1]+b[3] && b[1] < a[1]+a[3]
const FROZEN: Record<string,string> = {
  'README.md':'96098bb840fb4e047d5149571b2a6e64723c516c5784271042a41d1ff464c7ff',
  'build.py':'4f4f6eede3413d7fb0502103f543a563fb88e69e26600fd177c71648d2ff7539',
  'layout.json':'971c9a94cd1aa01c771f3defa8cede70754c87f43eb4f7fb0ed4cf98921096eb',
  'source-provenance.json':'66b18e7c0f235f46828b87811ba344da3a4dc1c6025ae5622493026444b9d617',
  'revenge-scene.png':'7d657d5cd22ceafa4edf4bff7df5b75843927208ae8a8704747c4a645b51e071',
  'parade-scene.png':'a8ac1c806adfc3cd36af21c655131b3c19e80034528632c23001eb49adcae9e1',
  'danger-scene.png':'b54be823fe26b659b7223f4d2baacbdc2a232d8b1b78926540b87e790a05ec1b',
  'revenge-scene.svg':'40b1bbe84a3857861f4079563ef5cc10b80f2e564f1902d008c1bcfef8203aa2',
  'parade-scene.svg':'b66ed2c2c4ad70a109e3a5f2b4d9bb26c5ed849f05e1b800a1b9ba1e30ef4b7d',
  'danger-scene.svg':'197249431cccf8b50741f494da88f1283140cc28ee4a70141869ce5a35da57b1',
}
function projection(row: Row,crop: Crop): Rect {
  const [, ,w,h] = row.rect, sw = crop.sourceRect.width, sh = crop.sourceRect.height
  const scale = Math.min(w/sw,h/sh), rw = Math.max(1,Math.floor(sw*scale+.5)), rh = Math.max(1,Math.floor(sh*scale+.5))
  return [row.rect[0]+Math.floor((w-rw)/2),row.rect[1]+h-rh,rw,rh]
}

/** Expects parsed typed records; this is not a consumer document boundary. */
export function qualifyRoofStreet(base: Layout, candidate: Layout, proof: Proof, original: Original): readonly string[] {
  const reasons = new Set<string>(), fail = (s: string) => { reasons.add(s) }
  if (candidate.version !== 'map-roof-street-derived-v2' || proof.version !== candidate.version
    || proof.sourceCommit !== SOURCE || candidate.roofStreetDerivation?.sourceCommit !== SOURCE) fail('SOURCE_IDENTITY')
  for (const key of Object.keys(base)) if (!['version','recipeRevision','assets'].includes(key) && !SAME(base[key],candidate[key])) fail('IMMUTABLE_LAYOUT:'+key)
  if (base.decorations.length !== 285 || candidate.decorations.length !== 285
    || proof.incumbentDecorationsExact !== 285 || !SAME(base.decorations,candidate.decorations)) fail('INCUMBENT_DECORATION')
  const pins = new Map(proof.baselinePins.map(p => [p.path,p.sha256]))
  if (pins.size !== 10 || proof.baselinePins.length !== 10 || Object.entries(FROZEN).some(([n,h]) => pins.get(BASE_PREFIX+n) !== 'sha256:'+h)
    || !SAME(proof.baselinePins,candidate.roofStreetDerivation?.baselinePins)) fail('BASELINE_PINS')
  if (!SAME(proof.sourceInputs,original.sourceInputs) || !SAME(proof.originalCropPins,original.originalCropPins)
    || !SAME(proof.additionCropPins,original.additionCropPins) || proof.sourceRgbaPolicy !== 'EXACT_ORIGINAL_CROPS_UNCHANGED_PROJECTION') fail('ORIGINAL_RGBA_OR_INPUT')
  if (proof.protectedLabelRectanglesExact !== 18) fail('LABEL_RECTANGLES')
  if (proof.license.sourceRecord !== 'frontend/public/world/reference-scene-v2/map-cozy-v1/README.md'
    || !proof.license.newVectors || !proof.license.inheritedSources || proof.license.inputPins.length !== 3
    || ['README.md','prompt.txt','source-original.png'].some(n => !proof.license.inputPins.some(p => p.path === 'frontend/public/world/reference-scene-v2/map-cozy-v1/'+n))) fail('SOURCE_LICENSE_LINEAGE')
  if (!SAME(proof.roofPlansByTheme,candidate.roofStreetDerivation?.roofPlansByTheme)) fail('ROOF_PROOF')
  const ids = new Map(base.decorations.map(row => [row.id,row]))
  for (const era of ERAS) {
    const plans = proof.roofPlansByTheme[era]!, stats = proof.actualByTheme[era]!
    if (plans.length < 12 || new Set(plans.map(p => p.id)).size !== plans.length || stats.roofs !== plans.length
      || stats.volumes !== plans.reduce((n,p) => n+p.volumes.length,0)) fail('ROOF_COVERAGE')
    if (stats.changedPixels <= 15000 || stats.roofChangedPixels <= 5000 || stats.streetChangedPixels <= 5000
      || stats.pavingFaces < 350 || stats.pavingRects.length !== stats.pavingFaces) fail('ACTUAL_ROOF_AND_STREET_DELTA')
    for (const plan of plans) {
      const row = ids.get(plan.id), crop = row && original.originalCropPins.find(p => p.themeId === era && p.frame === row.frame)
      if (!row || !crop || !['meeting_pavilion','giveaway_pavilion','north_hall','west_hall'].includes(row.frame)) { fail('ROOF_AUTHORITY'); continue }
      if (!SAME(plan.renderedRect,projection(row,crop))) fail('ORIGINAL_PROJECTION')
      const [x,y,w,h] = plan.renderedRect, top = h-Math.max(1,Math.floor(h*.46*.80+.5))
      if (!SAME(plan.roofDomain,[x,y,w,Math.max(top,Math.ceil(h*.72))])) fail('ROOF_DOMAIN')
      if (base.serviceLabelAnchors.some(({exclusionRect:r}) => overlap(plan.roofDomain,[r.x,r.y,r.width,r.height]))) fail('LABEL_RESERVE')
      if (!plan.volumes.length) fail('ROOF_VOLUME')
      for (const [index,v] of plan.volumes.entries()) {
        const [vx,vy,vw,vh] = v.rect
        if (v.id !== plan.id+'_volume_'+index || !['hip','front_gable','saltbox'].includes(v.profile)
          || !v.rect.every(Number.isInteger) || vx < 0 || vy < 0 || vw < 1 || vh < 1 || vx+vw > w || vy+vh > top) fail('ROOF_VOLUME')
        if (index && !overlap(plan.volumes[index-1]!.rect,v.rect)) fail('DISCONNECTED_ROOF')
      }
    }
    for (const r of stats.pavingRects) {
      if (!r.every(Number.isInteger) || r[0] < 0 || r[1] < 0 || r[2] !== 12 || r[3] !== 7 || r[0]+12 > 768 || r[1]+7 > 1296) fail('PAVING_RECT')
      if (base.serviceLabelAnchors.some(({exclusionRect:e}) => overlap(r,[e.x,e.y,e.width,e.height]))) fail('LABEL_RESERVE')
    }
    if (!SAME(plans,proof.roofPlansByTheme.PARADE)) fail('ERA_GEOMETRY')
  }
  return [...reasons]
}

export async function verifyArtifacts(proof: Proof, readset: ReadonlyMap<string,Uint8Array>): Promise<readonly string[]> {
  const failures: string[] = []
  const expected = ['layout.json',...ERAS.flatMap(e => ['png','svg'].map(ext => e.toLowerCase()+'-scene.'+ext))]
  if (proof.outputs.length !== 7 || new Set(proof.outputs.map(p => p.path)).size !== 7
    || expected.some(n => !proof.outputs.some(p => p.path === PREFIX+n))) failures.push('OUTPUT_SET')
  const helper = proof.helperReuse
  if (helper.path !== 'frontend/public/world/map-reference-composition-v1/build.py'
    || helper.sha256 !== 'sha256:6bf696b7c3edd5adf35e0b5995ca9cd33f915e87898de0e9266f2923cffc0433') failures.push('HELPER_PIN')
  const pinsToRead = [...proof.baselinePins,...proof.sourceInputs,...proof.license.inputPins,...proof.outputs,proof.recipe]
  for (const p of pinsToRead) {
    const raw = readset.get(p.path)
    if (!raw || raw.byteLength !== p.bytes) { failures.push('BYTES:'+p.path); continue }
    const hash = await crypto.subtle.digest('SHA-256',Uint8Array.from(raw))
    if ('sha256:'+Array.from(new Uint8Array(hash),v => v.toString(16).padStart(2,'0')).join('') !== p.sha256) failures.push('SHA256:'+p.path)
  }
  return failures
}
