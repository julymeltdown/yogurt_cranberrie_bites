import assert from 'node:assert/strict'
import {createRequire} from 'node:module'
import fs from 'node:fs'
import path from 'node:path'
import {fileURLToPath} from 'node:url'
const here=path.dirname(fileURLToPath(import.meta.url))
const require=createRequire(process.env.MCR_PLAYWRIGHT_PACKAGE||path.resolve(here,'../../../../package.json'))
const {chromium}=require('@playwright/test')
const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
const cases=[],errors=[],pngRequests=new Set(),fontRequests=new Set()
try{
 for(const width of [360,390,430]){
  const page=await browser.newPage({viewport:{width,height:844},deviceScaleFactor:1})
  page.on('pageerror',e=>errors.push(e.message))
  page.on('response',r=>{if(r.url().endsWith('.png'))pngRequests.add(new URL(r.url()).pathname);if(r.url().endsWith('.ttf'))fontRequests.add(new URL(r.url()).pathname)})
  await page.goto('http://127.0.0.1:8896/frontend/public/world/reference-scene-v2/map-labels-v1/review.html')
  await page.waitForFunction(()=>window.eraSceneReview?.ready)
  await page.locator('[data-view=map]').click()
  for(let cycle=0;cycle<2;cycle++)for(const era of ['parade','revenge','danger']){
   await page.locator('#era').selectOption(era)
   await page.waitForFunction(era=>window.eraSceneReview?.ready&&window.eraSceneReview.era===era&&window.eraSceneReview.view==='map',era)
   const actual=await page.evaluate(()=>({scene:window.eraSceneReview,labels:window.labelReview(),feet:window.eraSceneOcclusion(),overflow:document.documentElement.scrollWidth>innerWidth,canvases:document.querySelectorAll('canvas').length}))
   assert.equal(actual.overflow,false);assert.equal(actual.canvases,1);assert.equal(actual.scene.physicalObjects,34)
   assert.equal(actual.scene.geometryHash,'sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f')
   assert.equal(actual.labels.fontLoaded,true);assert.equal(actual.labels.labels.length,6)
   assert.equal(actual.labels.labels.filter(l=>l.icon).length,5)
   assert.equal(actual.labels.labels.filter(l=>l.interaction_id).length,2)
   for(const l of actual.labels.labels){
    assert(l.rect.x>=5&&l.rect.y>=5&&l.rect.x+l.rect.width<=763&&l.rect.y+l.rect.height<=1291)
    for(const line of l.lines)assert(line.width<=l.availableTextWidth+.1,JSON.stringify(l))
   }
   assert(actual.feet.every(p=>p.blockers.length===0))
   if(cycle===0)await page.screenshot({path:path.join(here,`${era}-map-${width}.png`),fullPage:true})
   cases.push({width,cycle,era,...actual})
  }
  await page.close()
 }
 assert.deepEqual(errors,[])
 const sceneryPNGs=[...pngRequests].filter(p=>p.includes('/quality-v'))
 assert.equal(sceneryPNGs.length,4) // one shared ground, three era atlases; source target excluded.
 assert.equal(fontRequests.size,1)
 fs.writeFileSync(path.join(here,'browser-verification.json'),JSON.stringify({status:'PASS_SOURCE_LABEL_COMPOSITION_ONLY',browser:browser.version(),cases,errors,sceneryPNGs,fontRequests:[...fontRequests],limits:['Actual source Canvas2D composition and native exports, not application/Phaser/backend release adoption.','No fourth critic panel or claimed target acceptance.','Named-foot scenery samples do not certify moving-avatar body occlusion; labels are a nonphysical MAP overlay.']},null,2)+'\n')
 console.log(JSON.stringify({status:'PASS',cases:cases.length,labelsChecked:cases.length*6,sceneryPNGs:sceneryPNGs.length,fontRequests:fontRequests.size}))
}finally{await browser.close()}
