import {readFile,writeFile} from 'node:fs/promises'
import {resolve} from 'node:path'
import assert from 'node:assert/strict'
import {chromium} from '@playwright/test'
import {CASES} from '../../reference-integrated/cases.mjs'
const out=process.argv[2];assert(out?.includes('/docs/mcr/evidence/visual/qa06/'))
const r=JSON.parse(await readFile(resolve(out,'report.json')))
const crop=i=>{const s=390/i.rect.width;return `<div class="crop" style="height:${Math.round(i.rect.height*s)}px"><img src="../reference-target.png" alt="Original ${i.id} crop" style="width:${1536*s}px;left:${-i.rect.x*s}px;top:${-i.rect.y*s}px"></div>`}
const cards=CASES.map(i=>{const c=r.cases.find(c=>c.id===i.id);return `<article id="${i.id}"><h2>${i.id} · same390px width and reference aspect</h2><div class="pair"><figure><figcaption>Supplied unchanged reference</figcaption>${crop(i)}</figure><figure><figcaption>Actual current UI · synthetic producers</figcaption><img src="${c.screenshot}" alt="Actual ${i.id}"></figure></div></article>`}).join('')
await writeFile(resolve(out,'comparison.html'),`<html lang="en"><meta charset="utf-8"><style>body{margin:0;padding:14px;background:#eeeae0;color:#222;font:14px system-ui}h2{font-size:18px;margin:0 0 8px}.pair{display:flex;gap:18px}figure{margin:0;width:390px;flex:none}figcaption{height:24px}.crop{position:relative;overflow:hidden;width:390px}.crop img{position:absolute;max-width:none}figure>img{width:390px;height:auto;display:block}article{width:798px;padding:8px 0 20px}</style>${cards}</html>`)
const browser=await chromium.launch({channel:'chrome',headless:true})
try{const page=await browser.newPage({viewport:{width:830,height:1000}});await page.goto('file://'+resolve(out,'comparison.html'));await page.waitForLoadState('networkidle');assert(await page.locator('img').evaluateAll(nodes=>nodes.every(i=>i.complete&&i.naturalWidth>0)))
 for(const item of CASES)await page.locator('#'+item.id).screenshot({path:resolve(out,`compare-${item.id}-390.png`)})
}finally{await browser.close()}
console.log('Ten equal-aspect comparison panels rendered without image stretching')
