import assert from 'node:assert/strict'
import {mkdir,readFile,writeFile} from 'node:fs/promises'
import {createHash} from 'node:crypto'
import {resolve} from 'node:path'
import {createServer} from 'vite'
import {chromium,expect} from '@playwright/test'
const root=resolve(import.meta.dirname,'../../../../..'),out=resolve(import.meta.dirname,'evidence/browser');await mkdir(out,{recursive:true})
const report={result:'FAIL',scope:'FE23 isolated native UI against deterministic C04 fetch fixture and canonical Auth/privacy; no real backend/world/device/physical authority acceptance.',widths:[],checks:[]}
report.sourceHashes=Object.fromEntries(await Promise.all(['model/controller.ts','ui/PartyManagement.tsx','ui/management.css','__tests__/harness.ts','__tests__/preview.tsx'].map(async p=>[p,createHash('sha256').update(await readFile(resolve(import.meta.dirname,'..',p))).digest('hex')])))
const server=await createServer({root:resolve(root,'frontend'),logLevel:'error',server:{host:'127.0.0.1',port:5198,strictPort:true}});let browser
try{await server.listen();browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});report.browser=browser.version()
for(const width of [360,390,430,1280]){const page=await browser.newPage({viewport:{width,height:900}}),errors=[];page.on('pageerror',e=>errors.push(e.message))
 await page.goto('http://127.0.0.1:5198/src/features/party-management/__tests__/preview.html');const approve=page.getByRole('button',{name:'Approve Jules · fixture',exact:true});await expect(approve).toBeEnabled()
 await page.getByRole('button',{name:'Cancel party',exact:true}).click();await expect(page.getByRole('dialog')).toBeVisible();await page.getByRole('button',{name:'Keep unchanged'}).click();assert.equal(await page.evaluate(()=>window.__FE23__.requests().filter(r=>r.method!=='GET').length),0)
 await page.getByRole('button',{name:'Read current rally'}).click();await expect(page.getByRole('checkbox')).toBeEnabled();await expect(page.getByRole('button',{name:'HERE',exact:true})).toBeDisabled()
 await page.screenshot({path:resolve(out,`${width}-owner.png`),fullPage:true})
 await approve.click();await expect(approve).toBeDisabled();await expect(page.getByText('OPEN · 5 of 5 members · revision 4')).toBeVisible()
 await page.evaluate(()=>window.__FE23__.moved());await expect(page.getByText('Meeting revision 2. Previous meeting cards are superseded.')).toBeVisible();await expect(page.getByText('Current authorized physical meeting details are unavailable or expired.')).toBeVisible()
 await page.evaluate(()=>window.__FE23__.mode('lost'));await page.getByRole('button',{name:'Cancel party',exact:true}).click();await page.getByRole('button',{name:'Confirm cancellation'}).click();await expect(page.getByRole('button',{name:'Check original command'})).toBeVisible();await expect(page.getByRole('button',{name:'Cancel party',exact:true})).toBeDisabled()
 await page.screenshot({path:resolve(out,`${width}-uncertain.png`),fullPage:true})
 assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'horizontal overflow');await page.evaluate(()=>window.__FE23__.logout());await expect(page.getByText('Before the encore',{exact:true})).toHaveCount(0);await expect(page.getByText('Jules · fixture',{exact:true})).toHaveCount(0)
 assert.deepEqual(errors,[]);report.widths.push(width);await page.close()
}report.result='PASS';report.checks=['confirmation cancel makes no write','explicit rally acknowledgement required','last-slot conflict refreshes full count','moved meeting removes old authority','unknown cancellation locks all mutations','logout clears private rows','no horizontal overflow','no browser errors']
}finally{await browser?.close();await server.close();await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n')}
console.log(JSON.stringify(report,null,2))
