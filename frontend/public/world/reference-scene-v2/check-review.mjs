import assert from 'node:assert/strict'
import {createRequire} from 'node:module'
import fs from 'node:fs'
import path from 'node:path'
import {fileURLToPath} from 'node:url'
const here=path.dirname(fileURLToPath(import.meta.url))
const require=createRequire(process.env.MCR_PLAYWRIGHT_PACKAGE||path.resolve(here,'../../../package.json'))
const {chromium}=require('@playwright/test')
const browser=await chromium.launch({executablePath:process.env.MCR_BROWSER_EXECUTABLE||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
const results=[],failures=[];fs.mkdirSync(path.join(here,'evidence'),{recursive:true})
try{
 for(const width of [360,390,430]){
  const page=await browser.newPage({viewport:{width,height:844},deviceScaleFactor:1})
  page.on('pageerror',error=>failures.push(error.message))
  await page.goto(process.env.MCR_SCENE_REVIEW_URL||'http://127.0.0.1:8896/frontend/public/world/reference-scene-v2/review.html')
  await page.waitForFunction(()=>window.referenceSceneReview?.ready)
  for(const finish of ['painted','native']){
   await page.locator('#finish').selectOption(finish)
   await page.waitForFunction(finish=>window.referenceSceneReview?.ready&&window.referenceSceneReview.finish===finish,finish)
  for(const theme of finish==='painted'?['parade']:['parade','revenge','danger']){
   await page.locator('#theme').selectOption(theme)
   await page.waitForFunction(theme=>window.referenceSceneReview?.ready&&window.referenceSceneReview.theme===theme,theme)
   for(const view of ['world','map']){
    await page.locator(`[data-view=${view}]`).click()
    await page.waitForFunction(view=>window.referenceSceneReview?.ready&&window.referenceSceneReview.view===view,view)
    const state=await page.evaluate(()=>({state:window.referenceSceneReview,occlusion:window.referenceSceneOcclusion(),overflow:document.documentElement.scrollWidth>innerWidth,canvases:document.querySelectorAll('canvas').length}))
    assert.equal(state.overflow,false);assert.equal(state.canvases,1)
    assert(state.occlusion.every(p=>p.blockers.length===0),JSON.stringify(state.occlusion))
    assert.deepEqual(state.state.ground,[768,1296]);assert.deepEqual(state.state.atlas,finish==='painted'?[1254,1254]:[1024,1024])
    if(finish==='painted')await page.screenshot({path:path.join(here,'evidence',`${view}-${width}.png`),fullPage:true})
    if(width===390){
     await page.locator('#scene').screenshot({path:path.join(here,'evidence',`${finish}-${theme}-${view}-390.png`)})
     if(view==='map')fs.writeFileSync(path.join(here,'evidence',`${finish}-${theme}-scene.png`),Buffer.from((await page.evaluate(()=>window.referenceSceneExport())).split(',')[1],'base64'))
    }
    results.push({width,theme,finish,view,...state})
   }
  }
  }
  await page.locator('#geometry').click();await page.waitForFunction(()=>window.referenceSceneReview?.ready)
  await page.locator('#scene').screenshot({path:path.join(here,'evidence',`collision-${width}.png`)})
  await page.close()
 }
 assert.deepEqual(failures,[])
 fs.writeFileSync(path.join(here,'browser-verification.json'),JSON.stringify({status:'PASS_ASSET_REVIEW_ONLY',browser:browser.version(),cases:results.length,results,errors:failures,limitations:['Canvas2D technical review, not current Phaser/app/server integration.','No characters are rendered; feet alpha checks do not establish full dynamic avatar occlusion.','Visual target acceptance is a separate review.']},null,2)+'\n')
 console.log(JSON.stringify({status:'PASS_ASSET_REVIEW_ONLY',cases:results.length,clearFeet:results.reduce((sum,r)=>sum+r.occlusion.length,0),errors:failures}))
}finally{await browser.close()}
