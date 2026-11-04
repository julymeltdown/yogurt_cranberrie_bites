import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {readFile} from 'node:fs/promises'
import {join} from 'node:path'
import {fileURLToPath} from 'node:url'
import test from 'node:test'
import {facadeMasks,BOUNDS,type Layout,type Proof} from './qualifier.ts'
import {decodePng} from './png.ts'

assert.equal(process.versions.node,'24.22.2')
const root=fileURLToPath(new URL('../../../../../',import.meta.url))
const basePath='frontend/public/world/map-roof-street-derived-v2/'
const load=async<T>(path:string):Promise<T>=>JSON.parse(await readFile(path,'utf8')) as T
type Components=Proof['componentsByTheme'][string]
type Era={file:string;bytes:number;sha256:string;frozenRecompositionExact:boolean;changedPixels:number;
 components:Components;trueGaps:Proof['actualByTheme'][string]['trueGaps'];masks:Proof['facadeMaskPins'][string]}
const tmp=process.argv[2]!
const foundation=await load<{format:string;sourceCommit:string;eras:Record<string,Era>}>(join(tmp,'foundation.json'))
const base=await load<Layout>(root+basePath+'layout.json')
const original=await load<Proof>(root+basePath+'source-provenance.json')
const sha=(v:Uint8Array)=>'sha256:'+createHash('sha256').update(v).digest('hex')
assert.equal(foundation.format,'FACADE_FOUNDATION_ONLY')
assert.equal(foundation.sourceCommit,'da33467cd084ecba071244a1939b6217f4b0de02')
assert.deepEqual(Object.keys(foundation.eras),['REVENGE','PARADE','DANGER'])
for(const [era,stats]of Object.entries(foundation.eras)) {
 const raw=await readFile(join(tmp,stats.file)),after=decodePng(raw)
 const before=decodePng(await readFile(root+basePath+era.toLowerCase()+'-scene.png'))
 const atlasPin=original.originalCropPins.find(p=>p.themeId===era)!.atlas
 const atlasRaw=await readFile(root+atlasPin.path)
 assert.equal(sha(atlasRaw),atlasPin.sha256)
 const masks=facadeMasks(base,original,era,decodePng(atlasRaw))
 test(era+' original alpha, foreground, protected regions and actual facade deltas',()=>{
  assert.equal(stats.frozenRecompositionExact,true);assert.equal(raw.length,stats.bytes);assert.equal(sha(raw),stats.sha256)
  assert.equal(after.width,768);assert.equal(after.height,1296)
  const union=new Uint8Array(768*1296)
  for(const [id,mask]of Object.entries(masks)) {
   assert.equal(sha(mask),stats.masks[id]!.sha256);assert.equal(mask.length,stats.masks[id]!.bytes)
   assert.equal(mask.reduce((n,v)=>n+(v===255?1:0),0),stats.masks[id]!.pixels)
   for(let i=0;i<mask.length;i++)if(mask[i])union[i]=255
   const components=stats.components[id]!
   assert(components.some(c=>c.role==='negative_gap'));assert(components.some(c=>c.role==='recessed_bay'))
   for(const c of components)for(let y=c.rect[1];y<c.rect[1]+c.rect[3];y++)for(let x=c.rect[0];x<c.rect[0]+c.rect[2];x++)assert.equal(mask[y*768+x],255,c.id)
  }
  let count=0
  for(let i=0;i<union.length;i++) {
   const at=i*4
   if([0,1,2].every(c=>before.rgba[at+c]===after.rgba[at+c]))continue
   assert.equal(union[i],255,'Change outside actual facade alpha at '+i);count++
  }
  assert.equal(count,stats.changedPixels);assert(count>=1000)
 })
 test(era+' true gap pixels equal underlying recomposed scene',()=>{
  assert(stats.trueGaps.length>=3)
  for(const gap of stats.trueGaps) {
   const [x,y,w,h]=gap.rect,rgb=new Uint8Array(w*h*3);let n=0
   for(let yy=y;yy<y+h;yy++)for(let xx=x;xx<x+w;xx++)for(let c=0;c<3;c++)rgb[n++]=after.rgba[(yy*768+xx)*4+c]!
   assert.equal(sha(rgb),gap.underlyingRgbSha256)
   const component=Object.values(stats.components).flat().find(c=>c.id===gap.id)!
   assert.deepEqual(component.operations,[{kind:'clear',rect:gap.rect}])
  }
 })
}
test('all three named facade plans retain identical geometry across eras',()=>{
 for(const stats of Object.values(foundation.eras)) {
  assert.deepEqual(Object.keys(stats.components).sort(),Object.keys(BOUNDS).sort())
  assert.deepEqual(stats.components,foundation.eras.PARADE!.components)
 }
})
