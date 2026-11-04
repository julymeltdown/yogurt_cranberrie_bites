import assert from'node:assert/strict'
import{readFile,writeFile}from'node:fs/promises'
import{createHash}from'node:crypto'
import{pathToFileURL}from'node:url'
import{candidateServer,root,publicPath,evidencePath}from'./server.mjs'
const m=JSON.parse(await readFile(root+publicPath+'front-garments.json','utf8')),hash=b=>createHash('sha256').update(b).digest('hex')
for(const f of m.files){const b=await readFile(root+publicPath+f.file);assert.equal(hash(b),f.sha256);assert.equal(b.length,f.bytes)}
const{chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href),server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/tools/pixels.mjs`)
 const result=await page.evaluate(async m=>{const{decode}=await import('/tools/pixels.mjs'),base='/world/avatars/derived-v1/motion-candidates-v1/',atlas=await decode(base+m.atlas.file),cases=[]
  for(const[n,item]of m.items.entries()){const original=await decode(base+item.originalNative.file),native=await decode(base+item.native.file);let removed=0,retained=0,retainedWhite=0
   for(let y=0;y<native.height;y++)for(let x=0;x<native.width;x++){const i=(y*native.width+x)*4,gx=x+item.sourceRect.x,gy=y+item.sourceRect.y,inWindow=item.removedHandWindows.some(([hx,hy,w,h])=>gx>=hx&&gx<hx+w&&gy>=hy&&gy<hy+h),expected=inWindow?0:original.rgba[i+3];if(native.rgba[i+3]!==expected)throw Error('Garment native mask mismatch');if(!expected){if(original.rgba[i+3])removed++;continue}for(let c=0;c<3;c++)if(native.rgba[i+c]!==original.rgba[i+c])throw Error('Retained garment RGB changed');retained++;if(Math.min(...native.rgba.subarray(i,i+3))>=230)retainedWhite++}
   if(!removed||removed!==item.removedOpaquePixels)throw Error('Missing hand cut');const t=item.transform
   for(let y=0;y<64;y++)for(let x=0;x<64;x++){const sx=Math.floor((x+.5-t.targetAnchor.x)/t.scale+t.sourceAnchor.x),sy=Math.floor((y+.5-t.targetAnchor.y)/t.scale+t.sourceAnchor.y),i=(y*atlas.width+n*64+x)*4,si=(sy*native.width+sx)*4,valid=sx>=0&&sy>=0&&sx<native.width&&sy<native.height,alpha=valid?native.rgba[si+3]:0;if(atlas.rgba[i+3]!==alpha)throw Error('Garment atlas alpha mismatch');if(alpha)for(let c=0;c<3;c++)if(atlas.rgba[i+c]!==native.rgba[si+c])throw Error('Garment atlas sample mismatch')}
   cases.push({sourceLocation:item.sourceLocation,removedHandPixels:removed,retained,retainedWhite,outsideHandWindowPixelChanges:0})
  }return{result:'PASS',qualification:'Six real hand-cut garment parts; source pixels outside explicit hand windows unchanged. Unfitted: not skin/body-hand joint or production admission.',cases}
 },m)
 await writeFile(root+evidencePath+'garment-validation.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({result:result.result,parts:result.cases.length}))
}finally{await browser?.close();await new Promise(r=>server.close(r))}
