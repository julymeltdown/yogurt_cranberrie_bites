import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createServer as createNetServer } from 'node:net'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
const root = resolve(import.meta.dirname, '../../../../../..'), output = process.env.MCR_MEET_REFERENCE_OUTPUT
assert(output?.startsWith('/'), 'Set MCR_MEET_REFERENCE_OUTPUT to a new private absolute directory')
await mkdir(output, { mode: 0o700 })
const reservation = createNetServer(); await new Promise(done => reservation.listen(0, '127.0.0.1', done))
const reserved = reservation.address(); assert(reserved && typeof reserved !== 'string'); const port = reserved.port
await new Promise(done => reservation.close(done))
const frontend = resolve(root, 'frontend'), server = await createServer({ root: frontend, logLevel: 'error', server: { host: '127.0.0.1', port, strictPort: true } })
const hash = b => createHash('sha256').update(b).digest('hex')
const report = { kind: 'mcr-fe-ref-meet-v1', status: 'FAIL', scope: 'Frozen fixtures, actual production PartyDiscovery and native Chrome; no live services/approved portrait/physical arrival acceptance', widths: [], sources: {}, browser: '' }
for (const path of ['frontend/src/features/party-discovery/ui/PartyDiscovery.tsx', 'frontend/src/features/party-discovery/ui/party-discovery.css', 'contracts/domain/v1/party.schema.json']) report.sources[path] = hash(await readFile(resolve(root, path)))
let browser
try {
  await server.listen(); const address = server.httpServer.address(); assert(address && typeof address !== 'string')
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const width of [360,390,430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, reducedMotion: 'reduce' }), page = await context.newPage(), errors = []
    page.on('pageerror', e => errors.push(e.message))
    await page.goto(`http://127.0.0.1:${address.port}/src/features/party-discovery/__tests__/reference/preview.html`)
    const rows = page.locator('.meet-party'); await expect(rows).toHaveCount(3)
    await expect(page.getByRole('button', { name: 'Parties', exact: true })).toHaveAttribute('aria-pressed','true')
    assert.deepEqual(await page.locator('.party-tabs').evaluate(el => [...el.children].map(c => c.textContent)), ['Parties','People','MY PLAN'])
    assert.equal(await page.locator('img,canvas').count(),0, 'unreviewed or fabricated portrait rendered')
    const boxes = await rows.evaluateAll(els => els.map(el => { const r=el.getBoundingClientRect(); return { x:r.x,y:r.y,width:r.width,height:r.height } }))
    assert(boxes.every(b => b.height >= 100 && b.height <= 140), 'rows lost compact reference density')
    assert(boxes[2].y+boxes[2].height < 510, 'three party rows must remain immediately visible')
    for (const el of await page.locator('.party-browse button:visible,.party-browse a:visible,.party-browse summary:visible').all()) { const r = await el.boundingBox(); assert(r && r.width >= 44 && r.height >= 44, 'touch target under 44px') }
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal page overflow')
    assert(await page.locator('.party-browse').evaluate(el => el.scrollWidth <= el.clientWidth), 'feature overflow')
    await page.screenshot({ path: resolve(output,`${width}-parties.png`), fullPage:true })
    const join = page.getByRole('link',{name:'View details for Area A Queue Group',exact:true}); await join.focus()
    assert(await join.evaluate(el => getComputedStyle(el).outlineStyle !== 'none'), 'missing keyboard focus outline')
    await join.press('Enter'); await expect(page.getByRole('heading',{name:'Area A Queue Group',exact:true})).toBeVisible()
    assert.equal((await page.evaluate(() => window.__MEET_REFERENCE__.requests())).filter(r => r.method !== 'GET').length,0,'JOIN sent mutation')
    await page.goBack(); await expect(join).toBeFocused()
    await page.getByRole('link',{name:'People',exact:true}).click(); await expect(page.getByRole('heading',{name:'People route fixture'})).toBeVisible(); await page.goBack()
    await page.getByRole('link',{name:'Create party',exact:true}).click(); await expect(page.getByRole('heading',{name:'Create party route fixture'})).toBeVisible(); await page.goBack()
    await page.getByRole('button',{name:'MY PLAN',exact:true}).click(); await expect(page.getByRole('heading',{name:'Your applications'})).toBeVisible()
    await page.getByRole('button',{name:'Parties',exact:true}).click(); await expect(rows).toHaveCount(3)
    await page.evaluate(() => window.__MEET_REFERENCE__.mode('full')); await expect(page.locator('.meet-party__join').first()).toHaveText('Full')
    await page.evaluate(() => window.__MEET_REFERENCE__.mode('empty')); await expect(page.getByText('No matching parties',{exact:true})).toBeVisible()
    await page.evaluate(() => window.__MEET_REFERENCE__.mode('ready')); await expect(rows).toHaveCount(3)
    await page.evaluate(() => window.__MEET_REFERENCE__.revoke()); await expect(rows).toHaveCount(0); await expect(page.getByRole('button',{name:'Create party',exact:true})).toBeDisabled()
    await page.screenshot({path:resolve(output,`${width}-privacy.png`),fullPage:true})
    assert.deepEqual(errors,[]); report.widths.push({width,rows:boxes,checks:['navigation','no-fake-art','density','44px-targets','no-overflow','visible-focus','join-read-only','back-focus','people-route','create-route','my-plan','full','empty','privacy-withdrawal']})
    await page.evaluate(() => window.__MEET_REFERENCE__.dispose()); await context.close()
  }
  report.status = 'PASS'
} finally { await browser?.close(); await server.close(); await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n',{mode:0o600}) }
console.log(JSON.stringify({status:report.status,widths:report.widths.map(x=>x.width),output}))
