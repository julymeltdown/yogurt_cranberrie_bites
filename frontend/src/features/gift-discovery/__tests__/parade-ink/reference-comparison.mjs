// TEST_ONLY pixel-preserving reference crops; never mounted in product UI.
import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
export const referenceDirectory='/private/tmp/codex-remote-attachments/01a19e5c-e9c6-7973-a53b-a6d19141620f/9FC49E03-6C0E-4776-A954-2CA5C12CB495'
export async function compareReferences(browser,output,actual){
 const references=[['1-Photo-1.jpg','c2af15948299caf1c2c0905ffd89a84963975d8078298e6050f5cdd5ca344dbc'],['2-Photo-2.jpg','9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27']]
 const originals=[]
 for(const [name,sha256] of references){const bytes=await readFile(resolve(referenceDirectory,name));assert.equal(createHash('sha256').update(bytes).digest('hex'),sha256);originals.push('data:image/jpeg;base64,'+bytes.toString('base64'))}
 const context=await browser.newContext({viewport:{width:1200,height:900},deviceScaleFactor:1}),page=await context.newPage()
 const records=[]
 try{
  for(const [name,key,crop,overlay] of [['row-side-by-side','row',[454,183,377,115],false],['row-normalized-overlay','row',[454,183,377,115],true],['reservation-side-by-side-overlay','reserve',[877,299,378,115],true]]){
   const source=actual[key];assert.ok(source,`Missing actual ${key} capture`)
   const width=Math.round(source.width),referenceHeight=Math.round(crop[3]*width/crop[2]),height=Math.max(referenceHeight,Math.ceil(source.height))
   const panels=name==='reservation-side-by-side-overlay'?3:overlay?1:2
   const args={reference:originals[0],palette:originals[1],actual:source.data,crop,width,height,panels,overlay,paletteWidth:395,paletteCrop:[24,60,395,52]}
   await page.setViewportSize({width:panels*(width+12)+12,height:height+112})
   await page.setContent('<html><body style="margin:0"><canvas></canvas></body></html>')
   await page.evaluate(async a=>{
    const load=src=>new Promise((done,fail)=>{const i=new Image();i.onload=()=>done(i);i.onerror=fail;i.src=src})
    const [reference,actual,palette]=await Promise.all([load(a.reference),load(a.actual),load(a.palette)])
    const canvas=document.querySelector('canvas');canvas.width=innerWidth;canvas.height=innerHeight
    const c=canvas.getContext('2d');c.fillStyle='#ffffff';c.fillRect(0,0,canvas.width,canvas.height);c.font='14px sans-serif';c.fillStyle='#171717'
    c.fillText('Photo 2 chrome/palette authority; original crop, width-normalized',12,18)
    c.drawImage(palette,...a.paletteCrop,12,24,a.width,52*a.width/a.paletteWidth)
    const drawReference=x=>c.drawImage(reference,...a.crop,x,104,a.width,a.crop[3]*a.width/a.crop[2])
    const drawActual=x=>c.drawImage(actual,x,104,a.width,actual.height*a.width/actual.width)
    if(a.panels===1){c.fillText('Photo 1 / actual: 50% overlay, top-left aligned',12,96);drawReference(12);c.globalAlpha=.5;drawActual(12);c.globalAlpha=1}
    else {c.fillText('Photo 1 anatomy (unrepainted)',12,96);drawReference(12);c.fillText('Actual default-App component',a.width+24,96);drawActual(a.width+24);if(a.panels===3){c.fillText('50% overlay',2*a.width+36,96);drawReference(2*a.width+36);c.globalAlpha=.5;drawActual(2*a.width+36);c.globalAlpha=1}}
   },args)
   await page.screenshot({path:resolve(output,`comparison-${name}.png`)})
   records.push({name,reference:'1-Photo-1.jpg',crop,transform:{scale:width/crop[2],translation:[0,0],repaint:false,masks:[],overlayAlpha:overlay?.5:null},actual:{width:source.width,height:source.height},normalizedReference:{width,height:referenceHeight}})
  }
 }finally{await context.close()}
 await writeFile(resolve(output,'reference-comparison.json'),JSON.stringify({references:references.map(([name,sha256])=>({name,sha256,width:1280,height:853})),classification:'Visual comparison only; variable text, real stock and authorized media differ. No broad masks or acceptance score.',records},null,2)+'\n')
}
