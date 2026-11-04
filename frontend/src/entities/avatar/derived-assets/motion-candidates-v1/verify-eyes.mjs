import assert from'node:assert/strict'
import{writeFile,readFile}from'node:fs/promises'
import{createHash}from'node:crypto'
import{pathToFileURL}from'node:url'
import{candidateServer,root,publicPath,evidencePath}from'./server.mjs'
const hash=b=>createHash('sha256').update(b).digest('hex'),m=JSON.parse(await readFile(root+publicPath+'front-eyes.json','utf8')),bytes=await readFile(root+publicPath+m.atlas.file);assert.equal(hash(bytes),m.atlas.sha256)
const{chromium}=await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href),server=candidateServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser
try{browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/tools/review.html`);await page.waitForFunction(()=>window.__MOTION__)
 const report=await page.evaluate(async()=>{
  const{decode}=await import('/tools/pixels.mjs'),{manifest,eyeLayers:m,drawFrame,drawEyes}=window.__MOTION__,a=await decode('/world/avatars/derived-v1/motion-candidates-v1/'+m.atlas.file),c=document.createElement('canvas');c.width=c.height=64;const ctx=c.getContext('2d',{willReadFrequently:true}),cases=[]
  for(const[n,item]of m.items.entries()){const native=await decode('/world/avatars/derived-v1/motion-candidates-v1/'+item.native.file),t=item.transform;for(let y=0;y<64;y++)for(let x=0;x<64;x++){const sx=Math.floor((x+.5-t.targetAnchor.x)/t.scale+t.sourceAnchor.x),sy=Math.floor((y+.5-t.targetAnchor.y)/t.scale+t.sourceAnchor.y),i=(y*a.width+n*64+x)*4,valid=sx>=0&&sy>=0&&sx<native.width&&sy<native.height,si=(sy*native.width+sx)*4,alpha=valid?native.rgba[si+3]:0;if(a.rgba[i+3]!==alpha)throw Error('Eye alpha mismatch');if(alpha)for(let k=0;k<3;k++)if(a.rgba[i+k]!==native.rgba[si+k])throw Error('Eye source pixel mismatch')}}
  for(let body=0;body<8;body++){ctx.clearRect(0,0,64,64);drawFrame(ctx,body);const original=ctx.getImageData(0,0,64,64).data,t=manifest.items[body].registered,rect=manifest.items[body].rect
   for(let eye=0;eye<4;eye++){ctx.clearRect(0,0,64,64);drawEyes(ctx,eye);const pixels=ctx.getImageData(0,0,64,64).data;let oldDark=0,uncovered=0;const positions=[]
    for(let y=0;y<64;y++)for(let x=0;x<64;x++){
     const sx=Math.floor((x+.5-32)/t.scale+t.sourceAnchor.x)+rect.x,sy=Math.floor((y+.5-60)/t.scale+t.sourceAnchor.y)+rect.y,localX=sx-(body%4)*301,localY=sy-Math.floor(body/4)*395,i=(y*64+x)*4
     // Reviewed original eye rectangles, excluding cheek/head outlines.
     if(localY<220||localY>288||!((localX>=96&&localX<=171)||(localX>=211&&localX<=281)))continue
     if(original[i+3]&&Math.max(...original.subarray(i,i+3))<=60){oldDark++;if(!pixels[i+3]){uncovered++;positions.push([x,y])}}
    }
    cases.push({body,eye,oldDark,uncovered,positions})
   }
  }
  return{sourceRgbMismatches:0,cases,qualification:'Original-coordinate eye-rectangle dark-pixel occlusion study; bodies retain baked sclera/face. Not a clean-body or C06 admission.'}
 })
 report.result=report.cases.every(c=>c.uncovered===0)?'PASS_EYE_IRIS_OCCLUSION':'IN_PROGRESS_OCCLUSION_MISMATCH';await writeFile(root+evidencePath+'eye-fit-attempt.json',JSON.stringify(report,null,2)+'\n')
 await page.setViewportSize({width:390,height:844});await page.locator('#body').selectOption('7');await page.locator('#eyes').selectOption('2');await page.locator('#hair').selectOption('none');await page.screenshot({path:root+evidencePath+'eye-fit-attempt.jpg',fullPage:true,type:'jpeg',quality:80});console.log(JSON.stringify({result:report.result,maxUncovered:Math.max(...report.cases.map(c=>c.uncovered))}))
}finally{await browser?.close();await new Promise(r=>server.close(r))}
