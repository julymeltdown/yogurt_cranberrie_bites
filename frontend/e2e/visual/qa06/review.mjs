// Verify saved evidence and render proportional contact sheets, not product approval.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {readFile,writeFile} from 'node:fs/promises'
import {resolve} from 'node:path'
import {chromium} from '@playwright/test'
import {CASES} from '../../reference-integrated/cases.mjs'
const out=process.argv[2]
assert(out?.includes('/docs/mcr/evidence/visual/qa06/'),'Use an owned QA06 evidence directory')
const report=JSON.parse(await readFile(resolve(out,'report.json'))),modal=JSON.parse(await readFile(resolve(out,'../modal-v2/report.json'))),checks=[],screenshots=[]
assert.equal(modal.status,'PASS');assert.equal(modal.cases.length,9)
assert.equal(report.cases.length,78);assert.equal(report.sourceCases.length,9)
for(const width of [360,390,430])for(const item of CASES){const rows=report.cases.filter(c=>c.width===width&&c.id===item.id);assert.equal(rows.length,item.public?1:3)}
for(const c of report.cases){assert.equal(c.status,'CAPTURED');assert.deepEqual(c.errors,[]);assert.equal(c.metrics.overflow,false);assert.deepEqual(c.metrics.smallControls,[])
 assert(c.reachability.every(x=>x.reachable));assert(c.metrics.images.every(x=>x.loaded))
 if(c.reducedHeight){assert.equal(c.reducedHeight.overflow,false);assert.deepEqual(c.reducedHeight.smallControls,[]);assert(c.reducedReachability.every(x=>x.reachable));screenshots.push([c.reducedScreenshot,c.width,420])}
 screenshots.push([c.screenshot,c.width,844])
}
for(const c of report.sourceCases){assert.equal(c.status,'CAPTURED');assert.deepEqual(c.errors,[]);assert.equal(c.world.sprites,6);assert.equal(c.world.games,1);assert.equal(c.state.map.revision,7004)
 assert.equal(c.state.originalMapProvenance.review_state,'UNREVIEWED');assert.equal(c.productionDenied.sourceOnly,false);assert.equal(c.productionDenied.canvases,0);assert.equal(c.productionDenied.sockets,0);assert(!c.productionDeniedError)
 for(const k of ['worldMetrics','mapMetrics','reducedMetrics']){assert.equal(c[k].overflow,false);assert(c[k].images.every(x=>x.loaded))}
 // Native dialog blocks background controls; active modal scope is verified by modal-v2.
 assert(modal.cases.some(m=>m.width===c.width&&m.era===c.era&&m.status==='PASS'));screenshots.push([c.worldScreenshot,c.width,844],[c.mapScreenshot,c.width,844],[c.reducedScreenshot,c.width,420])
}
for(const [file,width,height] of screenshots){const b=await readFile(resolve(out,file));assert.equal(b.readUInt32BE(16),width);assert.equal(b.readUInt32BE(20),height)}
checks.push({name:'78 screen/width/era records and9 source cases; every capture PNG has exact viewport dimensions',status:'PASS',screenshots:screenshots.length},
 {name:'No recorded page errors/overflow/broken DOM images. Baseline enabled targets/reachability and reduced-height states pass. Nine supplementary native-modal cases prove44px active controls/focus/scroll/return; intentionally blocked background excluded',status:'PASS'},
 {name:'Nine source cases render six sprites/one game, preserve UNREVIEWED map7004, and separately deny default production with no canvas/socket',status:'PASS'},
 {name:'Physical iOS Safari / Android Chrome, software keyboard and hardware safe areas',status:'NOT_RUN'},
 {name:'Qualified Wall visual acceptance',status:'NOT_RUN'})
const style='<style>body{margin:0;padding:16px;font:14px system-ui;background:#eeeae0;color:#222}h1{font-size:22px}.grid{display:grid;grid-template-columns:repeat(5,195px);gap:12px}figure{margin:0}figcaption{height:24px}figure img{width:195px;height:auto;display:block}.source{grid-template-columns:repeat(6,156px)}.source img{width:156px}.target{display:block;width:1023px;height:auto}p{max-width:1023px}</style>'
const html='<html lang="en"><meta charset="utf-8">'+style+'<h1>QA06 · supplied reference and current ten screens</h1><p>Desktop Chrome only. Current captures390×844; reference crops differ in aspect. Synthetic producers; Wall unavailable is not accepted; physical device checks pending. No image is stretched.</p><img class="target" alt="Unchanged ten-screen supplied reference" src="../reference-target.png"><h2>Current actual routes · REVENGE · 390</h2><div class="grid">'+CASES.map(i=>{const c=report.cases.find(c=>c.id===i.id&&c.width===390&&c.era===(i.public?'N/A':'REVENGE'));return `<figure><figcaption>${i.id}</figcaption><img src="${c.screenshot}" alt="${i.id}"></figure>`}).join('')+'</div></html>'
await writeFile(resolve(out,'contact-sheet.html'),html)
const source='<html lang="en"><meta charset="utf-8">'+style+'<h1>QA06 · spatial-v4 source fixture</h1><p>Original UNREVIEWED source, synthetic map7004, explicit resolver. No production or backend revision5 admission. Each pair is world/map; native proportions retained.</p><div class="grid source">'+report.sourceCases.flatMap(c=>['world','map'].map(mode=>`<figure><figcaption>${c.width} ${c.era} ${mode}</figcaption><img src="${c[mode+'Screenshot']}" alt="${c.width} ${c.era} ${mode}"></figure>`)).join('')+'</div></html>'
await writeFile(resolve(out,'source-contact-sheet.html'),source)
const browser=await chromium.launch({channel:'chrome',headless:true})
try{const page=await browser.newPage({viewport:{width:1060,height:1000}})
 for(const name of ['contact-sheet','source-contact-sheet']){await page.goto('file://'+resolve(out,name+'.html'));await page.waitForLoadState('networkidle');assert(await page.locator('img').evaluateAll(nodes=>nodes.every(i=>i.complete&&i.naturalWidth>0)));await page.screenshot({path:resolve(out,name+'.png'),fullPage:true})}
 await page.goto('file://'+resolve(out,'comparison.html'));await page.waitForLoadState('networkidle');assert.equal(await page.locator('article').count(),11);assert(await page.locator('img').evaluateAll(nodes=>nodes.every(i=>i.complete&&i.naturalWidth>0)));checks.push({name:'Comparison11sections and contact sheets render with all local image links loaded',status:'PASS'})
}finally{await browser.close()}
const result={task:'QA06',status:'DESKTOP_EVIDENCE_VERIFIED_ORIGINAL_TASK_IN_PROGRESS',modalReportSha256:createHash('sha256').update(await readFile(resolve(out,'../modal-v2/report.json'))).digest('hex'),reportSha256:createHash('sha256').update(await readFile(resolve(out,'report.json'))).digest('hex'),checks,sourceDefects:[],requiredOpenCriteria:['Physical iOS Safari','Physical Android Chrome','Actual software keyboard','Actual hardware safe areas','Qualified current-world Wall visuals','Genuine product art/release admission'],physicalDeviceEmulationIsAcceptance:false}
await writeFile(resolve(out,'review.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2))
