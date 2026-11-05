import test from 'node:test'
import assert from 'node:assert/strict'
import {readFile} from 'node:fs/promises'
import {fileURLToPath} from 'node:url'
import {qualifyPack,qualifyInterface,qualifyImage,jpegDimensions,photo5} from './qualify.ts'
const root=fileURLToPath(new URL('../../../../../',import.meta.url))
const pack=root+'frontend/public/venue/paradise-city/v1/'
const original=JSON.parse(await readFile(pack+'asset-interface.json','utf8'))
const jpeg=await readFile(pack+'siteplan-photo-5.jpg')
const clone=()=>structuredClone(original)
test('real three-file pack, exact supplied JPEG and single source-anchor/provenance closure qualify',async()=>{
 const result=await qualifyPack(root);assert.equal(result.imageBytes,328720);assert.equal(result.imageSHA256,photo5.sha256);assert.equal(result.sourceCandidates,22);console.log(JSON.stringify(result))
})
test('independent JPEG header identifies full unchanged board and rejects malformed input',()=>{
 assert.deepEqual(jpegDimensions(jpeg),{width:1024,height:1280});assert.throws(()=>jpegDimensions(new Uint8Array([255,216,255])))
 const forged=Buffer.from(jpeg);forged[200]^=1;assert(qualifyImage(forged).includes('EXACT_JPEG_BYTES'))
 assert(qualifyImage(jpeg.subarray(0,100)).includes('EXACT_JPEG_BYTES'))
})
test('rejects a mixed primary image, world alias and geographic coordinate brand',()=>{
 const v=clone();v.variantId='photo-1';v.image.path='world/map-facade-derived-v3/parade-scene.png';v.sourcePixelCoordinateSpace='wgs84'
 assert(qualifyInterface(v).includes('VARIANT_IDENTITY'));assert(qualifyInterface(v).includes('EXACT_IMAGE'));assert(qualifyInterface(v).includes('PRESENTATION_COORDINATES'))
})
test('rejects dimensions or bytes/hash not exactly bound to Photo 5',()=>{
 for(const [field,value] of [['width',768],['height',1296],['bytes',328721],['sha256','0'.repeat(64)]]){const v=clone();v.image[field]=value;assert(qualifyInterface(v).includes('EXACT_IMAGE'))}
})
test('rejects duplicated/invalid source keys and missing mandatory pictured landmarks',()=>{
 const v=clone();v.landmarks[1].id='stage';assert(qualifyInterface(v).includes('CANDIDATE_ID'));assert(qualifyInterface(v).includes('COMPLETE_PICTURED_PLAN_ANCHORS'))
 v.landmarks[1].id='GPS/Place';assert(qualifyInterface(v).includes('CANDIDATE_ID'))
})
test('rejects negative, nonfinite, fractional, outside-image and half-open region-edge anchors',()=>{
 for(const x of [-1,1024,NaN,Infinity,240.5]){const v=clone();v.landmarks[0].anchor.x=x;assert(qualifyInterface(v).includes('PIXEL_BOUNDS'))}
 const v=clone();v.landmarks[0].anchor.x=v.landmarks[0].hitRegion.x+v.landmarks[0].hitRegion.width;assert(qualifyInterface(v).includes('PIXEL_BOUNDS'))
})
test('rejects zero/negative/overflow hit regions without clipping',()=>{
 for(const [field,value] of [['width',0],['height',-1],['x',1000],['y',1280]]){const v=clone();v.landmarks[0].hitRegion[field]=value;assert(qualifyInterface(v).includes('PIXEL_BOUNDS'))}
})
test('rejects wrong reference pins and unsupported per-landmark source references',()=>{
 const v=clone();v.sourceRefs[0].sha256='0'.repeat(64);v.landmarks[0].sourceRefs=['unknown'];assert(qualifyInterface(v).includes('EXACT_SOURCE_REFS'));assert(qualifyInterface(v).includes('CANDIDATE_SOURCE_REFS'))
})
test('Photo Zone label requires its explicit Notes reference, with geometry kept on Photo 5',()=>{
 const v=clone();v.landmarks.find((a:any)=>a.id==='photo-zone').sourceRefs=['photo-5'];assert(qualifyInterface(v).includes('PHOTO_ZONE_LABEL_CORROBORATION'))
 v.landmarks[0].sourceRefs=['photo-10'];assert(qualifyInterface(v).includes('CANDIDATE_SOURCE_REFS'))
})
test('rejects forged operational review or extra GPS/live-user/permission authority',()=>{
 const v=clone();v.landmarks[0].review='VERIFIED_OPEN';assert(qualifyInterface(v).includes('SOURCE_REVIEW'))
 v.landmarks[0].latitude=37;assert(qualifyInterface(v).includes('CANDIDATE_FIELDS'))
 const w=clone();w.phoneLocation={live:true};assert(qualifyInterface(w).includes('ENVELOPE'))
})
test('rejects source labels containing hidden control characters or padding',()=>{
 for(const label of [' Stage','STAGE\n','', 'x'.repeat(121)]){const v=clone();v.landmarks[0].displayName=label;assert(qualifyInterface(v).includes('SOURCE_LABEL'))}
})
test('rejects unbounded source lists and non-finite/unknown image-pixel geometry fields',()=>{
 const v=clone();v.landmarks.push(...Array(128).fill(v.landmarks[0]));assert(qualifyInterface(v).includes('LANDMARK_ARRAY'))
 const w=clone();w.landmarks[0].anchor.latitude=0;assert(qualifyInterface(w).includes('PIXEL_GEOMETRY'))
})
