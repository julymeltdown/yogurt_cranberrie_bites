import assert from 'node:assert/strict'
import {createRequire} from 'node:module'
import fs from 'node:fs'
import path from 'node:path'
import {fileURLToPath} from 'node:url'
const here=path.dirname(fileURLToPath(import.meta.url)),require=createRequire(process.env.MCR_PLAYWRIGHT_PACKAGE||path.resolve(here,'../../../../package.json')),presentation=JSON.parse(fs.readFileSync(path.join(here,'presentation.json'),'utf8'))
const {chromium}=require('@playwright/test'),browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true}),cases=[],errors=[]
try{
 for(const width of [360,390,430]){
  const page=await browser.newPage({viewport:{width,height:844},deviceScaleFactor:1});page.on('pageerror',e=>errors.push(e.message))
  await page.goto('http://127.0.0.1:8896/frontend/public/world/reference-scene-v2/map-landscape-v1/review.html');await page.waitForFunction(()=>window.eraSceneReview?.ready)
  for(let cycle=0;cycle<2;cycle++)for(const era of ['parade','revenge','danger']){
   await page.locator('#era').selectOption(era);await page.waitForFunction(era=>window.eraSceneReview?.ready&&window.eraSceneReview.era===era,era)
   for(const mode of ['landscaped','base']){
    await page.locator(`[data-mode=${mode}]`).click();await page.waitForFunction(mode=>window.eraSceneReview?.ready&&window.eraSceneReview.mode===mode,mode)
    const state=await page.evaluate(()=>({state:window.eraSceneReview,feet:window.gardenFootSamples(),labels:window.labelReview(),overflow:document.documentElement.scrollWidth>innerWidth,canvases:document.querySelectorAll('canvas').length}))
    assert.equal(state.state.geometryHash,presentation.geometry_hash);assert.equal(state.state.view,'map');assert.equal(state.state.addedDecorations,mode==='landscaped'?10:0)
    assert.deepEqual(state.state.ground,[768,1296]);assert.deepEqual(state.state.atlas,[1024,1024]);assert.equal(state.state.physicalObjects,34);assert.equal(state.state.mapRepresentations,8);assert.equal(state.state.resources,2);assert(state.state.cacheEntries<=3)
    assert.equal(state.overflow,false);assert.equal(state.canvases,1);assert.equal(state.feet.length,6);assert(state.feet.every(p=>p.addedDecorationBlockers.length===0))
    assert.equal(state.labels.fontLoaded,true);assert.equal(state.labels.labels.length,6)
    for(const label of state.labels.labels)for(const line of label.lines)assert(line.width<=label.availableTextWidth+.1)
    if(cycle===0&&mode==='landscaped')await page.screenshot({path:path.join(here,`${era}-map-${width}.png`),fullPage:true})
    cases.push({width,cycle,era,mode,...state})
   }
  }
  await page.close()
 }
 assert.deepEqual(errors,[])
 fs.writeFileSync(path.join(here,'browser-verification.json'),JSON.stringify({status:'PASS_MAP_PLANTING_SOURCE_COMPOSITION_ONLY',browser:browser.version(),cases,errors,limits:['Actual Canvas2D MAP source composition at360/390/430, not application/Phaser/server/release acceptance.','Named-foot checks concern only the ten added MAP illustrations; no moving-avatar or whole-MAP occlusion claim.','No WORLD change, new runtime texture, physical evidence or fourth visual score.']},null,2)+'\n')
 console.log(JSON.stringify({status:'PASS',cases:cases.length,newDecorationFootSamples:cases.reduce((n,c)=>n+c.feet.length,0),labelSamples:cases.reduce((n,c)=>n+c.labels.labels.length,0),errors}))
}finally{await browser.close()}
