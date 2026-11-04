import assert from'node:assert/strict'
import{readFile,writeFile}from'node:fs/promises'
import{createHash}from'node:crypto'
import{pathToFileURL}from'node:url'
import{candidateServer,root,publicPath}from'./server.mjs'
const hash=b=>createHash('sha256').update(b).digest('hex'),presets=JSON.parse(await readFile(root+publicPath+'motion-release-candidate.json','utf8')),wardrobe=JSON.parse(await readFile(root+publicPath+'wardrobe-release-candidate.json','utf8'))
assert.equal(presets.frames.length,120);assert.equal(wardrobe.physicalFrames,456)
const{chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href),server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/tools/pixels.mjs`)
 const result=await page.evaluate(async({presets,wardrobe})=>{const{decode,pack}=await import('/tools/pixels.mjs'),parts=[];for(const[m,count]of[[presets,120],[wardrobe,456]]){const source=await decode('/world/avatars/derived-v1/motion-candidates-v1/'+m.atlas.file);for(let n=0;n<count;n++){const rgba=new Uint8ClampedArray(4096*4);for(let y=0;y<64;y++){const i=((Math.floor(n/20)*64+y)*source.width+n%20*64)*4;rgba.set(source.rgba.subarray(i,i+256),y*256)}parts.push({rgba,width:64,height:64})}}return pack(parts,20)},{presets,wardrobe})
 const b=Buffer.from(result.png,'base64'),sha256=hash(b),file=`avatar-candidates-64-${sha256.slice(0,16)}.png`;await writeFile(root+publicPath+file,b);delete result.png;const atlas={...result,file,sha256,bytes:b.length}
 for(const item of wardrobe.items)for(const view of item.directions){view.frames=view.frames.map(n=>n+120);if(view.handFrames)view.handFrames=view.handFrames.map(n=>n+120)}
 presets.atlas=atlas;wardrobe.atlas=atlas;wardrobe.ownPhysicalFrames=wardrobe.physicalFrames;wardrobe.physicalFrames=576
 const m={kind:'SELECTED_CHARACTER_COMBINED_ART_CANDIDATE',status:'IN_PROGRESS',productionEligible:false,sourcePack:presets.sourcePack,atlas,physicalFrames:576,presets,wardrobe,qualification:'One shared 64px texture for six complete four-direction presets and independently composable wardrobe. Authored/supplied attribution preserved, no inventory/C06/product adoption.'}
 await writeFile(root+publicPath+'combined-release-candidate.json',JSON.stringify(m,null,2)+'\n');console.log(JSON.stringify({result:'COMBINED_NOT_ADMITTED',physicalFrames:576,file,bytes:b.length}))
}finally{await browser?.close();await new Promise(r=>server.close(r))}
