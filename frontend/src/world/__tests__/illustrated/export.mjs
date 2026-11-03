// Deterministic browser export of original native SVG detail groups. No original is modified.
import { chromium } from '@playwright/test'
import { createServer } from 'vite'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
const frontend=resolve(import.meta.dirname,'../../../..'),out=resolve(frontend,'public/world/illustrated-runtime')
await mkdir(out,{recursive:true})
const source=JSON.parse(await readFile(resolve(frontend,'public/world/illustrated/binding-proposal.json')))
const receipt=JSON.parse(await readFile(resolve(frontend,'public/world/illustrated/exported-resources.json')))
const server=await createServer({root:frontend,configLoader:'runner',cacheDir:'/tmp/mcr-illustrated-export-cache',server:{host:'127.0.0.1',port:0,strictPort:false},logLevel:'warn'});await server.listen()
const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
const sha=bytes=>'sha256:'+createHash('sha256').update(bytes).digest('hex')
const result={kind:'ILLUSTRATED_RUNTIME_BINDING_NOT_BOOTSTRAP_OR_AVATAR_RELEASE',version:1,sourceRevision:'19d7085 (integrated bc60edb)',browser:browser.version(),referenceHash:source.sources.at(-1).sha256,geometryHash:source.geometry_hash,sourceHashes:{},themes:[]}
try{
 const page=await browser.newPage(),base=`http://127.0.0.1:${server.httpServer.address().port}`
 for(const era of ['REVENGE','PARADE','DANGER']){
  const lower=era.toLowerCase(),path=`world/illustrated/${lower}-landmarks.svg`,svg=await readFile(resolve(frontend,'public',path))
  result.sourceHashes[path]=sha(svg)
  await page.goto(`${base}/${path}`)
  const exported=await page.evaluate(async()=>{
   await document.fonts.ready
   const groups=Array.from(document.querySelectorAll('g[id]')).filter(g=>/^(label-|lamp-|banner-)/.test(g.id)),canvas=document.createElementNS('http://www.w3.org/1999/xhtml','canvas');canvas.width=512;canvas.height=256
   const ctx=canvas.getContext('2d');ctx.imageSmoothingEnabled=false
   let x=0,y=0,row=0;const objects=[]
   for(const g of groups){
    const b=g.getBBox(),left=Math.floor(b.x)-2,top=Math.floor(b.y)-2,width=Math.ceil(b.x+b.width)-left+2,height=Math.ceil(b.y+b.height)-top+2
    if(x+width>512){x=0;y+=row+2;row=0}if(y+height>256)throw Error('Detail atlas overflow')
    const xml=`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="${left} ${top} ${width} ${height}">${new XMLSerializer().serializeToString(g)}</svg>`,image=new Image(),url=URL.createObjectURL(new Blob([xml],{type:'image/svg+xml'}))
    try{image.src=url;await image.decode();ctx.drawImage(image,x,y)}finally{URL.revokeObjectURL(url)}
    let depth=-1
    if(g.id.startsWith('banner-'))depth=Number(g.id.split('-')[2])+76
    else if(g.id.startsWith('lamp-'))depth=Number(g.id.split('-')[2])+3
    else depth=({'label-STUDIO':288,'label-LIVESTAGE':480,'label-MERCHGIFTS':384,'label-PIXELWALL':704,'label-PLAZA':-1})[g.id]
    if(depth===undefined)throw Error('Unbound detail '+g.id)
    objects.push({id:g.id.toLowerCase(),atlas_id:'details',frame:{x,y,width,height},x:left,y:top,width,height,depth_y:depth});x+=width+2;row=Math.max(row,height)
   }
   return{png:canvas.toDataURL('image/png').split(',')[1],objects}
  })
  const png=Buffer.from(exported.png,'base64'),detailPath=`world/illustrated-runtime/${lower}-details.png`;await writeFile(resolve(frontend,'public',detailPath),png)
  const descriptor=(suffix,id)=>{const r=receipt.resources.find(r=>r.path===`world/illustrated/${lower}-${suffix}.png`);return{id,path:r.path,sha256:r.sha256,width_px:r.width_px,height_px:r.height_px,bytes:r.bytes}}
  const scenery={version:1,ground:descriptor('ground','ground'),atlases:[descriptor('landmarks-atlas','landmarks'),{id:'details',path:detailPath,sha256:sha(png),width_px:512,height_px:256,bytes:png.length}],objects:[...source.placements.map(o=>({id:o.id,atlas_id:'landmarks',frame:{x:o.atlas_frame[0],y:o.atlas_frame[1],width:o.atlas_frame[2],height:o.atlas_frame[3]},x:o.x,y:o.y,width:o.width,height:o.height,depth_y:o.depth_y})),...exported.objects]}
  await writeFile(resolve(out,`${lower}-scenery.json`),JSON.stringify(scenery,null,2)+'\n');result.themes.push({theme_id:era,scenery})
 }
 for(const name of ['binding-proposal.json','exported-resources.json'])result.sourceHashes[`world/illustrated/${name}`]=sha(await readFile(resolve(frontend,'public/world/illustrated',name)))
 await writeFile(resolve(out,'bindings.json'),JSON.stringify(result,null,2)+'\n')
}finally{await browser.close();await server.close()}
