import assert from 'node:assert/strict'
import {createRequire} from 'node:module'
import fs from 'node:fs'
import path from 'node:path'
import {fileURLToPath} from 'node:url'
const here=path.dirname(fileURLToPath(import.meta.url)),out=path.join(here,'runtime-v4')
const require=createRequire(process.env.MCR_PLAYWRIGHT_PACKAGE||path.resolve(here,'../../../package.json'))
const {chromium}=require('@playwright/test')
const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
const base=process.env.MCR_RUNTIME_BASE||'http://127.0.0.1:8896/frontend/public/world/reference-scene-v2/'
const cases=[],errors=[]
try{
 for(const width of [360,390,430]){
  const page=await browser.newPage({viewport:{width:width*2+32,height:900},deviceScaleFactor:1})
  page.on('pageerror',e=>errors.push(e.message))
  await page.goto(base+'review.html');await page.waitForFunction(()=>window.referenceSceneReview?.ready)
  for(const view of ['world','map']){
   const result=await page.evaluate(async({base,width,view})=>{
    const read=async p=>{const r=await fetch(base+p);if(!r.ok)throw Error(p);return r.json()}
    const [proposal,source,compact,meta]=await Promise.all(['geometry-proposal.json','painted/layout.json','runtime-v4/parade-layout.json','runtime-v4/parade-map-metadata.json'].map(read))
    document.body.innerHTML='';document.body.style.cssText='margin:8px;background:#e8e3d9;color:#212126;font:14px system-ui;display:flex;gap:16px;align-items:start'
    const decode=async p=>{const i=new Image();i.src=base+p;await i.decode();return i}
    const points=[...proposal.geometry.spawns,...proposal.geometry.interactions],results=[]
    for(const [name,folder,layout] of [['Frozen source v3','painted/',source],['Compact integer v4','runtime-v4/',compact]]){
     const [ground,atlas]=await Promise.all([decode(folder+layout.ground),decode(folder+layout.atlas)])
     const panel=document.createElement('div');panel.style.width=width+'px';const label=document.createElement('p');label.textContent=name+' · '+view.toUpperCase();panel.append(label)
     const c=document.createElement('canvas'),camera=view==='world'?layout.world_camera:layout.overview_camera
     c.width=width;c.height=Math.round(width*camera.height/camera.width);c.style.display='block';panel.append(c);document.body.append(panel)
     const ctx=c.getContext('2d');ctx.imageSmoothingEnabled=false;ctx.scale(c.width/camera.width,c.height/camera.height);ctx.translate(-camera.x,-camera.y);ctx.drawImage(ground,0,0)
     if(view==='map'){
      ctx.beginPath();meta.plaza_parcel.forEach(([x,y],i)=>i?ctx.lineTo(x,y):ctx.moveTo(x,y));ctx.closePath();ctx.fillStyle=meta.zone_rendering.fill;ctx.fill();ctx.strokeStyle=meta.zone_rendering.stroke;ctx.lineWidth=meta.zone_rendering.stroke_width_px;ctx.stroke()
     }
     const reps=view==='map'?layout.overview_representations:[],replaced=new Set(reps.map(o=>o.replaces_prop_id).filter(Boolean)),objects=layout.placements.filter(o=>!replaced.has(o.id)).concat(reps).sort((a,b)=>a.depth_y-b.depth_y)
     for(const o of objects){const f=layout.frames[o.frame];ctx.drawImage(atlas,f.x,f.y,f.width,f.height,o.x,o.y,o.width,o.height)}
     const buffer=document.createElement('canvas');buffer.width=atlas.width;buffer.height=atlas.height;const a=buffer.getContext('2d',{willReadFrequently:true});a.drawImage(atlas,0,0)
     const feet=points.map(p=>{
      const foot={x:(p.cell_x+.5)*16,y:(p.cell_y+.5)*16},blockers=[]
      for(const o of layout.placements){if(foot.y>=o.depth_y||foot.x<o.x||foot.x>=o.x+o.width||foot.y<o.y||foot.y>=o.y+o.height)continue
       const f=layout.frames[o.frame],x=Math.floor(f.x+(foot.x-o.x)*f.width/o.width),y=Math.floor(f.y+(foot.y-o.y)*f.height/o.height)
       if(a.getImageData(x,y,1,1).data[3]>128)blockers.push(o.id)
      }
      return {id:p.spawn_id??p.interaction_id,blockers}
     })
     results.push({name,ground:[ground.width,ground.height],atlas:[atlas.width,atlas.height],objects:objects.length,feet})
    }
    return {width,view,overflow:document.documentElement.scrollWidth>innerWidth,results}
   },{base,width,view})
   assert.equal(result.overflow,false)
   assert.deepEqual(result.results[1].atlas,[1024,1024]);assert.deepEqual(result.results[1].ground,[768,1296])
   assert(result.results.every(r=>r.feet.every(p=>p.blockers.length===0)),JSON.stringify(result))
   await page.screenshot({path:path.join(out,`comparison-${view}-${width}.png`),fullPage:true});cases.push(result)
  }
  await page.close()
 }
 assert.deepEqual(errors,[])
 fs.writeFileSync(path.join(out,'browser-verification.json'),JSON.stringify({status:'PASS_RUNTIME_DERIVATIVE_ASSET_REVIEW_ONLY',browser:browser.version(),cases,errors,limitations:['Source/compact Canvas2D comparison; not Phaser or app/server admission.','No moving avatars or new fidelity panel; frozen source scores remain unchanged.','Both MAP panels use the approved native solid semantic-zone fill for a controlled packing comparison.']},null,2)+'\n')
 console.log(JSON.stringify({cases:cases.length,clearFeet:cases.reduce((n,c)=>n+c.results.reduce((s,r)=>s+r.feet.length,0),0),errors}))
}finally{await browser.close()}
