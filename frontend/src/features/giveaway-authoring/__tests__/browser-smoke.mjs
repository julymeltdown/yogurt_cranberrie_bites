import assert from 'node:assert/strict'
import {mkdir,readFile,writeFile} from 'node:fs/promises'
import {createHash} from 'node:crypto'
import {resolve} from 'node:path'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../../..'),out=resolve(import.meta.dirname,'evidence/browser');await mkdir(out,{recursive:true})
const report={result:'FAIL',scope:'FE25 isolated C04 fixture/native UI via canonical auth/privacy. No production upload/backend/device or IN02 acceptance.',widths:[]}
report.sourceHashes=Object.fromEntries(await Promise.all(['../party-authoring/model/time.ts','model/controller.ts','model/contract.ts','ui/GiveawayAuthoring.tsx','ui/giveaway-authoring.css','__tests__/harness.ts','__tests__/preview.tsx'].map(async p=>[p,createHash('sha256').update(await readFile(resolve(import.meta.dirname,'..',p))).digest('hex')])))
const server=await createServer({root:resolve(root,'frontend'),logLevel:'error',server:{host:'127.0.0.1',port:5199,strictPort:true}});let browser
try{await server.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
for(const width of [360,390,430,1280]){const page=await browser.newPage({viewport:{width,height:900}}),errors=[];page.on('pageerror',e=>errors.push(e.message))
 await page.goto('http://127.0.0.1:5199/src/features/giveaway-authoring/__tests__/preview.html');const title=page.getByRole('textbox',{name:'Item title',exact:true});await expect(title).toHaveValue('Fan badges');await expect(page.getByText(/Image upload is unavailable/)).toBeVisible();await expect(page.getByRole('table')).toBeVisible()
 await title.fill('Preserve the postcards');await page.getByRole('button',{name:'Save giveaway changes'}).click();await expect(page.getByRole('status')).toContainText('Stock or settings changed');await expect(title).toHaveValue('Preserve the postcards')
 await page.getByRole('button',{name:'Read latest stock and settings'}).click();await page.getByRole('button',{name:'Keep my form with this revision'}).click();await expect(title).toHaveValue('Preserve the postcards');await page.screenshot({path:resolve(out,`${width}-stock.png`),fullPage:true})
 await page.evaluate(()=>window.__FE25__.mode('lost'));await page.getByRole('button',{name:'Save giveaway changes'}).click();await expect(page.getByRole('button',{name:'Check original command'})).toBeVisible();await expect(page.getByRole('button',{name:'Save giveaway changes'})).toBeDisabled();await page.screenshot({path:resolve(out,`${width}-uncertain.png`),fullPage:true})
 assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));await page.evaluate(()=>window.__FE25__.logout());await expect(page.getByRole('heading',{name:'Giveaway owner tools unavailable'})).toBeVisible();await expect(page.getByRole('table')).toHaveCount(0);assert.deepEqual(errors,[]);report.widths.push(width);await page.close()
}report.result='PASS';report.checks=['loaded edit values','missing upload honest','stock conflict keeps form','explicit rebase keeps edits','unknown write locks resubmit','authorized inventory shown','logout scrubs draft and inventory','no horizontal overflow or browser errors']
}finally{await browser?.close();await server.close();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n')}
console.log(JSON.stringify(report,null,2))
