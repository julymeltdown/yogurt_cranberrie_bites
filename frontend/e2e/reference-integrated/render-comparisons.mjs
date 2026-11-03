import assert from 'node:assert/strict'
import { readFile, writeFile, statfs } from 'node:fs/promises'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { chromium, expect } from '@playwright/test'
const output=process.env.MCR_REFERENCE_INTEGRATED_OUTPUT
assert(output?.startsWith('/'),'Set the absolute captured evidence directory')
const s=await statfs(output);assert(s.bavail*s.bsize>=8*1024**3,'Disk floor below 8 GiB')
const report=JSON.parse(await readFile(resolve(output,'report.json')))
const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true})
const results=[]
try{
 const page=await browser.newPage({viewport:{width:1000,height:1100}})
 await page.goto(pathToFileURL(resolve(output,'comparison.html')).href)
 await expect(page.locator('article')).toHaveCount(report.cases.length)
 for(const c of report.cases.filter(c=>c.width===390&&c.measurements)){
  const article=page.locator(`#${c.id}-${c.width}`),pair=article.locator('.pair')
  const images=await article.locator('img').evaluateAll(async images=>{await Promise.all(images.map(img=>img.decode()));return images.map(img=>({width:img.naturalWidth,height:img.naturalHeight}))})
  assert.equal(images.length,3);assert.deepEqual(images[0],{width:1536,height:1024});assert.deepEqual(images[1],c.viewport)
  const artifact=`comparison-${c.id}-390.png`;await pair.screenshot({path:resolve(output,artifact)})
  results.push({id:c.id,artifact,actualViewport:c.viewport,targetOriginal:images[0],targetCrop:report.reference.crops.find(x=>x.id===c.id).rect,status:'PASS'})
 }
 await writeFile(resolve(output,'comparison-render.json'),JSON.stringify({kind:'STATIC_ARTIFACT_RENDER_ONLY',candidate:report.candidate,browser:browser.version(),results},null,2)+'\n')
 console.log(JSON.stringify({comparisons:results.length,status:'PASS'}))
}finally{await browser.close()}
