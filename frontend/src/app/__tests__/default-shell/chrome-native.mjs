import assert from 'node:assert/strict'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const frontend = resolve(import.meta.dirname, '../../../..'), evidence = resolve(import.meta.dirname, 'evidence')
const server = await createServer({ root: frontend, configLoader: 'runner', cacheDir: '/tmp/mcr-default-shell-cache', server: { host: '127.0.0.1', port: 0 } })
await server.listen()
const report = { result: 'FAIL', qualification: 'Actual createAppRoutes defaults/WorldShell. Instrumented world fixture only, no Phaser/Auth/C04/C05 or product art qualification.', cases: [], errors: [] }
let browser
try {
 browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
 const base = `http://127.0.0.1:${server.httpServer.address().port}`
 for (const [width,height] of [[360,800],[390,844],[430,932]]) {
  const page = await browser.newPage({ viewport: { width,height }, reducedMotion: 'reduce', hasTouch: true })
  page.on('pageerror', e => report.errors.push(e.message))
  await page.goto(`${base}/src/app/__tests__/default-shell/preview.html`)
  await expect(page.getByRole('navigation', {name:'Main navigation'})).toBeVisible()
  await expect(page.getByText('Eras unavailable.')).toBeVisible()
  await page.screenshot({ path: resolve(evidence,`default-world-${width}.png`) })
  for (const label of ['Feed','Meet','Gifts','Wall','Me']) {
   await page.getByRole('navigation', {name:'Main navigation'}).getByRole('link', {name:label,exact:true}).click()
   await expect(page.getByRole('heading', {name:label,exact:true,level:1})).toBeVisible()
   assert.equal((await page.evaluate(() => window.__DEFAULT_CHROME__.state())).mounts,1)
  }
  await page.getByRole('link', {name:'World',exact:true}).click()
  await page.getByRole('link', {name:'Inbox',exact:true}).click()
  await expect(page.getByRole('heading', {name:'Inbox',exact:true,level:1})).toBeVisible()
  await page.getByRole('button',{name:'Back to previous panel'}).click()
  await expect(page.getByRole('link',{name:'Inbox',exact:true})).toBeFocused()
  await page.getByRole('link', {name:'Settings',exact:true}).click()
  await expect(page.getByRole('heading',{name:'Settings',exact:true,level:1})).toBeVisible()
  await page.getByRole('button',{name:'Expand panel'}).click()
  assert.equal((await page.evaluate(() => window.__DEFAULT_CHROME__.state())).visible,false)
  await page.getByRole('button',{name:'Reduce panel'}).click()
  await page.screenshot({path:resolve(evidence,`default-settings-${width}.png`)})
  await page.setViewportSize({width,height:430})
  await expect(page.getByRole('navigation',{name:'Main navigation'})).toBeInViewport()
  const layout = await page.evaluate(() => ({ overflow:document.documentElement.scrollWidth>innerWidth, chrome:document.querySelectorAll('.mcr-chrome-header,.mcr-chrome-bottom').length }))
  assert.equal(layout.overflow,false); assert.equal(layout.chrome,2)
  await page.evaluate(() => window.__DEFAULT_CHROME__.dispose())
  const final = await page.evaluate(() => window.__DEFAULT_CHROME__.state())
  assert.equal(final.games,0);assert.equal(final.connections,0);assert.equal(final.disposals,1)
  report.cases.push({width,height,layout, mounts:final.mounts,disposals:final.disposals});await page.close()
 }
 assert.deepEqual(report.errors,[]);report.result='PASS'
} finally { await writeFile(resolve(evidence,'chrome-native.json'),JSON.stringify(report,null,2)+'\n'); await browser?.close();await server.close() }
