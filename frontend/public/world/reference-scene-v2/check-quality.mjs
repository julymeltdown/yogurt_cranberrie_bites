import assert from 'node:assert/strict'
import {createRequire} from 'node:module'
import fs from 'node:fs'
import path from 'node:path'
import {fileURLToPath} from 'node:url'
const revision=process.env.MCR_ART_REVISION||'6';assert(['6','7'].includes(revision))
const here=path.dirname(fileURLToPath(import.meta.url)),out=path.join(here,`quality-v${revision}`),require=createRequire(process.env.MCR_PLAYWRIGHT_PACKAGE||path.resolve(here,'../../../package.json'))
const {chromium}=require('@playwright/test'),browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true}),cases=[],errors=[]
try{
 for(const width of [360,390,430]){
  const page=await browser.newPage({viewport:{width,height:844},deviceScaleFactor:1});page.on('pageerror',e=>errors.push(e.message))
  await page.goto(`http://127.0.0.1:8896/frontend/public/world/reference-scene-v2/quality-v${revision}/review.html`);await page.waitForFunction(()=>window.eraSceneReview?.ready)
  for(let cycle=0;cycle<2;cycle++)for(const era of ['parade','revenge','danger']){
   await page.locator('#era').selectOption(era);await page.waitForFunction(era=>window.eraSceneReview?.ready&&window.eraSceneReview.era===era,era)
   for(const view of ['world','map']){
    await page.locator(`[data-view=${view}]`).click();await page.waitForFunction(view=>window.eraSceneReview?.ready&&window.eraSceneReview.view===view,view)
    const state=await page.evaluate(()=>({state:window.eraSceneReview,feet:window.eraSceneOcclusion(),overflow:document.documentElement.scrollWidth>innerWidth,canvases:document.querySelectorAll('canvas').length}))
    assert.equal(state.overflow,false);assert.equal(state.canvases,1);assert(state.feet.every(p=>p.blockers.length===0),JSON.stringify(state))
    assert.deepEqual(state.state.ground,[768,1296]);assert.deepEqual(state.state.atlas,[1024,1024]);assert.equal(state.state.physicalObjects,22);assert.equal(state.state.mapRepresentations,6);assert(state.state.cacheEntries<=3)
    if(cycle===0)await page.screenshot({path:path.join(out,`${era}-${view}-${width}.png`),fullPage:true})
    cases.push({width,cycle,era,view,...state})
   }
  }
  await page.close()
 }
 assert.deepEqual(errors,[])
 fs.writeFileSync(path.join(out,'browser-verification.json'),JSON.stringify({status:`PASS_V${revision}_REFERENCE_ASSET_COMPARISON_ONLY`,browser:browser.version(),cases,errors,limitations:['Canvas2D static asset/skin cycling review, not Phaser, server release or multiplayer acceptance.','No characters/moving-avatar occlusion; six named feet only.','No new critic score or target-fidelity acceptance.']},null,2)+'\n')
 console.log(JSON.stringify({status:'PASS',cases:cases.length,clearFeet:cases.reduce((n,c)=>n+c.feet.length,0),errors}))
}finally{await browser.close()}
