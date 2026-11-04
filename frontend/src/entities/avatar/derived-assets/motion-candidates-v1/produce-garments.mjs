import assert from'node:assert/strict'
import{readFile,writeFile}from'node:fs/promises'
import{createHash}from'node:crypto'
import{pathToFileURL}from'node:url'
import{candidateServer,root,publicPath}from'./server.mjs'
const hash=b=>createHash('sha256').update(b).digest('hex'),catalog=JSON.parse(await readFile(root+'frontend/src/entities/avatar/character-source-pack.json','utf8')),source=catalog.files.find(f=>f.source_id==='photo_06'),m=JSON.parse(await readFile(root+publicPath+'candidate-index.json','utf8'))
assert.equal(hash(await readFile(root+'frontend/public/world/avatars/supplied-v1/'+source.file)),source.sha256)
// Explicit source-coordinate hand windows; they do not remove the intentional
// white collars, pale jacket braid, drawstrings, shirt or other source detail.
const choices=[
 ['row1-col1',[[85,158,36,33],[280,158,36,33]]],
 ['row1-col2',[[385,158,37,33],[581,157,35,34]]],
 ['row1-col3',[[681,183,17,10],[877,183,17,10]]],
 ['row1-col4',[[972,159,36,33],[1168,159,35,33]]],
 ['row3-col2',[[387,617,26,16],[574,617,27,16]]],
 ['row4-col4',[[974,845,35,22],[1173,845,34,23]]],
].map(([location,hands])=>({source:m.items.find(i=>i.sourceId==='photo_06'&&i.location===location),hands}))
const{chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href),server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/tools/pixels.mjs`)
 const result=await page.evaluate(async choices=>{const{decode,register,pack,png}=await import('/tools/pixels.mjs'),parts=[],items=[]
  for(const c of choices){const image=await decode('/world/avatars/derived-v1/motion-candidates-v1/'+c.source.native.file),rect=c.source.rect,b=c.source.native.bounds;let removed=0
   for(const[x,y,w,h]of c.hands)for(let gy=y;gy<y+h;gy++)for(let gx=x;gx<x+w;gx++){const lx=gx-rect.x,ly=gy-rect.y;if(lx<0||ly<0||lx>=image.width||ly>=image.height)continue;const i=(ly*image.width+lx)*4;if(image.rgba[i+3])removed++;image.rgba[i+3]=0}
   const sourceAnchor={x:b.x+b.width/2,y:b.y+b.height/2},f=register(image,sourceAnchor,{x:32,y:32},Math.min(56/b.width,56/b.height));parts.push(f)
   items.push({sourceId:'photo_06',sourceLocation:c.source.location,sourceRect:rect,originalNative:c.source.native,removedHandWindows:c.hands,removedOpaquePixels:removed,role:'HAND_CUT_GARMENT_LAYER_UNFITTED',neckline:'SUPPLIED_UNCHANGED_NO_INFERRED_SKIN',jointRule:'OPEN_SOURCE_HAND_WINDOWS; BODY_HAND_ALIGNMENT_NOT_IMPLEMENTED',productItemId:null,native:{width:image.width,height:image.height,png:png(image.rgba,image.width,image.height)},transform:f.transform})
  }return{items,atlas:pack(parts,6)}
 },choices)
 const files=[];async function save(name,data){const b=Buffer.from(data,'base64'),sha256=hash(b),file=`${name}-${sha256.slice(0,16)}.png`;await writeFile(root+publicPath+file,b);files.push({file,sha256,bytes:b.length});return file}
 for(const item of result.items){item.native.file=await save('hand-cut-garment-'+item.sourceLocation,item.native.png);delete item.native.png}
 result.atlas.file=await save('hand-cut-garments',result.atlas.png);delete result.atlas.png
 await writeFile(root+publicPath+'front-garments.json',JSON.stringify({kind:'SOURCE_PIXEL_HAND_CUT_GARMENT_LAYERS',status:'IN_PROGRESS',productionEligible:false,sourcePack:m.sourcePack,source,items:result.items,atlas:result.atlas,files,qualification:'Six genuine masked garment parts; hand windows removed without filling hidden cloth. Unfitted: not an all-skin body-hand composition or C06 outfit admission.'},null,2)+'\n');console.log(JSON.stringify({result:'MASKED_GARMENT_PARTS_NOT_ADMITTED',parts:6,files:files.length}))
}finally{await browser?.close();await new Promise(r=>server.close(r))}
