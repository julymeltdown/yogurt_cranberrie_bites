import assert from 'node:assert/strict'
import {readFile,writeFile} from 'node:fs/promises'
import {createHash} from 'node:crypto'
import {pathToFileURL} from 'node:url'
import {candidateServer,root,publicPath,evidencePath} from './server.mjs'
const hash=b=>createHash('sha256').update(b).digest('hex')
const catalog=JSON.parse(await readFile(root+'frontend/src/entities/avatar/character-source-pack.json','utf8'))
const source=catalog.files.find(f=>f.source_id==='photo_01'),bytes=await readFile(root+'frontend/public/world/avatars/supplied-v1/'+source.file)
assert.equal(hash(bytes),source.sha256);assert.equal(bytes.length,source.byte_length)
assert(process.env.MCR_ART_PLAYWRIGHT)
const{chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href)
const server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser
try{
 browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
 const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/tools/pixels.mjs`)
 const result=await page.evaluate(async source=>{
  const{decode,cut,pack}=await import('/tools/pixels.mjs'),{rigs,poses,articulate}=await import('/tools/rig.mjs')
  const original=await decode('/world/avatars/supplied-v1/'+source.file),frames=[],parts=[],maps=[]
  for(const rig of rigs){const part=cut(original,rig.rect)
   for(const pose of poses){const f=articulate(part,rig,pose);parts.push(f);maps.push(...f.provenance)
    frames.push({preset:rig.id,pose:pose.name,direction:'front',scale:f.scale,sourceAnchor:f.anchor,rig,offsets:pose})
   }
  }
  const binary=new Uint8Array(maps.length*2),v=new DataView(binary.buffer);maps.forEach((n,i)=>v.setInt16(i*2,n,true))
  let s='';for(const n of binary)s+=String.fromCharCode(n)
  return{frames,atlas:pack(parts,5),provenance:btoa(s),browserSourceDimensions:[original.width,original.height]}
 },source)
 const files=[]
 async function save(name,data,extension){const b=Buffer.from(data,'base64'),sha256=hash(b),file=`${name}-${sha256.slice(0,16)}.${extension}`;await writeFile(root+publicPath+file,b);files.push({file,sha256,bytes:b.length});return file}
 const atlas={...result.atlas,file:await save('front-articulated-gait',result.atlas.png,'png')};delete atlas.png
 const provenance=await save('front-gait-source-pixel-map',result.provenance,'bin')
 const manifest={kind:'SOURCE_PIXEL_ARTICULATED_FRONT_GAIT',status:'IN_PROGRESS',productionEligible:false,sourcePack:{id:catalog.source_pack_id,revision:catalog.revision},source:{...source},baseCommit:'a3526254a716e14e3134de551c3b277379a1096f',method:'Reviewed source-coordinate limb partitions; independent translations under 12-source-pixel stationary joint overlap; one nearest-source sample per output pixel. No new design pixels or hidden-surface fill.',qualification:'Authored front walking derivative, not supplied motion capture, full-direction atlas, slot contract, inventory or production admission.',frameSize:[64,64],foot:[32,60],directions:['front'],missingDirections:['left','right','rear'],frames:result.frames,atlas,provenance:{file:provenance,format:'little-endian int16 [originalX,originalY] per pixel, frame-major 64x64; -1,-1 transparent'},files}
 await writeFile(root+publicPath+'front-gait-index.json',JSON.stringify(manifest,null,2)+'\n')
 await writeFile(root+evidencePath+'front-gait-generation.json',JSON.stringify({result:'GENERATED_NOT_ADMITTED',sourceHashVerified:true,frames:result.frames.length,authoredWalkFrames:24,originalDimensions:result.browserSourceDimensions,browser:browser.version()},null,2)+'\n')
 console.log(JSON.stringify({result:'GENERATED_NOT_ADMITTED',frames:30,authoredWalkFrames:24,files}))
}finally{await browser?.close();await new Promise(r=>server.close(r))}
