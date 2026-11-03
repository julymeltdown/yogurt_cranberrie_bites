const fs=require('node:fs/promises'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto')
const root=path.resolve(__dirname,'../../../../..'),{chromium}=require(process.env.MCR_PLAYWRIGHT_MODULE||path.join(root,'frontend/node_modules/@playwright/test'))
const origin=process.env.MCR_ART_ORIGIN||'http://127.0.0.1:41865'
const hash=b=>'sha256:'+crypto.createHash('sha256').update(b).digest('hex')
;(async()=>{const browser=await chromium.launch({channel:'chrome',headless:true});const results=[],errors=[];let geometry
try{for(const width of [360,390,430])for(const era of ['REVENGE','PARADE','DANGER']){
 const page=await browser.newPage({viewport:{width,height:844},deviceScaleFactor:1});page.on('pageerror',e=>errors.push(e.message))
 await page.goto(`${origin}/frontend/public/world/illustrated/review.html`);await page.waitForFunction(()=>window.illustratedReview?.ready)
 await page.getByRole('button',{name:era,exact:true}).click()
 for(const landmark of ['stage','studio','park','gifts','wall']){
  await page.locator('#landmark').selectOption(landmark);await page.waitForFunction(l=>window.illustratedReview.landmark===l,landmark)
  const state=await page.evaluate(()=>{const s=window.illustratedReview,c=document.querySelector('#world'),r=c.getBoundingClientRect(),a=document.querySelector('.access'),ar=a.getBoundingClientRect();return {...s,overflow:document.documentElement.scrollWidth>innerWidth,smoothing:c.getContext('2d').imageSmoothingEnabled,canvasCssWidth:r.width,button:{width:ar.width,height:ar.height,left:ar.left,top:ar.top,right:ar.right,bottom:ar.bottom},canvasBounds:{left:r.left,top:r.top,right:r.right,bottom:r.bottom}}})
  geometry??=state.geometry;assert.equal(state.geometry,geometry);assert.equal(state.era,era);assert.deepEqual(state.scene,[1152,896]);assert.equal(state.overflow,false);assert.equal(state.smoothing,false);assert.equal(state.canvas[0],state.canvasCssWidth)
  const a=state.button,c=state.canvasBounds;assert(a.width>=44&&a.height>=44&&a.left>=c.left&&a.right<=c.right&&a.top>=c.top&&a.bottom<=c.bottom,'unclipped44px interaction target')
  await page.getByRole('button',{name:`Inspect virtual ${landmark} interaction`}).click();assert.match(await page.getByRole('status').innerText(),new RegExp(state.interaction))
  // Keyboard focus must reach the same target without moving the camera.
  await page.getByRole('button',{name:`Inspect virtual ${landmark} interaction`}).focus();await page.keyboard.press('Enter');assert.equal(await page.evaluate(()=>window.illustratedReview.camera.x),state.camera.x)
  const screenshot=`${era.toLowerCase()}-${landmark}-${width}.png`;await page.screenshot({path:path.join(__dirname,screenshot),fullPage:true})
  results.push({era,landmark,width,interaction:state.interaction,camera:state.camera,target:state.target,canvas:state.canvas,button:state.button,geometry_hash:'sha256:463a46ba69bcb48fc691cc33c76e53a43328473edf8b8b61c235d96043dc9e27',click_and_keyboard:'PASS',screenshot,sha256:hash(await fs.readFile(path.join(__dirname,screenshot)))})
 }await page.close()
}assert.equal(errors.length,0);await fs.writeFile(path.join(__dirname,'native-validation.json'),JSON.stringify({result:'PASS',scope:'45 native source-art CSS-width/era/landmark cases; exact reference retained alongside in every capture',browser:browser.version(),playwright:require(path.join(process.env.MCR_PLAYWRIGHT_MODULE||path.join(root,'frontend/node_modules/@playwright/test'),'package.json')).version,node:process.version,results,page_errors:errors,limits:['No avatar art','No actual iOS/Android hardware','No product Phaser or real backend integration','No final release acceptance']},null,2)+'\n');console.log('PASS:45 native 360/390/430×era×landmark cases,45clicks+keyboard,unclipped44px targets,1:1pixel scene,nooverflow')
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1})
