import assert from'node:assert/strict'
import{readFile,writeFile}from'node:fs/promises'
import{createHash}from'node:crypto'
import{pathToFileURL}from'node:url'
import{candidateServer,root,publicPath}from'./server.mjs'
const source=JSON.parse(await readFile(root+publicPath+'candidate-index.json','utf8')),hash=b=>createHash('sha256').update(b).digest('hex')
const choices=[['row1-col1','Red bow',.045,44,16],['row1-col2','Black bow',.045,44,16],['row2-col2','Cat-ear headband',.13,32,20],['row2-col3','Headphones',.145,32,25]].map(([location,label,scale,x,y])=>({label,scale,targetAnchor:{x,y},source:source.items.find(i=>i.sourceId==='photo_19'&&i.location===location)}))
for(const c of choices){const f=source.files.find(f=>f.file===c.source.native.file);assert.equal(hash(await readFile(root+publicPath+f.file)),f.sha256)}
const{chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href),server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/tools/pixels.mjs`)
 const result=await page.evaluate(async choices=>{const{decode,register,pack}=await import('/tools/pixels.mjs'),parts=[],items=[]
  for(const c of choices){const native=await decode('/world/avatars/derived-v1/motion-candidates-v1/'+c.source.native.file),b=c.source.native.bounds,sourceAnchor={x:b.x+b.width/2,y:b.y+b.height/2},f=register(native,sourceAnchor,c.targetAnchor,c.scale);parts.push(f);items.push({label:c.label,sourceId:c.source.sourceId,sourceLocation:c.source.location,sourceRect:c.source.rect,native:c.source.native,transform:f.transform,slot:'FRONT_HEAD_ACCESSORY_CANDIDATE',productItemId:null,occlusion:'ABOVE_FRONT_HAIR',direction:'front',skinIndependent:true})}
  return{items,atlas:pack(parts,4)}
 },choices)
 const b=Buffer.from(result.atlas.png,'base64'),sha256=hash(b),file=`front-head-attachments-${sha256.slice(0,16)}.png`;await writeFile(root+publicPath+file,b);delete result.atlas.png
 const manifest={kind:'SOURCE_PIXEL_FRONT_HEAD_ATTACHMENTS',status:'IN_PROGRESS',productionEligible:false,sourcePack:source.sourcePack,items:result.items,atlas:{...result.atlas,file,sha256,bytes:b.length},qualification:'Four actual source-pixel independently toggled front attachments only, no back/side geometry, wardrobe item IDs or production adoption.'}
 await writeFile(root+publicPath+'front-attachments.json',JSON.stringify(manifest,null,2)+'\n');console.log(JSON.stringify({result:'GENERATED_NOT_ADMITTED',attachments:4,file}))
}finally{await browser?.close();await new Promise(r=>server.close(r))}
